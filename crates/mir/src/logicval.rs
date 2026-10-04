use smallvec::SmallVec;
use std::fmt;
use std::ops::{BitAnd, BitOr, BitXor, Not};

/// 4-value logic (0/1/X/Z) using aval/bval 2-plane representation.
/// (a,b) = (0,0)->0, (1,0)->1, (0,1)->Z, (1,1)->X
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicVal {
    Small {
        width: u16,
        a: u64,
        b: u64,
    },
    Large {
        width: u32,
        a: SmallVec<[u64; 4]>,
        b: SmallVec<[u64; 4]>,
    },
}

// ── chunk helpers ─────────────────────────────────────────────────────────────

fn num_chunks(width: u32) -> usize {
    width.div_ceil(64) as usize
}

fn top_mask(width: u32) -> u64 {
    let b = width % 64;
    if b == 0 {
        u64::MAX
    } else {
        (1u64 << b) - 1
    }
}

fn chunk_mask(width: u32, idx: usize) -> u64 {
    let n = num_chunks(width);
    if idx == n - 1 {
        top_mask(width)
    } else {
        u64::MAX
    }
}

// ── impl ──────────────────────────────────────────────────────────────────────

impl LogicVal {
    pub const ZERO: LogicVal = LogicVal::Small {
        width: 1,
        a: 0,
        b: 0,
    };
    pub const ONE: LogicVal = LogicVal::Small {
        width: 1,
        a: 1,
        b: 0,
    };
    pub const X: LogicVal = LogicVal::Small {
        width: 1,
        a: 1,
        b: 1,
    };
    pub const Z: LogicVal = LogicVal::Small {
        width: 1,
        a: 0,
        b: 1,
    };

    pub fn width(&self) -> u32 {
        match self {
            LogicVal::Small { width, .. } => *width as u32,
            LogicVal::Large { width, .. } => *width,
        }
    }

    /// Construct from a single (a,b) pair; bits above chunk 0 are zero.
    pub fn new(width: u16, a: u64, b: u64) -> Self {
        if width <= 64 {
            let mask = if width == 64 {
                u64::MAX
            } else {
                (1u64 << width) - 1
            };
            LogicVal::Small {
                width,
                a: a & mask,
                b: b & mask,
            }
        } else {
            let mut av: SmallVec<[u64; 4]> = SmallVec::new();
            let mut bv: SmallVec<[u64; 4]> = SmallVec::new();
            av.push(a);
            bv.push(b);
            let n = num_chunks(width as u32);
            while av.len() < n {
                av.push(0);
            }
            while bv.len() < n {
                bv.push(0);
            }
            LogicVal::Large {
                width: width as u32,
                a: av,
                b: bv,
            }
        }
    }

    /// Build from chunk vectors (internal use; truncates/extends as needed).
    pub fn from_chunks(width: u32, a: &[u64], b: &[u64]) -> Self {
        let n = num_chunks(width);
        let tm = top_mask(width);
        if width <= 64 {
            let av = a.first().copied().unwrap_or(0) & tm;
            let bv = b.first().copied().unwrap_or(0) & tm;
            LogicVal::Small {
                width: width as u16,
                a: av,
                b: bv,
            }
        } else {
            let mut av: SmallVec<[u64; 4]> = SmallVec::new();
            let mut bv: SmallVec<[u64; 4]> = SmallVec::new();
            for i in 0..n {
                let m = if i == n - 1 { tm } else { u64::MAX };
                av.push(a.get(i).copied().unwrap_or(0) & m);
                bv.push(b.get(i).copied().unwrap_or(0) & m);
            }
            LogicVal::Large {
                width,
                a: av,
                b: bv,
            }
        }
    }

    // ── chunk accessors ───────────────────────────────────────────────────────

    fn get_chunk(&self, idx: usize) -> u64 {
        match self {
            LogicVal::Small { a, .. } => {
                if idx == 0 {
                    *a
                } else {
                    0
                }
            }
            LogicVal::Large { a, .. } => a.get(idx).copied().unwrap_or(0),
        }
    }

    fn get_chunk_b(&self, idx: usize) -> u64 {
        match self {
            LogicVal::Small { b, .. } => {
                if idx == 0 {
                    *b
                } else {
                    0
                }
            }
            LogicVal::Large { b, .. } => b.get(idx).copied().unwrap_or(0),
        }
    }

    // ── scalar property tests ─────────────────────────────────────────────────

    pub fn is_zero(&self) -> bool {
        let n = num_chunks(self.width());
        for i in 0..n {
            let m = chunk_mask(self.width(), i);
            if (self.get_chunk(i) | self.get_chunk_b(i)) & m != 0 {
                return false;
            }
        }
        true
    }

    /// 値が論理的に真かどうか（IEEE 1364の if 文・while 文・&&・||の短絡規則:
    /// 既知の1ビットが1つでもあれば真。それ以外（全既知0、またはXを含み既知1
    /// ビットが無い）は偽として扱う）。
    pub fn is_true(&self) -> bool {
        let n = num_chunks(self.width());
        for i in 0..n {
            let m = chunk_mask(self.width(), i);
            // 既知の1ビット = a=1 かつ b=0
            if (self.get_chunk(i) & !self.get_chunk_b(i)) & m != 0 {
                return true;
            }
        }
        false
    }

    pub fn is_known(&self) -> bool {
        let n = num_chunks(self.width());
        for i in 0..n {
            let m = chunk_mask(self.width(), i);
            if self.get_chunk_b(i) & m != 0 {
                return false;
            }
        }
        true
    }

    pub fn is_one(&self) -> bool {
        if !self.is_known() {
            return false;
        }
        let n = num_chunks(self.width());
        for i in 0..n {
            let m = chunk_mask(self.width(), i);
            if self.get_chunk(i) & m != m {
                return false;
            }
        }
        true
    }

    pub fn is_z(&self) -> bool {
        // Every set bit must be in b (Z), none in a
        let n = num_chunks(self.width());
        let mut any_b = false;
        for i in 0..n {
            let m = chunk_mask(self.width(), i);
            let a = self.get_chunk(i) & m;
            let b = self.get_chunk_b(i) & m;
            if a != 0 {
                return false;
            }
            if b != 0 {
                any_b = true;
            }
        }
        any_b
    }

    pub fn is_x(&self) -> bool {
        // Every set bit must satisfy a==b (X), and no pure-Z bit
        let n = num_chunks(self.width());
        let mut any_b = false;
        for i in 0..n {
            let m = chunk_mask(self.width(), i);
            let a = self.get_chunk(i) & m;
            let b = self.get_chunk_b(i) & m;
            if a != b {
                return false;
            }
            if b != 0 {
                any_b = true;
            }
        }
        any_b
    }

    // ── low-64 extraction (used for index/shift amounts) ──────────────────────

    pub fn pad_to_width(&self, width: u32) -> u64 {
        let mask = if width >= 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        self.get_chunk(0) & mask
    }

    pub fn pad_to_width_b(&self, width: u32) -> u64 {
        let mask = if width >= 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        self.get_chunk_b(0) & mask
    }

    // ── zero extension / sign extension ──────────────────────────────────────

    /// Truncates or zero-extends to `width`, regardless of whether it's wider or narrower
    /// than the current value. Used when storing into a net of a fixed declared width.
    pub fn resize(&self, width: u32) -> Self {
        let n = num_chunks(width);
        let mut av: SmallVec<[u64; 4]> = SmallVec::new();
        let mut bv: SmallVec<[u64; 4]> = SmallVec::new();
        for i in 0..n {
            av.push(self.get_chunk(i));
            bv.push(self.get_chunk_b(i));
        }
        LogicVal::from_chunks(width, &av, &bv)
    }

    pub fn extend_zero(&self, new_width: u32) -> Self {
        let n = num_chunks(new_width);
        let mut a: Vec<u64> = (0..n).map(|i| self.get_chunk(i)).collect();
        let mut b: Vec<u64> = (0..n).map(|i| self.get_chunk_b(i)).collect();
        // clear bits above old width in the chunk containing old MSB
        if self.width() < new_width {
            let old_n = num_chunks(self.width());
            if old_n > 0 {
                let tm = top_mask(self.width());
                a[old_n - 1] &= tm;
                b[old_n - 1] &= tm;
            }
        }
        Self::from_chunks(new_width, &a, &b)
    }

    // ── equality / comparison ─────────────────────────────────────────────────

    pub fn eq(&self, rhs: &LogicVal) -> LogicVal {
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::X;
        }
        let w = self.width().max(rhs.width());
        let n = num_chunks(w);
        for i in 0..n {
            let m = chunk_mask(w, i);
            if (self.get_chunk(i) & m) != (rhs.get_chunk(i) & m) {
                return LogicVal::ZERO;
            }
        }
        LogicVal::ONE
    }

    pub fn ne(&self, rhs: &LogicVal) -> LogicVal {
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::X;
        }
        match self.eq(rhs) {
            LogicVal::ONE => LogicVal::ZERO,
            LogicVal::ZERO => LogicVal::ONE,
            other => other,
        }
    }

    pub fn case_eq(&self, rhs: &LogicVal) -> LogicVal {
        let w = self.width().max(rhs.width());
        let n = num_chunks(w);
        for i in 0..n {
            let m = chunk_mask(w, i);
            if (self.get_chunk(i) & m) != (rhs.get_chunk(i) & m) {
                return LogicVal::ZERO;
            }
            if (self.get_chunk_b(i) & m) != (rhs.get_chunk_b(i) & m) {
                return LogicVal::ZERO;
            }
        }
        LogicVal::ONE
    }

    /// casez比較。両辺のZビットをdon't careとして除外し、残りを厳密比較する。
    pub fn casez_eq(&self, rhs: &LogicVal) -> LogicVal {
        self.case_wild_eq(rhs, false)
    }

    /// casex比較。両辺のXまたはZビットをdon't careとして除外し、残りを厳密比較する。
    pub fn casex_eq(&self, rhs: &LogicVal) -> LogicVal {
        self.case_wild_eq(rhs, true)
    }

    /// ワイルドカード位置を除いたa/b両平面を比較する。
    /// `x_is_wildcard`がfalseならZのみ、trueならX/Zをワイルドカードにする。
    fn case_wild_eq(&self, rhs: &LogicVal, x_is_wildcard: bool) -> LogicVal {
        let w = self.width().max(rhs.width());
        let n = num_chunks(w);
        for i in 0..n {
            let m = chunk_mask(w, i);
            let la = self.get_chunk(i) & m;
            let lb = self.get_chunk_b(i) & m;
            let ra = rhs.get_chunk(i) & m;
            let rb = rhs.get_chunk_b(i) & m;
            // casexではb=1のX/Zを、casezではa=0かつb=1のZを除外する。
            let ld = if x_is_wildcard { lb } else { lb & !la };
            let rd = if x_is_wildcard { rb } else { rb & !ra };
            let compared = m & !(ld | rd);
            if ((la ^ ra) | (lb ^ rb)) & compared != 0 {
                return LogicVal::ZERO;
            }
        }
        LogicVal::ONE
    }

    pub fn case_ne(&self, rhs: &LogicVal) -> LogicVal {
        match self.case_eq(rhs) {
            LogicVal::ONE => LogicVal::ZERO,
            LogicVal::ZERO => LogicVal::ONE,
            other => other,
        }
    }

    // Multi-word unsigned comparison helper
    fn cmp_unsigned(&self, rhs: &LogicVal) -> Option<std::cmp::Ordering> {
        if !self.is_known() || !rhs.is_known() {
            return None;
        }
        let w = self.width().max(rhs.width());
        let n = num_chunks(w);
        // Compare from MSB chunk down
        for i in (0..n).rev() {
            let m = chunk_mask(w, i);
            let la = self.get_chunk(i) & m;
            let ra = rhs.get_chunk(i) & m;
            match la.cmp(&ra) {
                std::cmp::Ordering::Equal => continue,
                ord => return Some(ord),
            }
        }
        Some(std::cmp::Ordering::Equal)
    }

    pub fn lt(&self, rhs: &LogicVal) -> LogicVal {
        match self.cmp_unsigned(rhs) {
            None => LogicVal::X,
            Some(std::cmp::Ordering::Less) => LogicVal::ONE,
            _ => LogicVal::ZERO,
        }
    }

    pub fn gt(&self, rhs: &LogicVal) -> LogicVal {
        rhs.lt(self)
    }

    pub fn le(&self, rhs: &LogicVal) -> LogicVal {
        match self.cmp_unsigned(rhs) {
            None => LogicVal::X,
            Some(std::cmp::Ordering::Greater) => LogicVal::ZERO,
            _ => LogicVal::ONE,
        }
    }

    pub fn ge(&self, rhs: &LogicVal) -> LogicVal {
        rhs.le(self)
    }

    // ── arithmetic ────────────────────────────────────────────────────────────

    pub fn add(&self, rhs: &LogicVal) -> LogicVal {
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::X;
        }
        let w = self.width().max(rhs.width());
        let n = num_chunks(w);
        let mut a: Vec<u64> = Vec::with_capacity(n);
        let mut carry = 0u64;
        for i in 0..n {
            let m = chunk_mask(w, i);
            let la = self.get_chunk(i) & m;
            let ra = rhs.get_chunk(i) & m;
            let (s1, c1) = la.overflowing_add(ra);
            let (s2, c2) = s1.overflowing_add(carry);
            carry = (c1 as u64) + (c2 as u64);
            a.push(s2 & m);
        }
        let b = vec![0u64; n];
        Self::from_chunks(w, &a, &b)
    }

    pub fn sub(&self, rhs: &LogicVal) -> LogicVal {
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::X;
        }
        let w = self.width().max(rhs.width());
        let n = num_chunks(w);
        let mut a: Vec<u64> = Vec::with_capacity(n);
        let mut borrow = 0u64;
        for i in 0..n {
            let m = chunk_mask(w, i);
            let la = self.get_chunk(i) & m;
            let ra = rhs.get_chunk(i) & m;
            let (d1, b1) = la.overflowing_sub(ra);
            let (d2, b2) = d1.overflowing_sub(borrow);
            borrow = (b1 as u64) + (b2 as u64);
            a.push(d2 & m);
        }
        let b = vec![0u64; n];
        Self::from_chunks(w, &a, &b)
    }

    /// `lo` から `sel_w` ビットを `val` で置き換えた値を返す（任意幅）。`val` は `sel_w` へゼロ拡張/切り詰め、
    /// 範囲がネット幅を超える部分は無視する。
    pub fn insert_bits(&self, lo: u32, sel_w: u32, val: &LogicVal) -> LogicVal {
        let w = self.width();
        let n = num_chunks(w);
        let mut a: Vec<u64> = (0..n).map(|i| self.get_chunk(i)).collect();
        let mut b: Vec<u64> = (0..n).map(|i| self.get_chunk_b(i)).collect();
        for k in 0..sel_w {
            let pos = lo + k;
            if pos >= w {
                break;
            }
            let (va, vb) = val.bit_ab(k);
            let (c, o) = ((pos / 64) as usize, pos % 64);
            a[c] = (a[c] & !(1u64 << o)) | (va << o);
            b[c] = (b[c] & !(1u64 << o)) | (vb << o);
        }
        Self::from_chunks(w, &a, &b)
    }

    /// 2つのドライバ値をビット単位で解決する（wire/tri）。Zは中立、同値ならその値、
    /// 不一致（0と1、またはいずれかがX）はX。幅は広い方に揃える。
    pub fn resolve(&self, other: &LogicVal) -> LogicVal {
        let w = self.width().max(other.width());
        let n = num_chunks(w);
        let (mut a, mut b) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for i in 0..n {
            let (a1, b1) = (self.get_chunk(i), self.get_chunk_b(i));
            let (a2, b2) = (other.get_chunk(i), other.get_chunk_b(i));
            let z1 = !a1 & b1;
            let z2 = !a2 & b2;
            let equal = !((a1 ^ a2) | (b1 ^ b2));
            let conflict = !z1 & !z2 & !equal;
            // 自身がZなら相手の値、そうでなければ自身の値（相手がZのときも自身の値）
            a.push((z1 & a2) | (!z1 & a1) | conflict);
            b.push((z1 & b2) | (!z1 & b1) | conflict);
        }
        Self::from_chunks(w, &a, &b)
    }

    /// wand/triand 用の解決: Zは中立、0が支配（0 > X > 1）。
    pub fn resolve_wand(&self, other: &LogicVal) -> LogicVal {
        self.resolve_dominant(other, false)
    }

    /// wor/trior 用の解決: Zは中立、1が支配（1 > X > 0）。
    pub fn resolve_wor(&self, other: &LogicVal) -> LogicVal {
        self.resolve_dominant(other, true)
    }

    fn resolve_dominant(&self, other: &LogicVal, one_dominates: bool) -> LogicVal {
        let w = self.width().max(other.width());
        let n = num_chunks(w);
        let (mut a, mut b) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for i in 0..n {
            let (a1, b1) = (self.get_chunk(i), self.get_chunk_b(i));
            let (a2, b2) = (other.get_chunk(i), other.get_chunk_b(i));
            let (z1, z2) = (!a1 & b1, !a2 & b2);
            let known = !z1 & !z2;
            let (x1, x2) = (a1 & b1, a2 & b2);
            let (zero1, zero2) = (!a1 & !b1, !a2 & !b2);
            let (one1, one2) = (a1 & !b1, a2 & !b2);
            let (dom, xx) = if one_dominates {
                let d = one1 | one2;
                (d, (x1 | x2) & !d)
            } else {
                let d = zero1 | zero2;
                (d, (x1 | x2) & !d)
            };
            // 支配値が1(wor)なら a=1、0(wand)なら a=0。支配値が無くXも無ければ非支配値(wor:0, wand:1)。
            let ka = if one_dominates { dom | xx } else { !dom };
            a.push((z1 & a2) | (!z1 & z2 & a1) | (known & ka));
            b.push((z1 & b2) | (!z1 & z2 & b1) | (known & xx));
        }
        Self::from_chunks(w, &a, &b)
    }

    /// Zのビットを `one` の値（pullup=1 / pulldown=0）へ置き換える。
    pub fn pull_z(&self, one: bool) -> LogicVal {
        self.pull_z_range(0, self.width(), one)
    }

    /// `lo` から `w` ビットの範囲内のZを `one` の値へ置き換える。
    pub fn pull_z_range(&self, lo: u32, w: u32, one: bool) -> LogicVal {
        let n = num_chunks(self.width());
        let (mut a, mut b) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for i in 0..n {
            let (ai, bi) = (self.get_chunk(i), self.get_chunk_b(i));
            let mut m = 0u64;
            for bit in 0..64u32 {
                let pos = i as u32 * 64 + bit;
                if pos >= lo && pos < lo + w {
                    m |= 1u64 << bit;
                }
            }
            let z = !ai & bi & m;
            a.push(if one { ai | z } else { ai });
            b.push(bi & !z);
        }
        Self::from_chunks(self.width(), &a, &b)
    }

    /// Zのビットを `prev` の対応ビットで置き換える（trireg: 全ドライバZなら直前の値を保持）。
    pub fn keep_on_z(&self, prev: &LogicVal) -> LogicVal {
        let n = num_chunks(self.width());
        let (mut a, mut b) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for i in 0..n {
            let (ai, bi) = (self.get_chunk(i), self.get_chunk_b(i));
            let (pa, pb) = (prev.get_chunk(i), prev.get_chunk_b(i));
            let z = !ai & bi;
            a.push((ai & !z) | (z & pa));
            b.push((bi & !z) | (z & pb));
        }
        Self::from_chunks(self.width(), &a, &b)
    }

    /// 全ビットが `one` の値（任意幅）。
    pub fn all_bits(width: u32, one: bool) -> LogicVal {
        let n = num_chunks(width);
        let fill = if one { u64::MAX } else { 0 };
        Self::from_chunks(width, &vec![fill; n], &vec![0u64; n])
    }

    /// 全ビットZの値（任意幅）。
    pub fn z_of_width(width: u32) -> LogicVal {
        let n = num_chunks(width);
        Self::from_chunks(width, &vec![0u64; n], &vec![u64::MAX; n])
    }

    /// ビット位置 `idx` の (a, b) プレーン値（各0/1）。範囲外は (0, 0)。
    pub fn bit_ab(&self, idx: u32) -> (u64, u64) {
        if idx >= self.width() {
            return (0, 0);
        }
        let (c, o) = ((idx / 64) as usize, idx % 64);
        ((self.get_chunk(c) >> o) & 1, (self.get_chunk_b(c) >> o) & 1)
    }

    /// 全ビット既知の値を10進文字列にする（任意幅）。X/Zを含む場合は `None`。
    /// `signed` かつMSBが1なら二の補数を負数として扱う。
    pub fn to_decimal_string(&self, signed: bool) -> Option<String> {
        if !self.is_known() {
            return None;
        }
        let w = self.width();
        let mut chunks = self.to_chunks(w);
        let neg = signed && self.sign_bit() == 1;
        if neg {
            chunks = Self::neg_chunks(&chunks, w);
        }
        let mut digits = Vec::new();
        while chunks.iter().any(|c| *c != 0) {
            let mut rem = 0u128;
            for c in chunks.iter_mut().rev() {
                let cur = (rem << 64) | *c as u128;
                *c = (cur / 10) as u64;
                rem = cur % 10;
            }
            digits.push(b'0' + rem as u8);
        }
        if digits.is_empty() {
            digits.push(b'0');
        }
        if neg {
            digits.push(b'-');
        }
        digits.reverse();
        String::from_utf8(digits).ok()
    }

    /// 幅 `w` の値を符号なしチャンク列として取り出す（未使用上位ビットは0）。
    fn to_chunks(&self, w: u32) -> Vec<u64> {
        (0..num_chunks(w))
            .map(|i| self.get_chunk(i) & chunk_mask(w, i))
            .collect()
    }

    /// チャンク列の2の補数（幅 `w` で切り詰め）。
    fn neg_chunks(a: &[u64], w: u32) -> Vec<u64> {
        let mut carry = 1u64;
        (0..a.len())
            .map(|i| {
                let (s, c) = (!a[i]).overflowing_add(carry);
                carry = c as u64;
                s & chunk_mask(w, i)
            })
            .collect()
    }

    /// 符号なし multi-word 乗算（幅 `w` へ切り詰め）。
    fn mul_chunks(a: &[u64], b: &[u64], w: u32) -> Vec<u64> {
        let n = num_chunks(w);
        let mut r = vec![0u64; n];
        for i in 0..n {
            let mut carry = 0u128;
            for j in 0..(n - i) {
                let cur = r[i + j] as u128 + (a[i] as u128) * (b[j] as u128) + carry;
                r[i + j] = cur as u64;
                carry = cur >> 64;
            }
        }
        for (i, c) in r.iter_mut().enumerate() {
            *c &= chunk_mask(w, i);
        }
        r
    }

    /// 符号なし multi-word 除算（shift-subtract 長除算）。除数は非0であること。
    fn divmod_chunks(a: &[u64], b: &[u64], w: u32) -> (Vec<u64>, Vec<u64>) {
        let n = num_chunks(w);
        let mut q = vec![0u64; n];
        let mut r = vec![0u64; n];
        for bit in (0..w as usize).rev() {
            // r = (r << 1) | a[bit]
            let mut carry = (a[bit / 64] >> (bit % 64)) & 1;
            for c in r.iter_mut() {
                let next = *c >> 63;
                *c = (*c << 1) | carry;
                carry = next;
            }
            // r >= b なら r -= b、商のビットを立てる
            let ge = (0..n)
                .rev()
                .map(|i| r[i].cmp(&b[i]))
                .find(|o| !o.is_eq())
                .is_none_or(|o| o.is_gt());
            if ge {
                let mut borrow = 0u64;
                for i in 0..n {
                    let (d1, b1) = r[i].overflowing_sub(b[i]);
                    let (d2, b2) = d1.overflowing_sub(borrow);
                    r[i] = d2;
                    borrow = (b1 | b2) as u64;
                }
                q[bit / 64] |= 1u64 << (bit % 64);
            }
        }
        (q, r)
    }

    /// 64bit超の符号なし除算・剰余の共通処理。`want_rem` で剰余/商を選ぶ。
    fn wide_divmod(&self, rhs: &LogicVal, w: u32, want_rem: bool) -> LogicVal {
        let (a, b) = (self.to_chunks(w), rhs.to_chunks(w));
        if b.iter().all(|c| *c == 0) {
            return LogicVal::x_of_width(w);
        }
        let (q, r) = Self::divmod_chunks(&a, &b, w);
        Self::from_chunks(w, if want_rem { &r } else { &q }, &vec![0u64; q.len()])
    }

    /// 64bit超の符号付き除算・剰余（0への切り捨て、剰余の符号は被除数側）。
    fn wide_divmod_signed(&self, rhs: &LogicVal, w: u32, want_rem: bool) -> LogicVal {
        let (l, r) = (self.extend_sign(w), rhs.extend_sign(w));
        let (ln, rn) = (l.sign_bit() == 1, r.sign_bit() == 1);
        let abs = |v: &LogicVal, neg: bool| {
            let c = v.to_chunks(w);
            if neg {
                Self::neg_chunks(&c, w)
            } else {
                c
            }
        };
        let (a, b) = (abs(&l, ln), abs(&r, rn));
        if b.iter().all(|c| *c == 0) {
            return LogicVal::x_of_width(w);
        }
        let (q, rem) = Self::divmod_chunks(&a, &b, w);
        let (res, neg) = if want_rem { (rem, ln) } else { (q, ln != rn) };
        let res = if neg { Self::neg_chunks(&res, w) } else { res };
        Self::from_chunks(w, &res, &vec![0u64; res.len()])
    }

    pub fn mul(&self, rhs: &LogicVal) -> LogicVal {
        let w = self.width().max(rhs.width());
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::x_of_width(w);
        }
        if w > 64 {
            let r = Self::mul_chunks(&self.to_chunks(w), &rhs.to_chunks(w), w);
            return Self::from_chunks(w, &r, &vec![0u64; r.len()]);
        }
        let la = self.pad_to_width(w);
        let ra = rhs.pad_to_width(w);
        Self::from_chunks(w, &[la.wrapping_mul(ra)], &[0])
    }

    pub fn div(&self, rhs: &LogicVal) -> LogicVal {
        let w = self.width().max(rhs.width());
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::x_of_width(w);
        }
        if w > 64 {
            return self.wide_divmod(rhs, w, false);
        }
        let ra = rhs.pad_to_width(w);
        if ra == 0 {
            return LogicVal::x_of_width(w);
        }
        Self::from_chunks(w, &[self.pad_to_width(w) / ra], &[0])
    }

    pub fn mod_(&self, rhs: &LogicVal) -> LogicVal {
        let w = self.width().max(rhs.width());
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::x_of_width(w);
        }
        if w > 64 {
            return self.wide_divmod(rhs, w, true);
        }
        let ra = rhs.pad_to_width(w);
        if ra == 0 {
            return LogicVal::x_of_width(w);
        }
        Self::from_chunks(w, &[self.pad_to_width(w) % ra], &[0])
    }

    // ── logical operators ─────────────────────────────────────────────────────

    pub fn log_and(&self, rhs: &LogicVal) -> LogicVal {
        if self.is_zero() || rhs.is_zero() {
            return LogicVal::ZERO;
        }
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::X;
        }
        LogicVal::ONE
    }

    pub fn log_or(&self, rhs: &LogicVal) -> LogicVal {
        if self.is_true() || rhs.is_true() {
            return LogicVal::ONE;
        }
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::X;
        }
        LogicVal::ZERO
    }

    pub fn log_not(&self) -> LogicVal {
        if !self.is_known() {
            return LogicVal::X;
        }
        if self.is_zero() {
            LogicVal::ONE
        } else {
            LogicVal::ZERO
        }
    }

    pub fn cond(&self, true_val: &LogicVal, false_val: &LogicVal) -> LogicVal {
        if !self.is_known() {
            return true_val.merge_unknown(false_val);
        }
        if self.is_zero() {
            false_val.clone()
        } else {
            true_val.clone()
        }
    }

    /// 条件がX/Zの三項演算子の結果（IEEE 1364 5.1.13）: 両枝をビット単位で比較し、
    /// 同じ値のビットはそのまま、異なるビットはXにする。幅は広い方に揃える。
    pub fn merge_unknown(&self, other: &LogicVal) -> LogicVal {
        let w = self.width().max(other.width());
        let n = num_chunks(w);
        let (mut a, mut b) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for i in 0..n {
            let (a1, b1) = (self.get_chunk(i), self.get_chunk_b(i));
            let (a2, b2) = (other.get_chunk(i), other.get_chunk_b(i));
            let diff = (a1 ^ a2) | (b1 ^ b2);
            a.push(a1 | diff);
            b.push(b1 | diff);
        }
        Self::from_chunks(w, &a, &b)
    }

    // ── shift operators ───────────────────────────────────────────────────────

    pub fn shl(&self, rhs: &LogicVal) -> LogicVal {
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::X;
        }
        let shift = rhs.pad_to_width(32) as u32;
        let w = self.width();
        if shift >= w {
            return Self::from_chunks(w, &[0], &[0]);
        }
        let n = num_chunks(w);
        let chunk_shift = (shift / 64) as usize;
        let bit_shift = shift % 64;
        let mut a = vec![0u64; n];
        for (i, value) in a.iter_mut().enumerate().skip(chunk_shift) {
            let src = i - chunk_shift;
            let m = chunk_mask(w, i);
            let lo = self.get_chunk(src) << bit_shift;
            let hi = if bit_shift > 0 && src > 0 {
                self.get_chunk(src - 1) >> (64 - bit_shift)
            } else {
                0
            };
            *value = (lo | hi) & m;
        }
        Self::from_chunks(w, &a, &vec![0u64; n])
    }

    pub fn shr(&self, rhs: &LogicVal) -> LogicVal {
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::X;
        }
        let shift = rhs.pad_to_width(32) as u32;
        let w = self.width();
        if shift >= w {
            return Self::from_chunks(w, &[0], &[0]);
        }
        let n = num_chunks(w);
        let chunk_shift = (shift / 64) as usize;
        let bit_shift = shift % 64;
        let mut a = vec![0u64; n];
        for (i, value) in a.iter_mut().enumerate().take(n - chunk_shift) {
            let src = i + chunk_shift;
            let sm = chunk_mask(w, src);
            let lo = (self.get_chunk(src) & sm) >> bit_shift;
            let hi = if bit_shift > 0 && src + 1 < n {
                let sm2 = chunk_mask(w, src + 1);
                (self.get_chunk(src + 1) & sm2) << (64 - bit_shift)
            } else {
                0
            };
            *value = lo | hi;
        }
        Self::from_chunks(w, &a, &vec![0u64; n])
    }

    pub fn ashl(&self, rhs: &LogicVal) -> Self {
        self.shl(rhs)
    }

    /// 値自身のMSB（aval）を符号ビットとして返す。幅0のときは0。
    fn sign_bit(&self) -> u64 {
        let w = self.width();
        if w == 0 {
            return 0;
        }
        let n = num_chunks(w);
        let sign_bit_pos = if w.is_multiple_of(64) {
            63
        } else {
            (w % 64) - 1
        };
        (self.get_chunk(n - 1) >> sign_bit_pos) & 1
    }

    /// `new_width` へ符号拡張する（自身のMSBで埋める）。`new_width <= width()` の場合は `resize` と同じ。
    /// signed 演算（比較・除算・剰余・`>>>`）の共通境界を揃えるために使う。
    pub fn extend_sign(&self, new_width: u32) -> Self {
        let old_w = self.width();
        if new_width <= old_w {
            return self.resize(new_width);
        }
        let fill = if self.sign_bit() == 1 { u64::MAX } else { 0u64 };
        let n = num_chunks(new_width);
        let old_n = num_chunks(old_w);
        let mut a: Vec<u64> = Vec::with_capacity(n);
        let mut b: Vec<u64> = Vec::with_capacity(n);
        for i in 0..n {
            if i < old_n {
                let mut av = self.get_chunk(i);
                let bv = self.get_chunk_b(i);
                if i == old_n - 1 && !old_w.is_multiple_of(64) {
                    let tm = top_mask(old_w);
                    av = (av & tm) | (fill & !tm);
                }
                a.push(av);
                b.push(bv);
            } else {
                a.push(fill);
                b.push(0);
            }
        }
        Self::from_chunks(new_width, &a, &b)
    }

    /// 64bit以内の値を符号付き i64 として取り出す（signed 除算・剰余用）。
    fn as_i64(&self) -> i64 {
        self.extend_sign(64).get_chunk(0) as i64
    }

    // Multi-word signed comparison helper（両辺を共通幅へ符号拡張してから比較）
    fn cmp_signed(&self, rhs: &LogicVal) -> Option<std::cmp::Ordering> {
        if !self.is_known() || !rhs.is_known() {
            return None;
        }
        let w = self.width().max(rhs.width());
        let l = self.extend_sign(w);
        let r = rhs.extend_sign(w);
        let (ls, rs) = (l.sign_bit(), r.sign_bit());
        if ls != rs {
            return Some(if ls == 1 {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            });
        }
        l.cmp_unsigned(&r)
    }

    pub fn lt_signed(&self, rhs: &LogicVal) -> LogicVal {
        match self.cmp_signed(rhs) {
            None => LogicVal::X,
            Some(std::cmp::Ordering::Less) => LogicVal::ONE,
            _ => LogicVal::ZERO,
        }
    }

    pub fn gt_signed(&self, rhs: &LogicVal) -> LogicVal {
        rhs.lt_signed(self)
    }

    pub fn le_signed(&self, rhs: &LogicVal) -> LogicVal {
        match self.cmp_signed(rhs) {
            None => LogicVal::X,
            Some(std::cmp::Ordering::Greater) => LogicVal::ZERO,
            _ => LogicVal::ONE,
        }
    }

    pub fn ge_signed(&self, rhs: &LogicVal) -> LogicVal {
        rhs.le_signed(self)
    }

    /// signed 除算（0への切り捨て）。
    pub fn div_signed(&self, rhs: &LogicVal) -> LogicVal {
        let w = self.width().max(rhs.width());
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::x_of_width(w);
        }
        if w > 64 {
            return self.wide_divmod_signed(rhs, w, false);
        }
        let rv = rhs.as_i64();
        if rv == 0 {
            return LogicVal::x_of_width(w);
        }
        let result = self.as_i64().wrapping_div(rv);
        Self::from_chunks(w, &[result as u64], &[0])
    }

    /// signed 剰余（結果の符号は被除数側、Rust の `%` と同じ切り捨て規則）。
    pub fn mod_signed(&self, rhs: &LogicVal) -> LogicVal {
        let w = self.width().max(rhs.width());
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::x_of_width(w);
        }
        if w > 64 {
            return self.wide_divmod_signed(rhs, w, true);
        }
        let rv = rhs.as_i64();
        if rv == 0 {
            return LogicVal::x_of_width(w);
        }
        let result = self.as_i64().wrapping_rem(rv);
        Self::from_chunks(w, &[result as u64], &[0])
    }

    pub fn ashr(&self, rhs: &LogicVal) -> LogicVal {
        if !self.is_known() || !rhs.is_known() {
            return LogicVal::X;
        }
        let shift = rhs.pad_to_width(32) as u32;
        let w = self.width();
        let n = num_chunks(w);
        let sign = self.sign_bit();
        if sign == 0 {
            return self.shr(rhs);
        }
        // Fill with 1s from MSB
        if shift >= w {
            let fill = top_mask(w);
            let mut a = vec![u64::MAX; n];
            *a.last_mut().unwrap() &= fill;
            return Self::from_chunks(w, &a, &vec![0u64; n]);
        }
        let mut result = self.shr(rhs);
        // OR in sign-extension bits above (w - shift)
        let fill_from = w - shift;
        let fc = (fill_from / 64) as usize;
        let fb = fill_from % 64;
        for i in fc..n {
            let m = chunk_mask(w, i);
            let fill_mask = if i == fc {
                if fb == 0 {
                    u64::MAX
                } else {
                    !((1u64 << fb) - 1)
                }
            } else {
                u64::MAX
            };
            let cur = result.get_chunk(i);
            let new_val = (cur | fill_mask) & m;
            match &mut result {
                LogicVal::Small { a, .. } => {
                    if i == 0 {
                        *a = new_val;
                    }
                }
                LogicVal::Large { a, .. } => {
                    if let Some(v) = a.get_mut(i) {
                        *v = new_val;
                    }
                }
            }
        }
        result
    }

    // ── reduction operators ───────────────────────────────────────────────────

    pub fn reduce_and(&self) -> LogicVal {
        if !self.is_known() {
            return LogicVal::X;
        }
        let n = num_chunks(self.width());
        for i in 0..n {
            let m = chunk_mask(self.width(), i);
            if self.get_chunk(i) & m != m {
                return LogicVal::ZERO;
            }
        }
        LogicVal::ONE
    }

    pub fn reduce_or(&self) -> LogicVal {
        if !self.is_known() {
            return LogicVal::X;
        }
        let n = num_chunks(self.width());
        for i in 0..n {
            let m = chunk_mask(self.width(), i);
            if self.get_chunk(i) & m != 0 {
                return LogicVal::ONE;
            }
        }
        LogicVal::ZERO
    }

    pub fn reduce_xor(&self) -> LogicVal {
        if !self.is_known() {
            return LogicVal::X;
        }
        let n = num_chunks(self.width());
        let mut result = 0u64;
        for i in 0..n {
            let m = chunk_mask(self.width(), i);
            let mut val = self.get_chunk(i) & m;
            while val > 0 {
                result ^= val & 1;
                val >>= 1;
            }
        }
        if result & 1 == 1 {
            LogicVal::ONE
        } else {
            LogicVal::ZERO
        }
    }

    pub fn reduce_nand(&self) -> LogicVal {
        self.reduce_and().not()
    }
    pub fn reduce_nor(&self) -> LogicVal {
        self.reduce_or().not()
    }
    pub fn reduce_xnor(&self) -> LogicVal {
        self.reduce_xor().not()
    }

    // ── concatenation / repeat ────────────────────────────────────────────────

    /// `{self, rhs}` — self is MSB part, rhs is LSB part
    pub fn concat(&self, rhs: &LogicVal) -> LogicVal {
        let total = self.width() + rhs.width();
        let n = num_chunks(total);
        let mut a = vec![0u64; n];
        let mut b = vec![0u64; n];
        // Place rhs in low bits
        let rn = num_chunks(rhs.width());
        for i in 0..rn {
            let m = chunk_mask(rhs.width(), i);
            a[i] |= rhs.get_chunk(i) & m;
            b[i] |= rhs.get_chunk_b(i) & m;
        }
        // Place self shifted left by rhs.width()
        let shift = rhs.width();
        let sn = num_chunks(self.width());
        let chunk_shift = (shift / 64) as usize;
        let bit_shift = shift % 64;
        for i in 0..sn {
            let sm = chunk_mask(self.width(), i);
            let av = self.get_chunk(i) & sm;
            let bv = self.get_chunk_b(i) & sm;
            let dst = i + chunk_shift;
            if dst < n {
                let dm = chunk_mask(total, dst);
                a[dst] |= (av << bit_shift) & dm;
                b[dst] |= (bv << bit_shift) & dm;
            }
            if bit_shift > 0 && dst + 1 < n {
                let dm = chunk_mask(total, dst + 1);
                a[dst + 1] |= (av >> (64 - bit_shift)) & dm;
                b[dst + 1] |= (bv >> (64 - bit_shift)) & dm;
            }
        }
        Self::from_chunks(total, &a, &b)
    }

    pub fn repeat(&self, n: u32, value: &LogicVal) -> LogicVal {
        if n == 0 {
            return Self::from_chunks(0, &[], &[]);
        }
        let mut result = value.clone();
        for _ in 1..n {
            result = result.concat(value);
        }
        result
    }

    // ── bit/part select ───────────────────────────────────────────────────────

    fn get_bit(&self, idx: u32) -> LogicVal {
        if idx >= self.width() {
            return LogicVal::X;
        }
        let chunk = (idx / 64) as usize;
        let bit = idx % 64;
        let a = (self.get_chunk(chunk) >> bit) & 1;
        let b = (self.get_chunk_b(chunk) >> bit) & 1;
        LogicVal::Small { width: 1, a, b }
    }

    pub fn bit_select(&self, idx: u32) -> Result<LogicVal, &'static str> {
        if idx >= self.width() {
            return Err("bit index out of range");
        }
        Ok(self.get_bit(idx))
    }

    pub fn part_select(&self, left: u32, right: u32) -> Result<LogicVal, &'static str> {
        if left >= self.width() || left < right {
            return Err("invalid part select range");
        }
        let out_width = left - right + 1;
        let n = num_chunks(out_width);
        let mut a = vec![0u64; n];
        let mut b = vec![0u64; n];
        for bit in 0..out_width {
            let src_bit = bit + right;
            let src_chunk = (src_bit / 64) as usize;
            let src_pos = src_bit % 64;
            let dst_chunk = (bit / 64) as usize;
            let dst_pos = bit % 64;
            a[dst_chunk] |= ((self.get_chunk(src_chunk) >> src_pos) & 1) << dst_pos;
            b[dst_chunk] |= ((self.get_chunk_b(src_chunk) >> src_pos) & 1) << dst_pos;
        }
        Ok(Self::from_chunks(out_width, &a, &b))
    }

    /// 指定幅の全ビットX値を生成する（範囲外indexed part-selectのフォールバック用）
    pub fn x_of_width(width: u32) -> LogicVal {
        let n = num_chunks(width);
        Self::from_chunks(width, &vec![u64::MAX; n], &vec![u64::MAX; n])
    }
}

// ── Display ───────────────────────────────────────────────────────────────────

impl fmt::Display for LogicVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_known() {
            // Print as decimal for ≤64-bit known values
            if self.width() <= 64 {
                write!(f, "{}", self.pad_to_width(self.width()))
            } else {
                // Hex for large values
                let n = num_chunks(self.width());
                let mut parts: Vec<String> = (0..n)
                    .rev()
                    .map(|i| {
                        let m = chunk_mask(self.width(), i);
                        format!("{:016x}", self.get_chunk(i) & m)
                    })
                    .collect();
                // Trim leading zeros from first segment
                let first = parts[0].trim_start_matches('0');
                parts[0] = if first.is_empty() {
                    "0".to_string()
                } else {
                    first.to_string()
                };
                write!(f, "0x{}", parts.join(""))
            }
        } else if self.is_x() {
            write!(f, "x")
        } else if self.is_z() {
            write!(f, "z")
        } else {
            write!(f, "x") // mixed X/Z treated as X
        }
    }
}

// ── NOT ───────────────────────────────────────────────────────────────────────

impl Not for LogicVal {
    type Output = Self;
    // ~0=1, ~1=0, ~X=X, ~Z=X  →  a_out=(~a|b)&mask, b_out=b
    fn not(self) -> Self::Output {
        let w = self.width();
        let n = num_chunks(w);
        let mut a: SmallVec<[u64; 4]> = SmallVec::new();
        let mut b: SmallVec<[u64; 4]> = SmallVec::new();
        for i in 0..n {
            let m = chunk_mask(w, i);
            let ai = self.get_chunk(i) & m;
            let bi = self.get_chunk_b(i) & m;
            a.push(((!ai) | bi) & m);
            b.push(bi);
        }
        if w <= 64 {
            LogicVal::Small {
                width: w as u16,
                a: a[0],
                b: b[0],
            }
        } else {
            LogicVal::Large { width: w, a, b }
        }
    }
}

// ── AND ───────────────────────────────────────────────────────────────────────

impl BitAnd for LogicVal {
    type Output = Self;
    // a_out=(a1|b1)&(a2|b2), b_out=a_out&~((a1&~b1)&(a2&~b2))
    fn bitand(self, rhs: Self) -> Self::Output {
        let w = self.width().max(rhs.width());
        let n = num_chunks(w);
        let mut a: SmallVec<[u64; 4]> = SmallVec::new();
        let mut b: SmallVec<[u64; 4]> = SmallVec::new();
        for i in 0..n {
            let m = chunk_mask(w, i);
            let a1 = self.get_chunk(i) & m;
            let b1 = self.get_chunk_b(i) & m;
            let a2 = rhs.get_chunk(i) & m;
            let b2 = rhs.get_chunk_b(i) & m;
            let ao = (a1 | b1) & (a2 | b2);
            let bo = ao & !((a1 & !b1) & (a2 & !b2));
            a.push(ao);
            b.push(bo);
        }
        if w <= 64 {
            LogicVal::Small {
                width: w as u16,
                a: a[0],
                b: b[0],
            }
        } else {
            LogicVal::Large { width: w, a, b }
        }
    }
}

// ── OR ────────────────────────────────────────────────────────────────────────

impl BitOr for LogicVal {
    type Output = Self;
    // a_out=(a1|b1)|(a2|b2), b_out=a_out&(~a1|b1)&(~a2|b2)
    fn bitor(self, rhs: Self) -> Self::Output {
        let w = self.width().max(rhs.width());
        let n = num_chunks(w);
        let mut a: SmallVec<[u64; 4]> = SmallVec::new();
        let mut b: SmallVec<[u64; 4]> = SmallVec::new();
        for i in 0..n {
            let m = chunk_mask(w, i);
            let a1 = self.get_chunk(i) & m;
            let b1 = self.get_chunk_b(i) & m;
            let a2 = rhs.get_chunk(i) & m;
            let b2 = rhs.get_chunk_b(i) & m;
            let ao = (a1 | b1) | (a2 | b2);
            let bo = ao & ((!a1) | b1) & ((!a2) | b2);
            a.push(ao);
            b.push(bo);
        }
        if w <= 64 {
            LogicVal::Small {
                width: w as u16,
                a: a[0],
                b: b[0],
            }
        } else {
            LogicVal::Large { width: w, a, b }
        }
    }
}

// ── XOR ───────────────────────────────────────────────────────────────────────

impl BitXor for LogicVal {
    type Output = Self;
    // b_out=b1|b2, a_out=(a1^a2)|b_out
    fn bitxor(self, rhs: Self) -> Self::Output {
        let w = self.width().max(rhs.width());
        let n = num_chunks(w);
        let mut a: SmallVec<[u64; 4]> = SmallVec::new();
        let mut b: SmallVec<[u64; 4]> = SmallVec::new();
        for i in 0..n {
            let m = chunk_mask(w, i);
            let a1 = self.get_chunk(i) & m;
            let b1 = self.get_chunk_b(i) & m;
            let a2 = rhs.get_chunk(i) & m;
            let b2 = rhs.get_chunk_b(i) & m;
            let bo = b1 | b2;
            let ao = (a1 ^ a2) | bo;
            a.push(ao);
            b.push(bo);
        }
        if w <= 64 {
            LogicVal::Small {
                width: w as u16,
                a: a[0],
                b: b[0],
            }
        } else {
            LogicVal::Large { width: w, a, b }
        }
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logic_values() {
        assert!(LogicVal::ZERO.is_zero());
        assert!(LogicVal::ONE.is_one());
        assert!(LogicVal::Z.is_z());
        assert!(LogicVal::X.is_x());
    }

    #[test]
    fn test_bit_operations() {
        let a = LogicVal::new(4, 0b1010, 0);
        let b = LogicVal::new(4, 0b1100, 0);
        let and = a.clone() & b.clone();
        assert_eq!(and.pad_to_width(4), 0b1000);
        let or = a.clone() | b.clone();
        assert_eq!(or.pad_to_width(4), 0b1110);
        let xor = a.clone() ^ b.clone();
        assert_eq!(xor.pad_to_width(4), 0b0110);
    }

    #[test]
    fn test_not() {
        let a = LogicVal::new(4, 0b1010, 0);
        assert_eq!((!a).pad_to_width(4), 0b0101);
    }

    #[test]
    fn test_extension() {
        let a = LogicVal::new(4, 0b1010, 0);
        let e = a.extend_zero(8);
        assert_eq!(e.width(), 8);
        assert_eq!(e.pad_to_width(8), 0b00001010);
    }

    #[test]
    fn test_extend_sign_negative_fills_with_ones() {
        // 4bit -6 (0b1010) を 8bit へ符号拡張すると 0b11111010 (-6) になる
        let a = LogicVal::new(4, 0b1010, 0);
        let e = a.extend_sign(8);
        assert_eq!(e.width(), 8);
        assert_eq!(e.pad_to_width(8), 0b11111010);
    }

    #[test]
    fn test_extend_sign_positive_fills_with_zeros() {
        let a = LogicVal::new(4, 0b0101, 0); // +5
        let e = a.extend_sign(8);
        assert_eq!(e.pad_to_width(8), 0b00000101);
    }

    #[test]
    fn test_signed_comparison() {
        // 4bit: -1 (0b1111) と 1 (0b0001) は unsigned では -1 の方が大きいが signed では小さい
        let neg1 = LogicVal::new(4, 0b1111, 0);
        let pos1 = LogicVal::new(4, 0b0001, 0);
        assert!(neg1.lt_signed(&pos1).is_one());
        assert!(neg1.lt(&pos1).is_zero()); // unsigned は逆
        assert!(pos1.gt_signed(&neg1).is_one());
        assert!(neg1.le_signed(&neg1).is_one());
        assert!(neg1.ge_signed(&neg1).is_one());
    }

    #[test]
    fn test_signed_div_mod() {
        // 8bit: -7 / 2 = -3 (0への切り捨て), -7 % 2 = -1（被除数の符号）
        let neg7 = LogicVal::new(8, (-7i8) as u8 as u64, 0);
        let two = LogicVal::new(8, 2, 0);
        let q = neg7.div_signed(&two);
        assert_eq!(q.as_i64(), -3);
        let r = neg7.mod_signed(&two);
        assert_eq!(r.as_i64(), -1);
    }

    #[test]
    fn test_ashr_vs_shr_on_negative_bitpattern() {
        // 4bit 0b1000 (unsigned解釈なら8、signed解釈なら-8)
        let v = LogicVal::new(4, 0b1000, 0);
        let one = LogicVal::new(4, 1, 0);
        // shr (論理右シフト): unsigned オペランドとして常にこちらを使う
        assert_eq!(v.shr(&one).pad_to_width(4), 0b0100);
        // ashr (算術右シフト): signed オペランドの場合に使う。MSBで埋める
        assert_eq!(v.ashr(&one).pad_to_width(4), 0b1100);
    }

    #[test]
    fn test_large_add() {
        // 2^64 + 1 + 2^64 + 1 = 2^65 + 2
        let one64: u64 = 1;
        let a = LogicVal::from_chunks(128, &[one64, one64], &[0, 0]);
        let b = LogicVal::from_chunks(128, &[one64, one64], &[0, 0]);
        let s = a.add(&b);
        assert!(s.is_known());
        assert_eq!(s.get_chunk(0), 2);
        assert_eq!(s.get_chunk(1), 2);
    }

    #[test]
    fn test_large_concat() {
        let hi = LogicVal::new(4, 0b1010, 0); // 4-bit
        let lo = LogicVal::new(4, 0b0101, 0); // 4-bit
        let cat = hi.concat(&lo);
        assert_eq!(cat.width(), 8);
        assert_eq!(cat.pad_to_width(8), 0b10100101);
    }

    #[test]
    fn test_large_is_zero() {
        let v = LogicVal::from_chunks(128, &[0, 0], &[0, 0]);
        assert!(v.is_zero());
        let v2 = LogicVal::from_chunks(128, &[0, 1], &[0, 0]);
        assert!(!v2.is_zero());
    }

    #[test]
    fn test_large_eq() {
        let a = LogicVal::from_chunks(128, &[0xDEAD, 0xBEEF], &[0, 0]);
        let b = LogicVal::from_chunks(128, &[0xDEAD, 0xBEEF], &[0, 0]);
        let c = LogicVal::from_chunks(128, &[0xDEAD, 0xCAFE], &[0, 0]);
        assert_eq!(a.eq(&b), LogicVal::ONE);
        assert_eq!(a.eq(&c), LogicVal::ZERO);
    }

    #[test]
    fn test_casez_eq_wildcards_and_width() {
        // casezではセレクタ側・case項側いずれのZもワイルドカードになる。
        let selector_z = LogicVal::new(4, 0b1010, 0b0100);
        let pattern = LogicVal::new(4, 0b1110, 0);
        assert_eq!(selector_z.casez_eq(&pattern), LogicVal::ONE);

        let selector = LogicVal::new(4, 0b1010, 0);
        let pattern_z = LogicVal::new(4, 0b1000, 0b0010);
        assert_eq!(selector.casez_eq(&pattern_z), LogicVal::ONE);

        // Xはcasezのワイルドカードではない。
        let selector_x = LogicVal::new(4, 0b1010, 0b0010);
        assert_eq!(selector_x.casez_eq(&selector), LogicVal::ZERO);

        // 幅は既存のcase_eqと同様に狭い方をゼロ拡張して比較する。
        assert_eq!(
            LogicVal::new(2, 0b01, 0).casez_eq(&LogicVal::new(4, 0b0001, 0)),
            LogicVal::ONE
        );
        assert_eq!(
            LogicVal::new(2, 0b01, 0).casez_eq(&LogicVal::new(4, 0b0101, 0)),
            LogicVal::ZERO
        );
    }

    #[test]
    fn test_casex_eq_wildcards_and_large_values() {
        // casexではX/Zのどちらも両辺でワイルドカードになる。
        let selector_x = LogicVal::new(4, 0b1010, 0b0010);
        let pattern_z = LogicVal::new(4, 0b1000, 0b0100);
        assert_eq!(selector_x.casex_eq(&pattern_z), LogicVal::ONE);
        assert_eq!(selector_x.casez_eq(&pattern_z), LogicVal::ZERO);

        // Largeでも上位チャンクのワイルドカードを除外して比較する。
        let large_x = LogicVal::from_chunks(128, &[0, 1], &[0, 1]);
        let large_one = LogicVal::from_chunks(128, &[0, 1], &[0, 0]);
        assert_eq!(large_x.casex_eq(&large_one), LogicVal::ONE);
        assert_eq!(large_x.casez_eq(&large_one), LogicVal::ZERO);
    }

    #[test]
    fn test_part_select() {
        // 0b10110100: bit7=1,bit6=0,bit5=1,bit4=1,bit3=0,bit2=1,bit1=0,bit0=0
        // [6:4] = {bit6=0, bit5=1, bit4=1} → result = 0b011 = 3
        let v = LogicVal::new(8, 0b10110100, 0);
        let ps = v.part_select(6, 4).unwrap();
        assert_eq!(ps.width(), 3);
        assert_eq!(ps.pad_to_width(3), 0b011);
        // [5:2] = {bit5=1,bit4=1,bit3=0,bit2=1} → 0b1101 = 13
        let ps2 = v.part_select(5, 2).unwrap();
        assert_eq!(ps2.width(), 4);
        assert_eq!(ps2.pad_to_width(4), 0b1101);
    }

    #[test]
    fn test_shl_shr() {
        let v = LogicVal::new(8, 0b00001111, 0);
        let sl = v.shl(&LogicVal::new(4, 2, 0));
        assert_eq!(sl.pad_to_width(8), 0b00111100);
        let sr = v.shr(&LogicVal::new(4, 2, 0));
        assert_eq!(sr.pad_to_width(8), 0b00000011);
    }

    #[test]
    fn test_large_shl() {
        // shift 1 by 64 positions in a 128-bit value
        let v = LogicVal::from_chunks(128, &[1, 0], &[0, 0]);
        let shifted = v.shl(&LogicVal::new(8, 64, 0));
        assert_eq!(shifted.get_chunk(0), 0);
        assert_eq!(shifted.get_chunk(1), 1);
    }

    fn lv128(v: u128) -> LogicVal {
        LogicVal::from_chunks(128, &[v as u64, (v >> 64) as u64], &[0, 0])
    }

    fn to_u128(v: &LogicVal) -> u128 {
        assert!(v.is_known());
        v.get_chunk(0) as u128 | ((v.get_chunk(1) as u128) << 64)
    }

    #[test]
    fn test_wide_mul_div_mod_unsigned() {
        let vals: [u128; 6] = [
            0,
            1,
            7,
            u64::MAX as u128,
            (1u128 << 64) + 12345,
            0xdead_beef_cafe_f00d_0123_4567_89ab_cdef,
        ];
        for &a in &vals {
            for &b in &vals {
                assert_eq!(to_u128(&lv128(a).mul(&lv128(b))), a.wrapping_mul(b));
                if b == 0 {
                    assert!(lv128(a).div(&lv128(b)).is_x());
                    assert!(lv128(a).mod_(&lv128(b)).is_x());
                } else {
                    assert_eq!(to_u128(&lv128(a).div(&lv128(b))), a / b);
                    assert_eq!(to_u128(&lv128(a).mod_(&lv128(b))), a % b);
                }
            }
        }
    }

    #[test]
    fn test_wide_div_mod_signed() {
        let vals: [i128; 7] = [
            1,
            7,
            -1,
            -7,
            i64::MIN as i128 - 5,
            (1i128 << 90) + 3,
            -(1i128 << 100),
        ];
        for &a in &vals {
            for &b in &vals {
                let (la, lb) = (lv128(a as u128), lv128(b as u128));
                assert_eq!(
                    to_u128(&la.div_signed(&lb)),
                    a.wrapping_div(b) as u128,
                    "{a} / {b}"
                );
                assert_eq!(
                    to_u128(&la.mod_signed(&lb)),
                    a.wrapping_rem(b) as u128,
                    "{a} % {b}"
                );
            }
        }
        assert!(lv128(5).div_signed(&lv128(0)).is_x());
    }

    #[test]
    fn test_wide_arith_x_and_identity() {
        assert!(lv128(5).mul(&LogicVal::x_of_width(128)).is_x());
        // 200bit: q*d + r == n
        let n = LogicVal::from_chunks(200, &[u64::MAX, 0x1234, 0x5678, 0xab], &[0; 4]);
        let d = LogicVal::from_chunks(200, &[0xffff_0001, 0x77, 0, 0], &[0; 4]);
        let (q, r) = (n.div(&d), n.mod_(&d));
        assert_eq!(q.mul(&d).add(&r), n);
        assert!(r.lt(&d).is_one());
    }

    #[test]
    fn test_to_decimal_string_wide() {
        let n = lv128(1_000_000_000_000_000_000_000_000_000u128);
        assert_eq!(
            n.to_decimal_string(false).unwrap(),
            "1000000000000000000000000000"
        );
        assert_eq!(lv128(0).to_decimal_string(false).unwrap(), "0");
        let m = lv128((-12345678901234567890123i128) as u128);
        assert_eq!(
            m.to_decimal_string(true).unwrap(),
            "-12345678901234567890123"
        );
        assert!(LogicVal::x_of_width(100).to_decimal_string(false).is_none());
        assert_eq!(
            lv128(u128::MAX).to_decimal_string(false).unwrap(),
            u128::MAX.to_string()
        );
    }

    #[test]
    fn test_resolve_truth_table() {
        let (z, x) = (LogicVal::Z, LogicVal::X);
        let (o, l) = (LogicVal::ONE, LogicVal::ZERO);
        for v in [&z, &x, &o, &l] {
            assert_eq!(v.resolve(&z), v.clone(), "v,Z");
            assert_eq!(z.resolve(v), v.clone(), "Z,v");
        }
        assert_eq!(o.resolve(&o), o);
        assert_eq!(l.resolve(&l), l);
        assert!(o.resolve(&l).is_x());
        assert!(l.resolve(&o).is_x());
        assert!(x.resolve(&o).is_x());
        assert!(x.resolve(&x).is_x());
        // 64bit超: 上位チャンクも同じ規則
        let a = LogicVal::z_of_width(128).insert_bits(100, 4, &LogicVal::new(4, 0b1010, 0));
        let b = LogicVal::z_of_width(128).insert_bits(100, 4, &LogicVal::new(4, 0b1001, 0));
        let r = a.resolve(&b);
        assert_eq!(r.bit_ab(100), (1, 1)); // 0 vs 1 -> X
        assert_eq!(r.bit_ab(101), (1, 1)); // 1 vs 0 -> X
        assert_eq!(r.bit_ab(102), (0, 0)); // 0 vs 0 -> 0
        assert_eq!(r.bit_ab(103), (1, 0)); // 1 vs 1 -> 1
        assert_eq!(r.bit_ab(0), (0, 1)); // Z のまま
    }

    #[test]
    fn test_resolve_wand_wor_pull() {
        let (z, x, o, l) = (LogicVal::Z, LogicVal::X, LogicVal::ONE, LogicVal::ZERO);
        // wand: Z中立、0が支配、X > 1
        assert_eq!(z.resolve_wand(&o), o);
        assert_eq!(l.resolve_wand(&z), l);
        assert_eq!(z.resolve_wand(&z), z);
        assert_eq!(o.resolve_wand(&l), l);
        assert!(o.resolve_wand(&x).is_x());
        assert_eq!(l.resolve_wand(&x), l);
        assert_eq!(o.resolve_wand(&o), o);
        // wor: Z中立、1が支配、X > 0
        assert_eq!(z.resolve_wor(&l), l);
        assert_eq!(o.resolve_wor(&z), o);
        assert_eq!(z.resolve_wor(&z), z);
        assert_eq!(o.resolve_wor(&l), o);
        assert!(l.resolve_wor(&x).is_x());
        assert_eq!(o.resolve_wor(&x), o);
        assert_eq!(l.resolve_wor(&l), l);
        // pull_z: Zだけ置換
        assert_eq!(z.pull_z(true), o);
        assert_eq!(z.pull_z(false), l);
        assert_eq!(x.pull_z(true), x);
        assert_eq!(l.pull_z(true), l);
        // 64bit超
        let a = LogicVal::z_of_width(100).insert_bits(90, 2, &LogicVal::new(2, 0b10, 0));
        let b = LogicVal::z_of_width(100).insert_bits(90, 2, &LogicVal::new(2, 0b11, 0));
        let r = a.resolve_wand(&b);
        assert_eq!(r.bit_ab(90), (0, 0));
        assert_eq!(r.bit_ab(91), (1, 0));
        assert_eq!(r.bit_ab(0), (0, 1));
        let p = r.pull_z(true);
        assert_eq!(p.bit_ab(0), (1, 0));
        assert_eq!(p.bit_ab(90), (0, 0));
        assert_eq!(LogicVal::all_bits(100, true).bit_ab(99), (1, 0));
        // pull_z_range: 範囲内のZだけ置換
        let m = LogicVal::z_of_width(8);
        let p = m.pull_z_range(2, 3, true);
        assert_eq!(p.bit_ab(1), (0, 1));
        assert_eq!(p.bit_ab(2), (1, 0));
        assert_eq!(p.bit_ab(4), (1, 0));
        assert_eq!(p.bit_ab(5), (0, 1));
        // keep_on_z: Zだけ直前の値に
        let cur = LogicVal::new(4, 0b0101, 0b0010).keep_on_z(&LogicVal::new(4, 0b1111, 0));
        assert_eq!(cur.bit_ab(0), (1, 0));
        assert_eq!(cur.bit_ab(1), (1, 0));
        assert_eq!(cur.bit_ab(2), (1, 0));
    }

    #[test]
    fn test_merge_unknown() {
        let (z, x, o, l) = (LogicVal::Z, LogicVal::X, LogicVal::ONE, LogicVal::ZERO);
        assert_eq!(o.merge_unknown(&o), o);
        assert_eq!(z.merge_unknown(&z), z);
        assert!(o.merge_unknown(&l).is_x());
        assert!(o.merge_unknown(&z).is_x());
        assert!(x.merge_unknown(&x).is_x());
        // ビット単位: 一致するビットは保持
        let a = LogicVal::new(4, 0b1010, 0);
        let b = LogicVal::new(4, 0b1001, 0);
        let m = a.merge_unknown(&b);
        assert_eq!(m.bit_ab(3), (1, 0));
        assert_eq!(m.bit_ab(2), (0, 0));
        assert_eq!(m.bit_ab(1), (1, 1));
        assert_eq!(m.bit_ab(0), (1, 1));
    }
}

use std::ops::{Not, BitAnd, BitOr, BitXor};
use std::fmt;
use smallvec::SmallVec;

/// 4-value logic (0/1/X/Z) using aval/bval 2-plane representation.
/// (a,b) = (0,0)->0, (1,0)->1, (0,1)->Z, (1,1)->X
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicVal {
    Small { width: u16, a: u64, b: u64 },
    Large { width: u32, a: SmallVec<[u64; 4]>, b: SmallVec<[u64; 4]> },
}

// ── chunk helpers ─────────────────────────────────────────────────────────────

fn num_chunks(width: u32) -> usize {
    ((width + 63) / 64) as usize
}

fn top_mask(width: u32) -> u64 {
    let b = width % 64;
    if b == 0 { u64::MAX } else { (1u64 << b) - 1 }
}

fn chunk_mask(width: u32, idx: usize) -> u64 {
    let n = num_chunks(width);
    if idx == n - 1 { top_mask(width) } else { u64::MAX }
}

// ── impl ──────────────────────────────────────────────────────────────────────

impl LogicVal {
    pub const ZERO: LogicVal = LogicVal::Small { width: 1, a: 0, b: 0 };
    pub const ONE:  LogicVal = LogicVal::Small { width: 1, a: 1, b: 0 };
    pub const X:    LogicVal = LogicVal::Small { width: 1, a: 1, b: 1 };
    pub const Z:    LogicVal = LogicVal::Small { width: 1, a: 0, b: 1 };

    pub fn width(&self) -> u32 {
        match self {
            LogicVal::Small { width, .. } => *width as u32,
            LogicVal::Large { width, .. } => *width,
        }
    }

    /// Construct from a single (a,b) pair; bits above chunk 0 are zero.
    pub fn new(width: u16, a: u64, b: u64) -> Self {
        if width <= 64 {
            let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
            LogicVal::Small { width, a: a & mask, b: b & mask }
        } else {
            let mut av: SmallVec<[u64; 4]> = SmallVec::new();
            let mut bv: SmallVec<[u64; 4]> = SmallVec::new();
            av.push(a);
            bv.push(b);
            let n = num_chunks(width as u32);
            while av.len() < n { av.push(0); }
            while bv.len() < n { bv.push(0); }
            LogicVal::Large { width: width as u32, a: av, b: bv }
        }
    }

    /// Build from chunk vectors (internal use; truncates/extends as needed).
    pub fn from_chunks(width: u32, a: &[u64], b: &[u64]) -> Self {
        let n = num_chunks(width);
        let tm = top_mask(width);
        if width <= 64 {
            let av = a.get(0).copied().unwrap_or(0) & tm;
            let bv = b.get(0).copied().unwrap_or(0) & tm;
            LogicVal::Small { width: width as u16, a: av, b: bv }
        } else {
            let mut av: SmallVec<[u64; 4]> = SmallVec::new();
            let mut bv: SmallVec<[u64; 4]> = SmallVec::new();
            for i in 0..n {
                let m = if i == n - 1 { tm } else { u64::MAX };
                av.push(a.get(i).copied().unwrap_or(0) & m);
                bv.push(b.get(i).copied().unwrap_or(0) & m);
            }
            LogicVal::Large { width, a: av, b: bv }
        }
    }

    // ── chunk accessors ───────────────────────────────────────────────────────

    fn get_chunk(&self, idx: usize) -> u64 {
        match self {
            LogicVal::Small { a, .. } => if idx == 0 { *a } else { 0 },
            LogicVal::Large { a, .. } => a.get(idx).copied().unwrap_or(0),
        }
    }

    fn get_chunk_b(&self, idx: usize) -> u64 {
        match self {
            LogicVal::Small { b, .. } => if idx == 0 { *b } else { 0 },
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
        if !self.is_known() { return false; }
        let n = num_chunks(self.width());
        for i in 0..n {
            let m = chunk_mask(self.width(), i);
            if self.get_chunk(i) & m != m { return false; }
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
            if a != 0 { return false; }
            if b != 0 { any_b = true; }
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
            if a != b { return false; }
            if b != 0 { any_b = true; }
        }
        any_b
    }

    // ── low-64 extraction (used for index/shift amounts) ──────────────────────

    pub fn pad_to_width(&self, width: u32) -> u64 {
        let mask = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
        self.get_chunk(0) & mask
    }

    pub fn pad_to_width_b(&self, width: u32) -> u64 {
        let mask = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
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
        if !self.is_known() || !rhs.is_known() { return LogicVal::X; }
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
        if !self.is_known() || !rhs.is_known() { return LogicVal::X; }
        match self.eq(rhs) {
            LogicVal::ONE  => LogicVal::ZERO,
            LogicVal::ZERO => LogicVal::ONE,
            other => other,
        }
    }

    pub fn case_eq(&self, rhs: &LogicVal) -> LogicVal {
        if self.width() != rhs.width() { return LogicVal::ZERO; }
        let n = num_chunks(self.width());
        for i in 0..n {
            let m = chunk_mask(self.width(), i);
            if (self.get_chunk(i) & m) != (rhs.get_chunk(i) & m) { return LogicVal::ZERO; }
            if (self.get_chunk_b(i) & m) != (rhs.get_chunk_b(i) & m) { return LogicVal::ZERO; }
        }
        LogicVal::ONE
    }

    pub fn case_ne(&self, rhs: &LogicVal) -> LogicVal {
        match self.case_eq(rhs) {
            LogicVal::ONE  => LogicVal::ZERO,
            LogicVal::ZERO => LogicVal::ONE,
            other => other,
        }
    }

    // Multi-word unsigned comparison helper
    fn cmp_unsigned(&self, rhs: &LogicVal) -> Option<std::cmp::Ordering> {
        if !self.is_known() || !rhs.is_known() { return None; }
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

    pub fn gt(&self, rhs: &LogicVal) -> LogicVal { rhs.lt(self) }

    pub fn le(&self, rhs: &LogicVal) -> LogicVal {
        match self.cmp_unsigned(rhs) {
            None => LogicVal::X,
            Some(std::cmp::Ordering::Greater) => LogicVal::ZERO,
            _ => LogicVal::ONE,
        }
    }

    pub fn ge(&self, rhs: &LogicVal) -> LogicVal { rhs.le(self) }

    // ── arithmetic ────────────────────────────────────────────────────────────

    pub fn add(&self, rhs: &LogicVal) -> LogicVal {
        if !self.is_known() || !rhs.is_known() { return LogicVal::X; }
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
        if !self.is_known() || !rhs.is_known() { return LogicVal::X; }
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

    pub fn mul(&self, rhs: &LogicVal) -> LogicVal {
        let w = self.width().max(rhs.width());
        if w > 64 { return LogicVal::X; }
        if !self.is_known() || !rhs.is_known() { return LogicVal::X; }
        let la = self.pad_to_width(w);
        let ra = rhs.pad_to_width(w);
        Self::from_chunks(w, &[la.wrapping_mul(ra)], &[0])
    }

    pub fn div(&self, rhs: &LogicVal) -> LogicVal {
        let w = self.width().max(rhs.width());
        if w > 64 { return LogicVal::X; }
        if !self.is_known() || !rhs.is_known() { return LogicVal::X; }
        let ra = rhs.pad_to_width(w);
        if ra == 0 { return LogicVal::X; }
        Self::from_chunks(w, &[self.pad_to_width(w) / ra], &[0])
    }

    pub fn mod_(&self, rhs: &LogicVal) -> LogicVal {
        let w = self.width().max(rhs.width());
        if w > 64 { return LogicVal::X; }
        if !self.is_known() || !rhs.is_known() { return LogicVal::X; }
        let ra = rhs.pad_to_width(w);
        if ra == 0 { return LogicVal::X; }
        Self::from_chunks(w, &[self.pad_to_width(w) % ra], &[0])
    }

    // ── logical operators ─────────────────────────────────────────────────────

    pub fn log_and(&self, rhs: &LogicVal) -> LogicVal {
        if !self.is_known() || !rhs.is_known() { return LogicVal::X; }
        if self.is_zero() || rhs.is_zero() { LogicVal::ZERO } else { LogicVal::ONE }
    }

    pub fn log_or(&self, rhs: &LogicVal) -> LogicVal {
        if !self.is_known() || !rhs.is_known() { return LogicVal::X; }
        if !self.is_zero() || !rhs.is_zero() { LogicVal::ONE } else { LogicVal::ZERO }
    }

    pub fn log_not(&self) -> LogicVal {
        if !self.is_known() { return LogicVal::X; }
        if self.is_zero() { LogicVal::ONE } else { LogicVal::ZERO }
    }

    pub fn cond(&self, true_val: &LogicVal, false_val: &LogicVal) -> LogicVal {
        if !self.is_known() { return LogicVal::X; }
        if self.is_zero() { false_val.clone() } else { true_val.clone() }
    }

    // ── shift operators ───────────────────────────────────────────────────────

    pub fn shl(&self, rhs: &LogicVal) -> LogicVal {
        if !self.is_known() || !rhs.is_known() { return LogicVal::X; }
        let shift = rhs.pad_to_width(32) as u32;
        let w = self.width();
        if shift >= w { return Self::from_chunks(w, &[0], &[0]); }
        let n = num_chunks(w);
        let chunk_shift = (shift / 64) as usize;
        let bit_shift = shift % 64;
        let mut a = vec![0u64; n];
        for i in chunk_shift..n {
            let src = i - chunk_shift;
            let m = chunk_mask(w, i);
            let lo = self.get_chunk(src) << bit_shift;
            let hi = if bit_shift > 0 && src > 0 {
                self.get_chunk(src - 1) >> (64 - bit_shift)
            } else { 0 };
            a[i] = (lo | hi) & m;
        }
        Self::from_chunks(w, &a, &vec![0u64; n])
    }

    pub fn shr(&self, rhs: &LogicVal) -> LogicVal {
        if !self.is_known() || !rhs.is_known() { return LogicVal::X; }
        let shift = rhs.pad_to_width(32) as u32;
        let w = self.width();
        if shift >= w { return Self::from_chunks(w, &[0], &[0]); }
        let n = num_chunks(w);
        let chunk_shift = (shift / 64) as usize;
        let bit_shift = shift % 64;
        let mut a = vec![0u64; n];
        for i in 0..(n - chunk_shift) {
            let src = i + chunk_shift;
            let sm = chunk_mask(w, src);
            let lo = (self.get_chunk(src) & sm) >> bit_shift;
            let hi = if bit_shift > 0 && src + 1 < n {
                let sm2 = chunk_mask(w, src + 1);
                (self.get_chunk(src + 1) & sm2) << (64 - bit_shift)
            } else { 0 };
            a[i] = lo | hi;
        }
        Self::from_chunks(w, &a, &vec![0u64; n])
    }

    pub fn ashl(&self, rhs: &LogicVal) -> Self { self.shl(rhs) }

    pub fn ashr(&self, rhs: &LogicVal) -> LogicVal {
        if !self.is_known() || !rhs.is_known() { return LogicVal::X; }
        let shift = rhs.pad_to_width(32) as u32;
        let w = self.width();
        let n = num_chunks(w);
        let sign_chunk = n - 1;
        let sign_bit_pos = if w % 64 == 0 { 63 } else { (w % 64) - 1 };
        let sign = (self.get_chunk(sign_chunk) >> sign_bit_pos) & 1;
        if sign == 0 { return self.shr(rhs); }
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
                if fb == 0 { u64::MAX } else { !((1u64 << fb) - 1) }
            } else { u64::MAX };
            let cur = result.get_chunk(i);
            let new_val = (cur | fill_mask) & m;
            match &mut result {
                LogicVal::Small { a, .. } => { if i == 0 { *a = new_val; } }
                LogicVal::Large { a, .. } => { if let Some(v) = a.get_mut(i) { *v = new_val; } }
            }
        }
        result
    }

    // ── reduction operators ───────────────────────────────────────────────────

    pub fn reduce_and(&self) -> LogicVal {
        if !self.is_known() { return LogicVal::X; }
        let n = num_chunks(self.width());
        for i in 0..n {
            let m = chunk_mask(self.width(), i);
            if self.get_chunk(i) & m != m { return LogicVal::ZERO; }
        }
        LogicVal::ONE
    }

    pub fn reduce_or(&self) -> LogicVal {
        if !self.is_known() { return LogicVal::X; }
        let n = num_chunks(self.width());
        for i in 0..n {
            let m = chunk_mask(self.width(), i);
            if self.get_chunk(i) & m != 0 { return LogicVal::ONE; }
        }
        LogicVal::ZERO
    }

    pub fn reduce_xor(&self) -> LogicVal {
        if !self.is_known() { return LogicVal::X; }
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
        if result & 1 == 1 { LogicVal::ONE } else { LogicVal::ZERO }
    }

    pub fn reduce_nand(&self)  -> LogicVal { self.reduce_and().not() }
    pub fn reduce_nor(&self)   -> LogicVal { self.reduce_or().not()  }
    pub fn reduce_xnor(&self)  -> LogicVal { self.reduce_xor().not() }

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
        if n == 0 { return Self::from_chunks(0, &[], &[]); }
        let mut result = value.clone();
        for _ in 1..n {
            result = result.concat(value);
        }
        result
    }

    // ── bit/part select ───────────────────────────────────────────────────────

    fn get_bit(&self, idx: u32) -> LogicVal {
        if idx >= self.width() { return LogicVal::X; }
        let chunk = (idx / 64) as usize;
        let bit   = idx % 64;
        let a = (self.get_chunk(chunk) >> bit) & 1;
        let b = (self.get_chunk_b(chunk) >> bit) & 1;
        LogicVal::Small { width: 1, a, b }
    }

    pub fn bit_select(&self, idx: u32) -> Result<LogicVal, &'static str> {
        if idx >= self.width() { return Err("bit index out of range"); }
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
            let src_pos   = src_bit % 64;
            let dst_chunk = (bit / 64) as usize;
            let dst_pos   = bit % 64;
            a[dst_chunk] |= ((self.get_chunk(src_chunk) >> src_pos) & 1) << dst_pos;
            b[dst_chunk] |= ((self.get_chunk_b(src_chunk) >> src_pos) & 1) << dst_pos;
        }
        Ok(Self::from_chunks(out_width, &a, &b))
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
                let mut parts: Vec<String> = (0..n).rev()
                    .map(|i| {
                        let m = chunk_mask(self.width(), i);
                        format!("{:016x}", self.get_chunk(i) & m)
                    })
                    .collect();
                // Trim leading zeros from first segment
                let first = parts[0].trim_start_matches('0');
                parts[0] = if first.is_empty() { "0".to_string() } else { first.to_string() };
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
            LogicVal::Small { width: w as u16, a: a[0], b: b[0] }
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
            a.push(ao); b.push(bo);
        }
        if w <= 64 {
            LogicVal::Small { width: w as u16, a: a[0], b: b[0] }
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
            a.push(ao); b.push(bo);
        }
        if w <= 64 {
            LogicVal::Small { width: w as u16, a: a[0], b: b[0] }
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
            a.push(ao); b.push(bo);
        }
        if w <= 64 {
            LogicVal::Small { width: w as u16, a: a[0], b: b[0] }
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
        let hi = LogicVal::new(4, 0b1010, 0);  // 4-bit
        let lo = LogicVal::new(4, 0b0101, 0);  // 4-bit
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
}

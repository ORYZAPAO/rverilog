use std::ops::{Not, BitAnd, BitOr, BitXor};
use std::cmp::PartialEq;
use std::fmt;
use smallvec::SmallVec;

/// 4-value logic (0/1/X/Z) using aval/bval 2-plane representation
/// (a,b) = (0,0) -> 0, (1,0) -> 1, (0,1) -> Z, (1,1) -> X
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogicVal {
    Small { width: u16, a: u64, b: u64 },
    Large { width: u32, a: SmallVec<[u64; 4]>, b: SmallVec<[u64; 4]> },
}

impl LogicVal {
    pub const ZERO: LogicVal = LogicVal::Small { width: 1, a: 0, b: 0 };
    pub const ONE: LogicVal = LogicVal::Small { width: 1, a: 1, b: 0 };
    pub const X: LogicVal = LogicVal::Small { width: 1, a: 1, b: 1 };
    pub const Z: LogicVal = LogicVal::Small { width: 1, a: 0, b: 1 };

    pub fn width(&self) -> u32 {
        match self {
            LogicVal::Small { width, .. } => *width as u32,
            LogicVal::Large { width, .. } => *width,
        }
    }

    pub fn pad_to_width(&self, width: u32) -> u64 {
        let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
        match self {
            LogicVal::Small { a, .. } => *a & mask,
            LogicVal::Large { a, .. } => *a.get(0).unwrap_or(&0) & mask,
        }
    }

    pub fn new(width: u16, a: u64, b: u64) -> Self {
        if width <= 64 {
            LogicVal::Small { width, a, b }
        } else {
            LogicVal::Large { width: width as u32, a: SmallVec::from_slice(&[a]), b: SmallVec::from_slice(&[b]) }
        }
    }

    pub fn is_zero(&self) -> bool {
        self.pad_to_width(self.width()) == 0 && self.pad_to_width_b(self.width()) == 0
    }

    pub fn is_one(&self) -> bool {
        let mask = if self.width() == 64 { u64::MAX } else { (1u64 << self.width()) - 1 };
        self.pad_to_width(self.width()) == mask && self.pad_to_width_b(self.width()) == 0
    }

    pub fn is_z(&self) -> bool {
        self.pad_to_width_b(self.width()) != 0 && self.pad_to_width(self.width()) == 0
    }

    pub fn is_x(&self) -> bool {
        self.pad_to_width(self.width()) == self.pad_to_width_b(self.width()) && self.pad_to_width_b(self.width()) != 0
    }

    pub fn is_known(&self) -> bool {
        self.pad_to_width_b(self.width()) == 0
    }

 

    fn get_bit(&self, idx: u32) -> Result<LogicVal, &'static str> {
        if idx >= self.width() {
            return Err("bit index out of range");
        }
        let mask = 1u64 << idx;
        let a = (self.pad_to_width(self.width()) & mask) >> idx;
        let b = (self.pad_to_width_b(self.width()) & mask) >> idx;
        Ok(LogicVal::Small { width: 1, a, b })
    }

    pub fn extend_zero(&self, new_width: u32) -> Self {
        if new_width <= 64 && self.width() <= 64 {
            let a = self.pad_to_width(self.width());
            LogicVal::Small { width: new_width as u16, a, b: 0 }
        } else {
            LogicVal::X
        }
    }

    pub fn pad_to_width_b(&self, width: u32) -> u64 {
        let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
        match self {
            LogicVal::Small { b, .. } => *b & mask,
            LogicVal::Large { b, .. } => *b.get(0).unwrap_or(&0) & mask,
        }
    }

    pub fn eq(&self, rhs: &LogicVal) -> LogicVal {
        if self.width() != rhs.width() {
            return LogicVal::X;
        }
        let self_a = self.pad_to_width(self.width());
        let rhs_a = rhs.pad_to_width(rhs.width());
        let self_b = self.pad_to_width_b(self.width());
        let rhs_b = rhs.pad_to_width_b(rhs.width());
        if self_b != 0 || rhs_b != 0 {
            LogicVal::X
        } else {
            if self_a == rhs_a { LogicVal::ONE } else { LogicVal::ZERO }
        }
    }

    pub fn ne(&self, rhs: &LogicVal) -> LogicVal {
        if self.width() != rhs.width() {
            return LogicVal::X;
        }
        let self_a = self.pad_to_width(self.width());
        let rhs_a = rhs.pad_to_width(rhs.width());
        let self_b = self.pad_to_width_b(self.width());
        let rhs_b = rhs.pad_to_width_b(rhs.width());
        if self_b != 0 || rhs_b != 0 {
            LogicVal::X
        } else {
            if self_a != rhs_a { LogicVal::ONE } else { LogicVal::ZERO }
        }
    }

    pub fn case_eq(&self, rhs: &LogicVal) -> LogicVal {
        if self.width() != rhs.width() {
            return LogicVal::X;
        }
        let self_a = self.pad_to_width(self.width());
        let rhs_a = rhs.pad_to_width(rhs.width());
        let self_b = self.pad_to_width_b(self.width());
        let rhs_b = rhs.pad_to_width_b(rhs.width());
        if (self_a == rhs_a) && (self_b == rhs_b) {
            LogicVal::ONE
        } else {
            LogicVal::ZERO
        }
    }

    pub fn case_ne(&self, rhs: &LogicVal) -> LogicVal {
        if self.width() != rhs.width() {
            return LogicVal::X;
        }
        let self_a = self.pad_to_width(self.width());
        let rhs_a = rhs.pad_to_width(rhs.width());
        let self_b = self.pad_to_width_b(self.width());
        let rhs_b = rhs.pad_to_width_b(rhs.width());
        if (self_a != rhs_a) || (self_b != rhs_b) {
            LogicVal::ONE
        } else {
            LogicVal::ZERO
        }
    }

    pub fn lt(&self, rhs: &LogicVal) -> LogicVal {
        let self_a = self.pad_to_width(self.width());
        let rhs_a = rhs.pad_to_width(rhs.width());
        let self_b = self.pad_to_width_b(self.width());
        let rhs_b = rhs.pad_to_width_b(rhs.width());
        if self_b != 0 || rhs_b != 0 {
            return LogicVal::X;
        }
        if self_a < rhs_a { LogicVal::ONE } else { LogicVal::ZERO }
    }

    pub fn gt(&self, rhs: &LogicVal) -> LogicVal {
        rhs.lt(self)
    }

    pub fn le(&self, rhs: &LogicVal) -> LogicVal {
        if self.width() != rhs.width() {
            return LogicVal::X;
        }
        let self_a = self.pad_to_width(self.width());
        let rhs_a = rhs.pad_to_width(rhs.width());
        let self_b = self.pad_to_width_b(self.width());
        let rhs_b = rhs.pad_to_width_b(rhs.width());
        if self_b != 0 || rhs_b != 0 {
            return LogicVal::X;
        }
        if self_a <= rhs_a { LogicVal::ONE } else { LogicVal::ZERO }
    }

    pub fn ge(&self, rhs: &LogicVal) -> LogicVal {
        if self.width() != rhs.width() {
            return LogicVal::X;
        }
        let self_a = self.pad_to_width(self.width());
        let rhs_a = rhs.pad_to_width(rhs.width());
        let self_b = self.pad_to_width_b(self.width());
        let rhs_b = rhs.pad_to_width_b(rhs.width());
        if self_b != 0 || rhs_b != 0 {
            return LogicVal::X;
        }
        if self_a >= rhs_a { LogicVal::ONE } else { LogicVal::ZERO }
    }

    pub fn shl(&self, rhs: &LogicVal) -> LogicVal {
        let shift = rhs.pad_to_width(rhs.width()) as u32;
        if shift >= self.width() {
            return LogicVal::ZERO;
        }
        let self_a = self.pad_to_width(self.width());
        let self_b = self.pad_to_width_b(self.width());
        if self_b != 0 {
            return LogicVal::X;
        }
        let a = self_a << shift;
        let b = self_b << shift;
        LogicVal::Small { width: self.width() as u16, a, b }
    }

    pub fn shr(&self, rhs: &LogicVal) -> LogicVal {
        let shift = rhs.pad_to_width(rhs.width()) as u32;
        if shift >= self.width() {
            return LogicVal::ZERO;
        }
        let self_a = self.pad_to_width(self.width());
        let self_b = self.pad_to_width_b(self.width());
        if self_b != 0 {
            return LogicVal::X;
        }
        let a = self_a >> shift;
        let b = self_b >> shift;
        LogicVal::Small { width: self.width() as u16, a, b }
    }

    pub fn ashl(&self, rhs: &LogicVal) -> LogicVal {
        let shift = rhs.pad_to_width(rhs.width()) as u32;
        if shift >= self.width() {
            let sign = if (self.pad_to_width(self.width()) & (1u64 << (self.width() - 1))) != 0 { u64::MAX } else { 0 };
            return LogicVal::Small { width: self.width() as u16, a: sign, b: 0 };
        }
        let self_a = self.pad_to_width(self.width());
        let self_b = self.pad_to_width_b(self.width());
        if self_b != 0 {
            return LogicVal::X;
        }
        let a = self_a << shift;
        let b = self_b << shift;
        LogicVal::Small { width: self.width() as u16, a, b }
    }

    pub fn ashr(&self, rhs: &LogicVal) -> LogicVal {
        let shift = rhs.pad_to_width(rhs.width()) as u32;
        if shift >= self.width() {
            let sign_bit = (self.pad_to_width(self.width()) & (1u64 << (self.width() - 1))) != 0;
            let fill = if sign_bit { u64::MAX } else { 0 };
            return LogicVal::Small { width: self.width() as u16, a: fill, b: 0 };
        }
        let self_a = self.pad_to_width(self.width());
        let self_b = self.pad_to_width_b(self.width());
        if self_b != 0 {
            return LogicVal::X;
        }
        let a = self_a >> shift;
        let b = self_b >> shift;
        LogicVal::Small { width: self.width() as u16, a, b }
    }

    pub fn add(&self, rhs: &LogicVal) -> LogicVal {
        let width = self.width().max(rhs.width());
        if width > 64 {
            return LogicVal::X;
        }
        let self_a = self.pad_to_width(width);
        let rhs_a = rhs.pad_to_width(width);
        let self_b = self.pad_to_width_b(width);
        let rhs_b = rhs.pad_to_width_b(width);
        if self_b != 0 || rhs_b != 0 {
            return LogicVal::X;
        }
        let a = self_a.wrapping_add(rhs_a);
        LogicVal::Small { width: width as u16, a, b: 0 }
    }

    pub fn sub(&self, rhs: &LogicVal) -> LogicVal {
        let width = self.width().max(rhs.width());
        if width > 64 {
            return LogicVal::X;
        }
        let self_a = self.pad_to_width(width);
        let rhs_a = rhs.pad_to_width(width);
        let self_b = self.pad_to_width_b(width);
        let rhs_b = rhs.pad_to_width_b(width);
        if self_b != 0 || rhs_b != 0 {
            return LogicVal::X;
        }
        let a = self_a.wrapping_sub(rhs_a);
        LogicVal::Small { width: width as u16, a, b: 0 }
    }

    pub fn mul(&self, rhs: &LogicVal) -> LogicVal {
        let width = self.width().max(rhs.width());
        if width > 64 {
            return LogicVal::X;
        }
        let self_a = self.pad_to_width(width);
        let rhs_a = rhs.pad_to_width(width);
        let self_b = self.pad_to_width_b(width);
        let rhs_b = rhs.pad_to_width_b(width);
        if self_b != 0 || rhs_b != 0 {
            return LogicVal::X;
        }
        let a = self_a.wrapping_mul(rhs_a);
        LogicVal::Small { width: width as u16, a, b: 0 }
    }

    pub fn div(&self, rhs: &LogicVal) -> LogicVal {
        let width = self.width().max(rhs.width());
        if width > 64 {
            return LogicVal::X;
        }
        let self_a = self.pad_to_width(width);
        let rhs_a = rhs.pad_to_width(width);
        let self_b = self.pad_to_width_b(width);
        let rhs_b = rhs.pad_to_width_b(width);
        if self_b != 0 || rhs_b != 0 || rhs_a == 0 {
            return LogicVal::X;
        }
        let a = self_a.wrapping_div(rhs_a);
        LogicVal::Small { width: width as u16, a, b: 0 }
    }

    pub fn mod_(&self, rhs: &LogicVal) -> LogicVal {
        let width = self.width().max(rhs.width());
        if width > 64 {
            return LogicVal::X;
        }
        let self_a = self.pad_to_width(width);
        let rhs_a = rhs.pad_to_width(width);
        let self_b = self.pad_to_width_b(width);
        let rhs_b = rhs.pad_to_width_b(width);
        if self_b != 0 || rhs_b != 0 || rhs_a == 0 {
            return LogicVal::X;
        }
        let a = self_a.wrapping_rem(rhs_a);
        LogicVal::Small { width: width as u16, a, b: 0 }
    }

    pub fn log_and(&self, rhs: &LogicVal) -> LogicVal {
        let self_known = self.is_known();
        let rhs_known = rhs.is_known();
        if !self_known || !rhs_known {
            return LogicVal::X;
        }
        if self.is_zero() || rhs.is_zero() { LogicVal::ZERO } else { LogicVal::ONE }
    }

    pub fn log_or(&self, rhs: &LogicVal) -> LogicVal {
        let self_known = self.is_known();
        let rhs_known = rhs.is_known();
        if !self_known || !rhs_known {
            return LogicVal::X;
        }
        if self.is_one() || rhs.is_one() { LogicVal::ONE } else { LogicVal::ZERO }
    }

    pub fn log_not(&self) -> LogicVal {
        let self_known = self.is_known();
        if !self_known {
            return LogicVal::X;
        }
        if self.is_zero() { LogicVal::ONE } else { LogicVal::ZERO }
    }

    pub fn cond(&self, true_val: &LogicVal, false_val: &LogicVal) -> LogicVal {
        if self.is_one() {
            true_val.clone()
        } else if self.is_zero() {
            false_val.clone()
        } else {
            LogicVal::X
        }
    }

    pub fn reduce_and(&self) -> LogicVal {
        if self.is_zero() {
            return LogicVal::ONE;
        }
        let mut a = self.pad_to_width(self.width());
        let b = self.pad_to_width_b(self.width());
        if b != 0 {
            return LogicVal::X;
        }
        a = a & ((1u64 << self.width()) - 1);
        if a == ((1u64 << self.width()) - 1) { LogicVal::ONE } else { LogicVal::ZERO }
    }

    pub fn reduce_or(&self) -> LogicVal {
        let a = self.pad_to_width(self.width());
        let b = self.pad_to_width_b(self.width());
        if b != 0 {
            return LogicVal::X;
        }
        if a == 0 { LogicVal::ZERO } else { LogicVal::ONE }
    }

    pub fn reduce_xor(&self) -> LogicVal {
        let a = self.pad_to_width(self.width());
        let b = self.pad_to_width_b(self.width());
        if b != 0 {
            return LogicVal::X;
        }
        let mut result = 0u64;
        let mut val = a;
        while val > 0 {
            result ^= val & 1;
            val >>= 1;
        }
        if result == 1 { LogicVal::ONE } else { LogicVal::ZERO }
    }

    pub fn reduce_nand(&self) -> LogicVal {
        let red_and = self.reduce_and();
        red_and.log_not()
    }

    pub fn reduce_nor(&self) -> LogicVal {
        let red_or = self.reduce_or();
        red_or.log_not()
    }

    pub fn reduce_xnor(&self) -> LogicVal {
        let red_xor = self.reduce_xor();
        red_xor.log_not()
    }

    fn get_chunk(&self, idx: usize) -> u64 {
        match self {
            LogicVal::Small { a, .. } => {
                if idx == 0 { *a } else { 0 }
            }
            LogicVal::Large { a, .. } => {
                *a.get(idx).unwrap_or(&0)
            }
        }
    }

    fn get_chunk_b(&self, idx: usize) -> u64 {
        match self {
            LogicVal::Small { b, .. } => {
                if idx == 0 { *b } else { 0 }
            }
            LogicVal::Large { b, .. } => {
                *b.get(idx).unwrap_or(&0)
            }
        }
    }

    pub fn concat(&self, rhs: &LogicVal) -> LogicVal {
        let total_width = self.width() + rhs.width();
        if total_width <= 64 {
            let self_a = self.pad_to_width(self.width());
            let rhs_a = rhs.pad_to_width(rhs.width());
            let self_b = self.pad_to_width_b(self.width());
            let rhs_b = rhs.pad_to_width_b(rhs.width());
            let a = (self_a << rhs.width() as u64) | rhs_a;
            let b = (self_b << rhs.width() as u64) | rhs_b;
            LogicVal::Small { width: total_width as u16, a, b }
        } else {
            LogicVal::X
        }
    }

    pub fn repeat(&self, n: u32, value: &LogicVal) -> LogicVal {
        let total_width = value.width() * n;
        if total_width <= 64 {
            let mut result_a = 0u64;
            let mut result_b = 0u64;
            let val_a = value.pad_to_width(value.width());
            let val_b = value.pad_to_width_b(value.width());
            for i in 0..n {
                 result_a |= val_a << (i * value.width() as u32);
                 result_b |= val_b << (i * value.width() as u32);
             }
            LogicVal::Small { width: total_width as u16, a: result_a, b: result_b }
        } else {
            LogicVal::X
        }
    }

    pub fn bit_select(&self, idx: u32) -> Result<LogicVal, &'static str> {
        if idx >= self.width() {
            return Err("bit index out of range");
        }
        self.get_bit(idx)
    }

    pub fn part_select(&self, left: u32, right: u32) -> Result<LogicVal, &'static str> {
        if left >= self.width() || right >= self.width() || left < right {
            return Err("invalid part select range");
        }
        let width = left - right + 1;
        if width <= 64 {
            let mask = ((1u64 << width) - 1) << right;
            let self_a = self.pad_to_width(self.width()) & mask;
            let self_b = self.pad_to_width_b(self.width()) & mask;
            let a = self_a >> right;
            let b = self_b >> right;
            Ok(LogicVal::Small { width: width as u16, a, b })
        } else {
            Ok(LogicVal::X)
        }
    }
}

impl fmt::Display for LogicVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogicVal::Small { a, b, .. } => {
                let mask = if self.width() == 64 { u64::MAX } else { (1u64 << self.width()) - 1 };
                let a = *a & mask;
                let b = *b & mask;
                if b == 0 {
                    write!(f, "{}", a)
                } else if a == b {
                    write!(f, "x")
                } else {
                    write!(f, "z")
                }
            }
            LogicVal::Large { a, b, .. } => {
                if b.iter().all(|&x| x == 0) {
                    let hex: String = a.iter().rev().map(|x| format!("{:x}", x)).collect();
                     write!(f, "{}", if hex.is_empty() { "0" } else { hex.trim_start_matches('0') })
                } else if a.iter().zip(b.iter()).all(|(x, y)| x == y) {
                    write!(f, "x")
                } else if a.iter().zip(b.iter()).all(|(_, y)| y == &0) {
                    write!(f, "z")
                } else {
                    write!(f, "x")
                }
            }
        }
    }
}

impl Not for LogicVal {
    type Output = Self;

    // IEEE 4-value NOT: ~0=1, ~1=0, ~X=X, ~Z=X
    // a_out = (~a | b) & mask,  b_out = b & mask
    fn not(self) -> Self::Output {
        match self {
            LogicVal::Small { a, b, width } => {
                let mask = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
                let a = a & mask;
                let b = b & mask;
                LogicVal::Small { width, a: ((!a) | b) & mask, b }
            }
            LogicVal::Large { a, b, width } => {
                let chunks = ((width + 63) / 64) as usize;
                let top = width % 64;
                let last_mask = if top == 0 { u64::MAX } else { (1u64 << top) - 1 };
                let mut na: SmallVec<[u64; 4]> = SmallVec::new();
                let mut nb: SmallVec<[u64; 4]> = SmallVec::new();
                for i in 0..chunks {
                    let mask = if i == chunks - 1 { last_mask } else { u64::MAX };
                    let ai = a.get(i).copied().unwrap_or(0) & mask;
                    let bi = b.get(i).copied().unwrap_or(0) & mask;
                    na.push(((!ai) | bi) & mask);
                    nb.push(bi);
                }
                LogicVal::Large { width, a: na, b: nb }
            }
        }
    }
}

impl BitAnd for LogicVal {
    type Output = Self;

    // IEEE 4-value AND: 0&X=0, 1&X=X, X&X=X etc.
    // a_out = (a1|b1) & (a2|b2)
    // b_out = a_out & !((a1&~b1) & (a2&~b2))
    fn bitand(self, rhs: Self) -> Self::Output {
        let width = self.width().max(rhs.width());
        let bitand_chunk = |a1: u64, b1: u64, a2: u64, b2: u64| -> (u64, u64) {
            let a = (a1 | b1) & (a2 | b2);
            let b = a & !((a1 & !b1) & (a2 & !b2));
            (a, b)
        };
        if width <= 64 {
            let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
            let a1 = self.pad_to_width(width) & mask;
            let b1 = self.pad_to_width_b(width) & mask;
            let a2 = rhs.pad_to_width(width) & mask;
            let b2 = rhs.pad_to_width_b(width) & mask;
            let (a, b) = bitand_chunk(a1, b1, a2, b2);
            LogicVal::Small { width: width as u16, a, b }
        } else {
            let chunks = ((width + 63) / 64) as usize;
            let mut na = SmallVec::new();
            let mut nb = SmallVec::new();
            for i in 0..chunks {
                let (a, b) = bitand_chunk(
                    self.get_chunk(i), self.get_chunk_b(i),
                    rhs.get_chunk(i), rhs.get_chunk_b(i),
                );
                na.push(a); nb.push(b);
            }
            LogicVal::Large { width, a: na, b: nb }
        }
    }
}

impl BitOr for LogicVal {
    type Output = Self;

    // IEEE 4-value OR: 1|X=1, 0|X=X, X|X=X etc.
    // a_out = (a1|b1) | (a2|b2)
    // b_out = a_out & (~a1|b1) & (~a2|b2)
    fn bitor(self, rhs: Self) -> Self::Output {
        let width = self.width().max(rhs.width());
        let bitor_chunk = |a1: u64, b1: u64, a2: u64, b2: u64| -> (u64, u64) {
            let a = (a1 | b1) | (a2 | b2);
            let b = a & ((!a1) | b1) & ((!a2) | b2);
            (a, b)
        };
        if width <= 64 {
            let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
            let a1 = self.pad_to_width(width) & mask;
            let b1 = self.pad_to_width_b(width) & mask;
            let a2 = rhs.pad_to_width(width) & mask;
            let b2 = rhs.pad_to_width_b(width) & mask;
            let (a, b) = bitor_chunk(a1, b1, a2, b2);
            LogicVal::Small { width: width as u16, a, b }
        } else {
            let chunks = ((width + 63) / 64) as usize;
            let mut na = SmallVec::new();
            let mut nb = SmallVec::new();
            for i in 0..chunks {
                let (a, b) = bitor_chunk(
                    self.get_chunk(i), self.get_chunk_b(i),
                    rhs.get_chunk(i), rhs.get_chunk_b(i),
                );
                na.push(a); nb.push(b);
            }
            LogicVal::Large { width, a: na, b: nb }
        }
    }
}

impl BitXor for LogicVal {
    type Output = Self;

    // IEEE 4-value XOR: deterministic only when both inputs are known
    // b_out = b1 | b2,  a_out = (a1 ^ a2) | b_out
    fn bitxor(self, rhs: Self) -> Self::Output {
        let width = self.width().max(rhs.width());
        let bitxor_chunk = |a1: u64, b1: u64, a2: u64, b2: u64| -> (u64, u64) {
            let b = b1 | b2;
            let a = (a1 ^ a2) | b;
            (a, b)
        };
        if width <= 64 {
            let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
            let a1 = self.pad_to_width(width) & mask;
            let b1 = self.pad_to_width_b(width) & mask;
            let a2 = rhs.pad_to_width(width) & mask;
            let b2 = rhs.pad_to_width_b(width) & mask;
            let (a, b) = bitxor_chunk(a1, b1, a2, b2);
            LogicVal::Small { width: width as u16, a, b }
        } else {
            let chunks = ((width + 63) / 64) as usize;
            let mut na = SmallVec::new();
            let mut nb = SmallVec::new();
            for i in 0..chunks {
                let (a, b) = bitxor_chunk(
                    self.get_chunk(i), self.get_chunk_b(i),
                    rhs.get_chunk(i), rhs.get_chunk_b(i),
                );
                na.push(a); nb.push(b);
            }
            LogicVal::Large { width, a: na, b: nb }
        }
    }
}

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
        assert_eq!(and.width(), 4);
        assert_eq!(and.pad_to_width(4), 0b1000);
        
        let or = a.clone() | b.clone();
        assert_eq!(or.pad_to_width(4), 0b1110);
        
        let xor = a.clone() ^ b.clone();
        assert_eq!(xor.pad_to_width(4), 0b0110);
    }

    #[test]
    fn test_not() {
        let a = LogicVal::new(4, 0b1010, 0);
        let not_a = !a;
        assert_eq!(not_a.pad_to_width(4), 0b0101);
    }

    #[test]
    fn test_extension() {
        let a = LogicVal::new(4, 0b1010, 0);
        let extended = a.extend_zero(8);
        assert_eq!(extended.width(), 8);
        assert_eq!(extended.pad_to_width(8), 0b00001010);
    }
}

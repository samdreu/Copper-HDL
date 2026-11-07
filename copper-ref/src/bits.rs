//! `RefBits` — the interpreter's bit-vector value, mirroring `copper_core::Bits<N>`
//! bit for bit, with the width a run-time field instead of a const generic.
//!
//! The semantics are **copied from `copper-core/src/types.rs`**, not invented: every
//! operator here has the same X rule as the `Bits<N>` / `Logic` impl it stands for
//! (bitwise ops are per-bit three-valued; arithmetic is all-X on any X operand;
//! shifts are logical and move X with the data; `==` is the *derived* `PartialEq`,
//! so X == X). `tests/bits_property.rs` pins that agreement against `Bits<N>` at
//! widths {1, 4, 8, 16, 32, 64, 128} with random operands — the first executable
//! check of SPEC §I.14.
//!
//! Representation: `val` holds the known-1 bits, `x` the unknown bits; a bit set in
//! `x` is always clear in `val` (canonical), and both are masked to `width`.

use copper_core::Logic;
use std::fmt;

/// A bit vector of run-time width `1..=128`, three-valued per bit.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RefBits {
    pub width: u32,
    pub val: u128,
    pub x: u128,
}

impl fmt::Debug for RefBits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}'", self.width)?;
        // MSB first, like a Verilog literal.
        for i in (0..self.width).rev() {
            let b = 1u128 << i;
            f.write_str(if self.x & b != 0 { "x" } else if self.val & b != 0 { "1" } else { "0" })?;
        }
        Ok(())
    }
}

impl RefBits {
    /// `2^width − 1`.
    pub fn mask(width: u32) -> u128 {
        if width >= 128 { u128::MAX } else { (1u128 << width) - 1 }
    }

    fn canon(width: u32, val: u128, x: u128) -> Self {
        let m = Self::mask(width);
        let x = x & m;
        RefBits { width, val: val & m & !x, x }
    }

    pub fn zero(width: u32) -> Self {
        RefBits { width, val: 0, x: 0 }
    }

    pub fn all_ones(width: u32) -> Self {
        RefBits { width, val: Self::mask(width), x: 0 }
    }

    pub fn all_x(width: u32) -> Self {
        RefBits { width, val: 0, x: Self::mask(width) }
    }

    /// A known value; the caller guarantees it fits (`from_uint` in `copper-core`
    /// silently drops bits above `N`, and so does this).
    pub fn from_uint(width: u32, val: u128) -> Self {
        Self::canon(width, val, 0)
    }

    pub fn from_logic(l: Logic) -> Self {
        match l {
            Logic::Zero => RefBits { width: 1, val: 0, x: 0 },
            Logic::One => RefBits { width: 1, val: 1, x: 0 },
            Logic::X => RefBits { width: 1, val: 0, x: 1 },
        }
    }

    /// From a `Logic` slice, index 0 the LSB — the layout `Bits::as_array` and the
    /// harness's `RandStim::as_bits` use.
    pub fn from_logic_slice(bits: &[Logic]) -> Self {
        assert!(!bits.is_empty() && bits.len() <= 128, "RefBits width {} out of range", bits.len());
        let (mut val, mut x) = (0u128, 0u128);
        for (i, b) in bits.iter().enumerate() {
            match b {
                Logic::One => val |= 1 << i,
                Logic::X => x |= 1 << i,
                Logic::Zero => {}
            }
        }
        RefBits { width: bits.len() as u32, val, x }
    }

    /// To a `Logic` vector, index 0 the LSB.
    pub fn to_logic_vec(&self) -> Vec<Logic> {
        (0..self.width).map(|i| self.get(i as usize)).collect()
    }

    pub fn is_valid(&self) -> bool {
        self.x == 0
    }

    /// The value as an integer; `None` on any X (`Bits::as_uint` panics there).
    pub fn as_uint(&self) -> Option<u128> {
        if self.x == 0 { Some(self.val) } else { None }
    }

    pub fn get(&self, i: usize) -> Logic {
        assert!((i as u32) < self.width, "bit index {i} out of bounds for width {}", self.width);
        let b = 1u128 << i;
        if self.x & b != 0 { Logic::X } else if self.val & b != 0 { Logic::One } else { Logic::Zero }
    }

    pub fn set(&mut self, i: usize, l: Logic) {
        assert!((i as u32) < self.width, "bit index {i} out of bounds for width {}", self.width);
        let b = 1u128 << i;
        self.val &= !b;
        self.x &= !b;
        match l {
            Logic::One => self.val |= b,
            Logic::X => self.x |= b,
            Logic::Zero => {}
        }
    }

    pub fn as_logic(&self) -> Logic {
        assert_eq!(self.width, 1, "as_logic on width {}", self.width);
        self.get(0)
    }

    // ── bitwise, per-bit three-valued (types.rs `impl Not/BitAnd/BitOr/BitXor`) ──

    pub fn not(&self) -> Self {
        Self::canon(self.width, !self.val, self.x)
    }

    fn same_width(&self, o: &Self, op: &str) -> u32 {
        assert_eq!(self.width, o.width, "{op}: width mismatch {} vs {}", self.width, o.width);
        self.width
    }

    pub fn and(&self, o: &Self) -> Self {
        let w = self.same_width(o, "and");
        let m = Self::mask(w);
        let zero = (!self.val & !self.x) | (!o.val & !o.x);
        let ones = self.val & o.val;
        Self::canon(w, ones, m & !zero & !ones)
    }

    pub fn or(&self, o: &Self) -> Self {
        let w = self.same_width(o, "or");
        let m = Self::mask(w);
        let ones = self.val | o.val;
        let zeros = (!self.val & !self.x) & (!o.val & !o.x);
        Self::canon(w, ones, m & !ones & !zeros)
    }

    pub fn xor(&self, o: &Self) -> Self {
        let w = self.same_width(o, "xor");
        Self::canon(w, self.val ^ o.val, self.x | o.x)
    }

    // ── arithmetic: wrapping at the width, all-X on any X operand ──────────────

    fn arith(&self, o: &Self, op: &str, f: impl Fn(u128, u128) -> Option<u128>) -> Self {
        let w = self.same_width(o, op);
        match (self.as_uint(), o.as_uint()) {
            (Some(a), Some(b)) => match f(a, b) {
                Some(r) => Self::from_uint(w, r),
                None => Self::all_x(w),
            },
            _ => Self::all_x(w),
        }
    }

    pub fn add(&self, o: &Self) -> Self {
        self.arith(o, "add", |a, b| Some(a.wrapping_add(b)))
    }
    pub fn sub(&self, o: &Self) -> Self {
        self.arith(o, "sub", |a, b| Some(a.wrapping_sub(b)))
    }
    pub fn mul(&self, o: &Self) -> Self {
        self.arith(o, "mul", |a, b| Some(a.wrapping_mul(b)))
    }
    /// `Bits::div`: X on a zero divisor (and the quotient is not masked, as in
    /// `types.rs`, though it always fits).
    pub fn div(&self, o: &Self) -> Self {
        self.arith(o, "div", |a, b| if b == 0 { None } else { Some(a / b) })
    }
    pub fn rem(&self, o: &Self) -> Self {
        self.arith(o, "rem", |a, b| if b == 0 { None } else { Some(a % b) })
    }
    pub fn neg(&self) -> Self {
        match self.as_uint() {
            Some(a) => Self::from_uint(self.width, a.wrapping_neg()),
            None => Self::all_x(self.width),
        }
    }

    // ── shifts: logical, X moves with the data (`shift_left` / `shift_right`) ───

    pub fn shl(&self, n: usize) -> Self {
        if n >= 128 { return Self::zero(self.width); }
        Self::canon(self.width, self.val << n, self.x << n)
    }
    pub fn shr(&self, n: usize) -> Self {
        if n >= 128 { return Self::zero(self.width); }
        Self::canon(self.width, self.val >> n, self.x >> n)
    }
    /// `arithmetic_shift_right`: the vacated bits take the sign (MSB), X included.
    pub fn arithmetic_shift_right(&self, n: usize) -> Self {
        let w = self.width as usize;
        let sign = self.get(w - 1);
        let mut r = self.shr(n);
        let keep = w.saturating_sub(n);
        for i in keep..w {
            r.set(i, sign);
        }
        r
    }

    pub fn with_lsb(&self, l: Logic) -> Self {
        let mut r = *self;
        r.set(0, l);
        r
    }
    pub fn with_msb(&self, l: Logic) -> Self {
        let mut r = *self;
        r.set(self.width as usize - 1, l);
        r
    }

    // ── reductions (fold with the three-valued Logic operators) ────────────────

    pub fn and_reduce(&self) -> Logic {
        self.to_logic_vec().into_iter().fold(Logic::One, |a, b| a & b)
    }
    pub fn or_reduce(&self) -> Logic {
        self.to_logic_vec().into_iter().fold(Logic::Zero, |a, b| a | b)
    }
    pub fn xor_reduce(&self) -> Logic {
        self.to_logic_vec().into_iter().fold(Logic::Zero, |a, b| a ^ b)
    }

    // ── X-aware comparisons (`eq_logic` and friends) ───────────────────────────

    pub fn eq_logic(&self, o: &Self) -> Logic {
        self.same_width(o, "eq_logic");
        // IEEE 1800 `==` (SPEC I.14.2(h)): a known differing bit pair decides 0.
        let known = Self::mask(self.width) & !(self.x | o.x);
        if (self.val ^ o.val) & known != 0 { return Logic::Zero; }
        if !self.is_valid() || !o.is_valid() { return Logic::X; }
        Logic::One
    }
    fn cmp_logic(&self, o: &Self, f: impl Fn(u128, u128) -> bool) -> Logic {
        match (self.as_uint(), o.as_uint()) {
            (Some(a), Some(b)) => Logic::from_bool(f(a, b)),
            _ => Logic::X,
        }
    }
    pub fn lt_logic(&self, o: &Self) -> Logic { self.cmp_logic(o, |a, b| a < b) }
    pub fn le_logic(&self, o: &Self) -> Logic { self.cmp_logic(o, |a, b| a <= b) }
    pub fn gt_logic(&self, o: &Self) -> Logic { self.cmp_logic(o, |a, b| a > b) }
    pub fn ge_logic(&self, o: &Self) -> Logic { self.cmp_logic(o, |a, b| a >= b) }

    // ── width conversion ───────────────────────────────────────────────────────

    pub fn zero_extend(&self, m: u32) -> Self {
        assert!(m >= self.width, "zero_extend: {m} < {}", self.width);
        RefBits { width: m, val: self.val, x: self.x }
    }
    pub fn sign_extend(&self, m: u32) -> Self {
        assert!(m >= self.width, "sign_extend: {m} < {}", self.width);
        let sign = self.get(self.width as usize - 1);
        let mut r = RefBits { width: m, val: self.val, x: self.x };
        for i in self.width..m {
            r.set(i as usize, sign);
        }
        r
    }
    pub fn truncate(&self, m: u32) -> Self {
        assert!(m <= self.width, "truncate: {m} > {}", self.width);
        Self::canon(m, self.val, self.x)
    }
    pub fn part_select(&self, lo: usize, len: u32) -> Self {
        assert!(lo + len as usize <= self.width as usize, "part_select out of bounds");
        Self::canon(len, self.val >> lo, self.x >> lo)
    }
    /// `self.concat(other)`: `self` becomes the HIGH part (`{self, other}`).
    pub fn concat(&self, other: &Self) -> Self {
        let w = self.width + other.width;
        assert!(w <= 128, "concat width {w} > 128");
        let s = other.width;
        RefBits { width: w, val: (self.val << s) | other.val, x: (self.x << s) | other.x }
    }
    pub fn replicate(&self, out: u32) -> Self {
        assert_eq!(out % self.width, 0, "replicate: {out} not a multiple of {}", self.width);
        let mut r = RefBits::zero(out);
        let mut i = 0;
        while i < out {
            r.val |= self.val << i;
            r.x |= self.x << i;
            i += self.width;
        }
        r
    }
    /// `Bits::mux(sel, a, b)`: `a` on One, `b` on Zero, bitwise agreement on X.
    pub fn mux(sel: Logic, a: &Self, b: &Self) -> Self {
        match sel {
            Logic::One => *a,
            Logic::Zero => *b,
            Logic::X => {
                let w = a.same_width(b, "mux");
                let agree = !(a.val ^ b.val) & !(a.x | b.x);
                Self::canon(w, a.val & agree, Self::mask(w) & !agree)
            }
        }
    }
}

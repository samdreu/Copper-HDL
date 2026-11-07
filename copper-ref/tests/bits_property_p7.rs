//! **P7 — the operators the `RefBits` ≡ `Bits<N>` pin did not cover** (SPEC I.14.8's
//! III.1 list, D-20 P7; Phase C step 8, `design_docs/review/MEASUREMENTS.md` §M).
//!
//! `bits_property.rs` compares every `RefBits` operator against `copper-core` at
//! widths {1, 4, 8, 16, 32, 64, 128} with shift amounts up to N+1. D-19 (ii) listed
//! what it left out; each is pinned here against the same oracle (`copper-core`):
//!
//! - the constructors: `zero`, `all_ones`, `x`, `from_lit`, `from_u8 … from_u128`,
//!   `from_usize`, `from_array`, `from_slice`;
//! - the views `as_u8 … as_u64` / `as_usize` (and `as_u128` on X);
//! - shift amounts above N+1 (to 3N+5), logical and arithmetic;
//! - `Logic`'s `!` and `as_bool` (X: a panic in the simulator, `None` here);
//! - `From` between `Logic` and `Bits<1>`; `as_array`;
//! - widths outside {1, 4, …, 128}: 2, 3, 5, 7, 12, 33, 63, 65, 100, 127;
//! - the interpreter's integer, `bool` and array port values (the `Value` layer, which
//!   `RefBits` does not model): `to_bits` / `from_bits` against the integer's bits.

use copper_core::types::Bits;
use copper_core::Logic;
use copper_ref::{IntTy, RefBits, Ty, Value};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn logic(&mut self, x_rate: u64) -> Logic {
        let r = self.next() % 100;
        if r < x_rate { Logic::X } else if r % 2 == 0 { Logic::Zero } else { Logic::One }
    }
    fn u128(&mut self) -> u128 {
        ((self.next() as u128) << 64) | self.next() as u128
    }
}

fn rand_bits<const N: usize>(rng: &mut Rng, x_rate: u64) -> Bits<N> {
    let mut b = Bits::<N>::zero();
    for i in 0..N {
        b.as_array_mut()[i] = rng.logic(x_rate);
    }
    b
}

fn same<const N: usize>(what: &str, r: RefBits, b: Bits<N>) {
    assert_eq!(r.width as usize, N, "{what}: width");
    assert_eq!(r.to_logic_vec(), b.as_array().to_vec(), "{what}: {r:?} vs {b:?}");
}

fn mask(n: usize) -> u128 {
    if n >= 128 { u128::MAX } else { (1u128 << n) - 1 }
}

/// Constructors, views, long shifts and `as_array` at one width.
fn check<const N: usize>(rng: &mut Rng) {
    let w = N as u32;
    same("zero", RefBits::zero(w), Bits::<N>::zero());
    same("all_ones", RefBits::all_ones(w), Bits::<N>::all_ones());
    same("x", RefBits::all_x(w), Bits::<N>::x());
    for _ in 0..200 {
        let v = rng.u128() & mask(N);
        if N <= 64 {
            // `from_usize` checks the fit at run time (the `from_uN` and `as_uN`
            // width checks are post-mono asserts: `ctor_and_view_widths` below).
            same("from_usize", RefBits::from_uint(w, v), Bits::<N>::from_usize(v as usize));
        }
        // `from_array` / `from_slice`, X included, and `as_array` back.
        let a = rand_bits::<N>(rng, 20);
        let arr = *a.as_array();
        same("from_array", RefBits::from_logic_slice(&arr), Bits::<N>::from_array(arr));
        same("from_slice", RefBits::from_logic_slice(&arr[..]), Bits::<N>::from_slice(&arr[..]));
        assert_eq!(RefBits::from_logic_slice(&arr).to_logic_vec(), a.as_array().to_vec(), "as_array");
        let k = rand_bits::<N>(rng, 0);
        assert_eq!(RefBits::from_logic_slice(k.as_array()).as_uint(), Some(k.as_u128()), "as_u128");
        // An X bit: the simulator panics ("Cannot convert X to integer"), the
        // interpreter's view is `None` — the same run-time failure (I.14.2(c)).
        let xa = rand_bits::<N>(rng, 50);
        if !xa.is_valid() {
            assert_eq!(RefBits::from_logic_slice(xa.as_array()).as_uint(), None, "view of X");
            let r = std::panic::catch_unwind(|| xa.as_u128());
            assert!(r.is_err(), "the simulator's view of an X value panics");
        }
        // Shift amounts past N+1, up to 3N+5.
        let n = (rng.next() % (3 * N as u64 + 6)) as usize;
        let ra = RefBits::from_logic_slice(a.as_array());
        same("shl long", ra.shl(n), a.shift_left(n));
        same("shr long", ra.shr(n), a.shift_right(n));
        same("<< long", ra.shl(n), a << n);
        same(">> long", ra.shr(n), a >> n);
        same("asr long", ra.arithmetic_shift_right(n), a.arithmetic_shift_right(n));
    }
}

#[test]
fn constructors_views_and_long_shifts_match_bits_n() {
    let mut rng = Rng(0x5EED_2026_11_07);
    // The widths bits_property.rs covers, then the ones it did not.
    check::<1>(&mut rng);
    check::<4>(&mut rng);
    check::<8>(&mut rng);
    check::<16>(&mut rng);
    check::<32>(&mut rng);
    check::<64>(&mut rng);
    check::<128>(&mut rng);
    check::<2>(&mut rng);
    check::<3>(&mut rng);
    check::<5>(&mut rng);
    check::<7>(&mut rng);
    check::<12>(&mut rng);
    check::<33>(&mut rng);
    check::<63>(&mut rng);
    check::<65>(&mut rng);
    check::<100>(&mut rng);
    check::<127>(&mut rng);
}

/// The binary operators of bits_property.rs, at the widths it did not run.
fn ops<const N: usize>(rng: &mut Rng) {
    for x_rate in [0u64, 10, 50] {
        for _ in 0..200 {
            let (a, b) = (rand_bits::<N>(rng, x_rate), rand_bits::<N>(rng, x_rate));
            let (ra, rb) = (RefBits::from_logic_slice(a.as_array()), RefBits::from_logic_slice(b.as_array()));
            same("not", ra.not(), !a);
            same("and", ra.and(&rb), a & b);
            same("or", ra.or(&rb), a | b);
            same("xor", ra.xor(&rb), a ^ b);
            same("add", ra.add(&rb), a + b);
            same("sub", ra.sub(&rb), a - b);
            same("mul", ra.mul(&rb), a * b);
            same("div", ra.div(&rb), a / b);
            same("rem", ra.rem(&rb), a % b);
            same("neg", ra.neg(), -a);
            assert_eq!(ra.eq_logic(&rb), a.eq_logic(&b), "eq_logic");
            assert_eq!(!ra.eq_logic(&rb), a.ne_logic(&b), "ne_logic");
            assert_eq!(ra.lt_logic(&rb), a.lt_logic(&b), "lt_logic");
            assert_eq!(ra.ge_logic(&rb), a.ge_logic(&b), "ge_logic");
            assert_eq!(ra.and_reduce(), a.and_reduce(), "and_reduce");
            assert_eq!(ra.xor_reduce(), a.xor_reduce(), "xor_reduce");
        }
    }
}

#[test]
fn the_operators_match_bits_n_at_widths_outside_the_first_pin() {
    let mut rng = Rng(0xA11_0DD5);
    ops::<2>(&mut rng);
    ops::<3>(&mut rng);
    ops::<5>(&mut rng);
    ops::<7>(&mut rng);
    ops::<12>(&mut rng);
    ops::<33>(&mut rng);
    ops::<63>(&mut rng);
    ops::<65>(&mut rng);
    ops::<100>(&mut rng);
    ops::<127>(&mut rng);
}

#[test]
fn from_lit_matches_bits_n() {
    same("from_lit 0xA5", RefBits::from_uint(8, 0xA5), Bits::<8>::from_lit::<0xA5>());
    same("from_lit 1_000", RefBits::from_uint(16, 1000), Bits::<16>::from_lit::<1_000>());
    same("from_lit 1", RefBits::from_uint(1, 1), Bits::<1>::from_lit::<1>());
    same("from_lit max", RefBits::from_uint(128, u128::MAX), Bits::<128>::from_lit::<{ u128::MAX }>());
}

#[test]
fn logic_not_as_bool_and_from_match() {
    for l in [Logic::Zero, Logic::One, Logic::X] {
        let r = RefBits::from_logic(l);
        assert_eq!(r.not().as_logic(), !l, "!{l:?}");
        // `as_bool`: the simulator panics on X, the interpreter's integer view is None.
        match l {
            Logic::X => {
                assert_eq!(r.as_uint(), None);
                assert!(std::panic::catch_unwind(|| l.as_bool()).is_err(), "as_bool(X) panics");
            }
            _ => assert_eq!(r.as_uint() == Some(1), l.as_bool(), "as_bool {l:?}"),
        }
        // `From<Logic> for Bits<1>` and back: the identity on the bit.
        let b1: Bits<1> = Bits::from(l);
        same("Bits::from(Logic)", r, b1);
        assert_eq!(Logic::from(b1), r.as_logic(), "Logic::from(Bits<1>)");
        let into: Bits<1> = l.into();
        same("Logic.into()", r, into);
    }
}

/// The interpreter's integer and `bool` port values (Phase C step 8, I.14.7 (10)):
/// a port's bits, LSB first, are the integer's two's-complement bits; a value read
/// back from them is the same integer. Arrays are element-major.
#[test]
fn integer_bool_and_array_port_values_round_trip() {
    let mut rng = Rng(42);
    let tys = [
        (IntTy::U8, 8usize), (IntTy::U16, 16), (IntTy::U32, 32), (IntTy::U64, 64), (IntTy::Usize, 64),
        (IntTy::I8, 8), (IntTy::I16, 16), (IntTy::I32, 32), (IntTy::I64, 64), (IntTy::Isize, 64),
    ];
    for (t, w) in tys {
        assert_eq!(Ty::Int(t).bit_width(), Some(w), "{t:?} width");
        let (lo, hi) = t.range();
        for _ in 0..200 {
            let raw = (rng.next() as i128) & mask(w) as i128;
            let v = if lo < 0 && (raw >> (w - 1)) & 1 == 1 { raw - (1i128 << w) } else { raw };
            assert!(v >= lo && v <= hi);
            let bits = Value::Int(v, t).to_bits().unwrap();
            assert_eq!(bits.len(), w);
            for (k, b) in bits.iter().enumerate() {
                assert_eq!(*b == Logic::One, (raw >> k) & 1 == 1, "{t:?} {v} bit {k}");
            }
            assert_eq!(Value::from_bits(&Ty::Int(t), &bits).unwrap(), Value::Int(v, t), "{t:?} round trip");
        }
        // An X bit on an integer port has no value (I.14.2(a)).
        let mut xb = vec![Logic::Zero; w];
        xb[0] = Logic::X;
        assert!(Value::from_bits(&Ty::Int(t), &xb).is_err(), "{t:?}: X has no integer value");
    }
    assert_eq!(Ty::Bool.bit_width(), Some(1));
    for b in [false, true] {
        let bits = Value::Bool(b).to_bits().unwrap();
        assert_eq!(bits, vec![if b { Logic::One } else { Logic::Zero }]);
        assert_eq!(Value::from_bits(&Ty::Bool, &bits).unwrap(), Value::Bool(b));
    }
    assert!(Value::from_bits(&Ty::Bool, &[Logic::X]).is_err(), "bool has no X");
    let arr = Ty::Array(Box::new(Ty::Int(IntTy::U8)), 3);
    let v = Value::Array(vec![Value::Int(1, IntTy::U8), Value::Int(0x80, IntTy::U8), Value::Int(0xFF, IntTy::U8)]);
    let bits = v.to_bits().unwrap();
    assert_eq!(bits.len(), 24);
    assert_eq!(Value::from_bits(&arr, &bits).unwrap(), v, "array of u8 round trip");
}

/// `from_uN` needs `N >= the integer's width` and `as_uN` `N <= it` — post-mono
/// asserts in `types.rs`, so each pair is instantiated where it compiles.
macro_rules! ctor_at {
    ($rng:expr, $n:literal, $ctor:ident, $t:ty) => {{
        for _ in 0..100 {
            let v = ($rng.u128() as $t) as u128;
            same(stringify!($ctor), RefBits::from_uint($n, v), Bits::<$n>::$ctor(v as $t));
        }
    }};
}
macro_rules! view_at {
    ($rng:expr, $n:literal, $view:ident) => {{
        for _ in 0..100 {
            let k = rand_bits::<$n>(&mut $rng, 0);
            let r = RefBits::from_logic_slice(k.as_array()).as_uint().expect("known");
            assert_eq!(r, k.$view() as u128, "{} at {}", stringify!($view), $n);
            let mut x = k;
            x.as_array_mut()[0] = Logic::X;
            assert_eq!(RefBits::from_logic_slice(x.as_array()).as_uint(), None);
            assert!(std::panic::catch_unwind(|| x.$view()).is_err(), "{} of X panics", stringify!($view));
        }
    }};
}

#[test]
fn ctor_and_view_widths() {
    let mut rng = Rng(0xFEED);
    ctor_at!(rng, 8, from_u8, u8);
    ctor_at!(rng, 12, from_u8, u8);
    ctor_at!(rng, 128, from_u8, u8);
    ctor_at!(rng, 16, from_u16, u16);
    ctor_at!(rng, 33, from_u16, u16);
    ctor_at!(rng, 32, from_u32, u32);
    ctor_at!(rng, 65, from_u32, u32);
    ctor_at!(rng, 64, from_u64, u64);
    ctor_at!(rng, 100, from_u64, u64);
    ctor_at!(rng, 128, from_u128, u128);
    view_at!(rng, 1, as_u8);
    view_at!(rng, 5, as_u8);
    view_at!(rng, 8, as_u8);
    view_at!(rng, 12, as_u16);
    view_at!(rng, 16, as_u16);
    view_at!(rng, 33, as_u64);
    view_at!(rng, 32, as_u32);
    view_at!(rng, 3, as_u32);
    view_at!(rng, 64, as_u64);
    view_at!(rng, 63, as_usize);
    view_at!(rng, 64, as_usize);
    view_at!(rng, 7, as_usize);
    view_at!(rng, 127, as_u128);
}

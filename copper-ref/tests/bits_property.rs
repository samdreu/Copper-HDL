//! **The `RefBits` ≡ `Bits<N>` pin** — the first executable check of SPEC §I.14.
//!
//! Every operator and method of `RefBits` is compared against the `Bits<N>` /
//! `Logic` implementation in `copper-core/src/types.rs` at widths {1, 4, 8, 16,
//! 32, 64, 128}, on seeded random operands with X bits sprinkled in. The oracle is
//! `copper-core` itself: the interpreter must compute exactly what the simulator's
//! values compute, or a disagreement between the two legs would be the value
//! model's and not the semantics'.

use copper_core::types::Bits;
use copper_core::Logic;
use copper_ref::RefBits;

/// SplitMix64 — the same generator the harness uses, so a failure is reproducible
/// from the seed.
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
}

fn rand_bits<const N: usize>(rng: &mut Rng, x_rate: u64) -> Bits<N> {
    let mut b = Bits::<N>::zero();
    for i in 0..N {
        b.as_array_mut()[i] = rng.logic(x_rate);
    }
    b
}

fn to_ref<const N: usize>(b: &Bits<N>) -> RefBits {
    RefBits::from_logic_slice(b.as_array())
}

fn same<const N: usize>(what: &str, r: RefBits, b: Bits<N>) {
    assert_eq!(r.width as usize, N, "{what}: width");
    assert_eq!(r.to_logic_vec(), b.as_array().to_vec(), "{what}: {r:?} vs {b:?}");
}

fn same_logic(what: &str, r: Logic, l: Logic) {
    assert_eq!(r, l, "{what}");
}

fn check_width<const N: usize>(rng: &mut Rng, rounds: usize) {
    for x_rate in [0u64, 10, 50] {
        for _ in 0..rounds {
            let a = rand_bits::<N>(rng, x_rate);
            let b = rand_bits::<N>(rng, x_rate);
            let (ra, rb) = (to_ref(&a), to_ref(&b));
            // Round trip.
            same("roundtrip", ra, a);
            // Bitwise.
            same("not", ra.not(), !a);
            same("and", ra.and(&rb), a & b);
            same("or", ra.or(&rb), a | b);
            same("xor", ra.xor(&rb), a ^ b);
            // Arithmetic.
            same("add", ra.add(&rb), a + b);
            same("sub", ra.sub(&rb), a - b);
            same("mul", ra.mul(&rb), a * b);
            same("div", ra.div(&rb), a / b);
            same("rem", ra.rem(&rb), a % b);
            same("neg", ra.neg(), -a);
            // Shifts.
            let n = (rng.next() % (N as u64 + 2)) as usize;
            same("shl", ra.shl(n), a.shift_left(n));
            same("shr", ra.shr(n), a.shift_right(n));
            same("shl op", ra.shl(n), a << n);
            same("shr op", ra.shr(n), a >> n);
            same("asr", ra.arithmetic_shift_right(n), a.arithmetic_shift_right(n));
            // Single-bit edits.
            let l = rng.logic(x_rate);
            same("with_lsb", ra.with_lsb(l), a.with_lsb(l));
            same("with_msb", ra.with_msb(l), a.with_msb(l));
            let i = (rng.next() % N as u64) as usize;
            same_logic("get", ra.get(i), a.get(i));
            let mut ma = a;
            ma.set(i, l);
            let mut mra = ra;
            mra.set(i, l);
            same("set", mra, ma);
            // Reductions.
            same_logic("and_reduce", ra.and_reduce(), a.and_reduce());
            same_logic("or_reduce", ra.or_reduce(), a.or_reduce());
            same_logic("xor_reduce", ra.xor_reduce(), a.xor_reduce());
            same_logic("nand_reduce", !ra.and_reduce(), a.nand_reduce());
            same_logic("nor_reduce", !ra.or_reduce(), a.nor_reduce());
            same_logic("xnor_reduce", !ra.xor_reduce(), a.xnor_reduce());
            // Comparisons: the derived `==` (X == X) and the X-aware family.
            assert_eq!(ra == rb, a == b, "derived eq");
            assert_eq!(ra == ra, a == a, "derived eq self (X == X)");
            same_logic("eq_logic", ra.eq_logic(&rb), a.eq_logic(&b));
            same_logic("ne_logic", !ra.eq_logic(&rb), a.ne_logic(&b));
            same_logic("lt_logic", ra.lt_logic(&rb), a.lt_logic(&b));
            same_logic("le_logic", ra.le_logic(&rb), a.le_logic(&b));
            same_logic("gt_logic", ra.gt_logic(&rb), a.gt_logic(&b));
            same_logic("ge_logic", ra.ge_logic(&rb), a.ge_logic(&b));
            // Validity and the integer view.
            assert_eq!(ra.is_valid(), a.is_valid(), "is_valid");
            if a.is_valid() && N <= 128 {
                assert_eq!(ra.as_uint(), Some(a.as_u128()), "as_u128");
            } else {
                assert_eq!(ra.as_uint(), None, "as_uint on X");
            }
            // Mux.
            let sel = rng.logic(x_rate);
            same("mux", RefBits::mux(sel, &ra, &rb), Bits::mux(sel, &a, &b));
        }
    }
}

#[test]
fn every_refbits_op_matches_bits_n_at_every_width() {
    let mut rng = Rng(0xC0FFEE_2026_11_05);
    check_width::<1>(&mut rng, 400);
    check_width::<4>(&mut rng, 400);
    check_width::<8>(&mut rng, 400);
    check_width::<16>(&mut rng, 400);
    check_width::<32>(&mut rng, 400);
    check_width::<64>(&mut rng, 400);
    check_width::<128>(&mut rng, 400);
}

#[test]
fn width_conversions_match_bits_n() {
    let mut rng = Rng(7);
    for x_rate in [0u64, 30] {
        for _ in 0..200 {
            let a = rand_bits::<8>(&mut rng, x_rate);
            let b = rand_bits::<4>(&mut rng, x_rate);
            let (ra, rb) = (to_ref(&a), to_ref(&b));
            same("zero_extend", ra.zero_extend(16), a.zero_extend::<16>());
            same("sign_extend", ra.sign_extend(16), a.sign_extend::<16>());
            same("truncate", ra.truncate(4), a.truncate::<4>());
            same("truncate 1", ra.truncate(1), a.truncate::<1>());
            let lo = (rng.next() % 5) as usize;
            same("part_select", ra.part_select(lo, 4), a.part_select::<4>(lo));
            same("concat", ra.concat(&rb), a.concat::<4, 12>(&b));
            same("concat rev", rb.concat(&ra), b.concat::<8, 12>(&a));
            same("replicate", rb.replicate(16), b.replicate::<16>());
            same("replicate 1", rb.replicate(4), b.replicate::<4>());
        }
    }
}

#[test]
fn logic_as_width_one_matches_logic_ops() {
    for a in [Logic::Zero, Logic::One, Logic::X] {
        for b in [Logic::Zero, Logic::One, Logic::X] {
            let (ra, rb) = (RefBits::from_logic(a), RefBits::from_logic(b));
            assert_eq!(ra.and(&rb).as_logic(), a & b, "{a:?} & {b:?}");
            assert_eq!(ra.or(&rb).as_logic(), a | b, "{a:?} | {b:?}");
            assert_eq!(ra.xor(&rb).as_logic(), a ^ b, "{a:?} ^ {b:?}");
            assert_eq!(ra.not().as_logic(), !a, "!{a:?}");
            assert_eq!(ra == rb, a == b, "{a:?} == {b:?}");
        }
    }
}

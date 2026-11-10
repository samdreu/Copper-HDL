// Bit-operator lowering — one module per claim about what `Bits` operators emit.
//
// Every module here transpiles. That is the point: the failures in this file are
// WRONG CODE, not refusals, so the only gate that separates them from the working
// spelling is the differential sweep (and, for two of them, Verilator's `-Wall`).
// Each broken module is paired with the spelling that works, so a fix shows up as
// a SKIP entry that can be deleted rather than as a test nobody wrote.

/// `!` on a `Bits<N>` — a BITWISE `~` now (fixed 2026-08-27; width- and
/// boolean-guarded, so `!` on a bool or a comparison stays logical). Was
/// emitted as SystemVerilog LOGICAL `!`, collapsing the result to one bit —
/// found via `rv32i_cpu_pipelined`'s JALR alignment mask, where it turned every
/// jump target into 0. Sweeps green on three legs. Not anchored: its former
/// `REFERENCE` file `reference_sv/bit_not_bits.sv` was written in a session, not
/// independently, and its row was removed on 2026-11-10 (MEASUREMENTS §AK).
#[hardware(combinational)]
pub fn bit_not_bits(
    a: In<Bits<32>, MainClk>,
    o: Out<Bits<32>, MainClk>,
) {
    o.write(!a.read());
}

/// The working spelling: XOR with an all-ones mask is the same function and
/// lowers to a real bitwise operation.
#[hardware(combinational)]
pub fn bit_not_via_xor(
    a: In<Bits<32>, MainClk>,
    o: Out<Bits<32>, MainClk>,
) {
    o.write(a.read() ^ Bits::<32>::from_u32(0xFFFF_FFFF));
}

/// `!` on a **bool** is one bit and is correct — the bug above is specific to
/// `Bits`. Without this, a fix that made `!` bitwise everywhere would look right.
#[hardware(combinational)]
pub fn bit_not_bool(
    a: In<Bits<32>, MainClk>,
    o: Out<Logic, MainClk>,
) {
    let z: bool = a.read() == Bits::zero();
    if !z { o.write(Logic::One); } else { o.write(Logic::Zero); }
}

/// A constructor call with no sibling operand to take its width from. Both arms
/// are `Bits<32>` in the source; the emitted literals are 32 bits (fixed
/// 2026-08-27: the type-position turbofish pins `from_lit`'s width, and a
/// default-width literal mux arm takes the other arm's width like a binop
/// operand does). Sweeps green.
#[hardware(combinational)]
pub fn lit_width_in_ternary(
    a: In<Bits<32>, MainClk>,
    o: Out<Bits<32>, MainClk>,
) {
    o.write(if a.read() == Bits::zero() {
        Bits::<32>::from_lit::<1>()
    } else {
        Bits::zero()
    });
}

/// The same value through named, explicitly-typed locals — the spelling that
/// pins the width at the `let`.
#[hardware(combinational)]
pub fn lit_width_via_locals(
    a: In<Bits<32>, MainClk>,
    o: Out<Bits<32>, MainClk>,
) {
    let one: Bits<32> = Bits::<32>::from_u32(1);
    let zero: Bits<32> = Bits::<32>::from_u32(0);
    o.write(if a.read() == Bits::zero() { one } else { zero });
}

/// A shift by a run-time `usize`, the shape every ALU shamt path uses.
#[hardware(combinational)]
pub fn dynamic_shift(
    a: In<Bits<32>, MainClk>,
    n: In<Bits<5>, MainClk>,
    o: Out<Bits<32>, MainClk>,
) {
    let sh: usize = n.read().as_usize();
    o.write(a.read() >> sh);
}

/// Arithmetic shift right built out of supported primitives, since
/// `arithmetic_shift_right` is not a lowerable method. `ones ^ (ones >> n)` is
/// the high-n-bits mask.
#[hardware(combinational)]
pub fn manual_sra(
    a: In<Bits<32>, MainClk>,
    n: In<Bits<5>, MainClk>,
    o: Out<Bits<32>, MainClk>,
) {
    let v: Bits<32> = a.read();
    let sh: usize = n.read().as_usize();
    let logical: Bits<32> = v >> sh;
    let ones: Bits<32> = Bits::<32>::from_u32(0xFFFF_FFFF);
    let neg: bool = ((v >> 31) & Bits::<32>::from_u32(1)) == Bits::<32>::from_u32(1);
    o.write(if neg && sh != 0 { logical | (ones ^ (ones >> sh)) } else { logical });
}

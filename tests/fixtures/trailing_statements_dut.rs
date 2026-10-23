// Trailing combinational statements — statements after the LAST `clk.tick().await`
// of a multi-tick loop.
//
// The semantics, decided 2026-08-25 from `design_docs/SYNCHRONOUS_SEMANTICS.md`
// ("a clock cycle is a maximal tick-free region of an execution"): the trailing
// statements and the head's are ONE cycle, because falling off the end of the body
// and re-entering it costs no clock. Their combinational logic therefore belongs to
// phase 0, and the simulator runs them BEFORE the head's — the trailing statements in
// the post-edge settle of the tick that opens the cycle, the head's in the pre-edge
// settle of the tick that closes it.
//
// The multi-tick path used to refuse these outright, while the single-tick path
// hoisted them and the extracted path accepted them; the corpus sweep generates a
// differential case for each module here, so the decision is checked against the
// emitted SystemVerilog rather than asserted.

/// An output driven in the trailing segment, alongside one driven mid-loop. This is
/// the shape the multi-tick path refused.
///
/// `tail` is a `RegOut`: a `RegOut` write takes effect at the first clock edge
/// after it executes, so a trailing write publishes at the next edge on both
/// sides. (A plain `Out` driven from a register here was refused by
/// `unprotected_trailing_out_write` until phase C landed on 2026-10-23; the
/// extraction route now emits such a drive as a forwarded combinational mux with
/// a hold register, and the linear route already committed it at the right edge,
/// so the rule is retired.) A plain `Out` written with a CONSTANT only past the
/// second edge is refused by `out_first_driven_late` (a startup transient: the
/// simulator shows the initial value at the first observation, the `assign`
/// never does); that shape was `trailing_constant` here and is now
/// `copper-macros/tests/ui/fail/out_first_driven_late.rs`. The agreeing half of
/// its flipping pair is `late_constant_one_edge` below.
#[hardware(sequential)]
async fn trailing_out(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    mid: Out<Bits<8>, MainClk>,
    tail: RegOut<Bits<8>, MainClk>,
) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = a.read();
        clk.tick().await;
        mid.write(r);
        clk.tick().await;
        tail.write(r);
    }
}

/// A plain local computed in the trailing segment and consumed by the HEAD. It lives
/// across no tick, so it is a wire and not a register — which makes it the one thing
/// that can observe the emission order within the merged cycle. If the head were
/// emitted first it would read a stale value.
#[hardware(sequential)]
async fn trailing_local_into_head(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    o: Out<Bits<8>, MainClk>,
) {
    let mut r: Bits<8> = Bits::zero();
    let mut w: Bits<8> = Bits::zero();
    loop {
        o.write(w + r);
        clk.tick().await;
        r = a.read();
        clk.tick().await;
        w = r + Bits::from_lit::<1>();
    }
}

/// The flipping witness for `out_first_driven_late`'s boundary (measured 2026-10-23):
/// a constant written ONE edge late — in the segment the first tick enters — AGREES
/// with its SystemVerilog, while the same statement one tick later
/// (`trailing_constant`, now `copper-macros/tests/ui/fail/out_first_driven_late.rs`)
/// mismatches at cycle 0 only. That segment runs in edge 0's post-edge settle, so
/// the write is visible at the first observation, exactly when `assign dv = 1'b1`
/// is. The rule therefore keys on the SECOND edge, and this module sweeps as a
/// positive case to keep that boundary measured rather than argued.
#[hardware(sequential)]
async fn late_constant_one_edge(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    dv: Out<Logic, MainClk>,
    o: Out<Bits<8>, MainClk>,
) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = a.read();
        clk.tick().await;
        dv.write(Logic::One);
        o.write(r);
        clk.tick().await;
    }
}

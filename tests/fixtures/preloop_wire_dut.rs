// SPEC I.2.3 / I.7.2(c), D-21 C2 (2026-11-07, `design_docs/review/MEASUREMENTS.md` §N):
// a pre-loop `let` that register inference does not call a register is a wire of the
// first resumption, whatever its initializer — R11 is the power-on rule for registers
// only. Step 6's throwaway fixture (§K.12 item 6), made permanent: `trailing_local_into_head`
// (tests/fixtures/trailing_statements_dut.rs) with `w`'s initializer an input read. `w` is
// read by the first resumption's head and rewritten by every trailing segment before the
// next head reads it, so the read reaches only obs_1.

/// `w` is a wire: the first resumption reads `a` once for it; every later head reads the
/// trailing segment's `r + 1`.
#[hardware(sequential)]
async fn preloop_wire_read(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    o: RegOut<Bits<8>, MainClk>,
) {
    let mut r: Bits<8> = Bits::zero();
    let mut w: Bits<8> = a.read();
    loop {
        o.write(w + r);
        clk.tick().await;
        r = a.read();
        clk.tick().await;
        w = r + Bits::from_lit::<1>();
    }
}

/// The single-tick spelling: an immutable pre-loop read consumed by the first head only.
#[hardware(sequential)]
async fn preloop_wire_first_head(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    o: RegOut<Bits<8>, MainClk>,
) {
    let mut w = a.read();
    loop {
        o.write(w);
        clk.tick().await;
        w = a.read() + Bits::from_lit::<1>();
    }
}

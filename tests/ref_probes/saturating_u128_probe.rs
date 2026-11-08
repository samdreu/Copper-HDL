// `saturating_add` / `saturating_sub` on `u128` (SPEC II.3.3 (e), MEASUREMENTS §U.7): the
// unsigned clamp at 128 bits. The ports are 64-bit halves because the Verilator testbench
// compares a port through a 64-bit integer. Not under `tests/fixtures/` because the sweep's
// reference leg cannot run it — copper-ref's `u128` is `(0, i128::MAX)`, and a value at or above
// 2^127 overflows its `i128` arithmetic (a recorded interpreter follow-up).
// `tests/saturating_u128_equivalence.rs` checks the simulator against the transpiled SV instead.

#[hardware(sequential)]
async fn sat_u128(
    clk: Clock<MainClk>,
    a_hi: In<Bits<64>, MainClk>,
    a_lo: In<Bits<64>, MainClk>,
    b_hi: In<Bits<64>, MainClk>,
    b_lo: In<Bits<64>, MainClk>,
    s_hi: RegOut<Bits<64>, MainClk>,
    s_lo: RegOut<Bits<64>, MainClk>,
    t_hi: RegOut<Bits<64>, MainClk>,
    t_lo: RegOut<Bits<64>, MainClk>,
) {
    loop {
        let x: u128 = ((a_hi.read().as_u64() as u128) << 64) | (a_lo.read().as_u64() as u128);
        let y: u128 = ((b_hi.read().as_u64() as u128) << 64) | (b_lo.read().as_u64() as u128);
        let s: u128 = x.saturating_add(y);
        let t: u128 = x.saturating_sub(y);
        s_hi.write(Bits::from_u64((s >> 64) as u64));
        s_lo.write(Bits::from_u64(s as u64));
        t_hi.write(Bits::from_u64((t >> 64) as u64));
        t_lo.write(Bits::from_u64(t as u64));
        clk.tick().await;
    }
}

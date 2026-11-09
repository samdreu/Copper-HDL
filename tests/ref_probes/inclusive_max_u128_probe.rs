// A tick-bearing `for` over a `u128` inclusive range whose run-time end reaches `u128::MAX` (D-28,
// MEASUREMENTS §AB.1): no counter type holds `e + 1`, so the transpiler realizes I.8.5's done flag
// (the counter is never incremented past the end). Not under `tests/fixtures/` because the sweep's
// reference leg cannot run it — copper-ref's `u128` is `(0, i128::MAX)` and it has no `i128`.
// `tests/inclusive_max_u128.rs` checks the simulator against the transpiled SV instead.

#[hardware(sequential)]
async fn imx_u128_done(clk: Clock<MainClk>, a: In<Bits<2>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let n: u128 = u128::MAX - 3 + (a.read().as_u8() as u128);
        q.write(Bits::from_u8(0x33));
        clk.tick().await;
        for i in (u128::MAX - 1)..=n {
            q.write(Bits::from_u8(i as u8));
            clk.tick().await;
        }
    }
}

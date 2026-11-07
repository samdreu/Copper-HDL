// Startup probes for the reference-interpreter measurements
// (tests/reference_leg_measurements.rs, design_docs/review/MEASUREMENTS.md §D).
// Each was an ADMITTED-AND-DIVERGENT row of II.3: (f4) here (closed by Phase C step 4),
// (f2)/(f3) now in tests/fixtures/preloop_entry_dut.rs (closed by step 5).

// The pre-loop pair — §B probe (i) (row (f2)) and D-11's (i-b) (row (f3)) — moved to
// `tests/fixtures/preloop_entry_dut.rs` when the Phase C entry closed both (2026-11-06).

// D-11 probe (iv), II.3 row (f4): the linear route's phase-0 fold runs a trailing
// region the coroutine has not executed at edge 1 — the trailing drive's power-on
// operand shows one observation early (SV obs_1 = 5, coroutine obs_1 = 0).
#[hardware(sequential)]
async fn trailing_fold(clk: Clock<MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let r: Bits<8> = Bits::from_u8(5);
    loop {
        clk.tick().await;
        q.write(r);
    }
}

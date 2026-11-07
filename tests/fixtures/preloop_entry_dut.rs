// The ENTRY of SPEC I.9.2 — pre-loop expression statements are statements of the
// first resumption, executed ONCE, in program order before the head (I.9.3).
//
// Both were II.3 ADMITTED-AND-DIVERGENT rows until Phase C step 5 (2026-11-06,
// `design_docs/review/MEASUREMENTS.md` §J.1): the extraction route moved the
// statement into state 0 and re-ran it at every return to state 0 ((f2), §B probe
// (i): `q` re-written to 7 whenever `pc` came back to 0), and the linear route
// dropped it ((f3), D-11 probe (i-b): `obs_1 = 0` where the coroutine shows 7). Both
// routes now run it in the first resumption only, behind the `__copper_started`
// carrier. Moved here from `tests/ref_probes/startup_probes.rs` so the corpus sweep
// covers both shapes; `tests/reference_leg_measurements.rs` keeps its quiet-start
// three-leg assertion (a first-cycle `go = 1` masks the write: the head's write
// overrides it in the same resumption, D-11 probe (ii)).

/// §B probe (i), row (f2): the extraction route (a tick inside a branch).
#[hardware(sequential)]
async fn preloop_probe(clk: Clock<MainClk>, go: In<Logic, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    q.write(Bits::from_u8(7));
    loop {
        cnt = cnt + Bits::from_u8(1);
        if go.read() == Logic::One {
            clk.tick().await;
            q.write(cnt);
        }
        clk.tick().await;
    }
}

/// D-11 probe (i-b), row (f3): the same pre-loop write on the LINEAR route.
#[hardware(sequential)]
async fn preloop_linear(clk: Clock<MainClk>, go: In<Logic, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    q.write(Bits::from_u8(7));
    loop {
        cnt = cnt + Bits::from_u8(1);
        if go.read() == Logic::One {
            q.write(cnt);
        }
        clk.tick().await;
    }
}

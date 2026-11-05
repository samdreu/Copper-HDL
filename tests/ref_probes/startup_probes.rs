// Startup probes for the reference-interpreter measurements
// (tests/reference_leg_measurements.rs, design_docs/review/MEASUREMENTS.md §D):
// the pre-loop re-execution of §B and the two linear-route defects of SPEC I.9.4,
// each an ADMITTED-AND-DIVERGENT row of II.3 ((f2), (f3), (f4)). Not in
// tests/fixtures/ because they are known-divergent and the sweep must stay green;
// they become sweep fixtures with the Phase C entry (II.3.6's fix list, item 1).

// §B probe (i), II.3 row (f2): a pre-loop RegOut write under control extraction.
// The coroutine runs it once (I.9.3); the extraction route re-runs it at every
// return to state 0.
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

// D-11 probe (i-b), II.3 row (f3): the same pre-loop write on the LINEAR route,
// which drops it silently (chir_lower's `_ => {}`).
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

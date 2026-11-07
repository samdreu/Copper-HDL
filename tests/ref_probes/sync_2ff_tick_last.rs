// The one-job spelling of the two-flop synchronizer (SPEC I.7.5, measured
// 2026-11-04 identical to examples/cdc/sv/sync_2ff_ref.sv): `ff2` is a wire of
// one resumption (dead at the tick), `ff1` a register, `q` the second flop. The
// poison run's first NEGATIVE (ff2) and first POSITIVE (ff1) cases (I.7.7).
#[hardware(synchronizer)]
async fn sync_2ff_tick_last(clk: Clock<MainClk>, d: In<Logic, MainClk>, q: RegOut<Logic, MainClk>) {
    let mut ff1 = Logic::Zero;
    let mut ff2 = Logic::Zero;
    loop {
        ff2 = ff1;
        ff1 = d.read();
        q.write(ff2);
        clk.tick().await;
    }
}

// The nested-loop fold defect of SPEC I.7.5, verbatim: `v` is assigned before a
// ticking `for` and read inside it after an interior tick — live at that tick, a
// register of I.7.1. Until Phase C step 6 the inference folded the nested loop
// into one node with no definitions, saw `v`'s use at the node's entry only, and
// missed it (the poison run flagged `v`, MEASUREMENTS §D.4). The register graph
// now descends into the loop and infers {i, v} (§K.2); the swept twin is
// `tests/fixtures/nested_loop_registers_dut.rs::nested_loop_missed`.
#[hardware(sequential)]
async fn fold_defect(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v = x.read();
        for i in 0..4 {
            q.write(v + Bits::from_usize(i));
            clk.tick().await;
        }
    }
}

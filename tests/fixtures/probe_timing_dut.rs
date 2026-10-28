// Timing-reconciliation investigation fixture (EXECUTION_MODEL_RECONCILIATION.md).
//
// Two codings of the SAME intent — "every 2 cycles, sample `inp`, then drive it
// onto `out` one cycle later":
//
//   * `probe`     — the multi-tick async form (2 ticks/loop). This is the coding
//     whose sim-vs-transpiled timing is under dispute.
//   * `probe_fsm` — the explicit SINGLE-tick FSM (an explicit `phase` register).
//     Single-tick is the category where sim and transpiled Verilog PROVABLY
//     agree, so `probe_fsm` is a reference-quality, human-written witness of what
//     the hardware actually does (the same role `mac_fsm` plays for `mac_pipeline`).
//
// If `probe_fsm` (sim == verilog) picks one sampling schedule, that schedule IS
// the hardware-accurate answer, and whichever of {multi-tick sim, multi-tick
// transpiler} matches it is the correct execution model.
//
// `probe_fsm` itself diverged from its own transpiled Verilog by one cycle until
// 2026-10-26: the `inp.read()` in the `phase == 0` arm marked codegen's whole
// statement walk, so the `phase == 1` arm's `out.write(x)` was edge-sampled one
// edge after the simulator's opening-instant write. With the walk arm-granular the
// drive is opening in its own arm, takes phase C's mux + hold form, and sim ==
// verilog (`probe_fsm_sim_matches_verilog`, un-ignored; the module sweeps). Since
// the D1 re-derivation (2026-10-28) the rule ADMITS it — the read in the sibling
// arm reaches the write on no path, so its anchor is opening on both sides — and it
// compiles without an opt-out; `w4_mixed_alignment` (the read-free arm assigning
// the register the shared write reads) is the continuous-assign class's divergent
// witness.

#[hardware(sequential)]
async fn probe(
    clk: Clock<MainClk>,
    inp: In<Bits<8>, MainClk>,
    out: Out<Bits<8>, MainClk>,
) {
    loop {
        let x = inp.read(); // x lives across .await → a register
        clk.tick().await; // edge A
        out.write(x);
        clk.tick().await; // edge B
    }
}

#[hardware(sequential)]
async fn probe_fsm(
    clk: Clock<MainClk>,
    inp: In<Bits<8>, MainClk>,
    out: Out<Bits<8>, MainClk>,
) {
    let mut phase: u8 = 0;
    let mut x: Bits<8> = Bits::from_lit::<0>();
    loop {
        if phase == 0 {
            x = inp.read();
            phase = 1;
        } else {
            out.write(x.clone());
            phase = 0;
        }
        clk.tick().await;
    }
}

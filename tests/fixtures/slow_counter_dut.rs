// Single source of truth for the slow_counter equivalence test.
// `include!`d for simulation and `include_str!`d for transpilation.
// A counter with TWO back-to-back `clk.tick().await`s per iteration: the register
// advances once every two clock cycles. Exercises consecutive suspension points
// within a single loop iteration (multiple FSM states between register updates) —
// the "back-to-back tick awaits" corner case.
#[hardware(sequential)]
async fn slow_counter(clk: Clock<MainClk>, out: RegOut<Bits<8>, MainClk>) {
    let mut count: Bits<8> = Bits::zero();
    loop {
        // One job per cycle (D-8): the even phase does nothing; the odd phase
        // bumps the count and registers it on `out`, which holds it through the
        // next even phase.
        clk.tick().await;
        count = count + Bits::from_lit::<1>();
        out.write(count);
        clk.tick().await;
    }
}

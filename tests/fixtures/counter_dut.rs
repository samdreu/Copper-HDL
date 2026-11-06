// Single source of truth for the M1 equivalence test.
// Included as code (`include!`) to run in the Copper simulator, and read as
// text (`include_str!`) to feed the transpiler — so the simulated DUT and the
// transpiled DUT are byte-identical by construction.
#[hardware(sequential)]
async fn counter(clk: Clock<MainClk>, step: In<Bits<8>, MainClk>, out: RegOut<Bits<8>, MainClk>) {
    let mut count: Bits<8> = Bits::zero();
    loop {
        // One job per cycle (D-8): fold this cycle's step into the count and
        // publish the result; both take effect at the edge that ends the
        // resumption, so `out` shows the running sum through x_{N+1} from
        // obs_{N+1} — the frozen golden trace's timing.
        count = count + step.read();
        out.write(count);
        clk.tick().await;
    }
}

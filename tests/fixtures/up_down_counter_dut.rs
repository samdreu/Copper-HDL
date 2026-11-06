// Single source of truth for the up_down_counter equivalence test.
// `include!`d for simulation and `include_str!`d for transpilation.
// A sequential counter whose register update branches on a direction input —
// exercises a per-tick `if/else` over a single accumulator register, both `+` and
// `-` (wrapping) on `Bits`, and a Moore output written before the tick.
#[hardware(sequential)]
async fn up_down_counter(clk: Clock<MainClk>, dir: In<Logic, MainClk>, out: RegOut<Bits<8>, MainClk>) {
    let mut count: Bits<8> = Bits::zero();
    loop {
        if dir.read() == Logic::One {
            count = count + Bits::from_lit::<1>();
        } else {
            count = count - Bits::from_lit::<1>();
        }
        out.write(count);
        clk.tick().await;
    }
}

#![allow(unused_imports)]
use copper_core::port::{In, RegOut};
use copper_core::{Bits, Clock, ClockDomain};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: `q` is a `RegOut` written after the last tick from `step`, read
// in that same trailing region. A `RegOut` write takes effect at the first clock
// edge after it executes, so this write commits at the NEXT edge, where the
// emitted `q <= d` (a wire of `step`) samples the input one drive later than the
// simulator's immediate read did. Through a register (`acc = acc + d; q.write(acc)`,
// the `regout_trailing_single_tick` fixture) the value is held and the two agree.
#[hardware(sequential)]
async fn trailing_regout_from_input(clk: Clock<MainClk>, step: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        clk.tick().await;
        let d: Bits<8> = step.read();
        q.write(d);
    }
}

fn main() {}

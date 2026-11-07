use copper_core::port::{In, RegOut};
use copper_core::{Bits, Clock, ClockDomain};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// `allow_pretick_alignment` was retired with the pre-tick alignment rules by D-8
// (SPEC II.3.2): a clocked module has no plain `Out`, so there is nothing to opt
// out of. The attribute takes a mode only.
#[hardware(sequential, allow_pretick_alignment)]
async fn m(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    loop {
        o.write(a.read());
        clk.tick().await;
    }
}

fn main() {}

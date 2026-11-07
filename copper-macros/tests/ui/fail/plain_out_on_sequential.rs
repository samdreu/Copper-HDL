use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// SPEC II.1 R8 (D-8): a `sequential` module has no plain `Out`. Every clocked
// output is a register — declare it `RegOut`, or compute a value that must follow
// an input within the cycle in a `#[hardware(combinational)]` module.
#[hardware(sequential)]
async fn passthrough(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    loop {
        o.write(a.read());
        clk.tick().await;
    }
}

fn main() {}

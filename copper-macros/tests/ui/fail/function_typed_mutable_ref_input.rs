use copper_core::{Clock, ClockDomain};
use copper_core::port::{In, Out};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: combinational cannot have a Clock parameter
#[hardware(combinational)]
fn gate(clk: Clock<MainClk>, a: In<u8, MainClk>, out: Out<u8, MainClk>) {
    out.write(a.read());
}

fn main() {}

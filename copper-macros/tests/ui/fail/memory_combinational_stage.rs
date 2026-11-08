// SPEC I.10.6 (D-16 A8, D-25): the staging half — an evaluation has no resumption whose
// effect a staging could be. Refused by both front-ends at the header.
use copper_core::{Bits, Clock, ClockDomain, Memory};
use copper_core::port::{In, Out, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

#[hardware(combinational)]
fn c_stage(x: In<Bits<4>, MainClk>, m: Memory<Bits<8>, 1, 0, MainClk, 1, 1>, o: Out<Bits<4>, MainClk>) {
    m.read_port::<0>().read(x.read().as_usize());
    o.write(x.read());
}

fn main() {}

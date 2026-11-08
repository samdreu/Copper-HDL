// SPEC I.10.6 (D-16 A8, D-25): a `combinational` module neither stages nor observes a
// memory — `Memory` is not a parameter kind of its header. Refused by both front-ends.
use copper_core::{Bits, Clock, ClockDomain, Memory};
use copper_core::port::{In, Out, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

#[hardware(combinational)]
fn c_obs(m: Memory<Bits<8>, 1, 0, MainClk, 1, 1>, o: Out<Bits<8>, MainClk>) {
    o.write(m.read_port::<0>().data());
}

fn main() {}

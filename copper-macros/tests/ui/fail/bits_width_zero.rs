use copper_core::{Bits, ClockDomain};
use copper_core::port::{In, Out};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC I.14.1(c), II.1.2 R16: `Bits<0>` is OUTSIDE (no value to observe). Both
// front-ends admitted it until 2026-11-08 (MEASUREMENTS §M.9 item 3, §V.2).
#[hardware(combinational)]
fn m(a: In<Bits<0>, A>, o: Out<Bits<0>, A>) {
    o.write(a.read());
}

fn main() {}

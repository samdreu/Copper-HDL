use copper_core::{ClockDomain, Logic};
use copper_core::port::{In, Out};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC I.11.3 (D-14), II.2 D2: every path writes every `Out` — a port no path writes is
// refused. Both front-ends admitted it until Phase C step 10 (II.2.2's under-direction).
#[hardware(combinational)]
fn c(a: In<Logic, A>, o: Out<Logic, A>, p: Out<Logic, A>) {
    o.write(a.read());
}

fn main() {}

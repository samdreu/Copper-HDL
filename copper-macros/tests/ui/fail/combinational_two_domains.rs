use copper_core::{ClockDomain, Logic};
use copper_core::port::{In, Out};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}
struct B;
impl ClockDomain for B {}

// SPEC I.12.4(c) (D-17 A4a; II.1 S0): a combinational module's ports lie in one domain.
// Both front-ends admitted `c_mixed` (transpiled to `assign o = a`) until Phase C step 10.
#[hardware(combinational)]
fn c_mixed(a: In<Logic, A>, o: Out<Logic, B>) {
    o.write(a.read());
}

fn main() {}

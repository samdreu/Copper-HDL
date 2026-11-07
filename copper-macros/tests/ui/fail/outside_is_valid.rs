use copper_core::{Bits, ClockDomain, Logic};
use copper_core::port::{In, Out};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail (SPEC I.14.3(d), D-19; Phase C step 8): `is_valid()` asks whether a value
// holds the simulator's third value X, which no 2-state realization holds — OUTSIDE,
// refused by both front-ends (`copper_analysis::check_outside_methods`). Phase C step 5
// had lowered it to `1`.
#[hardware(combinational)]
fn outside_is_valid(a: In<Bits<8>, MainClk>, q: Out<Logic, MainClk>) {
    q.write(if a.read().is_valid() { Logic::One } else { Logic::Zero });
}

fn main() {}

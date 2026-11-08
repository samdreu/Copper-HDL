use copper_core::{ClockDomain, Logic};
use copper_core::port::{In, Out};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC I.14.3(d) (D-23 (3)): `v == Logic::X` asks whether a value holds the simulator's
// third value — `is_valid()` spelled with an operator — and is OUTSIDE. Until 2026-11-08
// the macro admitted it (the derived `PartialEq`) and the transpiler refused it; both
// now refuse it in `check_outside_methods`, with one message.
#[hardware(combinational)]
fn m(a: In<Logic, A>, q: Out<Logic, A>) {
    q.write(if a.read() == Logic::X { Logic::One } else { Logic::Zero });
}

fn main() {}

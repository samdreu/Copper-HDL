use copper_core::Logic;
use copper_core::port::{In, Out};
use copper_macros::hardware;

// Should fail: a port names its domain — `In<T>` has no default domain since the unit
// domain was retired (SPEC I.12.6 (4), D-17 A1; D-22). Refused by the shared front-end
// rule before rustc would report the missing generic argument.
#[hardware(combinational)]
fn inv(a: In<Logic>, o: Out<Logic>) {
    o.write(!a.read());
}

fn main() {}

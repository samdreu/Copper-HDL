use copper_core::Logic;
use copper_core::port::{In, Out};
use copper_macros::hardware;

// Should fail: `()` is not a clock domain (SPEC I.12.6 (4), D-17 A1; D-22). A
// combinational module that serves any domain is generic over it and lies in the
// domain of its ports (I.12.4(c)) — `pass/combinational_mux_valid.rs`'s spelling.
#[hardware(combinational)]
fn inv(a: In<Logic, ()>, o: Out<Logic, ()>) {
    o.write(!a.read());
}

fn main() {}

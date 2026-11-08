use copper_core::{ClockDomain, Logic};
use copper_core::port::{In, Out};
use copper_macros::hardware;

// Should pass: valid combinational module
#[hardware(combinational)]
fn and_gate<D: ClockDomain>(a: In<Logic, D>, b: In<Logic, D>, out: Out<Logic, D>) {
    out.write(a.read() & b.read());
}

fn main() {}

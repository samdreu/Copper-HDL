use copper_core::{Bits, ClockDomain, Logic};
use copper_core::port::{In, Out};
use copper_macros::hardware;

// Should fail: `o` has a default and is written on every path, but `p` is
// written only under `c` — one port's default does not cover another (§10.12).
#[hardware(combinational)]
fn half_defaulted<D: ClockDomain>(c: In<Logic, D>, x: In<Bits<8>, D>, o: Out<Bits<8>, D>, p: Out<Bits<8>, D>) {
    o.write(Bits::from_lit::<3>());
    if c.read() == Logic::One {
        o.write(x.read());
        p.write(x.read());
    }
}

fn main() {}

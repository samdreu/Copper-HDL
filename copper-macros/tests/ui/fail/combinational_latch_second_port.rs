use copper_core::{Bits, Logic};
use copper_core::port::{In, Out};
use copper_macros::hardware;

// Should fail: `o` has a default and is written on every path, but `p` is
// written only under `c` — one port's default does not cover another (§10.11).
#[hardware(combinational)]
fn half_defaulted(c: In<Logic, ()>, x: In<Bits<8>, ()>, o: Out<Bits<8>, ()>, p: Out<Bits<8>, ()>) {
    o.write(Bits::from_lit::<3>());
    if c.read() == Logic::One {
        o.write(x.read());
        p.write(x.read());
    }
}

fn main() {}

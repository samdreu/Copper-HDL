use copper_core::{Bits, Logic};
use copper_core::port::{In, Out};
use copper_macros::hardware;

// Should fail: two conditional writes are not a default — when neither `c` nor
// `d` holds, `o` is not written (§10.11).
#[hardware(combinational)]
fn two_partials(c: In<Logic, ()>, d: In<Logic, ()>, x: In<Bits<8>, ()>, o: Out<Bits<8>, ()>) {
    if c.read() == Logic::One {
        o.write(Bits::from_lit::<3>());
    }
    if d.read() == Logic::One {
        o.write(x.read());
    }
}

fn main() {}

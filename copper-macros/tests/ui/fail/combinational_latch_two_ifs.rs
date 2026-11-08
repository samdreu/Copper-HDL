use copper_core::{Bits, ClockDomain, Logic};
use copper_core::port::{In, Out};
use copper_macros::hardware;

// Should fail: two conditional writes are not a default — when neither `c` nor
// `d` holds, `o` is not written (§10.12).
#[hardware(combinational)]
fn two_partials<D: ClockDomain>(c: In<Logic, D>, d: In<Logic, D>, x: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    if c.read() == Logic::One {
        o.write(Bits::from_lit::<3>());
    }
    if d.read() == Logic::One {
        o.write(x.read());
    }
}

fn main() {}

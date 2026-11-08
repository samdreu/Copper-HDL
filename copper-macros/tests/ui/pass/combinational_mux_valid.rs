use copper_core::{Bits, Logic, ClockDomain};
use copper_core::port::{In, Out};
use copper_macros::hardware;

// Should pass: `o` is driven on *every* path (both arms of the `if/else`), so no
// latch is inferred — the combinational counterpart to the rejected `leaky_mux`.
#[hardware(combinational)]
fn mux2<D: ClockDomain>(sel: In<Logic, D>, a: In<Bits<8>, D>, b: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    if sel.read() == Logic::One {
        o.write(a.read());
    } else {
        o.write(b.read());
    }
}

fn main() {}

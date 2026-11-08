use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC II.1 R14 (S0; I.2.3, I.4.8): an input read inside macro tokens is no
// `let t = x.read()`; refused with the macro that holds it.
#[hardware(sequential)]
async fn m(clk: Clock<A>, a: In<Bits<8>, A>, q: RegOut<Bits<8>, A>) {
    loop {
        println!("{:?}", a.read());
        q.write(Bits::zero());
        clk.tick().await;
    }
}

fn main() {}

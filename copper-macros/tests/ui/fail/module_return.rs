use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC II.1 R7 (S4): `return` ends the coroutine. Both front-ends refuse it.
#[hardware(sequential)]
async fn m(clk: Clock<A>, a: In<Logic, A>, q: RegOut<Logic, A>) {
    loop {
        let t = a.read();
        q.write(t);
        clk.tick().await;
        if t == Logic::from_bool(true) {
            return;
        }
    }
}

fn main() {}

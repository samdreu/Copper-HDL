use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC II.1 R7 (S4): a `break` of the module loop ends the coroutine. Both front-ends
// refuse it with one message (Phase C step 10; before it the macro admitted it and the
// simulator panicked at run time).
#[hardware(sequential)]
async fn m(clk: Clock<A>, a: In<Logic, A>, q: RegOut<Logic, A>) {
    loop {
        let t = a.read();
        q.write(t);
        if t == Logic::from_bool(true) {
            break;
        }
        clk.tick().await;
    }
}

fn main() {}

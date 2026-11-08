use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC II.1 R12 (S0; I.2.3, I.8.3): a `break` carries no value. The transpiler refused it by
// an accident (a tick inside a `let` initializer), the macro admitted it.
#[hardware(sequential)]
async fn m(clk: Clock<A>, a: In<Logic, A>, q: RegOut<Logic, A>) {
    loop {
        let v = loop {
            let t = a.read();
            if t == Logic::from_bool(true) {
                break t;
            }
            clk.tick().await;
        };
        q.write(v);
        clk.tick().await;
    }
}

fn main() {}

use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC II.1 R12 (S0; I.2.3, I.8.3): `break` is unlabelled. The transpiler refused it, the
// macro admitted it, until Phase C step 10.
#[hardware(sequential)]
async fn m(clk: Clock<A>, a: In<Logic, A>, q: RegOut<Logic, A>) {
    loop {
        'w: loop {
            let t = a.read();
            if t == Logic::from_bool(true) {
                break 'w;
            }
            clk.tick().await;
        }
        q.write(Logic::from_bool(true));
        clk.tick().await;
    }
}

fn main() {}

use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC II.1 R10 (S0/S2; I.12.5): a clocked module has exactly one `Clock`. Both admitted it
// until Phase C step 10.
struct B;
impl ClockDomain for B {}

#[hardware(sequential)]
async fn m(clk: Clock<A>, clk2: Clock<B>, q: RegOut<Logic, A>) {
    loop {
        q.write(Logic::from_bool(true));
        clk.tick().await;
        clk2.tick().await;
    }
}

fn main() {}

use copper_core::{Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct Fast;
impl ClockDomain for Fast {}
struct Slow;
impl ClockDomain for Slow {}

// SPEC I.12.3, the two-flop rule (D-17 A3; II.1 realizability): a `RegOut` written from
// a foreign read of the same resumption is one flop sampling an asynchronous wire. Both
// front-ends admitted this (`sync_1ff_oj`, emitted `q <= d`) until Phase C step 10.
#[hardware(synchronizer)]
async fn sync_one_flop(clk: Clock<Slow>, d: In<Logic, Fast>, q: RegOut<Logic, Slow>) {
    loop {
        let ff1 = d.read();
        q.write(ff1);
        clk.tick().await;
    }
}

fn main() {}

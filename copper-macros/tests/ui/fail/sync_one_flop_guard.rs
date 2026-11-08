use copper_core::{Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct Fast;
impl ClockDomain for Fast {}
struct Slow;
impl ClockDomain for Slow {}

// SPEC I.12.3, the two-flop rule: the guard clause (`sync_guard`, D-17 (iv)). The write
// stands under a condition on a foreign read of the same resumption.
#[hardware(synchronizer)]
async fn sync_guard(clk: Clock<Slow>, d: In<Logic, Fast>, q: RegOut<Logic, Slow>) {
    let mut ff1 = Logic::Zero;
    loop {
        if d.read() == Logic::One {
            ff1 = Logic::One;
        }
        q.write(ff1);
        clk.tick().await;
    }
}

fn main() {}

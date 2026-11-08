use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC II.2.2, R3's over-refusal (D-12 §0 (iii), packet 10's shape): the enclosing loop's
// only boundary is a nested `loop` with no `break` whose first statement is an `if` with a
// tick in it, then the bare tick. Every path ticks; the walk used to answer "exitable" at
// the `if` and both front-ends refused it. Admitted since Phase C step 10 (all-paths walk).
#[hardware(sequential)]
async fn livelock_a(clk: Clock<A>, c: In<Logic, A>, q: RegOut<Bits<8>, A>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        loop {
            let t = c.read();
            if t == Logic::One {
                n = n + Bits::from_lit::<1>();
                q.write(n);
                clk.tick().await;
            }
            q.write(n);
            clk.tick().await;
        }
    }
}

fn main() {}

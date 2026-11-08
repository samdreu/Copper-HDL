use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC II.2.2, R3's over-refusal (D-12 §0 (iii)): an `if`/`else` whose both arms tick,
// then `if done { break }`, then the bare tick; a write after the nested loop. The `break`
// is reached only after a tick on every path. Admitted since Phase C step 10.
#[hardware(sequential)]
async fn livelock_b(clk: Clock<A>, c: In<Logic, A>, d: In<Logic, A>, q: RegOut<Bits<8>, A>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        loop {
            let t = c.read();
            if t == Logic::One {
                n = n + Bits::from_lit::<1>();
                clk.tick().await;
            } else {
                clk.tick().await;
            }
            let done = d.read();
            if done == Logic::One {
                break;
            }
            clk.tick().await;
        }
        q.write(n);
    }
}

fn main() {}

use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC I.8.4(b), R3 (MEASUREMENTS §X.2): a nested `loop` whose only boundary is a ticking
// `for` over a run-time range — the same shape one level down, refused as a `while` in its
// place is. Until 2026-11-08 the macro admitted it and the transpiler refused it with the
// arms-only-loop message.
#[hardware(sequential)]
async fn m(clk: Clock<A>, c: In<Bits<2>, A>, g: In<Logic, A>, q: RegOut<Bits<8>, A>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        let cnt: u32 = c.read().as_u32();
        loop {
            if g.read() == Logic::One {
                break;
            }
            for _ in 0..cnt {
                q.write(n);
                clk.tick().await;
            }
        }
        clk.tick().await;
    }
}

fn main() {}

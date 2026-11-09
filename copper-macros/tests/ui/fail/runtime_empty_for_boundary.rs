use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC I.8.4(b) (D-10 A4-i), R3, line `II.3 found runtime-empty-boundary` (MEASUREMENTS §X.2):
// the module loop's only boundary is a ticking `for` over a range that is empty when
// `cnt == 0`, a value of the run. It then runs zero iterations in zero time (I.8.4(a)) and
// the module loop returns to its top without a tick. Until 2026-11-08 the macro admitted it
// and the transpiler refused it with its flattener's message.
#[hardware(sequential)]
async fn m(clk: Clock<A>, c: In<Bits<2>, A>, g: In<Logic, A>, q: RegOut<Bits<8>, A>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        let cnt: u32 = c.read().as_u32();
        for _ in 0..cnt {
            n = n + Bits::from_lit::<1>();
            q.write(n);
            clk.tick().await;
        }
    }
}

fn main() {}

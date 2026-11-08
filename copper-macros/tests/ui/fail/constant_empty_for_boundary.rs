use copper_core::{Bits, Clock, ClockDomain};
use copper_core::port::RegOut;
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

const EMPTY: usize = 0;

// SPEC I.8.4(b), II.2.2 R3: the module loop's only boundary is a `for` over a range
// empty only through a constant — it runs no iteration, a zero-time cycle. Both
// front-ends admitted it until 2026-11-08 (MEASUREMENTS §P.5's residue, §V.3); the
// literal spelling is `literal_empty_for_boundary`.
#[hardware(sequential)]
async fn m(clk: Clock<A>, q: RegOut<Bits<8>, A>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        n = n + Bits::from_lit::<1>();
        q.write(n);
        for _i in 0..EMPTY {
            clk.tick().await;
        }
    }
}

fn main() {}

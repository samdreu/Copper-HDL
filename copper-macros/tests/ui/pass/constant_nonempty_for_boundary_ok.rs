use copper_core::{Bits, Clock, ClockDomain};
use copper_core::port::RegOut;
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

const BEATS: usize = 3;
const START: usize = 1;

// The twin of ui/fail/constant_empty_for_boundary (SPEC I.8.4(b), §V.3): the same
// shape over a range that is not empty, through two file `const`s, is a boundary,
// admitted by both front-ends.
#[hardware(sequential)]
async fn m(clk: Clock<A>, q: RegOut<Bits<8>, A>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        n = n + Bits::from_lit::<1>();
        q.write(n);
        for _i in START..BEATS + START {
            clk.tick().await;
        }
    }
}

fn main() {}

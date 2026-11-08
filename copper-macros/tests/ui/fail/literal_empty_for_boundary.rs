use copper_core::{Bits, Clock, ClockDomain};
use copper_core::port::RegOut;
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC II.2.2, R3's under-admission (I.8.4(b)): the module loop's only boundary is a
// `for` over a literal-empty range, which runs no iteration — a zero-time cycle the
// simulator livelocks on. Both front-ends admitted it until Phase C step 10.
#[hardware(sequential)]
async fn m(clk: Clock<A>, q: RegOut<Bits<8>, A>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        n = n + Bits::from_lit::<1>();
        q.write(n);
        for _i in 0..0 {
            clk.tick().await;
        }
    }
}

fn main() {}

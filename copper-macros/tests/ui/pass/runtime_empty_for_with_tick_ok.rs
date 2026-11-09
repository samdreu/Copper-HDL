use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// The twin of ui/fail/runtime_empty_for_boundary (SPEC I.8.4(b), MEASUREMENTS §X.2): the
// same run-time `for` followed by a tick of the module loop's own. A zero-count pass is a
// zero-time exit the resumption continues through to that tick; admitted by both.
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
        let _ = g.read();
        clk.tick().await;
    }
}

fn main() {}

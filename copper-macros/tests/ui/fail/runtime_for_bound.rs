use copper_core::{Bits, Clock, ClockDomain};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC II.1 R5 (S3; I.2.5(c), I.8.5): a tick-free `for` is unrolled, so its bound is a
// Rust constant. Both front-ends admitted a run-time bound until Phase C step 10 (the
// transpiler emitted an `always_comb` `for` with a run-time limit).
#[hardware(sequential)]
async fn m(clk: Clock<A>, n: In<Bits<4>, A>, q: RegOut<Bits<8>, A>) {
    loop {
        let k = n.read().as_usize();
        let mut s: Bits<8> = Bits::zero();
        for _i in 0..k {
            s = s + Bits::from_lit::<1>();
        }
        q.write(s);
        clk.tick().await;
    }
}

fn main() {}

use copper_core::{Bits, Clock, ClockDomain};
use copper_core::port::RegOut;
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC I.14.1(c), II.1.2 R16: `Bits<N>` with N > 128 is OUTSIDE (its arithmetic does
// not monomorphize). Both front-ends admitted it until 2026-11-08 (§V.2) — here in an
// annotated local, the rule's other position.
#[hardware(sequential)]
async fn m(clk: Clock<A>, q: RegOut<Bits<8>, A>) {
    loop {
        let _wide: Bits<200> = Bits::zero();
        q.write(Bits::zero());
        clk.tick().await;
    }
}

fn main() {}

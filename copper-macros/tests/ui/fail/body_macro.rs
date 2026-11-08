use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC II.1 R13 (S0; I.2.3): no macro in a body outside `const { }` (`matches!` and a
// `from_contents(vec![…])` preload excepted). The transpiler refused it, the macro
// admitted it, until Phase C step 10.
#[hardware(sequential)]
async fn m(clk: Clock<A>, a: In<Bits<8>, A>, q: RegOut<Bits<8>, A>) {
    loop {
        let t = a.read();
        assert!(t.as_u64() < 200);
        q.write(t);
        clk.tick().await;
    }
}

fn main() {}

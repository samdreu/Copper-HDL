use copper_core::{Bits, Clock, ClockDomain};
use copper_core::port::RegOut;
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

const TOP: u8 = 255;

// SPEC II.1.2 R23 (D-27, 2026-11-09): a tick-bearing `for` over an inclusive range whose
// constant end is its type's maximum. I.8.5's counter is incremented past the end before the
// exit test `c > e` reads it, so it overflows. Refused by both front-ends (the macro through
// rustc, the transpiler through `ConstEnv`); a tick-free one is unrolled and admitted.
#[hardware(sequential)]
async fn m(clk: Clock<A>, q: RegOut<Bits<8>, A>) {
    loop {
        for i in 250..=TOP {
            q.write(Bits::from_u8(i));
            clk.tick().await;
        }
    }
}

fn main() {}

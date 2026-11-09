use copper_core::{Bits, Clock, ClockDomain};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

const N: u8 = 3;

// D-29 (2026-11-09, MEASUREMENTS §AE): the twin of ui/fail/runtime_for_ends_in_loop. A counted
// `for` whose body ends in a skippable wait, an inner `for` over a range non-empty by its constants
// ticking first: R3 holds (the macro has rustc check `0..N` non-empty), and the transpiler lowers it
// (I.8.5's `D` with no hoisted tick). Swept: tests/fixtures/ticking_for_ends_in_loop_dut.rs.
#[hardware(sequential)]
async fn m(clk: Clock<A>, a: In<Bits<4>, A>, q: RegOut<Bits<8>, A>) {
    loop {
        for i in 0..N {
            for j in 0u8..2 {
                q.write(Bits::from_u8(j));
                clk.tick().await;
            }
            q.write(Bits::from_u8(i));
            loop {
                if a.read().as_u8() & 1 == 1 {
                    break;
                }
                clk.tick().await;
            }
        }
    }
}

fn main() {}

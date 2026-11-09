use copper_core::{Bits, Clock, ClockDomain};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// D-29 (2026-11-09, MEASUREMENTS §AE, line `II.3 found ticking-for-ends-in-loop`): a counted
// `for` whose body ends in a skippable wait is admitted by both front-ends when an earlier inner
// `for` ticks on every path — its range non-empty by its constants (`ConstEnv` in the transpiler,
// rustc in the macro). An inner range known only at run time may be empty: an iteration can then
// skip both the inner `for` and the wait and take no time, so the shape stays refused, by both
// front-ends with R3's one message. The admitted twin is the sweep fixture
// `tests/fixtures/ticking_for_ends_in_loop_dut.rs::tfl_outer_ends_wait`.
#[hardware(sequential)]
async fn m(clk: Clock<A>, a: In<Bits<4>, A>, q: RegOut<Bits<8>, A>) {
    loop {
        for i in 0u8..2 {
            let n: u8 = a.read().as_u8() & 3;
            for _ in 0..n {
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

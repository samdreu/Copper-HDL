use copper_core::{Bits, Clock, ClockDomain};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC II.3.3 (c), I.2.4 (D-26 (1), 2026-11-08): an element write to an array register inside
// a tick-free `for` is lowered by unrolling the `for`, one write port per iteration, which needs
// the trip count. A bound that names a const generic has none in the parametric module the
// transpiler emits, so both front-ends refuse it with one message (MEASUREMENTS §U.11; the
// constant-bound twin is `tests/fixtures/array_for_write_dut.rs`).
#[hardware(sequential)]
async fn m<const N: usize>(clk: Clock<A>, a: In<Bits<8>, A>, q: RegOut<Bits<8>, A>) {
    let mut arr = [Bits::<8>::zero(); 4];
    loop {
        q.write(arr[0]);
        for j in 0..N {
            arr[j] = a.read() + arr[j];
        }
        clk.tick().await;
    }
}

fn main() {}

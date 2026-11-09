use copper_core::{Bits, Clock, ClockDomain};
use copper_core::port::RegOut;
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

const TOP: u8 = 255;

// A tick-bearing `for` over an inclusive range whose constant end is its type's maximum. Refused by
// both front-ends as II.1.2 R23 (D-27) until D-28 (2026-11-09) gave the inclusive `for` Rust's meaning
// everywhere: the last iteration runs and the loop stops, nothing overflows (I.8.5's done flag; the
// transpiler's counter is wider than the range's type). R23 is retired; admitted by both
// (`front_end_parity_ui.rs`), and swept as `inclusive_max_dut.rs::imx_u8_const`.
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

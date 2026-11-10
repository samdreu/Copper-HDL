use copper_core::{Clock, ClockDomain};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail (SPEC I.14.3(c), D-32 (2)): `to_be` means the byte order of the machine the
// program runs on, and SystemVerilog has none — refused by both front-ends with one message
// (`copper_analysis::check_target_byte_order`).
#[hardware(sequential)]
async fn target_byte_order_to_be(clk: Clock<MainClk>, a: In<u32, MainClk>, q: RegOut<u32, MainClk>) {
    loop {
        q.write(a.read().to_be());
        clk.tick().await;
    }
}

fn main() {}

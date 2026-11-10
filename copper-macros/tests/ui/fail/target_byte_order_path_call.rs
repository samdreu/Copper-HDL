use copper_core::{Clock, ClockDomain};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail (SPEC I.14.3(c), D-32 (2)): the path-call form `u32::to_be(v)` is the method
// call (MEASUREMENTS §AI.2), and refused as `v.to_be()` is, with the same message.
#[hardware(sequential)]
async fn target_byte_order_path_call(clk: Clock<MainClk>, a: In<u32, MainClk>, q: RegOut<u32, MainClk>) {
    loop {
        let v: u32 = a.read();
        q.write(u32::to_be(v));
        clk.tick().await;
    }
}

fn main() {}

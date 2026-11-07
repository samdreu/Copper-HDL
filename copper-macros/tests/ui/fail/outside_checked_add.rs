use copper_core::{Clock, ClockDomain};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail (SPEC I.14.3(c), D-19; Phase C step 8): `checked_*` returns an
// `Option`, which is not a hardware value — OUTSIDE, refused by both front-ends with
// one message (`copper_analysis::check_outside_methods`).
#[hardware(sequential)]
async fn outside_checked_add(clk: Clock<MainClk>, a: In<u8, MainClk>, q: RegOut<u8, MainClk>) {
    loop {
        q.write(a.read().checked_add(1).unwrap_or(0));
        clk.tick().await;
    }
}

fn main() {}

use copper_core::{Clock, ClockDomain, Bits};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should pass: valid sequential module (the one-job spelling, D-8: a `RegOut`
// output written before the tick).
#[hardware(sequential)]
async fn counter(clk: Clock<MainClk>, input: In<Bits<8>, MainClk>, out: RegOut<Bits<8>, MainClk>) {
    let mut count = Bits::<8>::from_lit::<0>();
    loop {
        count = count + input.read();
        out.write(count);
        clk.tick().await;
    }
}

fn main() {}

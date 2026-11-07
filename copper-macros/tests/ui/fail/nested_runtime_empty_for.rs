use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail (Phase C step 5, 2026-11-07, MEASUREMENTS §J.15): a counted `for` whose
// range may be empty at run time (`0..cnt`, an input) is the last statement of an
// enclosing tick-bearing `for`. With `cnt == 0` each enclosing iteration takes zero
// time, which the emitted state machine cannot run without unrolling the enclosing
// loop — a dated II.3 limitation, refused by both front-ends with one message
// (`copper_analysis::check_counted_for_nesting`).
#[hardware(sequential)]
async fn nested_for(clk: Clock<MainClk>, c: In<Bits<2>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        let cnt: u32 = c.read().as_u32();
        for _ in 0..2 {
            n = n + Bits::from_u8(1);
            for _ in 0..cnt {
                q.write(n);
                clk.tick().await;
            }
        }
        q.write(Bits::from_u8(255));
        clk.tick().await;
    }
}

fn main() {}

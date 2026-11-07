use copper_core::{Bits, Clock, ClockDomain};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail (SPEC II.1.2 R21, Phase C step 5): a `tick` before the module's
// `loop`. The statements before the loop are the first resumption's and run in no
// time (I.2.2's `s_pre` is tick-free, I.9.3). The macro admitted this until
// 2026-11-07 while the transpiler's linear route dropped the tick (II.3 (f6)); both
// front-ends now refuse it with one message (`copper_analysis::check_no_preloop_tick`).
#[hardware(sequential)]
async fn preloop_tick(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    clk.tick().await;
    loop {
        q.write(a.read() + Bits::from_lit::<1>());
        clk.tick().await;
    }
}

fn main() {}

#![allow(unused_imports)]
use copper_core::port::{In, RegOut};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: the nested wait ticks BEFORE testing. The test then lands in the
// window where a simulator samples the value the just-past edge produced and a
// flip-flop samples the value present before its own edge — measured a full cycle
// apart, and not reconciled by holding the stimulus. The ordering is outside the
// language by decision (2026-08-24); until 2026-10-23 only the transpiler refused
// it, so this module simulated and then failed to transpile.
#[hardware(sequential)]
async fn tick_first_waiter(clk: Clock<MainClk>, go: In<Logic, MainClk>, count: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        count.write(n);
        loop {
            clk.tick().await;
            if go.read() == Logic::One {
                break;
            }
        }
        n = n + Bits::from_lit::<1>();
        clk.tick().await;
    }
}

fn main() {}

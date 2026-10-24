#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

async fn something_else() {}

// Should fail: the only `await` a hardware body may write is `clk.tick().await`.
// Anything else has no hardware meaning; the transpiler always refused it and,
// since 2026-10-23, so does the macro.
#[hardware(sequential)]
async fn foreign(clk: Clock<MainClk>, a: In<Logic, MainClk>, o: Out<Logic, MainClk>) {
    loop {
        something_else().await;
        o.write(a.read());
        clk.tick().await;
    }
}

fn main() {}

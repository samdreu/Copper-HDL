use copper_core::{Clock, ClockDomain, Logic};
use copper_core::port::{wire, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC I.13.5 (D-18 A2): a sequential, synchronizer or combinational body contains no
// net. D-18 probe `b1b` / MEASUREMENTS §S.4 `wirebody`: the macro admitted the call (a
// plain fn) and the transpiler refused it with "cannot infer bit width"; both refuse
// it with one message since 2026-11-08 (§V.1, line `II.3 found body-wire`).
#[hardware(sequential)]
async fn m(clk: Clock<A>, q: RegOut<Logic, A>) {
    let (_d, _r) = wire::<Logic, A>(Logic::Zero);
    loop {
        q.write(Logic::One);
        clk.tick().await;
    }
}

fn main() {}

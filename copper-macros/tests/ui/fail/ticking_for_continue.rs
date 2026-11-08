use copper_core::{Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC I.8.5, II.3.3 "continue-for" (D-23 (2)): a `continue` of a `for` whose body
// holds a tick means increment-then-test (D-10 A2-i), which the transpiler's
// counted-`for` lowering does not realize. Until 2026-11-08 the macro admitted it and
// the transpiler refused it (MEASUREMENTS §R.3); both now refuse it with one message,
// a dated lowering limitation whose fix direction is the lowering.
#[hardware(sequential)]
async fn m(clk: Clock<A>, a: In<Logic, A>, o: RegOut<Logic, A>) {
    loop {
        for _i in 0..4 {
            if a.read() == Logic::One {
                clk.tick().await;
                continue;
            }
            o.write(Logic::Zero);
            clk.tick().await;
        }
    }
}

fn main() {}

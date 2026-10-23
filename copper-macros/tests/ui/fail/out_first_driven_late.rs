#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: `dv` is first driven LATE — every write of it is a constant, and the
// only one sits in the trailing segment of a two-tick body, so it first executes
// after the second clock edge. The simulator shows `dv`'s initial value at the
// first observation (the segment has not run yet) while the emitted
// `assign dv = 1'b1` drives from time 0: a one-cycle startup transient that no
// continuous assign can reproduce. Measured (the corpus differential sweep, as
// `tests/fixtures/trailing_statements_dut.rs::trailing_constant` before it moved
// here): cycle 0 `dv` expected 0, got 1; agreement from cycle 1 on. The same
// constant one tick earlier (`late_constant_one_edge`, still in that fixture)
// agrees, which is what fixes the boundary at the second edge.
#[hardware(sequential)]
async fn trailing_constant(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    dv: Out<Logic, MainClk>,
    o: Out<Bits<8>, MainClk>,
) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = a.read();
        clk.tick().await;
        o.write(r);
        clk.tick().await;
        dv.write(Logic::One);
    }
}

fn main() {}

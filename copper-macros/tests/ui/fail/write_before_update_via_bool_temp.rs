#![allow(unused_imports)]
use copper_core::port::{In, Out, RegOut};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: `write_before_update_via_temp.rs` with the temporary typed
// `bool` instead of `Logic` — the comparison is bound first and the port is
// written from a conditional on it. The dataflow is identical (the write's
// value depends on `full`, which is updated later in the segment behind the
// leading `drain.read()`), so the same V8a divergence follows; a `bool` temp
// evaded the rule exactly as a `Logic` one did.
#[hardware(sequential)]
async fn mealy_ready_via_bool_temp(
    clk: Clock<MainClk>,
    valid: In<Logic, MainClk>,
    data: In<Bits<8>, MainClk>,
    drain: In<Logic, MainClk>,
    ready: Out<Logic, MainClk>,
    sum: RegOut<Bits<8>, MainClk>,
) {
    let mut full: Logic = Logic::Zero;
    let mut held: Bits<8> = Bits::zero();
    let mut acc: Bits<8> = Bits::zero();
    loop {
        let can_take: bool = full == Logic::Zero || drain.read() == Logic::One;
        ready.write(if can_take { Logic::One } else { Logic::Zero });
        if full == Logic::One && drain.read() == Logic::One {
            acc = acc + held;
            full = Logic::Zero;
        }
        if can_take && valid.read() == Logic::One {
            held = data.read();
            full = Logic::One;
        }
        sum.write(acc);
        clk.tick().await;
    }
}

fn main() {}

#![allow(unused_imports)]
use copper_core::port::{In, Out, RegOut};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: `write_before_update.rs` with the value routed through a `let`
// temporary. `ready` is a combinational (Mealy) `ready = !full || drain`
// computed into `can_take`, written, and only then is `full` — the register
// the temporary reads — updated in the same single-tick segment. The leading
// `drain.read()` drags the write to the pre-edge settle, where `can_take`
// holds `full`'s PRE-update value: one generation behind the emitted
// `assign ready = can_take` (a wire of `full`'s Q). Measured in the corpus sweep
// (`tests/fixtures/valid_ready_dut.rs::vr_sink_mealy`): the hardware leads the
// simulator by one cycle, silently. Until 2026-10-20 the rule saw only the
// temporary's name and accepted this spelling while refusing the inline one.
#[hardware(sequential)]
async fn mealy_ready_via_temp(
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
        let can_take: Logic = if full == Logic::Zero || drain.read() == Logic::One {
            Logic::One
        } else {
            Logic::Zero
        };
        ready.write(can_take);
        if full == Logic::One && drain.read() == Logic::One {
            acc = acc + held;
            full = Logic::Zero;
        }
        if can_take == Logic::One && valid.read() == Logic::One {
            held = data.read();
            full = Logic::One;
        }
        sum.write(acc);
        clk.tick().await;
    }
}

fn main() {}

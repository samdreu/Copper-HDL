#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: in a register-free body the direct `o.write(x.read())` is an
// immediate passthrough — the simulator samples `x` once, at the opening of the
// segment's cycle — while the linear multi-tick path emits `assign o = x`, which
// tracks the input at every observation. One observation per iteration (the
// single-tick body, `e8l`) agrees; two or more cannot. Measured as
// `e15g_middle_input_direct` in tests/sequential_forwarding_divergence.rs (sim
// `[10, 10, 12, 12, …]`, SV `[10, 11, 12, …]`), `CYCLE_DATAFLOW_SEMANTICS.md`
// §10.8 (the registered twin `e14c` is §10.7's). The remedy is a wire behind the
// read (`linear_regfree_wire_behind_read` in ui/pass/pretick_alignment_ok.rs) or
// `RegOut`.
#[hardware(sequential)]
async fn linear_regfree_passthrough(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    loop {
        clk.tick().await;
        o.write(x.read());
        clk.tick().await;
    }
}

fn main() {}

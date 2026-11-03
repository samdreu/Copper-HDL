#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: on the linear multi-tick path a conditional write of `o` after
// the last `clk.tick().await` is hoisted into phase 0's edge-sampled flop, which
// loads at the even edge AFTER the trailing instant the simulator runs the
// write at — one observation late, with no input read for any other rule to
// see. Measured as `e16d_trailing_held_register_guard_no_read` in
// tests/sequential_forwarding_divergence.rs (sim `[0, 1, 1, 1, 1, 3, …]`, SV
// `[0, 0, 1, 1, 1, 1, 3, …]`), `CYCLE_DATAFLOW_SEMANTICS.md` §10.9. The remedy
// is the same write before the last tick (`linear_held_no_read` in
// ui/pass/pretick_alignment_ok.rs) or `RegOut`.
#[hardware(sequential)]
async fn linear_trailing_conditional(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        clk.tick().await;
        clk.tick().await;
        if cnt[0] == Logic::One { o.write(cnt); }
    }
}

fn main() {}

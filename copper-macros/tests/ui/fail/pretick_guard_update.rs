#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: in a loop with one `clk.tick().await`, a plain `Out` written on
// every path behind a leading input read is chosen by a guard on `cnt`, which the
// segment updates AFTER the guard. The read parks the write at the pre-edge
// settle, where the guard tests `cnt`'s pre-update value; the synthesized
// `always_comb` mux tests the flip-flop's Q, which from the observation on is the
// post-update value — one generation ahead. Measured as
// `e18b_head_every_path_register_guard_behind_read` in
// tests/sequential_forwarding_divergence.rs (sim `[3, 7, 3, …]`, SV
// `[7, 3, 7, …]`), `CYCLE_DATAFLOW_SEMANTICS.md` §10.11. The remedy is the update
// before the guard, the write before the read, or `RegOut`.
#[hardware(sequential)]
async fn pretick_guard_update(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        let _ = x.read();
        if cnt[0] == Logic::One { o.write(Bits::from_lit::<7>()); } else { o.write(Bits::from_lit::<3>()); }
        cnt = cnt + Bits::from_lit::<1>();
        clk.tick().await;
    }
}

fn main() {}

#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: in a loop with one `clk.tick().await`, a plain `Out` written on
// every path behind a leading input read reads `cnt`, which the statements after
// the tick update. The read parks the write at the pre-edge settle, where it
// captures `cnt`'s pre-update value; the update commits at that same edge, so
// the synthesized `assign o = cnt` shows the next generation from the
// observation on — one ahead. Measured as `e17h_head_register_default_behind_read`
// in tests/sequential_forwarding_divergence.rs (sim `[0, 1, 2, …]`, SV
// `[1, 2, 3, …]`), `CYCLE_DATAFLOW_SEMANTICS.md` §10.10. The remedy is the update
// before the tick, the write before the read, or `RegOut`.
#[hardware(sequential)]
async fn single_tick_trailing_update(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        let _ = x.read();
        o.write(cnt);
        clk.tick().await;
        cnt = cnt + Bits::from_lit::<1>();
    }
}

fn main() {}

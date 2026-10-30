#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: a PATH-DEPENDENT drive of a held port. The input read sits in one
// arm of the first branch; the write after the merge is reached by that read on
// one path (closing in the simulator) and not on the other (opening) — and it
// only ever executes on the read-free path. Codegen anchors a drive once, closing
// because a read reaches it on SOME path, and edge-samples it, so the SV shows the
// constant one observation after the simulator. Measured as
// `d1f_const_path_dependent_hold` in tests/sequential_forwarding_divergence.rs
// (sim `[7, 7, …]`, SV `[0, 7, …]`). Clause (A1) of the rule as narrowed
// 2026-10-30 — the mechanism the drive-level mux + hold form cannot reach, since
// no per-drive form gives one drive two anchors.
#[hardware(sequential)]
async fn path_dependent_drive(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut phase: u8 = 0;
    loop {
        if phase == 0 {
            let _ = x.read();
            phase = 1;
        } else {
            phase = 0;
        }
        if phase == 0 {
            o.write(Bits::from_lit::<7>());
        }
        clk.tick().await;
    }
}

fn main() {}

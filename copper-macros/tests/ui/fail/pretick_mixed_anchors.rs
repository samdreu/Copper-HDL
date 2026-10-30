#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: MIXED anchors on a port written on EVERY path. One arm writes
// behind its input read (closing on both sides), the other with no read before it
// (opening on both sides) — each drive's anchor is uniform, but an every-path port
// is emitted as a comb mux on the committed `phase` with no hold register, so the
// SV shows the closing `1` whenever `phase == 0` is committed, at the opening
// observation, where the simulator has not run that write yet (and never shows
// it: the next iteration's opening write overwrites it first). Measured as
// `d2b_mixed_two_drives_all_paths` in tests/sequential_forwarding_divergence.rs
// (sim `[2, 2, 2, …]`, SV `[2, 1, 2, 1, …]`). Clause (A2) of the rule as narrowed
// 2026-10-30. The same two drives on a HELD port (`d2a`, the former content of
// this file) are realized by the drive-level mux + hold form and accepted —
// `mixed_anchors_held` in ui/pass/pretick_alignment_ok.rs.
#[hardware(sequential)]
async fn mixed_anchors_every_path(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut phase: u8 = 0;
    loop {
        if phase == 0 {
            let _ = x.read();
            o.write(Bits::from_lit::<1>());
            phase = 1;
        } else {
            o.write(Bits::from_lit::<2>());
            phase = 0;
        }
        clk.tick().await;
    }
}

fn main() {}

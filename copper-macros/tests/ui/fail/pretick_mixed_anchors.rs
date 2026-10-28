#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: MIXED anchors through two drives of a held port. The first write
// has no read before it (the simulator runs it at the cycle's opening, on
// committed state); the second sits behind the input read (closing). Codegen
// edge-samples the whole port, so the opening write lands one observation late —
// and the simulator never shows the closing write at all, because the next
// iteration's opening write overwrites it before the observation. Measured as
// `d2a_mixed_two_drives_hold` in tests/sequential_forwarding_divergence.rs
// (sim `[0, 5, 5, 5, …]`, SV `[0, 9, 5, 9, 5, …]`). Clause (A) of the re-derived
// rule (2026-10-28); note that every register assignment here sits behind the
// read, which the pre-2026-10-28 rule took as protection.
#[hardware(sequential)]
async fn mixed_anchors(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut pc: u8 = 0;
    loop {
        if pc == 1 {
            o.write(Bits::from_lit::<5>());
        }
        if x.read()[0] == Logic::One {
            o.write(Bits::from_lit::<9>());
            pc = 1;
        } else {
            pc = 0;
        }
        clk.tick().await;
    }
}

fn main() {}

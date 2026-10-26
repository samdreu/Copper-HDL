#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: `o` is driven in two clock phases with MIXED anchors. The first
// write sits behind a deferred `x.read()` and runs at the pre-edge settle (closing);
// the second is a constant with no read before it and runs at its cycle's opening.
// Because the body reads an input, codegen emits the whole port as an edge-sampled
// register under `pc` guards, so the constant shows one observation after the
// simulator does. Measured (`e4_closing_then_opening` in
// tests/sequential_forwarding_divergence.rs): sim `[0,0,12,0,0,0,16,…]`, SV
// `[0,0,12,12,0,0,16,16,…]`. The same two writes with no input anywhere
// (`pulse_plain`, `e2_two_registers`) or both behind a deferred read
// (`e5d_two_closing_deferred`) are accepted and agree.
#[hardware(sequential)]
async fn mixed_anchors(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
        for _ in 0..2 { clk.tick().await; }
        o.write(Bits::zero());
    }
}

fn main() {}

#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: MIXED anchors on a continuous assign — the input read reaches
// the shared `o.write(r)` on one arm only, and the read-free arm assigns the
// register the write reads: the simulator runs that write at the cycle's
// opening with the forwarded `r + 1`, while `assign o = r` shows the committed
// register. Measured (the W4 witness, re-measured under forwarded emission
// 2026-08-26): the simulator holds `i + 1` while the SV alternates. Clause (A)
// of the re-derived rule (2026-10-28); the agreeing flip — the read-free arm
// assigning only a register the write does NOT read — is
// `unrelated_register_on_read_free_arm` in ui/pass/pretick_alignment_ok.rs.
//
// The V1 shape that used to live here (`r = r + 1; o.write(r);` with no read
// anywhere) was DISSOLVED by the cycle-dataflow forwarded emission and now lives
// in ui/pass/pretick_alignment_ok.rs.
#[hardware(sequential)]
async fn mixed_alignment(clk: Clock<MainClk>, i: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut phase: u8 = 0;
    let mut r: Bits<8> = Bits::zero();
    loop {
        if phase == 0 {
            r = i.read();
            phase = 1;
        } else {
            r = r + Bits::from_lit::<1>();
            phase = 0;
        }
        o.write(r);
        clk.tick().await;
    }
}

fn main() {}

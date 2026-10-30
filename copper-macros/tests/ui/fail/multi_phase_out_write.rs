#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: `o` is driven in two clock phases and one of its writes has
// anchors that differ between the two sides. In the middle phase the input read
// sits in a branch only odd `cnt` takes, and `o.write(cnt)` follows the merge: on
// the even path the simulator runs it at the cycle's opening, on the odd path
// behind the read's barrier; codegen sees a read before it on some path and holds
// it to the clock edge on every path, so the even-`cnt` writes show one
// observation late. Measured as `e11_path_dependent_write_in_a_middle_phase` in
// tests/sequential_forwarding_divergence.rs (sim `[0,0,1,0,0,2,2,0,…]`, SV
// `[0,0,1,0,0,0,2,0,…]`). Clause 4 of the rule as narrowed 2026-10-30 (a write
// fed only by a trailing immediate read, `e5`, is the clause's other witness). A
// mixed-anchor port whose drives each have a uniform anchor — a deferred read
// before one write, none before the other (`e4_closing_then_opening`, the former
// content of this file) — is realized by the drive-level mux + hold form and
// accepted: `two_phase_mixed_anchors` in ui/pass/pretick_alignment_ok.rs.
#[hardware(sequential)]
async fn path_dependent_middle_phase(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        if cnt[0] == Logic::One {
            let _ = x.read();
        }
        o.write(cnt);
        for _ in 0..2 { clk.tick().await; }
        o.write(Bits::zero());
    }
}

fn main() {}

#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: `o` is written ONCE, in a phase after the first, by a write whose
// anchors differ between the two sides — the `e11` shape of
// ui/fail/multi_phase_out_write.rs with its constant write removed. The input
// read sits in a branch only odd `cnt` takes and `o.write(cnt)` follows the
// merge: on the even path the simulator runs it at the cycle's opening, on the
// odd path behind the read's barrier; codegen sees a read before it on some path
// and edge-samples it on every path, so the even-`cnt` writes show one
// observation late. Measured as `e12_path_dependent_single_phase_extracted` in
// tests/sequential_forwarding_divergence.rs (sim `[0,0,1,1,1,2,2,…]`, SV
// `[0,0,1,1,1,1,2,…]`), `CYCLE_DATAFLOW_SEMANTICS.md` §10.6 (2026-10-30). Until
// that day neither rule reached a single non-head phase and this compiled clean.
// The two uniform controls — the read in both arms, no read at all — agree and
// are accepted: `single_phase_read_on_every_path` / `single_phase_no_read` in
// ui/pass/pretick_alignment_ok.rs.
#[hardware(sequential)]
async fn path_dependent_single_phase(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        if cnt[0] == Logic::One {
            let _ = x.read();
        }
        o.write(cnt);
        for _ in 0..2 { clk.tick().await; }
    }
}

fn main() {}

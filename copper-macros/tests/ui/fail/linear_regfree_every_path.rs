#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: on the linear multi-tick path a conditional write of `o` that
// covers every path of its segment is emitted as a combinational mux gated by
// the segment's clock phase and assigned nowhere else — a latch outside the
// phase, which Verilator refuses under the sweep's `-Wall`, and which with the
// lint off shows each closing value one observation early (the condition's read
// has a tick after it, so the simulator writes at the closing edge; the
// transparent copy shows `x` at every odd observation). The body has no
// register: until 2026-11-02 the rule returned nothing for this class.
// Measured as `e15w_head_every_path_constants` in
// tests/sequential_forwarding_divergence.rs (sim all `3`, SV `[3, 7, 3, 7, …]`),
// `CYCLE_DATAFLOW_SEMANTICS.md` §10.8. The remedies — the value computed into a
// `let` and written unconditionally (a wire behind the read, the flop form), or
// a single-tick body — are `linear_regfree_wire_behind_read` and
// `linear_regfree_held_behind_read` in ui/pass/pretick_alignment_ok.rs.
#[hardware(sequential)]
async fn linear_regfree_every_path(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    loop {
        if x.read()[0] == Logic::One { o.write(Bits::from_lit::<7>()); } else { o.write(Bits::from_lit::<3>()); }
        clk.tick().await;
        clk.tick().await;
    }
}

fn main() {}

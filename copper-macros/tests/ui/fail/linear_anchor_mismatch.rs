#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: on the linear multi-tick path (every `clk.tick().await` a bare
// statement) `o.write(cnt)` is a continuous assign of the committed register —
// the opening form — whatever reads precede it, while the input read with a
// tick after it parks the simulator so the write runs at the segment's closing
// edge. `cnt` changes at the other edge (it is updated in the head segment), so
// the hardware shows each value one observation before the simulator does, with
// no branch involved. Measured as `e13a_read_on_every_path_linear` in
// tests/sequential_forwarding_divergence.rs (sim `[0,1,1,2,2,…]`, SV
// `[1,1,2,2,3,…]`), `CYCLE_DATAFLOW_SEMANTICS.md` §10.6–10.7 (2026-10-30). The
// remedies — the update in the same segment before the write, or the read past
// the tick — are `linear_closing_in_segment` / `linear_opening_register` in
// ui/pass/pretick_alignment_ok.rs; a wire behind a read is `linear_wire_behind_read`.
#[hardware(sequential)]
async fn linear_register_behind_read(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        clk.tick().await;
        let _ = x.read();
        o.write(cnt);
        clk.tick().await;
    }
}

fn main() {}

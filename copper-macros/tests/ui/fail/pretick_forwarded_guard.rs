#![allow(unused_imports)]
use copper_core::port::Out;
use copper_core::{Bits, Clock, ClockDomain};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: a FORWARDED guard. The drive has no read before it (opening on
// both sides, so the port takes the mux + hold form), but its `if` tests `cnt`
// right after the segment assigned it: the simulator tests the forwarded value
// (`cnt + 1`), the mux's combinational copy tests the committed register, which
// never equals 3 here. Measured as `d4a_forwarded_guard_hold` in
// tests/sequential_forwarding_divergence.rs (sim `[0, 7, 7, …]`, SV `[0, 0, 7, …]`:
// only the hold register, loaded under the edge-form guard, catches the pulse,
// one observation late). Clause (B) of the re-derived rule (2026-10-28). The
// agreeing spelling tests committed state and updates after the test
// (`committed_guard_pulse` in ui/pass/pretick_alignment_ok.rs).
#[hardware(sequential)]
async fn forwarded_guard(clk: Clock<MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        if cnt == Bits::from_lit::<3>() {
            o.write(Bits::from_lit::<7>());
            cnt = Bits::zero();
        }
        clk.tick().await;
    }
}

fn main() {}

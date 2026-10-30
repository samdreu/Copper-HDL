use copper_core::port::{In, Out, RegOut};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// All four of these are ACCEPTED, and each isolates one clause of the rule.

// 1. The register update happens AFTER the tick, so the pre-tick segment only reads
//    state. This is the form measured to match independent hand-written Verilog.
#[hardware(sequential)]
async fn post_tick_update(clk: Clock<MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        o.write(r);
        clk.tick().await;
        r = r + Bits::from_lit::<1>();
    }
}

// 2. `RegOut` is immune — it commits at the clock edge, so the phase at which the
//    write executes cannot be observed.
#[hardware(sequential)]
async fn registered_output(clk: Clock<MainClk>, o: RegOut<Bits<8>, MainClk>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = r + Bits::from_lit::<1>();
        o.write(r);
        clk.tick().await;
    }
}

// 3. A leading `In` read PRECEDES the assignment, installing the barrier that pins
//    the segment to the pre-edge phase.
#[hardware(sequential)]
async fn leading_read(clk: Clock<MainClk>, i: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = r + i.read();
        o.write(r);
        clk.tick().await;
    }
}

// 4. The write is a CONSTANT on every path with no read anywhere: opening on
//    both sides, a continuous assign of the constant.
#[hardware(sequential)]
async fn constant_write(clk: Clock<MainClk>, o: Out<Logic, MainClk>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = r + Bits::from_lit::<1>();
        o.write(Logic::One);
        clk.tick().await;
    }
}

fn main() {}

// ── Admitted by the re-derived D1 rule (2026-10-28, CYCLE_DATAFLOW_SEMANTICS.md
// §10) — each the measured-agreeing flip of a refused shape. ─────────────────

// A constant written in a state arm that no input read reaches (the read sits in
// the SIBLING arm): opening on both sides, phase C's mux + hold. The pre-2026-10-28
// constant-write clause refused this because `pc` is assigned with no read before
// it; measured agreeing as `pc_arm_write` since the arm-granular opening walk.
#[hardware(sequential)]
async fn state_arm_constant(clk: Clock<MainClk>, sel: In<Logic, MainClk>, o: Out<Logic, MainClk>) {
    let mut pc: u8 = 0;
    loop {
        match pc {
            0u8 => { if sel.read() == Logic::One { pc = 1; } }
            1u8 => { o.write(Logic::One); pc = 0; }
            _ => {}
        }
        clk.tick().await;
    }
}

// A constant written on some paths BEHIND the read in its own condition, with a
// register assigned unprotected before it: closing on both sides, edge-sampled
// with the edge-form guard (`d1b_const_read_in_arm`, measured agreeing).
#[hardware(sequential)]
async fn constant_behind_read(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        if x.read()[0] == Logic::One {
            o.write(Bits::from_lit::<7>());
        }
        clk.tick().await;
    }
}

// W4 with the read-free arm assigning only a register the shared write does NOT
// read: the opening write publishes the committed `r`, which is what
// `assign o = r` shows (`d3a_w4_unrelated_register`, measured agreeing). One
// assignment away from the refused ui/fail/pretick_alignment.rs case.
#[hardware(sequential)]
async fn unrelated_register_on_read_free_arm(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut phase: u8 = 0;
    let mut r: Bits<8> = Bits::zero();
    loop {
        if phase == 0 {
            r = x.read();
            phase = 1;
        } else {
            phase = 0;
        }
        o.write(r);
        clk.tick().await;
    }
}

// A held pulse whose guard tests the COMMITTED register (the update comes after
// the test): opening on both sides, mux + hold (`d4b_committed_guard_hold`,
// measured agreeing). The refused spelling updates before the test —
// ui/fail/pretick_forwarded_guard.rs.
#[hardware(sequential)]
async fn committed_guard_pulse(clk: Clock<MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        if cnt == Bits::from_lit::<2>() {
            o.write(Bits::from_lit::<7>());
            cnt = Bits::zero();
        } else {
            cnt = cnt + Bits::from_lit::<1>();
        }
        clk.tick().await;
    }
}

// ── Accepting clauses of the write-between-read-and-update rule (V8) ─────────
// Each is a measured-agreeing flip of the V8a divergent shape.

// V8b: the write comes AFTER the update, so it publishes the committing
// (forwarded) value — which is Q from the next observation on. lfsr's shape.
#[hardware(sequential)]
async fn v8b_publish_after_update(
    clk: Clock<MainClk>,
    step: In<Bits<8>, MainClk>,
    o: Out<Bits<8>, MainClk>,
) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        let s = step.read();
        r = r + s;
        o.write(r);
        clk.tick().await;
    }
}

// V8c: the write comes BEFORE the read, so it executes ahead of the barrier
// point — at the cycle's opening, on committed state, exactly where the
// observation samples it.
#[hardware(sequential)]
async fn v8c_publish_before_read(
    clk: Clock<MainClk>,
    step: In<Bits<8>, MainClk>,
    o: Out<Bits<8>, MainClk>,
) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        o.write(r);
        r = r + step.read();
        clk.tick().await;
    }
}

// 5. The V1 shape — a register updated pre-tick with NO input read anywhere, and
//    a plain `Out` written from it. DISSOLVED 2026-08-26 by the cycle-dataflow
//    forwarded emission (phase B): the opening-prefix drive now emits
//    `assign o = r + 1`, the meaning the simulator always had — measured
//    agreeing. This used to be the ui/fail case.
#[hardware(sequential)]
async fn forwarded_opening_drive(clk: Clock<MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = r + Bits::from_lit::<1>();
        o.write(r);
        clk.tick().await;
    }
}

// Phase E (2026-10-23): a plain `Out` driven in TWO clock phases is ACCEPTED when
// the body reads no input — every drive runs at its cycle's opening on both sides,
// and the extracted route emits a forwarded mux with a hold register. Measured
// agreeing as `e2_two_registers` in tests/sequential_forwarding_divergence.rs.
#[hardware(sequential)]
async fn two_phase_opening_mux(clk: Clock<MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut a: Bits<8> = Bits::zero();
    let mut b: Bits<8> = Bits::zero();
    loop {
        a = a + Bits::from_lit::<1>();
        b = b + Bits::from_lit::<3>();
        for _ in 0..2 { clk.tick().await; }
        o.write(a);
        clk.tick().await;
        o.write(b);
    }
}

// …also when the body reads an input in ANOTHER phase that no write of the port
// is reached by (2026-10-26: codegen's opening walk is arm-granular, so the read
// demotes nothing; `e2x_two_registers_plus_unrelated_read`, measured agreeing)…
#[hardware(sequential)]
async fn two_phase_opening_mux_with_unrelated_read(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut a: Bits<8> = Bits::zero();
    let mut b: Bits<8> = Bits::zero();
    loop {
        a = x.read();
        b = b + Bits::from_lit::<3>();
        for _ in 0..2 { clk.tick().await; }
        o.write(a);
        clk.tick().await;
        o.write(b);
    }
}

// …and when every write of it sits behind an input read that a tick follows:
// every drive then lands at the closing edge on both sides
// (`e5d_two_closing_deferred`, measured agreeing).
#[hardware(sequential)]
async fn two_phase_closing_mux(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read() + cnt);
        clk.tick().await;
    }
}

// ── Admitted 2026-10-30 (CYCLE_DATAFLOW_SEMANTICS.md §10.4): the drive-level mux +
// hold form realizes a held mixed-anchor port drive by drive, so the two
// mixed-anchor clauses were narrowed to the drives whose anchors differ between
// the sides (and the every-path mux). Each shape below is measured agreeing. ──

// `d2a`: an opening constant under committed state beside a closing constant
// behind the input read, on a HELD port. Neither drive is path-dependent; the comb
// copy muxes the `5`, the hold register is loaded by both drives. Refused by
// clause (A) from 2026-10-28 to 2026-10-30; the every-path spelling is still
// refused — ui/fail/pretick_mixed_anchors.rs.
#[hardware(sequential)]
async fn mixed_anchors_held(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
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

// `d5b`: the same with the opening drive register-valued and FORWARDING its own
// update (the comb arm shows `cnt + 1`), and the state keyed off another bit so
// the closing value reaches the output through the hold register. Measured
// agreeing on this route and on the extraction route (`d5e`).
#[hardware(sequential)]
async fn mixed_anchors_held_register_opening(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut st: u8 = 0;
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        if st == 1 {
            o.write(cnt);
        }
        let xv = x.read();
        if xv[0] == Logic::One {
            o.write(Bits::from_lit::<9>());
        }
        if xv[1] == Logic::One {
            st = 1;
        } else {
            st = 0;
        }
        clk.tick().await;
    }
}

// `e4_closing_then_opening`: a two-phase port with a deferred read before one
// write and no read before the other. Each drive's anchor is uniform on both
// sides, so the drive-level mux + hold form shows both at the simulator's instant.
// Refused by `multi_phase_out_write` clause 4 from 2026-10-23 to 2026-10-30; the
// path-dependent spelling is still refused — ui/fail/multi_phase_out_write.rs.
#[hardware(sequential)]
async fn two_phase_mixed_anchors(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
        for _ in 0..2 { clk.tick().await; }
        o.write(Bits::zero());
    }
}

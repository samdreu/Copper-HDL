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

// 4. The write is a CONSTANT, so it is idempotent across the phase shift — the
//    misalignment changes only *when* the write happens, which is unobservable.
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

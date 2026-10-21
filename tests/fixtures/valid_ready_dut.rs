// Valid/ready handshake spellings, pinned as swept fixtures (2026-10-20).
//
// A review probed how the common handshake idioms fare across the three
// front-ends — the `#[hardware]` macro's compile-time rules, the transpiler, and
// the differential sweep (simulator vs. the emitted SystemVerilog under
// Verilator). Each module below is one spelling; the sweep is the measurement.
// The verdicts, recorded in `build.rs`'s SKIP table where a module does not
// sweep as-is:
//
// * `vr_src_regout`, `vr_sink_moore`, `vr_src_same_cycle`,
//   `vr_read_after_delay` — AGREE. Every output is a `RegOut`, so the phase a
//   write executes in is unobservable through the port.
// * `vr_sink_mealy`, `vr_skid` — DIVERGE by one cycle (hardware leads). A plain
//   `Out` ready is written from a register and an input THROUGH A `let`
//   TEMPORARY, and the register is updated later in the same segment: the V8a
//   shape of `pretick_out_write_before_update`, which the rule did not see
//   through the temporary. As of 2026-10-20 it does, so both carry
//   `allow_pretick_alignment` as demonstrations; the refused spellings are
//   `copper-macros/tests/ui/fail/write_before_update_via_temp.rs` and
//   `write_before_update_via_bool_temp.rs`.

/// Producer with registered `valid`/`data`: idle until `go`, then present one
/// item and hold it until the consumer's `ready`.
#[hardware(sequential)]
async fn vr_src_regout(
    clk: Clock<MainClk>,
    go: In<Logic, MainClk>,
    ready: In<Logic, MainClk>,
    valid: RegOut<Logic, MainClk>,
    data: RegOut<Bits<8>, MainClk>,
) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        valid.write(Logic::Zero);
        while go.read() == Logic::Zero {
            clk.tick().await;
        }
        data.write(n);
        valid.write(Logic::One);
        clk.tick().await;
        while ready.read() == Logic::Zero {
            clk.tick().await;
        }
        valid.write(Logic::Zero);
        n = n + Bits::from_lit::<1>();
        clk.tick().await;
    }
}

/// Consumer with a registered (Moore) `ready`: raise it, wait for `valid`,
/// take the item, drop `ready` for two cycles.
#[hardware(sequential)]
async fn vr_sink_moore(
    clk: Clock<MainClk>,
    valid: In<Logic, MainClk>,
    data: In<Bits<8>, MainClk>,
    ready: RegOut<Logic, MainClk>,
    sum: RegOut<Bits<8>, MainClk>,
) {
    let mut acc: Bits<8> = Bits::zero();
    loop {
        ready.write(Logic::One);
        clk.tick().await;
        while valid.read() == Logic::Zero {
            clk.tick().await;
        }
        acc = acc + data.read();
        ready.write(Logic::Zero);
        sum.write(acc);
        clk.tick().await;
        clk.tick().await;
    }
}

/// Consumer with a combinational (Mealy) `ready = !full || drain`, computed
/// into a `let` temporary before the write. `full` is updated later in the
/// same single-tick segment, so the write publishes its PRE-update value: the
/// V8a divergence, one cycle, hardware leading. Measured in the sweep before
/// the rule learned to see through the temporary; kept as the demonstration.
#[hardware(sequential, allow_pretick_alignment)]
async fn vr_sink_mealy(
    clk: Clock<MainClk>,
    valid: In<Logic, MainClk>,
    data: In<Bits<8>, MainClk>,
    drain: In<Logic, MainClk>,
    ready: Out<Logic, MainClk>,
    sum: RegOut<Bits<8>, MainClk>,
) {
    let mut full: Logic = Logic::Zero;
    let mut held: Bits<8> = Bits::zero();
    let mut acc: Bits<8> = Bits::zero();
    loop {
        let can_take: Logic = if full == Logic::Zero || drain.read() == Logic::One {
            Logic::One
        } else {
            Logic::Zero
        };
        ready.write(can_take);
        if full == Logic::One && drain.read() == Logic::One {
            acc = acc + held;
            full = Logic::Zero;
        }
        if can_take == Logic::One && valid.read() == Logic::One {
            held = data.read();
            full = Logic::One;
        }
        sum.write(acc);
        clk.tick().await;
    }
}

/// One-entry skid buffer: `out_valid`/`out_data` are written before any input
/// read (the agreeing V8c position), while `in_ready = !full || out_ready`
/// goes through a `let` temporary behind the `out_ready` read and ahead of the
/// `full` update — the same V8a divergence as `vr_sink_mealy`.
#[hardware(sequential, allow_pretick_alignment)]
async fn vr_skid(
    clk: Clock<MainClk>,
    in_valid: In<Logic, MainClk>,
    in_data: In<Bits<8>, MainClk>,
    out_ready: In<Logic, MainClk>,
    in_ready: Out<Logic, MainClk>,
    out_valid: Out<Logic, MainClk>,
    out_data: Out<Bits<8>, MainClk>,
) {
    let mut full: Logic = Logic::Zero;
    let mut held: Bits<8> = Bits::zero();
    loop {
        out_valid.write(full);
        out_data.write(held);
        let ir: Logic = if full == Logic::Zero || out_ready.read() == Logic::One {
            Logic::One
        } else {
            Logic::Zero
        };
        in_ready.write(ir);
        if full == Logic::One && out_ready.read() == Logic::One {
            full = Logic::Zero;
        }
        if ir == Logic::One && in_valid.read() == Logic::One {
            held = in_data.read();
            full = Logic::One;
        }
        clk.tick().await;
    }
}

/// Producer whose `valid` may rise and fall in the same cycle: written `One`,
/// then dropped right after the `ready` wait, which costs no cycle when `ready`
/// is already high. Both writes are to a `RegOut`, so the last one commits.
#[hardware(sequential)]
async fn vr_src_same_cycle(
    clk: Clock<MainClk>,
    ready: In<Logic, MainClk>,
    valid: RegOut<Logic, MainClk>,
    count: RegOut<Bits<8>, MainClk>,
) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        valid.write(Logic::One);
        while ready.read() == Logic::Zero {
            clk.tick().await;
        }
        valid.write(Logic::Zero);
        n = n + Bits::from_lit::<1>();
        count.write(n);
        clk.tick().await;
    }
}

/// Consumer that reads `data` two ticks after it saw `valid` — the
/// read-after-delay shape README's Limitations section discusses. The read
/// opens its own segment (nothing precedes it there), so it samples at that
/// cycle's opening in both the simulator and the FSM.
#[hardware(sequential)]
async fn vr_read_after_delay(
    clk: Clock<MainClk>,
    valid: In<Logic, MainClk>,
    data: In<Bits<8>, MainClk>,
    sum: RegOut<Bits<8>, MainClk>,
) {
    let mut acc: Bits<8> = Bits::zero();
    loop {
        while valid.read() == Logic::Zero {
            clk.tick().await;
        }
        clk.tick().await;
        clk.tick().await;
        acc = acc + data.read();
        sum.write(acc);
        clk.tick().await;
    }
}

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
// * `vr_sink_mealy`, `vr_skid` — deleted 2026-11-06 (Phase C step 3): each was a
//   plain-`Out` Mealy `ready` kept with `allow_pretick_alignment` to demonstrate
//   the V8a divergence. Under D-8 a clocked body has no plain `Out`, so the shape
//   is unexpressible; the registered spelling is `vr_sink_moore`, and the Mealy
//   ready's one-job form is `q1_b_vr_sink` (`tests/fixtures/q1_b_rewrites_dut.rs`).

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

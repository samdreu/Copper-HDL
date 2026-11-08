//! Tracked regressions for constructs the transpiler does NOT support today
//! (P2, `TODO` TESTING plan). Distinct from `transpile_inference_gaps.rs` (which
//! pins bit-width *inference* gaps): these are constructs the lowering explicitly
//! rejects — an unsupported operator, method, or port type.
//!
//! Each test PINS the current "unsupported" boundary with the exact diagnostic, so
//! the limitation stays visible and a future contributor who adds support gets a
//! loud `expect_err` failure — the signal to promote the construct to a positive
//! equivalence test rather than relax the assertion.
//!
//! Discovered empirically via the `copper-transpile` CLI while filling the P2
//! construct matrix (see `TODO` TRANSPILATION). Supported neighbours that DO
//! transpile — and now have equivalence tests — are wide datapaths, `*`, `-`
//! (two's-complement negate), and `<<`/`>>`.

fn transpile(src: &str) -> Result<String, String> {
    copper_codegen::transpile_source(src, None, &copper_codegen::EmitConfig::default())
        .map_err(|e| e.to_string())
}

/// The division operator `/` — SUPPORTED since Phase C step 8 (2026-11-07, SPEC
/// I.14.3(b), I.14.7 (5)): lowered as `%` is, `(a / b)`. Until then it was refused
/// ("binary operator '/' not supported", D-19's `xp_div`); the behavioural check is
/// the corpus sweep of `tests/fixtures/data_i14_dut.rs::xp_div`.
const DIV_DUT: &str = r#"
#[hardware(combinational)]
fn div8(a: In<Bits<8>, C>, b: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    o.write(a.read() / b.read());
}
"#;

/// `arithmetic_shift_right` — the signed (sign-filling) shift. SUPPORTED as of
/// 2026-08-27: it lowers to `$signed(a) >>> n`, the same SignCast path the
/// `as i32` cast spelling takes (signedness_dut anchors that semantics against
/// Verilator). The sim≡transpiled behavioural check is the corpus sweep of
/// `tests/fixtures/struct_pipeline_dut.rs::sra_method`.
const ASR_DUT: &str = r#"
#[hardware(combinational)]
fn asr8(a: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    o.write(a.read().arithmetic_shift_right(2));
}
"#;


#[test]
fn division_operator_is_supported() {
    let sv = transpile(DIV_DUT).expect("`/` transpiles since Phase C step 8");
    assert!(sv.contains("(a / b)"), "expected `(a / b)` in the emitted SV, got:\n{sv}");
}

/// Guards the `/`-vs-`%` asymmetry: `%` really does transpile. If this ever starts
/// failing, `%` support regressed and the comment on `DIV_DUT` is stale.
#[test]
fn remainder_operator_is_supported() {
    let sv = transpile(
        r#"
        #[hardware(combinational)]
        fn rem8(a: In<Bits<8>, C>, b: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
            o.write(a.read() % b.read());
        }
        "#,
    )
    .expect("`%` should transpile (it is supported, unlike `/`)");
    assert!(sv.contains("%"), "expected a `%` in the emitted SV, got:\n{sv}");
}

#[test]
fn arithmetic_shift_right_is_supported() {
    let sv = transpile(ASR_DUT)
        .expect("`arithmetic_shift_right` should transpile (supported 2026-08-27)");
    assert!(
        sv.contains(">>>") && sv.contains("$signed"),
        "expected the signed shift `$signed(…) >>> …` in the emitted SV, got:\n{sv}"
    );
}

/// A tick inside a nested `loop` — the "wait until ready" idiom — DOES transpile,
/// as a self-looping FSM state (control extraction increment B).
/// `tests/wait_loop_equivalence.rs` at the repo root carries the behavioural
/// checks; this is the control that the shape reaches the flattener at all.
///
/// Historically this is also the shape that used to CRASH the transpiler: the gate
/// and the tick search in `control_extract.rs` disagreed about where a tick can
/// live. `no_transpiler_panics.rs` guards that failure MODE corpus-wide.
#[test]
fn tick_inside_a_nested_loop_is_supported() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, go: In<Logic, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        o.write(n);
        loop {
            if go.read() == Logic::One { break; }
            clk.tick().await;
        }
        n = n + Bits::from_lit::<1>();
        clk.tick().await;
    }
}
"#;
    let sv = transpile(src).expect("a repeating wait must transpile");
    assert!(
        sv.contains("case (pc)"),
        "a nested wait must flatten to a pc FSM, got:\n{sv}"
    );
}

/// The tick must be the LAST statement of the wait's body. The other ordering is
/// **outside the language by decision** (2026-08-24), not merely unimplemented: it
/// puts the test in the window where an `Immediate` read consumes the value the
/// just-past edge produced while the flip-flop it lowers to samples the value
/// before its own edge. Measured, the transpiled module reacted a full cycle
/// early, and holding the stimulus two cycles did not reconcile them.
///
/// Same disposition as the pre-tick alignment hazard: the divergent program is
/// made unwritable rather than the divergence adjudicated. See
/// `design_docs/SYNCHRONOUS_SEMANTICS.md`.
#[test]
fn ticking_before_the_test_in_a_wait_is_unsupported() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, go: In<Logic, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        o.write(n);
        loop {
            clk.tick().await;
            if go.read() == Logic::One { break; }
        }
        n = n + Bits::from_lit::<1>();
        clk.tick().await;
    }
}
"#;
    let err = transpile(src).expect_err(
        "This ordering is refused BY DESIGN, not pending support — see \
         SYNCHRONOUS_SEMANTICS.md. If it now transpiles, a language decision was reversed; \
         that needs sign-off and a hardware-anchored check that the module no longer reacts a \
         cycle earlier than the simulator, not just a green test.",
    );
    assert!(
        err.contains("LAST statement"),
        "the diagnostic must state the ordering rule: {err}"
    );
}

/// **Cause K** — a nested loop whose body ends in ANOTHER tick-bearing loop, so its
/// clock boundary comes from the inner loop rather than a tick of its own. The
/// flattener always modelled this (a `break` inlines the enclosing loop's
/// continuation in the same cycle); only the gate refused it.
///
/// `tests/nested_boundary_equivalence.rs` carries the behavioural half.
#[test]
fn a_loop_ending_in_a_tick_bearing_loop_is_supported() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, step: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        o.write(n);
        loop {
            n = n + step.read();
            clk.tick().await;
        }
    }
}
"#;
    let sv = transpile(src).expect("a loop ending in a tick-bearing loop must transpile");
    assert!(
        sv.contains("case (pc)"),
        "the shape must flatten to a pc FSM, got:\n{sv}"
    );
}

/// The other half of cause K, and the reason relaxing the gate was not enough on its
/// own. An enclosing loop whose ONLY boundary is a nested loop that can `break`
/// before it ticks returns to its top in zero time. Measured before this was
/// rejected: the simulator livelocks (99.5% CPU, no progress) while the flattened
/// FSM runs one cycle per iteration — a silent sim ≢ SV divergence, and a false PASS.
///
/// Owned by `copper_analysis::check_reachability`, not by the gate, so both
/// front-ends refuse it. See `copper-analysis/src/cfg.rs::may_exit_without_tick`.
#[test]
fn a_loop_whose_only_boundary_can_exit_before_ticking_is_rejected() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, a: In<Logic, MainClk>, b: In<Logic, MainClk>, o: RegOut<Logic, MainClk>) {
    loop {
        loop {
            o.write(a.read());
            if a.read() == Logic::One { break; }
            loop {
                if b.read() == Logic::One { break; }
                clk.tick().await;
            }
        }
    }
}
"#;
    let err = transpile(src).expect_err(
        "a zero-time outer cycle must be refused; if this transpiles, the transpiler and \
         the simulator disagree about a program the simulator cannot even run",
    );
    assert!(
        err.contains("left before it ticks"),
        "the diagnostic must name the nested loop as the missing boundary rather than \
         advise adding a tick to a body that already has one: {err}"
    );
}

/// Ticks that live only inside `if`/`match` arms of a nested loop's body. Refused in
/// both front-ends ("no clock boundary of its own", `check_nested_loop_shape` clause
/// 4) from 2026-08-24 until Phase C step 5 (2026-11-06, II.3 (b), MEASUREMENTS §J.5):
/// the lowering behind the clause spent a cycle on every fall-through to the head.
/// The fall-through is now zero-time and the shape transpiles; its behaviour is swept
/// by `tests/fixtures/arms_only_loop_dut.rs` (D-13's `b1`, `b2`, and a `match`).
#[test]
fn ticks_only_inside_branches_of_a_nested_loop_transpile() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, c: In<Logic, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        o.write(n);
        loop {
            if c.read() == Logic::One { clk.tick().await; } else { break; }
        }
        n = n + Bits::from_lit::<1>();
        clk.tick().await;
    }
}
"#;
    transpile(src).expect("ticks only inside branches of a nested loop transpile (II.3 (b) closed)");
}

#[test]
fn continue_in_a_wait_is_unsupported() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, go: In<Logic, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        o.write(n);
        loop {
            if go.read() == Logic::Zero { continue; }
            break;
        }
        n = n + Bits::from_lit::<1>();
        clk.tick().await;
    }
}
"#;
    let err = transpile(src).expect_err("`continue` must be refused");
    assert!(
        !err.is_empty() && !err.starts_with("0:0:"),
        "the `continue` diagnostic must carry a span: {err}"
    );
}

/// A nested `loop` with no tick, in a module that does tick elsewhere. Distinct
/// message from the one above because the fix is different — and it has to be its
/// own case: a module with NO tick at all is caught earlier and better by the
/// shared reachability guarantee ("every path through a `#[hardware]` loop must
/// reach `clk.tick().await`"), which is asserted below so the division of labour
/// between the two checks stays visible.
#[test]
fn nested_loop_without_a_tick_is_unsupported() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    loop {
        loop {
            o.write(a.read());
        }
        clk.tick().await;
    }
}
"#;
    let err = transpile(src).expect_err("an unbounded combinational loop must be rejected");
    assert!(
        err.contains("would never terminate"),
        "reproduced a *different* error than the untick'd nested loop: {err}"
    );

    // Same shape, but the module never ticks at all: the header rule R20's tick
    // clause owns this one in both front-ends since Phase C step 10 (SPEC II.1.3:
    // the shared rules run first, in the macro's order), and its message names
    // the missing tick, the better of the two.
    let no_tick = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    loop {
        loop {
            o.write(a.read());
        }
    }
}
"#;
    let err = transpile(no_tick).expect_err("a loop with no tick must be rejected");
    assert!(
        err.contains("body must contain `clk.tick().await`"),
        "the header's tick clause should own the no-tick case: {err}"
    );
}

// ── Memory: what lowers, and where the boundary is (P4) ──────────────────────
//
// `Memory` used to be listed here as a whole feature with NO transpiled path —
// an entire first-class construct that existed only in the simulator, so every
// memory guarantee was sim-only and the project's central claim (one source both
// simulates and synthesises, provably in agreement) did not extend to designs
// using it.
//
// As of 2026-08-24 the single-cycle ReadFirst form lowers, and
// `tests/dual_port_ram_equivalence.rs` carries sim ≡ transpiled SV for the
// shipped example — which in turn is checked against an independent hand-written
// `examples/memory/sv/dual_port_ram.sv`. Preloaded contents followed on the same
// day (`initial` block; `tests/preloaded_memory_equivalence.rs`), then WriteFirst
// (`tests/write_first_memory_equivalence.rs`), multi-phase access
// (`tests/multiphase_memory_equivalence.rs`) and pipelined latency
// (`tests/pipelined_memory_equivalence.rs`). Every remaining memory test below is
// a RULE — a shape hardware cannot express, or contents the transpiler cannot
// see — rather than an unimplemented construct.
//
// One decision still rests on the absence of a synthesised counterpart:
// out-of-range addressing is a deliberate panic naming the port, address and size
// (`copper-core/src/memory.rs::out_of_range`). That was decided on diagnostic
// grounds because nothing existed to adjudicate against. An emitted array now
// exists, so it is worth revisiting — SystemVerilog reads X out of range.

/// A `Memory<..>` DUT that differs from the transpilable baseline in exactly one
/// way. Every pin below shares this shape so the *only* reason each fails is the
/// construct it names.
fn mem_dut(decl: &str, body: &str) -> String {
    format!(
        r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, d: In<Bits<16>, MainClk>,
           we: In<Logic, MainClk>, o: RegOut<Bits<16>, MainClk>) {{
    {decl}
    let mut q: Bits<16> = Bits::zero();
    loop {{
        {body}
        clk.tick().await;
        if mem.read_port::<0>().is_ready() {{ q = mem.read_port::<0>().data(); }}
        o.write(q);
    }}
}}
"#
    )
}

const MEM_DECL: &str = "let mem = Memory::<Bits<16>, 1, 1, MainClk, 1, 1>::new(clk.clone(), 256);";
const MEM_BODY: &str = r#"
        if we.read() == Logic::One { mem.write_port::<0>().write(a.read().as_usize(), d.read()); }
        mem.read_port::<0>().read(a.read().as_usize());
"#;

/// The control for every memory pin below: this exact shape DOES transpile, so a
/// failure in one of them is about the construct it changes and nothing else.
/// `tests/dual_port_ram_equivalence.rs` at the repo root carries the behavioural
/// sim ≡ transpiled-SV check for the real example.
#[test]
fn single_cycle_readfirst_memory_is_supported() {
    let sv = transpile(&mem_dut(MEM_DECL, MEM_BODY))
        .expect("the baseline memory shape must transpile — every pin below assumes it");
    assert!(
        sv.contains("logic [15:0] mem [0:255];"),
        "expected a packed memory array, got:\n{sv}"
    );
}

/// Read/write latency greater than one cycle DOES transpile: a read port becomes
/// a register chain and a write port's commit comes from its last stage.
/// `tests/pipelined_memory_equivalence.rs` carries the behavioural checks at
/// READ_LAT 2 and 3 and WRITE_LAT 2, including a WriteFirst forward from the
/// committing stage.
#[test]
fn pipelined_memory_latency_is_supported() {
    let src = mem_dut(
        "let mem = Memory::<Bits<16>, 1, 1, MainClk, 2, 1>::new(clk.clone(), 256);",
        MEM_BODY,
    );
    let sv = transpile(&src).expect("a pipelined memory must transpile");
    assert!(
        sv.contains("mem_rd0_q0 <= mem_rd0_data;"),
        "READ_LAT=2 must emit a capture stage, got:\n{sv}"
    );
}

/// Zero latency is not a thing a synchronous port can do, and the simulator's
/// pipelines index `[LAT - 1]`, so it is refused rather than underflowing.
#[test]
fn zero_latency_memory_is_rejected() {
    let src = mem_dut(
        "let mem = Memory::<Bits<16>, 1, 1, MainClk, 0, 1>::new(clk.clone(), 256);",
        MEM_BODY,
    );
    let err = transpile(&src).expect_err("READ_LAT = 0 must be rejected");
    assert!(
        err.contains("must be at least 1"),
        "reproduced a *different* error than the zero-latency rule: {err}"
    );
}

/// WriteFirst read-during-write DOES transpile, as a same-address forwarding mux
/// on the read port's output net. `tests/write_first_memory_equivalence.rs` at the
/// repo root carries the behavioural check, including a differential against
/// ReadFirst on identical stimulus (the two modes agree on every cycle except a
/// same-address read/write, so nothing else can tell them apart).
#[test]
fn write_first_memory_is_supported() {
    let src = mem_dut(
        "let mem = Memory::<Bits<16>, 1, 1, MainClk, 1, 1>::new(clk.clone(), 256).write_first();",
        MEM_BODY,
    );
    let sv = transpile(&src).expect("a WriteFirst memory must transpile");
    assert!(
        sv.contains("mem_wr0_en && (mem_wr0_addr == mem_rd0_addr)"),
        "WriteFirst must forward a same-address write to the read, got:\n{sv}"
    );
}

/// A read-only preloaded memory: the shape the pins below vary one thing from.
fn rom_dut(decl: &str) -> String {
    format!(
        r#"
#[hardware(sequential)]
async fn r(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, o: RegOut<Bits<16>, MainClk>) {{
    let rom = {decl};
    let mut q: Bits<16> = Bits::zero();
    loop {{
        rom.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        if rom.read_port::<0>().is_ready() {{ q = rom.read_port::<0>().data(); }}
        o.write(q);
    }}
}}
"#
    )
}

/// Preloaded contents DO transpile, as an `initial` block.
/// `tests/preloaded_memory_equivalence.rs` at the repo root carries the
/// behavioural sim ≡ transpiled-SV check for both constructors.
#[test]
fn preloaded_memory_is_supported() {
    let sv = transpile(&rom_dut(
        "Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::from_fn(clk.clone(), 16, |i| Bits::from_usize(i * 3))",
    ))
    .expect("a `from_fn` preload must transpile");
    assert!(
        sv.contains("initial begin") && sv.contains("for (int i = 0; i < 16; i++)"),
        "expected a fill loop in an initial block, got:\n{sv}"
    );
}

/// Contents that only exist at run time. `examples/cpu/rv32i_cpu.rs` is the real
/// instance: `from_contents(clk, flat.clone())`, where `flat` is a program image
/// assembled by the harness. The transpiler does not execute Rust, so there is
/// nothing to emit — and emitting a zero-filled array instead would be a silently
/// wrong design.
#[test]
fn runtime_computed_preload_is_unsupported() {
    let err = transpile(&rom_dut(
        "Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::from_contents(clk.clone(), flat.clone())",
    ))
    .expect_err(
        "NOW SUPPORTED: a run-time `Vec` preload transpiles — which would mean the transpiler \
         gained a way to evaluate Rust. Check very carefully what it actually emits.",
    );
    assert!(
        err.contains("computed at run time") && err.contains("does not execute Rust"),
        "reproduced a *different* error than the tracked run-time-contents gap: {err}"
    );
}

/// A fill that is not written at the call site — a named function, or a closure
/// bound to a variable. Same root cause as above: there is no body to emit.
#[test]
fn non_inline_fill_function_is_unsupported() {
    let err = transpile(&rom_dut(
        "Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::from_fn(clk.clone(), 16, make_word)",
    ))
    .expect_err("a `from_fn` fill that is not an inline closure must be rejected");
    assert!(
        err.contains("must be a closure written at the call site"),
        "reproduced a *different* error than the inline-fill rule: {err}"
    );
}

/// A preload that reads one of the module's own signals. The simulator would
/// evaluate the captured port ONCE at construction; an `initial` block samples it
/// at time 0. Those are different things that look alike in source, so the shape
/// is rejected rather than emitted.
#[test]
fn preload_reading_a_signal_is_rejected() {
    let err = transpile(&rom_dut(
        "Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::from_fn(clk.clone(), 16, |i| Bits::from_usize(i) + a.read())",
    ))
    .expect_err("a preload that reads a signal must be rejected");
    assert!(
        err.contains("Initial contents must be constant"),
        "reproduced a *different* error than the constant-preload rule: {err}"
    );
}

/// Two stagings of one port in one resumption DO transpile, the last winning (SPEC
/// I.10.1(b); rule 6 (b) retired by D-16 B, removed by D-25 — until then this exact
/// shape was refused as "accessed 2 times in one cycle"). The emitted `always_comb`
/// assigns the write nets twice and the later blocking assignment is what the port
/// sees — the simulator's overwrite of pipeline stage 0. The three-leg check is the
/// sweep fixture `tests/fixtures/memory_staging_dut.rs::stage_twice_write`.
#[test]
fn two_stagings_of_one_memory_port_lower_last_wins() {
    let src = mem_dut(
        MEM_DECL,
        &format!("{MEM_BODY}\n        mem.write_port::<0>().write(0, d.read());"),
    );
    let sv = transpile(&src).expect("a port staged twice in one resumption transpiles (I.10.1(b))");
    let first = sv.find("mem_wr0_addr = 8'(a)").expect("the guarded staging's address");
    let last = sv.rfind("mem_wr0_addr = 8'(64'd0)").expect("the unconditional staging's address");
    assert!(first < last, "the later staging must be the later assignment:\n{sv}");
}

/// A received memory may leave its latencies at the type's defaults (SPEC I.10.5,
/// D-25 (5)): `Memory<T, R, W, D>` lowers exactly as `Memory<T, R, W, D, 1, 1>` — the
/// same text, byte for byte. Before D-25 the transpiler refused the short spelling
/// ("must spell all six generics") while the macro admitted it (`review/MEASUREMENTS.md`
/// §W.1 #13); the parity case is `ui:pass/memory_parameter_default_latencies`.
#[test]
fn a_received_memory_with_default_latencies_lowers_as_latency_one() {
    let child = |ty: &str| {
        format!(
            r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, mem: {ty}, a: In<Bits<4>, MainClk>, d: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {{
    loop {{
        mem.write_port::<0>().write(a.read().as_usize(), d.read());
        mem.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        if mem.read_port::<0>().is_ready() {{ q.write(mem.read_port::<0>().data()); }}
    }}
}}
"#
        )
    };
    let short = transpile(&child("Memory<Bits<8>, 1, 1, MainClk>")).expect("the defaulted spelling transpiles");
    let read_only = transpile(&child("Memory<Bits<8>, 1, 1, MainClk, 1>")).expect("a defaulted WRITE_LAT transpiles");
    let full = transpile(&child("Memory<Bits<8>, 1, 1, MainClk, 1, 1>")).expect("the full spelling transpiles");
    assert_eq!(short, full);
    assert_eq!(read_only, full);
}

/// Multi-phase memory DOES transpile: the address buses are phase-gated and the
/// read result is captured into a pipeline register that survives the edge.
/// `tests/multiphase_memory_equivalence.rs` carries the behavioural checks.
#[test]
fn memory_in_a_multi_phase_loop_is_supported() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<16>, MainClk>) {
    let mem = Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::new(clk.clone(), 256);
    let mut q: Bits<16> = Bits::zero();
    loop {
        o.write(q);
        mem.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        if mem.read_port::<0>().is_ready() { q = mem.read_port::<0>().data(); }
        clk.tick().await;
    }
}
"#;
    let sv = transpile(src).expect("a two-phase memory read must transpile");
    assert!(
        sv.contains("mem_rd0_q0 <= mem_rd0_data;"),
        "a cross-phase read result must be captured into a pipeline register, got:\n{sv}"
    );
}

/// Combinational statements after the LAST tick of a multi-tick loop **lower**, into
/// phase 0 — decided 2026-08-25 from `design_docs/SYNCHRONOUS_SEMANTICS.md`, which
/// says a clock cycle is *"a maximal tick-free region of an execution"*. The trailing
/// statements and the head's are one such region, because falling off the end of the
/// body and re-entering it costs no clock, so they are the same cycle.
///
/// They used to be refused here — "an output written there would be silently dropped"
/// — while the single-tick path hoisted them and `control_extract`'s path accepted
/// them and agreed with its SystemVerilog (`uart/rx`'s trailing `rx_dv.write(Zero)`).
/// The same source was therefore accepted or rejected depending on whether an
/// unrelated part of the module triggered extraction.
///
/// The behavioural half is `tests/fixtures/trailing_statements_dut.rs`, swept
/// differentially. Since Phase C step 4 (2026-11-06) the segment is the next
/// resumption's head: it lowers into phase 0 under the `__copper_started` guard
/// (the first resumption runs the head only), so `t`'s drive is guarded.
#[test]
fn trailing_combinational_statements_in_a_multi_tick_loop_lower_into_phase_zero() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>,
           t: RegOut<Bits<8>, MainClk>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = a.read();
        clk.tick().await;
        o.write(r);
        clk.tick().await;
        t.write(r);
    }
}
"#;
    let sv = transpile(src).expect(
        "trailing combinational statements must lower — they are in the head's cycle, \
         which is what a maximal tick-free region means",
    );
    assert!(
        sv.contains("if (__copper_started) begin\n                t <= r;"),
        "the trailing `RegOut` drive must reach the emitted module, not be dropped:\n{sv}"
    );
}

/// The read result observed on the wrong side of the tick. `data()` is what the
/// clock edge produced, so reading it before the edge would shift the design by a
/// cycle — silently, since the value is well-typed either way.
#[test]
fn memory_read_result_before_the_tick_is_rejected() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<16>, MainClk>) {
    let mem = Memory::<Bits<16>, 1, 1, MainClk, 1, 1>::new(clk.clone(), 256);
    let mut q: Bits<16> = Bits::zero();
    loop {
        mem.read_port::<0>().read(a.read().as_usize());
        q = mem.read_port::<0>().data();
        o.write(q);
        clk.tick().await;
    }
}
"#;
    let err = transpile(src).expect_err("observing a read result before the edge must be rejected");
    assert!(
        err.contains("is read before the `clk.tick().await` that produces it"),
        "reproduced a *different* error than the read-ordering rule: {err}"
    );
}

/// A memory in a module that **control extraction rewrites**. Until 2026-08-25
/// this was refused: the staging rules were decided over the lowered body's
/// tick-delimited segments, and extraction folds a branch- or loop-nested tick
/// into a single-tick `match pc` FSM, so every access landed in one segment and
/// an ordinary ROM read looked like a same-cycle one. The rules now run on the
/// source (`copper_analysis::check_memory_staging`). `tests/extracted_memory_equivalence.rs`
/// carries the behavioural sim ≡ transpiled-SV check for both extraction triggers.
#[test]
fn memory_in_an_extracted_module_is_supported() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<16>, MainClk>) {
    let rom = Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::new(clk.clone(), 256);
    loop {
        rom.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        o.write(rom.read_port::<0>().data());
        for _ in 0..2 { clk.tick().await; }
    }
}
"#;
    let sv = transpile(src).expect("a memory design that needs extraction must transpile");
    assert!(
        sv.contains("case (pc)") && sv.contains("rom_rd0_q0 <= rom_rd0_data;"),
        "expected an extracted FSM with a read pipeline stage, got:\n{sv}"
    );
}

/// …and the rule still bites there. This is the direction that matters: the check
/// has to be able to REJECT inside an extracted module, which the segment-based
/// one could not do at all once the ticks had become `pc` states.
#[test]
fn a_same_cycle_read_result_in_an_extracted_module_is_still_rejected() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<16>, MainClk>) {
    let rom = Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::new(clk.clone(), 256);
    loop {
        rom.read_port::<0>().read(a.read().as_usize());
        o.write(rom.read_port::<0>().data());
        clk.tick().await;
        for _ in 0..2 { clk.tick().await; }
    }
}
"#;
    let err = transpile(src).expect_err(
        "NOW ACCEPTED: an extracted module observing a read result in its staging cycle. That is \
         a cycle-early read; check what the emitted FSM does with it.",
    );
    assert!(
        err.contains("is read before the `clk.tick().await` that produces it"),
        "reproduced a *different* error than the read-ordering rule: {err}"
    );
}

/// Same, in a module that needs control extraction: the double staging transpiles
/// there too (D-25; refused before as "accessed 2 times in one cycle").
#[test]
fn two_stagings_in_one_resumption_of_an_extracted_module_transpile() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<16>, MainClk>) {
    let rom = Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::new(clk.clone(), 256);
    loop {
        rom.read_port::<0>().read(a.read().as_usize());
        rom.read_port::<0>().read(0);
        clk.tick().await;
        o.write(rom.read_port::<0>().data());
        for _ in 0..2 { clk.tick().await; }
    }
}
"#;
    transpile(src).expect("two stagings of one port in one resumption transpile (I.10.1(b))");
}

// ── Counted repetition (`TODO` cause M) ───────────────────────────────────────

/// A tick inside a counted `for` is now COUNTED REPETITION: `control_extract`
/// desugars it into a counter-driven `loop`. These pin the boundary of what the
/// desugar accepts, and — the half cause M was really about — that a module
/// declined for one construct is not reported against another.
#[test]
fn a_tick_inside_a_counted_for_is_supported() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, n: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    loop {
        o.write(n.read());
        for _ in 0..3 {
            clk.tick().await;
        }
    }
}
"#;
    let sv = transpile(src).expect("a counted delay must transpile");
    assert!(
        sv.contains("__copper_ctr0"),
        "the delay should become a counter register, not unroll: {sv}"
    );
    // 434 states for a UART bit period is the failure mode this must not have.
    assert!(
        sv.matches("8'd").count() < 40,
        "the counted delay looks unrolled rather than counted: {sv}"
    );
}

/// A statically empty range is zero cycles and zero work, so the loop disappears
/// entirely. It cannot merely break at once: the desugar hoists the final tick out
/// of the loop, and that tick would then be a cycle the source does not have.
#[test]
fn a_statically_empty_counted_range_costs_nothing() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, n: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    loop {
        o.write(n.read());
        for _ in 0..0 {
            clk.tick().await;
        }
        clk.tick().await;
    }
}
"#;
    let sv = transpile(src).expect("an empty counted range must transpile");
    assert!(
        !sv.contains("__copper_ctr") && !sv.contains("pc"),
        "an empty range should leave no counter and no FSM behind: {sv}"
    );
}

/// The tick has to be the body's LAST statement, for the same reason it does in a
/// hand-written `loop` — and the message must say so rather than fall through to
/// the repeating-wait advice, which does not apply to a `for`.
#[test]
fn a_counted_for_whose_tick_is_not_last_is_refused() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, n: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    loop {
        for _ in 0..3 {
            clk.tick().await;
            o.write(n.read());
        }
    }
}
"#;
    let err = transpile(src).expect_err("a `for` whose tick is not last must be refused");
    assert!(
        err.contains("`for`") && err.contains("LAST"),
        "the diagnostic must name the `for` and its ordering rule: {err}"
    );
}

/// The half of cause M that was not about lowering at all. A module declined for
/// one construct used to be reported against another: the linear path downstream
/// blames the first unsupported thing it REACHES, which left `uart/rx`'s
/// well-formed repeating wait accused of having its tick in the wrong place.
///
/// The DUT below is `uart/rx`'s shape as it stands today — a correct `while` wait
/// followed by a `for` whose tick is not its last statement. The `for` is what
/// declines the module, and the `for` is what must be named. (The original
/// instance was a `continue`, which is supported as of cause O; the mechanism
/// under test is the same.)
#[test]
fn a_module_declined_for_one_construct_is_not_reported_against_another() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, go: In<Logic, MainClk>, n: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    loop {
        while go.read() == Logic::Zero {
            clk.tick().await;
        }
        for _ in 0..4 {
            clk.tick().await;
            o.write(n.read());
        }
    }
}
"#;
    let err = transpile(src).expect_err("a `for` whose tick is not last must be refused");
    assert!(
        err.contains("`for`"),
        "the reported construct must be the `for`, not the well-formed wait above \
         it: {err}"
    );
    assert!(
        !err.contains("repeating wait"),
        "the well-formed `while` must not be blamed for the `for`: {err}"
    );
}

// ── `continue` (`TODO` cause O) ───────────────────────────────────────────────

/// A `continue` in the module's own loop is a ZERO-TIME back edge, lowered as a
/// goto marker and spliced with the head state's body once every body exists.
/// Nothing of the marker may survive into the emitted SystemVerilog.
#[test]
fn a_continue_in_the_module_loop_is_supported() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, halt: In<Logic, MainClk>, n: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    loop {
        for _ in 0..2 {
            clk.tick().await;
        }
        if halt.read() == Logic::One {
            continue;
        }
        o.write(n.read());
        clk.tick().await;
    }
}
"#;
    let sv = transpile(src).expect("a top-level `continue` must transpile");
    assert!(
        !sv.contains("__copper_goto"),
        "a zero-time goto marker leaked into the output — it is not a real signal: {sv}"
    );
}

/// …and it must still be REJECTED when the path that reaches it never ticks: the
/// back edge then returns to the head in zero time. Owned by the shared
/// reachability guarantee, which the transpiler re-runs, so the sim macro rejects
/// it at `cargo build` too. This is the invariant that had to hold before codegen
/// could emit a `continue` at all — see `copper-analysis`' own tests.
#[test]
fn a_continue_that_skips_every_tick_is_still_rejected() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, halt: In<Logic, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    loop {
        if halt.read() == Logic::One {
            continue;
        }
        o.write(Bits::zero());
        clk.tick().await;
    }
}
"#;
    let err = transpile(src).expect_err("a tickless `continue` cycle must be rejected");
    assert!(
        !err.is_empty() && !err.starts_with("0:0:"),
        "the rejection must carry a span: {err}"
    );
}

/// A `continue` inside a NESTED loop was refused outright until Phase C step 5
/// (II.3 (a), 2026-11-06): that loop's head state holds the rotated `C ; W`. It is
/// now lowered (a zero-time goto to the head when `C` is empty, the loop's entry
/// copy otherwise; swept by `tests/fixtures/nested_continue_dut.rs`). THIS module
/// stays refused for another reason: its nested loop has no tick at all, so the
/// `continue` path is a zero-time loop — the nested-loop shape rule's message.
#[test]
fn a_tickless_nested_loop_with_a_continue_is_refused_by_the_shape_rule() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<MainClk>, go: In<Logic, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        o.write(n);
        loop {
            if go.read() == Logic::Zero { continue; }
            break;
        }
        n = n + Bits::from_lit::<1>();
        clk.tick().await;
    }
}
"#;
    let err = transpile(src).expect_err("a tick-less nested loop must be refused");
    assert!(
        err.contains("would never terminate") && !err.starts_with("0:0:"),
        "the shape rule's diagnostic, with a span: {err}"
    );
}

/// SPEC II.1 R8 (D-8, Phase C step 3): the transpiler refuses a plain `Out` on a
/// clocked module with the macro's message (`copper_analysis::clocked_out_message`;
/// the macro's witnesses are `copper-macros/tests/ui/fail/plain_out_on_*.rs`).
#[test]
fn a_plain_out_on_a_clocked_module_is_refused_by_r8() {
    for (attr, name) in [("sequential", "pass_seq"), ("synchronizer", "pass_sync")] {
        let src = format!(
            r#"
#[hardware({attr})]
async fn {name}(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {{
    loop {{
        o.write(a.read());
        clk.tick().await;
    }}
}}
"#
        );
        let err = transpile(&src).expect_err("R8: a plain Out on a clocked module must be refused");
        assert!(
            err.contains(&copper_analysis::clocked_out_message("o")),
            "{attr}: expected R8's shared message, got: {err}"
        );
    }
}

/// A counted `for` whose range may be empty at run time, as the LAST statement of an
/// enclosing tick-bearing `for` (Phase C step 5, MEASUREMENTS §J.2). Since the hoisted
/// tick of a possibly-empty range is guarded (II.3 (f1)), the enclosing body ends in a
/// conditional tick and has no clock boundary to hoist; with an empty inner range the
/// enclosing loop's iterations take zero time (SPEC I.8.4(b)), which a one-state-per-
/// tick FSM cannot run. Refused by the transpiler only — the macro admits (rustc runs
/// it) — so this is a REFUSED-one limitation for the coordinator's II.3. Before the
/// guard it was admitted and divergent whenever the inner range was empty. The same
/// shape with a constant inner bound (`0..CLKS_PER_BIT`, the UART) transpiles.
const NESTED_RUNTIME_EMPTY_FOR: &str = r#"
#[hardware(sequential)]
async fn nested_for(clk: Clock<MainClk>, c: In<Bits<2>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        let cnt: u32 = c.read().as_u32();
        for _ in 0..2 {
            n = n + Bits::from_u8(1);
            for _ in 0..cnt {
                q.write(n);
                clk.tick().await;
            }
        }
        q.write(Bits::from_u8(255));
        clk.tick().await;
    }
}
"#;

#[test]
fn a_runtime_empty_counted_for_ending_an_enclosing_for_has_its_own_diagnostic() {
    let err = transpile(NESTED_RUNTIME_EMPTY_FOR)
        .expect_err("NOW SUPPORTED: the nested run-time-empty counted `for` transpiles — sweep it");
    // Since 2026-11-07 a shared rule refuses it in BOTH front-ends with this message
    // (`copper_analysis::check_counted_for_nesting`; the macro's witness is
    // `copper-macros/tests/ui/fail/nested_runtime_empty_for.rs`, MEASUREMENTS §J.15).
    assert!(err.contains(copper_analysis::NESTED_RUNTIME_EMPTY_FOR_MESSAGE), "not the shared message: {err}");
    // A const-generic inner bound may be 0 at an instance: refused too.
    let generic = NESTED_RUNTIME_EMPTY_FOR
        .replace("async fn nested_for(", "async fn nested_for<const N: usize>(")
        .replace("0..cnt", "0..N");
    let err = transpile(&generic).expect_err("a const-generic inner bound is refused");
    assert!(err.contains(copper_analysis::NESTED_RUNTIME_EMPTY_FOR_MESSAGE), "{err}");
    let constant = NESTED_RUNTIME_EMPTY_FOR
        .replace("let cnt: u32 = c.read().as_u32();", "")
        .replace("0..cnt", "0..CNT");
    let constant = format!("const CNT: usize = 0x3;\n{constant}");
    transpile(&constant).expect("a constant inner bound is statically non-empty and transpiles");
}

/// SPEC II.3.3 (h) (D-23 (2), Phase C step 11b): a `continue` of a tick-bearing `for`
/// means increment-then-test (I.8.5), which the counted-`for` lowering does not
/// realize. Until 2026-11-08 the transpiler refused it alone (MEASUREMENTS §R.3);
/// `copper_analysis::front_end::check_ticking_for_continue` now refuses it in both
/// front-ends with one message (the macro's witness is
/// `copper-macros/tests/ui/fail/ticking_for_continue.rs`). A `continue` of a loop
/// NESTED in the `for` targets that loop (row (a), realized) and stays admitted.
#[test]
fn a_continue_of_a_tick_bearing_for_is_refused_with_the_shared_message() {
    let src = r#"
#[hardware(sequential)]
async fn cf(clk: Clock<MainClk>, a: In<Logic, MainClk>, o: RegOut<Logic, MainClk>) {
    loop {
        for _i in 0..4 {
            if a.read() == Logic::One { clk.tick().await; continue; }
            o.write(Logic::Zero);
            clk.tick().await;
        }
    }
}
"#;
    let err = transpile(src).expect_err("NOW SUPPORTED: a `continue` of a tick-bearing `for` transpiles — sweep it");
    assert!(err.contains(copper_analysis::front_end::TICKING_FOR_CONTINUE_MESSAGE), "not the shared message: {err}");
    let nested = r#"
#[hardware(sequential)]
async fn cfn(clk: Clock<MainClk>, a: In<Logic, MainClk>, o: RegOut<Logic, MainClk>) {
    loop {
        for _i in 0..4 {
            loop {
                let v = a.read();
                if v == Logic::Zero { break; }
                if v == Logic::One { clk.tick().await; continue; }
                clk.tick().await;
            }
            o.write(Logic::Zero);
            clk.tick().await;
        }
    }
}
"#;
    transpile(nested).expect("a `continue` of a loop nested in the `for` is the nested loop's, and admitted");
}

/// An element write to an array register inside a combinational `for`: one write site
/// run once per iteration, all in one cycle. Measured 2026-11-06 (Phase C step 5,
/// MEASUREMENTS §J.6) admitted and divergent — the site's one write port had its nets
/// reassigned per iteration, so only the last iteration's write committed — then refused,
/// transpiler only (line `II.3 found array-for`). Since 2026-11-08 (MEASUREMENTS §U.4) the
/// `for` is unrolled when its bounds fold to constants, each iteration's write its own port
/// (II.3 (c)'s form; swept by `tests/fixtures/array_for_write_dut.rs`). A bound the fold cannot
/// see — a const generic, whose value is the instance's — keeps the refusal: the emitted SV is
/// one parametric module, and an unrolling needs the trip count.
#[test]
fn an_array_register_element_write_inside_a_combinational_for_is_unrolled_unless_the_bound_is_generic() {
    let src = r#"
#[hardware(sequential)]
async fn arr_for_writes(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, k: In<Bits<2>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut arr = [Bits::<8>::zero(); 4];
    loop {
        q.write(arr[k.read().as_usize()]);
        for j in 0..4 {
            arr[j] = a.read() + arr[j];
        }
        clk.tick().await;
    }
}
"#;
    let sv = transpile(src).expect("an element write in a constant-bound comb `for` unrolls");
    assert!(sv.contains("arr_wr3_en"), "four write ports, one per iteration:\n{sv}");
    let generic = r#"
#[hardware(sequential)]
async fn arr_for_generic<const N: usize>(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut arr = [Bits::<8>::zero(); 4];
    loop {
        q.write(arr[0]);
        for j in 0..N {
            arr[j] = a.read() + arr[j];
        }
        clk.tick().await;
    }
}
"#;
    let err = transpile(generic).expect_err("a generic bound has no trip count to unroll");
    assert!(err.contains("inside a combinational `for`"), "a different error: {err}");
}

/// SPEC II.1.2 R21 (Phase C step 5, 2026-11-07): a `tick` before the module's `loop`
/// is refused by the transpiler with the macro's message
/// (`copper_analysis::PRELOOP_TICK_MESSAGE`; the macro's witness is
/// `copper-macros/tests/ui/fail/preloop_tick.rs`), on both routes — the linear route
/// dropped the tick until then (II.3 (f6)).
#[test]
fn a_tick_before_the_module_loop_is_refused_by_r21_on_both_routes() {
    let linear = r#"
#[hardware(sequential)]
async fn r21_linear(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    clk.tick().await;
    loop {
        q.write(a.read() + Bits::from_lit::<1>());
        clk.tick().await;
    }
}
"#;
    let extracted = r#"
#[hardware(sequential)]
async fn r21_extracted(clk: Clock<MainClk>, go: In<Logic, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: Bits<8> = Bits::zero();
    clk.tick().await;
    loop {
        if go.read() == Logic::One { clk.tick().await; n = n + Bits::from_lit::<1>(); }
        q.write(n);
        clk.tick().await;
    }
}
"#;
    for src in [linear, extracted] {
        let err = transpile(src).expect_err("a pre-loop tick must be refused (R21)");
        assert!(err.contains(copper_analysis::PRELOOP_TICK_MESSAGE), "not R21's message: {err}");
    }
}


// ── Phase C step 8 (SPEC I.14.7; MEASUREMENTS §M) ─────────────────────────────

/// `checked_*` and `is_valid()` are OUTSIDE (I.14.3(c), (d)): the transpiler refuses
/// them with the macro's message (`copper_analysis::check_outside_methods`; the
/// macro's witnesses are `copper-macros/tests/ui/fail/outside_{checked_add,is_valid}.rs`).
#[test]
fn checked_arithmetic_and_is_valid_are_outside_in_both_front_ends() {
    let checked = r#"
#[hardware(sequential)]
async fn m(clk: Clock<C>, a: In<u8, C>, q: RegOut<u8, C>) {
    loop {
        q.write(a.read().checked_add(1).unwrap_or(0));
        clk.tick().await;
    }
}
"#;
    let err = transpile(checked).expect_err("checked_add is OUTSIDE");
    assert!(err.contains(copper_analysis::CHECKED_METHOD_MESSAGE), "{err}");
    let valid = r#"
#[hardware(combinational)]
fn m(a: In<Bits<8>, C>, q: Out<Logic, C>) {
    q.write(if a.read().is_valid() { Logic::One } else { Logic::Zero });
}
"#;
    let err = transpile(valid).expect_err("is_valid is OUTSIDE");
    assert!(err.contains(copper_analysis::IS_VALID_MESSAGE), "{err}");
}

/// A memory port index at or above the port count (I.14.6(c), D-19 6b-A): refused by
/// the shared rule, the macro's message (witness `copper-macros/tests/ui/fail/
/// memory_port_index.rs`); its in-range twin transpiles.
#[test]
fn a_memory_port_index_beyond_the_port_count_is_refused_in_both_front_ends() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<C>, a: In<Bits<2>, C>, q: RegOut<Bits<8>, C>) {
    let mem = Memory::<Bits<8>, 1, 1, C, 1, 1>::new(clk.clone(), 4);
    loop {
        if mem.read_port::<0>().is_ready() {
            q.write(mem.read_port::<0>().data());
        }
        mem.read_port::<K>().read(a.read().as_usize());
        clk.tick().await;
    }
}
"#;
    let err = transpile(&src.replace("<K>", "<1>")).expect_err("read_port::<1> of a 1-port memory");
    assert!(err.contains(&copper_analysis::memory_port_index_message("mem", "read_port", 1, 1)), "{err}");
    transpile(&src.replace("<K>", "<0>")).expect("read_port::<0> transpiles");
}

/// `Bits::x()` / `Logic::X` as a VALUE emit the type's X literal (I.14.7 (6), D-19's
/// `xw_port`); a comparison with one asks whether a value is X — the `is_valid`
/// class — and is OUTSIDE, refused in both front-ends with one message since Phase C
/// step 11b (D-23 (3); until then the transpiler only, REFUSED-one, §M).
#[test]
fn the_x_literal_is_emitted_as_a_value_and_refused_in_a_comparison() {
    let src = r#"
#[hardware(sequential)]
async fn m(clk: Clock<C>, c: In<Logic, C>, a: In<Bits<8>, C>, q: RegOut<Bits<8>, C>, ql: RegOut<Logic, C>) {
    loop {
        if c.read() == Logic::One { q.write(Bits::x()); } else { q.write(a.read()); }
        ql.write(Logic::X);
        clk.tick().await;
    }
}
"#;
    let sv = transpile(src).expect("an X write transpiles");
    assert!(sv.contains("q <= 'x;") && sv.contains("ql <= 1'bx;"), "{sv}");
    let cmp = r#"
#[hardware(combinational)]
fn m(a: In<Logic, C>, q: Out<Logic, C>) {
    q.write(if a.read() == Logic::X { Logic::One } else { Logic::Zero });
}
"#;
    let err = transpile(cmp).expect_err("a comparison with X is refused");
    assert!(err.contains(copper_analysis::X_COMPARISON_MESSAGE), "{err}");
}

/// A pattern matching the X literal is the `is_valid` question spelled as a pattern
/// (SPEC I.14.3(d)–(e), D-24 (2), Phase C step 11c): OUTSIDE, refused in both
/// front-ends by `check_outside_methods` with one message, in every pattern position
/// — a `match` arm, an or-pattern, `if let`, a tuple sub-pattern, `matches!` — and as
/// an identifier pattern naming a body `const` whose value is X. Until then the macro
/// admitted it and the transpiler refused it ("unsupported pattern"). The twin, a
/// `Logic::One` arm, transpiles.
#[test]
fn a_pattern_matching_the_x_literal_is_refused_with_the_shared_message() {
    let comb = |body: &str, ins: &str| {
        format!("#[hardware(combinational)]\nfn m({ins} q: Out<Logic, C>) {{ {body} }}\n")
    };
    let a = "a: In<Logic, C>,";
    let ab = "a: In<Logic, C>, b: In<Logic, C>,";
    for (what, body, ins) in [
        ("arm", "q.write(match a.read() { Logic::X => Logic::One, _ => Logic::Zero });", a),
        ("or-pattern", "q.write(match a.read() { Logic::X | Logic::One => Logic::One, _ => Logic::Zero });", a),
        ("tuple", "q.write(match (a.read(), b.read()) { (Logic::X, _) => Logic::One, _ => Logic::Zero });", ab),
        ("matches!", "q.write(if matches!(a.read(), Logic::X) { Logic::One } else { Logic::Zero });", a),
        ("body const", "const U: Logic = Logic::X; q.write(match a.read() { U => Logic::One, _ => Logic::Zero });", a),
    ] {
        let err = transpile(&comb(body, ins)).expect_err(what);
        assert!(err.contains(copper_analysis::X_PATTERN_MESSAGE), "{what}: {err}");
    }
    let iflet = r#"
#[hardware(sequential)]
async fn m(clk: Clock<C>, a: In<Logic, C>, q: RegOut<Logic, C>) {
    loop {
        let v = a.read();
        if let Logic::X = v { q.write(Logic::One); }
        clk.tick().await;
    }
}
"#;
    let err = transpile(iflet).expect_err("if let");
    assert!(err.contains(copper_analysis::X_PATTERN_MESSAGE), "if let: {err}");
    transpile(&comb("q.write(match a.read() { Logic::One => Logic::Zero, _ => Logic::One });", a)).expect("the twin transpiles");
}

/// SPEC I.2.4, line `II.3 found tickfree-continue` (MEASUREMENTS §U.1, 2026-11-08): a `continue`
/// of a tick-free `for` is lowered by placing the rest of the iteration under the negated
/// condition (`copper_codegen::for_continue`; swept by `tests/fixtures/tickfree_continue_dut.rs`).
/// The rewrite copies the rest of the iteration into each branch, so it declines when that rest
/// names a binding made inside the branch (the copy would capture it); the transpiler then
/// refuses with a message about the `for`, not the module loop (the message before §U.1).
#[test]
fn a_tickfree_for_continue_whose_rest_names_a_branch_binding_is_refused_with_its_own_message() {
    let lowered = r#"
#[hardware(combinational)]
fn tfc_ok<D: ClockDomain>(a: In<Bits<4>, D>, o: Out<Bits<4>, D>) {
    let av = a.read();
    let mut acc: Bits<4> = Bits::zero();
    for i in 0..4 {
        if av[i] == Logic::One { continue; }
        acc = acc + Bits::from_lit::<1>();
    }
    o.write(acc);
}
"#;
    let shadowed = r#"
#[hardware(combinational)]
fn tfc_shadow<D: ClockDomain>(a: In<Bits<4>, D>, o: Out<Bits<4>, D>) {
    let av = a.read();
    let t: Bits<4> = Bits::from_lit::<1>();
    let mut acc: Bits<4> = Bits::zero();
    for i in 0..4 {
        if av[i] == Logic::One {
            let t: Bits<4> = Bits::from_lit::<2>();
            acc = acc ^ t;
            if acc[0] == Logic::One { continue; }
        }
        acc = acc + t;
    }
    o.write(acc);
}
"#;
    let sv = transpile(lowered).expect("a tick-free `for`'s `continue` lowers");
    assert!(!sv.contains("continue"), "no `continue` reaches the SV:\n{sv}");
    let err = transpile(shadowed).expect_err("the shadowing rest is not rewritten");
    assert!(err.contains("this `continue` of a tick-free `for` is not lowered"), "a different error: {err}");
}

/// Line `II.3 found const-pattern` (MEASUREMENTS §T.3, §U.2, 2026-11-08): a pattern naming a
/// non-integer `const` is lowered by the const's value (swept by
/// `tests/fixtures/const_pattern_dut.rs`). A FILE const whose value is X is OUTSIDE (D-24,
/// SPEC I.14.3(d)): the shared rule sees only the `ItemFn` (a body `const`'s X literal), so the
/// transpiler refuses the file spelling here, with the same `X_PATTERN_MESSAGE`, once it folds
/// the value. Before §U.2 it refused all four spellings with "tuple-pattern match lowering is not
/// yet implemented (M2)", X or not.
#[test]
fn a_pattern_naming_a_file_const_whose_value_is_x_is_refused_as_outside() {
    for (name, decl, ty) in [
        ("UX", "const UX: Logic = Logic::X;", "Logic"),
        ("UB", "const UB: Bits<2> = Bits::x();", "Bits<2>"),
    ] {
        let src = format!(
            r#"
{decl}
#[hardware(combinational)]
fn xp<D: ClockDomain>(a: In<{ty}, D>, o: Out<Bits<2>, D>) {{
    match a.read() {{ {name} => o.write(Bits::from_lit::<1>()), _ => o.write(Bits::zero()) }}
}}
"#
        );
        let err = transpile(&src).expect_err("an X-valued const pattern is OUTSIDE");
        assert!(err.contains(copper_analysis::X_PATTERN_MESSAGE), "{name}: a different error: {err}");
    }
}

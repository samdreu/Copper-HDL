# Synchronous semantics

The reference for Copper's execution/timing model — **rebased 2026-08-27 onto the
cycle-dataflow denotation** (`CYCLE_DATAFLOW_SEMANTICS.md`), which is *normative*:
it defines one value per signal per cycle, and the simulator and transpiler are
both implementations with stated obligations. This document states that
denotation first and the machinery that realizes it second; the refusal rules
appear as the **derived residue** — shapes no implementation can realize — rather
than as the definition. Every claim is grounded in the code cited inline, the
per-module derivation record (`DERIVATION_TABLE.md`), and the test suite; the
semantics is independent of any particular construction (codegen's `match pc`
FSM is one realization, not the semantics). Companions:
`SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md` (the staged work),
`PAIRED_IMPLEMENTATION_SCOPE.md` (the migration's execution record).

## Execution model — c2 + "just Rust"

**The default simulator always executes plain Rust.** A `#[hardware]` async fn is run as-is by the
async executor (`copper-sim`); rustc's own coroutine lowering *is* the FSM. A shared compile-time
control/liveness analysis (`copper-analysis`, the CFG below) *informs* the sim — it supplies register
and read-timing facts the macro bakes into the generated code — but the sim never *interprets* an IR.
This is load-bearing: sim and transpiler are then two independent derivations from one source, which
is what makes their same-source equivalence non-circular (threat T6 in the paper's
threats-to-validity discussion) and is LEAD contribution #1. A CHIR interpreter may exist only as
an optional **validation-only backend, never the default** (impl-plan item 7; not built as of
2026-09-01). This "b vs c2" decision was settled 2026-07-29 against the alternative of making the
sim consume an FSM IR.

## The denotation — cycle dataflow (normative)

Fix a clock domain; edges are numbered `1, 2, …` and **cycle N** is `[edge N,
edge N+1)`. The observation instant of cycle N is just after edge N's post-edge
settle (the harness convention: drive, tick, observe). Every signal has **one
value per cycle**, and the whole sim-vs-synthesis question is governed by one
invariant:

> **The write-window invariant.** A simulator port write executed at instant `t`
> is observed for the window `[t, next write)`. The emitted hardware for that
> port is a continuous function of registers and inputs. The two agree iff, at
> every observation instant inside the write's intended window, the written
> value equals that function evaluated at the instant's wire values.

A **segment** is a maximal tick-free region (trailing statements merge with the
head — see below). Within a segment, statements mean what plain Rust means:
**program order with forwarding** — a local read yields the last value assigned
in the segment, else the value committed at the opening edge. The segment
divides into **regions at the read-site barrier**: an *opening prefix*
(statements before the first leading `In` read) executes at the cycle's opening
instant on committed state; a *closing suffix* executes at the pre-edge settle
on closing-edge samples and forwarded values. That the barrier site *is* the
region boundary is a validated fact, not an accident — a region-entry barrier
was proposed and falsified by measurement (V8c; `PAIRED_IMPLEMENTATION_SCOPE.md`
§0).

**Commits** — register locals (the liveness rule), `RegOut` writes, memory
stagings, and the implicit state register a control-steering read feeds — anchor
to the segment's **closing edge**; a *trailing* region's commits anchor to the
edge that **opens** its cycle. **Reads are defined by value, not instant**: an
`In` read denotes the settled input value of the window its consumer belongs to
(`Deferred`/`Immediate` below are the implementation). **Emission obligations**
follow: an opening-prefix plain-`Out` drive's continuous assign is the
**forwarded** expression (landed 2026-08-26, commit `7abfd98`; the claim that this
changed no live module's emitted SystemVerilog was measured once with the
`sv-baseline` tool at that commit and is not a regression gate — see *How these
semantics are checked*); a closing-suffix drive's is the unforwarded
**commit-frontier** form, legal exactly when its value is frontier-expressible — a
committed or committing register, or an every-path constant.

The model was validated before it was trusted: every corpus module carries a
derived disposition, and every derivation that predicted a measurable outcome
was measured — including one silent divergence predicted with exact traces
before its controlled measurement existed (`DERIVATION_TABLE.md`, F2/m1–m4).

## The clock tick — phases

`HardwareExecutor::tick_clock` (`copper-sim/src/executor.rs`) drives one clock cycle through four
phases:

1. **Pre-edge settle** — phase `PreEdge`; `HardwareExecutor::poll_tasks` settles the combinational
   logic (levelized by default, iterate-to-fixpoint under `SchedulerMode::Fixpoint` — see *Poll-order
   … independence*) with the registers still holding the *previous* cycle's values. Pre-edge
   (leading) input reads sample here.
2. **Clock edge** — `Clock::advance` (`copper-core/src/types.rs`); registers commit their next-state
   values.
3. **Post-edge settle** — phase `PostEdge`; `poll_tasks` settles again with the new register values.
4. **Post-edge observation** — after `tick_clock` returns, a testbench reads the settled post-edge
   values.

**Post-edge continuation convention.** A `clk.tick()` future resolves in the *post-edge* settle, so a
reaction's post-tick code runs in the **same** `tick_clock`, after the advance. A register clocked at
edge N is therefore observable in cycle N — the standard synchronous-testbench convention — so
Copper's primitives (flip-flop `q <= d`, enabled register, synchronous-read RAM) match hand-written
Verilog cycle-for-cycle. This is validated against independent BaseJump STL hardware
(`examples/basejump/`, each example's `main` is the check; `tests/sipo_block_equivalence.rs`,
`tests/bsg_counter_up_down_equivalence.rs` and their siblings run the same comparisons under
`cargo test`), not against the transpiler.

**Per-domain phase keying.** The phase, tick-resolution, and read-timing signals are keyed *per clock
domain* (`set_poll_phase::<Domain>` / `PollPhase` in `copper-sim/src/lib.rs`,
`set_tick_resolving::<Domain>` / `TICK_RESOLVING` in `copper-core/src/types.rs`; impl-plan item 1).
Ticking one clock cannot perturb another domain's tasks — the prerequisite for the multi-clock
interleave independence below. Pinned by `tests/poll_order_fuzz.rs::multi_domain_counters_are_poll_order_independent`
and `tests/levelized_differential.rs::diff_multi_domain_independent`.

## Cycle boundaries and FSM states

Each `clk.tick().await` is a **clock-cycle boundary**; a **clock cycle** is a maximal tick-free region
of an execution. A **suspension point** (a program point at which a coroutine can be paused across a
boundary) is an **FSM state**. **Every value live across a boundary is a register** (made precise by
the liveness rule under *The CFG model*). These are the four informal invariants — boundary,
FSM-state, register, and *every loop path reaches a tick* (reachability well-formedness, below) — now
each a checked property rather than an accident of construction.

### Trailing statements — the segment after the last tick

Decided 2026-08-25 (commit `062724f`); it follows from the definition above rather than adding to
it. The lowering is `shir_lower::lower_seq_body` (`copper-codegen/src/shir_lower.rs`); the
differential fixture is `tests/fixtures/trailing_statements_dut.rs` (in the corpus sweep), and the
shape pin is `copper-codegen/tests/unsupported_constructs.rs::trailing_combinational_statements_in_a_multi_tick_loop_lower_into_phase_zero`.
In

```text
loop { A; clk.tick().await; B; clk.tick().await; C; }
```

`C` and the *next* iteration's `A` sit in one maximal tick-free region — falling off
the end of the body and re-entering it costs no clock — so **`C` is in the head's
cycle**. Three consequences, all now implemented on every lowering path:

* **`C`'s combinational logic lowers into phase 0**, the head's phase. The single-tick
  path always did this (there the trailing segment *is* the one phase) and
  `control_extract`'s path accepts it (`examples/uart/rx.rs`'s trailing `rx_dv.write(Zero)`,
  sim ≡ SV, `tests/uart_rx_equivalence.rs`); the multi-tick path used to **refuse** it, so the
  same source was accepted or rejected depending on whether an unrelated part of the module
  triggered extraction.
* **`C` is emitted before `A`**, matching execution order: the simulator runs `C` in
  the post-edge settle of the tick that opens the cycle and `A` in the pre-edge settle
  of the tick that closes it. Unobservable through ports — `multi_write_collapse`
  rejects a port written on both sides of a tick — but observable for a plain local
  flowing from `C` into `A`.
* **`C`'s register-local updates commit at the last tick's edge**, which is the edge that
  *opens* `C`'s cycle. That is the post-edge continuation convention above ("a register
  clocked at edge N is observable in cycle N"), and it is why moving a register update
  after the tick is D1's sanctioned remedy. Because those updates and the preceding
  phase's commit at the same edge, they **share a forwarding map**: `r = a.read(); tick;
  w = r + 1;` must emit `w` from the *new* `r`, since the simulator computes it after
  the edge. (Both lowering paths claimed this in a comment and neither did it, until a
  differential case caught it — `tests/fixtures/trailing_statements_dut.rs`, and
  `tests/sequential_forwarding_divergence.rs::m2_model_forwarded_lowering_matches_the_simulator_for_trailing_update`.)

A value flowing from `C` into `A` is still a **register**, by the liveness rule's
back-edge clause (`Cfg::registers`, added 2026-08-21) — verified on `sync_2ff`
(`src/sync.rs`), where `ff2 = ff1` in the trailing segment must be a flop or the two
synchronizer stages collapse into one (`tests/cdc_synchronizer_anchor.rs` measures it
against `examples/cdc/sv/sync_2ff_ref.sv`). Same cycle, but the
trailing statements execute at an instant just after the edge, and a wire would keep
tracking its inputs for the rest of that cycle.

* **A `RegOut` written in `C` commits at the *next* edge**, not the opening one: a
  `RegOut` write takes effect at the first clock edge after it executes, and `C`
  executes after the edge that opened its cycle (the rule is stated once under
  *Output timing* below; this is its trailing instance). The simulator parks the
  write in `pending` until the next `Clock::advance`; the linear lowering emits
  it from the **committed** register at that edge (`q <= acc`, not the forwarded
  `acc + d` — `shir_lower`'s `trailing_reg_outs`; landed 2026-10-23, which is
  what made `regout_trailing_single_tick` agree and sweep); control extraction's
  trailing state already committed there. The one value that emission cannot
  realize is an `In` port read *directly* in `C` at an immediate read site — the
  flip-flop would sample it one drive later than the simulator did — so that
  shape is refused by `Cfg::trailing_regout_reads_input` (both front-ends; remedy:
  read before the tick, or through a register). Reads with a tick comb-reachable
  from them are deferred and agree (`ex_regfile`'s reads before its halt loop).
  A plain `Out` written in `C` from such a read has the same problem in a body
  that crosses more than one edge — the extraction route edge-samples the drive
  one observation late and the linear multi-tick route emits a continuous assign
  the phase does not gate — and is refused by `Cfg::trailing_out_reads_input`
  (2026-10-26, same remedies; the single-tick body, whose trailing region is the
  whole cycle, agrees and is exempt — the `e8*` probes measure every variant).

**Both lowering routes realize this (phase C, landed 2026-10-23).** The **linear**
multi-tick path commits trailing updates at the opening edge exactly as stated
(measured agreeing, `tests/sequential_forwarding_divergence.rs::linear_trailing_probe`).
**Control extraction** places the statements after the last top-level tick in a
state that runs in the window after that edge and commits at the next one — the
right place for register updates, memory stagings, memory results (which exist only
after the opening edge) and `RegOut` writes. Its one defect was the emitted form of
a plain `Out` driven in that state: a conditionally-driven output became an
edge-sampled register (`o <= edge_value` under the arm's guard), visible one cycle
after the simulator shows it — the measured lag of `trailing_update` (folded loop)
and `branch_trailing` (branch-nested ticks). The fix is an emission form, not a
state placement: `vlir_lower::opening_hold_outputs` emits a conditionally-driven
plain `Out` whose every drive is an **opening** drive (no `In` read precedes it in
its arm — `SHIRStmt::PortDrive::opening`, the same predicate that selects the
forwarded continuous-assign form) as a forwarded combinational mux with a
`<port>_hold` register loaded in `always_ff` under the same guards: the m2 hand
model, which `m2_model_forwarded_lowering_matches_the_simulator_for_trailing_update`
had pinned as matching the simulator cycle-for-cycle. Measured on landing: both
witnesses agree (`trailing_segment_register_update_agrees`, `branch_trailing_probe`),
and so does `pulse_plain`, the `multi_phase_out_write` witness (phase E then
derived and measured the multi-phase class and narrowed that rule, §9 of
`CYCLE_DATAFLOW_SEMANTICS.md`; the input-free pulse is accepted). The rule that
refused the class, `unprotected_trailing_out_write`, is retired. **Reach of the
form, measured 2026-10-23:** the opening-prefix predicate is statement-granular,
and an extracted body is one `match pc` statement, so the mux + hold form is
emitted only for modules that read **no** input; an extracted module with an
input read anywhere edge-samples every conditionally driven plain `Out` and shows
an opening write one cycle late (`e9_read_in_another_state_opening_write`,
unguarded — `CYCLE_DATAFLOW_SEMANTICS.md` §9.4; pinned as an `*_unguarded_gap`
test in `tests/sequential_forwarding_divergence.rs`, and `RegOut` is the form both
sides agree on until it is fixed). The same battery found a second single-phase
shape, `e8_single_phase_trailing_immediate_read` — a plain `Out` written in the
trailing region from an input read in that same statement, immediate here and a
closing drive in the emitted SystemVerilog — which is refused since 2026-10-26 by
`Cfg::trailing_out_reads_input` (see *Trailing statements*).

**Startup — refused where it is observable (2026-10-23).** An `Out` whose every
write is a constant and whose first write executes only *after the second clock
edge* of a multi-tick body shows its initial value at the first observation in the
simulator (the writing segment has not run yet) while the emitted continuous
`assign` drives the constant from time 0: a one-cycle startup transient that no
continuous assign can reproduce. `Cfg::out_first_driven_late` refuses the shape in
both front-ends (opt-out honoured; remedies: write the port in the head segment too,
or declare it `RegOut`). The boundary is measured, not guessed: the same constant one
edge earlier (`late_constant_one_edge`, `tests/fixtures/trailing_statements_dut.rs`)
agrees on every cycle, because the segment the first tick enters runs in edge 0's
post-edge settle and is visible at the first observation exactly when the `assign`
is; and register-valued late writes (`mac_pipeline`'s `out`, `trailing_out`'s `mid`)
agree because the flop's zero initial equals the wire's. The refused witness is the
former `trailing_constant` SKIP row, now `copper-macros/tests/ui/fail/out_first_driven_late.rs`;
exact-set pin `EXPECTED_LATE_CONSTANT` = {`pulse_plain`}.

## Output timing — `Out` vs `RegOut`

Two output-port kinds capture the Mealy/Moore distinction explicitly:

- **`Out<T,D>`** — a combinational (Mealy) output driven within the cycle (`assign out = …`). It
  reflects the current cycle's logic, with no added latency; an `Out` left unwritten on a path
  *holds* its value (a conditional write is an **enabled register** — `vlir_lower::conditional_output_ports`
  routes it into `always_ff` — verified `sim ≡ BaseJump` on `examples/basejump/bsg_dff_en.rs`).
- **`RegOut<T,D>`** — a registered (Moore) output driven from `always_ff`. **A `RegOut` write takes
  effect at the first clock edge after it executes** — one clause, no regions: for a write before a
  tick that is the tick's edge (so a write-before-tick Moore output appears one cycle later; verified
  on `examples/basejump/sipo_block.rs`, `tests/sipo_block_equivalence.rs`), for a write after the
  last tick it is the next edge (see *Trailing statements*). The two axes — input read timing and
  output register timing — are orthogonal.

**Tick position is not a dialect.** A tick-first body (`loop { tick; work }`) and a tick-last body
(`loop { work; tick }`) are the same four clauses applied at two execution instants: a local
assignment is visible to the following statements at once and is realized as a commit at its
region's edge; a plain `Out` write is a continuous drive from the write instant; a `RegOut` write
takes effect at the first clock edge after it executes; an `In` read yields the value present at the
instant it executes (deferred to the closing settle when a tick follows it, immediate otherwise —
*Input read timing* below). What tick position chooses is the instant: a tick-last body's work runs
toward the closing edge and samples the inputs present at it; a tick-first body's work runs just
after the opening edge on the registers that edge committed. The practical consequence for `RegOut`
is that `q.write(acc)` after `acc`'s update is a registered copy of `acc` in a tick-last body and a
copy one edge later in a tick-first one, because the write executes on opposite sides of the edge;
in a tick-first body the registered copy is the plain `Out` written from the committed register
(`out_from_reg_after_commit`, `assign q = acc`). No rule is keyed on tick position; the census of
2026-10-23 (187 sequential modules: 47 tick-last, 34 tick-first, 52 middle, 17 linear multi-tick,
37 extracted) all lower through the same three implementations.

**Multi-write-around-a-tick guardrail.** A combinational `Out` written on *both sides of one bare
`clk.tick().await`* within an iteration, after a *leading (deferred) input read*
(`… inp.read() …; out.write(x); clk.tick().await; out.write(y)`), is **rejected at compile time**.
The read shifts the pre-tick write into the pre-edge settle; the coroutine then runs the post-tick
write in the *same* `tick_clock`'s post-edge and clobbers the pre-tick value before it is observed —
a silent sim ≠ synth divergence (the synthesized combinational Mealy output is correct; the
simulator loses a cycle). The fix is to declare the port `RegOut` (registered/non-blocking, which
buffers and commits at the edge and reconciles) or to split the writes into distinct FSM states so
each state drives the output once. This is Copper's analogue of Verilog's blocking/non-blocking
(`=`/`<=`) choice, and follows MyHDL's discipline of restricting the synthesizable subset — so every
*accepted* program preserves sim ≡ synth. Detection is `Cfg::multi_write_collapse`
(`copper-analysis/src/cfg.rs`, over the shared CFG, top-level loop **and** nested ticking loops),
enforced as a compile error by the `#[hardware]` macro and, since 2026-10-23, by
`transpile_target` as well (neither honours an opt-out for it); it is precise (three necessary conditions — bare tick, both-sides
write, leading read comb-reaching the pre-tick write) and corpus-clean, which the build itself
enforces since every corpus module compiles through the macro. Pinned by
`copper-macros/tests/ui/fail/multi_write_collapse.rs`, `ui/pass/multi_write_regout_ok.rs`, and the
`multi_write_*` cases in `copper-analysis/tests/analysis_semantics.rs`. A single-write-per-cycle
output, a `RegOut`, and a write-straddle *without* a leading read (`counter`; `uart_rx`'s `rx_dv`)
are all unaffected.

**The pre-tick alignment family — dissolved where the denotation defines it,
refused where nothing can realize it.** Historically this family's canonical
divergence was `loop { r = r+1; o.write(r); tick; }` — the simulator's forwarded
`[2,3,4,…]` against an unforwarded `assign o = r`'s `[1,2,3,…]`. Under the
denotation that shape simply *means* the forwarded reading, and since the
2026-08-26 migration (phase B) codegen emits exactly that (`assign o = (r +
8'd1)`): the shape is **legal and agreeing**, along with its whole measured
family (V1/V2/V3/V5/V7 and both `fast_counter` ports), at zero byte-level cost
to any live module. What remains rejected is the derived residue below — each
rule a shape the write-window invariant shows *no* emission can realize, each
with flipping witnesses, and each pointing at the same remedies (`RegOut`,
reorder, or a uniform read). Demonstration fixtures opt out with
`#[hardware(sequential, allow_pretick_alignment)]` — the waiver every lint in
this space ships; it silences the error, not the detection (parsed by the macro's
`HardwareArgs`, honoured by the transpiler's `opts_out_of_pretick_alignment` in
`copper-codegen/src/lib.rs`). Each rule below is a method on `Cfg`
(`copper-analysis/src/cfg.rs`) and has an **exact-set corpus pin** in
`copper-analysis/tests/pretick_alignment_corpus.rs`: the test scans every clocked
module in the corpus and asserts the flagged set equals a listed set, so a rule that
starts flagging a new module — or stops flagging a measured divergence — fails.

**The residue is five rules, not one.** The obvious consolidation — one rule examining every
segment — was implemented and measured **three times**, and rejected every time on corpus evidence:
writing a plain `Out` after a tick is the *ordinary* multi-phase pattern and is correct, so a rule
that cannot say which region it is looking at drowns in false positives. What each rule may assume
about its region is what separates them.

- **`unprotected_pretick_out_write`** — the head segment (loop head → first tick). **NARROWED
  2026-08-26 (cycle-dataflow phase D)**: phase B's forwarded continuous-assign emission gave the
  no-read opening shapes their defined meaning (`assign o = r + 1` — the meaning the simulator
  always had; V1/V5/V7 and both `fast_counter` ports re-measured agreeing), so the register-reading
  clause now also requires the write to be **read-preceded**. What remains refused is the
  **path-dependent region boundary** — a read reaches the write on one path while a register is
  assigned unprotected on another (W4, `probe_fsm`: the write executes at the opening on one path
  and the pre-edge on the other, and no single emission matches both) — plus the unchanged
  conditional/constant hold clause below. `RegOut` remains immune. Pins:
  `pretick_alignment_corpus.rs::pretick_alignment_hazard_flags_exactly_the_known_divergent_modules`
  (as of 2026-09-01 the listed set is `probe_fsm`, `w4_mixed_alignment`, `branch_merge_explicit`,
  `pc_arm_toggle`, `pc_arm_write`), `copper-macros/tests/ui/fail/pretick_alignment.rs` and
  `ui/pass/pretick_alignment_ok.rs` (the no-read opening shape compiles clean).
- **`unprotected_trailing_out_write`** — the trailing hazard, now precisely classified as an
  **extraction lowering limitation** rather than a semantics rule. Two gates, each with a measured
  flipping pair: the body must cross **more than one clock edge** per iteration (in a single-tick
  loop the trailing statements share the head's phase — two widenings ignoring this were measured
  and rejected at 25 and 10 false positives), and — narrowed 2026-08-27 — some tick must sit
  **inside control flow** (`has_nested_tick`, the source-level mirror of control extraction's
  trigger). The linear multi-tick spelling commits trailing updates at the opening edge and is
  measured agreeing (`linear_trailing_probe`), so it is exempt and compiles clean; both
  extraction-shaped spellings are measured one edge late (`trailing_update`, `branch_trailing`).
  The rule retires wholesale when the migration's phase C corrects the extraction path (not
  landed as of 2026-09-01). Pin: `pretick_alignment_corpus.rs::trailing_out_write_flags_exactly_the_demonstration_modules`
  (listed set as of 2026-09-01: `trailing_update`, `branch_trailing`); the 25/10 false-positive
  counts were measured once (`PRETICK_ALIGNMENT_GUARDRAIL.md` §5.4) and are not a regression gate.
- **`multi_phase_out_write`** — a plain `Out` driven in **more than one clock phase** (a phase being
  a Comb-connected component of the CFG), **narrowed 2026-10-23 (cycle-dataflow phase E,
  `CYCLE_DATAFLOW_SEMANTICS.md` §9) to the measured divergent class.** Such a port is a mux over FSM
  state, and it agrees iff every write has the same anchor on both sides. The simulator anchors a
  write by whether a *deferred* `In` read parks the task before it; the transpiler anchors it by
  whether any read appears earlier in its statement walk — and under control extraction that walk
  is the whole `match pc`, so one input read anywhere edge-samples every drive. Accepted and
  measured agreeing: an **input-free** extracted body (every drive opening on both sides, phase C's
  mux — `pulse_plain`, `e2_two_registers`, `e3_forwarded_then_committed`) and a body where **every
  write of the port follows a deferred read** in a module with registers (every drive closing on
  both sides — `e5d_two_closing_deferred`). Refused, each with a flipping witness one edit away:
  the **linear** route (no mux form; `vlir_lower` refuses it as multiply driven or edge-samples the
  hold path — `e7`, `e7w`), an **input read anywhere** with an opening write (`e2x`), a
  **register-free** body with reads (immediate passthroughs — `e4i`, `e5f`), and **mixed anchors**
  (`e4`, `e5`, `e6`). History: the rule landed 2026-08-25 (commit `99290da`) as "any port in two
  phases", forced by the one-cycle pulse `loop { for _ in 0..3 { tick } dv.write(One); tick;
  dv.write(Zero); }` found while writing the UART receiver's first sim-vs-Verilator test, which
  diverged by one cycle until phase C's emission form; the three real modules it flagged then
  (`uart_tx`, `uart_rx`, the CPU's program counter) were migrated to `RegOut`, which remains the
  universal remedy. Pins: `pretick_alignment_corpus.rs::multi_phase_out_write_flags_exactly_the_demonstration_modules`
  (`EXPECTED_MULTI_PHASE`: the eight refused probes in `tests/sequential_forwarding_divergence.rs`),
  the `mphase_*` unit tests in `cfg.rs`, `ui/fail/multi_phase_out_write.rs`, and the two accepted
  shapes in `ui/pass/pretick_alignment_ok.rs`.

**The constant-write exemption is narrower than it looks.** The misalignment changes *when* a write
happens, so it is observable only if the value written differs between the phases — which is why a
write of a constant was exempt. That premise holds only when the write happens on **every** path.
Where a port is written on some paths and not others (the enabled-`Out` idiom), or different arms
write different constants, the alternative is the port's *held* value, so *when* the write lands is
observable even though the value written never changes. Both D1 rules therefore flag a port that is
driven from a register **or** not written on all paths through the region. The witnesses are
`pc_arm_write` and `pc_arm_toggle`, whose traces are each other shifted by exactly one cycle — a
phase shift, not an initialization artifact — and `branch_merge_explicit`, which this doc previously
cited as *agreeing*: it does not. It transpiles byte-identically to its twin, its twin agrees with
it, and it leads its own emitted SystemVerilog by one. Narrowing the exemption (2026-08-25, commit
`348ddd0`) cost exactly those three modules corpus-wide, and all three are in the
`unprotected_pretick_out_write` pin's listed set today (traces pinned by
`tests/sequential_forwarding_divergence.rs::a_write_in_a_state_arm_leads_the_hardware_by_one_cycle`
and `the_state_arm_lead_is_systematic_not_a_first_cycle_artifact`).

- **`pretick_out_write_before_update`** (2026-08-26) — a plain `Out` written **between a leading
  `In` read and the update of a register the write reads**. The read's barrier parks the task at the
  read site, so the write executes in the pre-edge settle with the register's *pre-update* value —
  which the emitted `assign` (Q) never shows at any observation instant; the hardware leads the
  simulator by one cycle, silently. D1 structurally cannot see it: the leading read is D1's
  *protection* (its shapes write after the update, so the barrier hands them the committing value),
  and here it is the *exposure*. The first rule in the family **derived before it was measured** —
  from the cycle-dataflow model (`design_docs/CYCLE_DATAFLOW_SEMANTICS.md`), with the V8 battery
  confirming every predicted trace and the CPU sweep's `program_counter` divergence as the in-vivo
  instance. Zero corpus cost, and it caught a silently-divergent compile-only UI fixture the day it
  landed. Remedies: move the write after the update, or before the read, or `RegOut`. A **trailing
  clause** was added 2026-08-27 (the same shape past the last tick publishes the previous generation
  for the whole cycle); its landing-day catch was three real pipeline stages in
  `tests/module_composition_hybrid.rs`, since rewritten to the write/read/tick/update spelling.
  Pins: `pretick_alignment_corpus.rs::write_before_update_flags_exactly_the_demonstration_modules`
  (listed set as of 2026-09-01: `v8a_read_write_update`, `v8d_temp_renamed_update`,
  `out_from_reg_before_commit`, `v8t_stage_publish_then_load`), the `v8*` verdict tests in
  `tests/sequential_forwarding_divergence.rs`, and `copper-macros/tests/ui/fail/write_before_update.rs`.

**Enforcement is symmetric (front-end parity, 2026-10-23).** The macro's sequential arm
(`copper-macros/src/lib.rs`) and the transpiler's `transpile_target` (`copper-codegen/src/lib.rs`)
run the same rules from the shared crate: `check_nested_loop_shape`, `check_awaits`,
`check_reachability`, `multi_write_collapse`, `unprotected_pretick_out_write`,
`pretick_out_write_before_update`, `trailing_regout_reads_input`, `trailing_out_reads_input`,
`multi_phase_out_write`, `out_first_driven_late`, `check_memory_staging` and
`memory_result_drives_plain_out`. The six pre-tick alignment rules honour `allow_pretick_alignment` in both (a module that exists to
demonstrate a divergence must still transpile, or there is nothing to measure it against);
`multi_write_collapse`, the memory rules and the two shape rules have no opt-out in either. A
module that compiles therefore transpiles, or is refused by both — the standing audit is
`copper-analysis/tests/front_end_parity.rs`, which runs the former transpiler-only rules over
every clocked module in the tree (expected failures: none) and checks that every module a
pre-tick rule flags carries the opt-out. The transpiler keeps its own copies of the shape
refusals (`chir_lower`, `control_extract`) as defence in depth, the way it keeps
`check_reachability`.

**The family's root cause, and what changed about its disposition.** These rules share one origin
worth stating plainly: **Copper infers the register/combinational boundary where every other HDL
makes it explicit.** MyHDL (`sig` vs
`sig.next`), Chisel (`Reg` vs `:=`), Amaranth (`m.d.sync` vs `m.d.comb`), Spade (`reg(clk) … = …`)
and Bluespec (atomic rules) all separate a register's *current* value from its *next* one
syntactically, so the hazard is unexpressible. Verilog leaves it expressible and lints it
(`BLKSEQ`) — but those lints compare an author-written **marker** (`=` vs `<=`) against an
author-written **block kind** (`always_comb` vs `always_ff`), two declarations checked against each
other. Copper has neither, which is why its rules must *infer* both sides. What changed with the
cycle-dataflow rebase is the *disposition*: instead of a lint family growing one measured
discriminator at a time, the normative denotation now **defines** the boundary (registers by the
liveness rule, consumed from one authority; drives by region anchor), the main historical hazard is
legal by construction, and each surviving rule is a derived unrealizability with its evidence
attached. See `design_docs/PRETICK_ALIGNMENT_GUARDRAIL.md` for the full measurement record.

**Passthrough reads are `Immediate` (the D2 fix, 2026-08-21, commit `d640526`;
`passthrough_read` under `classify_reads` in `copper-analysis/src/cfg.rs`).** A read that feeds a
*combinational* `Out` in a segment that assigns **no register** is classified `Immediate`, not
`Deferred`, even when a tick follows it. The barrier a `Deferred` read injects does two jobs — it defers the read *and*
pins the whole segment to the pre-edge phase. Pinning is essential when the segment updates a
register (that is the hazard above); when the segment assigns nothing there is nothing to pin, and
deferring a read that only feeds a wire makes that wire behave like a flop. A passthrough
(`loop { out.write(inp.read()); tick; }` → `assign out = inp;`) used to lag its clocked producer by
a cycle for exactly this reason.

Adjudicated against independent hand-written Verilog: a clocked producer feeding a passthrough gives
`mid == out` in hardware, and only the `Immediate` form reproduces that (pinned by
`tests/sequential_forwarding_divergence.rs::d2_is_fixed_and_d1_still_demonstrates_the_hazard`; the
two-domain chain it fixed is re-blessed in `tests/two_domain_hierarchy_cdc.rs`). The rule is deliberately
narrow — it does **not** apply to a read in a *condition*, because there the sampled value can decide
how many cycles elapse (`det_010_awaits` reads inside `while … { tick }`; `if_tick` picks a branch
with a different tick count), so its phase genuinely matters. A first attempt that keyed on the
module rather than the read broke exactly those two, one of them a hardware-anchored test.

## Input read timing — static edge-phase classification

Under the denotation a read is **defined by value, not instant**: it yields the settled input value
of the window its consumer belongs to — a closing-edge sample where it feeds a commit, the opening
window where it feeds only combinational logic. The static classification below is the
*implementation* of that definition (impl-plan item 3, landed 2026-07-30, commit `329d0c1`;
`classify_reads` / `ReadTiming` in `copper-analysis/src/cfg.rs`), replacing the retired runtime
freshness oracle. An `In` read is classified by its position relative to ticks:

- **`Deferred`** — a "leading"/pre-tick read (a clock tick follows it within the iteration). Its result
  is registered at that edge, so it samples at the **next pre-edge settle**. The macro's
  `ReadRewriter` (`copper-macros/src/lib.rs`) emits `pre_edge_barrier::<D>().await`
  (`copper-sim/src/lib.rs`) before the `.read()`.
- **`Immediate`** — a trailing/post-tick read (no tick follows before the iteration closes). It consumes
  the value the just-past edge produced and fires without deferral — a plain `.read()`.

At runtime there is then no timing heuristic, only the accepted phase machinery. The classification
reproduces the timing the old heuristic got right (loop-top reads in `mac_pipeline`/`sipo_block`
defer; the trailing next-state reads in `counter`/`traffic_light` fire immediately) and fixes the
class it got wrong (the variable-iteration `while in_i.read() == 0 { tick }` in `det_010_awaits`) —
anchored to the independent `examples/sequential/sv/pattern_detector_010.sv`
(`tests/det_010_independent_golden.rs::det_010_awaits_matches_independent_verilog`). The
classifier itself is pinned by `copper-analysis/tests/analysis_semantics.rs::pretick_reads_are_deferred`
and `trailing_read_is_immediate`.

**A repeating wait must test *before* its tick (2026-08-24).** In a nested wait loop, the ordering

```rust
loop { clk.tick().await; if ready.read() == Logic::One { break; } }   // REJECTED
loop { if ready.read() == Logic::One { break; } clk.tick().await; }   // the form to write
```

differs by more than style. The rejected ordering puts the read in the window *after* the entering
edge, where an `Immediate` read consumes the value the just-past edge produced while the flip-flop
the FSM lowers to samples the value present before *its own* edge. Under the harness convention
"drive, then clock" those are different values a cycle apart — measured, the transpiled module
reacted a full cycle earlier than the simulator, and holding each stimulus value for two cycles did
**not** reconcile them (the two models read in different *windows*, not at different points of one).

**Copper does not choose between the two samplings; it declines to let a design depend on which.**
Same disposition as the pre-tick alignment hazard: the divergent program is made unwritable rather
than the divergence adjudicated. The cost is low because the supported ordering expresses the same
designs, and because the divergence needs an input that changes *mid-cycle* — an `In` driven by a
clocked module in the same domain is stable across the window and both models agree. It is a
testbench-observable difference, which is exactly why it could not be left in: sim ≡ SV *under a
testbench* is this project's bar. Enforced in **both front-ends** since 2026-10-23:
`copper_analysis::check_nested_loop_shape` (the source-level mirror of
`control_extract::body_ends_at_a_clock_boundary`) runs in the `#[hardware]` macro and in
`transpile_target`, and `chir_lower` keeps its own copy as defence in depth, so the rejected
order fails at `cargo build` at its own declaration — it can no longer be written as a sim-only
design (the former `tick_first_waiter` fixture is now
`copper-macros/tests/ui/fail/tick_first_wait.rs`). Pinned by
`copper-codegen/tests/unsupported_constructs.rs::ticking_before_the_test_in_a_wait_is_unsupported`
and `tests/wait_loop_equivalence.rs::ticking_before_the_test_is_refused`. The measurement and the
lowering alternatives tried are recorded in the repo `TODO` (the entry *Control extraction
increment B — repeating waits* and the *mid-phase read seam* entry that follows it).

## Memory — staging and read latency

`Memory<T, R, W, D, READ_LAT, WRITE_LAT>` (`copper-core/src/memory.rs`) is a first-class multi-port
synchronous memory: `R` read ports, `W` write ports, latencies in cycles (`≥ 1`), a
`WriteMode::{ReadFirst, WriteFirst}` same-address collision policy and a `ReadMode::{Sync, Async}`
read policy. Both port kinds are fully pipelined — a new address or value may be presented every
cycle regardless of what is in flight.

**A read is *staged*, not returned.** `mem.read_port::<I>().read(addr)` presents an address; the
result appears **at the clock edge** and is observed after the tick with `is_ready()` / `data()`.
None of these are `async` — the `clk.tick().await` between them *is* the wait, which is what makes
memory latency a property of the design's cycle structure rather than of an extra await kind.

Four rules make the divergent shapes unwritable. All four live in `copper-analysis`
(`Cfg::check_memory_staging` and `Cfg::memory_result_drives_plain_out`, `copper-analysis/src/cfg.rs`,
moved there 2026-08-25, commit `8c876fd`), on the **source**, for the reason given under *One
analysis, both front-ends*: control extraction rewrites branch- and loop-nested ticks into a
single-tick `match pc` FSM, so a check downstream of it counts one phase where the `pc` states *are*
the phases it meant to count. **They are enforced in both front-ends** since 2026-10-23:
`transpile_target` (`copper-codegen/src/lib.rs`) and the `#[hardware]` macro's sequential arm
both call `Cfg::check_memory_staging` and `Cfg::memory_result_drives_plain_out`, so a design
that breaks one of them fails at `cargo build` rather than only when transpiled (the former
sim-only `mp_rom` fixture is now `copper-macros/tests/ui/fail/memory_result_drives_plain_out.rs`).

- **One access per bus per cycle** (`check_memory_staging`) — a physical port has a single address
  bus. Two accesses conflict iff one can reach the other **without crossing a clock edge**, not
  merely iff they share a phase. That distinction is the whole difference between a bus conflict and
  a multiplexer: `rv32i_cpu`'s seven regfile writebacks sit in exclusive `match` arms, no path joins
  any two, and each drives the bus in its own state — exactly what the emitted `always_comb` does.
  Counting them instead reported a design error where there is a mux. Pins: the `cfg.rs` unit tests
  `two_stagings_of_one_bus_in_one_cycle_are_rejected`, `stagings_on_exclusive_branches_are_a_mux`,
  `stagings_in_different_cycles_are_not_a_conflict`, `a_read_and_a_write_in_one_cycle_are_different_buses`;
  `copper-codegen/tests/unsupported_constructs.rs::two_accesses_to_one_memory_port_are_rejected`.
- **Observe after the edge that produces it** (`check_memory_staging`) — reading `data()`/`is_ready()`
  with no `read()` staged earlier on that port (the port never becomes ready), or before the
  `clk.tick().await` that produces it, is a spanned compile error. Pins: `cfg.rs` unit tests
  `an_unstaged_port_is_rejected`, `a_same_cycle_observation_is_rejected_even_when_extracted`,
  `a_read_observed_after_the_tick_is_accepted`;
  `unsupported_constructs.rs::memory_read_result_before_the_tick_is_rejected`.
- **No access inside a tick-free nested loop** (`check_memory_staging`) — such a loop is unrolled, so
  every iteration's access lands in the same cycle on one address bus. Refused rather than counted
  as one. Not pinned by a dedicated test as of 2026-09-01.
- **No plain `Out` driven from a read result in a multi-phase module**
  (`memory_result_drives_plain_out`) — the read pipeline re-captures on every clock edge, so a plain
  `Out` wired to it either tracks the result into phases that do not observe it or latches one edge
  after the capture the simulator reads. Measured: a full cycle late on every sampled value. The
  remedies are `RegOut` or a register between the result and the port. A **single-phase** module is
  unaffected and deliberately so — there the post-tick segment shares the head's phase and a plain
  `Out` driven from `data()` is correct (`rom_direct` in `tests/fixtures/multiphase_rom_dut.rs`,
  pinned in `tests/multiphase_memory_equivalence.rs`). Rule pins: the `cfg.rs` unit tests
  `a_plain_out_from_a_read_result_across_phases_is_flagged`, `a_register_between_the_result_and_the_port_is_not_flagged`,
  `a_single_phase_module_is_not_flagged`;
  `unsupported_constructs.rs::a_plain_out_driven_from_a_read_result_in_an_extracted_module_is_rejected`.

## Poll-order and cross-domain interleave independence

**Single domain — poll-order independence.** `poll_tasks` order is an implementation detail: a
well-formed design simulates **bit-identically** under any task order. Enforced by the poll-order
fuzzer (`tests/poll_order_fuzz.rs::combinational_chain_is_poll_order_independent` and
`multi_domain_counters_are_poll_order_independent`: `PollOrder::Insertion` ≡ `Reversed` ≡
`Seeded`; plus the executor unit test `poll_order_does_not_change_cross_task_settle`). **Levelized
scheduling (impl-plan item 6, landed 2026-07-30, commit `56bfe5e`) made the order canonical** and
is the compiled default (`SchedulerMode` has `#[default] Levelized`; `SchedulerMode::env_default`
in `copper-sim/src/executor.rs` honours a `COPPER_SCHEDULER` override) — it polls in a topological
order (`HardwareExecutor::poll_tasks_levelized`) and ignores `PollOrder` entirely. The fuzzer was
**not** retired: it is retained **pinned to `SchedulerMode::Fixpoint`**, because the fixpoint
scheduler stays in-tree permanently as the differential oracle levelized is validated against, and
an oracle that became order-dependent would silently cost that comparison its footing.

**Multiple domains — cross-domain interleave independence.** Independently-ticking clock domains have
no defined phase relationship. The generalized invariant: **a well-formed multi-clock design behaves
correctly under any relative tick interleaving/rate of its domains, provided every clock-domain
crossing goes through a synchronizer.** Two precise senses:

- For a *fixed* tick schedule, the result is poll-order-independent as above (per-domain phase keying
  is what decouples the domains).
- Across *different* relative rates, what is preserved is **functional correctness**, not the exact
  trace: a synchronized signal is monotone (no glitches), data is not corrupted, and events
  eventually propagate — while the exact cycle of an event legitimately shifts with the rate.

Worked example: `examples/cdc/two_domain_hierarchy.rs` checks this rate-independent CDC invariant
(monotone + eventually-asserts) across 2:1 / 3:1 / 1:1 interleavings, and
`tests/two_domain_hierarchy_cdc.rs` anchors the dual-clock timing to an independent hand-written SV
reference (`examples/cdc/sv/two_domain_hierarchy.sv`) under a two-clock Verilator testbench. An
**unsynchronized** crossing is rejected — by the phantom domain types for compiled code
(`copper-core/src/cdc.rs`; `copper-macros/tests/ui/fail/cdc_crossing.rs`), and by the transpiler's
call-site CDC check for the text-based path (`chir_lower::lower_structural_inst`, pinned in
`copper-codegen/tests/structural_hierarchy.rs`; impl-plan item 4).

## Allowed awaits

In `#[hardware]` code the only `await` a design may write is a direct `clk.tick().await`; the one
other await that appears in a hardware body, `pre_edge_barrier::<D>().await`, is injected by the
macro itself (*Input read timing* above), never written by hand. Enforced in **both front-ends**
since 2026-10-23: `copper_analysis::check_awaits` runs in the macro (on the pristine function,
before the barrier is injected) and in `transpile_target`, and `chir_lower` keeps its
`UnsupportedConstruct` "await on non-clock value" as defence in depth; pinned by
`copper-macros/tests/ui/fail/foreign_await.rs` and the shape module's unit tests. Every loop
path must reach a tick — the reachability well-formedness condition below, a hard compile error
in both front-ends.

## The control-flow-graph (CFG) model

The informal invariants above are made precise as a **control-flow graph** over the module's
top-level `loop`. This is the authoritative model: it is what the shared analysis crate
`copper-analysis` builds (`Cfg`, `copper-analysis/src/cfg.rs`) and what **both** front-ends — the
`#[hardware]` macro and the transpiler `copper-codegen` — consume, so the simulator and the
synthesized netlist agree on registers and well-formedness *by construction*, not by two analyses
that happen to match.

The definitions are stated as properties of the **design**, independent of any particular FSM
*construction* (e.g. codegen's `match pc` flattening is one realization; it is not the semantics).

### The graph

A **sequential module** is an `async fn` whose body is a top-level `loop { … }`. Its CFG is:

- **Nodes** — program points at single-statement granularity, plus a distinguished **loop head**
  `H` (the point every iteration re-enters).
- **Edges**, of two kinds:
  - **`E_comb`** — ordinary same-cycle control flow (straight-line, `if`/`else`, `match` arms).
  - **`E_tick`** — the edge *out of* a `clk.tick().await`. It is a **clock-cycle boundary**, and
    is labeled with the **clock receiver identity** (which `Clock<D>` was ticked) — the hook a
    multi-clock design needs to tag *which* domain a boundary belongs to.
- Every "fall off the end of the body" and every trailing tick is a back-edge to `H`.

A **clock cycle** is a maximal `E_tick`-free region of an execution; each `clk.tick().await` ends
one cycle and begins the next. An **FSM state** is a program point at which execution can be
suspended across a boundary — i.e. a node with an incoming `E_tick` edge. (rustc's coroutine
lowering realizes exactly these states for the *simulation*; the semantics here define them
independently of that lowering — threat T1 in the paper's threats-to-validity discussion.)

### Registers — the liveness rule

A local variable `v` is a **register** (a flip-flop; its value must survive a clock edge) iff:

1. `v` is **defined inside the loop** (a `let` binding or an assignment target within the body); and
2. `v` is **live across some `E_tick` edge** — its value produced on one side of a tick is read on
   the other before being overwritten.

Condition (2) is decided by a standard **backward-liveness** fixpoint over the CFG (edge kind is
irrelevant to liveness propagation; a value used after a tick is *live across* it —
`Cfg::registers`, exposed as `copper_analysis::infer_registers`). A value defined after the last
tick and read before the first is live across the loop **back edge** and is a register too — the
back-edge clause added 2026-08-21 for the 2-FF synchronizer's second flop (*Trailing statements*
above). Condition (1) is what distinguishes the two non-registers:

- a **pre-loop constant** read but never assigned in the loop (e.g. `lfsr`'s `xor_mask`) fails (1)
  — it is a combinational wire, even though it is trivially live across every tick;
- a **same-cycle combinational temp** (`let t = …; use t; tick`) fails (2) — it is redefined at the
  loop head before any post-tick use, so it is killed and does not cross the edge.

This is the **minimal synthesizable register set**, computed from control flow. It is deliberately
**not** read off rustc's `Future` layout, which is a conservative *superset* (rustc over-captures;
threat T1 in the paper's threats-to-validity discussion). The rule generalizes the earlier
"pre-loop `let mut` reassigned in the loop" heuristic to registers *born inside* the loop and live
across an *interior* tick (e.g. `mac_pipeline`'s pipeline registers `product`/`c_s`/`sum`) and to
tuple-assignment targets (`traffic_light`'s `(phase, timer)`). Pinned by
`copper-analysis/tests/analysis_semantics.rs::counter_infers_its_accumulator`,
`pipeline_infers_every_stage_register`, `fsm_state_variable_is_a_register`,
`read_only_preloop_binding_is_not_a_register`, and by the two validations under *One analysis, both
front-ends* below.

### Reachability well-formedness

**Condition.** Delete every `E_tick` edge; the subgraph reachable from `H` must be **acyclic**.

Equivalently: **every cycle in the CFG crosses a tick**, so every path through the loop eventually
reaches `clk.tick().await`. A cycle that survives the deletion is a path that returns to the top of
the loop without ticking — a **zero-time combinational loop**, which would spin the simulator's
delta-settle forever. It is a hard, spanned **compile error** (a DFS back-edge check,
`Cfg::check_reachability`, enforced in both front-ends — the macro's sequential arm and
`transpile_fir`). Pins: the `cfg.rs` unit tests `tickless_fallthrough_rejected` and
`uneven_but_both_tick_accepted`; `copper-analysis/tests/analysis_semantics.rs::tickless_loop_is_rejected`,
`partial_tickless_path_is_rejected`, `uneven_but_nonzero_tick_counts_are_well_formed`,
`a_continue_that_skips_the_only_tick_is_a_zero_time_cycle`; the macro-side
`copper-macros/tests/ui/fail/tickless_loop.rs` and `partial_tick_path.rs`; and the corpus scan
`copper-analysis/tests/real_examples.rs` (every sequential module in `examples/` and
`tests/fixtures/` is well-formed; it prints the count it checked).

**Why shape-restriction soundness is not enough (worked example).** Before this analysis, the
invariant held only as an *accident* of the single-trailing-tick construction. Codegen's control
extraction is where the masked failure mode was visible: `lower_into`
(`copper-codegen/src/control_extract.rs`, the "fell through without ticking" case) once emitted
`pc = head` **unconditionally** when a branch reached the loop tail without ticking — silently
handing a zero-tick branch a *free phantom cycle* rather than rejecting it. The CFG check makes
"reaches a tick on every path" a real, checked property instead of a construction accident.

Two independent changes closed that hole. The reachability guard runs *before* control extraction,
so the fall-through is unreachable for malformed input; and the fall-through itself no longer
guesses — it discriminates the two cases that look identical at the call site (`FallThrough`) and are
a clock cycle apart. **`AfterTick`**: the body's own trailing tick was removed by the rotation that
builds a nested loop's head state, so the fall-through *stands for* that tick and `pc = head` is
exactly right. **`ZeroTime`**: nothing was removed, the source genuinely returns to the head in the
same cycle, and a `goto` marker is emitted instead (`goto_marker`, spliced away by
`splice_zero_time_gotos`) — `pc = head` would spend a cycle the program does not have. A real
instance was caught in the wild: `det_010_awaits`'s
`else if` chain had no final `else`, so holding `rstn=1, in_i=1` spins with zero ticks — flagged by
the check and since fixed. Legitimate designs with *uneven* per-branch tick counts still pass
(every path still crosses *a* tick), which is the property shape-restriction could not express.

### Nested loops

A nested `for`/`while`/`loop` that contains a tick is handled two ways at once, both
rejection-sound: (i) in the **parent** graph it is folded into a single boundary node — a possible
0-iteration exit must not make the *outer* loop look tickless (so designs that only tick inside a
`for`/`while`, e.g. `uart_tx`, stay well-formed); and (ii) its body is *also* built as a real
sub-CFG (`Builder::nested_loop_cfg`, with `break`→exit and `continue`→head modeled) on which the
reachability condition is enforced **recursively** — so a tickless cycle *inside* a nested loop
(e.g. `loop { loop { if c { tick } } }`) is rejected (`cfg.rs` unit tests
`nested_loop_tickless_inner_cycle_rejected`, `nested_while_tickless_path_rejected`,
`nested_loop_continue_before_tick_rejected`). A tick-free nested loop is combinational (unrolled)
and is not subject to the must-tick rule. On the transpiler side a tick-bearing nested loop is
lowered by control extraction as a repeating wait (`copper-codegen/tests/while_loops.rs`,
`tests/wait_loop_equivalence.rs`, `tests/nested_tick_equivalence.rs`).

### Combinational loops — the other graph

The CFG above is **intra-module control flow**. A combinational loop lives in a different graph, and
the distinction is not bookkeeping: *within* a module a comb cycle is unexpressible (Rust rejects the
use-before-def), so every real one is **cross-module** and structurally invisible to the CFG. It is
found instead in the **inter-module producer→consumer wiring DAG** that the levelized scheduler
builds.

`HardwareExecutor::comb_cycles()` (`copper-sim/src/executor.rs`) returns each strongly-connected
component of size ≥ 2 in that combinational dependency graph — mutual plain-`Out` feedback with no
`RegOut`, memory, or synchronizer to break it (each of those commits at the clock edge and so induces
no combinational edge). It is **static**: it inspects the wiring without running the simulation, and
is empty for a well-formed acyclic design.

**Detection is not rejection.** A *convergent* combinational loop — a set-dominant latch, say — is
legal hardware, and the levelized settle simulates it by iterating that component to a fixpoint
(`iterate_scc`) while walking the SCCs in topological order, so the acyclic remainder is unaffected.
Only a *non-convergent* loop fails, and only when the settle actually fails to converge: a component
still dirty after `OSCILLATION_THRESHOLD` passes panics, naming the whole SCC. This is the one place
the semantics are enforced at runtime rather than at compile time, because convergence is not a
property the wiring graph alone can decide. The convergent case is pinned by
`tests/levelized_differential.rs::diff_convergent_comb_loop` (both schedulers agree on it); the
non-convergent panic is not pinned by a test as of 2026-09-01.

### One analysis, both front-ends (the c2 architecture)

The CFG is keyed off `syn::ItemFn` — the representation both front-ends already hold — so it is a
single authoritative pass, not two that must agree. **As of 2026-08-27 (commit `d7b2483`) the
register half is consumed, not merely reconciled**: `capture_frontend_ir`
(`copper-codegen/src/parser.rs`) fills `FrontendModuleIR::registers` from `infer_registers`,
`transpile_fir` appends the names its FIR passes synthesize (`pc`, counters, hoisted locals — by
diffing the pre-loop `let mut` set), and `chir_lower` consults that authority for the pre-loop
locals instead of re-deciding syntactically (`shir_lower::find_promoted_wires` still promotes
in-loop wires that are live across a tick). That the change altered no module's emitted
SystemVerilog was measured once with `sv-baseline` at that commit — the measurement that the old
syntactic decider had only ever coincided with the inference — and is not a regression gate. The
inference's output is additionally validated (a) against **independent hand-written SystemVerilog**
(structural reg-for-reg match, `copper_analysis::assert_source_registers_match_reference_sv` via
`EquivalenceTest::with_reference_registers` in `tests/common/mod.rs`, for
`mac_fsm`/`det_010`/`det_110101`/`lfsr`) and (b) against the **transpiler's own emitted flip-flops**
(`copper-codegen/tests/register_reconciliation.rs`: codegen ≡ this set + only its synthesized
phase/pc counter). Note (b)'s scope: it scans the clocked modules under `tests/fixtures/` only —
not `examples/` or `src/` — asserts it reconciled at least 8, and prints the count it actually
checked. See `SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md` item 2.

## How these semantics are checked

Five mechanisms, deliberately independent of each other:

- **Independent hand-written Verilog.** `examples/basejump/` checks Copper modules against
  third-party BaseJump STL Verilog, and the hand-written goldens
  (`examples/sequential/sv/pattern_detector_010.sv`, `tests/fixtures/timing_probe_sv/mac_fsm.sv`,
  the `REFERENCE` table in `build.rs`) anchor the rest; `tools/regression.sh`'s **G-E** guard
  reports how much of the sweep is anchored this way. This is load-bearing: the shared analysis makes the simulator and
  the transpiler agree on registers and well-formedness *by construction*, so an independent
  reference is the only thing left that can catch a **timing** bug. Never the transpiler's own
  output — that is circular.
- **The corpus differential sweep.** `build.rs` generates one case per `#[hardware]` module in
  `tests/fixtures/` and `examples/` into `tests/corpus_generated.rs` — seeded random stimulus, the
  simulator versus the SystemVerilog that module transpiles to, under Verilator. A new module is
  therefore covered the moment it exists, with no harness to write; what cannot be inferred lives in
  four reviewed tables in `build.rs` (`PARAMS` widths, `RESET`, `SKIP`, and `REFERENCE` for an
  independent golden), and `tools/regression.sh`'s **G-D** guard asserts the sweep covered the
  corpus and ran, printing how many differential cases ran and how many were `#[ignore]`d with a
  reason (that printout is the authoritative count; the file generated from the tree on 2026-09-01
  held 143 cases — 133 running, 10 `#[ignore]`d with a reason). Its first run found two defects that had been in the
  tree for weeks and were unreachable from the existing tests — a measured sim ≠ synth divergence in
  `branch_merge_explicit`, and emitted SystemVerilog that would not parse (a port named `event`, a
  keyword missing from the legalizer). See `design_docs/CORPUS_DIFFERENTIAL_SWEEP.md`.
- **Denotation anchors.** `tests/cycle_dataflow_memory_derivation.rs` and
  `tests/cycle_dataflow_uart_derivation.rs` derive a module's exact trace **from the model on
  paper** (in the test's comment) and assert that the simulator **and** the transpiled SV both
  reproduce it under deterministic stimulus — the memory windows (ReadFirst collision, one-edge
  write visibility, hold-when-unstaged) and the uart receiver's per-edge sampling (a stimulus that
  carries the complement bit at every non-sample edge, so an off-by-one read corrupts the byte
  loudly). The sweep proves the implementations agree with each other; these prove they agree with
  the *denotation*.
- **The emitted-SV baseline.** `cargo run -q -p copper-codegen --bin sv-baseline -- snapshot|diff`
  (`copper-codegen/src/bin/sv-baseline.rs`) byte-diffs every corpus module's emission against a
  snapshot the author took before the change, so a codegen change alters **exactly its declared
  module set and nothing else** — the migration's phase gates ran on it, and its first
  snapshot-then-diff on an unchanged tree caught the emitter being nondeterministic. It is a tool
  for a change's author, not a regression gate: the snapshot lives under `target/sv-baseline/`,
  nothing is committed, and no test pins byte-identity. Every "byte-identical" claim in this
  document is therefore a one-off measurement at the commit named.
- **The two schedulers.** `tests/levelized_differential.rs` steps a design under both
  `SchedulerMode::Levelized` and `SchedulerMode::Fixpoint` in lockstep and asserts **every wire holds
  an identical value after every phase of every cycle**, so a divergence is localized to the exact
  design, cycle, and phase. Its designs are chosen to span the topologies the levelized DAG must get
  right (`diff_comb_chain`, `diff_diamond_fanout_fanin`, `diff_register_then_comb`,
  `diff_regout_pipeline`, `diff_convergent_comb_loop`, `diff_multi_domain_independent`); corpus
  *breadth* under the levelized scheduler comes from `tests/golden_traces.rs` running the frozen
  goldens (`tests/golden_traces/*.trace`, re-blessed only with `BLESS_GOLDEN=1`) under both
  scheduler modes.
  Fixpoint is retained permanently for exactly this reason: a scheduler change must be provably
  behavior-neutral, not argued to be.

# Clause inventory of the design documents — 2026-11-03 baseline

Read-only inventory of every semantic statement in `design_docs/*.md` (excluding
`OUTDATED/`, the `FORMAL_SEMANTICS*` drafts and `prior_art/`) and
`copper-core/src/memory.md`, taken at commit `ea54d26`. Each clause gives (a) a
restatement, (b) the source and a short quote, (c) the status the document itself
assigns, (d) the implementing functions the document names.

Abbreviations: **SS** = `SYNCHRONOUS_SEMANTICS.md`; **CDS** = `CYCLE_DATAFLOW_SEMANTICS.md`;
**PAG** = `PRETICK_ALIGNMENT_GUARDRAIL.md`; **DT** = `DERIVATION_TABLE.md`; **PIS** =
`PAIRED_IMPLEMENTATION_SCOPE.md`; **TMU** = `TIMING_MODEL_UNIFICATION.md`; **TCM** =
`TIMING_COVERAGE_MATRIX.md`; **LSS** = `LEVELIZED_SCHEDULING_SCOPE.md`; **APA** =
`ARRAY_PORT_ABI.md`; **RMA** = `RECEIVED_MEMORY_ABI.md`; **CSW** =
`CORPUS_DIFFERENTIAL_SWEEP.md`; **AAM** = `ANCHORING_A_MODULE.md`; **IMPL** =
`SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md`; **MEMMD** = `copper-core/src/memory.md`.

## Summary

**254 clauses:** time and phases 17; segments, regions and anchors 22; registers 11;
`In` read timing 13; `Out` write, hold and observation 19; `RegOut` 8; trailing region
9; power-on and initial values 11; memory 16; combinational modules 6; synchronizer,
CDC and multi-domain 9; structural and hierarchy 7; scheduling, comb loops and CFG 20;
refusal rules 46; agreement claim and checks 16; miscellaneous refusals 24.

**Top contradictions:** (1) `has_nested_tick` is reported deleted but still used; (2)
the trailing `RegOut` commit edge differs between SS's normative paragraph and its own
trailing section; (3) the rule list is stale in SS, PAG and TMU; (4) the D2 passthrough
scope differs across SS, CDS §9.1/§10.1, CDS §10.8 and IMPL; (5) edges numbered from 1
vs from 0; (6) two meanings of "segment"; (7) the non-convergent loop panic is "not
pinned" in SS and pinned in LSS; (8) AAM says no `initial` blocks, contradicted by the
2026-10-29 decision; (9) CDS §4's constant-write derivation was later measured false
and relabelled a proxy; (10) SS's "every accepted program preserves sim ≡ synth" sits
beside recorded unrefused gaps.

**Top gaps:** the dual-consumer read; `RegOut` with an immediate read in a trailing
guard; `multi_phase_out_write` clause 5; the mixed wire+register operand; the V8 value
clause over-refusing held ports; a forwarded guard in a middle phase on the extraction
route; memory `ReadMode::Async`, latencies above 1, `WriteFirst`, write–write
collisions; structural modules' simulation meaning and the synchronizer arm's rule
coverage; multi-domain checked on three ratios only; a tick-bearing loop running zero
iterations; the observable value before the first edge; `ram_prewrite` and
`branch_on_updated_reg_bit` still owed.

---

## 1. Time & phases (T1–T17)

- **T1** (a) The default simulator runs the `#[hardware]` async fn as plain Rust; rustc's coroutine lowering is the FSM; the analysis supplies facts but the sim never interprets an IR. (b) SS "Execution model — c2" — "The default simulator always executes plain Rust." (c) DECIDED 2026-07-29. (d) `copper-sim` executor, `copper-analysis`.
- **T2** (a) A CHIR interpreter may exist only as an optional validation backend, never the default. (b) SS ibid.; IMPL item 7 — "validation-only backend, never the default". (c) DECIDED; NOT BUILT. (d) —
- **T3** (a) Per clock domain, edges are numbered 1, 2, …; cycle N is [edge N, edge N+1). (b) SS "The denotation"; CDS §1. (c) DECIDED (normative). (d) —
- **T4** (a) Cycle N's observation instant is just after edge N's post-edge settle (drive, tick, observe). (b) SS ibid.; CDS §1. (c) DECIDED. (d) harness.
- **T5** (a) The input value x_N is driven before edge N and stays stable through cycle N until x_{N+1} is driven late in it. (b) CDS §1 — "the value driven *before* edge N". (c) DECIDED (harness convention). (d) —
- **T6** (a) Every signal has one value per cycle. (b) SS denotation — "one value per signal per cycle". (c) DECIDED. (d) —
- **T7** (a) Write-window invariant: a sim write at instant t is observed for [t, next write); the hardware is a continuous function of registers and inputs; they agree iff, at every observation instant in the intended window, the written value equals that function. (b) SS; CDS §1 — "The two agree iff…". (c) DECIDED (normative). (d) —
- **T8** (a) `tick_clock` has four phases: a pre-edge settle (registers hold the previous values; leading reads sample here), the clock edge (registers commit), a post-edge settle, then observation by the testbench. (b) SS "The clock tick — phases". (c) DECIDED/implemented. (d) `HardwareExecutor::tick_clock`, `poll_tasks`, `Clock::advance`.
- **T9** (a) The `clk.tick()` future resolves in the post-edge settle, so post-tick code runs in the same `tick_clock`; a register clocked at edge N is observable in cycle N. (b) SS "Post-edge continuation convention". (c) MEASURED against BaseJump. (d) `tests/sipo_block_equivalence.rs`, etc.
- **T10** (a) Phase, tick-resolution and read-timing signals are keyed per clock domain; ticking one clock cannot perturb another domain. (b) SS "Per-domain phase keying". (c) DECIDED, pinned. (d) `set_poll_phase::<Domain>`, `PollPhase`, `set_tick_resolving`, `TICK_RESOLVING`.
- **T11** (a) Each `clk.tick().await` is a cycle boundary; a clock cycle is a maximal tick-free region of an execution. (b) SS "Cycle boundaries and FSM states". (c) DECIDED. (d) —
- **T12** (a) An FSM state is a suspension point, i.e. a CFG node with an incoming `E_tick`. (b) SS "The graph". (c) DECIDED. (d) `Cfg`.
- **T13** (a) The pre-edge barrier is phase-constant within a settle (Pending throughout post-edge, Ready throughout pre-edge); a tick resolves only at the pre→post transition. (b) LSS "Gate (item 3)". (c) DECIDED/verified. (d) `PreEdgeBarrier`, `pre_edge_barrier`.
- **T14** (a) A hardware future that returns `Ready` panics; a hardware coroutine must never complete. (b) IMPL item 1b. (c) DECIDED, pinned. (d) `tests/executor_guardrails.rs::module_whose_future_returns_panics`.
- **T15** (a) Tick position (tick-first vs tick-last) is not a dialect: the same four clauses apply at two execution instants, and no rule is keyed on tick position. (b) SS "Tick position is not a dialect". (c) DECIDED (2026-10-23 census). (d) —
- **T16** (a) The only await a design may write is `clk.tick().await`; `pre_edge_barrier` is injected by the macro. (b) SS "Allowed awaits". (c) DECIDED (enforced in both front-ends). (d) `copper_analysis::check_awaits`, `chir_lower` "await on non-clock value".
- **T17** (a) Open item: whether "restore phase after tick resolution" is a live bug was never investigated. (b) IMPL 1b — "never taken up as a separate item". (c) OPEN. (d) —

## 2. Segments / regions / anchors (S1–S22)

- **S1** (a) A segment is a maximal tick-free region, with trailing statements merged with the head. (b) SS denotation; CDS §2. (c) DECIDED 2026-08-25. (d) —
- **S2** (a) Inside a segment, statements run in program order with forwarding: a local read yields the last value assigned in the segment, else the value committed at the opening edge. Both implementations owe this. (b) CDS §2 — "*This clause is the meaning of the source; both implementations owe it.*" (c) DECIDED. (d) —
- **S3** (a) The segment that opens at edge N denotes cycle N's computation. (b) CDS §2. (c) DECIDED. (d) —
- **S4** (a) A segment splits at the read-site barrier into an opening prefix (before the first leading `In` read; runs at the opening instant on committed state) and a closing suffix (runs at the pre-edge settle on closing-edge samples and forwarded values). (b) SS denotation; CDS §2 refinement note. (c) MEASURED/VALIDATED; the region-entry barrier was falsified by V8c. (d) read-site barrier.
- **S5** (a) The unit of anchoring is the region, not the segment or the Comb-component. (b) DT §1 — "Anchoring is per *region*". (c) DERIVED. (d) `Cfg::derivation_facts` (reporting only).
- **S6** (a) Commits anchor to the segment's closing edge: register locals, `RegOut` writes, memory stagings, and the implicit state register fed by a control-steering read. (b) SS denotation. (c) DECIDED. (d) —
- **S7** (a) A trailing region's commits anchor to the edge that opens its cycle; the region is opening-anchored and its reads are Immediate. (b) SS; DT §1. (c) DERIVED; superseded for `RegOut` by RO3. (d) —
- **S8** (a) A segment is closing-anchored iff some committed value depends on a same-cycle `In` read; otherwise it is opening-anchored. (b) CDS §3 "Definition". (c) DERIVED; the per-segment `segment_anchor` was never written. (d) none (`segment_anchor`/`region_anchor` absent).
- **S9** (a) Obligations by anchor: opening means no barrier and forwarded emission; closing means a barrier and the unforwarded commit-frontier form. (b) CDS §3 table. (c) DERIVED; the executor cell turned out to be already satisfied by the read-site barrier (PIS §0). (d) —
- **S10** (a) A plain `Out` drive anchors to the opening edge (observed from edge N's observation on); an unwritten path holds. (b) CDS §2 — "Plain `Out` drives anchor to the opening edge." (c) DECIDED; refined by S4 (closing-suffix drives are seen at N+1). (d) —
- **S11** (a) Commit-frontier condition: a closing-anchored plain-`Out` write is realizable iff its value is a register committing at N+1 (forwarded) or a constant on every path; anything else is unrealizable. (b) CDS §4. (c) DERIVED. (d) —
- **S12** (a) Commit-frontier taxonomy: an every-path (or held) constant, a register with no update after the write, or a register updated before the write is legal; a write between a leading read and an update is hazardous. (b) DT §4b. (c) DERIVED (pre-dates the 2026-10 held-port findings). (d) `derivation-audit`.
- **S13** (a) Four legal shapes: an opening-region write of committed state; a closing write after every update; an every-path constant; a `RegOut`/register commit. (b) DT §1 table. (c) DERIVED/MEASURED. (d) —
- **S14** (a) The executor already implements the model and was not changed; the paired fix is codegen-only. (b) PIS §0 — "the executor already implements the model". (c) MEASURED/DECIDED. (d) —
- **S15** (a) Path-dependent region boundary: when the read is on one path only, a shared write runs at different instants per path and no single emission matches both. (b) PIS §0; DT F3. (c) DERIVED. (d) —
- **S16** (a) Simulator anchor of a write: closing iff a deferred `In` read parks the task before it; otherwise opening. Trailing reads and register-free passthrough reads are immediate. (b) CDS §9.1. (c) DERIVED/MEASURED. (d) `classify_reads`.
- **S17** (a) Transpiler anchor of a drive: opening iff no `In` read precedes it in the walk of its own arm (arm-granular); past a branch the walk carries any arm's read. It is control-flow based: the operand, enclosing condition and wait-exit test all count. (b) CDS §9.1. (c) DECIDED (arm-granular since 2026-10-26; statement-granular before). (d) `shir_lower::lower_stmt_list`, `SHIRStmt::PortDrive::opening`.
- **S18** (a) A drive agrees iff its codegen anchor equals the sim anchor on every path reaching it (drive-level since 2026-10-29). (b) CDS §9.1, §10.4 — "a drive agrees iff its codegen anchor equals its simulator anchor". (c) DERIVED/MEASURED. (d) —
- **S19** (a) A closing write followed by the next iteration's opening write of the same port is never observed. (b) CDS §10.1 — "is **never observed**". (c) MEASURED (`d2b`). (d) —
- **S20** (a) Input-fed class: a write fed from an `In` read through wires agrees on either anchor under drive-then-clock; the windows differ only if the input changes mid-cycle. (b) DT §4b. (c) DERIVED. (d) —
- **S21** (a) A read whose result feeds both a commit and a plain `Out` (two sample instants for one binding) is undecided. (b) CDS §8-1 — "**Open as a refinement.**" (c) OPEN. (d) —
- **S22** (a) The CFG is value-blind: a drive whose guard is false on the read-free path still counts as mixed (refusing direction). (b) CDS §10.3. (c) DECIDED (conservative). (d) `Cfg`.

## 3. Registers & inference (REG1–REG11)

- **REG1** (a) A local is a register iff it is defined inside the loop and live across some `E_tick`. (b) SS "Registers — the liveness rule". (c) DECIDED. (d) `Cfg::registers`, `copper_analysis::infer_registers`.
- **REG2** (a) Back-edge clause: a value defined after the last tick and read before the first is a register. (b) SS ibid. (c) DECIDED 2026-08-21. (d) `Cfg::registers`.
- **REG3** (a) A pre-loop binding read but never assigned in the loop is a wire. (b) SS ibid. (c) DECIDED. (d) —
- **REG4** (a) A same-cycle temp redefined before any post-tick use is not a register. (b) SS ibid. (c) DECIDED. (d) —
- **REG5** (a) The register set is the minimal synthesizable set, not rustc's `Future` layout. (b) SS ibid. (c) DECIDED. (d) —
- **REG6** (a) Registers stay plain locals; there is no current/next split. (b) CDS decision 2; TMU "standing decision"; PAG §9. (c) DECIDED. (d) —
- **REG7** (a) Codegen consumes the shared register set plus the names it synthesizes itself (`pc`, counters, hoisted locals); in-loop wires live across a tick are still promoted by codegen. (b) SS "One analysis, both front-ends"; CDS §2. (c) DECIDED (commit d7b2483). (d) `capture_frontend_ir`, `FrontendModuleIR::registers`, `transpile_fir`, `chir_lower`, `shir_lower::find_promoted_wires`.
- **REG8** (a) Inference is validated reg-for-reg against hand-written SV (four modules) and against codegen's flops (`tests/fixtures` only). (b) SS ibid. (c) MEASURED. (d) `assert_source_registers_match_reference_sv`, `register_reconciliation.rs`.
- **REG9** (a) A value flowing from the trailing region C into the head A is a register (`sync_2ff`'s `ff2`). (b) SS trailing. (c) MEASURED. (d) —
- **REG10** (a) Control extraction hoists a cross-state local into a flop. (b) CSW App. A (B4). (c) implementation fact. (d) `control_extract::hoist_cross_state_locals`.
- **REG11** (a) Tuple-assignment targets and registers born inside the loop across an interior tick are registers. (b) SS liveness rule. (c) DECIDED. (d) —

## 4. In read timing (IN1–IN13)

- **IN1** (a) A read is defined by value: the settled input of its consumer's window (the closing-edge sample when it feeds a commit, the opening window when it feeds only comb logic). (b) SS "Input read timing". (c) DECIDED. (d) `classify_reads` as implementation.
- **IN2** (a) Deferred: a tick follows the read within the iteration; it samples at the next pre-edge settle; the macro emits a barrier before it. (b) SS ibid. (c) DECIDED. (d) `ReadTiming::Deferred`, `ReadRewriter`, `pre_edge_barrier::<D>`.
- **IN3** (a) Immediate: no tick follows before the iteration closes; it consumes the value the just-past edge produced. (b) SS ibid. (c) DECIDED. (d) `ReadTiming::Immediate`.
- **IN4** (a) D2 passthrough: a read feeding a combinational `Out` in a segment assigning no register is Immediate even if a tick follows. (b) SS "Passthrough reads are `Immediate`". (c) DECIDED 2026-08-21 (adjudicated against hand-written Verilog). (d) `passthrough_read` in `classify_reads`.
- **IN5** (a) The passthrough exception does not apply to a read in a condition. (b) SS ibid. (c) DECIDED. (d) —
- **IN6** (a) The passthrough applies only to the direct argument of `o.write(…)`; `let t = x.read(); o.write(t)` is deferred. (b) CDS §10.8 "(c)". (c) MEASURED. (d) —
- **IN7** (a) A condition read that steers the next suspension point feeds the implicit `pc` (closing evidence). (b) DT §2.2. (c) DERIVED. (d) `control_input_read`.
- **IN8** (a) A repeating wait must test before its tick; the tick-first wait is refused, so Copper does not choose between the two samplings. (b) SS "A repeating wait must test *before* its tick". (c) DECIDED (refusal), MEASURED. (d) `check_nested_loop_shape`, `control_extract::body_ends_at_a_clock_boundary`, `chir_lower`.
- **IN9** (a) Sim and hardware differ only for inputs that change mid-cycle; an `In` driven by a clocked module in the same domain is stable. (b) SS ibid. (c) DECIDED (scope of the claim). (d) —
- **IN10** (a) A module with no `In` parameters gets no barrier. (b) PAG §1 — "`inject_synced_reads` returns early". (c) implementation fact. (d) `inject_synced_reads`.
- **IN11** (a) Every read after the last tick is immediate. (b) CDS §10.9 model. (c) DERIVED. (d) —
- **IN12** (a) A folded tick-bearing loop's input use counts as closing evidence. (b) DT uart row (m4). (c) MEASURED. (d) `Cfg::derivation_facts`.
- **IN13** (a) The impl plan states the rule as "Deferred iff a tick occurs after it", with no passthrough exception. (b) IMPL item 3 "The rule". (c) DECIDED 2026-07-30; stale (see contradictions). (d) `classify_reads`.

## 5. Out write / hold / observation (OUT1–OUT19)

- **OUT1** (a) `Out<T,D>` is a combinational Mealy output with no added latency; a path that leaves it unwritten holds it (a conditional write is an enabled register). (b) SS "Output timing". (c) MEASURED (BaseJump `bsg_dff_en`). (d) `vlir_lower::conditional_output_ports`.
- **OUT2** (a) A plain `Out` write is a continuous drive from the write instant. (b) SS "Tick position". (c) DECIDED. (d) —
- **OUT3** (a) An opening-prefix drive is emitted as the forwarded expression. (b) SS denotation — "is the **forwarded** expression". (c) DECIDED/landed 2026-08-26. (d) `shir_lower` `forward_opening_drives`/`seen_in_read`, `SHIRPortDrive::edge_value`.
- **OUT4** (a) A closing-suffix drive is emitted in the unforwarded commit-frontier form, legal iff its value is frontier-expressible. (b) SS denotation. (c) DECIDED. (d) `SHIRPortDrive::value`.
- **OUT5** (a) Mux + hold form: a conditionally driven plain `Out` with at least one opening drive becomes a comb mux over its opening drives plus a `<port>_hold` register loaded by every drive under edge-form guards. (b) SS trailing; CDS §9.4. (c) DECIDED (drive-level since 2026-10-29; port-wide before). (d) `vlir_lower::opening_hold_outputs`, `strip_closing_drives`, `shadow_hold_ff`, `hold_form_ports`.
- **OUT6** (a) A conditionally driven port with no opening drive is an edge-sampled register under guards. (b) CDS §9.1. (c) DECIDED. (d) —
- **OUT7** (a) A port driven on every path of its phase is an `always_comb` assign or mux with committed-form guards and no hold register. (b) CDS §10.1, §10.4. (c) implementation fact. (d) `emit.rs`.
- **OUT8** (a) Linear multi-tick route: an unconditional register, input or constant drive is a continuous assign outside every phase guard. (b) CDS §10.7–§10.8 table. (c) MEASURED. (d) —
- **OUT9** (a) Linear route: an unconditional same-segment wire drive is a phase-guarded flop; a trailing drive is hoisted into phase 0. (b) ibid. (c) MEASURED. (d) —
- **OUT10** (a) Linear route, conditional: if any drive of the port reads a same-segment wire, every drive becomes the phase-scoped flop. (b) CDS §10.9 form table. (c) MEASURED (`e16m`). (d) `phase_scoped_output_ports`.
- **OUT11** (a) Linear route, conditional, otherwise: with an opening drive it takes the mux + hold form with `o = o_hold` at the top outside the phase guards; with no opening drive, the flop. (b) CDS §10.9. (c) DECIDED (latch fix 2026-11-02). (d) `VLIRSeqBody::comb_defaults`.
- **OUT12** (a) Linear route: a port written both unconditionally and conditionally in one segment gets two drivers (refused, RR29). (b) CDS §10.9. (c) MEASURED. (d) —
- **OUT13** (a) Single-phase body: an unconditional write of a port also written in a nested `if`/`match`/`for` stays in the comb copy in program order, giving one every-path mux. (b) CDS §10.10 "The fix". (c) DECIDED 2026-11-02. (d) `vlir_lower::lower_seq`, `nested_port_drives`.
- **OUT14** (a) A wire-valued operand is one that reads a local defined in the loop that is neither a register nor an input; pre-loop bindings are constants or committed state. (b) CDS §10.8 clause 2. (c) DECIDED. (d) —
- **OUT15** (a) A mixed wire+register operand is treated as wire-valued. (b) CDS §10.7 tail — "no probe measured the mixed operand". (c) unmeasured. (d) —
- **OUT16** (a) A multi-phase plain `Out` is a mux over FSM state and agrees iff each write has the same anchor on both sides. (b) CDS §9.1. (c) DERIVED/MEASURED. (d) —
- **OUT17** (a) A single-tick body's trailing region is the whole phase; its continuous assign sees what the immediate read saw. (b) CDS §9.4 `e8l`. (c) MEASURED. (d) —
- **OUT18** (a) In mux forms, guards take the committed form in `always_comb` and the edge form only in the `always_ff` split. (b) CDS §10.1. (c) implementation fact. (d) `emit.rs` `condition`.
- **OUT19** (a) The trailing walk forwards nothing on the linear route; a trailing conditional drive is the phase-0 flop. (b) CDS §10.9. (c) implementation fact. (d) —

## 6. RegOut (RO1–RO8)

- **RO1** (a) `RegOut<T,D>` is a registered Moore output from `always_ff`; a write takes effect at the first clock edge after it executes. (b) SS "Output timing" — "one clause, no regions". (c) DECIDED/MEASURED (`sipo_block`). (d) —
- **RO2** (a) A write before a tick appears one cycle later. (b) SS ibid. (c) MEASURED. (d) —
- **RO3** (a) A `RegOut` written in the trailing region C commits at the next edge, not the opening one; the linear lowering emits it from the committed register. (b) SS trailing; CDS §5 PC#2 row — "the paper assignment was wrong for `RegOut`". (c) MEASURED 2026-10-23. (d) `shir_lower` `trailing_reg_outs`.
- **RO4** (a) `RegOut` is immune to the phase question, not to forwarding; commits are computed from forwarded values (repaired). (b) PAG §5.2 correction; TMU §1b; CDS §5. (c) MEASURED 2026-08-25. (d) `SHIRStmt::PortDrive::edge_value`, `vlir_lower::split_output_regs`.
- **RO5** (a) In a tick-first body `q.write(acc)` is a copy one edge later; a registered copy there is a plain `Out` from the committed register. (b) SS "Tick position". (c) DECIDED. (d) —
- **RO6** (a) `RegOut` commits via `ClockEdgeListener` at the edge, so it is a phase source with no comb edge. (b) LSS key risk. (c) DECIDED. (d) `WireKind::Registered`, `ClockEdgeListener`.
- **RO7** (a) `RegOut` is the universal remedy for the refusal family. (b) SS rules; PAG. (c) DECIDED. (d) —
- **RO8** (a) The trailing `RegOut` refusal reads value sources only; a guard read is unmeasured. (b) CDS §10.8 "(b)". (c) unmeasured/unrefused. (d) `trailing_regout_reads_input`.

## 7. Trailing region (TR1–TR9)

- **TR1** (a) In `loop {A; tick; B; tick; C}`, C and the next A share one cycle, the head's. (b) SS "Trailing statements". (c) DECIDED 2026-08-25. (d) `shir_lower::lower_seq_body`.
- **TR2** (a) C's combinational logic lowers into phase 0. (b) ibid. (c) DECIDED. (d) —
- **TR3** (a) C is emitted before A, matching execution order (C runs post-edge of the opening tick, A pre-edge of the closing tick). (b) ibid. (c) DECIDED. (d) —
- **TR4** (a) C's register updates commit at the last tick's edge and share the preceding phase's forwarding map. (b) ibid. (c) MEASURED. (d) `tests/sequential_forwarding_divergence.rs::m2_…`.
- **TR5** (a) Control extraction places post-last-tick statements in a state that runs after that edge and commits at the next one. (b) SS "Both lowering routes". (c) DECIDED (phase C, 2026-10-23). (d) `control_extract`.
- **TR6** (a) In a single-tick loop the trailing statements share the head's phase; in a multi-tick body they do not. (b) PAG §5.4; DT §1. (c) MEASURED. (d) —
- **TR7** (a) "More than one tick" counts tick nodes, and a folded tick-bearing loop counts as more than one. (b) PAG §5.4. (c) DECIDED. (d) `Cfg::crosses_more_than_one_tick`.
- **TR8** (a) The linear route and the extraction route are distinguished by whether some tick sits inside control flow. (b) PAG §5.4; CDS §9.3. (c) DECIDED; the predicate is reported deleted elsewhere. (d) `Cfg::has_nested_tick`.
- **TR9** (a) A trailing publish-then-load order shows the previous generation for the whole cycle. (b) PAG §5.6 trailing clause. (c) MEASURED. (d) —

## 8. Power-on / initial values (PO1–PO11)

- **PO1** (a) A register's power-on value is its binding's initializer at loop entry; SV states it as `initial`, and an array as a fill. (b) SS "Register power-on values". (c) DECIDED 2026-10-29. (d) `emit::reg_inits`, `chir_lower::lower_register_init`.
- **PO2** (a) The initializer must be a foldable constant (literals, `true`/`false`, `Logic::One`/`Zero`, enum variants, `Bits::zero`, `from_lit`, `from_u32`/`from_u8`, literal consts, arrays of these, arithmetic over them). (b) SS ibid. (c) DECIDED. (d) `copper_analysis::is_constant_expr`.
- **PO3** (a) `Bits::x()`/`Logic::X` means undefined: nothing is stated and a reset is needed; the sim carries X where Verilator reads 0. (b) SS ibid. (c) DECIDED. (d) `RESET` table.
- **PO4** (a) A non-constant initializer is refused by both front-ends, with no opt-out. (b) SS ibid. (c) DECIDED 2026-10-29. (d) `copper_analysis::check_register_initializers`.
- **PO5** (a) A register born inside the loop has no power-on value. (b) SS ibid. (c) DECIDED. (d) —
- **PO6** (a) Codegen-synthesized state starts at zero: `RegOut` flops, the edge register of a conditional `Out`, `_hold` registers, and hoisted locals with non-constant initializers. (b) SS second paragraph; CSW §3.3. (c) DECIDED 2026-10-29 ("the user decided"). (d) `VLIRSeqBody::port_inits`, `init: Some(0)`.
- **PO7** (a) A `Memory::new` array starts at `T::default()` (zero fill); read and write pipelines start empty. (b) SS ibid. (c) DECIDED. (d) —
- **PO8** (a) A wire starts at zero (harness `wire(Bits::zero())`). (b) SS ibid. (c) harness convention. (d) —
- **PO9** (a) `initial` is honoured by simulators and FPGA tools and ignored by most ASIC flows; such designs need a reset. (b) SS ibid. (c) DECIDED (stated regardless). (d) —
- **PO10** (a) A constant-only `Out` first written after the second edge of a multi-tick body is refused; one edge earlier agrees; register-valued late writes agree because the flop's zero equals the wire's. No initial-value rule for `Out` was added. (b) SS "Startup"; CDS §8-3. (c) DECIDED by refusal 2026-10-23; MEASURED. (d) `Cfg::out_first_driven_late`.
- **PO11** (a) `out_first_driven_late` is conservative: a constant past a folded loop counts as late even when the module agrees (`pulse_plain`). (b) CDS §9.3. (c) DECIDED (conservative). (d) —

## 9. Memory (M1–M16)

- **M1** (a) `Memory<T,R,W,D,READ_LAT,WRITE_LAT>` has R read and W write ports, latencies ≥ 1, `WriteMode::{ReadFirst, WriteFirst}`, `ReadMode::{Sync, Async}`, and is fully pipelined. (b) SS "Memory". (c) DECIDED. (d) `copper-core/src/memory.rs`.
- **M2** (a) A read is staged: `read(addr)` presents an address, the result appears at the clock edge, and it is observed after the tick via `is_ready()`/`data()` (not async). (b) SS ibid. (c) DECIDED. (d) —
- **M3** (a) One access per bus per cycle: two accesses conflict iff one reaches the other without crossing an edge; exclusive arms are a mux. (b) SS ibid. (c) DECIDED/pinned. (d) `Cfg::check_memory_staging`.
- **M4** (a) Observing `data()`/`is_ready()` on an unstaged port, or before the producing tick, is an error. (b) ibid. (c) DECIDED/pinned. (d) same.
- **M5** (a) No access inside a tick-free nested loop. (b) ibid. — "Not pinned by a dedicated test". (c) DECIDED; not pinned. (d) same.
- **M6** (a) No plain `Out` driven from a read result in a multi-phase module; single-phase is fine. (b) ibid. (c) MEASURED. (d) `Cfg::memory_result_drives_plain_out`.
- **M7** (a) The memory rules run on the source in both front-ends with no opt-out. (b) SS memory + parity. (c) DECIDED. (d) —
- **M8** (a) A memory staging is a commit (latched at the closing edge). (b) DT §2.1. (c) DERIVED. (d) `derivation_facts`.
- **M9** (a) `q = data()` is a trailing commit at the producing edge. (b) DT memory row. (c) DERIVED/MEASURED. (d) —
- **M10** (a) Denotation: a ROM gives obs N = rom[addr_N] with no warm-up; a RAM has a ReadFirst collision, one-edge write visibility, and holds when unstaged. (b) DT memory row; CDS §5. (c) MEASURED (m3). (d) `tests/cycle_dataflow_memory_derivation.rs`.
- **M11** (a) A received `Memory` parameter becomes ports per used port (`rd addr/data`, `wr en/addr/data`); the child keeps all timing and the owner supplies the continuous read plus a guarded non-blocking commit. (b) RMA §2–3. (c) DECIDED 2026-08-27. (d) `chir_lower::received_memory_decls`, `vlir_lower` `lower_mem_decls`, `mem_write_commits`, `lower_to_vlir`, `mem_net_defaults`.
- **M12** (a) The collision policy belongs to the owner; the owner's pair gives ReadFirst, and WriteFirst adds a forwarding mux on the owner's side. (b) RMA §2. (c) DECIDED. (d) —
- **M13** (a) `M_ADDR_W` is a module parameter defaulting to 1; only used ports get a bus; there is no read enable. (b) RMA §3. (c) DECIDED. (d) `addr_param_name`.
- **M14** (a) `WRITE_LAT` must be 1 for a received memory; any `READ_LAT` ≥ 1 works. (b) RMA §3. (c) DECIDED (refusal). (d) `received_memory_decls`.
- **M15** (a) Memory, `RegOut` and synchronizers break combinational edges. (b) SS comb loops; LSS. (c) DECIDED. (d) —
- **M16** (a) Design questions about memory (latency modelling, interface, simultaneous ports). (b) MEMMD whole file — "Would simulating latency be useful?" (c) OPEN (questions only, no semantics). (d) —

## 10. Combinational modules (CM1–CM6)

- **CM1** (a) A combinational module has one instant; its value is the last write in program order, matching SV's blocking `always_comb`. (b) CDS §10.12. (c) DECIDED. (d) —
- **CM2** (a) A partial output in a combinational module is a latch and is refused; both front-ends use the shared rule. (b) IMPL item 2; CDS §10.12. (c) DECIDED. (d) `Cfg::build_combinational`, `check_definite_assignment` (also in `vlir_for_target`), `vlir_lower::check_no_latches`.
- **CM3** (a) A default write followed by a conditional override is legal; the unconditional write stays in `always_comb` when the port is also nested. (b) CDS §10.12. (c) DECIDED 2026-11-02. (d) `lower_comb`, `nested_port_drives`.
- **CM4** (a) A port write of a bare variable of a different width is width-cast. (b) CDS §10.12. (c) DECIDED. (d) `expr_width`.
- **CM5** (a) A combinational body runs once per phase under the topological pass. (b) LSS sub-questions. (c) DECIDED. (d) `delta_yield`.
- **CM6** (a) A sequential `Out` may hold, so "assigned on all paths" applies only to combinational modules. (b) IMPL item 2. (c) DECIDED. (d) —

## 11. Synchronizer / CDC / multi-domain (X1–X9)

- **X1** (a) A well-formed multi-clock design is correct under any relative tick interleaving or rate, provided every crossing goes through a synchronizer. (b) SS "Multiple domains". (c) DECIDED (informal invariant). (d) —
- **X2** (a) For a fixed tick schedule, results are poll-order independent. (b) ibid. (c) DECIDED. (d) per-domain keying.
- **X3** (a) Across rates, functional correctness is preserved (monotone, no corruption, eventual propagation), but not the exact trace. (b) ibid. (c) DECIDED; checked on 2:1/3:1/1:1 only. (d) `examples/cdc/two_domain_hierarchy.rs`.
- **X4** (a) An unsynchronized crossing is rejected by phantom domain types and by the transpiler's call-site check. (b) SS ibid. (c) DECIDED. (d) `copper-core/src/cdc.rs`, `chir_lower::lower_structural_inst`.
- **X5** (a) `sync_2ff` has one destination cycle of observable latency, is independent of the source rate, drops a pulse that falls entirely between edges, and is two flops. (b) TCM §5. (c) MEASURED. (d) `tests/cdc_synchronizer_anchor.rs`.
- **X6** (a) `E_tick` edges are labelled with the clock receiver identity. (b) SS "The graph". (c) DECIDED. (d) `Cfg`.
- **X7** (a) There are no cross-domain comb paths; other-domain tasks are quiescent sources. (b) LSS sub-questions. (c) DECIDED. (d) —
- **X8** (a) Independently ticking domains have no defined phase relationship. (b) SS ibid. (c) DECIDED. (d) —
- **X9** (a) A synchronizer's trailing `ff2 = ff1` must be a flop. (b) SS trailing. (c) MEASURED. (d) `src/sync.rs`.

## 12. Structural / hierarchy (H1–H7)

- **H1** (a) `#[hardware(structural)]` is a pure hierarchy (clocks in, no `always_ff`/loop/tick) that instantiates clocked children; it is transpile-only, emitted inert and not spawned in sim, and needs at least one clock. (b) IMPL item 4. (c) DECIDED; sim-as-unit (4a) DEFERRED. (d) `HardwareMode::Structural`, `emit::structural_body`.
- **H2** (a) `wire::<T,D>(init)` declares a net; `n.0` is the driver and `n.1` the observer, giving one SV net. (b) IMPL item 4. (c) DECIDED. (d) —
- **H3** (a) A hierarchy is co-emitted deepest-first. (b) ibid. (c) DECIDED. (d) `transpile_source_hierarchy`.
- **H4** (a) An array port is a packed 2-D `[ELS-1:0][W-1:0]` with direct element select; nested array ports are refused. (b) APA §5, §7. (c) DECIDED 2026-08-24. (d) `CHIRType::Array`, `outer_dim`, `emit.rs`.
- **H5** (a) The harness cannot drive ports wider than 64 bits. (b) APA §3. (c) limitation. (d) `logic_vec_to_int`.
- **H6** (a) Keyword port names are legalized (`event` → `event_sig`). (b) CSW §0. (c) DECIDED. (d) `legalized_port_name`.
- **H7** (a) A dual-clock async FIFO anchor was never built. (b) IMPL 4b. (c) DEFERRED. (d) —

## 13. Scheduling, comb loops, CFG (SC1–SC20)

- **SC1** (a) A well-formed design simulates bit-identically under any poll order. (b) SS "Poll-order". (c) DECIDED/pinned. (d) `tests/poll_order_fuzz.rs`.
- **SC2** (a) Levelized topological scheduling is the default and ignores `PollOrder`; fixpoint is kept permanently as the oracle; `COPPER_SCHEDULER` overrides. (b) SS; LSS. (c) DECIDED. (d) `SchedulerMode`, `poll_tasks_levelized`, `env_default`.
- **SC3** (a) Dependency edges are combinational only, from plain-`Out` writes; port kind is the discriminator. (b) LSS. (c) DECIDED. (d) `WireKind`, `DirtyHandle`, `compute_scc_plan`.
- **SC4** (a) A wire is a shared cell identified by its pointer; spawns declare the wires they read. (b) LSS. (c) DECIDED. (d) `WireId`, `spawn_wired`/`spawn_untracked`/`spawn_child`.
- **SC5** (a) `comb_cycles` returns SCCs of size ≥ 2, statically; detection is not rejection. (b) SS; LSS. (c) DECIDED. (d) `HardwareExecutor::comb_cycles`.
- **SC6** (a) A convergent loop is legal and iterated to a fixpoint; a non-convergent one panics after `OSCILLATION_THRESHOLD` (20) and names the SCC; `MAX_DELTA_CYCLES` is 1000. (b) SS; LSS. (c) DECIDED (the one runtime-enforced rule). (d) `iterate_scc`.
- **SC7** (a) Intra-module comb cycles are unexpressible (Rust rejects use-before-def). (b) SS. (c) DECIDED. (d) —
- **SC8** (a) A plain `Out` holding a registered value inside feedback is classed combinational and iterated. (b) LSS residual. (c) accepted limitation. (d) —
- **SC9** (a) Both schedulers must agree on every wire after every phase. (b) SS checks. (c) MEASURED. (d) `tests/levelized_differential.rs`.
- **SC10** (a) CFG: statement-granular nodes plus loop head H; `E_comb` and `E_tick` edges; fall-off and trailing ticks are back-edges to H. (b) SS "The graph". (c) DECIDED. (d) `Cfg::build`.
- **SC11** (a) Delete every `E_tick`; the subgraph reachable from H must be acyclic. This is a compile error in both front-ends. (b) SS "Reachability". (c) DECIDED. (d) `Cfg::check_reachability`.
- **SC12** (a) The fall-through is split into `AfterTick` (`pc = head`) and `ZeroTime` (goto marker). (b) SS ibid. (c) DECIDED. (d) `control_extract::lower_into`, `goto_marker`, `splice_zero_time_gotos`.
- **SC13** (a) A tick-bearing nested loop is folded into a boundary node in the parent and checked recursively in its own sub-CFG; a possible zero-iteration exit does not make the outer loop look tickless. (b) SS "Nested loops". (c) DECIDED. (d) `Builder::nested_loop_cfg`.
- **SC14** (a) A tick-free nested loop is combinational (unrolled) and exempt from must-tick. (b) ibid. (c) DECIDED. (d) —
- **SC15** (a) A tick-bearing nested loop is lowered as a repeating wait. (b) ibid. (c) DECIDED. (d) `control_extract`.
- **SC16** (a) Uneven per-branch tick counts are legal. (b) SS. (c) DECIDED. (d) —
- **SC17** (a) Control extraction turns a body with branch- or loop-nested ticks into a single-tick `match pc` FSM; the `pc` states are the phases. (b) TMU §1. (c) MEASURED. (d) `control_extract`, `clock_phase_count`.
- **SC18** (a) A phase is a Comb-connected component of the CFG. (b) SS `multi_phase_out_write`. (c) DECIDED. (d) —
- **SC19** (a) Phase-sensitive checks belong on the source, not downstream of extraction (a structural guard enforces this). (b) TMU §2. (c) DECIDED. (d) `phase_sensitive_checks.rs`.
- **SC20** (a) A body that extraction handles only because of a `continue` counts as linear for the rules. (b) CDS §9.3. (c) DECIDED (refusing side). (d) —

## 14. Refusal rules (RR1–RR46)

- **RR1** `multi_write_collapse`: a combinational `Out` written on both sides of one bare tick, after a leading read that comb-reaches the pre-tick write (three necessary conditions), in top-level and nested loops. Both front-ends, no opt-out. SS. DECIDED. `Cfg::multi_write_collapse`.
- **RR2** `check_nested_loop_shape`: tick-first wait refused. SS. DECIDED.
- **RR3** `check_awaits`. SS. DECIDED.
- **RR4** `check_reachability`. SS. DECIDED.
- **RR5** D1 clause (A1): head segment, body with registers, mixed anchors (a read comb-reaches some drive and some drive has a read-free path from the head) and a path-dependent drive; exempt is a single every-path drive forwarding no register its read-free path assigns (`d3a`). SS; CDS §10.5. Narrowed 2026-10-30. `Cfg::unprotected_pretick_out_write`, `Cfg::read_free_path_reaches`.
- **RR6** D1 clause (A2): mixed anchors with the port written on every path. CDS §10.5. DECIDED; not applied on the linear multi-tick route since 2026-11-02 (§10.9).
- **RR7** D1 clause (B): a drive no read reaches, under a branch whose test reads a register assigned earlier on the path. CDS §10.3. DECIDED; recorded as an emission limitation with a fix direction. `Node::guards`.
- **RR8** D1's register-free exclusion: those shapes belong to other rules. CDS §10.3. DECIDED.
- **RR9** D1's constant-write clause (`written_on_all_paths`). PAG §5.5. RETIRED 2026-10-28.
- **RR10** D1's "unprotected register assignment" conjunct (as landed 2026-08-21) and its 2026-08-26 read-preceded register clause (`leading_read_reaches`). PAG §7, §5.7. RETIRED/superseded 2026-10-28.
- **RR11** `unprotected_trailing_out_write`. PAG §5.4; SS. RETIRED 2026-10-23.
- **RR12** `multi_phase_out_write` clause 1: the linear route. CDS §9.3. DECIDED.
- **RR13** Clause 2: an input read anywhere. RETIRED 2026-10-26.
- **RR14** Clause 3: a register-free body with reads. DECIDED.
- **RR15** Clause 4: extraction route, body with registers; a write comb-reached by an `In` read in its phase that the sim runs at the opening on some path (no deferred read reaches it, or a read-free path from the phase entry). CDS §10.5. Narrowed 2026-10-30. `Cfg::read_free_path_from_phase_entry`.
- **RR16** Clause 4 extended (path-dependent half only) to a single non-head phase on the extraction route; the immediate-read half stays gated on two phases. CDS §10.6. DECIDED 2026-10-30.
- **RR17** Clause 5: a wait loop's read before an opening write. CDS §9.3 — "Unmeasured; refused by construction". unmeasured.
- **RR18** `linear_out_write_anchor_mismatch` scope: `!has_nested_tick`, more than one tick, register requirement dropped 2026-11-02; a register-free body's own-operand read does not count as deferred. CDS §10.7–§10.9.
- **RR19** Linear clause A: a port written unconditionally and conditionally in one segment (two drivers). §10.9.
- **RR20** Linear clause B: a conditional drive after the last tick. §10.9.
- **RR21** Linear clause C, wire port: refused iff some path reaches it with no deferred read. §10.9.
- **RR22** Linear clause C, otherwise: an input read reaches the drive while the sim runs it at the opening (passthrough under a read-free guard; a read-free path), or no read reaches it and its guard tests a register assigned earlier in the segment. §10.9.
- **RR23** Linear clause 2, unconditional wire: refused iff some path has no deferred read, except an unconditional constant wire in the head segment. §10.8.
- **RR24** Linear clause 3, continuous assign: the operand reads an input; or a deferred read reaches it and the operand is a register assigned outside its segment or after it; or a constant outside the head segment. §10.8.
- **RR25** The phase-gated comb latch clause (§10.8 clause 1). Superseded by §10.9 (latch fixed).
- **RR26** `pretick_out_write_before_update`, pre-tick value clause: write between a leading read and the update of a register the write reads. PAG §5.6. DECIDED 2026-08-26; over-refuses held ports (`e18j`, "left to the user").
- **RR27** V8 trailing clause: written before the update of a register it reads in the trailing segment. 2026-08-27.
- **RR28** V8 single-tick trailing-update clause: one top-level tick, every-path head port behind a leading read, value or guard reading a register the trailing region assigns. CDS §10.10.
- **RR29** V8 guard edition: single tick, leading read reaches, the guard reads (through temporaries) a register assigned after the guard, every path of the head. CDS §10.11.
- **RR30** V8 scope: head plus trailing only; the middle segment was unexamined (R6) and is covered by `e10` (agrees) and `e14j` (linear rule). PAG §5.6; CDS §10.7.
- **RR31** `trailing_out_reads_input`: a plain `Out` after the last tick whose value or guard reads an `In` directly at an immediate site, in a body crossing more than one edge; single tick is exempt. SS; CDS §9.4, §10.8.
- **RR32** `trailing_regout_reads_input`: a `RegOut` in the trailing region read from an `In` directly at an immediate site; value only. SS trailing.
- **RR33** `out_first_driven_late` (see PO10). Opt-out honoured.
- **RR34–RR36** `check_memory_staging` (three clauses) and `memory_result_drives_plain_out` (M3–M6). No opt-out.
- **RR37** `check_register_initializers` (PO4).
- **RR38** `check_definite_assignment` (CM2).
- **RR39** CDC phantom types and the call-site check (X4).
- **RR40** Refused one IR late by `vlir_lower` or Verilator `-Wall`: multiply driven (`e7`), `LATCH`, `MULTIDRIVEN`, `BLKANDNBLK`, `WIDTHTRUNC`. CDS §9.3, §10.8–§10.10.
- **RR41** Received memory with `WRITE_LAT ≠ 1`; nested array ports (M14, H4).
- **RR42** The `allow_pretick_alignment` opt-out silences the error, not the detection, for the pre-tick rules; `multi_write_collapse`, the memory rules and the shape rules have none. SS parity; PAG header.
- **RR43** Front-end parity: a module that compiles also transpiles, or both front-ends refuse it; the audit scans clocked modules only. SS. `front_end_parity.rs`, `opts_out_of_pretick_alignment`.
- **RR44** The transpiler keeps defence-in-depth copies of the refusals (`chir_lower`, `control_extract`). SS.
- **RR45** Unmeasured shapes fall on the refusing side by construction (a sound-but-incomplete heuristic). CDS §9.3, §10.3; PAG §10.5.
- **RR46** Every rule is landed only with exact-set corpus pins and flipping unit pairs; "If any row misses, the model is wrong and nothing lands." CDS §10.7.

## 15. Agreement claim & how it is checked (A1–A16)

- **A1** The denotation is normative; the sim and the transpiler are implementations with obligations, and the model assigns which side failed. CDS decision 1.
- **A2** The bar is sim ≡ SV under a testbench; every accepted program preserves sim ≡ synth. SS multi-write; SS wait loops.
- **A3** Sim and transpiler agree on registers and well-formedness by construction (one CFG). SS CFG.
- **A4** Independent hand-written or BaseJump Verilog is the only timing adjudicator; the transpiler is never its own oracle. SS checks; TCM.
- **A5** All seven timing patterns are anchored (multi-tick, mid-phase read, Moore/Mealy, memory, CDC, variable iteration, trailing region). TCM.
- **A6** Corpus sweep: one case per module in `examples/` and `tests/fixtures/`; four seeds × 200 cycles plus first-cycle sequences; truth table for combinational modules; fresh state per sequence. CSW §3.2.
- **A7** `differential_only` records the sim's values as expected. CSW §3.3.
- **A8** The randomized power-on switch (`COPPER_VERILATOR_X_INITIAL`) is optional, not the default. CSW §3.3.
- **A9** The four tables `SKIP`, `PARAMS`, `RESET`, `REFERENCE`; guards G-D and G-E. CSW §3.4–3.5.
- **A10** The sweep cannot see shared misunderstandings or rare long sequences. CSW §1.
- **A11** A failure in the reference leg only means sim and transpiler are both wrong. AAM.
- **A12** Denotation anchors (paper-derived traces matched by both). SS; DT m3/m4.
- **A13** `sv-baseline` is an author's tool, not a gate; byte-identity claims are one-off measurements. SS; PIS A.
- **A14** Exact-set corpus pins fail in both directions. PAG.
- **A15** Anchors are re-verified, never re-blessed. CDS §7.
- **A16** Verilator `-Wall` warnings are fatal. AAM.

---

# CONTRADICTIONS AND SUPERSESSIONS

1. **`has_nested_tick` deleted vs still used.** Deleted: PAG §5.4 ("the `has_nested_tick` field are deleted"), PIS note C, CDS §5 trailing row (2026-10-23). Still used: CDS §9.3 clause 1 ("`Cfg::has_nested_tick` is false"), §10.6, §10.7 ("`!has_nested_tick`"), §10.8, §10.10, and the SS rule list. No passage reconciles them.
2. **Trailing `RegOut` commit edge.** SS denotation: "Commits — … `RegOut` writes … a *trailing* region's commits anchor to the edge that **opens** its cycle". DT §1 and CDS §2 say the same. SS trailing section and CDS §5 PC#2 row (2026-10-23): "commits at the *next* edge"; "the paper assignment was wrong for `RegOut`". The later measurement wins; the SS denotation paragraph is unedited.
3. **The rule list in SS is stale.** SS "The residue is five rules" still lists `unprotected_trailing_out_write` with live-sounding text ("not landed as of 2026-09-01", a pin "as of 2026-09-01"), although SS itself says it is retired. SS parity: "The six pre-tick alignment rules" omits `linear_out_write_anchor_mismatch`; CDS §10.7 says the parity audit gained it "as its seventh entry". PAG header: "four rule functions … four exact-set corpus pins"; its own table has six live rows. TMU §4 also says "residue of five rules".
4. **D2 passthrough scope.** SS: "in a *segment* that assigns **no register**". CDS §9.1 and §10.1: "a **register-free** body"; "In a body with registers every head-segment `In` read is deferred". CDS §10.8 (c): only the "direct argument of `o.write(…)`". IMPL item 3: "Deferred … iff a tick occurs *after* it", with no exception. Stale.
5. **Edge numbering.** CDS §1 numbers edges "1, 2, …". SS "Startup" ("edge 0's post-edge settle"), CDS §10.7 ("its closing edge is edge 0") and §10.10 ("before edge `k`, observation `k`") are 0-based.
6. **Two definitions of "segment".** SS/CDS §2 merge trailing statements with the head. CDS §10.7: "'Segment' is the Comb-component with the loop head cut out, so the trailing statements are a segment of their own".
7. **Non-convergent comb-loop panic.** SS says it "is not pinned by a test as of 2026-09-01". LSS phase 5 lists `levelized_oscillating_scc_trips_threshold` and `tests/simulator_corner_cases.rs::non_convergent_combinational_loop_is_detected`.
8. **`initial` blocks.** AAM says "the transpiler emits no `initial` either" and "No `initial` block for memories" is required, and that the harness passes no `--x-initial`. Superseded by SS power-on (2026-10-29) and CSW §3.3 (the `ram_read_first` reference gained an `initial` on 2026-10-30; the X-initial switch exists). AAM was never updated.
9. **`lfsr` initializer.** CSW §0 (2026-10-28) says whether a Rust initializer reaches the SV is "not settled". Superseded the next day in CSW §0 (2026-10-29) and SS.
10. **CDS §4's constant-write bullet** ("conditional constants are mixed after all (`pc_arm_write` / `pc_arm_toggle` / `branch_merge_explicit`)") is contradicted by the later measurement that all three agree, with the explanation re-labelled "the proxy at work" (PAG §5.8; CDS §9.4, §10.5). There is no dated note in §4 itself.
11. **CDS §4 "Landed" note** says D1 keeps "the conditional/constant hold clause (`written_on_all_paths`) unchanged". That clause was retired on 2026-10-28.
12. **CDS §5 table D1 row** ("retained for the path-dependent boundary", 2026-08-26) was later re-derived into (A)/(B) without an annotation on the row.
13. **CDS opening framing.** "the cycle … is decided by *where the task parks* … the defect" is superseded by its own dated note: "the executor's parking rule was never the defect".
14. **CDS §3 executor cell.** "Run the segment … **no barrier**" for opening-anchored segments vs CDS §6 and PIS §0: "Not done, and not needed"; the read-site barrier is correct.
15. **PAG §5.3 "THE GENERAL RESULT".** "The simulator must be made uniform first" is superseded by PIS §0 and PAG §9 (paired fix codegen-only, executor untouched).
16. **PAG §5.6.** V8 was "Enforced in the macro … not run by the transpiler"; superseded by front-end parity on 2026-10-23 (SS).
17. **PAG §5.5.** "The clause is retained; re-deriving it … is a decision of its own"; superseded by the 2026-10-28 retirement in the same doc.
18. **PAG §4.** Q2 says "`accum_2` itself is not yet measured"; Q5's note says "**`accum_2` does not diverge** … passes". Both are in the same section.
19. **PAG §4 Q5.** "that is still its whole `EXPECTED_MULTI_PHASE` today" ({`pulse_plain`}); now eight probes and `pulse_plain` is out (PAG table; CDS §9.3).
20. **Pin drift.** SS: `EXPECTED_LATE_CONSTANT = {pulse_plain}`; CDS §10.8 adds `e15p`, `e15z`, `e15aa`. SS: `EXPECTED_FLAGGED` = `w4` + six `d*`; CDS §10.10 adds `e17c`, `e17e`, `e17f`. SS: V8 "listed set as of 2026-09-01: [4]"; PAG lists additions including `vr_sink_mealy` and `vr_skid`, which no doc derives.
21. **SS's agreement claims vs recorded gaps.** SS says "every *accepted* program preserves sim ≡ synth" and that "every corpus module carries a derived disposition". DT §4b says modules added since 2026-08-26 "carry only the bin's first-cut label", and CDS §10.8 (b) records an unrefused unmeasured shape.
22. **SS on trailing ordering.** SS says C-before-A is "Unobservable through ports — `multi_write_collapse` rejects a port written on both sides of a tick". The same SS says a write-straddle without a leading read (`counter`, `rx_dv`) is "unaffected", i.e. allowed.
23. **DT §4b taxonomy.** "an every-path (or held) constant" is listed as legal, while held constants on a path-dependent boundary diverge (`d1f`, CDS §10.2). The taxonomy pre-dates this.
24. **PIS stale status.** §6 items 3–4 say "C has not landed" and "[phase E] not measured as of 2026-09-01"; the same doc's header says C landed on 2026-10-23 and E was measured.
25. **TMU §1b.** "What is actually left is D1's phase alignment" (open) is superseded by the dissolution (CDS §4, §10).
26. **CDS §10.7 operand-kind statement** was narrowed by §10.8 (form-by-shape; `e15fr`, `e15af`, `e15ag` admitted). §10.8 clause 1 (latch refused) was superseded by §10.9. §10.9 (a) was closed by §10.10; §10.10 (a) by §10.12; §10.10 (b) by §10.11.
27. **CDS §8-3.** "No initial-value rule was added" (about `Out` startup) sits beside SS's later register power-on rule (2026-10-29). Different objects, but SS never states an `Out` startup rule.
28. **IMPL item 2.** "`split_at_ticks` is not yet retired … codegen still recomputes" is marked SUPERSEDED by d7b2483; the dated note resolves it.

# GAPS

1. **Dual-consumer read** (an input feeding both a commit and an `Out`): "**Open as a refinement**" (CDS §8-1; DT §4b).
2. **`ram_prewrite`**: "**open** (no behavioral measurement)", and it does not transpile (DT §4).
3. **`branch_on_updated_reg_bit`**: "a derivation row is owed — not written" (DT §4b).
4. **Tick-free nested loop memory access**: "Not pinned by a dedicated test" (SS memory).
5. **`multi_phase_out_write` clause 5**: "Unmeasured; refused by construction" (CDS §9.3).
6. **Mixed wire+register operand**: "no probe measured the mixed operand on its own" (CDS §10.7).
7. **`RegOut` with an immediate read in a trailing guard**: "unmeasured and unrefused" (CDS §10.8 (b)).
8. **`MULTIDRIVEN` lint-off hiding an assign beside a non-blocking driver**: "stands" (CDS §10.9 (b), §10.10 (c)).
9. **Extending the hold form to wire-valued ports**: "unmeasured" (CDS §10.9 (c)).
10. **The V8 value clause over-refuses held ports (`e18j`)**: "a legality decision … left to the user" (CDS §10.11; PAG §5.8).
11. **D1 (B) is an emission limitation**: the fix direction is known, "the rule refuses the shapes until one lands" (PAG §5.8; CDS §10.3).
12. **The forwarded guard in a middle phase on the extraction route** was never measured: (B) is head-only and §10.9 covers only the linear route.
13. **The linear-route mux + hold form** is "not recorded in `hold_form_ports` … noted, not changed" (CDS §10.8).
14. **Memory semantics without a timing meaning.** `ReadMode::Async` is named in SS and given no meaning. `READ_LAT`/`WRITE_LAT` > 1 are allowed but never derived (m3 covers latency 1). `WriteFirst` is only named. Write–write same-address collisions are never specified (only the fixture name `ram_wf_priority` appears). MEMMD is questions only.
15. **Structural modules** have no simulation meaning ("not spawned in sim"; 4a deferred). Which rules the **synchronizer** macro arm runs is unstated: the parity text names only "the macro's sequential arm".
16. **Multi-clock**: the property is informal (monotone, eventually), checked on three fixed ratios, with "not a randomized fuzzer"; the async FIFO was not built.
17. **A tick-bearing loop that runs zero iterations at runtime**: folding into a boundary node is called "rejection-sound", but nothing states the meaning if `for _ in 0..n { tick }` has n = 0 dynamically. Prost, by contrast, requires one iteration (PAG §10.5).
18. **The observable value before the first edge, and the value of a pre-loop `Out` write** are undefined; only register initializers have a stated meaning.
19. **Several writes to one `RegOut` before the same edge**, and the ordering of several plain-`Out` writes in one segment, are only implicit (last write wins, by §10.10 and §10.12 for comb).
20. **X semantics**: the sim carries X and Verilator reads 0; division by zero (`mod8`) is X vs 0 and handled by a SKIP row only; there is no reset semantics in the language (`RESET` is harness-only).
21. **"Restore phase after tick resolution"**: "never taken up" (IMPL 1b).
22. **Mid-cycle input changes**: the agreement claim holds only under drive-then-clock; asynchronous inputs are undefined (SS wait loops; DT §4b).
23. **The parity audit scans clocked modules only**, which is how a combinational parity break went unseen (CDS §10.12). `register_reconciliation` scans `tests/fixtures` only. The sweep excludes `src/`, inline test modules and memory-receiving modules.
24. **R1 tension**: R1 demands "Zero false positives", yet the docs keep refusing agreeing shapes by design (`pulse_plain` by `out_first_driven_late`, `e18j`, and `e15ac`/`e15ad` until §10.9).
25. **PAG Q4**: for the sticky flag, "the general rewriting rule is unstated".
26. **Opt-out**: "must never be reached for in a real design"; nothing enforces this.
27. **Constructs accepted without a stated timing meaning**: `ReadMode::Async`; latencies above 1; `WriteFirst`; `continue`/`break` semantics beyond CFG modelling; word-indexed array registers (the CPU register file, `b70f2a1`); the `wire::<T,D>(init)` initial value in structural parents; `delta_yield`; host-side arbitrary Rust in a body (IMPL item 7 mentions it); file-scope `const` widths and `localparam`.

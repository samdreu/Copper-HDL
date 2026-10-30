# Cycle-dataflow semantics — the normative timing model for the async-await surface

**Status: DERIVED, VALIDATED, AND IMPLEMENTED — residue recorded (status as of
2026-09-01).** Phase 1 (the derivation table, `DERIVATION_TABLE.md`) is complete:
every corpus module carries a derived disposition and all four owed measurements
(m1–m4) confirmed their derivations on the first run — including one silent,
unguarded divergence the model predicted from first principles (V8a, guarded
2026-08-26 as the fifth pretick rule, `Cfg::pretick_out_write_before_update` in
`copper-analysis/src/cfg.rs`). The paired implementation was scoped and executed
the same day (`PAIRED_IMPLEMENTATION_SCOPE.md`, commit 7abfd98): phases A, B and D
landed — opening-prefix plain-`Out` drives emit their forwarded form (the V1 family
is *legal and agreeing*, measured at zero live-module SV cost), and D1 is narrowed
to the derived path-dependent-boundary rule. Phase C landed 2026-10-23 as an
emission form and retired the trailing rule; **phase E was derived and measured
the same day (§9 below)** — `multi_phase_out_write` is **narrowed, not retired**:
the input-free and all-closing classes are admitted, the mixed, register-free and
linear classes stay refused, and the battery found two single-phase divergences
that no rule guards (§9.4; both closed 2026-10-26). **D1 was re-derived 2026-10-28
(§10)** against the arm-granular opening walk: the rule now refuses exactly the two
head-segment hazards the model derives (mixed anchors, a forwarded guard), its
constant-write clause and "unprotected assignment" conjunct are gone, and four
measured divergences it had missed are refused. **The drive-level mux + hold form
landed 2026-10-29 (§9.4):** phase C's decision is per drive instead of per port,
so a MIXED-anchor plain `Out` realizes both anchors — `branch_merge`'s `tail_o`
(the sweep's last measured divergence, its `SKIP` row gone), `d2a`, `e4` and `e6`
agree, at an sv-baseline cost of exactly the seven mixed-anchor hold-form ports
and no rule change that day. **Both mixed-anchor clauses were narrowed
2026-10-30 (§10.4–10.5)** to the one mechanism that still diverges — a drive
whose anchors differ between the two sides (path-dependent, or fed only by an
immediate read) and the every-path mixed mux — so `d2a`, `branch_merge_explicit`,
`e4` and `e6` compile without an opt-out; eleven new probes predicted and matched
first run, one of them (`e11b`) a divergent shape clause 4 had admitted. The executor was **not** changed — scoping showed it already implements
the model (`PAIRED_IMPLEMENTATION_SCOPE.md` §0). The normative statement of the
model now lives in `SYNCHRONOUS_SEMANTICS.md` (rebased 2026-08-27, commit c1bf21f);
this document is the derivation record. Decided 2026-08-26 as:

1. **The denotation is normative.** The per-cycle value function defined here is the
   semantics; the simulator and the transpiler are both *implementations* of it, each
   with stated obligations. A sim ≠ SV disagreement is no longer adjudicated case by
   case — the model assigns which side failed its obligation. Independent hardware
   references validate the *model* (does the denotation match real hardware?), not one
   implementation against the other.
2. **Registers stay plain locals.** No current/next split for locals (the 2026-08-25
   standing decision holds); "a value live across a tick is a register" is kept.
3. **Derive on paper before touching code.** This document re-derives every existing
   timing rule and divergence from the model, and every derivation must be checked
   against a recorded measurement (or get a new one) before any executor/codegen
   change is scoped. This partially re-opens the 2026-08-25 "restrict, don't grow"
   decision **for the D1 family specifically** — the goal is that sim and transpiler
   agree on timing and registration *by construction*, with refusal reserved for the
   genuinely unrealizable residue.

**The defect this replaces**, in one sentence: today the cycle a statement's effects
land in is decided by *where the task parks* (barrier vs tick), which is decided by an
*incidental* property (whether a leading `In` read happens to appear) — so the
simulator is not internally uniform (`PRETICK_ALIGNMENT_GUARDRAIL.md` §5.3's general
result), no single lowering can match it, and every silent divergence to date has been
a patch surface over that non-uniformity.

> *Dated note (2026-08-26, after F3's revision — `PAIRED_IMPLEMENTATION_SCOPE.md`
> §0):* the paragraph above is the pre-validation framing. Scoping against the V8
> measurements showed the read-site barrier **is** the region boundary the model
> needs, so the executor's parking rule was never the defect; the defect was on the
> emission side (unforwarded assigns for opening-prefix drives, fixed in phase B)
> and the genuine non-uniformity is the *path-dependent* boundary, retained as
> D1's residue.

Companions: `SYNCHRONOUS_SEMANTICS.md` (the reference — **rebased onto this model
2026-08-27**: denotation first, rules as derived residue), `PRETICK_ALIGNMENT_GUARDRAIL.md`
(the measured evidence this document derives from).

> **Phase 1 is COMPLETE: `DERIVATION_TABLE.md`** (mechanical columns regenerate via
> `cargo run -q -p copper-codegen --bin derivation-audit`, source
> `copper-codegen/src/bin/derivation-audit.rs`, facts from
> `Cfg::derivation_facts`). Three refinements the derivation produced, now stated
> normatively in `SYNCHRONOUS_SEMANTICS.md` and recorded in §2 below as a dated
> note: **anchoring is per *region* (pre-tick vs trailing), not per
> Comb-component** — the trailing region always commits at its opening edge and is
> opening-anchored, which is why §5.4's two regions could never share a rule; §3's
> anchor classification counts **memory stagings, `RegOut` writes, and the implicit
> `pc`** (a control-steering read) as commits; and §4 gains the measured
> **commit-frontier taxonomy** (constants, committed reads, write-after-update; the
> between-read-and-update position is divergent and guarded). Validation record: F1 — the forwarded-emission dissolve set is
> **empty on the non-witness corpus**, so the paired fix changes no live module's
> SV; F2 — the V8 battery, predicted then measured, now the fifth pretick rule;
> F3 (as REVISED by the scoping, `PAIRED_IMPLEMENTATION_SCOPE.md` §0) — the
> read-site barrier is *correct* under the model (region-entry barriers are
> falsified by V8c), `probe_fsm` is a path-dependent boundary and stays refused,
> and **the executor needs no changes**; m2 — the sim already implements the
> model for trailing segments
> (codegen is the side to change); m3 — the memory family is anchored to the
> denotation itself; m4 — the uart sampling edges match the derivation exactly,
> and both `read-retime` flags were reporter blind spots, not retimings.

---

## 1. Instants, windows, and the invariant

Fix a clock domain. **Edges** are numbered `1, 2, …`; **cycle N** is the interval
`[edge N, edge N+1)`. The **observation instant** of cycle N is just after edge N's
post-edge settle (the harness convention: drive inputs, tick, observe — so "cycle N's
input value" `x_N` is the value driven *before* edge N and stable through cycle N
until `x_{N+1}` is driven late in it).

The simulator executes statements at *instants*; hardware evaluates continuous
functions. The whole reconciliation problem is window arithmetic:

> **The write-window invariant.** A sim port write executed at instant `t` is
> observed for the window `[t, next write)`. The emitted hardware for that port is a
> continuous function `e` of registers and inputs. The two agree iff, at every
> observation instant inside the write's intended window, the written value equals
> `e` evaluated at that instant's wire values.

Every rule in the D1 family is an instance of this invariant failing; every remedy
(`RegOut`, reorder, barrier) is a way of restoring it. The model below makes the
invariant hold by *construction* for accepted programs, instead of detecting its
violations one discriminator at a time.

## 2. The denotation

A sequential module's body is a top-level `loop`. A **segment** is a maximal
tick-free region of an execution (trailing statements merged with the head — decided
2026-08-25, unchanged). The segment that *opens* at edge N denotes cycle N's
computation. Its meaning:

- **Program order with forwarding.** Statements execute in source order; a local
  read yields the last value assigned to it in this segment, else the value the
  local's register committed at the segment's opening edge. This is what plain Rust
  does, so it is what "the async fn is just Rust" already promises — and it is what
  Prost, the only other coroutine HDL, lowers (§10.5). *This clause is the meaning of
  the source; both implementations owe it.*
- **Registers = the liveness rule** (defined-in-loop ∧ live-across-a-tick, with the
  back-edge clause). Unchanged, and already shared. **The authority obligation is
  DISCHARGED (2026-08-27):** `capture_frontend_ir` fills
  `FrontendModuleIR::registers` from `infer_registers` — the identical inference,
  on the identical input, the sim macro consumes — `transpile_fir` appends the
  names the FIR→FIR passes synthesize (`pc`, counters, hoisted locals, found by
  diffing the pre-loop `let mut` set across the passes), and `chir_lower`
  **consults** that authority instead of deciding syntactically
  (`copper-codegen/src/parser.rs::capture_frontend_ir`, `transpile_fir` in
  `copper-codegen/src/lib.rs`, the `let mut` arm of `copper-codegen/src/chir_lower.rs`;
  commit d7b2483, 2026-08-27). The change was byte-identical across the whole
  corpus under the `sv-baseline` snapshot/diff — measured once at commit d7b2483
  (195 modules at that commit; the snapshot lives in `target/sv-baseline/` and is
  not a regression gate) — which is the proof the old syntactic decider had only
  ever *coincided* with the inference;
  `copper-codegen/tests/register_reconciliation.rs::codegen_registers_match_shared_inference`
  remains the end-to-end oracle, now verifying plumbing rather than a coincidence. (`held_temp_trap`, the historical
  disagreement this bullet used to cite, had already been repaired by the
  2026-08-26 reassigned-wire fix.) Four degenerate unit fixtures that pinned the
  rival's behavior on never-touched locals were re-blessed with genuinely-live
  registers.
- **Commits anchor to the closing edge.** The segment's final forwarded values for
  register locals and `RegOut` ports commit at edge N+1. An `In` value that feeds a
  commit is sampled at the closing edge's pre-edge settle (`x_{N+1}`) — this is the
  anchored read-timing semantics (`det_010` vs `pattern_detector_010.sv`: the FSM
  samples at its own edge) and is **not** being changed.
- **Plain `Out` drives anchor to the opening edge.** A plain `Out` written in the
  segment is cycle N's combinational drive, observed from edge N's observation
  instant onward; unwritten-on-a-path = hold (the enabled-register idiom, unchanged).

The tension the model must resolve — and the denotational restatement of §5.1's
durable finding ("the pre-tick segment does two jobs") — is that **a segment has two
temporal anchors**: its commits belong to the closing edge, its comb drives to the
opening one. The current system picks *one* execution instant per segment (decided by
the incidental barrier) and patches the observable fallout. The model instead
classifies segments and assigns obligations per anchor.

> **Refinement absorbed from the derivation (2026-08-26, `DERIVATION_TABLE.md` §1;
> normative wording in `SYNCHRONOUS_SEMANTICS.md`):** the unit of anchoring is the
> *region*, not the segment. A segment divides at the read-site barrier into an
> **opening prefix** (statements before the first leading `In` read — executes at
> the opening instant on committed state) and a **closing suffix** (executes at the
> pre-edge settle on closing-edge samples and forwarded values); the **trailing
> region** (last tick → head) always commits at the edge that *opens* its cycle and
> is opening-anchored. Commits include register locals, `RegOut` writes, memory
> stagings, and the implicit state register a control-steering read feeds. That the
> barrier site *is* the region boundary was validated, not assumed: a region-entry
> barrier was proposed and falsified by V8c (`PAIRED_IMPLEMENTATION_SCOPE.md` §0).

## 3. Segment anchoring — derived, not incidental

**Definition.** A segment is **closing-anchored** iff some value it *commits*
(register or `RegOut`) depends on a same-cycle `In` read — that read must sample
`x_{N+1}`, so the segment's effects can only be finalized at the pre-edge settle of
the closing tick. Otherwise the segment is **opening-anchored**: nothing in it needs
the closing edge's input values, so it can execute at the opening edge's post-edge
settle — which is exactly the observation instant.

This replaces "does a leading read appear" (syntactic, incidental) with "does a
commit *depend* on a same-cycle input" (a data-flow fact of the source, computable on
the CFG both front-ends share).

**Obligations, per anchor:**

| | executor | transpiler (plain-`Out` emission) |
|---|---|---|
| **opening-anchored** | run the segment in the opening edge's post-edge settle — **no barrier** | emit the **forwarded** expression (`assign o = r + 1` for `r = r+1; o.write(r)`) — Prost's lowering |
| **closing-anchored** | barrier — run in the closing edge's pre-edge settle | a plain-`Out` write is realizable **only if its value is expressible over the commit frontier** (see §4); emit the **unforwarded** committed-register expression |

Why each cell, from the write-window invariant:

- *Opening-anchored + forwarded*: the sim's write executes at the observation
  instant with post-commit registers and cycle-N inputs on the wires; the forwarded
  assign evaluated there uses the same values. The window is the whole cycle
  (registers next change at edge N+1, when the next iteration re-writes; inputs are
  stable per the harness convention). Exact agreement. **Checked against
  measurement**: V1's sim trace `[2,3,4,…]` equals Prost-style SV exactly (§5.3
  table) — the sim was never wrong on V1; the *unforwarded* emission was.
- *Closing-anchored + unforwarded*: the sim's write executes at pre-edge N+1 with
  value `f(x_{N+1}, forwarded locals)` — and the forwarded locals *are* the values
  committing at edge N+1. So the written value equals the **unforwarded** expression
  evaluated just after edge N+1, and the window `[pre-edge N+1, …)` contains exactly
  the observations from N+1 on. Agreement — *iff* the written value reduces to
  committed state (§4). **Checked against measurement**: this is why V4, `lfsr`,
  `det_110101`, `shift_register` all agree today under (barrier, unforwarded)
  emission, and why Prost-style emission broke exactly them (§5.3).
- The two rejected blanket fixes are the two off-diagonal cells: always-barrier
  (§5.1) forced closing-anchoring onto opening-anchored segments (broke Moore
  outputs, 22 failures); Prost-everywhere (§5.3) forced forwarded emission onto
  closing-anchored segments (broke the barrier-protected majority). The pairing is
  what neither single-sided fix could be.

**Implementation (landed 2026-08-26, phase B of `PAIRED_IMPLEMENTATION_SCOPE.md`).**
The executor cell needed no change — the read-site barrier already realizes the
split (§0 there). The transpiler cell is the opening-prefix selection in
`copper-codegen/src/shir_lower.rs`: the `Forwarding` walk tracks whether an `In`
read has been seen in the segment (`forward_opening_drives` / `seen_in_read`), and
a `CHIRStmt::PortWrite` before any read takes its `edge_value` (the forwarded form
`SHIRPortDrive` already carried) as its continuous-assign form; trailing segments
are excluded. Pinned by
`tests/sequential_forwarding_divergence.rs::pre_tick_update_forwarding_agrees_end_to_end`
(V1 sim ≡ SV end to end) and `::d_narrowing_battery_verdicts` (V5, V7 agree; W4
diverges), and by the ui/pass case
`copper-macros/tests/ui/pass/pretick_alignment_ok.rs::forwarded_opening_drive`.

## 4. The commit-frontier condition — D1's family, derived

For a **closing-anchored** segment, a plain-`Out` write's value is computed from
`(x_{N+1}, r_N)` — a *mixed-generation* pair. A continuous assign can only ever show
same-generation pairs: at observation N it shows `e(x_N, r_N)`; at N+1,
`e(x_{N+1}, r_{N+1})`. So the write is realizable in hardware **iff its value is
independent of the mixed generation** — i.e. expressible over the commit frontier:

- the write equals a value **committing at N+1** (a register the segment assigned,
  forwarded) → emit the unforwarded register: window aligns at N+1. This is V4 /
  `lfsr` / the whole barrier-protected corpus. ✔ legal, and *why* it is legal is now
  stated instead of observed.
- the write is a **constant on every path** → generation-free. This derives the
  constant-write exemption *including* its 2026-08-25 all-paths narrowing: an
  unwritten path substitutes the held value, which is a different generation's
  function, so conditional constants are mixed after all (`pc_arm_write` /
  `pc_arm_toggle` / `branch_merge_explicit` — the measured set).
- anything else (a genuine Mealy function of the closing-edge input that is not
  committed) → **no continuous assign shows that value at any observation instant**.
  Unrealizable; refuse. This is the derived, positive form of what D1 + D2's original
  divergence + `memory_result_drives_plain_out` each caught a fragment of.

And **D1 itself dissolves**: the V1 shape is an opening-anchored segment (no commit
reads an input), so under the model it *has* a defined meaning — the forwarded
assign — which the simulator already implements and codegen currently does not. The
guard exists because codegen emits the unforwarded form for a segment whose anchor
entitles the source to forwarding. Under the paired fix the shape is simply legal,
means `assign o = r + 1`, and the "sticky flag" the engineer wanted is written
`o.write(r); r = r + 1;` — both hardwares expressible, ordering picks. The
`fast_counter` adjudication is consistent with this: it established that the *English
description* maps to the registered form (§3.2's correction), not that forwarding is
wrong — under the model the registered form is the write-then-update spelling.

> **Landed 2026-08-26 (phase D).** D1 did not vanish; it *narrowed* to its residue.
> `Cfg::unprotected_pretick_out_write` (`copper-analysis/src/cfg.rs`) now flags a
> register-reading write only when a leading read comb-reaches it
> (`leading_read_reaches`) — the path-dependent boundary (W4, `probe_fsm`) — and keeps
> the conditional/constant hold clause (`written_on_all_paths`) unchanged. Exact set
> pinned by
> `copper-analysis/tests/pretick_alignment_corpus.rs::pretick_alignment_hazard_flags_exactly_the_known_divergent_modules`
> (`EXPECTED_FLAGGED`: `probe_fsm`, `w4_mixed_alignment`, `branch_merge_explicit`,
> `pc_arm_toggle`, `pc_arm_write` as of 2026-09-01); the macro diagnostic in
> `copper-macros/src/lib.rs` describes the path-dependent boundary; ui cases
> `copper-macros/tests/ui/fail/pretick_alignment.rs::mixed_alignment` (W4) and
> `ui/pass/pretick_alignment_ok.rs::forwarded_opening_drive` (V1). The
> `ref_fast_counter` adjudication was re-pointed at the corrected spelling
> (`tests/sequential_forwarding_divergence.rs::independent_hardware_anchors_the_corrected_spelling`,
> rationale R7 recorded in the test).

## 5. Re-derivation of the existing rules

Each row must end up either **dissolved** (the shape acquires a defined meaning both
sides implement), **retained-derived** (a genuine unrealizability, now a consequence
of §1–§4 rather than a discovered discriminator), or **bug-with-assigned-side**.
"Checked" = the derivation matches a recorded measurement; "predict" = needs a new
measurement before it is believed.

| rule / divergence | status under the model | evidence |
|---|---|---|
| D1 `unprotected_pretick_out_write` | **dissolved for the V1 class, retained for the path-dependent boundary** — opening-prefix drives get forwarded emission; the sim was the correct side. LANDED 2026-08-26 (phase B + D, see the §4 note) | measured: `pre_tick_update_forwarding_agrees_end_to_end` (V1 sim ≡ SV), `d_narrowing_battery_verdicts` (V5, V7 agree; W4 diverges) in `tests/sequential_forwarding_divergence.rs` |
| D2 passthrough (fixed 2026-08-21) | **derived** — a segment with no commits is opening-anchored, so the read samples at the opening instant; the per-read `Immediate` fix (`copper_analysis::classify_reads`) is the partial implementation of exactly this | checked: `tests/sequential_forwarding_divergence.rs::d2_is_fixed_and_d1_still_demonstrates_the_hazard` |
| constant-write all-paths clause | **retained-derived** — §4's generation argument reproduces the exemption and its narrowing exactly | checked: `pc_arm_*`, `branch_merge_explicit` |
| `unprotected_trailing_out_write` | **DISSOLVED and RETIRED 2026-10-23** — phase C landed as an emission form (`vlir_lower::opening_hold_outputs`: an opening drive in a conditional arm stays combinational with a hold register); `trailing_update` and `branch_trailing` agree and dropped their opt-outs; the rule, its pin and `has_nested_tick` are gone. History of the row: **MEASURED — prediction half right.** The *derivation* held: the sim already implements the model for the trailing region (m2: sim ≡ hand-lowered model emission, cycle-for-cycle). But the shape did **not** dissolve in the tree — codegen's control-extraction route still places trailing effects one state late (phase C deferred, `PAIRED_IMPLEMENTATION_SCOPE.md`), so the rule is **retained, narrowed 2026-08-27** (commit 8ffade7) to the extracted class via `has_nested_tick`; the linear class is exempt and agrees. The single-vs-multi-tick discriminator is restated by the per-region refinement (a single-tick loop's trailing region shares the head's phase, `DERIVATION_TABLE.md` §1); the residual refusal is a lowering limitation, not a semantics rule | measured: `m2_model_forwarded_lowering_matches_the_simulator_for_trailing_update`, `linear_trailing_probe` (agrees), `branch_trailing_probe` and `d1_in_the_trailing_segment_is_an_unguarded_gap` (diverge one edge late), all in `tests/sequential_forwarding_divergence.rs`; exact set `EXPECTED_TRAILING` (`trailing_update`, `branch_trailing`) in `copper-analysis/tests/pretick_alignment_corpus.rs` |
| `multi_phase_out_write` | **retained-derived, NARROWED 2026-10-23 (phase E, §9)** — an `Out` written in several phases is a legal state mux iff every write has the same anchor on both sides: the input-free extracted class (every drive opening, phase C's mux) and the all-closing class (every write behind a deferred read, edge-sampled) are admitted; the mixed class, the register-free passthrough class and the linear route are refused | **measured** — the `e*` battery in `tests/sequential_forwarding_divergence.rs` (one test per row of §9.2, traces pinned); exact set `EXPECTED_MULTI_PHASE` = 8 probe modules; `pulse_plain` left the set and is clean under this rule |
| `multi_write_collapse` | **retained-derived** — the pre-tick write's intended window contains observation N, but its execution instant (pre-edge N+1) is after it; no scheduling realizes the window. Same conclusion, now from window arithmetic | checked: mechanism matches the rule's own analysis |
| post-entering-edge control read (refused ordering) | **retained-derived** — the value a control decision needs at the opening instant is defined at the closing settle; unrealizable by any implementation | checked: the two-window measurement (2026-08-24) |
| RegOut forwarding (L/L-1/L-2, fixed) | **derived** — commits are computed from forwarded values (§2); the repair implemented the model's clause | checked: `regout_forwarding_equivalence.rs` |
| trailing forwarding map (2026-08-25 fix) | **derived** — same clause at the trailing commit | checked: the differential case that found it |
| `program_counter` divergence #1 (CPU sweep) | **retained-derived, MEASURED and GUARDED** — the write sits between a leading read and the update of the register it reads: mixed generation, not frontier-expressible (§4). Predicted with exact traces, then measured (m1, the V8 battery), then guarded 2026-08-26 as `Cfg::pretick_out_write_before_update`; a trailing clause joined it 2026-08-27 (commit b439894) | measured: `v8a_write_between_leading_read_and_update_diverges_known_gap`, `v8b_…`, `v8c_…`, `v8d_…`, `v8t_trailing_stage_shape_verdict` in `tests/sequential_forwarding_divergence.rs`; exact set `EXPECTED_WRITE_BEFORE_UPDATE` (4 modules) in `copper-analysis/tests/pretick_alignment_corpus.rs`; ui/fail `write_before_update.rs` |
| `program_counter` divergence #2 (trailing `RegOut`, single-tick) | **MEASURED 2026-10-23, and the paper assignment was wrong for `RegOut`.** The minimal pair is `regout_trailing_single_tick`: the simulator parks a trailing `RegOut` write until the next edge, the linear fold published it at the opening edge, and the two differed by one cycle. The model clause is: *a `RegOut` write takes effect at the first clock edge after it executes* — the per-region "trailing commits anchor to the opening edge" refinement holds for register locals, not for `RegOut`. The linear single-tick lowering now emits a trailing `RegOut` from the committed register at the next edge (`shir_lower`'s `trailing_reg_outs`); the fixture agrees and sweeps; a trailing `RegOut` written from an input directly is refused (`Cfg::trailing_regout_reads_input`). Previous text: assigned on paper, not separately measured against the model as of 2026-09-01. The model's clause is the per-region refinement (a trailing region's commits anchor to the edge that *opens* its cycle). What is pinned in the tree: the shared trailing-forwarding map (commit 062724f, 2026-08-25) put trailing statements in the head's cycle on the linear path; the pipelined CPU keeps `program_counter` as `RegOut` and matches its Verilated self cycle-exact on every program (`tests/rv32i_pipelined_verilator.rs::pipelined_cpu_matches_its_verilated_self_on_all_programs`); the uart trailing `RegOut` write agrees (m4). A minimal-pair measurement of this row's own shape is still owed | not pinned by a dedicated test as of 2026-09-01 |
| memory staging rules (4) | **retained-derived, MEASURED** — bus-per-cycle and observe-after-edge are window statements about the address/data nets; the traces derived by hand from the model are reproduced by both the simulator and the transpiled SV (m3) | measured: `tests/cycle_dataflow_memory_derivation.rs::m3_rom_from_fn_matches_the_derived_denotation`, `::m3_dual_port_ram_matches_the_derived_denotation`; rules implemented in `Cfg::check_memory_staging` and `Cfg::memory_result_drives_plain_out` |
| reachability, CDC, poll-order, comb-loop rules | **untouched** — orthogonal to segment anchoring | — |

## 6. What the paired implementation would be (since scoped and LANDED — see `PAIRED_IMPLEMENTATION_SCOPE.md`; this section is the pre-scoping sketch)

Recorded so the shape is visible; written before §5's predict rows were measured
and the per-module table (§7) existed. Each bullet carries what actually happened
(2026-08-26, `PAIRED_IMPLEMENTATION_SCOPE.md` execution notes):

- `copper-analysis`: `segment_anchor(cfg) -> {Opening, Closing}` per segment, from
  commit-input data dependence. Both front-ends consume it — the same c2 discipline
  as every other shared fact. **Not written** (neither `segment_anchor` nor the
  scope's `region_anchor` exists in `copper-analysis/src/cfg.rs`): `edge_value`
  already *is* the per-drive forwarded form, so the selection needed no new
  analysis fact — execution note B. The reporting-only `Cfg::derivation_facts`
  is the closest thing, and no rule keys on it.
- executor / macro: barrier injection keyed on the anchor, not on leading-read
  presence. Opening-anchored segments run barrier-free by definition. **Not done,
  and not needed**: the read-site barrier is the region boundary (F3 revised;
  `PAIRED_IMPLEMENTATION_SCOPE.md` §0). The executor was not touched.
- codegen: plain-`Out` emission keyed on the anchor — forwarded expression for
  opening-anchored segments, unforwarded commit-frontier expression for
  closing-anchored ones (`SHIRPortDrive` already carries both `value` and
  `edge_value`; the selection point exists). **Landed** as the opening-prefix
  selection in `copper-codegen/src/shir_lower.rs` (§3's implementation note);
  trailing segments excluded pending phase C.
- the D1-family guards convert per §5: dissolved rules are deleted *with* their
  fixtures becoming positive differential cases; retained rules are restated as
  consequences (same detection sites, derived justification); the
  `allow_pretick_alignment` opt-out shrinks accordingly. **Landed as a narrowing,
  not a deletion**: D1 keeps its residue (§4 note); `add_then_write`, the
  `fast_counter` witness, V5 and V7 dropped their opt-outs; the trailing rule was
  kept (narrowed 2026-08-27); `pretick_out_write_before_update` and
  `multi_phase_out_write` are unchanged.

## 7. Migration discipline (standing rules apply verbatim)

- **Derivation table first.** Phase 1 is a per-module table over the whole corpus:
  each module's segments classified by anchor, predicted trace under the model,
  compared with today's trace — *predicted on paper, then measured*. Every module
  whose trace changes is listed with its rewrite (usually an ordering change) before
  any implementation lands. `fast_counter`-shaped modules are the known class.
- **The sweep is the oracle.** The differential corpus (G-D) plus the frozen goldens
  plus the independent anchors (BaseJump, `pattern_detector_010.sv`, the CDC
  reference) are the gate. Anchors get **re-verified, never re-blessed** — under a
  normative denotation they now test the model itself.
- **Old path kept.** *As written 2026-08-26:* the anchor-keyed behavior would go
  behind a mode with a lockstep differential test against the current behavior for
  the modules the model predicts are unchanged. **SUPERSEDED (2026-08-26, phase B
  execution note):** no mode or env var was ever added — there is no
  `COPPER_SEMANTICS` variable in the tree — because the `sv-baseline`
  snapshot/diff (`copper-codegen/src/bin/sv-baseline.rs`, snapshot under
  `target/sv-baseline/`, a manual tool and not a regression gate) gives the same
  "exactly the declared modules changed" comparison without doubling the test
  matrix. The prediction it checked — zero live modules change — held.

## 8. Known open questions for the derivation phase (status as of 2026-09-01)

1. The anchor definition's edge cases: a segment where an input feeds a commit on
   one path only; an input read whose result feeds *both* a commit and a plain `Out`
   (two sample instants for one binding — refuse, or split the read?). The V-battery
   method applies: construct the minimal pair, measure both sides. — **Open as a
   refinement.** The one-path case is the path-dependent boundary D1 retains
   (W4, `probe_fsm`); the dual-consumer read has no divergent instance
   (`DERIVATION_TABLE.md` §4b, the input-fed class).
2. Trailing segments: re-derive §5.4's single-vs-multi-tick discriminator from
   anchoring; if it does not fall out, the model is missing a clause — that is a
   finding, not a footnote. — **Answered.** It falls out of the per-region
   refinement (§2 note): a single-tick loop's trailing region is the head's phase.
   What the measurement then showed is that the *remaining* discriminator is the
   lowering route, not the phase count — `has_nested_tick`, the source-level mirror
   of control extraction (§5 trailing row).
3. Startup: an `Out` first written in a late segment vs the continuous assign's
   time-0 value (the documented startup discrepancy). Under a normative model this
   needs an initial-value rule, not a caveat. — **Answered 2026-10-23 by refusal**
   (`Cfg::out_first_driven_late`): a constant-only plain `Out` whose first write
   falls after the *second* edge is refused in both front-ends; the same constant
   one edge earlier is visible at the first observation and agrees
   (`late_constant_one_edge`, measured), and register-valued late writes agree
   because the flop's zero initial equals the wire's. No initial-value rule was
   added; the shape the simulator cannot show is unwritable instead.
4. Memory read results interact with anchoring (`is_ready`/`data` are
   edge-produced): restate the four staging rules in window terms. — **Done**
   (m3, `tests/cycle_dataflow_memory_derivation.rs`).
5. Whether `multi_phase_out_write`'s current flag set splits into legal-mux /
   refused-mixed as §5 predicts. — **Answered 2026-10-23 (§9).** It does, with
   one correction to the prediction: the split is not per *segment* but per
   *anchor agreement between the two implementations* — codegen's anchor
   predicate is coarser than the simulator's (any input read in an extracted
   body demotes every drive), so "every segment satisfies its own condition" is
   necessary but not sufficient. The rule is narrowed to the measured residue.

## 9. Phase E — multi-phase plain `Out` (derived and measured 2026-10-23)

The §8-5 question: a plain `Out` written in two or more clock phases is a mux over
FSM state — which such muxes does the transpiler realize, and which are mixed
generation? Derived first, then measured on a battery of minimal pairs
(`tests/sequential_forwarding_divergence.rs`, the `e*` tests; every trace pinned).

### 9.1 The two anchor predicates

A multi-phase port has one write per writing phase. Each write has an **anchor**
on each side, and the two sides decide it differently:

- **Simulator.** A write is *closing* when a **deferred** `In` read parks the task
  before it — a read with a `clk.tick().await` after it in the iteration
  (`classify_reads`), whose barrier moves everything after it to the pre-edge
  settle of the closing edge. Otherwise the write is *opening*: it executes in the
  post-edge settle of the edge that opened its cycle, on committed registers (and
  forwarded same-segment locals), and is visible at that cycle's observation. Two
  reads that look leading are **not** deferred: a read in the trailing segment
  (no tick follows it before the iteration closes) and a read feeding a plain
  `Out` in a **register-free** body (the D2 passthrough exception). Both are
  immediate, and the write they feed is opening.
- **Transpiler** (`shir_lower::lower_stmt_list`, `SHIRStmt::PortDrive::opening`;
  `vlir_lower::opening_hold_outputs`). A drive is *opening* when no `In` read
  precedes it in the walk of **its own arm**: an `if` or `match` seeds each arm
  from the reads of its condition or scrutinee alone, and past the branch the
  walk carries a read from any arm (**arm-granular since 2026-10-26**; until then
  the walk was statement-granular — a read anywhere inside a statement marked
  everything from that statement on, and since under control extraction the whole
  body is one `match pc` statement, any input read anywhere in an extracted
  module made every drive closing). The anchor is control-flow, not dataflow: the
  drive's own operand, an enclosing condition, and the exit test of a wait loop
  the write is duplicated into all count, deferred or immediate. A
  conditionally driven port with **at least one** opening drive is emitted as a
  forwarded combinational mux with a `<port>_hold` register (phase C): the comb
  copy muxes its opening drives only, and the hold register is loaded by every
  drive at the edge, so a closing drive of such a port takes effect through the
  hold register at the next edge — the edge-sampled timing — while its opening
  drives are visible at the opening observation (**drive-level since
  2026-10-29**, §9.4; until then the decision was port-wide, "all drives
  opening", and a port with one closing drive was edge-sampled as a whole). A
  conditionally driven port with no opening drive is an edge-sampled register
  under `pc` guards. On the
  linear multi-tick route there is no mux form at all: two continuous assigns to
  one port are refused as multiply driven, and a port whose value is a phase-local
  wire goes through the phase-guarded hold register.

Write-window arithmetic (§1) then gives the verdict per write:

| sim anchor | emitted form | what each side shows | verdict |
|---|---|---|---|
| opening | comb mux + hold | sim: value at obs N (opening edge N). SV: mux arm at obs N; hold loads the same operands at N+1 | **agree** |
| opening | edge-sampled | sim: obs N. SV: `o <= v` at edge N+1 → obs N+1 | **one cycle late** |
| closing | edge-sampled | sim: pre-edge N+1 with `x_{N+1}` and forwarded locals → obs N+1. SV: `o <= edge_value` at N+1, same operands | **agree** (frontier-expressible, §4) |
| closing | comb mux | cannot occur: a read before the drive makes it closing on the codegen side too | — |

So a multi-phase port agrees iff **every one of its writes has the same anchor on
both sides** — and since 2026-10-29 that criterion is **drive-level**: each
write is realized by the row of the table its own anchors select, independently
of the port's other writes. A port with one opening write and one closing write
(**mixed** anchors) is a forwarded mux over its opening drives with a hold
register every drive loads, and both writes agree (`branch_merge`'s `tail_o`,
`d2a`, `e4`, `e6`, measured §9.4). Until then codegen's emission was port-wide
("all drives opening"), so the closing write dragged the port into the
edge-sampled form and the opening write landed one observation late — the "mixed
generation" of §4 in its multi-phase form, not a mixed-generation *value* but a
mixed-anchor *mux*, and a limitation of the emission's granularity, not of the
denotation. What still diverges is a write whose anchors differ *between the two
sides* — an immediate read the codegen walk counts as closing (`e5`, `e4i`,
`e5f`: the register-free passthrough and trailing-read classes, §9.3 clauses 3
and 4) or a read on one path to the write and not on another (W4, `d1f`, `d1g`,
`d3c`, §10; `e11`, `e11b` in a middle phase, §10.4), which no per-drive form
reaches — and, on the every-path route only, a mixed-anchor comb mux (`d2b`),
which has no hold register to carry its closing drive to the edge. Both clauses
were narrowed to exactly that on 2026-10-30 (§10.5).

### 9.2 Predicted, then measured

Every row below was written as a prediction from §9.1 before the probe ran, and
every verdict matched on the first run except `e5`, whose *second* read turned out
to be a trailing (immediate) read rather than the deferred one intended — the
measurement corrected the row's classification, not the model, and `e5d` (one
tick added) is the shape the row meant.

| probe | writes (phase 1 / phase 2) | sim anchors | emission | verdict |
|---|---|---|---|---|
| `pulse_plain` | `One` / `Zero`, no input | opening / opening | mux + hold | **AGREE** |
| `e2_two_registers` | committed `a` / committed `b`, no input | opening / opening | mux + hold | **AGREE** |
| `e3_forwarded_then_committed` | `r = r+1; o.write(r)` / `o.write(r)`, no input | opening / opening | mux + hold (`r + 1` / `r`) | **AGREE** |
| `e5d_two_closing_deferred` | `x.read()` / `x.read() + cnt`, both followed by a tick | closing / closing | edge-sampled | **AGREE** |
| `e2x_two_registers_plus_unrelated_read` | `e2` plus `a = x.read()` in the head | opening / opening | edge-sampled (a read anywhere) until 2026-10-26; mux + hold since (arm-granular) | DIVERGE: SV = sim shifted one cycle → **AGREE** since 2026-10-26 |
| `e4_closing_then_opening` | `x.read()` / `Bits::zero()` | closing / opening | edge-sampled until 2026-10-29; drive-level mux + hold since | DIVERGE: the constant one cycle late → **AGREE** since 2026-10-29, admitted 2026-10-30 |
| `e6_closing_via_wire_then_opening` | `let r = x.read(); o.write(r)` / `Bits::zero()` | closing / opening | as `e4` | DIVERGE: as `e4` → **AGREE** since 2026-10-29, admitted 2026-10-30 |
| `e4r_closing_then_register_opening`, `e4f_closing_then_forwarded_opening` (2026-10-30) | `x.read()` / committed `cnt`, or `cnt = cnt + 1; o.write(cnt)` | closing / opening | drive-level mux + hold (comb arm `cnt`, `cnt + 1`) | **AGREE** (predicted, first run) |
| `e11_path_dependent_write_in_a_middle_phase` (2026-10-30) | `if cnt[0] { x.read() }; o.write(cnt)` / `Bits::zero()` | opening on even `cnt`, closing on odd / opening | mux + hold, `cnt` through the hold register | DIVERGE (predicted, first run): even-`cnt` writes one late |
| `e11b_path_dependent_write_then_closing` (2026-10-30) | the same / `x.read()` then a tick | as above / closing | edge-sampled | DIVERGE (predicted, first run): as `e11` — and the pre-narrowing clause 4 **admitted** it |
| `e5_two_closing` | `x.read()` / trailing `x.read() + cnt` (no tick after) | closing / **opening** (immediate read) | edge-sampled | DIVERGE: second write one cycle late |
| `e4i_register_free_passthrough_then_opening` | `x.read()` / `Bits::zero()`, no register | opening (passthrough) / opening | edge-sampled until 2026-10-29; mixed on the codegen side, so drive-level mux + hold since | DIVERGE: both one cycle late; since 2026-10-29 the constant agrees and the passthrough is still one late with the next edge's `x` |
| `e5f_two_closing_register_free` | `e5d` without its register | opening / opening (passthrough) | edge-sampled | DIVERGE: both one cycle late |
| `e7_linear_two_registers` | committed `a` / committed `b`, every tick top-level | opening / opening | **refused**: multiply driven (`vlir_lower`) | refused one IR late |
| `e7w_linear_phase_local_wires` | `a + 2` / `a + 5` via `let` wires, linear | opening / opening | phase-guarded hold register | DIVERGE: SV = sim shifted one cycle |

Three further probes are single-phase and sit outside the rule; they are in §9.4.

### 9.3 The decision: narrowed, not retired

Not every measured shape agrees, so the rule stays and is **narrowed to the
measured divergent class** (`Cfg::multi_phase_out_write`, both front-ends, same
opt-out). A port written in more than one phase is refused iff any of:

1. **linear route** — no tick of the loop body sits inside a statement
   (`Cfg::has_nested_tick` is false, the source mirror of control extraction's
   trigger): `vlir_lower` refuses or edge-samples it one IR later, and the macro
   must refuse what the transpiler refuses (front-end parity). Witness `e7`,
   `e7w`; flipping pair `e2` (the same writes under a folded wait loop, accepted).
2. ~~**an input read anywhere** in the body while some write of the port is
   opening in the simulator — codegen edge-samples the whole port.~~ **Retired
   2026-10-26**: it existed only for the statement-granular walk. Its witness
   `e2x` now agrees (its read sits in another state and reaches no write of the
   port) and is accepted; the flipping pair with `e2` is one `a = a + 1` for
   `a = x.read()` in the other direction.
3. **register-free body** with input reads — the reads feeding the writes are
   immediate passthroughs, so the writes are opening however they are spelled.
   Witness `e4i`, `e5f`; flipping pair `e5d` (one `let mut` added).
4. **a drive whose anchors differ between the sides** (**narrowed 2026-10-30**,
   §10.4–10.5; until then "mixed anchors": some write opening in the simulator
   while an input read reaches some write of the port, derived when the emission
   was port-wide). An input read comb-reaches the write in its phase — codegen
   holds it to the edge — while the simulator runs it at the opening on some
   path: every read reaching it is **immediate** (a trailing read: witness `e5`;
   the `e8` mechanism inside a multi-phase port — also derived for the
   all-opening case, `mphase_opening_write_then_trailing_immediate_read_flagged`),
   or the read sits on one path to the write and not on another
   (**path-dependent**, D1's clause (A1) in a middle phase: witnesses `e11`,
   `e11b` — the latter a shape the pre-narrowing clause admitted and that
   diverges). **Since 2026-10-30 (§10.6) the path-dependent half also applies
   to a port written in a single non-head phase** on the extraction route, in
   a body with registers — witness `e12`, the `e11` shape with its constant
   write removed, which compiled with no opt-out and diverges; uniform
   controls `e12a` (the read on every path) and `e12b` (on none) agree and are
   admitted. A port mixing a drive every path to which carries a deferred read
   with a drive no read reaches is realized drive by drive by the mux + hold form
   and **admitted**: `e4`, `e6` (the clause's former witnesses, agreeing since
   2026-10-29), `e4r`, `e4f`; flipping pairs `e5d` (one tick added after `e5`'s
   trailing write) and `e11` ↔ `e4` (the read moved out of its branch).
5. **a wait loop's read before an opening write** — a folded tick-bearing loop
   that reads an input is no Comb predecessor of the write after it, so the
   simulator side counts the write as opening, while control extraction
   duplicates that write into the loop's exit arm under the read's condition,
   where codegen sees a read before it. Unmeasured; refused by construction
   (`mphase_wait_loop_read_before_opening_writes_flagged`).

Accepted, each measured agreeing: the **extracted all-opening** class — no input
read reaches any write of the port, whatever the module reads in other states
(`pulse_plain`, `e2`, `e3`, and since 2026-10-26 `e2x`) — the **all-closing**
class (`e5d`), and since 2026-10-30 the **mixed class with uniform per-drive
anchors** (`e4`, `e6`, `e4r`, `e4f`). Unmeasured shapes fall on the refusing side by construction (clause
5; a body extraction handles only because of a `continue` counts as linear).
`pulse_plain` left `EXPECTED_MULTI_PHASE` and
keeps its opt-out for `out_first_driven_late` alone, which treats a constant past
a folded loop as late in the refusing direction although the module agrees
(§8-3's rule, conservative by design).

Pins: `copper-analysis/src/cfg.rs` unit tests `mphase_*` (one per clause;
`mphase_mixed_closing_and_opening_not_flagged`,
`mphase_register_opening_beside_closing_not_flagged`,
`mphase_path_dependent_middle_phase_write_flagged`,
`mphase_path_dependent_write_then_closing_flagged` since 2026-10-30);
`pretick_alignment_corpus.rs::EXPECTED_MULTI_PHASE` (`e7`, `e7w`, `e5`, `e11`,
`e11b`, `e12`, `e4i`, `e5f` — `e4` and `e6` left it 2026-10-30, `e2x` 2026-10-26,
`e12` joined 2026-10-30 by §10.6);
`copper-macros/tests/ui/fail/multi_phase_out_write.rs` (the `e11` shape; `e4`'s
until 2026-10-30), `ui/fail/path_dependent_single_phase.rs` (the `e12` shape) and
the accepted shapes in `ui/pass/pretick_alignment_ok.rs`
(`two_phase_mixed_anchors`, the `e4` shape, since 2026-10-30;
`single_phase_read_on_every_path` and `single_phase_no_read`, the `e12a` / `e12b`
shapes); the `e*` tests and
`phase_e_the_narrowed_rule_flags_exactly_the_diverging_probes` in
`tests/sequential_forwarding_divergence.rs`. `sv-baseline`: the only emission
delta is the new probe modules (no existing module's SV changed — the change is
a rule, not an emission).

### 9.4 Found on the way, outside the rule — one UNGUARDED, one FIXED

The battery's single-phase controls exposed two divergences that **no rule
flagged** when it landed (2026-10-23); one was the statement-granular predicate
of §9.1 at work in a port written in one phase and is fixed, the other is a
different mechanism and remains pinned as an `*_unguarded_gap` test:

- **`e9_read_in_another_state_opening_write`** — `for{tick}; r = x.read();
  for{tick}; o.write(r)`. The write reads only a committed register and no read
  precedes it in its own state, yet the drive was edge-sampled because the module
  reads an input somewhere. Without the read this is `trailing_update`, which
  agrees. Consequence then: **phase C's mux + hold form was reached only by
  input-free extracted modules**. **FIXED 2026-10-26** by arm-granular opening
  tracking in `shir_lower::lower_stmt_list` (an `if`/`match` seeds each arm from
  its condition's reads alone; past the branch the walk carries a read from any
  arm), pinned as `e9_a_read_in_another_state_leaves_an_opening_write_opening_agrees`;
  clause 2 above retired with it. The sv-baseline delta of that change was
  **seven** modules, every one a conditionally driven plain `Out` from a register
  or constant in an arm with no read: `e9`, `e2x`, `pc_arm_write`, `pc_arm_toggle`,
  `probe_fsm`, and `head_o` of the byte-identical twins `branch_merge` /
  `branch_merge_explicit`. All seven now take the mux + hold form, and **every
  pinned one-cycle lag among them flipped to agreement on the first run** —
  including the D1 constant-write clause's own witnesses (`pc_arm_*`) and the W4
  witness `probe_fsm` (its `#[ignore]`d sim-vs-Verilog test passes and it sweeps).
  `branch_merge_explicit`'s own lag was not among them: its `tail_o` is also
  written behind the deferred `sel` read — a mixed-anchor port, edge-sampled
  under the port-wide decision — and it still led by one when the first cycle
  took the `sel == 1` path (measured 2026-10-26), which the seeded sweep never
  did: the live twin `branch_merge` passed the sweep for that reason alone, a
  blind spot recorded here and in `PRETICK_ALIGNMENT_GUARDRAIL.md` §5.5, exposed
  by the multi-sequence stimulus 2026-10-28, and **fixed 2026-10-29 by the
  drive-level form below**. No referenced module (BaseJump, the pipelined CPU
  lane, the golden fixtures, `pattern_detector_010`, the CDC reference) changed
  in either landing. The D1 rules were untouched by the arm-granular change;
  their re-derivation on that evidence is §10 (2026-10-28).
- **The drive-level mux + hold form — LANDED 2026-10-29**
  (`vlir_lower::opening_hold_outputs`, the m3 hand model in
  `tests/sequential_forwarding_divergence.rs`). Phase C's decision was port-wide:
  a conditionally driven plain `Out` took the forwarded mux + `<port>_hold` form
  only when *every* drive was opening, so one closing drive edge-sampled every
  drive of the port and its opening drives landed one observation late — the
  mixed-anchor limitation of §9.1. Derived first from the per-write verdict
  table: a mixed port's comb copy should carry only its opening drives
  (`assign tail_o = (pc == 1) ? 1 : tail_o_hold;`) and its hold register should
  be loaded by **every** drive — the closing drive then takes effect through the
  hold register at the next edge (the edge-sampled timing, the table's third
  row) and the opening drive at the opening observation (the first row).
  Hand-written for `branch_merge` and measured against the simulator before any
  codegen change (`m3_drive_level_model_matches_the_simulator_for_branch_merge_on_both_first_cycles`:
  under `sel = 1` first both show `(1, 0, 1)` at cycle 0, under `sel = 0` first
  both show `(1, 1, 1)` throughout — matched on the first run); then codegen was
  changed to emit exactly that: a port with at least one opening drive takes
  the form, `strip_closing_drives` removes its closing drives from the comb
  copy, and `shadow_hold_ff` loads the hold register from the complete drive
  list. The all-opening case emits byte-for-byte what it did; the all-closing
  case stays edge-sampled. **sv-baseline delta: seven modules, every one a
  mixed-anchor hold-form port, no referenced module** — declared from the SHIR
  anchor tags before the diff (`copper-codegen/tests/hold_form_audit.rs` pins
  the exact set through `copper_codegen::hold_form_ports`): the `branch_merge`
  twins' `tail_o` (and the probe file's verbatim copy), `d2a`, `e4`, `e6`, and
  `e4i`, which the declared list had missed — its register-free body is mixed on
  the codegen side only. **Measured flips, every one on the first run:**
  `branch_merge` sim ≡ model ≡ SV on both first cycles and the sweep passes all
  six of its sequences (both twins' `SKIP` rows deleted; the explicit twin keeps
  its opt-out as a D1 witness); `d2a` SV `[0, 9, 5, 9, 5, …]` → `[0, 5, 5, …]`
  = sim; `e4` and `e6` SV `[0, 0, 12, 12, 0, 0, 16, 16, …]` → `[0, 0, 12, 0, 0,
  0, 16, 0, …]` = sim. `e4i` changed trace and still diverges (its constant
  agrees now, its passthrough drive is still one late with the next edge's
  `x`). Unchanged and still divergent, as the model says: `w4_mixed_alignment`,
  `d1f`, `d1g`, `d2b`, `d3c` (a single write node whose anchor is
  path-dependent, or an every-path mux — no per-drive form reaches them) and
  `e5` (a cross-side anchor mismatch). **No rule changed that day.** D1's
  clause (A) kept refusing `d2a` and `branch_merge_explicit`'s `tail_o`, and
  `multi_phase_out_write` clause 4 kept refusing `e4` and `e6`, although the
  emission realized them, until **both clauses were narrowed 2026-10-30**
  (§10.4–10.5) to exactly the residue named here.
- **`e8_single_phase_trailing_immediate_read`** — `for{tick}; o.write(x.read()
  + cnt)`. A trailing read is immediate in the simulator (it shows `x_N + cnt` at
  the opening observation) and a closing drive in codegen (`x_{N+1} + cnt` one
  observation later). This is the D2 passthrough class inside a conditional arm;
  realizing it needs a drive form that samples the input at the *opening* edge
  (a register loaded on state entry), or a refusal rule. **DECIDED 2026-10-26:
  refused**, by `Cfg::trailing_out_reads_input` (both front-ends, the pre-tick
  opt-out), scoped by four more probes in the same file: the single-tick linear
  body **agrees** (`e8l_linear_passthrough`, `e8lr_linear_with_register` — the
  trailing region is the whole phase and the continuous assign sees the input the
  immediate read saw, the D2 form), so the rule requires a body that crosses more
  than one edge; the register-free extracted passthrough diverges one late like
  `e8` (`e8p_extracted_passthrough`); and the linear two-tick body diverges
  *differently* (`e8m_linear_two_ticks`: the trailing drive is a continuous assign
  the phase does not gate, so it tracks `x + cnt` every cycle where the simulator
  holds it) — a second emission defect of the same family, refused by the same
  clause. Through a register or behind a deferred read the write agrees, as for
  the `RegOut` sibling. 0 live corpus modules; `e5_two_closing` joins the pin
  because its second write is exactly this shape.

A third control, **`e10_middle_phase_write_before_update`** (the V8a shape
`let s = x.read(); o.write(r); r = r + s` in a *middle* phase, which neither
clause of `pretick_out_write_before_update` covers), **agrees**: in an extracted
arm the drive is edge-sampled and the flop samples the pre-update register
exactly as the barrier-parked write reads it. The head-segment V8a diverges only
because its drive is a continuous assign. The V8 rule's head-plus-trailing scope
is therefore sufficient, and the test pins that.

## 10. The D1 re-derivation — `unprotected_pretick_out_write` (derived and measured 2026-10-28)

The arm-granular opening walk (§9.4, 2026-10-26) flipped every witness of D1's
constant-write clause and the W4 witness `probe_fsm` to agreement without
touching the rule. This section re-derives the rule from §9.1's two anchor
predicates for the **head segment** — loop head to the first tick — and
narrows it to what the model says diverges. Same method as phase E: every row of
§10.2 was written from the model before its probe ran, and every verdict matched
on the first run (the `d*` tests in `tests/sequential_forwarding_divergence.rs`,
sixteen probes, both traces pinned each).

### 10.1 The model, per drive

In a body with registers every head-segment `In` read is deferred (`classify_reads`:
a tick follows it, and the register-free passthrough exception does not apply), so
for a drive `W` of a plain `Out` port `o`:

- **Simulator, per path.** `W` is *closing* on a path where a read precedes it —
  the barrier parks the task, the write runs in the pre-edge settle and is
  visible at the next observation, with forwarded locals equal to the committing
  values. It is *opening* on a path with no read before it — it runs in the
  post-edge settle of the edge that opened the cycle, on committed registers and
  same-segment forwarded locals, and is visible at that cycle's observation. The
  same drive can be closing on one path and opening on another (W4).
- **Transpiler, per drive, port-wide form.** `W` is closing iff some read
  comb-reaches it on *any* path: the walk seeds each arm from its own condition
  but carries a read from any arm past the branch (`shir_lower::lower_stmt_list`).
  The port's form is then port-wide: driven on **every** path it is an
  `always_comb` continuous assign (forwarded if opening, committed if closing) or
  mux, whose **guards take the committed form** (`emit.rs`: `always_comb` uses
  `condition`, the edge form is taken only by the `always_ff` split); driven on
  **some** paths it is phase C's forwarded mux + hold register when some drive is
  opening — the comb copy muxing the opening drives, the hold register loaded by
  every drive (drive-level since 2026-10-29; "when every drive is opening" until
  then) — else an edge-sampled register with edge-form guards.
- **One more simulator fact the probes exposed.** A closing write followed by the
  next iteration's opening write of the same port is **never observed**: the
  opening write runs in the post-edge settle of the same edge, before the
  observation. So a port with both kinds of write shows only its opening writes
  in the simulator, while the SV shows both (`d2b`: sim `[2, 2, 2, …]`, SV
  `[2, 1, 2, 1, …]`).

Two hazards follow, and the derivation finds no third in the head segment:

- **(A) Mixed anchors.** A read reaches some drive of `o` (codegen: closing →
  edge-sampled or committed-form) while some drive of `o` has a read-free path
  from the head (simulator: opening on that path). The opening write lands one
  observation late in the SV, or is overwritten-and-shown where the simulator
  overwrote it unseen. Unobservable in exactly one case: a **single drive on every
  path** (a continuous assign) whose read-free path assigns no register the drive
  reads — the opening write then publishes the committed register, which is
  what `assign o = r` shows (`d3a`).
- **(B) Forwarded guard.** An opening drive (no read reaches it, so its comb copy
  is the one that is observed) nested under an `if`/`match` whose test reads a
  register assigned earlier on the path. The simulator tests the forwarded value,
  the comb copy the committed one. In the hold form the hold register, loaded
  under the edge-form guard, catches the pulse one observation late (`d4a`); in
  the every-path form nothing does (`d4c`).

The pre-2026-10-28 rule — "a conditional or register-reading, read-preceded
write in a segment that assigns a register no read reaches" — was a proxy for
these: the unprotected assignment coincides with (B) when it feeds a guard and
with (A)'s continuous-assign exception when it feeds the value, and with nothing
when it feeds neither (`pc_arm_*`, `d1b`, `d1c`, `d3a`, `d4b` — all refused,
all agreeing). It also exempted every-path writes outright, which is where
`d1g`, `d2b` and `d4c` hid, and required the unprotected assignment, which is
where `d2a` hid.

### 10.2 Predicted, then measured

Constant-operand drives (`x` is the `Bits<8>` input driven `10 + k`; `pc`/`phase`/
`cnt` are registers):

| probe | shape | sim anchors | emission | verdict |
|---|---|---|---|---|
| `pc_arm_write`, `pc_arm_toggle` | constant in an arm the read's sibling arm cannot reach | opening | mux + hold | **AGREE** (since 2026-10-26) |
| `d1b_const_read_in_arm` | `cnt = cnt+1; if x.read()[0] { o.write(7) }` | closing | edge-sampled, edge-form guard | **AGREE** |
| `d1c_const_no_read` | constants in two of three state arms, no input | opening | mux + hold | **AGREE** |
| `d1d_const_all_paths_read` | `if x.read()[0] { o.write(7) } else { o.write(3) }` | closing / closing | comb mux over `x` | **AGREE** |
| `d1e_const_all_paths_no_read` | `if pc == 0 { o.write(7); pc = 1 } else { o.write(3); pc = 0 }` | opening / opening | comb mux on committed `pc` | **AGREE** |
| `d1f_const_path_dependent_hold` | read on one arm; `if phase == 0 { o.write(7) }` after the merge | opening on the read-free path (closing path never writes) | edge-sampled (read carried past the branch) | DIVERGE: SV `[0, 7, …]` vs sim `[7, 7, …]` |
| `d1g_const_path_dependent_all_paths` | `d1f` with an `else { o.write(3) }` | closing / opening | comb mux on committed `phase` | DIVERGE: sim holds 7, SV alternates |
| `d2a_mixed_two_drives_hold` | `if pc == 1 { o.write(5) }; if x.read()[0] { o.write(9); pc = 1 } else { pc = 0 }` — no unprotected assignment | opening / closing | edge-sampled until 2026-10-29; drive-level mux + hold since | DIVERGE: sim `[0, 5, 5, …]`, SV `[0, 9, 5, 9, 5, …]` → **AGREE** since 2026-10-29 (the hold register loads 9 at the edge that sets `pc`, and the comb arm covers it at the very observation the simulator overwrote it) |
| `d2b_mixed_two_drives_all_paths` | `if phase == 0 { x.read(); o.write(1); … } else { o.write(2); … }` | closing / opening | comb mux | DIVERGE: sim `[2, 2, …]`, SV `[2, 1, 2, 1, …]` |
| `d2c_two_closing_drives_hold` | `d2a` with the first drive moved behind the read | closing / closing | edge-sampled | **AGREE** |

Register-operand drives (the W4 family) and guards:

| probe | shape | sim anchors | emission | verdict |
|---|---|---|---|---|
| `w4_mixed_alignment` | `if phase == 0 { r = x.read() } else { r = r + 1 }; o.write(r)` | closing / opening, forwards `r + 1` | `assign o = r` (committed) | DIVERGE (sim holds `i + 1`, SV alternates) |
| `d3a_w4_unrelated_register` | the read-free arm assigns `phase` only | closing / opening, forwards nothing `o` reads | `assign o = r` | **AGREE** |
| `d3b_w4_read_on_every_path` | both arms read | closing / closing | `assign o = r` | **AGREE** |
| `d3c_w4_hold` | W4 with `if phase == 0 { o.write(r) }` after the merge | opening on the read-free path | edge-sampled | DIVERGE: SV one late |
| `probe_fsm` | the write in the read's sibling arm | opening | mux + hold | **AGREE** (since 2026-10-26) |
| `d4a_forwarded_guard_hold` | `cnt = cnt+1; if cnt == 3 { o.write(7); cnt = 0 }`, no input | opening, forwarded guard | mux + hold, comb guard on committed `cnt` | DIVERGE: SV `[0, 0, 7, …]` vs sim `[0, 7, 7, …]` |
| `d4b_committed_guard_hold` | `if cnt == 2 { o.write(7); cnt = 0 } else { cnt = cnt+1 }` | opening, committed guard | mux + hold | **AGREE** |
| `d4c_forwarded_guard_all_paths` | `d4a` with an `else { o.write(3) }` | opening, forwarded guard | comb mux on committed `cnt` | DIVERGE: SV never shows 7 |
| `d4d_forwarded_guard_closing` | `d4a` behind `x.read()` on every path | closing | edge-sampled, edge-form guard | **AGREE** |

Every row matched its prediction on the first run. The old rule's verdicts on the
same table: it refused `d1b`, `d1c`, `d1f`, `d3a`, `d3c`, `d4a`, `d4b`, `w4`,
`pc_arm_*`, `probe_fsm` and admitted the rest — four false positives among the
agreeing rows (`d1b`, `d1c`, `d3a`, `d4b`, plus the three re-blessed witnesses)
and four false negatives among the diverging ones (`d1g`, `d2a`, `d2b`, `d4c`).

### 10.3 The decision: re-derived, both clauses flipping

`Cfg::unprotected_pretick_out_write` (both front-ends, the same opt-out) refuses a
plain `Out` iff, among its drives `D` in the pre-tick region of a body with
registers:

1. **(A)** some drive is comb-reached by an `In` read **and** some drive has a
   read-free path from the loop head (`Cfg::read_free_path_reaches`), **and** the
   mismatch is observable: the port is held on some path, or has more than one
   drive, or some drive's read-free path assigns a register the drive reads. The
   exempt case is the single every-path drive forwarding nothing it reads (`d3a`).
   Witnesses: `w4_mixed_alignment`, `d1f`, `d1g`, `d2a`, `d2b`, `d3c`,
   `branch_merge_explicit` (`tail_o`). Flips: `d3a`, `d3b`, `d1b`, `d2c`,
   `probe_fsm`, `pc_arm_*`.
2. **(B)** some drive no read reaches sits under a branch (`Node::guards`, recorded
   by the CFG builder) whose test reads a register that some node assigns on a
   path to that branch. Witnesses: `d4a`, `d4c`. Flips: `d4b`, `d4d`, `d1c`.

The register-free body is excluded (its reads are immediate; `multi_phase_out_write`
clause 3 and `trailing_out_reads_input` own those shapes). The CFG is value-blind:
a drive whose guard is false on the read-free path still counts as mixed, which
is the refusing direction for an unmeasured shape, as everywhere in the family.

**Corpus cost: zero live modules.** The exact-set pin (`EXPECTED_FLAGGED`) is now
`w4_mixed_alignment`, `branch_merge_explicit` and the seven refused `d*` probes;
`probe_fsm`, `pc_arm_write` and `pc_arm_toggle` left it and compile without an
opt-out, as do the agreeing `d*` probes. No other pre-tick rule claims any `d*`
probe (`d_the_rederived_rule_flags_exactly_the_diverging_probes` asserts both
halves). The sv-baseline delta is the new probe modules only — a rule change,
not an emission change — so `branch_merge`'s seed-masked `tail_o` lag (§9.4) is
unchanged and still out of scope.

Pins: `cfg.rs` unit tests `d1f_…_flagged` / `d1b_…_not_flagged`,
`d2a_…_flagged` / `d2c_…_not_flagged`, `d2b_…_flagged` / `d1d_…_not_flagged`,
`hazard_w4_mixed_alignment_flagged` / `d3a_…_not_flagged` / `d3b_…_not_flagged`,
`d4a_…_flagged` / `d4b_…_not_flagged`, `d4c_…_flagged` / `d4d_…_not_flagged`,
`hazard_probe_fsm_sibling_arm_read_not_flagged`, `d1c_…_not_flagged`;
`copper-macros/tests/ui/fail/pretick_alignment.rs` (W4),
`ui/fail/pretick_mixed_anchors.rs` (`d2a`), `ui/fail/pretick_forwarded_guard.rs`
(`d4a`) and the four admitted shapes added to `ui/pass/pretick_alignment_ok.rs`;
`front_end_parity.rs` unchanged (same function, both front-ends).

**Not done, recorded.** Hazard (B) is an emission limitation — the comb copy of a
branch guarding an opening drive could take the edge form, as the `always_ff`
copy already does (`TODO` cause N is the mirror image). **(A)'s two-drive held
form landed 2026-10-29** as the drive-level emission of §9.4: `d2a` and
`branch_merge_explicit`'s `tail_o` agree with their SV now. (A)'s
path-dependent forms do not and cannot by that route — `w4_mixed_alignment`,
`d1f`, `d1g`, `d3c` are one write node whose anchor differs per path — and the
every-path mux `d2b` has no hold register. **Clause (A) was narrowed to exactly
that residue on 2026-10-30** (§10.4–10.5 below); until then it refused `d2a` and
`branch_merge_explicit` as before.

### 10.4 Narrowing both mixed-anchor clauses to what the drive-level form leaves (derived 2026-10-30, then measured)

The drive-level mux + hold form (§9.4, 2026-10-29) flipped `d2a`,
`branch_merge_explicit`'s `tail_o`, `e4` and `e6` to agreement while D1's clause
(A) and `multi_phase_out_write`'s clause 4 kept refusing them. This section
derives, from §9.1's two anchor predicates and the three emitted forms, which
of the shapes the two clauses refuse **still** diverge and why, and restates
each clause as the smallest criterion over the CFG that covers exactly those.
Method as in §9.2 and §10.2: every row was written before anything ran, the
existing probes' verdicts were then read off the battery (all pinned on the
2026-10-29 branch), and the new probes were run once.

**The forms, as the lowering emits them today** (confirmed by transpiling `d2b`,
`d1g`, `d2a`, `d1f`, `e5` before writing the rows):

- a port driven on **every** path of its phase is an `always_comb` assign or
  mux whose guards take the **committed** form, with no hold register — whatever
  the drives' anchors (`d2b`, `d1g`: `if (phase == 0) o = 1; else o = 2;`);
- a **held** port with at least one opening drive is the drive-level mux + hold
  form: the comb copy muxes the opening drives over `<port>_hold`, and every
  drive loads the hold register at the edge under its edge-form guard (`d2a`);
- a held port none of whose drives is opening is an edge-sampled register
  (`d1f`, `e5`).

**One mechanism covers every residue: a drive whose two anchors differ between
the sides.** Codegen anchors a drive once — closing iff some `In` read
comb-reaches it on *any* path of its arm's walk — and realizes that anchor
exactly (the comb mux for opening, the hold register or edge-sampled flop for
closing). The simulator anchors each *execution* of the drive by the path it
took. So a drive agrees iff its codegen anchor equals its simulator anchor on
every path that reaches it, and diverges when a path exists on which the
simulator runs it at the opening while codegen holds it to the edge (one
observation late), or — on the every-path route only — when the committed-guard
comb mux shows a closing drive at the opening observation, where the simulator's
closing write has not run yet and is overwritten by the next opening write
before it is ever observed (§10.1's third fact). Three ways a drive's anchors
differ, each with a measured witness:

1. **path-dependent** — a read reaches the drive on one path and not on
   another (`w4`, `d1f`, `d1g`, `d3c`; D1's domain);
2. **immediate read** — every read reaching the drive is immediate in the
   simulator: a trailing read (`e5`) or a register-free passthrough (`e4i`,
   `e5f`), which codegen counts as closing all the same;
3. **every-path mixed mux** — the drives' anchors agree per drive, but the
   port is written on every path, so the lowering emits the committed-guard
   comb mux and not the hold form, and the closing drive is shown one
   observation early (`d2b`).

A held port whose drives each have a uniform anchor on both sides is realized
by the drive-level form whatever the mix (`d2a`, `branch_merge_explicit`,
`e4`, `e6`), and the forwarded-guard hazard (B) is a separate mechanism (§10.1)
that this section leaves as it is.

**Predicted, then measured — the shapes the two clauses refuse today.**
"Codegen" is the drive's anchor as `shir_lower` marks it; "sim" its anchor per
path. The verdict column is the battery's as pinned on 2026-10-29 (every
existing row) and the first run (every new row, marked NEW).

| probe | drives: sim anchor / codegen anchor | emitted form | predicted | mechanism | measured |
|---|---|---|---|---|---|
| `w4_mixed_alignment` | one every-path drive `o.write(r)`: closing on the read arm, opening (forwards `r + 1`) on the other / closing | `assign o = r` (committed) | DIVERGE | 1 — the opening execution publishes the forwarded `r + 1`, the assign the committed `r` | DIVERGE (sim holds `i + 1`, SV alternates) |
| `d1f_const_path_dependent_hold` | one held drive, only ever executed on the read-free path: opening / closing (the read is carried past the branch) | edge-sampled | DIVERGE: SV one late | 1 | DIVERGE (`[7, 7, …]` vs `[0, 7, …]`) |
| `d1g_const_path_dependent_all_paths` | two every-path drives after the merge: `7` opening, `3` closing / both closing | committed-guard comb mux | DIVERGE: SV alternates | 1 and 3 | DIVERGE (`[7, …]` vs `[3, 7, 3, 7, …]`) |
| `d3c_w4_hold` | one held drive `o.write(r)` after the merge: opening (forwards `r + 1`) / closing | edge-sampled | DIVERGE: SV one late | 1 | DIVERGE |
| `d2b_mixed_two_drives_all_paths` | `1` behind the read: closing / closing; `2` in the other arm: opening / opening | committed-guard comb mux (every path) | DIVERGE: SV shows `1` whenever `phase == 0` is committed, the sim never shows it | 3 | DIVERGE (`[2, …]` vs `[2, 1, 2, 1, …]`) |
| `d2a_mixed_two_drives_hold` | `5` under `pc == 1`: opening / opening; `9` behind the read: closing / closing | mux + hold (comb: `5`; hold: both) | **AGREE**: the hold loads `9` at the edge that sets `pc`, and the comb arm covers it at the observation the sim overwrote it | none — uniform per drive, held | **AGREE** (`[0, 5, 5, …]` both) |
| `branch_merge_explicit` (`tail_o`) | the `pc == 0` else-arm drive behind `sel`: closing / closing; the `pc == 1` drive: opening / opening | mux + hold | **AGREE** | none | **AGREE** (both first cycles, all six sweep sequences) |
| `e4_closing_then_opening` | `x.read()`: closing / closing; `Bits::zero()`: opening / opening | mux + hold | **AGREE** | none | **AGREE** |
| `e6_closing_via_wire_then_opening` | as `e4` through a wire | mux + hold | **AGREE** | none | **AGREE** |
| `e5_two_closing` | `x.read()`: closing / closing; trailing `x.read() + cnt`: **opening** (immediate) / closing | edge-sampled (no opening drive on the codegen side) | DIVERGE: the second write one late with `x_{N+1}` | 2 | DIVERGE |
| `e4i_register_free_passthrough_then_opening` | `x.read()`: **opening** (passthrough) / closing; `Bits::zero()`: opening / opening | mux + hold (mixed on the codegen side only) | DIVERGE: the constant agrees, the passthrough one late with `x_{N+1}` | 2 | DIVERGE (SV `[0, 0, 12, 0, 0, 0, 16, 0, …]` vs sim `[0, 11, 11, 0, 0, 15, 15, 0, …]`) |
| `e5f_two_closing_register_free` | both passthrough: opening / closing | edge-sampled | DIVERGE: both one late | 2 | DIVERGE |
| `d4a`, `d4c` | opening / opening under a forwarded guard | mux + hold / comb mux, committed guard | DIVERGE | (B), unchanged | DIVERGE |

**New probes — the shapes the narrowed clauses admit that had no witness, and
the two the path-dependent mechanism predicts in clause 4's domain.** The
D1 shapes are `d2a` with one or both constants replaced by a register, on the
single-tick route and, with `for _ in 0..2 { clk.tick().await; }` as the tick,
on the extraction route (the head segment then becomes the `pc == 0` arm of the
synthesized FSM); `st` is the state register, `cnt`/`a`/`b` counters updated
before the drives, so a register-valued opening drive forwards its update.

| probe | shape (head segment; `…` is the tick, bare or the 2-cycle `for`) | drives: sim / codegen | form | predicted | mechanism |
|---|---|---|---|---|---|
| `d5a_const_opening_reg_closing` (NEW, linear) | `cnt = cnt + 1; if st == 1 { o.write(5) }; if x.read()[0] { o.write(cnt); st = 1 } else { st = 0 }; …` | `5`: opening / opening; `cnt`: closing / closing | mux + hold | **AGREE** | none — the closing drive forwards `cnt + 1` into the hold register at the edge, as the sim's pre-edge write does |
| `d5b_reg_opening_const_closing` (NEW, linear) | `cnt = cnt + 1; if st == 1 { o.write(cnt) }; if x.read()[0] { o.write(9); st = 1 } else { st = 0 }; …` | `cnt`: opening (forwards `cnt + 1`) / opening; `9`: closing / closing | mux + hold (comb arm `cnt + 1`) | **AGREE** | none |
| `d5c_reg_opening_reg_closing` (NEW, linear) | `a = a + 1; b = b + 3; if st == 1 { o.write(a) }; if x.read()[0] { o.write(b); st = 1 } else { st = 0 }; …` | `a`: opening / opening; `b`: closing / closing | mux + hold | **AGREE** | none |
| `d5d_const_opening_reg_closing_extracted`, `d5e_reg_opening_const_closing_extracted`, `d5f_reg_opening_reg_closing_extracted` (NEW) | the three above with the 2-cycle `for` as the tick | as above | mux + hold in the `pc == 0` arm | **AGREE** (the port holds through the second cycle on both sides) | none |
| `e4r_closing_then_register_opening` (NEW) | `cnt = cnt + 1; for{tick}; o.write(x.read()); for{tick}; o.write(cnt)` | closing / closing; opening / opening | mux + hold | **AGREE** | none |
| `e4f_closing_then_forwarded_opening` (NEW) | `for{tick}; o.write(x.read()); for{tick}; cnt = cnt + 1; o.write(cnt)` | closing / closing; opening (forwards `cnt + 1`) / opening | mux + hold (comb arm `cnt + 1`, `e3`'s form) | **AGREE** | none |
| `e11_path_dependent_write_in_a_middle_phase` (NEW) | `cnt = cnt + 1; for{tick}; if cnt[0] == 1 { let _ = x.read(); }; o.write(cnt); for{tick}; o.write(0)` | `cnt`: opening on even `cnt`, closing on odd / closing (read carried past the branch); `0`: opening / opening | mux + hold (comb: `0`; hold: both) | DIVERGE: on even `cnt` the sim shows `cnt` at the state's first observation, the SV one later through the hold register; odd `cnt` agrees | 1, in a non-head phase — refused by clause 4 today (its constant is opening while a read reaches the other write), and the narrowed clause must keep refusing it |
| `e11b_path_dependent_write_then_closing` (NEW) | `e11` with `o.write(x.read()); clk.tick().await;` as its last phase | `cnt`: as `e11` / closing; `x.read()`: closing / closing | edge-sampled (no opening drive on the codegen side) | DIVERGE: as `e11` on even `cnt` | 1 — **admitted by clause 4 today** (no write of the port is opening in the simulator on every path, so `any_opening` is false): an unmeasured gap, refused by the narrowed clause |

If any prediction above misses, the model is wrong and the rules stay as they
are until it is repaired. **Measured 2026-10-30 — every prediction matched on
the first run.** `d5a` sim ≡ SV `[5, 5, 5, 4, 5, 5, 5, 8, 5, 5, 5, 12, 5]` (the
closing register value reaches the output through the hold register on both
sides; the first version of the `d5*` probes keyed the closing write and the
state update off the same bit as `d2a`, so the closing value never surfaced, and
on the extraction route the read sampled only even `x` and the drives never
fired — all-zero traces, agreeing vacuously; the probes were restructured to key
off different bits before the verdicts were taken); `d5b` `[2, 3, 3, 9, 6, 7, 7,
9, …]`, `d5c` `[2, 3, 3, 12, 6, 7, 7, 24, …]`, `d5d` `[1, 1, 1, 5, 3, 5, …, 7]`,
`d5e` `[9, 9, 9, 3, 9, 4, 4, 4, 9, …]`, `d5f` `[3, 3, 3, 3, 9, 4, 4, 4, 15, …]`,
`e4r` = `e4f` `[0, 0, 12, 1, 1, 1, 16, 2, 2, 2, 20, 3, 3]` — all AGREE; `e11` sim
`[0, 0, 1, 0, 0, 2, 2, 0, 0, 0, 3, 0, 0]` vs SV `[0, 0, 1, 0, 0, 0, 2, 0, 0, 0,
3, 0, 0]` (the even-`cnt` write one late, the odd one agreeing, as predicted);
`e11b` sim `[0, 0, 1, 1, 14, 14, 2, 2, 2, 19, 19, 19, 3]` vs SV `[0, 0, 1, 1, 14,
14, 14, 2, 2, 19, 19, 19, 3]` — both DIVERGE. The existing rows were as pinned.

### 10.5 The decision: both clauses narrowed to the residue (2026-10-30)

**`Cfg::unprotected_pretick_out_write`, clause (A)** — a plain `Out` of the head
segment, in a body with registers, is refused iff an `In` read comb-reaches some
drive of the port while some drive has a read-free path from the loop head
(mixed anchors), **and** either

- **(A1) a drive is path-dependent** — the same drive is both reached by a read
  on some path and reachable read-free on another — observable unless it is the
  port's single every-path drive and forwards no register its read-free path
  assigns (`d3a`'s exemption, unchanged); or
- **(A2) the port is written on every path** — the every-path comb mux has no
  hold register, so the read-preceded drive is shown at the opening observation.

Witnesses: `w4_mixed_alignment`, `d1f`, `d3c` (A1, held), `d1g` (A1 and A2),
`d2b` (A2). **Admitted:** a held port whose drives each have a uniform anchor —
`d2a`, `branch_merge_explicit` (`tail_o`; it compiles without an opt-out and
sweeps like its twin), `d5a`–`d5f`. Clause (B) is unchanged. The refused set
equals the measured divergent set: `EXPECTED_FLAGGED` = `w4_mixed_alignment`,
`d1f`, `d1g`, `d2b`, `d3c`, `d4a`, `d4c`.

**`Cfg::multi_phase_out_write`, clause 4** — a plain `Out` written in more than
one phase, on the extracted route, in a body with registers, is refused iff some
write of it is comb-reached by an `In` read in its phase (codegen: closing)
**and** the simulator runs that write at the opening on some path: no *deferred*
read reaches it (every reaching read is immediate — `e5`), or it has a read-free
path from its phase's entry (`Cfg::read_free_path_from_phase_entry`, the
generalization of (A1)'s walk to the target of every tick edge — `e11`,
`e11b`). Clauses 1, 3 and 5 are unchanged. **Admitted:** `e4`, `e6`, `e4r`,
`e4f`. `EXPECTED_MULTI_PHASE` = `e7`, `e7w`, `e5`, `e11`, `e11b`, `e4i`, `e5f`.
`e11b` is the one shape whose status moved in the refusing direction: the
pre-narrowing clause required some write of the port to be opening in the
simulator on *every* path, which a path-dependent write beside a closing one is
not, and it diverges — an unmeasured gap, closed.

**Corpus cost: zero live modules** in either direction. The sv-baseline delta is
the ten new probe modules and no existing module (a rule change, not an emission
change; `branch_merge_explicit`, `d2a`, `e4` and `e6` transpiled before under
their opt-outs and emit byte-identically without them). The mixed-anchor
hold-form set (`copper-codegen/tests/hold_form_audit.rs`) grew by the nine new
ports the form reaches (`d5a`–`d5f`, `e4r`, `e4f`, `e11`; `e11b` has no opening
drive and is edge-sampled). Both front-ends run the same two functions with
reworded diagnostics that name what is refused and the remedy (make each write's
anchor uniform, test only committed state, `RegOut`, or move the writes); the
parity audit is unchanged in code and exact in result. Pins: `cfg.rs` unit pairs
`d1f_…_flagged` / `d2a_…_not_flagged` / `d5a_…_not_flagged` /
`d5f_…_extracted_not_flagged`, `d2b_…_flagged` / `d2a_…_not_flagged`,
`branch_merge_explicit_tail_o_not_flagged`, `mphase_mixed_closing_and_opening_not_flagged`
/ `mphase_path_dependent_middle_phase_write_flagged` /
`mphase_path_dependent_write_then_closing_flagged`,
`mphase_register_opening_beside_closing_not_flagged`; ui: `fail/pretick_mixed_anchors.rs`
is the `d2b` shape (it was `d2a`'s), `fail/pretick_path_dependent_drive.rs` (new,
`d1f`), `fail/multi_phase_out_write.rs` is the `e11` shape (it was `e4`'s), and
`pass/pretick_alignment_ok.rs` gained `mixed_anchors_held` (`d2a`),
`mixed_anchors_held_register_opening` (`d5b`) and `two_phase_mixed_anchors`
(`e4`); the exact-set tests `d_the_rederived_rule_flags_exactly_the_diverging_probes`
and `phase_e_the_narrowed_rule_flags_exactly_the_diverging_probes` assert both
halves and that no other rule claims any admitted probe.

**Found and not changed (here; closed in §10.6).** The path-dependent mechanism
in a *single-phase* port of a non-head segment (the `e11` shape with its
constant write removed) was outside both rules when this section was written:
`unprotected_pretick_out_write` examines the head segment only (widening it to
every segment was measured and rejected, §5.4 of the guardrail doc), and
`multi_phase_out_write` keyed on two phases. The model predicted the same
one-late lag; §10.6 measured it on both routes and clause 4 now reaches a
single non-head phase on the extraction route.

### 10.6 Closing the §10.5 gap — the path-dependent drive in a single non-head phase (derived 2026-10-30, then measured)

§10.5 left one shape of the path-dependent mechanism unmeasured: a plain `Out`
written **once**, in **one non-head phase**, by a drive an `In` read comb-reaches
on one path and that has a read-free path from the phase's entry on another —
`e11` with its constant write removed. Neither rule reaches it: clause (A) of
`unprotected_pretick_out_write` examines the head segment only (by the measured
decision of guardrail §5.4), and clause 4 of `multi_phase_out_write` is gated on
the port being written in two or more phases. Such a module compiles with no
opt-out, and its sweep verdict is decided by stimulus. This section measures it
on both lowering routes, each with its two uniform-anchor controls (the read on
every path, and on none). Method as in §9.2, §10.2 and §10.4: the emitted forms
were read off the transpiler first, every row was written before anything ran,
and the probes were run once.

**The forms, as the lowering emits them (confirmed by transpiling the six shapes
before writing the rows).** `cnt` is a register incremented in the head segment;
the write's phase is entered by a tick and left by a tick.

- **Extraction route** (`for _ in 0..2 { clk.tick().await; }` on either side of
  the write, so the write's phase is the `pc == 2` arm of the synthesized FSM;
  the port is held in every other arm). With the read in a branch of the arm,
  or on every path of it, the walk carries a read to the drive and it is
  **edge-sampled**: `always_ff … case (pc) 8'd2: o <= cnt;`. With no read it is
  phase C's **mux + hold**: `always_comb o = o_hold; case (pc) 8'd2: o = cnt;`.
  These are exactly `e11`'s first drive and `e12b`'s single drive, the forms
  §10.4 derived for the multi-phase port, with the constant arm gone.
- **Linear route** (`clk.tick().await` bare on either side of the write, so the
  write sits in the middle segment of a two-tick body, `phase_r == 1`). All three
  shapes — the read in a branch, on every path, on none — emit the same thing:
  `assign o = cnt;`, a **continuous assign of the committed register**, outside
  any phase guard. The linear route has no anchor at all for a register-valued
  drive: a `let` wire would go through the phase-guarded hold register (`e7w`),
  and a register goes through the continuous assign, whatever reads precede the
  write. So on the linear route the drive's codegen anchor is *opening* on every
  path, however it is spelled.

**The simulator side** is the same on both routes (`classify_reads` works on the
source): the read has a tick after it and the body has a register, so it is
deferred, and a write it reaches on a path is closing on that path — it runs in
the pre-edge settle of the phase's closing edge and is seen at the observation
after it. A write on a read-free path is opening: it runs in the post-edge settle
of the edge that opened the phase and is seen at that edge's observation.

**Predicted, then measured.** Period four on the extraction route (two folded
edges on either side of the write), two on the linear route; `x` is driven
`10 + k` and read for its barrier only.

| probe | route | drive: sim anchor per path / codegen anchor | emitted form | predicted | mechanism |
|---|---|---|---|---|---|
| `e12_path_dependent_single_phase_extracted` (NEW) | extraction | opening on even `cnt`, closing on odd / closing | edge-sampled | DIVERGE: the even-`cnt` write one observation late in the SV, the odd one agreeing — `e11`'s first drive with the constant gone. sim `[0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3]`, SV `[0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3]` | 1 (path-dependent), in a single non-head phase — the §10.5 gap |
| `e12a_read_on_every_path_extracted` (NEW) | extraction | closing / closing | edge-sampled | **AGREE**: both `[0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3]` | none — uniform |
| `e12b_read_on_no_path_extracted` (NEW) | extraction | opening / opening | mux + hold | **AGREE**: both `[0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3]` | none — uniform (`e9` without its read) |
| `e13_path_dependent_single_phase_linear` (NEW) | linear | opening on even `cnt`, closing on odd / opening (the continuous assign) | `assign o = cnt` | DIVERGE: the odd-`cnt` (closing) executions show one observation late in the **simulator** — the other direction from `e12`, because the linear form is the opening one. sim `[0, 1, 2, 2, 2, 3, 4, 4, 4, 5, 6, 6, 6]`, SV `[1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7]` | 1 on the simulator side; but see the next row |
| `e13a_read_on_every_path_linear` (NEW) | linear | closing / opening | `assign o = cnt` | DIVERGE: every execution one observation late in the simulator — sim `[0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6]`, SV as `e13`. A **uniform** closing drive, and the model still says it diverges: the linear route has no closing form for a register-valued drive, so the drive's anchors differ between the sides on *every* path, not because of a branch | a fourth way the anchors differ: **the route has no form for the simulator's anchor** — not the path-dependent mechanism |
| `e13b_read_on_no_path_linear` (NEW) | linear | opening / opening | `assign o = cnt` | **AGREE**: both `[1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7]` | none — uniform |

Two consequences the table commits to before the run. First, on the extraction
route the gap is exactly the path-dependent mechanism and nothing else: both
uniform controls agree, and the rule that closes it is clause 4's criterion with
the two-phase precondition dropped. Second, on the linear route the
path-dependent probe diverges for a **different** reason — the route realizes
every register-valued drive at the opening, so a deferred read before the write
is unrealizable on any path, branch or no branch — and the honest control for
that mechanism is `e13a`, which the model predicts diverges with no branch in
it. If `e13a` agrees, the model is wrong about the linear route's form and the
rules stay as they are until it is repaired. If it diverges as predicted, the
linear finding is outside the §10.5 gap (a missing emitted form, the class of
`e7w`, not a mismatched anchor the source could avoid by restructuring a branch)
and is recorded, with the rule decision for it taken separately below.

**Measured 2026-10-30 — every prediction matched on the first run, trace for
trace.** `e12` sim `[0, 0, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3]` vs SV `[0, 0, 1, 1,
1, 1, 2, 2, 2, 2, 3, 3, 3]` (the even-`cnt` write at index 5 against 6, the odd
ones agreeing); `e12a` both `[0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3]`; `e12b`
both `[0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3]`; `e13` sim `[0, 1, 2, 2, 2, 3, 4,
4, 4, 5, 6, 6, 6]` vs SV `[1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7]` (indices 0, 4,
8, 12 — the odd-`cnt` executions — one late in the simulator); `e13a` sim `[0,
1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6]` vs the same SV (every even index one late
in the simulator, no branch involved); `e13b` both `[1, 1, 2, 2, 3, 3, 4, 4, 5,
5, 6, 6, 7]`. Before any rule changed, all six compiled with no opt-out and no
rule flagged any of them — the gap as §10.5 described it, on both routes.

**The decision — the extraction route is refused, the linear route is
recorded.**

*Extraction route.* Clause 4 of `multi_phase_out_write` now reaches a port
written in **one** phase: when that phase is not the head's (the component the
loop head's successors and the trailing segment share), the body takes the
extraction route (`has_nested_tick`) and has a register, the port is refused
iff some write of it is comb-reached by an `In` read and has a read-free path
from its phase's entry (`Cfg::read_free_path_from_phase_entry`) — the
path-dependent half of clause 4, with the two-phase precondition dropped. The
immediate-read half stays gated on two phases (a single-phase immediate read is
a trailing read, `trailing_out_reads_input`'s `e8`), and clauses 1, 3 and 5 are
unchanged. The head phase stays D1's (`d1f` is its witness there, and the
`mphase_single_phase_head_segment_not_flagged` pin asserts the two rules do not
overlap on it). The function's name is now one phase too narrow and its
documentation says so; both front-ends' diagnostics name the single-phase
case, the mechanism (a write an input read reaches on one control path and not
on another) and the remedies (read the input before every write of the port in
its phase or before none, `RegOut`, or a phase no read reaches).
`EXPECTED_MULTI_PHASE` = `e7`, `e7w`, `e5`, `e11`, `e11b`, `e12`, `e4i`, `e5f`.
**Corpus cost: zero live modules** — the exact-set pin passed with `e12` as the
only addition, and the parity audit is unchanged in code and exact in result.
Pins: `cfg.rs` `mphase_single_phase_path_dependent_non_head_flagged` /
`mphase_single_phase_read_on_every_path_not_flagged` /
`mphase_single_phase_no_read_not_flagged` (one `else { let _ = x.read(); }` or
one deleted branch flips the verdict), `mphase_single_phase_head_segment_not_flagged`
(the D1 boundary), `mphase_single_phase_linear_route_not_flagged` (the route
boundary); ui `fail/path_dependent_single_phase.rs` (the `e12` shape) and
`pass/pretick_alignment_ok.rs::single_phase_read_on_every_path` /
`single_phase_no_read`; the six `e12*` / `e13*` tests and
`phase_e_the_narrowed_rule_flags_exactly_the_diverging_probes` in the battery.
The sv-baseline delta is the six new probe modules and no existing module (a
rule change, not an emission change); the mixed-anchor hold-form set is
unchanged (`e12b` is an all-opening hold-form port, `e12` and `e12a` are
edge-sampled, the `e13*` are continuous assigns).

*Linear route — found here, closed in §10.7.* `e13` diverges, but its every-path
control `e13a` diverges the same way with no branch in it, so a clause keyed on
the branch would refuse a proxy and admit the mechanism. The mechanism is a
**missing emitted form**: the linear route realizes a register-valued (or
input-valued) plain-`Out` drive as a continuous assign outside any phase guard,
which is the *opening* form on every path, so a drive the simulator runs at the
closing — behind a deferred read — lands one observation late at its phase's
opening observation whenever the assign's value changes at that phase's
opening edge. That is the class of `e7w` (the route's other anchor-blind form,
the phase-guarded hold register for a wire), not the mismatched-anchor class
the source can repair by restructuring a branch. The model says the same thing
about the **head segment of a multi-tick linear body**: `loop { let _ =
x.read(); o.write(cnt); clk.tick().await; cnt = cnt + 1; clk.tick().await; }`
is a closing head write of a register committed at the *other* edge, which
`assign o = cnt` shows one observation before the simulator does (on paper: sim
`[0, 0, 1, 1, 2, 2, …]`, SV `[0, 1, 1, 2, 2, 3, …]`); D1's clause (A) does not
reach it (every drive closing, so not mixed) and `pretick_out_write_before_update`
does not either (the update is in another segment, past a tick). In a
single-tick body the same spelling is the `fast_counter_corrected` idiom and
agrees, because the trailing update is forwarded into the next head segment and
there is one observation per iteration — so the discriminator is again how many
edges the body crosses, as in guardrail §5.4. The clause the model derives, for
the linear route (`!has_nested_tick`) crossing more than one tick in a body
with registers: *a plain-`Out` write that a deferred `In` read comb-reaches on
some path diverges unless every register it reads is assigned only within the
write's own phase and it reads no input* — the exemption being the case where
the assign's value cannot change at any edge but the write's closing one (the
`d3b` principle across segments). Constants are not exempt: a constant behind a
deferred read in the segment the first tick enters is a one-observation startup
transient (the class `out_first_driven_late` refuses). None of the clause's
boundaries has a measured witness yet, and **it is not landed**: the probes for
its exemption (a segment-local register), its input-operand case and the
head-segment case above are the first step, and its corpus cost over the
linear-route modules is the second (see the measurement below). `e13` and
`e13a` stayed in the battery as `*_unguarded_gap` tests with no opt-out until
§10.7 landed the rule the same day; they are `*_diverges_refused` now.

**The linear clause's corpus cost, measured by prototype 2026-10-30 (not
landed).** The clause as first written above — *a deferred read reaches the
drive, and its value reads an input or a register assigned outside its phase*
— was run as a scratch `Cfg` method over the 236 clocked modules of
`examples/`, `src/` and `tests/`. It flagged `e13`, `e13a` and **one live
module**: `examples/basejump/sipo_block.rs`, port `w3_dbg` — `let w3 =
data_i.read(); w3_dbg.write(w3);` in the last segment of a four-tick linear
body, a write behind a deferred read whose value is the input through a
same-phase `let`. That module is anchored against independent BaseJump Verilog
and sweeps green on every sequence with `w3_dbg` compared, so the clause was
over-wide, and the transpiled SV says why: `w3` is a phase-local **wire**
(`always_comb … if (phase_r == 2'd3) w3 = data_i;`), and the route emits a
wire-valued drive through the **phase-guarded flop** (`if (phase_r == 2'd3)
w3_dbg <= w3;`) — the *closing* form, which a closing write agrees with. So the
linear route's form is decided by the **operand's kind**, not by any read: a
register-valued (or constant) drive is a continuous assign — the opening form
— and a wire-valued drive is the phase-guarded flop — the closing form — and
a drive agrees iff the simulator's anchor on every path equals the form its
operand kind selects. `sipo_block`'s `w3_dbg` is the measured witness of the
wire-plus-closing cell (agreeing); `e13b` of register-plus-opening (agreeing);
`e13a` of register-plus-closing (diverging); `e13` of register-plus-mixed
(diverging). Refined with the wire exemption — *the drive's value reads a
same-phase local that is neither a register nor an input* — the clause flags
exactly `e13` and `e13a`: **zero live modules**. The cells it predicts and no
probe has measured are the wire-valued drive **without** a deferred read on
some path (an opening write through the closing form, one late in the SV —
`e7w`'s mechanism in a single phase), a direct input operand (`o.write(x.read())`,
whose form was not read off), and a constant behind a deferred read (a
startup transient). The clause is not landed: each unmeasured cell wants its
probe first, under the family's standing rule, and the refusal belongs in a
rule of its own (the mechanism is the route's, not clause 4's). The prototype
and its scan were deleted after the measurement; the numbers above are the
record.

### 10.7 Closing the linear-route gap — the form is chosen by operand kind (derived 2026-10-30, then measured)

§10.6 found that the linear multi-tick route realizes a plain-`Out` drive by
its **operand's kind** and not by any read: a register-valued, input-valued or
constant drive is a continuous assign outside every phase guard (the opening
form on every path) and a wire-valued drive — one whose value goes through a
same-phase `let` — is the phase-guarded flop (the closing form). The simulator
anchors each execution by whether a deferred read precedes it on the path. So
on this route a drive agrees iff the simulator's anchor on every path that
reaches it equals the anchor its form has, with one refinement for the
continuous assign: a *closing* register-valued write still agrees when the
assign cannot change at any edge but the write's own closing one and the value
the write forwards is the value that edge commits (§10.1's `d3b` principle
across segments). This section measures every cell of that statement before
any rule is written. Forms read off the transpiler first (twelve shapes, all
confirmed as stated above: `assign o = …` for `cnt`, `x`, `8'd7`; `if (phase_r
== …) o <= t;` for a `let t`, in the head, a middle and the trailing segment).

**The model's cells.** Two-tick linear body; `cnt` a register; `x` driven
`10 + k`; period two. "Head" is the segment before the first tick (its closing
edge is edge 0 and every even edge), "middle" the segment between the ticks
(closing edge odd), "trailing" the statements after the second tick (the
head's phase, run at the opening of the head's cycle).

| probe | segment | operand kind → form | sim anchor | predicted | mechanism |
|---|---|---|---|---|---|
| `e14a_wire_no_read_middle` (cell 1) | middle | wire → flop | opening | DIVERGE: SV one late. sim `[3, 3, 4, 4, …, 9]`, SV `[0, 3, 3, 4, 4, …, 8]` | opening write through the closing form (`e7w`'s, single-phase) |
| `e14b_wire_behind_read_middle` (twin) | middle | wire → flop | closing | **AGREE**: both `[0, 3, 3, 4, 4, …, 8]` | uniform — the `sipo_block` `w3_dbg` cell |
| `e14c_input_operand_middle` (cell 2) | middle | input → `assign o = x` | closing | DIVERGE: sim `[0, 11, 11, 13, 13, …, 21]`, SV `[10, 11, 12, …, 22]` | closing write through the opening form |
| `e14d_constant_behind_read_middle` (cell 3) | middle | constant → `assign o = 7` | closing | DIVERGE at the first observation only: sim `[0, 7, 7, …]`, SV `[7, …]` | the closing write lands at the second edge; a startup transient (`out_first_driven_late`'s class, which clears the segment the first tick enters on the assumption the write is opening there) |
| `e14e_head_closing_other_edge` (cell 4) | head | register committed at the other edge → `assign o = cnt` | closing | DIVERGE: sim `[0, 0, 1, 1, 2, 2, …, 6]`, SV `[0, 1, 1, 2, 2, …, 6, 6]` | the assign changes at an edge that is not the write's closing one |
| `e14f_head_closing_in_phase` (twin) | head | register updated in-phase before the write → `assign o = cnt` | closing | **AGREE**: both `[1, 1, 2, 2, …, 7]` | the forwarded value is the committing one; nothing changes at the other edge |
| `e14g_middle_closing_in_phase` (twin of `e13a`) | middle | as `e14f` | closing | **AGREE**: both `[0, 1, 1, 2, 2, …, 6]` | same |
| `e14h_head_wire_no_read` | head | wire → flop | opening | DIVERGE: sim `[2, 3, 3, 4, 4, …, 8]`, SV `[2, 2, 3, 3, …, 8]` | as `e14a`, in the head |
| `e14i_head_constant_behind_read` | head | constant → `assign o = 7` | closing | **AGREE**: both all `7` | the head's closing edge is edge 0, so the transient does not exist |
| `e14j_middle_write_before_update` | middle | register updated in-phase AFTER the write → `assign o = r` | closing | DIVERGE: sim `[0, 0, 0, 11, 11, 24, 24, 39, 39, 56, 56, 75, 75]`, SV `[0, 11, 11, 24, 24, …, 96, 96]` | the write forwards the pre-update value, the assign shows the committed one (V8a's generation hazard, off the head where `pretick_out_write_before_update` stops) |
| `e14k_trailing_wire_no_read` | trailing | wire → flop (hoisted into the head's phase, sampled at its edge) | opening | DIVERGE: sim `[0, 2, 2, 3, 3, …, 7]`, SV `[2, 2, 3, 3, …, 8]` | as `e14a`, in the trailing region (`linear_trailing`, register-valued, agrees) |
| `e14l_head_wire_behind_read` (twin of `e14h`) | head | wire → flop | closing | **AGREE**: both `[2, 2, 3, 3, …, 8]` | uniform |
| `e14m_trailing_wire_after_update` | trailing | wire → flop, the register updated before the wire | opening | DIVERGE: sim `[0, 3, 3, 4, 4, …, 8]`, SV written as `[2, 2, 3, 3, …, 8]` — measured `[3, 3, 4, 4, …, 9]`: the verdict held, the row's SV arithmetic missed that the shared map forwards the trailing update into the hoisted wire (`t = cnt + 1 + 2` in phase 0), so the SV runs one value higher and still one observation ahead | as `e14k`, with no write-before-update for V8 to claim |
| `e14n_head_closing_trailing_update` | head | register updated in the TRAILING region → `assign o = cnt` | closing | DIVERGE: sim `[0, 0, 1, 1, 2, 2, …, 6]`, SV `[0, 1, 1, 2, 2, …, 6, 6]` (as `e14e`) | the trailing update is committed at the last tick's edge (the shared-map form), the other edge from the head write's closing one — so "in-phase" means the write's own SEGMENT, not the Comb-component that merges the trailing statements into the head's |

The clause these rows commit to, for the linear route (`!has_nested_tick`)
crossing more than one tick, in a body with registers, per plain-`Out` drive
`w`:

- **wire-valued** `w` (its value reads a same-phase local that is neither a
  register nor an input): refused iff some path from its phase's entry reaches
  it with no deferred read — the flop samples at the closing edge, the
  simulator wrote at the opening (`e14a`, `e14h`, `e14k`; `e14b`, `e14l`
  admitted);
- **otherwise** (register, input or constant operands, the continuous assign):
  refused iff a deferred read comb-reaches `w` on some path **and** the
  assign can show a value the write did not publish: `w` reads an input
  (`e14c`), or a register some assignment of which lies outside `w`'s phase
  (`e13`, `e13a`, `e14e`; a trailing update counts as outside the head
  segment, since it commits at the last tick's edge — `e14n`) or is
  comb-reachable from `w` (`e14j`), or `w`'s segment is not the head's and its
  value is a constant (`e14d`; `e14i` admitted). Admitted: every operand
  register assigned only in `w`'s own segment and only before it (`e14f`,
  `e14g`). "Segment" here is the Comb-component with the loop head cut out,
  so the head segment and the trailing statements are distinct even though
  they run in one cycle.

If any row misses, the model is wrong and nothing lands.

**Measured 2026-10-30 — every verdict matched on the first run** (fourteen
probes; the traces are pinned in the battery and quoted in the rows above). The
one correction is to `e14m`'s SV row, an arithmetic slip about the shared-map
form and not a verdict. Before any rule changed, every one of the fourteen
compiled with no opt-out; the only rule that touched any of them was
`pretick_out_write_before_update`'s trailing clause on `e14k` (the wire reads
`cnt` through `t` before its update — V8's shape through a temporary, which is
why `e14m` puts the update first).

**The decision — a rule of its own, `Cfg::linear_out_write_anchor_mismatch`,
enforced in both front-ends.** The clause is the one derived above, verbatim:
on the linear route (`!has_nested_tick`) crossing more than one tick, in a body
with registers, a plain-`Out` drive is refused iff it is wire-valued and some
path from its segment's entry reaches it with no deferred read, or it is
register-, input- or constant-valued and a deferred read reaches it on some
path while the assign can show a value the write did not publish (an input
operand; a register assigned outside the drive's segment or after the drive; a
constant outside the head segment). "Segment" is the Comb-component with the
loop head cut out, so the trailing statements are a segment of their own
(`e14n`). Neither of the two existing rules could host it: clause 4's premise
(codegen holds a read-preceded drive to the edge) is false on this route, and
D1 is head-only by §5.4's measured decision — and the mechanism is the route's,
not a phase's. The function stays outside clause 4's path-dependent
criterion by construction: the extraction route, the single-tick body
(`d3b`'s shape, one observation per iteration), a register-free body and
`RegOut` return nothing. Where it overlaps another rule it is on a shape that
diverges and that both refuse (`e14k` with V8's trailing clause, `e7w` with
clause 1), which both front-ends handle by running every rule.

**Corpus cost: zero live modules.** `EXPECTED_LINEAR_ANCHOR` = `e13`, `e13a`,
`e14a`, `e14c`, `e14d`, `e14e`, `e14h`, `e14j`, `e14k`, `e14m`, `e14n`, `e7w` —
twelve probes and nothing else among the clocked modules of `examples/`,
`src/` and `tests/`; `sipo_block`'s `w3_dbg`, the first prototype's one live
hit, is the wire-plus-closing cell the clause admits (`e14b`'s), and sweeps
green as before. The parity audit gained the rule as its seventh entry and is
exact. The sv-baseline delta is the fourteen new probe modules and no existing
module (a rule, not an emission); the hold-form set is unchanged (nothing on
the linear route takes the mux + hold form). `e13` and `e13a` retired their
`*_unguarded_gap` tests for `*_diverges_refused` and carry the opt-out;
`e14b`, `e14f`, `e14g`, `e14i`, `e14l` compile clean. Both front-ends'
diagnostics name the two forms, the two directions of the lag (a wire late in
the hardware, a register early) and the remedies (for a wire, read an input
before every write of the port in its segment; for a register, update it in
the same segment before the write or read the input past the tick; `RegOut`).
Pins: `cfg.rs` `linear_*` (eleven, every boundary a flagged / not-flagged pair
one edit apart); `pretick_alignment_corpus.rs::linear_out_write_anchor_mismatch_flags_exactly_the_demonstration_modules`;
ui `fail/linear_anchor_mismatch.rs` (the `e13a` shape) and
`pass/pretick_alignment_ok.rs::linear_closing_in_segment` (`e14g`),
`linear_opening_register` (`e13b`), `linear_wire_behind_read` (`e14b`); the
`e13*` / `e14*` tests and `linear_the_rule_flags_exactly_the_diverging_probes`
in the battery.

**Two older scopes this closes.** `pretick_out_write_before_update` declared
the middle segment of a multi-tick loop unexamined (its R6 note): on the
extraction route it agrees (`e10`, edge-sampled), and on the linear route it
is `e14j`, refused by this rule as the "register assigned after the drive"
case. `out_first_driven_late` clears a constant written in the segment the
first tick enters on the assumption that the write is opening there
(`late_constant_one_edge`); behind a deferred read it is closing and lands at
the second edge (`e14d`), which this rule refuses as the "constant outside the
head segment" case — both doc comments now say so.

**Unmeasured, by construction on the refusing side or outside the rule.** A
register-free linear multi-tick body (the rule returns nothing; the
passthrough classification makes a direct `o.write(x.read())` immediate there,
and any other shape is unmeasured — clause 3's territory). A drive whose
operand mixes a wire and a register (`o.write(t + cnt)`) is treated as
wire-valued, which is what the lowering does with any expression over a
phase-local wire; no probe measured the mixed operand on its own.

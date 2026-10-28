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
that no rule guards (§9.4). The executor was **not** changed — scoping showed it already implements
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
  conditionally driven port whose drives are all opening is emitted as a forwarded
  combinational mux with a `<port>_hold` register (phase C); any other
  conditionally driven port is an edge-sampled register under `pc` guards. On the
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
both sides**, and since codegen's emission is port-wide, that means: either no
write is closing on the codegen side (no input read reaches any write of the port
— whatever the module reads in other states) or every write is closing on the
simulator side (each is reached by a deferred read, in a module with registers).
A port with one opening write and one closing write is
**mixed**: the closing write drags the port into the edge-sampled form and the
opening write lands one observation late. That is the "mixed generation" of §4 in
its multi-phase form — not a mixed-generation *value* but a mixed-anchor *mux* —
and it is a limitation of the emission's granularity (the drive-level form that
would realize it is described in §9.4), not of the denotation.

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
| `e4_closing_then_opening` | `x.read()` / `Bits::zero()` | closing / opening | edge-sampled | DIVERGE: the constant one cycle late |
| `e6_closing_via_wire_then_opening` | `let r = x.read(); o.write(r)` / `Bits::zero()` | closing / opening | edge-sampled | DIVERGE: as `e4` |
| `e5_two_closing` | `x.read()` / trailing `x.read() + cnt` (no tick after) | closing / **opening** (immediate read) | edge-sampled | DIVERGE: second write one cycle late |
| `e4i_register_free_passthrough_then_opening` | `x.read()` / `Bits::zero()`, no register | opening (passthrough) / opening | edge-sampled | DIVERGE: both one cycle late |
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
4. **mixed anchors** — some write of the port is opening in the simulator (no
   deferred read reaches it) while an input read reaches some write of the port,
   so codegen edge-samples the whole port: a deferred read reaching another write
   (witness `e4`, `e6`; flipping pair `e5d`, one tick added after `e5`'s trailing
   write), or a trailing *immediate* read reaching this write (witness `e5`; the
   `e8` mechanism inside a multi-phase port — derived for the all-opening case,
   `mphase_opening_write_then_trailing_immediate_read_flagged`).
5. **a wait loop's read before an opening write** — a folded tick-bearing loop
   that reads an input is no Comb predecessor of the write after it, so the
   simulator side counts the write as opening, while control extraction
   duplicates that write into the loop's exit arm under the read's condition,
   where codegen sees a read before it. Unmeasured; refused by construction
   (`mphase_wait_loop_read_before_opening_writes_flagged`).

Accepted, each measured agreeing: the **extracted all-opening** class — no input
read reaches any write of the port, whatever the module reads in other states
(`pulse_plain`, `e2`, `e3`, and since 2026-10-26 `e2x`) — and the **all-closing**
class (`e5d`). Unmeasured shapes fall on the refusing side by construction (clause
5; a body extraction handles only because of a `continue` counts as linear).
`pulse_plain` left `EXPECTED_MULTI_PHASE` and
keeps its opt-out for `out_first_driven_late` alone, which treats a constant past
a folded loop as late in the refusing direction although the module agrees
(§8-3's rule, conservative by design).

Pins: `copper-analysis/src/cfg.rs` unit tests `mphase_*` (one per clause);
`pretick_alignment_corpus.rs::EXPECTED_MULTI_PHASE` (the seven refused probes —
eight until `e2x` left it on 2026-10-26);
`copper-macros/tests/ui/fail/multi_phase_out_write.rs` (the `e4` shape) and the
two accepted shapes in `ui/pass/pretick_alignment_ok.rs`; the `e*` tests and
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
  `branch_merge_explicit`'s own lag is not among them: its `tail_o` is also
  written behind the deferred `sel` read — a mixed-anchor port, edge-sampled —
  and it still leads by one when the first cycle takes the `sel == 1` path
  (measured 2026-10-26), which the seeded sweep never does: the live twin
  `branch_merge` passes the sweep for that reason alone, a pre-existing blind
  spot recorded here and in `PRETICK_ALIGNMENT_GUARDRAIL.md` §5.5, not fixed.
  No referenced module (BaseJump, the pipelined CPU lane, the golden fixtures,
  `pattern_detector_010`, the CDC reference) changed. The D1 rules are untouched
  by that change: their witnesses now agree, and re-deriving the constant-write
  clause and the W4 class on that evidence is a decision of its own, not a side
  effect of an emission change.
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

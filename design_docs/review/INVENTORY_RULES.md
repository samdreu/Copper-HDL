# Rule and evidence inventory — 2026-11-03 baseline

Read-only inventory of the refusal rules as `copper-analysis` implements them, the
probe battery's verdicts, the `build.rs` tables, and every open or pending item found by
grep. Taken at commit `ea54d26`. Paths are relative to the repository root.

**Pointer (2026-11-05, D-13):** the SR/LL column of §(1) is the 2026-11-03 snapshot; its re-cut — each rule's place in II.1 / II.2 / II.3 / retired / I.10 / I.11 — is `SPEC.md` §II.3.2 (D-13 A5), which is the authority. **(2026-11-09)** Since D-8 (Phase C step 3, 2026-11-06) the plain-`Out` rules 7–15 are deleted from both front-ends; their CLOSED lines are SPEC §III.3.5.

## Summary

**Rule count: 16 refusal rules in `copper-analysis`.** There are also 4 analyses that
refuse nothing: `infer_registers`, `classify_reads`, `derivation_facts` and
`clock_phase_count`. About 8 more refusals live only in `copper-macros`: CDC, parameter
shadowing, single top-level loop, tickless body, clock construction, `panic!`-style
idioms, unprotectable `In` use, and signature shape. The macro checks in this order:
admissible → shadowing → [CDC] → reachability → nested-loop shape → awaits → register
initializers → memory staging → memory-result → collapse → D1 → V8 → trailing `RegOut`
→ trailing `Out` → linear → multi-phase → late constant. Definite assignment runs only
in combinational mode.

**Probe ledger:** about 159 modules in `tests/sequential_forwarding_divergence.rs`.
- **92 admitted and agree.** Seven of them carry an opt-out that no rule needs any
  more: e2, e3, e5d, e2x, e9, e8l, e8lr.
- **62 diverge and are refused.**
- **3 refused for a lowering reason:** e7 is refused by `vlir_lower` as multiply driven;
  e16g and e16k are refused by Verilator as two drivers. e16i's pin depends on the
  Verilator version (5.044 builds it and the drivers fight; 5.051 refuses it).
- **2 agree but are refused (over-refusals):** e18j (`pretick_out_write_before_update`
  value clause, pinned as "a legality decision") and pulse_plain
  (`out_first_driven_late`, which agrees with its SV,
  `a_plain_out_driven_in_two_phases_agrees_under_phase_c_emission`).

**Admitted and DIVERGES: none in the battery. Each verdict-table test asserts that the
refused set equals the diverging set.** One unresolved case outside it:
- **`tests/fixtures/probe_timing_dut.rs::probe`** has a `build.rs` SKIP row that says it
  is "an `allow_pretick_alignment` witness… sim ≠ SV is its purpose". The source is
  plain `#[hardware(sequential)]` with no opt-out. Tracing the rules by hand, nothing
  flags it. No test pins its sim-vs-SV result. Either the SKIP row is stale, or this is
  an admitted divergence. It needs measuring (→ `MEASUREMENTS.md`).

**Contradictions in the code:**
- The `out_first_driven_late` doc calls pulse_plain "a measured transient", but the
  battery pins it agreeing.
- The same doc links to `unprotected_trailing_out_write`, which no longer exists.
- The `pretick_alignment_corpus.rs` header still says the middle-segment gap is
  "theoretical"; e10 and e14j have since measured it.
- The `mphase_single_phase_linear_route_not_flagged` comment calls e13 "an unguarded
  gap", but the linear rule now refuses it.

**Open decisions and owed items:**
1. e18j over-refusal: admitting it "is a legality decision".
2. `trailing_regout_reads_input` doesn't look at guards ("unmeasured there", §10.8).
3. `multi_phase_out_write`: the wait-loop opening write is "unmeasured, refused by
   construction", and so is a body extracted only because of a `continue`.
4. `out_first_driven_late`: a closing write after the first tick on the extraction route
   is unmeasured.
5. Empty constant `for` range treated as a clock boundary (`TODO:495`, `TODO:1841`).
   Closing it needs const evaluation.
6. `Bits::x()` is dropped in the SV and Verilator is 2-state
   (`tests/x_propagation_and_reset.rs:293`, KNOWN GAP).
7. An undriven `Out` isn't rejected at compile time (`TODO:1366`).
8. "Warn on persistent-state mutation before first tick" is deferred, "Design call
   pending" (`TODO:2099`).
9. `mod8`: an X-aware comparison is "the alternative recorded, not taken".
10. `check_admissible` runs in the macro only; the transpiler never calls it. It is
    calibrated one way (admissible Err ⟹ transpile Err), and there is no ui/fail case.
11. The `TODO` file was last committed 2026-08-27, older than the code.

**Semantics or lowering?** Reachability, awaits, register initializers, memory staging,
definite assignment and the type grammar are justified as language rules. Every rule in
the pre-tick alignment family (D1, V8, trailing ×2, linear, multi-phase, late constant,
memory-result) is justified by "the emitted form cannot show the simulator's trace". The
linear rule even tabulates the transpiler's forms. They sit in `copper-analysis`, which
the code calls the home of semantic rules, but each one's boundary moves whenever
codegen changes.

---

## (1) Rules

Abbreviations: **M** = macro (`copper-macros/src/lib.rs` 925–1385), **T** = transpiler
(`copper-codegen/src/lib.rs::vlir_for_target` 346–549), **OO** = honours
`allow_pretick_alignment`, **SR/LL** = how the code comment justifies it (semantics rule
/ lowering limitation).

| # | Rule | Clauses as implemented (any one refuses) | OO | M | T | Pins | SR/LL |
|---|---|---|---|---|---|---|---|
| 1 | `check_admissible` | (1a) an `In`/`Out`/`RegOut` payload whose type is not Array/Ref/Paren of, or a last path segment in {Logic, Bits, bool, u8..u128, usize, i8..i128, isize, Clock, In, Out, RegOut, Memory}; (1b) an annotated `let` with such a type. Unannotated lets aren't checked; array lengths aren't checked. | no | yes, all modes | **no** | `copper-codegen/tests/admissible_calibration.rs` only; no ui case, no unit test | SR ("positive grammar"), calibrated never to over-reject |
| 2 | `check_reachability` | With tick edges deleted, a Comb cycle reachable from head, or a cycle through a folded nested loop marked `may_exit_without_tick` (while always; for/loop unless `ticks_before_any_break`); recurses into nested ticking sub-CFGs. A top-level `continue` goes to head. | no | yes | yes (first) | about 16 cfg unit tests (`tickless_fallthrough_rejected` … `tick_free_nested_loop_accepted`); analysis_semantics (4, plus 2 for `continue`); real_examples corpus; ui `partial_tick_path`, `for_break_before_tick`; unsupported_constructs | SR (core invariant) |
| 3 | `check_nested_loop_shape` | Tick-free nested `loop` or `while` (a tick-free `for` is OK); a tick-bearing body whose bare tick isn't the last statement (no bare tick: last statement must be a nested loop that ends at a boundary); separate messages for `for`, tick-first, and ticks only in arms | no | yes | yes | shape.rs (6); ui `tick_first_wait`; front_end_parity; unsupported_constructs | SR ("outside the language by decision", 2026-08-24) |
| 4 | `check_awaits` | Any `.await` that isn't `<recv>.tick().await` | no | yes | yes | shape.rs (1); ui `foreign_await` | SR |
| 5 | `check_register_initializers` | A pre-loop `let` (stops at the loop), named by a simple identifier, that is in `registers()`, with an initializer that fails `is_constant_expr` (literal; multi-segment path; SCREAMING_CASE identifier; multi-segment-path call with constant args, which covers `Bits::x()`; paren/cast/unary/binary/array/repeat over constants) | **no** | yes | yes | init.rs (4); `register_initializer_corpus` EXPECTED=[]; ui fail `register_init_not_constant`, pass `register_init_constants_ok` | SR; constants the codegen evaluator can't fold are a lowering limitation |
| 6 | `check_memory_staging` | (a) memory access inside a tick-free nested loop; (b) two stagings on the same (mem, bus, port) that reach each other tick-free (walking through head); (c) an observe with no staging anywhere (top level only); (d) an observe that every staging reaches tick-free. Recurses into sub-CFGs with (c) off. | no | yes | yes | cfg unit tests (11 `mem_check` tests); unsupported_constructs (memory_*, two_accesses_*, extracted variants) | SR ("Copper's memory model") |
| 7 | `memory_result_drives_plain_out` | ≥2 Comb phases, and some node observes memory, and a plain-`Out` write node observes directly or uses a non-register local tainted by an observation | no | yes | yes | cfg (5); ui `memory_result_drives_plain_out`; unsupported_constructs | LL ("no correct emitted form"); the vlir copy is listed as a RULE in phase_sensitive_checks |
| 8 | `multi_write_collapse` | Bare tick t (`writes` empty, which excludes folded loops); port p written comb-after t; a non-folded write w1 of p that comb-reaches t and is comb-reached by an `In`-reading node; recurses into nested ticking loops | **no** | yes | yes | analysis_semantics (8); ui fail `multi_write_collapse`, pass `multi_write_regout_ok`; front_end_parity asserts the corpus set is empty | about sim/synth divergence (the coroutine executor collapses the writes) |
| 9 | `unprotected_pretick_out_write` (D1) | Requires registers ≠ ∅. Per port over non-folded pre-tick drives: **A** = some drive read-reached ∧ some drive read-free-reachable ∧ (path-dependent drive ∨ (written on all paths ∧ ¬linear-multi-tick)) ∧ observable (more than 1 drive ∨ held ∨ a drive reading a register that a read-free path assigns before it); **B** = a drive with no leading read whose guard tests a register assigned comb-before the guard | yes | yes | yes | EXPECTED_FLAGGED (10); cfg hazard_*/d1–d4 tests; battery `d_the_rederived…`, `single_tick_default_override…`; ui `pretick_alignment`, `_forwarded_guard`, `_mixed_anchors`, `_path_dependent_drive`; pass `pretick_alignment_ok` | LL-justified ("emitted SV cannot align") |
| 10 | `pretick_out_write_before_update` (V8) | **Pre-tick:** write w read-reached ∧ value_registers(w) ≠ ∅ ∧ a later region node redefines one of them. **Guard edition (single-tick only):** a register in a guard of read-reached w is redefined after the guard ∧ the port is written on all head paths. **Trailing:** same value test in the trailing region, no read needed. **Single-tick trailing-update:** read-reached head write whose value or guard registers ∩ trailing-region register defs ≠ ∅ ∧ every-path port. | yes | yes | yes | EXPECTED_WRITE_BEFORE_UPDATE (14); cfg v8*/v8e* tests; ui `write_before_update`, `_via_temp`, `_via_bool_temp`, `pretick_guard_update`, `single_tick_trailing_update` | LL; e18j over-refusal recorded |
| 11 | `trailing_regout_reads_input` | A `RegOut` write in the trailing region whose value reaches an `In` read (through non-register temps) at a site with no tick comb-reachable from it | yes | yes | yes | EXPECTED=[]; cfg (4); ui `trailing_regout_reads_input` | LL |
| 12 | `trailing_out_reads_input` | crosses more than one tick ∧ a trailing plain-`Out` write whose value input site, or a guard that reads `In`, has no tick comb-reachable from it | yes | yes | yes | EXPECTED (11); cfg (7+1); ui `trailing_out_reads_input` | LL |
| 13 | `linear_out_write_anchor_mismatch` | ¬nested tick ∧ crosses more than one tick. Per drive: **A** two drivers (same segment, opposite conditionality); **B** conditional ∧ trailing; **C** conditional ∧ port wire-valued ∧ (¬deferred ∨ read-free path); **C** conditional ∧ ((read_before ∧ (¬deferred ∨ read-free path)) ∨ (¬read_before ∧ forwarded guard)); **2** unconditional wire ∧ opening on some path ∧ ¬(constant wire in head); **3a** unconditional with a direct input operand; **3b** deferred ∧ (operand register defined in another segment or after w ∨ constant outside head). In a register-free body the drive's own reads don't count as deferred. | yes | yes | yes | EXPECTED_LINEAR_ANCHOR (36); about 25 cfg `linear_*` tests; 3 battery tables; ui `linear_anchor_mismatch`, `_regfree_passthrough`, `_trailing_conditional` | LL (forms "read off the transpiler") |
| 14 | `multi_phase_out_write` | Phases = Comb components. **One phase:** non-head phase ∧ nested tick ∧ has registers ∧ a write that is both read-reached and read-free from its phase entry. **≥2 phases:** ¬nested tick (linear) ⇒ refuse; register-free ∧ reads any input ⇒ refuse; a write read-reached ∧ (no deferred read ∨ read-free from phase entry) ⇒ refuse; some write not deferred-reached ∧ a folded tick-bearing loop reads input ⇒ refuse | yes | yes | yes | EXPECTED_MULTI_PHASE (8); cfg `mphase_*` (20); battery `phase_e…`; ui `multi_phase_out_write`, `path_dependent_single_phase` | LL |
| 15 | `out_first_driven_late` | crosses more than one tick ∧ port's every non-early write is `const_write` ∧ no write in early region (head + segment after first bare tick; stops at folded loops) | yes | yes | yes | EXPECTED_LATE_CONSTANT (4); cfg `late_*` (9); ui `out_first_driven_late` | LL ("no emitted form"; restrict-don't-grow 2026-10-23) |
| 16 | `check_definite_assignment` | Combinational only: a port in MAY \ MUST at exit (folded loops write every port; ports never written aren't flagged) | no | combinational mode only | yes (no-op when sequential) | cfg (5); analysis_semantics (3); real_examples; ui `combinational_latch` ×3, pass `combinational_mux_valid`; `comb_definite_assignment_parity.rs` | SR (latch) |

**Helpers the rules share:**
- `comb_reaches`: Comb-only, stops at head; `b` may be the start.
- `leading_read_reaches(w)`: some node with `uses ∩ inputs` comb-reaches w, w itself included.
- `read_free_path_reaches`: head entries to n without passing an input-reading node, n included; with `assigning`, the path must also define one of the given registers.
- `read_free_path_from_phase_entry`: also starts at Tick-edge targets.
- `deferred_read_reaches`: a non-folded read node that reaches w and reaches a tick.
- `comb_reaches_tick`.
- `tick_free_reaches`: walks through head.
- `written_on_all_paths`: memoized; folded nodes never count; a path reaching a tick or head counts as not written.
- `value_sources` / `value_registers`: reaching definitions, stopping at registers and inputs.
- `guards`: branch nodes recorded per arm range.
- `has_nested_tick`: a tick that isn't a bare top-level statement.
- `crosses_more_than_one_tick`: ≥2 tick nodes, or a folded tick node.
- `passthrough_read`: in a register-free CFG, `<comb_out>.write(args)` reads are Immediate.
- `classify_reads`: Deferred iff a tick follows in the iteration; condition and scrutinee reads also see ticks in branches.

## (2) Probe ledger (`tests/sequential_forwarding_divergence.rs`)

**Diverges, refused (62)**, with the refusing rule:
- **D1:** w4, d1f, d1g, d2b (also V8 guard edition), d3c, d4a, d4c, e17c, e17e, e17f
- **V8:** v8a, v8d, v8t, e17d, e17h, e17j, e18b, e18c (e14k is listed under the linear rule below; V8 also refuses it)
- **multi-phase:** e5 (also trailing `Out`), e4i, e5f, e11, e11b, e12, e7w (also linear)
- **trailing `Out`:** e8, e8p, e8m (also linear), e15ai
- **linear:** e13, e13a, e14a, e14c, e14d, e14e, e14h, e14j, e14k (also V8), e14m, e14n, e15a, e15b, e15g, e15h, e15j, e15l, e15n (also trailing), e15o (also trailing), e15q, e15u (also trailing), e15z (also trailing and late), e15aa (also trailing and late), e15ab (also trailing), e16d, e16e, e16f, e16i, e16l, e16m, e16n, e16o
- **late constant:** e15p

**Refused for a lowering reason (3):** e7 (multi-phase; `vlir_lower` multiply driven), e16g and e16k (linear clause A; Verilator MULTIDRIVEN / BLKANDNBLK).

**Agree but refused (2):** e18j (V8 value clause), pulse_plain (late constant).

**Agree and admitted (92):**
- add_then_write, add_with_leading_read, fast_counter, fast_counter_corrected, pulse_registered, trailing_update, pc_arm_write, pc_arm_toggle, branch_merge, branch_merge_explicit
- v5, v7, v8b, v8c, linear_trailing, branch_trailing
- e2, e3, e5d, e2x, e9, e8l, e8lr (carry an opt-out no rule needs any more), e4, e6, e4r, e4f, e10
- d1b, d1c, d1d, d1e, d2a, d2c, d3a, d3b, d4b, d4d, d5a–d5f
- e12a, e12b, e13b, e14b, e14f, e14g, e14i, e14l
- e15c/d/e/f/i/k/m/r/s/t/v/w/x/y/wr/ac/ad/fr/af/ag/ah
- e16a/b/c/h/j/p/q, e17a/b/g/i/k, e18a/d/e/f/g/h/i

**ADMITTED AND DIVERGES: none in this file.** The one open case is `probe`, described in the summary.

## (3) `build.rs` tables

**SKIP:**
- `probe`: "`allow_pretick_alignment` witness… sim ≠ SV is its purpose" (contradicted by the source, which has no opt-out)
- `rv32i_cpu_pipelined`: anchored outside the sweep by `rv32i_pipelined_verilator.rs`; its `Memory` parameter can't be supplied by the harness
- `out_from_reg_before_commit`, `vr_sink_mealy`, `vr_skid`: one-cycle divergence, guarded by V8 (trailing clause / temporaries), kept as opt-out demonstrations
- `two_domain_top`: structural parent, transpile-only
- `mod8`: `b == 0` is X; the sweep does no stimulus masking; an X-aware comparison was recorded but not taken

**RESET:** (shift_register, rstn, active-low), (simple_counter, reset_i, active-high), (rv32i_cpu_transpilable, rstn, active-low). The lfsr row was removed 2026-10-29 by the register-initializer rule.

**REFERENCE:** ram_read_first, bit_not_bits, handshake (all under `tests/fixtures/reference_sv/`).

**PARAMS:** det_for N=3; mux 8/4/2; priority_encode 8/3; ripple_carry_adder 8; rotate_right 8/3; shift_register 8/7; wide_alu 32.

**Codegen pins:**
- `hold_form_audit.rs`: 20 mixed-anchor ports — branch_merge ×2 (control_extraction_dut and the battery) plus branch_merge_explicit (tail_o); d2a; d5a–d5f; e11; e16a, e16b, e16h, e16q; e4, e4f, e4i, e4r, e6.
- `phase_sensitive_checks.rs` EXPECTED: chir `lower_expr_stmt`, `lower_seq_body`, `nested_loop_error`, `reject_tick_in_branch`, `validate_stmts`; shir `check_no_tick_in_branch`, `lower_seq_body`, `validate_seq_chir`; vlir `lower_seq`, `reject_memory_driven_comb_outputs` (marked RULE); plus any entries after line 110, not read.
- `register_reconciliation.rs`: `tests/fixtures`, sequential and synchronizer modules; codegen registers minus inferred registers ⊆ {phase, pc, `<port>_hold`}; WITNESSES=[].
- `unsupported_constructs.rs`: 33 tests (division, wait shapes, memory, counted `for`, `continue`).

## (4) Open, pending and unpinned items found by grep
- `copper-analysis/tests/pretick_alignment_corpus.rs:696` and `cfg.rs:950`: e18j over-refusal, "admitting it is a legality decision"
- `cfg.rs:1241`: guards not considered by the `RegOut` trailing rule ("unmeasured")
- `cfg.rs:1428` / `:1551` / `lib.rs:158`: wait-loop shape and `continue`-only extraction, "refused by construction"
- `cfg.rs:1950`: extraction-route closing late constant, unmeasured
- `pretick_alignment_corpus.rs:428`: e8 class "found unguarded and the user chose to refuse" (resolved)
- `cfg.rs:5624`: stale "unguarded gap" note on e13
- `seq_fwd:800` / `:1507`: stale "measured gap, NOT yet guarded" header
- `tests/x_propagation_and_reset.rs:293`: KNOWN GAP (X dropped; Verilator 2-state)
- `TODO:495` / `:1841`: KNOWN GAP (empty constant `for` range)
- `TODO:1366`: undriven `Out` is an open language question
- `TODO:2099`: "Design call pending"
- `TODO:383`: cause Q still open
- `build.rs:136`: X-aware comparison "recorded, not taken"
- `examples/uart/system.rs:36`: "phase … not pinned unless an input read precedes it" (design comment)

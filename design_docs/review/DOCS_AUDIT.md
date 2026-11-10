# Documentation audit — phase 1 inventory (2026-11-09, tree c81b59f)

**What this is.** An inventory of every documentation file in scope, classified against the
tree as it stands at c81b59f (MEASUREMENTS §AF, ledger 234/234/0), with each stale claim given
as `file:line` plus the grep evidence. **No doc was edited in phase 1.** Phase 2 applies the
fixes once the user has decided the open items in §1 and batch J has merged.

**Scope.**
- Every file under `design_docs/` except `review/MEASUREMENTS.md`, which is an append-only log.
  `SPEC.md` was checked for its status line only, since the conformance ledger (G-K) is its check
  against the tree. `SPEC_DECISIONS.md` is a dated decision log and was not audited line by line.
- The root `README.md`, `copper-ref/README.md` and `tools/stats/README.md`.
- Crate-level `//!` docs in each `lib.rs`.
- The usage headers of the `tools/` scripts.
- `CLAUDE.md`, which is recorded here and edited only in phase 2.
- There are no `examples/**/README*` files, and `tests/fixtures/` has no notes file.
- `design_docs/OUTDATED/` was checked only for its label.
- Nothing under `paper/` was read or touched.

**Method.**
- Every doc was read in full.
- Every function, type, path, test, guard, env var and flag a doc names was grepped in the code:
  `*.rs`, `build.rs`, `tools/`, `Cargo.toml`. Other docs were not used as evidence.
- Changes were dated from `SPEC_DECISIONS.md` (D-1..D-30) and `MEASUREMENTS.md` (§A..§AF).
- Five read-only audit agents each took a group of `design_docs/`; their full per-doc findings
  make up the appendix (§4). I spot-checked their headline claims against the tree:
  - SPEC.md:3; CYCLE_DATAFLOW_SEMANTICS.md:1;
  - `cfg.rs` doc links to `multi_phase_out_write` / `multi_write_collapse`;
  - `iterate_scc` has 0 hits; `ReadMode` has 0 hits;
  - `tests/sequential_forwarding_divergence.rs` has 1 `#[test]`;
  - `G-D NOT SATISFIED` has 0 hits; SPEC_OVERVIEW.md:86 "twelve rows".
- One agent claim was corrected: TIMING_MODEL_UNIFICATION.md has code referrers (§2).
- A resolver script pulls every backticked path and identifier out of a doc and reports those
  with 0 hits in the code. It lives in this session's scratchpad, not the repo; phase 2 reuses it
  for its check.

**Measured along the way.**
- `tools/transpile_coverage.sh --quiet` printed `transpiler coverage: 40/40 modules`.
- The corpus has 391 `#[hardware]` modules in `examples/` + `tests/fixtures/`: 306 sequential,
  81 combinational, 3 synchronizer, 1 structural.
- `build.rs` SKIP has 3 rows, so 388 modules are swept. REF_SKIP and REF_SKIP_PATTERN are empty.
- The stimulus-policy constants at `build.rs:247-263` match the docs and
  `the_stimulus_policy_is_what_the_design_doc_says`.

---

## 1. Decisions needed from the user (each with a recommendation)

| # | Decision | Recommendation |
|---|---|---|
| Q1 | **Adopt SPEC.md as normative.** `SPEC.md:3-5` still says "UNDER CONSTRUCTION … Not yet normative. Until this document is adopted, `SYNCHRONOUS_SEMANTICS.md` remains the reference and wins". In turn, `SYNCHRONOUS_SEMANTICS.md:3-5` calls `CYCLE_DATAFLOW_SEMANTICS.md` normative. Read literally, the deleted two-job model is the authority. No adoption is recorded: SPEC_DECISIONS uses "when SPEC is adopted" only in the future tense (:723, :921, :1118). | Adopt: a dated status line on SPEC.md (and a D-entry or note if the user wants one). Batch J owns CLAUDE.md's version of this; SPEC.md:3-5 is a separate line, so confirm who edits it. |
| Q2 | `SYNCHRONOUS_SEMANTICS.md`: label it historical in place, or rewrite it as a current summary. | Label it in place with a dated header, "superseded by SPEC.md Part I (D-8, Phase C)", and drop "normative" at L3-5 and L29. Keep the path: a test message (`unsupported_constructs.rs:138`) and about 15 code comments cite it. SPEC_OVERVIEW.md becomes the "start here". |
| Q3 | `PRETICK_ALIGNMENT_GUARDRAIL.md` and `CYCLE_DATAFLOW_SEMANTICS.md`: give them a header in place, or move them to `OUTDATED/`. | Header in place. About 30 inbound refs (code comments, SPEC_DECISIONS:1181, test headers) would break on a move. For CYCLE_DATAFLOW, mark §10.12 (L1927-1982) "still current". Neither doc's section numbers may move. |
| Q4 | `DERIVATION_TABLE.md` and `PAIRED_IMPLEMENTATION_SCOPE.md`: give them a header in place, or move them. | Header in place, "pre-D-8 record". DERIVATION_TABLE is cited by SPEC_DECISIONS:2590 and 3 code comments. |
| Q5 | `TIMING_MODEL_UNIFICATION.md`: give it a header in place, or move it to OUTDATED/. | Header in place. It is cited by `copper-analysis/src/cfg.rs:583` and by `tests/timing_model_derivations.rs:24,233` (a test's failure message tells the reader to record fixes there). Its lasting conclusion, that phase checks belong on the source, is already in CLAUDE.md. |
| Q6 | `FORMAL_SEMANTICS.md`, the 2026-11-02 draft whose two-instant φ D-8 rejected: give it a SUPERSEDED header in place, or move it. | Header in place. SPEC_DECISIONS.md and FORMAL_SEMANTICS_SURVEY.md cite it. |
| Q7 | `LEVELIZED_SCHEDULING_SCOPE.md`: add a historical header with dated inline notes, or move it. | Header and notes in place. executor.rs, port.rs, `levelized_differential.rs` and CLAUDE.md cite it. |
| Q8 | `TIMING_COVERAGE_MATRIX.md`: fix it in place, or fold it into ANCHORING_A_MODULE.md. | Fix in place (6 stale claims; the anchors are real). |
| Q9 | `CORPUS_DIFFERENTIAL_SWEEP.md`: restate SPEC §III.2 (the relation, the verdicts, the reference leg, observation 0), or point to it. | Point to SPEC §III.2 with a short summary. Fix the 14 stale claims in place. |
| Q10 | `OUTDATED/README.md` is an old copy of the project README (aa25fda, 2026-07-27). It calls docs "canonical", lists 18 of 31 entries, and links a `PROGRESS.md` that does not exist. | Rename it `README_2026-07.md` and write a new short dated index of all 31 entries, headed "historical, not authoritative". |
| Q11 | `copper-core/src/memory.md` is an orphan brainstorm outline (32 lines) that nothing references. | Delete it. Its content is questions answered by SPEC §I.10 and D-16/D-25. |
| Q12 | `prior_art/`: the "Transfer to Copper" sections of the 16 notes from 2026-11-02 predate D-8. | Add one dated sentence to `prior_art/README.md`, plus a one-line header on `prost.md` §8.3, which is the most overtaken since D-8 adopted Prost's one-job model. Do not rewrite the notes. |
| Q13 | `review/INVENTORY_CODE.md` and `INVENTORY_DOCS.md` are 2026-11-03 baselines with no pointer to what replaced them. INVENTORY_RULES.md already has one at L7. | Add a dated one-line pointer header to each (SPEC cites INVENTORY_CODE by item number), and extend INVENTORY_RULES' L7 with the D-8 deletions. |
| Q14 | The `//!` fixes in `copper-ref/src/lib.rs` and `copper-analysis/src/lib.rs` touch `.rs` files, so bare `tools/regression.sh` must run. | Do them in phase 2 and run the bare regression. For copper-analysis, add a short current overview on top and keep the G6 history below it. |
| Q15 | `ANCHORING_A_MODULE.md` uses "reference leg", which now collides with the copper-ref `_reference_leg` case name. | Rename it, e.g. "independent-Verilog leg". |
| Q16 | `SPEC_OVERVIEW.md` §6 ("twelve rows today", owed to Phase C) and §4.6 (power-on "zero of its type", against SPEC I.7.4). SPEC.md:3209 and :3831 already record these edits as owed. SPEC_OVERVIEW is SPEC's "generating page" (SPEC.md:9). | Fix in place. It is wording on a normative-adjacent page, so the user should confirm. |
| Q17 | Stale **code comments** found in passing are outside this docs-only scope (§3). | Handle them in a separate code-comment pass with its own regression. |

---

## 2. Inventory — one row per doc

Classes: **CURRENT** (accurate today), **STALE** (meant to be current, parts now wrong),
**HISTORICAL** (a record of past work; keep and label), **SUPERSEDED** (fully replaced),
**ORPHAN** (unreferenced, no lasting value).

### design_docs/ (top level)

| Doc | Class | Header today | Stale claims (count; key `file:line`) | Proposed action |
|---|---|---|---|---|
| SPEC.md | STALE (status line only; body checked by G-K) | — | 1: L3-5 "Not yet normative … SYNCHRONOUS_SEMANTICS.md … wins" | Q1 |
| SPEC_DECISIONS.md | CURRENT (dated log; not audited line by line) | — | — | none |
| SPEC_OVERVIEW.md | STALE (minor) | — | 6; L86 "twelve rows today" (II.3.3 now 15 rows, ledger 234/234/0); §4.6 power-on wording vs SPEC I.7.4; status line; §8 | fix in place (Q16) |
| SYNCHRONOUS_SEMANTICS.md | STALE (in substance HISTORICAL) | none; L3-5 says "normative" (via CYCLE_DATAFLOW) | ~40 claims in ~25 passages. Two-instant executor / barrier: L47-67, L79-91, L182-186, L310-324. `set_poll_phase`/`PollPhase`: L98-100. Read classifier / `ReadRewriter`: L541-583, L712-716. Plain `Out` and the pre-tick family as live: L141-148, L218-539. Back-edge clause: L197-203, L763-766. `is_constant_expr` as R11's predicate: L126. `ReadMode`: L625. `memory_result_drives_plain_out`: L634-673. `iterate_scc`: L851-859. The trailing fold: L172-196. "Five mechanisms" omits G-F..G-K: L884-932. "CHIR interpreter not built": L20-26 (copper-ref exists). | dated historical header (Q1, Q2) |
| SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md | HISTORICAL | yes (dated status notes) | 11; L5 defers to SYNCHRONOUS_SEMANTICS.md as authority | fix header L5 → SPEC; add one dated "since D-8" line |
| FORMAL_SEMANTICS.md | SUPERSEDED (by SPEC.md; D-8 rejected its two-instant φ) | no; L6 defers to SYNCHRONOUS_SEMANTICS.md | ~14 | dated SUPERSEDED header in place (Q6) |
| FORMAL_SEMANTICS_SURVEY.md | HISTORICAL (the survey keeps its value) | no | 9 (Copper-position paragraphs are pre-D-8) | dated header marking those paragraphs pre-D-8 |
| PRETICK_ALIGNMENT_GUARDRAIL.md | HISTORICAL | no; L3-175 opens "the family is GUARDED … the rules as they stand" | ~25 ranges. Rules table, opt-out, pins and "55 #[test]s": L104-153. Barrier / `inject_synced_reads`: L198-203. Deleted pins: L222-228, L296-303. "D1 and D2 cancel": L263-270. All of §5.8: L959-1131. "4c OPEN — retire this doc": L1326. "Copper currently has neither": L1396, L1446. The 14 ui files and `pretick_alignment_corpus.rs` it names are missing. | dated historical header in place (Q3) |
| CYCLE_DATAFLOW_SEMANTICS.md | HISTORICAL (§10.12 CURRENT) | no; L1 "the normative timing model", L35 "The denotation is normative" | ~20 ranges. The two-anchor model, which is D-8's reversed two-job body: §2-§3, L163-235. The back-edge clause: L132-133. Plain `Out` anchors: L158. `forward_opening_drives`, `seen_in_read`, `edge_value`, `SHIRPortDrive` (0 hits): L224-235. `memory_result_drives_plain_out`: L256, L307. `leading_read_reaches` / `written_on_all_paths`: L270-284. `has_nested_tick` / `out_first_driven_late`: L374-388. §9 and §10.1-10.11: L399-1926. The L130 "(§10.5)" pointer is wrong; the Prost section is PRETICK_ALIGNMENT_GUARDRAIL §10.5. | dated header, mark §10.12 current, fix L130 (Q3) |
| CORPUS_DIFFERENTIAL_SWEEP.md | STALE | — | 14. Says nothing of `_reference_leg`, §III.2, the verdict ledger, observation 0, REF_SKIP / REF_SKIP_PATTERN, `SweepDomain` or `corpus_manifest.rs`. L248 "one `#[test]` per module" (now two). L408 "four tables" (now six). L438 "`G-D NOT SATISFIED`" (0 hits; Verilator presence is checked at regression.sh:490). L15-20 SKIP classes no row has. L389-391 RESET row starts at cycle 0 (now obs 0 is outside the SV pair, `with_reset_row`). `<port>_hold`, `opening_hold_outputs`, `hold_form_audit.rs` (missing). §2 counts (143 then, 391 now). | fix in place (Q9) |
| ANCHORING_A_MODULE.md | STALE | — | 5. L81-88 says "no `initial` needed" (backwards since 2026-10-29/30; its own `ram_read_first.sv` gained one). L74-77 says the harness always drives `clk` (it does so only when `verilog_has_clock_port`; the transpiler emits no clock for a combinational module). The "reference leg" name collides. | fix in place (Q15) |
| LEVELIZED_SCHEDULING_SCOPE.md | HISTORICAL (partly stale) | partial (a D-14 box at L168) | 8: `PreEdgeBarrier`, `spawn_child`, `diff_convergent_comb_loop` (now `convergent_comb_loop_is_refused_at_spawn_by_both_schedulers`), `iterate_scc`, three phase-5 test names, the plain-`Out` Moore loop, and "`tick_clock` polls all tasks" (only the ticked domain is resumed, D-8) | dated header + inline notes (Q7) |
| ARRAY_PORT_ABI.md | CURRENT | — | 0 (re-transpiled `mux` and `bsg_mux_one_hot`, got the documented packed 2-D ports) | none |
| RECEIVED_MEMORY_ABI.md | CURRENT | — | 0 (D-25 refusals match `memory_rules.rs`) | none |
| DERIVATION_TABLE.md | HISTORICAL | no | 15. "The mechanical columns regenerate" no longer holds: `derivation-audit`'s guard column is hard-wired empty, and its plain-`Out` columns cannot fire since R8. | dated "pre-D-8" header (Q4) |
| PAIRED_IMPLEMENTATION_SCOPE.md | HISTORICAL | no | 11: `edge_value`, `forward_opening_drives`, `opening_hold_outputs`, `unprotected_trailing_out_write`, `trailing_out_reads_input` are all gone | dated header (Q4) |
| TIMING_MODEL_UNIFICATION.md | HISTORICAL | no | 9 | dated header in place (Q5) |
| TIMING_COVERAGE_MATRIX.md | STALE | — | 6. Pattern 3 calls `det_110101`/`det_010` comb-out (both are `RegOut`). The back-edge clause (deleted, cfg.rs:365). `synchronizer_behaves_as_two_flops_not_one` (gone). "`pattern_detector_2` has no `test = true`" (Cargo.toml sets it now). The 2026-10-23 addendum's m2 anchor: `trailing_update`, `branch_trailing`, `opening_hold_outputs`. | fix in place (Q8) |

### design_docs/review/ (except MEASUREMENTS.md)

| Doc | Class | Header today | Stale claims | Proposed action |
|---|---|---|---|---|
| review/README.md | STALE | — | 5. "Three … inventories" over a 4-row table. Calls MEASUREMENTS a "not updated" Phase A snapshot (it is the live §A..§AF log). DECISION_AUDIT.md and Q1_TRADEOFFS.md are not in the table. Says the ledger is "built from them" (`conformance_ledger.rs` reads SPEC §III.3 only). | fix in place; add DOCS_AUDIT.md to its table |
| review/INVENTORY_RULES.md | HISTORICAL | yes (titled "2026-11-03 baseline", L7 pointer) | 0 errors (expected drift) | extend the L7 pointer (Q13) |
| review/INVENTORY_CODE.md | HISTORICAL | baseline title, no pointer | 0 errors; heavy drift (read barrier, `spawn_child`, `ReadMode`, plain-`Out` forms are deleted) | dated pointer header (Q13) |
| review/INVENTORY_DOCS.md | HISTORICAL | baseline title, no pointer | 0 errors | dated pointer header (Q13); this audit is its successor |
| review/DECISION_AUDIT.md | HISTORICAL | — | 2. L103 "The two pending accidents are exactly as before" contradicts L97-98 (resolved by D-17/D-18). L97's "any-domain first settle" is gone (executor.rs:863-871). | one-sentence fix |
| review/Q1_TRADEOFFS.md | HISTORICAL | — | 1: L3 says "to be recorded … by the next packet" (it was recorded as D-8, SPEC_DECISIONS.md:1166) | fix the status line |

### design_docs/prior_art/ (20 files)

| Doc | Class | Stale claims | Proposed action |
|---|---|---|---|
| prior_art/README.md | CURRENT (indexes all 19 notes) | 0 | add one dated pre-D-8 sentence (Q12) |
| esterel, filament, anvil, arch, basejump, bluespec, chisel, clash, cummings, golden_age, myhdl, pymtl3, rhdl, spade, systemc, verismith (16 notes) | HISTORICAL. Their surveys of other HDLs read as current; their Copper statements describe the pre-D-8 language. | 1–6 each, about 55 lines in all (plain `Out` on clocked bodies, `<port>_hold`, the barrier, opening/closing anchors, the pre-tick family). basejump.md:130 cites an uncheckable claim "per memory"; esterel.md:511 and spade.md:210 cite a private memory note. spade.md:198, cummings.md:244, verismith.md:227 and arch.md:280 call SYNCHRONOUS_SEMANTICS / CYCLE_DATAFLOW normative, which is stale once Q1 is decided. | covered by the README note (Q12) |
| prior_art/prost.md | HISTORICAL (the most overtaken) | 5. §8.3 "Prost has no plain `Out` … where Copper is richer" (L210-216) is misleading now that D-8 adopted Prost's model. | one-line header on §8.3 (Q12) |
| prior_art/kami.md, koika.md | CURRENT (2026-11-05, written against SPEC) | 0 | none |

### design_docs/OUTDATED/

| Doc | Class | Finding | Proposed action |
|---|---|---|---|
| OUTDATED/README.md | STALE as a label | Does not label the directory historical (only CLAUDE.md:386 does). Lists 18 of 31 entries; the 13 missing are ATOMIC_INSTANT_EXECUTOR, AUDIT_2026-08-26, CONTROL_EXTRACTION, DIAGRAMS, EXECUTION_MODEL, EXECUTION_MODEL_RECONCILIATION, EXECUTOR_CONVENTION_EXPERIMENT, HDL_SEMANTICS_COMPARISON, LOOP_SEMANTICS, REGISTERED_OUTPUTS, SYNCHRONOUS_SEMANTICS, TRANSPILATION_TODO and `diagrams/`. Links a `PROGRESS.md` that does not exist. | Q10 |

### READMEs, crate docs, tools headers, other

| Doc | Class | Stale claims (key `file:line`) | Proposed action |
|---|---|---|---|
| README.md (root, 628 lines) | STALE (heavily) | ~20 (detail in §4, group F). L10: `In<Logic, ()>` (D-22). L64-121: `det_110101` with a plain `Out` and its SV, claimed "verbatim" at L123, but the file has `RegOut`. L43, L387: "34 of 34" (measured 40/40). L185-193, L369: "five wiring guards" (ten). L268-274: plain `Out` on sequential modules. L303: "Five crates". L309: "per-read freshness guards". L338: `iterate_scc`. L374-381: `allow_pretick_alignment` kept on purpose. L428-501: the retired rule family listed as live refusals, the array two-writes refusal (lowered since step 5), and `continue`. L502: `while`. L508: `/` called a gap (it lowers, chir_lower.rs:6881). L518-563: pinned divergences / `opening_hold_outputs`. L575-591: a docs table without SPEC.md. SPEC is mentioned 0 times. | fix in place: new examples from today's CLI, a short current Limitations pointing at SPEC II.1/II.3, a docs table built around SPEC.md |
| copper-ref/README.md | STALE (partly) | L55-56 "`obs_0` is not recorded" (it has been compared since 2026-11-06, tests/common/relation.rs:9). L70 `/` Unsupported (interp.rs:1890 evaluates `Div`). L85 the "two-job residue" aside. L91-101 the constant-evaluator paragraph predates D-21 B2 and the 2026-11-08 widened grammar. L103-110 "today's corpus … the two-job executor reads `x_N`". | fix in place (re-measure L91-101 against copper-ref/src first) |
| copper-ref/src/lib.rs `//!` | STALE | L30-31 "`combinational` / `structural` … all *deferred*" (combinational is evaluated, module.rs:127; structural is `Design`). L33 `/` refused (implemented). | fix in place (Q14) |
| copper-analysis/src/lib.rs `//!` | STALE (framing) | L3-27 framed as the G6 feasibility answer, "THIS module (item 2)". The facts are accurate, but it omits `front_end`, `consts`, `data`, `memory_rules`, `check_memory_staging`. | add a current overview on top (Q14) |
| copper-core, copper-macros, copper-codegen, copper-sim `src/lib.rs` | CURRENT (no `//!` crate docs at all) | — | none (adding docs is new work, not an audit fix) |
| src/lib.rs (root `copper`) `//!` | CURRENT | D-30 `compile_fail,E0080` doctest and clock doctests checked | none |
| tools/stats/README.md | CURRENT | L56 cites `paper/00_claims_audit.md`, which does not exist (paper tooling) | optional one-line fix |
| tools/ script headers (regression.sh, regression-remote.sh, transpile_coverage.sh, coverage_audit.py, refusal_reachability.py, clean_mermaid.js, stats/*.py, collect.sh, paper_numbers.sh) | CURRENT | regression.sh L36-66 describes G-A..G-K and G-H's retirement correctly. transpile_coverage.sh L5 "28/34" is cited as TODO's historical number. | none |
| copper-core/src/memory.md | ORPHAN | 0 referrers, not `include_str!`d; an early brainstorm | delete (Q11) |
| CLAUDE.md (548 lines) | STALE (phase 2 only; batch J is editing it) | L208 "The five crates" (six; copper-ref is undescribed). L221 `E_comb`/`E_tick` (the kinds are `EdgeKind::Comb`/`Tick`, cfg.rs:25-48). L226 "pinned ⊆ it corpus-wide by `register_reconciliation.rs`" (it scans `tests/fixtures/` only, register_reconciliation.rs:13). L321 and L373-386 make SYNCHRONOUS_SEMANTICS normative and omit SPEC.md, SPEC_DECISIONS, SPEC_OVERVIEW, review/, prior_art/, FORMAL_SEMANTICS* (batch J). L381-383 lists PRETICK/CYCLE_DATAFLOW/DERIVATION/PAIRED/TIMING_* as live roles. L525 names `copper-analysis/tests/pretick_alignment_corpus.rs` (deleted). History to trim, pointing at MEASUREMENTS: L40-49 (the G-F count history), L305-313 (the five-forms history), L409-522 (the ~110-line front-end-parity narrative §U..§AF). Not re-measured here: "388, 3 skipped" for G-F needs a run (SKIP has 3 rows, so 388 swept is consistent). | phase 2, after J merges |

### Counts by classification (54 rows above)

| Class | Count | Rows |
|---|---|---|
| CURRENT | 9 | SPEC_DECISIONS, ARRAY_PORT_ABI, RECEIVED_MEMORY_ABI, prior_art/README, kami, koika, tools/stats/README, src/lib.rs, tools headers (one row); the four crates without `//!` docs are not counted |
| STALE | 13 | SPEC.md (status line), SPEC_OVERVIEW, SYNCHRONOUS_SEMANTICS, CORPUS_DIFFERENTIAL_SWEEP, ANCHORING_A_MODULE, TIMING_COVERAGE_MATRIX, review/README, OUTDATED/README, README.md, copper-ref/README, copper-ref lib.rs, copper-analysis lib.rs, CLAUDE.md |
| HISTORICAL | 30 | SYNCHRONOUS_SEMANTICS_IMPL_PLAN, FORMAL_SEMANTICS_SURVEY, PRETICK_ALIGNMENT_GUARDRAIL, CYCLE_DATAFLOW_SEMANTICS, LEVELIZED_SCHEDULING_SCOPE, DERIVATION_TABLE, PAIRED_IMPLEMENTATION_SCOPE, TIMING_MODEL_UNIFICATION, 5 review/ files, 17 prior_art notes |
| SUPERSEDED | 1 | FORMAL_SEMANTICS |
| ORPHAN | 1 | copper-core/src/memory.md |

Of the 30 HISTORICAL docs, only SYNCHRONOUS_SEMANTICS_IMPL_PLAN and INVENTORY_RULES already
say so with a pointer to what replaced them.

---

## 3. Stale code comments found in passing (out of scope; Q17)

These are `.rs` comments, not docs, so a docs-only change does not fix them:

- copper-analysis/src/cfg.rs:238, :487, :908 — rustdoc links to `Self::multi_write_collapse` /
  `Self::multi_phase_out_write`, which no longer exist (dangling); :927 comment.
- copper-analysis/tests/front_end_parity.rs:21 — "`allow_pretick_alignment` (the opt-out both
  front-ends honour)", which is retired.
- tests/two_domain_hierarchy_cdc.rs:31-34 — D1 "guarded at compile time" by
  `unprotected_pretick_out_write`, which is deleted.
- copper-codegen/src/vlir_lower.rs:3349 — "`unprotected_pretick_out_write` refuses the module in
  both front-ends".
- copper-codegen/src/bin/derivation-audit.rs:176 — cites `pretick_out_write_before_update`. The bin's
  guard column is hard-wired empty and its plain-`Out` columns cannot fire. It and
  `Cfg::derivation_facts` may be worth retiring.
- copper-codegen/tests/phase_sensitive_checks.rs:7-8 — "58/8" count; `EXPECTED_DISAGREEMENTS` has 47 entries.
- examples/cpu/rv32i_cpu_pipelined.rs:227 — `unprotected_trailing_out_write` "now rejects".
- tests/cdc_synchronizer_anchor.rs:27, :81 — name the deleted `synchronizer_behaves_as_two_flops_not_one`.
- copper-analysis/tests/register_initializer_corpus.rs:2, :40 — cite the deleted `pretick_alignment_corpus.rs`.
- build.rs:65-77 — the comment says the rows below are known-wrong lowerings; the only rows there
  now are `two_domain_top` and `mod8`, neither of which is.

---

## 4. Appendix — per-doc findings with evidence

The sections below are the audit agents' and my own full findings, unedited apart from heading
levels. Groups: A semantics docs, B pre-tick and cycle-dataflow, C sweep/anchoring/scheduler/ABI,
D derivation and timing, E review/ and prior_art/ and OUTDATED/README, F READMEs, crate docs,
tools and CLAUDE.md.

Where a group's text and §1–§2 above differ, §1–§2 win. The one known difference:
TIMING_MODEL_UNIFICATION.md is cited from code, so its recommendation is a header in place, not a move.


### Group A (A_semantics.md)

#### Audit A — semantics docs (tip c81b59f, 2026-11-09)

Evidence convention: "0 hits" = `grep -rn --include='*.rs' --exclude-dir=target --exclude-dir=paper` over the
repo (build.rs, tools/ included where relevant). File existence checked with `[ -e ]`.

#### Cross-cutting finding (read first) — the normativity chain is circular and partly self-contradictory

- `design_docs/SPEC.md:3-4` — **SPEC.md's own status line still says** "Status: UNDER CONSTRUCTION
  (started 2026-11-03). Not yet normative. Until this document is adopted, `SYNCHRONOUS_SEMANTICS.md`
  remains the reference and wins where the two disagree." No adoption is recorded: `grep -n 'adopt'`
  in SPEC_DECISIONS.md finds only future-tense "retired when SPEC is adopted" (SPEC_DECISIONS.md:723,
  :921, :1118). Yet SPEC III.3.6 (SPEC.md:4419-4420) reports the ledger complete (234 lines, 234
  CLOSED, 0 OPEN) and every Phase C step has been implemented against SPEC.
- `design_docs/SYNCHRONOUS_SEMANTICS.md:3-5` calls `CYCLE_DATAFLOW_SEMANTICS.md` normative; its L29
  heads "The denotation — cycle dataflow (normative)".
- `CLAUDE.md:374-375` calls SYNCHRONOUS_SEMANTICS.md "normative semantics ... start here; where another
  doc disagrees with it, it wins"; `CLAUDE.md:321` "This is the semantic core; read
  design_docs/SYNCHRONOUS_SEMANTICS.md". `README.md:343`, `README.md:579` ("the timing model — start
  here") point the same way. `SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md:5` and `FORMAL_SEMANTICS.md:6` both
  defer to SYNCHRONOUS_SEMANTICS.md.
- Net effect: by the docs' literal text the two-job, barrier, plain-`Out` model of
  SYNCHRONOUS_SEMANTICS.md "wins" over SPEC.md, while the tree implements SPEC (D-8 one resumption,
  R8, etc. — evidence below).
- **JUDGEMENT (user's call):** (a) flip SPEC.md:3-4 to "normative since <date>", demote
  SYNCHRONOUS_SEMANTICS.md to historical, and fix CLAUDE.md:321/:374 and README.md:343/:579 to point
  at SPEC.md + SPEC_OVERVIEW.md; or (b) keep SPEC "not yet adopted" and instead rewrite
  SYNCHRONOUS_SEMANTICS.md to current. **Recommend (a).** The tree, the ledger and every recent commit
  already treat SPEC as the authority, and SYNCHRONOUS_SEMANTICS.md is mostly about deleted machinery.
  Adopting SPEC is a normative act, so it needs the user's explicit say-so and a dated D-entry or note.

---

##### design_docs/SYNCHRONOUS_SEMANTICS.md (932 lines)
- Classification: **STALE**. It claims to be current and normative, but most of its sections describe
  the two-job executor, the barrier, plain `Out` on clocked modules and the pre-tick rule family, all
  deleted by D-8 / Phase C steps 3–6. In substance it is **HISTORICAL** and largely **SUPERSEDED** by
  SPEC.md Part I (I.0–I.11) and II.3.2.
- Claims normative?: Yes, twice. L3-5: "The reference for Copper's execution/timing model — rebased
  2026-08-27 onto the cycle-dataflow denotation (`CYCLE_DATAFLOW_SEMANTICS.md`), which is *normative*".
  L29: "## The denotation — cycle dataflow (normative)". CLAUDE.md:374 and SPEC.md:4 also name it the
  winner.
- Header present?: No file-level historical/superseded header. There are only two local notes: L213-216
  ("Superseded by D-8 and Phase C step 4 ... `trailing_reg_outs` and the fold are deleted") and
  L619-621 (Memory: "Superseded in meaning by `SPEC.md` §I.10 (D-16, 2026-11-05)"). Plus inline
  "*(deleted in Phase C step 4 ...)*" at L241-242.
- Inbound refs: CLAUDE.md:321, :374; README.md:343, :579; SPEC.md:4, :2575, :2845; SPEC_DECISIONS.md:975,
  :2586, :2786, :3146; SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md (many); FORMAL_SEMANTICS.md:6, :235;
  FORMAL_SEMANTICS_SURVEY.md:19; CYCLE_DATAFLOW_SEMANTICS.md:34, :68, :76, :170; DERIVATION_TABLE.md:324;
  TIMING_MODEL_UNIFICATION.md:73, :168, :195; PAIRED_IMPLEMENTATION_SCOPE.md:232;
  PRETICK_ALIGNMENT_GUARDRAIL.md:172, :1318, :1553; CORPUS_DIFFERENTIAL_SWEEP.md:484, :497;
  review/INVENTORY_DOCS.md:9; prior_art/spade.md:198; OUTDATED/ATOMIC_INSTANT_EXECUTOR.md:14;
  code comments in copper-sim/src/executor.rs:316, copper-codegen/src/control_extract.rs:2213,
  copper-codegen/tests/unsupported_constructs.rs:118, :138, :583, copper-codegen/tests/phase_sensitive_checks.rs:91,
  tests/poll_order_fuzz.rs:3, tests/read_timing_equivalence.rs:60, tests/fixtures/trailing_statements_dut.rs:4,
  examples/uart/rx.rs:87; TODO:1593, :1765.
  Note: unsupported_constructs.rs:138 prints the doc's name in a test failure message, so the file
  must keep existing under that path or the message must change.
- Stale claims, by topic. Each is contradicted by SPEC and/or the tree.

  **Execution model / executor (contradicts SPEC I.0.2, D-8; executor.rs:842-882)**
  - L20-21: "it supplies register and read-timing facts the macro bakes into the generated code". No
    read-timing facts exist any more: `classify_reads` survives only in "(retired with the barrier,
    Phase C steps 3–4)" comments (tests/read_timing_equivalence.rs:3 and 5 others); `ReadTiming` 0 hits.
  - L24-26: "A CHIR interpreter may exist only as an optional validation-only backend ... (not built as of
    2026-09-01)". A reference interpreter now exists as a sixth workspace member, `copper-ref`
    (Cargo.toml:3; copper-ref/README.md:1-8). It runs on `syn::ItemFn`, not CHIR, and is the G-F sweep
    leg. Superseded in substance.
  - L47-53: "The segment divides into regions at the read-site barrier: an opening prefix ... a closing
    suffix executes at the pre-edge settle". There is no barrier and no second instant
    (copper-sim/src/lib.rs:57-60: "`PreEdgeBarrier` / `pre_edge_barrier` ... and the per-domain
    `POLL_PHASE` it read were deleted with the two-job executor (Phase C step 3, D-8)").
  - L55-67: commits "anchor to the segment's closing edge; a trailing region's commits anchor to the edge
    that opens its cycle"; "an opening-prefix plain-`Out` drive's continuous assign is the forwarded
    expression ... a closing-suffix drive's is the unforwarded commit-frontier form". Both contradict
    SPEC I.0.2/I.3: the trailing region is the head of the next resumption, and there is no clocked
    plain `Out`.
  - L79-82: "Pre-edge settle — phase `PreEdge` ... Pre-edge (leading) input reads sample here". No
    `PreEdge`/`PostEdge` phase enum exists in copper-sim or copper-core (grep 0 hits for
    `PreEdge\b`/`PostEdge\b`/`enum PollPhase`).
  - L85-91: "A `clk.tick()` future resolves in the post-edge settle, so a reaction's post-tick code runs in
    the same `tick_clock`, after the advance". Contradicted by copper-sim/src/executor.rs:842-848: "resume
    each of `Domain`'s coroutines once to its next `tick`, the edge, settle the network for the
    observation ... the post-edge settle polls no coroutine at all". Coroutines resume in
    `pre_edge_settle` (:863-872).
  - L98-100: "`set_poll_phase::<Domain>` / `PollPhase` in `copper-sim/src/lib.rs`". 0 hits; deleted per
    copper-sim/src/lib.rs:57-60. (`set_tick_resolving`/`TICK_RESOLVING` do still exist,
    copper-core/src/types.rs:989.)
  - L182-186: "the simulator runs `C` in the post-edge settle of the tick that opens the cycle and `A` in
    the pre-edge settle of the tick that closes it". Two-job timing, contradicted by executor.rs:842-848.
    The same paragraph cites `multi_write_collapse`, which is retired (copper-macros/src/lib.rs:404-405;
    copper-codegen/src/lib.rs:583).
  - L310-324: "Tick position is not a dialect" describes a plain `Out` write as a continuous drive, the
    read as "deferred to the closing settle when a tick follows it, immediate otherwise", and
    `out_from_reg_after_commit` as `assign q = acc`. All of this is two-job.
  - L541-558: "Passthrough reads are `Immediate` (the D2 fix ...)". `passthrough_read` 0 hits; the pinning
    test `d2_is_fixed_and_d1_still_demonstrates_the_hazard` 0 hits.
  - L560-583: "Input read timing — static edge-phase classification": `classify_reads` / `ReadTiming`
    (retired; ReadTiming 0 hits); "The macro's `ReadRewriter` emits `pre_edge_barrier::<D>().await`"
    (deleted; copper-analysis/src/front_end.rs:652-653 records the former per-read barrier injection);
    test pins `pretick_reads_are_deferred`, `trailing_read_is_immediate` 0 hits.
  - L592-597: the rationale for "a repeating wait must test before its tick" (Immediate read in the
    post-edge window) is two-job. SPEC.md:1171-1180 keeps the rule (`check_nested_loop_shape`) but
    re-derives it: "Under one resumption per cycle (D-8) both spellings read `x_{N+1}` ... The rule's
    principle is therefore design — one spelling per meaning". The rule is current; the reason given
    here is stale.
  - L712-716: "the one other await that appears in a hardware body, `pre_edge_barrier::<D>().await`, is
    injected by the macro itself". Deleted (copper-sim/src/lib.rs:57-60; copper-analysis/src/shape.rs:205
    only keeps the historical wording "before the macro injects its own ...").

  **Plain `Out` on a clocked module (contradicts SPEC II.1 R8; `copper_analysis::check_no_plain_out`,
  copper-analysis/src/lib.rs:78-94, called at copper-macros/src/lib.rs:366)**
  - L141-148: power-on of "the edge-sampled register of a conditionally driven plain `Out` (the
    enabled-register idiom), or the `<port>_hold` register of the opening-drive form". These forms are
    gone (Phase C step 4).
  - L230-277: control extraction's plain-`Out` emission, `vlir_lower::opening_hold_outputs` (now only a
    historical comment at tests/fixtures/control_extraction_dut.rs:109), "Drive-level since 2026-10-29",
    pinned by `copper-codegen/tests/hold_form_audit.rs` (**file missing**).
  - L279-293: "Startup — refused where it is observable": `Cfg::out_first_driven_late`. Only comments
    remain (copper-analysis/src/cfg.rs:2028, :2919; tests/fixtures/trailing_statements_dut.rs:27-30).
    Pins `copper-macros/tests/ui/fail/out_first_driven_late.rs` (**missing**) and `EXPECTED_LATE_CONSTANT`
    (0 hits).
  - L295-302: "`Out<T,D>` — a combinational (Mealy) output driven within the cycle ... a conditional
    write is an enabled register — `vlir_lower::conditional_output_ports` routes it into `always_ff`".
    R8 refuses a plain `Out` on a clocked module. `conditional_output_ports` survives only in a comment
    (tests/plain_out_multiphase_equivalence.rs:31). Per SPEC_OVERVIEW §4.3, `bsg_dff_en` is now anchored
    in its `RegOut` spelling.
  - L326-345: "Multi-write-around-a-tick guardrail ... `Cfg::multi_write_collapse` ... enforced as a
    compile error". Retired (copper-macros/src/lib.rs:404-405 "The pre-tick alignment family (rules
    9–15), `multi_write_collapse` (rule 8) ... were checked"; copper-analysis/tests/front_end_parity.rs:163
    "are retired (SPEC II.3.2)"). Pin `copper-macros/tests/ui/fail/multi_write_collapse.rs` is
    **missing**, and `copper-analysis/tests/analysis_semantics.rs` has 0 `fn multi_write_*` tests.
    (`ui/pass/multi_write_regout_ok.rs` still exists.)
  - L347-508: the whole "pre-tick alignment family" with its five or six rules
    (`unprotected_pretick_out_write`, `unprotected_trailing_out_write`, `multi_phase_out_write`,
    `linear_out_write_anchor_mismatch`, `pretick_out_write_before_update`). Every name now appears only
    in comments (e.g. copper-analysis/src/cfg.rs:2885 section header, copper-codegen/src/bin/derivation-audit.rs:176).
    `copper-analysis/tests/pretick_alignment_corpus.rs` is **missing**, so `EXPECTED_FLAGGED` (only an
    unrelated hit, register_initializer_corpus.rs:72), `EXPECTED_MULTI_PHASE` (0) and
    `EXPECTED_LINEAR_ANCHOR` (0) are gone. Pins `ui/fail/pretick_alignment.rs` and
    `ui/fail/write_before_update.rs` are **missing**. `opts_out_of_pretick_alignment` 0 hits.
    `allow_pretick_alignment` is now **itself refused**: copper-analysis/src/front_end.rs:53-54
    "`allow_pretick_alignment` was retired with the pre-tick alignment rules". L359-362 says the opposite
    ("Demonstration fixtures opt out with `#[hardware(sequential, allow_pretick_alignment)]`").
  - L218-228: `Cfg::trailing_regout_reads_input` and `Cfg::trailing_out_reads_input` are presented as
    live refusals. `trailing_out_reads_input` 0 hits; `trailing_regout_reads_input` appears only in a
    comment (tests/fixtures/out_phase_dut.rs:19).
  - L510-524: "Enforcement is symmetric" lists `multi_write_collapse`, `unprotected_pretick_out_write`,
    `pretick_out_write_before_update`, `trailing_regout_reads_input`, `trailing_out_reads_input`,
    `multi_phase_out_write`, `out_first_driven_late`, `memory_result_drives_plain_out` as run by both
    front-ends, and says "The six pre-tick alignment rules honour `allow_pretick_alignment`". All are
    retired. The current list is `check_front_end_rules` + `check_no_plain_out` + `check_nested_loop_shape`
    + `check_awaits` + `check_no_preloop_tick` + `check_register_initializers_with` +
    `check_held_preloop_values` (copper-macros/src/lib.rs:324-393).
  - L526-539: "The family's root cause ... each surviving rule is a derived unrealizability". No rule of
    the family survives.

  **Register inference (contradicts SPEC I.7.1 / I.0.5; Phase C step 6)**
  - L197-203: "A value flowing from `C` into `A` is still a register, by the liveness rule's back-edge
    clause (`Cfg::registers`, added 2026-08-21) — verified on `sync_2ff` (`src/sync.rs`), where
    `ff2 = ff1` in the trailing segment must be a flop". Contradicted by copper-analysis/src/cfg.rs:364-370
    ("No back-edge clause. From 2026-08-21 to Phase C step 6 (2026-11-07) ... Under D-8's one resumption
    per cycle such a local is ... a wire") and by src/sync.rs:83-94: `sync_2ff` is now spelled
    `let ff2 = ff1; ff1 = d.read(); q.write(ff2); tick`, with ff2 a wire and `q` the second flop.
  - L763-766: "A value defined after the last tick and read before the first is live across the loop
    back edge and is a register too — the back-edge clause". Deleted (same evidence).
    `Cfg::registers` now = `defined_in_loop ∩ live-out of tick nodes` (cfg.rs:384-396).
  - L114-139: power-on predicate "`copper_analysis::is_constant_expr`, the same predicate the
    late-constant rule applies". Superseded by D-21. copper-analysis/src/init.rs:18: "Until D-21 the rule
    was the syntactic [`is_constant_expr`]". The macro now wraps the initializer in `const { … }` and
    rustc judges it (copper-macros/src/lib.rs:74-79, :111). The "late-constant rule" is retired.

  **Memory (has a local superseded note, L619, but names deleted things as current)**
  - L625: "`ReadMode::{Sync, Async}` read policy". `ReadMode` 0 hits; the `Memory` signature is
    `<T, R, W, D, READ_LAT = 1, WRITE_LAT = 1>` (copper-core/src/memory.rs:22-29), and only `WriteMode`
    exists (memory.rs:7).
  - L634-643, L663-673: "Four rules ... `Cfg::memory_result_drives_plain_out` ... enforced in both
    front-ends". Retired (rule 7): copper-codegen/src/lib.rs:605, copper-macros/src/lib.rs:405,
    tests/multiphase_memory_equivalence.rs:41. `ui/fail/memory_result_drives_plain_out.rs` is **missing**.
    The pins `a_plain_out_from_a_read_result_across_phases_is_flagged`, `a_single_phase_module_is_not_flagged`,
    `a_plain_out_driven_from_a_read_result_in_an_extracted_module_is_rejected`,
    `two_stagings_of_one_bus_in_one_cycle_are_rejected` and `two_accesses_to_one_memory_port_are_rejected`
    all have 0 hits. (The other `check_memory_staging` pins it names still exist.)

  **Combinational loops (contradicts SPEC I.11.4 / D-14)**
  - L851-859: "Detection is not rejection. A convergent combinational loop ... is legal hardware, and the
    levelized settle simulates it by iterating that component to a fixpoint (`iterate_scc`)". Contradicted
    by copper-sim/src/executor.rs:536 (`refuse_comb_cycle`, called at :471/:516) and :1297-1300 ("A
    convergent combinational loop ... is refused too: D-14 makes every cycle OUTSIDE ... (it used to be
    detected by `comb_cycles`, simulated, and never rejected)"). `iterate_scc` 0 hits. Pin
    `diff_convergent_comb_loop` 0 hits; it was replaced by
    `convergent_comb_loop_is_refused_at_spawn_by_both_schedulers` (tests/levelized_differential.rs:364).
    Also L926-927 lists `diff_convergent_comb_loop` among the differential designs.

  **Trailing region / FOLD (the L213 note covers only part)**
  - L172-196: "`C`'s register-local updates commit at the last tick's edge ... they share a forwarding
    map". This is the trailing FOLD, deleted in Phase C step 4 (CLAUDE.md "the trailing region was
    FOLDED ... all of that is gone", MEASUREMENTS §H). The pins `m2_model_forwarded_lowering_matches_the_simulator_for_trailing_update`,
    `linear_trailing_probe` (L232), `trailing_segment_register_update_agrees` and `branch_trailing_probe`
    (L249) all have 0 hits.

  **Checking mechanisms (incomplete)**
  - L884-932: "Five mechanisms". This omits the reference-interpreter leg (G-F, copper-ref), the
    agreement-relation ledger (G-G), the projection test (G-I), the constant-evaluator test (G-J) and the
    conformance ledger (G-K), all in tools/regression.sh. L902's "143 cases — 133 running" is dated
    2026-09-01 and self-labelled as a snapshot, so it is fine as history.
- Accurate claims checked: about 35. Examples: `HardwareExecutor::tick_clock` exists (executor.rs:851);
  `Clock::advance` exists; `set_tick_resolving`/`TICK_RESOLVING` (types.rs:989); `SchedulerMode`
  default Levelized, `COPPER_SCHEDULER`, `poll_tasks_levelized` (executor.rs:632); `comb_cycles`
  (executor.rs:572); `Cfg::check_reachability`, `Cfg::build_combinational`, `check_definite_assignment`,
  `nested_loop_cfg` (cfg.rs:1303); `FallThrough::{AfterTick, ZeroTime}`, `goto_marker`,
  `splice_zero_time_gotos` (control_extract.rs:1830, :1861); `check_nested_loop_shape`, `check_awaits`,
  `check_register_initializers`, `emit::reg_inits` (emit.rs:424); `lower_seq_body`;
  `chir_lower::lower_structural_inst` (chir_lower.rs:3128); `find_promoted_wires`;
  `assert_source_registers_match_reference_sv` / `with_reference_registers`;
  register_reconciliation.rs scans `tests/fixtures/` only and asserts ≥ 8 (register_reconciliation.rs:370,
  :427). That makes L877-879 accurate, and **CLAUDE.md's "pinned ⊆ it corpus-wide by
  register_reconciliation.rs" is the overstatement** (note for the CLAUDE.md auditor). Also accurate:
  every poll-order and levelized test name except `diff_convergent_comb_loop`; the analysis_semantics
  register/reachability test names; the tests/ui pass/fail files `tick_first_wait`, `foreign_await`,
  `tickless_loop`, `partial_tick_path`, `cdc_crossing`; all example/golden SV paths; `sv-baseline` bin.
  The surviving accurate material is: the CFG model (L722-835, minus the back-edge clause),
  reachability, nested loops, poll-order and cross-domain interleave independence (L675-708), the
  register power-on paragraphs on `initial`/Yosys (L154-158), and the sweep/anchor description.
- Proposed action: **add a dated header**: "HISTORICAL as of 2026-11-09 — superseded by SPEC.md (Part I
  meaning, II.3.2 rule sort) and SPEC_OVERVIEW.md; describes the two-job executor retired by D-8 (Phase C
  step 3, 2026-11-06). Sections still accurate: CFG model, reachability, nested loops, poll-order /
  interleave independence." Remove "(normative)" at L29, change L3-5 accordingly, and fix CLAUDE.md:321
  and :374 and README.md:343 and :579. Keep the file at its path, because of the test message at
  copper-codegen/tests/unsupported_constructs.rs:138 and about 15 code comments.
  **JUDGEMENT:** (i) header-only, keeping it in design_docs/ — recommended, since many code comments
  cite it by path for dated decisions (2026-08-25 trailing statements); (ii) move to OUTDATED/, which
  breaks those path citations; (iii) rewrite to current, which duplicates SPEC Part I. Recommend (i),
  together with the cross-cutting SPEC adoption fix.

##### design_docs/SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md (664 lines)
- Classification: **HISTORICAL**. It is labelled as such and stays a useful record of July–Sep 2026.
  The header's pointer to the authority is now wrong, and its 2026-09-01 status table has rotted.
- Claims normative?: No for itself. L3-6: "This is the historical implementation record ... The normative
  semantics document is `SYNCHRONOUS_SEMANTICS.md`; where the two disagree, that document and the code
  are right". The pointer is stale (see the cross-cutting finding).
- Header present?: Yes, a historical header (L3-8), plus inline *SUPERSEDED* markers.
- Inbound refs: README.md:588 (already described there as "the historical implementation plan"),
  CLAUDE.md:381-382 ("historical plan ... do not take 'not started' there at face value"),
  copper-analysis/src/lib.rs (CLAUDE.md quotes "see SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md (item 2)"), and
  SYNCHRONOUS_SEMANTICS.md:13, :880. (Not exhaustively grepped beyond these; the header already covers it.)
- Stale claims (all inside the "Status as of 2026-09-01" table or text presented as current):
  - L5: "The normative semantics document is `SYNCHRONOUS_SEMANTICS.md`". It should be SPEC.md.
  - L65 (item 1): "`set_poll_phase`/`PollPhase` (`copper-sim/src/lib.rs`)". 0 hits; deleted per
    copper-sim/src/lib.rs:57-60 (POLL_PHASE deleted with the two-job executor, D-8).
  - L68 (item 3): "`classify_reads`; `ReadRewriter` in `copper-macros/src/lib.rs`". Both retired;
    `classify_reads` survives only in "(retired with the barrier, Phase C steps 3–4)" comments, and
    `ReadRewriter` only in copper-analysis/src/front_end.rs:652.
  - L72 (item 7): "CHIR interpreter | NOT BUILT". The reference interpreter `copper-ref` exists (a
    workspace member, Cargo.toml:3). It is not CHIR-based but fills item 7's role as the third leg
    (G-F).
  - L249: "`Cfg::registers` now has a back-edge clause". Deleted in Phase C step 6
    (copper-analysis/src/cfg.rs:364-370).
  - L252: the item-4 row says the D2/D1 fix is pinned by `sequential_forwarding_divergence.rs::d2_is_fixed_and_d1_still_demonstrates_the_hazard`
    and `independent_hardware_anchors_the_corrected_spelling`. Both have 0 hits.
  - L336-353, L577-580: the comb-loop runtime net is "`OSCILLATION_THRESHOLD` panic in
    `HardwareExecutor::iterate_scc`", and the landed version is "detect-not-reject; a convergent SCC is
    iterated". `iterate_scc` 0 hits; comb cycles are refused at spawn (executor.rs:536, :1297-1300, D-14).
  - L336-341: "a sequential `Out` legitimately holds when unwritten ... (it would reject `bsg_dff_en`)".
    Superseded by R8 (no plain `Out` on clocked modules; `check_no_plain_out`).
  - L536: "`PreEdgeBarrier` is phase-constant within a settle". Deleted (copper-sim/src/lib.rs:57).
  - L606-612 "What does NOT change": "the post-edge continuation convention ... is validated and stays".
    Reversed by D-8 (executor.rs:842-848: the post-edge settle polls no coroutine).
  - L484: `chir_lower::lower_hardware_call`. Deleted (D-23; chir_lower.rs:3255, :5414 comments). It sits
    under "Original task notes (historical)", so this is minor.
  - L457-468 item 4a: "Still deferred". Still true in substance (a structural parent is not spawned;
    `spawn_wired` hand-wires).
  - L67, L320-335: these mention `register_reconciliation.rs`. Its "scans `tests/fixtures/` and asserts at
    least 8" is accurate (register_reconciliation.rs:370, :427).
- Accurate claims checked: about 25. Examples: `tests/executor_guardrails.rs::module_whose_future_returns_panics`;
  `transpile_source_hierarchy`; `emit::structural_body`; `Cfg::fsm_report`; `check_definite_assignment`;
  `vlir_lower::check_no_latches`; `registers_from_fir` (copper-analysis/src/lib.rs:540);
  `reference_sv_registers`/`RegMatch`/`assert_registers_match_reference_sv`;
  `tick_inside_a_nested_loop_is_supported`; `probe_fsm_sim_matches_verilog` still exists
  (tests/probe_timing_investigation.rs:89); `accum_2_sim_matches_verilog`; `tests/rv32i_pipelined_verilator.rs`;
  `COPPER_SCHEDULER`, `BLESS_GOLDEN`; `synced_read.rs` gone (2 hits, both historical comments).
- Proposed action: **fix the header in place**. Change L5 to name SPEC.md as normative (and note that
  SYNCHRONOUS_SEMANTICS.md is itself historical), and add one dated line under the 2026-09-01 table:
  "Since then (D-8, Phase C 2026-11-06..09): item 1's PollPhase, item 3's classify_reads/ReadRewriter/
  barrier and item 2's back-edge clause are deleted; item 7's role is filled by copper-ref; comb cycles
  are refused at spawn (D-14)." Optionally move it to OUTDATED/. No JUDGEMENT needed beyond the
  cross-cutting one.

##### design_docs/SPEC_OVERVIEW.md (108 lines)
- Classification: **STALE (minor)**. It is the generating page and its substance is current
  (one-job/D-8, RegOut-only clocked outputs, three legs + anchor). A few counts and lists are frozen at
  2026-11-04/05.
- Claims normative?: Yes, L3: "Status (2026-11-04): the generating document. Every clause of `SPEC.md`
  must follow from this page". SPEC.md:9 agrees.
- Header present?: A status line dated 2026-11-04. No staleness note.
- Inbound refs: SPEC.md:9, :1777, :2091, :2154, :2289, :2926, :3209, :3232, :3831; SPEC_DECISIONS.md;
  review/DECISION_AUDIT.md; review/Q1_TRADEOFFS.md; Cargo.toml:31; build.rs:214; tools/regression.sh:44,
  :367; copper-ref/README.md:3, copper-ref/src/lib.rs, copper-ref/tests/interp_data.rs;
  copper-analysis/src/front_end.rs:10; tests/common/mod.rs:696, :817, :1121, :1542;
  tests/reference_leg_measurements.rs; tests/fixtures/q1_b_rewrites_dut.rs. Its §7 is load-bearing in
  code comments.
- Structure vs SPEC.md: the §-structure (1 module, 2 one-job choice, 3 contract, 4 choices, 5
  principles, 6 refusals, 7 checking, 8 done) is conceptual, not a TOC of SPEC, and it maps cleanly onto
  SPEC Parts I/II/III. **It does not state the ledger count (234 lines, all CLOSED) or the D-1..D-30
  range anywhere**, so it has no stale count there, and also no current one. SPEC_DECISIONS.md has
  D-1..D-30 (`^### D-` 31 headings including the template at :12; the last is D-30 at :3947).
- Stale claims:
  - L5-6: "`review/DECISION_AUDIT.md` sorts D-1…D-8 by that test". True of that file's date, but the
    decision log now runs to D-30. A reader could take D-8 as the latest. Minor; date it.
  - L85-92 (§6): "twelve rows today (`SPEC.md` §II.3, D-13, 2026-11-05) — six dated refusals ... and six
    shapes the transpiler admits and realizes wrongly ... each with a fixture owed to Phase C". The SPEC
    II.3.3 table (SPEC.md:3722-3738) now has 15 rows: (a)–(e), (f1)–(f6), (g), (h), (i), (j). SPEC.md:3741
    says "every row but (e)'s saturating_* residue is closed", and SPEC III.3.6 (SPEC.md:4419-4420)
    reports 234/234 CLOSED, 0 OPEN. **SPEC itself records this edit as owed to the overview**:
    SPEC.md:3209 ("`SPEC_OVERVIEW.md` §6's count of II.3 rows — the coordinator's edit when II.3 gains the
    rows above") and SPEC.md:3831-3832.
  - L88-89: "a constant initializer the evaluator cannot fold" is listed as a current dated refusal.
    CLOSED by D-21 (SPEC.md:3727 row (d) "CLOSED (D-21, 2026-11-07)"). Likewise "a hex literal" (row (g))
    and "the `Bits` methods" (row (e)) are closed per SPEC.md:3741.
  - L67 (§4.6): "Every port and register powers on at the zero of its type (D-6; wires in I.11;
    registers in I.7)". Inaccurate for registers. SPEC I.7.4(a) (SPEC.md:991-993): a register has "the
    value of its initializer `c` — a constant", and (b) `Bits::x()` means unstated/X. The zero-of-type rule
    holds for ports (`init(q)`, I.6.6) and `Memory::new` (SPEC.md:1777), and for synthesized state
    (SPEC.md:1015, :2985). JUDGEMENT-light: reword to "every port powers on at the zero of its type; a
    register at its constant initializer (R11)".
  - L92: "The eight alignment rules went with the second instant (D-8)". This is consistent with the
    tree (retired rules 8–15 per copper-macros/src/lib.rs:404-405, rule 7 separately), so it is
    accurate. Note only that SPEC/ledger count "retired rule 7–15" (nine).
  - L104-108 (§8 "What done looks like") reads as future work. "The interpreter as a sweep leg with its
    guard" is done (copper-ref, G-F in tools/regression.sh:44). "Every rule in §6's two lists with its
    principle or its date" is done via the ledger. Consider marking §8 as achieved on 2026-11-09 (§AF).
  - L83-84: the semantic-refusal list ("a non-tick await, a tickless loop path, a return, a plain `Out`
    on a sequential module") is illustrative. II.1 now has about 24 rule rows (`^| R[0-9]` 24 hits in
    SPEC.md), and II.1.3 has three named parity exceptions. Not wrong, since it says "Two kinds" and gives
    examples, but it could add "e.g." or point at II.1.
- Accurate claims checked: about 15. One job/D-8; every clocked output a RegOut (check_no_plain_out);
  the trailing region as the next head; the barrier, the five plain-Out forms and the alignment rules and
  their opt-out removed (copper-sim/src/lib.rs:57; front_end.rs:53); the three legs (sim, copper-ref,
  Verilator) and the anchors; `review/DECISION_AUDIT.md` and `review/Q1_TRADEOFFS.md` exist; the §5
  principles match SPEC II.1/II.3.1 (SPEC.md:3635-3637 cites §5).
- Proposed action: **fix in place** (small). Restate §6's II.3 count as "15 rows recorded 2026-11-05..08;
  all closed by Phase C (ledger 234/234 CLOSED, §AF, 2026-11-09)", which is the edit SPEC.md:3209/:3831
  asks for. Reword §4.6 for registers. Add "D-1…D-30" and the ledger state to the status line. Mark §8
  as reached. Because this is the generating page, its edits are the coordinator's/user's (SPEC.md:3831
  says "the coordinator's edit"). **JUDGEMENT:** the §4.6 reword is a substantive statement on the
  generating page, so confirm with the user.

##### design_docs/FORMAL_SEMANTICS.md (401 lines)
- Classification: **SUPERSEDED** (by SPEC.md; this was the 2026-11-02 draft that SPEC grew from) /
  HISTORICAL. Its central device, the two-instant phase flag `φ`, was rejected by D-8.
- Claims normative?: No. L3: "Status: DRAFT PROPOSAL (2026-11-02). Nothing here is decided." L5-6:
  "Where this draft and `SYNCHRONOUS_SEMANTICS.md` disagree, the latter wins until this is adopted."
  It never was. SPEC_DECISIONS.md:1118: "FORMAL_SEMANTICS §3.2's `φ` is superseded by I.0.2 when SPEC is
  adopted".
- Header present?: A draft header only. No "superseded" header.
- Inbound refs: FORMAL_SEMANTICS_SURVEY.md:4, :258; SPEC.md:110, :559, :760, :791, :3872 (citing its
  Lemma 2.1, P6/G6, §3.4 as precursors); SPEC_DECISIONS.md; prior_art/README.md.
- Stale claims (relative to the tree; all expected for a superseded draft):
  - L15-16, L105-136: "two instants" `t_N⁺`/`t_{N+1}⁻`, executor phases `PostEdge`/`PreEdge`, and "φ flips
    to C at the first deferred read ... exactly what the macro's injected `pre_edge_barrier` does". The
    barrier is deleted (copper-sim/src/lib.rs:57-60) and `PreEdge`/`PostEdge` have 0 hits. D-8: one
    instant.
  - L39, L157, L213-226, L268-272: a plain `Out` drive in a sequential kernel module, with its observation
    rule. Refused by R8 (`check_no_plain_out`).
  - L67: W5 says "initialized by a constant (`is_constant_expr`)". Superseded by D-21 (init.rs:18).
  - L70, L293-311: W8–W14 rules and the `allow_pretick_alignment` silencing. All retired; the opt-out is
    refused (front_end.rs:53-54).
  - L84-85: register "live across some E_tick edge, including the back edge". The back-edge clause was
    deleted (cfg.rs:364-370).
  - L97-101: read classification `classify_reads` and "barrier injection". Retired.
  - L249-251: "a convergent SCC iterated to its fixpoint". Refused at spawn (executor.rs:536, D-14).
  - L332-336 (ledger): `PollPhase` (0 hits), `pre_edge_barrier` (deleted), `seen_in_read` (0 hits),
    `leading_read_reaches` (0 hits), `pretick_reads_are_deferred` (0 hits), `hold_form_audit.rs`
    (**missing**).
  - L344, L371-376 (G2): "no executable reference semantics; a kernel interpreter would be the third
    leg". Built as copper-ref (Cargo.toml:3). G4 "non-convergent SCC panic unpinned" is moot (all
    cycles refused at spawn, executor.rs:1297-1300 test). P6 retired as vacuous (SPEC.md:3872).
  - L356-360 (G1): `Cfg::drive_anchor` 0 hits. Moot, since there are no anchors under D-8.
- Accurate claims checked: about 10. `check_awaits`, `Cfg::check_reachability`, `check_nested_loop_shape`,
  `check_register_initializers`, `Cfg::check_memory_staging`, `front_end_parity.rs`,
  `register_initializer_corpus.rs` and `regout_forwarding_equivalence.rs` exist; `RegOutShared`,
  `on_posedge` exist; `regout_trailing_single_tick` exists (15 hits); the kernel grammar and the
  Esterel-style judgment survive in SPEC I.2.
- Proposed action: **add a dated header**: "SUPERSEDED 2026-11-03..09 by SPEC.md (kernel → I.2,
  registers → I.0.5/I.7, propositions → III.1, ledger → III.3); its two-instant φ was rejected by D-8
  (2026-11-04). Kept because SPEC.md and SPEC_DECISIONS.md cite its Lemma 2.1 / P6 / §3.4 as
  precursors." Then **move to OUTDATED/**. JUDGEMENT: moving breaks the bare-name citations in SPEC.md
  (:110, :559, :760, :791, :3872), which name it without a path, so either a move or header-in-place
  works. Recommend header + move to OUTDATED/, since the citations are by name, not path.

##### design_docs/FORMAL_SEMANTICS_SURVEY.md (276 lines)
- Classification: **HISTORICAL**, with lasting value. The prior-art table (§1) and the per-document
  findings (§3) are a durable literature survey. The Copper-position sentences are dated 2026-11-02
  and describe the two-job model.
- Claims normative?: No. L3: "Status: survey complete, recommendation pending the user's decisions
  (2026-11-02)." The pending recommendation was decided (SPEC + D-1..D-30), and on the anchor it was
  decided against: D-8 rejected the two-instant refinement.
- Header present?: A dated status line only.
- Inbound refs: FORMAL_SEMANTICS.md:7; prior_art/README.md; SPEC_DECISIONS.md.
- Stale claims (the Copper-specific ones):
  - L3: "recommendation pending the user's decisions". Decided (SPEC.md; SPEC_DECISIONS.md D-1..D-30).
  - L19-24: "`SYNCHRONOUS_SEMANTICS.md` declares a denotation normative ... There is no grammar, no
    judgment, no inference rule, and no stated proposition". SPEC.md now has a kernel (I.2), propositions
    (III.1) and a ledger (III.3).
  - L98-100: "Esterel has one observation point per instant; Copper has two (pre-edge and post-edge
    settle)". Under D-8 Copper has one instant per cycle (SPEC_OVERVIEW §4.1).
  - L212-216: "`seen_in_read` ... `leading_read_reaches` ... computed twice". Both 0 hits.
  - L222-227, L238-240: "two instants per cycle ... This is Copper's content". Reversed by D-8. "a
    reference interpreter over the kernel ... would make ... a measured statement". Built (copper-ref).
  - L143-145, L163-164: Copper's "plain-`Out` hazards ... refuses by form", "`out_first_driven_late` and
    the D2 fix". Retired (R8; rule family deleted).
  - L249-250: Copper "iterates convergent comb SCCs instead of rejecting them". Now refuses them at
    spawn (executor.rs:536, :1297-1300; D-14).
  - L271-276: the recommendation "define a statement's anchor dynamically (the phase flag ...)" was not
    adopted (D-8).
- Accurate claims checked: the prior-art descriptions were not re-verified (they cite paper/references,
  which is out of scope). Copper names that are still valid: `check_reachability`,
  `check_nested_loop_shape`, `check_awaits`, `control_extract`, `verify_with_verilator`, the levelized
  /fixpoint pair, G-E anchoring.
- Proposed action: **add a dated header**: "HISTORICAL survey (2026-11-02). The prior-art findings
  stand. Its Copper-position and recommendation paragraphs (§0 para 2, §4, §5 item 3, §6) describe the
  pre-D-8 two-instant model, and the recommendation was resolved by SPEC.md / D-8 (one instant)." Keep
  it in design_docs/ next to prior_art/, or move it with FORMAL_SEMANTICS.md to OUTDATED/. JUDGEMENT:
  recommend keeping it in design_docs/ with the header, because the comparison table has
  paper-related-work value.

---

#### Short summary
| path | class | #stale | action | judgement? |
|---|---|---|---|---|
| design_docs/SYNCHRONOUS_SEMANTICS.md | STALE (in substance HISTORICAL/SUPERSEDED by SPEC Part I) | ~40 claims across ~25 passages | dated HISTORICAL header, drop "normative" (L3-5, L29), keep path; fix CLAUDE.md:321/:374, README.md:343/:579 | YES — SPEC.md:3-4 still says SPEC is "Not yet normative" and SYNCHRONOUS_SEMANTICS.md wins; adopting SPEC is the user's call |
| design_docs/SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md | HISTORICAL (header present) | 11 | fix header L5 (authority → SPEC) + one dated "since D-8" line under the 2026-09-01 table | no |
| design_docs/SPEC_OVERVIEW.md | STALE (minor) | 6 | fix in place: §6 II.3 count (owed per SPEC.md:3209/:3831), §4.6 register power-on wording, status line D-1..D-30 + ledger 234/234, §8 reached | YES (light) — the generating page's wording is the coordinator's/user's |
| design_docs/FORMAL_SEMANTICS.md | SUPERSEDED by SPEC.md (draft 2026-11-02; φ rejected by D-8) | ~14 | dated SUPERSEDED header + move to OUTDATED/ | minor (move vs header-in-place; recommend move) |
| design_docs/FORMAL_SEMANTICS_SURVEY.md | HISTORICAL (survey value persists) | 9 | dated header marking Copper-position paragraphs pre-D-8; keep | minor (keep vs OUTDATED/; recommend keep) |

### Group B (B_pretick_dataflow.md)

#### Audit B: PRETICK_ALIGNMENT_GUARDRAIL.md and CYCLE_DATAFLOW_SEMANTICS.md

Tree: worktree at c81b59f (2026-11-09). Read-only audit. All greps are over tracked
`*.rs`, `build.rs`, `tools/`, `Cargo.toml` (excluding `target/`, `paper/`), unless noted.

Evidence that the whole subject is retired (used below as "RETIRED-FAMILY"):
- `copper-macros/src/lib.rs:404-405`: "The pre-tick alignment family (rules 9–15), `multi_write_collapse`
  (rule 8) and `memory_result_drives_plain_out` (rule 7) were checked [here]" (past tense; deleted).
- `copper-codegen/src/lib.rs:583,605`: the same note on the transpiler side.
- `copper-analysis/tests/front_end_parity.rs:163`: "`multi_write_collapse` are retired (SPEC II.3.2)".
- `copper-analysis/src/front_end.rs:53-57`: `allow_pretick_alignment` "was retired with the pre-tick
  alignment rules"; ui case `copper-macros/tests/ui/fail/retired_pretick_opt_out.rs` refuses the flag.
- `copper-analysis/src/lib.rs:101` `check_no_plain_out` (R8): "output port `{port}` is a plain `Out` on
  a clocked module". The whole subject of both docs (a plain `Out` on a clocked body) can't be written any more.
- `tests/sequential_forwarding_divergence.rs:1-14`: "This file held the measured two-job divergence
  probes — 167 tests over 160 modules … the tests went with them". The file now has 149 lines and
  1 `#[test]` (`the_registered_output_form_agrees`).
- `copper-sim/src/lib.rs:57`: "`PreEdgeBarrier` / `pre_edge_barrier` (the parking point the macro injected…)" (deleted).
- `copper-analysis/src/cfg.rs:365,2932`: "No back-edge clause … From 2026-08-21 to Phase C step 6 (2026-11-07)".
- SPEC_DECISIONS.md D-8 (2026-11-04) "Reverses D-1 A, … D-7 A (the barrier)". The question it settles is
  the two-job body (outputs at `t_N⁺`, inputs at `t_{N+1}⁻`) vs one job. That two-anchor body is the
  model CYCLE_DATAFLOW_SEMANTICS.md §2–§3 defines.

Rule functions: 0 definitions anywhere in the tree. Every hit is a comment or a fixture/doc-comment
mention: `unprotected_pretick_out_write` (comments in `vlir_lower.rs:3349`,
`tests/two_domain_hierarchy_cdc.rs:32`, a fixture), `pretick_out_write_before_update` (comment in
`derivation-audit.rs:176`), `trailing_regout_reads_input` (fixture comment), `trailing_out_reads_input` (0),
`linear_out_write_anchor_mismatch` (a section-header comment in `cfg.rs:2885`), `multi_phase_out_write`
(doc-links in `cfg.rs:487,908,927`, which are now dangling), `out_first_driven_late` (comments in
`cfg.rs:2028,2058,2919`), `multi_write_collapse` (doc-link in `cfg.rs:238`, comments elsewhere),
`memory_result_drives_plain_out` (comments), `unprotected_trailing_out_write` (comments). Never defined:
`ALLOW_PRETICK` 0, `opts_out_of_pretick_alignment` 0, `has_nested_tick` 0, `leading_read_reaches` 0,
`written_on_all_paths` 0, `forward_opening_drives` 0, `seen_in_read` 0, `edge_value` 0,
`SHIRPortDrive` 0, `trailing_reg_outs` 0, `inject_synced_reads` 0, `segment_anchor`/`region_anchor` 0,
`COPPER_SEMANTICS` 0, `opening_hold_outputs` (only a fixture comment), `classify_reads` (only comments
reading "(retired with the barrier, Phase C steps 3–4)", e.g. `tests/read_timing_equivalence.rs:3`).

Missing files cited as live: `copper-analysis/tests/pretick_alignment_corpus.rs` (the corpus-pin file;
CLAUDE.md's attribute-parser note and `register_initializer_corpus.rs:2,40` still cite it).
Also every ui case named: `ui/fail/pretick_alignment.rs`, `pretick_mixed_anchors.rs`,
`pretick_path_dependent_drive.rs`, `pretick_forwarded_guard.rs`, `multi_phase_out_write.rs`,
`path_dependent_single_phase.rs`, `write_before_update.rs`, `single_tick_trailing_update.rs`,
`pretick_guard_update.rs`, `trailing_out_reads_input.rs`, `linear_anchor_mismatch.rs`,
`linear_regfree_passthrough.rs`, `linear_trailing_conditional.rs`, and `ui/pass/pretick_alignment_ok.rs`.
Plus `copper-codegen/tests/hold_form_audit.rs` and `tests/corpus_equivalence.rs`.
Bulk check of the backticked identifiers containing `_`: PAG 189 of 276 have 0 tree hits, CDS 256 of 332.
Most of the rest are only comment or fixture mentions.

---

##### design_docs/PRETICK_ALIGNMENT_GUARDRAIL.md (1573 lines)

- Classification: **HISTORICAL**, but still framed as current. It is the measured record of a rule family
  that D-8 / Phase C step 3 (2026-11-06) deleted in full. CLAUDE.md:396 already calls it "the historical
  record". The doc never says so itself.
- Claims normative?: Not normative. It does claim to be current: L14 "§5.4–§5.7 and the tables in this
  header describe the rules as they stand"; L108 "the exact-set pins below are what is enforced today".
- Header present?: **No retirement header.** The status block (L3-175) ends on 2026-11-02. The first line
  of status reads "verified against the tree 2026-09-01: the family is GUARDED". D-8, R8 and the deletion
  are not mentioned anywhere in the doc (grep `D-8\|R8\|retired by\|2026-11-0[4-9]`: 0 hits for a
  retirement note).
- Inbound refs: CLAUDE.md:378, 396; README.md:445, 580; README.md:580 still describes it as "its
  compile-time rules", present tense, which is stale too. Also TODO:860, 1652, 1721, 2214;
  `copper-analysis/src/cfg.rs:2783` (section comment); `examples/uart/rx.rs:127`;
  `tests/sequential_forwarding_divergence.rs:10`, which correctly points here as "the record";
  `tests/two_domain_hierarchy_cdc.rs:33`; `tests/two_domain_counter_behavioral.rs:7`;
  SPEC_DECISIONS.md:1181 ("PRETICK_ALIGNMENT_GUARDRAIL §5.8"); CYCLE_DATAFLOW_SEMANTICS.md (3);
  SYNCHRONOUS_SEMANTICS.md; PAIRED_IMPLEMENTATION_SCOPE.md; DERIVATION_TABLE.md;
  CORPUS_DIFFERENTIAL_SWEEP.md; FORMAL_SEMANTICS.md; TIMING_*; prior_art/*; review/INVENTORY_DOCS.md.
  Inbound refs are numerous, so the doc is not an orphan.
- Stale claims (present tense about deleted mechanisms):
  - L3-14: "Status (verified against the tree 2026-09-01): the family is GUARDED — four rule functions …
    four exact-set corpus pins" plus "describe the rules as they stand". RETIRED-FAMILY; the rule
    functions have 0 definitions.
  - L104-108: "All four live in `copper-analysis/src/cfg.rs` as methods on `Cfg` … what is enforced
    today". 0 definitions in cfg.rs. (Internal inconsistency too: "four", but the table has six live rows.)
  - L110-118 (rules table, every row): `unprotected_pretick_out_write`, `multi_phase_out_write`,
    `linear_out_write_anchor_mismatch`, `pretick_out_write_before_update`, `trailing_out_reads_input` are
    all presented as live rules. 0 definitions (see above).
  - L120-129: "Every rule is a spanned compile error in the `#[hardware(sequential)]` arm of
    `copper-macros/src/lib.rs` … silenced by `allow_pretick_alignment` (`ALLOW_PRETICK` in the macro) …
    also run by the transpiler in `transpile_target`, … `opts_out_of_pretick_alignment`". Evidence:
    `ALLOW_PRETICK` 0 hits, `opts_out_of_pretick_alignment` 0 hits, macro `lib.rs:404` says "were checked",
    and the opt-out is now refused (`front_end.rs:53-57`, `ui/fail/retired_pretick_opt_out.rs`).
  - L131-140 (pins table): "Each rule has three layers, all green". Every `cfg.rs` unit test named is gone
    (e.g. `hazard_w4_mixed_alignment_flagged`, `v8a_write_between_read_and_update_flagged`,
    `trailing_reads_are_immediate`: 0 hits). Every ui file named is missing (listed above).
    `pretick_alignment_corpus.rs` is missing, and `EXPECTED_MULTI_PHASE`, `EXPECTED_WRITE_BEFORE_UPDATE`,
    `EXPECTED_TRAILING_OUT_INPUT`, `EXPECTED_LINEAR_ANCHOR` have 0 hits. (The only `EXPECTED_FLAGGED`
    left is `register_initializer_corpus.rs:72`, an unrelated pin.)
  - L142-148: "The corpus pins scan every clocked … module … `cargo test -p copper-analysis --test
    pretick_alignment_corpus`". There is no such test target.
  - L150-153: "`tests/sequential_forwarding_divergence.rs` (55 `#[test]`s, none ignored …)". The file has
    1 `#[test]` (grep -c). Its own header says the probes were deleted.
  - L155-171: e9 "pins the agreement" via `e9_a_read_in_another_state_leaves_an_opening_write_opening_agrees`
    (0 hits). e8 "is REFUSED by `trailing_out_reads_input`" (0 hits).
  - L173-175: "Read first: `SYNCHRONOUS_SEMANTICS.md` §Output timing". The section exists
    (SYNCHRONOUS_SEMANTICS.md:295), but it is outside this audit. Not checked further.
  - L198-203: "A leading `In` read classifies `Deferred` … and injects `pre_edge_barrier()` …
    `inject_synced_reads` returns early". Present tense; barrier and injection deleted
    (`copper-sim/src/lib.rs:57`; `inject_synced_reads` 0 hits; `classify_reads` "retired with the barrier").
  - L222-228: "the fix is pinned today by `…::d2_is_fixed_and_d1_still_demonstrates_the_hazard` and the
    unit test `trailing_reads_are_immediate` in `cfg.rs`". Both have 0 hits.
  - L243-259 (§1 "Relationship to `multi_write_collapse`"): a live comparison of two rules, both deleted.
    It also says `analysis_semantics.rs`'s "`multi_write_*` tests cover the collapse rule's own shapes".
    `grep -c 'fn multi_write' copper-analysis/tests/analysis_semantics.rs` = 0.
  - L263-270 (§2): "`tests/two_domain_hierarchy_cdc.rs` … is green *because D1 and D2 cancel*". Present
    tense, but §8 (L1337-1340) says it was resolved 2026-08-21. Contradicts itself and is not annotated.
  - L296-303: the superseded note cites `d_narrowing_battery_verdicts`,
    `pre_tick_update_forwarding_agrees_end_to_end`, `hazard_v1_…/v5_…/v7_…_dissolved` as live
    measurements. 0 hits for each.
  - L340-365 (§3.3 table, "today" column): e.g. L355 `probe_fsm` "still flagged … in `EXPECTED_FLAGGED`".
    No rule exists, and `tests/fixtures/probe_timing_dut.rs:33,47` now declares `RegOut` outputs.
  - L401-404 (Q2) and L1343-1348 (§8): `tests/probe_timing_investigation.rs` "stays `#[ignore]`d" in the
    first passage, un-ignored per the second. `probe_fsm_sim_matches_verilog` exists at line 89, so the
    test survives with different meaning.
  - L475-487: "So the fix is `Cfg::multi_phase_out_write`, enforced in both front-ends … Gate:
    `pretick_alignment_corpus.rs::multi_phase_out_write_flags_exactly_the_demonstration_modules` … that is
    still its whole `EXPECTED_MULTI_PHASE` today". All gone.
  - L489-490: "D1's own rule is UNCHANGED and still head-segment-only". Deleted.
  - L612-616, L688-702 (§5.4): "Pinned as `…::d1_in_the_trailing_segment_is_an_unguarded_gap`"; unit pins
    `trailing_linear_class_exempt` etc. (0 hits). L668-673: `pulse_plain` "is flagged today only by
    `out_first_driven_late`" (`pulse_plain` 0 hits, rule deleted). L658-661: `Cfg::crosses_more_than_one_tick`
    "counts tick nodes and treats a folded tick-bearing loop as more than one". This one is **accurate**:
    it still exists (`cfg.rs:444`), now feeding only `derivation_facts` (`cfg.rs:1067`).
  - L706-710: "The head rule is unchanged and still head-segment-only". Deleted.
  - L764-790, L792-796 (§5.5): "The exemption now applies only where … (`Cfg::written_on_all_paths`)";
    "the constant-write entries of `EXPECTED_FLAGGED` today"; "All three modules carry
    `allow_pretick_alignment` … A real design in this shape now gets a compile error pointing at `RegOut`".
    It now gets R8's error. `written_on_all_paths` 0 hits.
  - L886-917 (§5.6): "Rule: `Cfg::pretick_out_write_before_update` … Enforced in the macro … Pins: unit
    tests `v8a_…`, … trybuild `ui/fail/write_before_update.rs` … `pretick_alignment_corpus.rs`". All gone.
    The only remaining mention is a comment, `derivation-audit.rs:176`.
  - L938-956 (§5.7): `d_narrowing_battery_verdicts`, `hazard_*_dissolved`,
    `ui/pass/pretick_alignment_ok.rs::forwarded_opening_drive`, `EXPECTED_FLAGGED`. All gone. This
    passage is mostly past tense ("was then"), so it is a mild case.
  - L959-1131 (§5.8, whole section): the re-derived D1 clauses (A)/(B), the probe battery `d*` "in
    `tests/sequential_forwarding_divergence.rs`", its unit pins and ui cases. Every one deleted. Written as
    the current rule.
  - L1132-1157 (§6 Requirements R1-R6): requirements for a guardrail that no longer exists. Historical.
  - L1158-1315 (§7 Phases 0-3): marked DONE / SUPERSEDED. Historical, labelled adequately, no change needed.
  - L1326-1330 (Phase 4, item 4c): "**4c OPEN** — retire this doc to a status note, or move it to
    `OUTDATED/`. Not yet: the trailing rule's retirement is still gated on phase C". Phase C landed
    2026-10-23 (the doc's own header says so) and the whole family went 2026-11-06. This open item is now
    actionable, which is the action proposed below.
  - L1396-1399 (§10 intro): "The field has converged on two answers, and Copper currently has neither".
    Stale. D-8 adopted Prost's one-job body and every clocked output is a `RegOut` (R8; `check_no_plain_out`).
    That is §10.1's "structural prevention" for outputs.
  - L1446-1453 (§10.3): "Copper is currently in the worst of the three positions … the hazard is writable".
    Stale for the same reason: a plain `Out` on a clocked body is refused by R8.
- Accurate claims checked (about 20): `crosses_more_than_one_tick` exists (cfg.rs:444); `lower_stmt_list`
  is in shir_lower.rs:344; `tests/regout_forwarding_equivalence.rs` exists (L565, with RegOut tests);
  `tests/module_composition_hybrid.rs`, `tests/fixtures/out_phase_dut.rs`, `examples/cpu/rv32i_cpu_pipelined.rs`,
  `rv32i_cpu_transpilable` and `tests/probe_timing_investigation.rs::probe_fsm_sim_matches_verilog`
  exist; `mac_fsm_sim_matches_transpiled_verilog` is in tests/mac_fsm_equivalence.rs;
  `if_tick_sim_matches_transpiled_verilog` is in tests/nested_tick_equivalence.rs; `COPPER_ALWAYS_BARRIER`
  is correctly described as gone (L515-516). §1 D1/D2, §3.1-§3.4, §5.1-§5.3 are dated measurements with
  dated verdicts and remain valid *as history*. §10 (prior art: Chisel, Verible, Prost, MyHDL) is still
  valid research, apart from the two "currently" sentences above.
- Proposed action: **add a dated "historical, superseded" header** at L1-2, above the status block. It
  should say: the whole family (all rule functions, the opt-out, the pins, the ui cases, the
  `sequential_forwarding_divergence.rs` battery) was deleted by D-8 / Phase C step 3 (2026-11-06); a
  plain `Out` on a clocked module is refused by R8 (`check_no_plain_out`); the authority is SPEC §II.3.2;
  everything below is the record as of 2026-11-02. Also fix the two "currently" sentences in §10 (or
  footnote them), and close 4c (L1326).
  - **JUDGEMENT: header in place vs move to `OUTDATED/`.** Options: (a) header only, keep the path;
    (b) move to `design_docs/OUTDATED/`, which breaks about 30 inbound refs, including code comments,
    `tests/sequential_forwarding_divergence.rs:10`, CLAUDE.md, README, SPEC_DECISIONS.md:1181.
    **Recommend (a).** Too many live pointers name the path, and CLAUDE.md already presents it as the
    historical record at this path. Separately, fix README.md:580's "its compile-time rules" wording.
    Side findings outside this doc, in code comments (report only):
    - `tests/two_domain_hierarchy_cdc.rs:31-34`: "D1 is guarded at compile time
      (`copper_analysis::unprotected_pretick_out_write`) … The pinned measurements live in
      `tests/sequential_forwarding_divergence.rs`".
    - `copper-analysis/src/cfg.rs:238,487,908`: rustdoc intra-doc links to the deleted
      `Self::multi_write_collapse` / `Self::multi_phase_out_write`, now dangling.
    - CLAUDE.md's attribute-parser bullet still names `copper-analysis/tests/pretick_alignment_corpus.rs`
      as an existing scan.

---

##### design_docs/CYCLE_DATAFLOW_SEMANTICS.md (1982 lines)

- Classification: **HISTORICAL**, with a small **CURRENT** remnant (§10.12, plus the denotation's
  program-order and commit clauses), and its own title claims to be normative. Its central construct is
  the segment's **two temporal anchors** (§2 L165-176) with the opening-prefix/closing-suffix split "at
  the read-site barrier" (L171-180), plus the executor/transpiler obligation table (§3 L195-201). That is
  exactly the two-job body D-8 reversed (SPEC_DECISIONS.md D-8: "Reverses … D-7 A (the barrier)";
  `PreEdgeBarrier` deleted per copper-sim/src/lib.rs:57). So the model is **SUPERSEDED by SPEC §I.0**,
  not just its §9–§10 rules. CLAUDE.md:396 calls only "§9–§10" historical, and CLAUDE.md:381 still lists
  the doc under "the cycle-dataflow model and its derivation" as if current. Both understate how much
  is superseded.
- Claims normative?: **Yes.**
  - L1 title: "the normative timing model for the async-await surface".
  - L35: "**The denotation is normative.** The per-cycle value function defined here is the semantics".
  - L45-47 does hedge: "The normative statement of the model now lives in `SYNCHRONOUS_SEMANTICS.md` …
    this document is the derivation record". But SPEC.md is normative now, and SYNCHRONOUS_SEMANTICS is
    another audit's subject.
- Header present?: **No retirement header.** The status block (L3-50) is "status as of 2026-09-01", updated
  through 2026-10-30. Nothing mentions D-8, R8, SPEC.md, or the deletion of the rules §9-§10 derive.
- Inbound refs: CLAUDE.md:381, 396; README.md:481, 532, 550, 585 (481 cites `linear_out_write_anchor_mismatch`
  "§10.7–10.9" as a rule); `copper-analysis/src/lib.rs:160`, `cfg.rs:903` (`derivation_facts`, "phase 1");
  `copper-core/src/frontend_ir.rs:36` (§2 c2 obligation); `tests/fixtures/comb_default_override_dut.rs:9` and
  `copper-codegen/tests/comb_definite_assignment_parity.rs:2` (§10.12, current); `copper-codegen/src/bin/derivation-audit.rs:3`
  (§7); `tests/sequential_forwarding_divergence.rs:9` (as the record of §9–§10); SPEC.md:635
  ("CDS §8-1's dual-consumer refinement"); SPEC_DECISIONS.md:1181, 2591 (CDS §5, §9–§10.12);
  PRETICK_ALIGNMENT_GUARDRAIL.md (22); DERIVATION_TABLE.md (6); PAIRED_IMPLEMENTATION_SCOPE.md (5);
  SYNCHRONOUS_SEMANTICS.md; others. Heavily referenced, including from SPEC.
- Stale claims:
  - L1: title "the normative timing model". SPEC.md is normative (D-8 rewrote §I.0–§I.6).
  - L3-50 (status block): "DERIVED, VALIDATED, AND IMPLEMENTED". It describes five live plain-`Out`
    rules (`multi_phase_out_write` "narrowed, not retired" L15; D1 "the rule now refuses exactly …"
    L19-20) and "The executor was **not** changed — scoping showed it already implements the model" (L42-43).
    The executor has since been replaced by the one-job resumption (D-8), and all rules are deleted
    (RETIRED-FAMILY).
  - L35-38: "1. **The denotation is normative.** … A sim ≠ SV disagreement is no longer adjudicated case by
    case — the model assigns which side failed". Superseded by SPEC.md §III.2's relation.
  - L53-58: "today the cycle a statement's effects land in is decided by *where the task parks* (barrier
    vs tick)". The barrier is deleted, and "today" is false. (The dated note at L60-66 revises this but
    still asserts the barrier "**is** the region boundary".)
  - L68-70: "Companions: `SYNCHRONOUS_SEMANTICS.md` (the reference — rebased onto this model 2026-08-27)".
    The reference is SPEC.md now.
  - L130: "Prost, the only other coroutine HDL, lowers (§10.5)". This document's §10.5 is "The decision:
    both clauses narrowed" (L954); the Prost section is PRETICK_ALIGNMENT_GUARDRAIL.md §10.5. A broken
    cross-reference.
  - L132-133: "**Registers = the liveness rule** (defined-in-loop ∧ live-across-a-tick, with the back-edge
    clause). Unchanged". The clause was deleted (cfg.rs:365 and 2932: "The back-edge clause is gone (Phase
    C step 6, SPEC I.7.5, D-9)").
  - L158-161: "**Plain `Out` drives anchor to the opening edge.** A plain `Out` written in the segment is
    cycle N's combinational drive". A plain `Out` on a clocked module is refused (R8, `check_no_plain_out`,
    copper-analysis/src/lib.rs:101).
  - L163-180: "a segment has two temporal anchors … A segment divides at the read-site barrier into an
    opening prefix … and a closing suffix". This is the two-job body D-8 reversed. Barrier deleted.
  - L181-235 (§3, whole section): the obligation table "executor: run … — no barrier / barrier — run in
    the closing edge's pre-edge settle" and the plain-`Out` emission column. Then L224-235: "The
    transpiler cell is the opening-prefix selection in `copper-codegen/src/shir_lower.rs`: the `Forwarding`
    walk tracks whether an `In` read has been seen (`forward_opening_drives` / `seen_in_read`) … takes its
    `edge_value` (… `SHIRPortDrive` …). Pinned by `…::pre_tick_update_forwarding_agrees_end_to_end` …
    `::d_narrowing_battery_verdicts` … `ui/pass/pretick_alignment_ok.rs::forwarded_opening_drive`".
    `Forwarding` still exists (shir_lower.rs:198). `forward_opening_drives`, `seen_in_read`, `edge_value`
    and `SHIRPortDrive` have 0 hits; both tests have 0 hits; the ui file is missing.
  - L237-284 (§4, the commit-frontier condition for plain-`Out` writes): L256-257 cites
    `memory_result_drives_plain_out` as a live rule (deleted, macro lib.rs:405). L270-284: "`Cfg::unprotected_pretick_out_write`
    … now flags … (`leading_read_reaches`) … keeps … (`written_on_all_paths`) … pinned by
    `copper-analysis/tests/pretick_alignment_corpus.rs::…` (`EXPECTED_FLAGGED`: …)". All 0 hits or missing.
    Also `independent_hardware_anchors_the_corrected_spelling` (0 hits).
  - L286-309 (§5 table): rows cite deleted tests (`d2_is_fixed_and_d1_still_demonstrates_the_hazard`,
    `m2_model_forwarded_lowering_matches_the_simulator_for_trailing_update`, `v8a_…`, all 0 hits). They
    cite `copper_analysis::classify_reads` as "the partial implementation" (L296, retired). L299 cites
    `vlir_lower::opening_hold_outputs` (0 code hits). L305 says `Cfg::trailing_regout_reads_input` refuses
    a shape (0 definitions; fixture comment only). L307 says the memory rules are "implemented in
    `Cfg::check_memory_staging` and `Cfg::memory_result_drives_plain_out`": the first exists
    (memory_rules.rs, cfg.rs), the second is deleted. Exceptions that check out: L303
    `regout_forwarding_equivalence.rs` exists; L306 `rv32i_pipelined_verilator.rs::pipelined_cpu_matches_its_verilated_self_on_all_programs`
    exists at line 146; L307 `m3_*` tests exist (tests/cycle_dataflow_memory_derivation.rs:116,175).
  - L310-341 (§6): labelled "pre-scoping sketch", so it is historical. But L333-341 says "the
    `allow_pretick_alignment` opt-out shrinks … `pretick_out_write_before_update` and
    `multi_phase_out_write` are unchanged". The opt-out is now refused (front_end.rs:53).
  - L374-388 (§8 items 2, 3): `has_nested_tick` "the source-level mirror of control extraction" (0 hits);
    "Answered 2026-10-23 by refusal (`Cfg::out_first_driven_late`) … refused in both front-ends". Deleted.
  - L399-684 (§9, Phase E, whole section): the derivation and measurement of `multi_phase_out_write` over
    the `e*` battery "in `tests/sequential_forwarding_divergence.rs` … every trace pinned". The rule and
    battery are deleted. L422 and 617 cite `vlir_lower::opening_hold_outputs` (0 code hits); L596 cites
    `e9_a_read_…` (0 hits); L638 cites `copper-codegen/tests/hold_form_audit.rs` (missing); L572 cites
    `ui/pass/pretick_alignment_ok.rs` (missing).
  - L685-1926 (§10.1-§10.11, whole range): the D1 re-derivation and the linear-route / V8 / trailing rule
    family. Present-tense examples: L856 "**The forms, as the lowering emits them today**"; L897 "the
    shapes the two clauses refuse today"; L1287-1289 "enforced in both front-ends … a plain-`Out` drive is
    refused iff …"; L1111, 1163, 1253, 1288, 1479 key on `has_nested_tick` (0 hits). The ui cases cited
    (L830, 1007, 1133, 1324, 1574, 1705) are all missing; the `e*`/`d*` probes are all deleted. Every
    "plain `Out` on a clocked body" here is now refused by R8. The §10.8 and §10.9 tables do say "rules
    today is what the tree at 444e0bd says" (L1429), which dates them correctly. The surrounding prose
    does not.
- Still valid and current:
  - §1 L97-117: the write-window invariant as an abstract statement. It is moot for clocked modules
    (which have no plain `Out`), but not false.
  - §2 L124-131, first bullet: program order with forwarding. This is still the meaning (SPEC I.0.2:
    "every statement of a segment reads a register it assigned earlier as the assigned value").
  - §2 L134-150: register authority. `capture_frontend_ir` fills `registers` from
    `copper_analysis::infer_registers` (copper-codegen/src/parser.rs:68). `codegen_registers_match_shared_inference`
    exists (register_reconciliation.rs:368). `held_temp_trap` exists (tests/fixtures/aggregate_locals_dut.rs:22).
    Only the "back-edge clause" clause is stale.
  - §2 L152-157: "Commits anchor to the closing edge … sampled … (`x_{N+1}`)". Consistent with D-8
    ("every read sees `x_{N+1}`", CLAUDE.md).
  - §5 L307 / §8 item 4: the m3 memory derivation. The tests exist and are cited by SPEC.md:1727, 1932.
  - §7 L342-361: the migration discipline, plus `copper-codegen/src/bin/sv-baseline.rs` (exists) and
    "there is no `COPPER_SEMANTICS` variable" (correct, 0 hits).
  - L71-75: `derivation-audit` exists (copper-codegen/src/bin/derivation-audit.rs). It still runs
    `Cfg::derivation_facts` (cfg.rs:924), which is reporting only. Its own comments (L168-169) note that
    the guard columns are retired.
  - **§10.12 L1927-1982: CURRENT.** It concerns the combinational route, untouched by D-8.
    - `check_definite_assignment` is cfg.rs:804, called by the transpiler at
      copper-codegen/src/lib.rs:532 inside `vlir_for_target` (lib.rs:394), as stated.
    - `lower_comb`, `check_no_latches` and `nested_port_drives` (vlir_lower.rs:1185) exist; so does
      `expr_width` (chir_lower.rs).
    - The six `cdo_*` probes are in tests/fixtures/comb_default_override_dut.rs, and the ui cases
      `combinational_latch`, `_second_port`, `_two_ifs` (plus `_in_for`) exist.
- Accurate claims checked: about 25, listed above.
- Proposed action: **add a dated "historical, superseded by SPEC.md §I.0 (D-8)" header**. It should:
  - drop or qualify "normative" in the title, L1, and L35;
  - say the two-anchor model (§2–§3) was reversed by D-8 (2026-11-04, landed Phase C step 3,
    2026-11-06), that every rule §4–§10.11 derives is deleted, and that the back-edge clause went with D-9;
  - list what survives: program-order forwarding, the closing-edge commit, the register authority, the m3
    memory derivation, and §10.12;
  - fix the L130 cross-ref to point at PRETICK_ALIGNMENT_GUARDRAIL.md §10.5.
  - **JUDGEMENT: what to do with §10.12.** Options:
    - (a) leave it in place under the header with a "still current" marker;
    - (b) move its substance to SPEC (the combinational definite-assignment rule) or to the parity audit
      docs, and leave a pointer;
    - (c) split the doc into a short "cycle-dataflow model: what survived D-8" note and an OUTDATED/ record.
    **Recommend (a) now, (b) later.** Two live code comments cite §10.12 by number, and SPEC and
    SPEC_DECISIONS cite §5, §8-1 and §9–§10.12 by number, so section numbers must not move.
  - Also: CLAUDE.md:381 lists this doc as the current "cycle-dataflow model". It should say "superseded by
    SPEC §I.0 (D-8); historical except §10.12", and CLAUDE.md:396's "§9–§10" understates the scope.
    README.md:481 presents `linear_out_write_anchor_mismatch` as a live rule.

### Group C (C_sweep_abi.md)

#### Audit C — sweep, anchoring, scheduler, ABI docs (tip c81b59f, 2026-11-09)

Measured on the worktree. Corpus count by `grep '^\s*#\[hardware'` over `examples/` + `tests/fixtures/`
(old/ pruned): **391 modules** (306 sequential, 81 combinational, 3 synchronizer, 1 structural).
`build.rs` SKIP has **3 rows** (`rv32i_cpu_pipelined`, `two_domain_top`, `mod8`) → 388 swept,
matching CLAUDE.md's "388 … 3 skipped". REF_SKIP and REF_SKIP_PATTERN are both empty.
Stimulus-policy constants verified at `build.rs:247-263` (SEEDS=4, SEED_CYCLES=200, FIRST_CYCLES=32,
EXHAUSTIVE_BITS=4, COMB_EXHAUSTIVE_BITS=8, COMB_VECTORS=64) and pinned in
`tests/corpus_generated.rs::the_stimulus_policy_is_what_the_design_doc_says` (asserts `(4,200,32,4,8,64)`,
2-sequence 1-bit case, 16-sequence 4-bit case, pattern list at 5 bits, RESET first live cycle 1,
truth table ≤ 8 bits). Seed k = `fnv1a_seed("<name>#k")` (`tests/common/mod.rs:360`). All CORRECT.

---

##### design_docs/CORPUS_DIFFERENTIAL_SWEEP.md (559 lines)
- Classification: **STALE** (meant to be the current as-built design of the sweep; §3.2 stimulus policy is
  accurate and pinned, but the doc predates the reference-interpreter leg, the agreement relation and
  observation 0, and several mechanism claims are wrong).
- Claims normative?: Not normative; "as-built design" (L1 "scope and as-built design"). It is the doc the
  policy pin names ("the sentence in the doc and the sequences the sweep drives cannot drift apart",
  `tests/corpus_generated.rs:24-29`), and CLAUDE.md lists it as "the sweep and anchoring" reference.
- Header present?: Status line L3 "phases 1–4 BUILT and green (2026-08-25)"; no superseded/historical header.
  §0 and Appendix A are self-labelled dated logs.
- Inbound refs (17): Cargo.toml, build.rs, TODO, CLAUDE.md, README.md, tools/regression.sh,
  tests/sweep_sequence_restart.rs, tests/common/mod.rs, tests/corpus_generated.rs,
  copper-sim/src/verification.rs, examples/sequential/simple_counter.rs, examples/combinational/full_adder.rs,
  design_docs/{SYNCHRONOUS_SEMANTICS,TIMING_COVERAGE_MATRIX,PRETICK_ALIGNMENT_GUARDRAIL,SPEC}.md,
  design_docs/review/INVENTORY_DOCS.md.
- Stale claims:
  - L15-20: "Every ignored one is a row in `SKIP` — a recorded transpiler refusal, … a deliberate divergence
    witness, a stimulus precondition (`mod8`), or the pipelined CPU" — `build.rs:52-110` has exactly 3 rows:
    CPU (Memory parameter), `two_domain_top` (structural/two-clock), `mod8`. No transpiler-refusal or
    divergence-witness row exists; the build.rs comment at L65-77 announcing "The claim ledger … Each module
    below is a KNOWN-WRONG lowering" precedes only `two_domain_top` and `mod8` (itself a stale comment in
    build.rs, outside this doc but worth noting).
  - L12-14: latest count quoted is 2026-10-28 "146 running, 9 ignored … out of 155"; tree is 388/3/391.
    Dated, and L9 says trust the tool, so minor — but "printouts above" (L12) refers to nothing above.
  - L197-211 (§2 table, "counted 2026-09-01"): 143 modules / 121 seq / 19 comb / 2 sync; now 391 / 306 / 81 / 3
    (1 structural, 2 array-port modules, 1 Memory-parameter module still true). Dated, but §2 is titled
    "The corpus, measured" in a current doc.
  - L248: "emits one `#[test]` per module" — `emit_file` (`build.rs:1004-1022`) emits TWO per module:
    `<name>_differential` and `<name>_reference_leg` (`Leg::Reference`, `build.rs:1031-1043`).
  - L351-358 (§3.3 Comparison): "the trace comparison is trivially satisfied and the Verilator leg carries the
    weight" — still the comment on `record_differential` (`tests/common/mod.rs:946-956`), but the differential
    case now also runs the copper-ref interpreter (`build.rs:1033` "and — unless REF_SKIPped — the reference
    interpreter, so one failure report carries all three readings") and judges each sequence under SPEC §III.2's
    relation (`tests/common/relation.rs`, verdict ledger `COPPER_VERDICT_LEDGER`, OUTSIDE/TRUNCATED/EXCUSED/
    DISAGREE; `record_observation_zero`, `with_reset_row`, `record_failure` at `tests/common/mod.rs:899-944`).
    None of this is in the doc: 0 hits for `reference leg`, `copper-ref`, `REF_SKIP`, `observation 0`,
    `relation`, `SweepDomain` in the file.
  - L389-391: "the three `RESET`-row modules … their reset is asserted on cycle 0 and the comparison starts
    there" — since the relation landed, a RESET-row module's observation 0 is outside the SV pair
    (`with_reset_row`, `tests/common/mod.rs:899-905`: "the relation covers obs_k, k ≥ 1").
  - L376-384: present-tense description of registers codegen synthesizes includes "the edge-sampled register of
    a conditionally driven plain `Out` (`bsg_dff_en`'s `data_o`, `conditional_plain_out`'s `o`)" and
    "`vlir_lower` already gave a `<port>_hold` register `initial = 0`" — plain `Out` on a clocked module is refused
    (R8); `conditional_plain_out`'s `o` is now `RegOut` (`tests/fixtures/regout_forwarding_dut.rs:59`); the hold
    form is deleted (`copper-codegen/src/vlir_lower.rs:336` comment lists it among the removed forms; `_hold`
    0 hits in codegen emission).
  - L402-415 (§3.4): "four reviewed tables at the top of `build.rs`" — there are six: SKIP, PARAMS, REFERENCE,
    RESET, **REF_SKIP** (`build.rs:222`, build-time validated against the corpus, L422-445) and
    **REF_SKIP_PATTERN** (`build.rs:230`, required by G-G). Table omits both.
  - L417-420: generator ignore reasons list is complete for the differential leg, but omits `ref_skip_reason`'s
    signature classes (structural, Memory, >1 clock; `build.rs:787-833`).
  - L435-441 (§3.5): "G-D … prints `G-D NOT SATISFIED` and forces `PARTIAL` when `verilator` is not on the path" —
    0 hits for `NOT SATISFIED` in the tree; G-D (`tools/regression.sh:277-296`) now requires the coverage guard's
    and the stimulus pin's `ok` lines and counts `_differential` ok/ignored, with no Verilator probe. Verilator
    presence is enforced only by `tools/regression.sh:490` (`fail "verilator not on PATH"`, examples phase), so a
    bare run still cannot pass without it. G-D also now requires the stimulus-policy pin (doc §3.2 L290 says so,
    §3.5 does not).
  - L443-444 (§3.5): "This repo already had G-A, G-B and G-C" — the guards relevant to the sweep now include
    G-F (reference leg count = build.rs `REF_LEG_EXPECTED`), G-G (verdict ledger), G-I (projection over
    `corpus_manifest.rs`), none mentioned. `build.rs:536-545` also writes `corpus_manifest.rs`
    (`SWEEP_MODULES`) — not mentioned.
  - L104, L107 (§0 2026-10-29 log): `vlir_lower::opening_hold_outputs` — 0 definitions (one stale comment at
    `tests/fixtures/control_extraction_dut.rs:109`); `copper-codegen/tests/hold_form_audit.rs` — file missing.
    Dated log entry, but no note that both are gone (Phase C step 4).
  - L73: `tests/sequential_forwarding_divergence.rs::branch_merge_tail_leads_its_sv_when_the_first_cycle_takes_sel_one`
    — 0 hits (file exists; test renamed/removed). Dated log.
  - L173, L177 (§1 table): "Memory result → plain `Out`", "D1 pre-tick alignment instances" — historical defect
    rows naming retired mechanisms (D-8); acceptable as evidence but unlabelled.
  - L505 (Appendix A, B2): `vlir_lower::conditional_output_ports` — 0 definitions (one mention in
    `tests/plain_out_multiphase_equivalence.rs:31`). Appendix is labelled "measured 2026-10-29 (since stated)",
    so this is historical; fine as is.
  - Minor: the generic-over-domain handling (`SWEEP_DOMAIN = "SweepDomain"`, `build.rs:592`, D-22) is not
    described in §3.1.
- Accurate claims checked (~35): all six stimulus constants and §3.2 semantics (seed 0 = historical FNV-1a
  `seed_of`, `#k` seeds, W excludes the RESET port, first live cycle 1 for RESET, pattern list 2+2P with dedup,
  truth table ≤ 8, comb SEEDS×64 + patterns); `StimulusPlan`, `COPPER_SWEEP_SEQUENCE` (`SWEEP_SEQUENCE_ENV`);
  `delete top; top = new V<module>()` (`verification.rs:247-248`); FAIL message format with `(sequence k "…",
  cycle n)` (`verification.rs:390`); `collect_rs` prunes `old/`; `wrapper_name` path-derived (`ex_`/`fx_`);
  `//!` header → ignored (`emit_file`, no current file starts with `//!`); `rerun-if-changed` on both dirs +
  build.rs; `Module::from_item`, `classify`, `Kind::Memory`, `alias_ty`, `skip_reason` reasons; SKIP-key and
  REFERENCE build-time validation (exists/file/declares module/not `@generated`); anchored count excludes
  SKIPped; `every_corpus_module_has_a_generated_case`; `differential_only`/`record_differential` mutual
  exclusion; `with_hand_written_reference`; `RandStim for [T; N]`; `COPPER_VERILATOR_X_INITIAL` →
  `--x-initial unique --x-assign unique` + `+verilator+rand+reset+2` (`verification.rs:575-632`),
  `copper_sim::VerilatorPowerOn`, `VLIRSeqBody::port_inits`, `chir_lower::lower_register_init`,
  `control_extract::hoist_cross_state_locals`, `copper_analysis::check_register_initializers`,
  `copper-analysis/tests/register_initializer_corpus.rs` (doc's L123 says `register_initializer_corpus.rs`, it
  lives in copper-analysis); `copper_codegen::legalized_port_name`; RESET rows (shift_register, simple_counter,
  rv32i_cpu_transpilable); `mod8` SKIP; `tests/corpus_equivalence.rs` deleted; arrays still only `mux` and
  `bsg_mux_one_hot`; Memory parameter still only `rv32i_cpu_pipelined`; referenced test files
  (`lfsr_equivalence.rs`, `wide_and_mod_datapath_equivalence.rs`, `rv32i_pipelined_verilator.rs`,
  `randomized_sequential_equivalence.rs`, `tools/transpile_coverage.sh`, `paper/stats/SUMMARY.md`) exist.
- Proposed action: **fix in place.** (1) Rewrite the lead paragraph's SKIP-class sentence to the three present
  classes; drop "printouts above". (2) §3.1: two cases per module (`_differential`, `_reference_leg`), the
  `SweepDomain` rule, `corpus_manifest.rs`. (3) New §3.3a (or replace §3.3's first paragraph): the three legs
  and the SPEC §III.2 relation, pointing to SPEC rather than restating it (observation 0, RESET-row obs 0
  outside the SV pair, verdict ledger). (4) §3.4: add REF_SKIP / REF_SKIP_PATTERN rows. (5) §3.5: delete the
  `NOT SATISFIED` sentence, state what G-D checks now (coverage guard + policy pin + counts), and list G-F/G-G/G-I
  as the sweep's other guards. (6) Add "(since removed, Phase C step 4)" notes on the §0/§3.3 references to the
  hold form, `opening_hold_outputs`, `hold_form_audit.rs` and plain-Out ports; mark §2's table and §1's defect
  table as dated. Keep §0 and Appendix A as the dated record.
- JUDGEMENT: whether to restate the relation here or only point to SPEC §III.2. Recommend pointing (SPEC is
  normative; CLAUDE.md already restates it at length) with a 5-line summary.

---

##### design_docs/ANCHORING_A_MODULE.md (160 lines)
- Classification: **STALE** (a current how-to; mostly accurate, two sections wrong since 2026-10-29/30).
- Claims normative?: No; procedural guide ("Adding one", "The contract the reference has to meet").
- Header present?: None (no date/status line).
- Inbound refs (8): TODO, CLAUDE.md:377, README.md:226 and :582, design_docs/SPEC_DECISIONS.md, SPEC.md,
  CORPUS_DIFFERENTIAL_SWEEP.md, prior_art/basejump.md, review/INVENTORY_DOCS.md.
- Stale claims:
  - L81-88: "**No `initial` block for memories.** The harness passes no `--x-initial` flag … the transpiler emits
    no `initial` either, so the two start from the same state." — false since 2026-10-29: the transpiler emits
    `initial` values for every constant register initializer and zero fills for `Memory::new` arrays and
    synthesized registers (`chir_lower::lower_register_init`, `VLIRSeqBody::port_inits`); the harness DOES pass
    `--x-initial unique --x-assign unique` and `+verilator+rand+reset+2` under `COPPER_VERILATOR_X_INITIAL=1`
    (`copper-sim/src/verification.rs:575-632, 997-1001`); and the worked example itself,
    `tests/fixtures/reference_sv/ram_read_first.sv:20,37`, gained an `initial` block on 2026-10-30 precisely so
    it passes under randomized power-on (as do `handshake.sv:35`, `fifo_mem_new.sv:26`, `hybrid_pipeline.sv`).
    The guidance should now be the opposite: state the same zero power-on the transpiler states.
  - L83-84: the quoted Verilator command line omits `-CFLAGS -std=c++14`, the `-O0` MAKEFLAGS and the power-on
    flags (`verification.rs:990-998`). Minor.
  - L74-77: "**A combinational reference still needs `clk`** — the harness always drives one … Consume it with an
    empty `always_ff @(posedge clk) begin end`, which is exactly what the transpiler emits" — the harness drives
    `top->clk` only when the SV text contains `input` and `clk` (`verilog_has_clock_port`,
    `verification.rs:1151-1154`, used at :277/:892); the transpiler emits no clock and no empty `always_ff` for a
    combinational module (`copper-transpile examples/combinational/one_bit_comparator.rs` output has no `clk`;
    `begin end` 0 hits in `copper-codegen/src`). `bit_not_bits.sv` still carries the idiom (harmless).
  - L127: quoted failure line `FAIL: Cycle 14 data expected 0 got 166` — multi-sequence runs now append
    `(sequence k "label", cycle n)` (`verification.rs:390`). Minor; re-measure if kept.
  - L27-28: "first verified 2026-09-01 at 2" — dated; REFERENCE now has 3 rows (adds `handshake`,
    `build.rs:177-187`). Accurate as a date-stamped fact; L53-55 example table fine.
  - Terminology: the doc calls the hand-written Verilog check "the reference leg" (L22, L136, L142); since Phase C
    step 9 `_reference_leg` is the copper-ref interpreter case (`build.rs:1038-1042`, G-F). Not wrong at writing,
    now ambiguous.
- Accurate claims checked (~15): G-E prints and does not fail (`tools/regression.sh:405-420`); anchored =
  REFERENCE ∧ ¬SKIP (`build.rs:452-458`); `ANCHORED`/`UNANCHORED` constants and `cargo:warning=anchoring:` line;
  build-time REFERENCE validation (module exists, file exists, `module <name>`, not `// @generated`);
  `-Wall -Wno-DECLFILENAME` fatal; `with_params` mirrored; `HardwareTest::with_verilog`
  (`copper-sim/src/testing.rs:109`); `tests/verilog_hybrid_pipeline.rs`, `tests/verilog_fifo_memory_new.rs`,
  `tests/det_010_independent_golden.rs`, `tests/cdc_synchronizer_anchor.rs`, `examples/basejump/sv/bsg_dff_en.sv`
  exist; "=== … vs INDEPENDENT reference" print (`tests/common/mod.rs:1052,1321`); the three-way failure
  diagnosis messages (`tests/common/mod.rs:1098-1108`).
- Proposed action: **fix in place** — rewrite the "not required" bullet on `initial` (L81-88) to "state the
  zero power-on the transpiler states, or the reference fails under `COPPER_VERILATOR_X_INITIAL=1`", correct the
  combinational-`clk` paragraph (L74-77), add a dated status line, and rename "reference leg" to "independent-
  Verilog leg" to avoid colliding with the copper-ref `_reference_leg`.
- JUDGEMENT: none beyond the terminology rename (recommend doing it).

---

##### design_docs/LEVELIZED_SCHEDULING_SCOPE.md (198 lines)
- Classification: **HISTORICAL** design record with a current-facing header — partly STALE where it speaks
  in the present tense.
- Claims normative?: No — L3-4 "**Meaning:** `SPEC.md` §I.13.2–I.13.3 (D-18 …) … this document records the
  realization"; L6-7 "written 2026-07-30 before implementation and kept as the design record".
- Header present?: Yes — the SPEC "Meaning" pointer (L3) and a dated D-14 reversal box (L168-178) ending
  "The text above is the 2026-08-03 design record." No overall "historical" label at the top.
- Inbound refs (12): CLAUDE.md, README.md, copper-core/src/port.rs, copper-sim/src/executor.rs,
  tests/levelized_differential.rs, design_docs/{SYNCHRONOUS_SEMANTICS_IMPL_PLAN,SPEC,SPEC_DECISIONS,
  FORMAL_SEMANTICS}.md, prior_art/esterel.md, review/{MEASUREMENTS,INVENTORY_DOCS}.md.
- Stale claims:
  - L48-50: lists `diff_convergent_comb_loop` among the topologies run "continuously" under both schedulers —
    0 hits; replaced by `convergent_comb_loop_is_refused_at_spawn_by_both_schedulers`
    (`tests/levelized_differential.rs:364`). The file has five `diff_*` tests + that refusal pin. (The D-14 box
    names the replacement but sits 120 lines later, in phase 5.)
  - L72-77 (Gate section): "`PreEdgeBarrier` (`copper-sim/src/lib.rs`) is phase-constant …" — deleted
    (`copper-sim/src/lib.rs:57` comment: "`PreEdgeBarrier` / `pre_edge_barrier` (the parking point the macro
    injected …"), D-8 / Phase C step 3.
  - L106: "`spawn_wired` / `spawn_untracked` / `spawn_child` take … `reads`" and L192 "`spawn_child`
    (structural hierarchy)" — `fn spawn_child` 0 hits; deleted at Phase C step 11b
    (`tests/module_composition_hybrid.rs:6-9`). `spawn_wired` (`executor.rs:498`) and `spawn_untracked`
    (`executor.rs:457`) exist.
  - L124-129 (residual edge case): plain `Out` holding a registered value on `bsg_dff_en`, and "a Moore design on
    plain `Out` inside a cross-module loop still simulates correctly … iterated" — plain `Out` on a clocked module
    is refused (R8), and a multi-task comb SCC is refused at spawn (`HardwareExecutor::refuse_comb_cycle`,
    `executor.rs:536`; `executor.rs:207-209`: "every component the levelized settle meets is a singleton").
  - L141-143 (phase 3) "iterate-to-fixpoint only within an SCC (`iterate_scc`)" and L58-62 "SCC-iteration wins
    over panicking … A loop is reported only when the settle actually fails to converge" — `iterate_scc` 0 hits;
    reversed by D-14 (covered by the L168 box for phase 5, but the Correctness-discipline bullet L58-62 is not
    flagged).
  - L161-165: `levelized_oscillating_scc_trips_threshold`, `levelized_three_module_cycle_reports_whole_scc`,
    `comb_cycles_reports_convergent_latch_without_rejecting_it`, `non_convergent_combinational_loop_is_detected`
    — all 0 hits (the box gives the two replacement pins; `comb_cycles_empty_for_acyclic_design` still exists,
    `executor.rs:1332`).
  - L189: "`tick_clock` polls all tasks each settle" — since D-8 the pre-edge settle polls
    `PollSet::Network` then `PollSet::NetworkAnd(TypeId::of::<Domain>())` (`executor.rs:863-871`): only `D`'s
    coroutines are resumed by `D`'s tick.
- Accurate claims checked (~20): `SchedulerMode::{Fixpoint,Levelized}` with Levelized `#[default]`
  (`executor.rs:212-221`); `env_default` honouring `COPPER_SCHEDULER=fixpoint|levelized` case-insensitively
  (`executor.rs:233-240`); `PollOrder::{Insertion,Reversed,Seeded}` (`executor.rs:326`); poll-order fuzzer
  pinned to Fixpoint (`tests/poll_order_fuzz.rs:93-98`); golden traces: 6 designs × both modes (12
  `check_golden` calls); `OSCILLATION_THRESHOLD` 20 / `MAX_DELTA_CYCLES` 1000 (`executor.rs:21,24`);
  `tarjan_scc`, `compute_scc_plan`, `poll_tasks_levelized`, `poll_tasks_fixpoint`, `visit_order`, `port_dirties`,
  `ensure_graph`/`graph_dirty`/`levelized_plan`, `comb_cycles()`; `WireId`, `In/Out::wire_id()`, `WireKind`
  (`copper-core/src/port.rs:15,28,51,103`); `ClockEdgeListener`; `delta_yield`; generated cases pass `reads`
  (`build.rs:1207`); the D-14 box's refusal pins exist.
- Proposed action: **add a dated "historical design record; the current mechanism is SPEC §I.13.2–I.13.3 and
  `copper-sim/src/executor.rs`" header at the top**, and extend the existing reversal-box practice with short
  dated notes at L48-50 (diff_convergent → refusal pin), L72 (PreEdgeBarrier deleted, D-8), L106/L192
  (`spawn_child` deleted, D-23), L124-129 and L58-62 (D-14 / R8), L189 (one resumption per tick, D-8). Keep in
  `design_docs/` (CLAUDE.md lists it as "the scheduler" doc and executor.rs refers to it).
- JUDGEMENT: header-plus-notes (recommended — it is the only rationale for the levelized design and is cited by
  code) vs moving to `OUTDATED/` (would orphan inbound refs from executor.rs, port.rs, levelized_differential.rs
  and CLAUDE.md).

---

##### design_docs/ARRAY_PORT_ABI.md (206 lines)
- Classification: **CURRENT** (status header + rationale kept explicitly "in the tense it was written").
- Claims normative?: No; L3 "**Status:** DECIDED AND LANDED 2026-08-24 (`2618426`)"; L8-10 "§7 records what
  landed; the rest … is the option analysis … kept as the rationale".
- Header present?: Yes (status line with commit; commit `2618426` dated 2026-08-24 verified).
- Inbound refs (13): README.md, TODO, CLAUDE.md, copper-codegen/tests/array_ports.rs, copper-core/src/{vlir,chir}.rs,
  copper-codegen/src/chir_lower.rs, tests/{bsg_mux_one_hot_equivalence,mux_equivalence}.rs, tests/common/mod.rs,
  design_docs/{SPEC_DECISIONS,SPEC}.md, review/INVENTORY_DOCS.md.
- Stale claims: none found. (Nit: L68-69 describes `logic_vec_to_int` packing X as part of the drive; it still
  does — `verification.rs:399-409`, X → 1 — and the relation no longer compares X that way, but that is outside
  this doc's ABI claim.)
- Accurate claims checked (~20): `copper-transpile examples/combinational/mux.rs` emits
  `input logic [ELS_P-1:0][WIDTH_P-1:0] data_i` and `data_o = data_i[sel_i]` (run today);
  `bsg_mux_one_hot` emits `[ELS-1:0][WIDTH-1:0]` with `localparam`s; references
  `examples/combinational/sv/mux.sv:19` and `examples/basejump/sv/bsg_mux_one_hot.sv:15` declare
  `[els_p-1:0][width_p-1:0] data_i`; flatten comment at `examples/combinational/mux.rs:86`;
  `logic_vec_to_int` → `u64`, `top->name = value` (`verification.rs:273-274`); `Width` is still
  `Concrete | Param` with the `// M2 (later): Sub` note (`copper-core/src/chir.rs:108-115`);
  `CHIRType::Array { elem, len }`; `outer_dim` on VLIR port and `WireAssign` (`copper-core/src/vlir.rs:73,260`);
  `range_str` in emit.rs; the five `array_ports.rs` tests exist; `array_typed_port_is_unsupported` 0 hits;
  `tests/mux_equivalence.rs`, `tests/bsg_mux_one_hot_equivalence.rs` exist; `mux` in PARAMS (8,4,2);
  `RandStim for [T; N]` element-major (`tests/common/mod.rs:170-186`, cites this doc); still exactly two
  array-port modules in the corpus.
- Proposed action: **none** (keep as is).

---

##### design_docs/RECEIVED_MEMORY_ABI.md (121 lines)
- Classification: **CURRENT**.
- Claims normative?: No; L3 "**Status: DECIDED AND LANDED 2026-08-27** (`0ef2b5a`…)"; L8-9 "**Meaning:**
  `SPEC.md` §I.10.5 (D-16, 2026-11-05) … this document records the emitted realization".
- Header present?: Yes (status + SPEC pointer; D-25 additions dated inline at L76-83).
- Inbound refs (14): TODO, CLAUDE.md, README.md, copper-codegen/src/{vlir_lower,chir_lower}.rs,
  copper-core/src/chir.rs, tests/received_memory_abi.rs, design_docs/prior_art/{spade,basejump}.md,
  SPEC_DECISIONS.md, SPEC.md (II.3.3 row (i) cites §3), OUTDATED/AUDIT_2026-08-26.md,
  review/{MEASUREMENTS,INVENTORY_DOCS}.md.
- Stale claims: none found.
- Accurate claims checked (~20): `chir_lower::received_memory_decls` (`chir_lower.rs:2017`) refuses
  `WRITE_LAT != 1` (`:2054-2069`); `copper_analysis::memory_rules` refuses it in both front-ends with one message
  (`received_write_latency_message`, `memory_rules.rs:36-41`), refuses a literal latency 0 (`:113-115`) and a
  `Memory` on a `structural` module (`STRUCTURAL_MEMORY_MESSAGE`, `:31-32`); `Memory<…, READ_LAT = 1,
  WRITE_LAT = 1>` defaults (`copper-core/src/memory.rs:22-29`); simulator default ReadFirst
  (`write_first_mode: AtomicBool::new(false)`, `memory.rs:268`) and `.write_first()` (`memory.rs:332`);
  `CHIRMemoryDecl { received: bool }` (`copper-core/src/chir.rs:221,250`); `memory_locals` in
  `copper-analysis/src/cfg.rs:1668`; `lower_mem_decls`, `mem_write_commits`, `lower_to_vlir`, `addr_param_name`
  (`<M>_ADDR_W`), `mem_net_defaults`, `drop_unread_wires` all in `vlir_lower.rs`; `build_memory` in
  `examples/cpu/rv32i_cpu_pipelined.rs:515`; `tests/received_memory_abi.rs` and
  `tests/rv32i_pipelined_verilator.rs` exist; CPU SKIP row describes the dedicated lane (`build.rs:53-63`);
  `Kind::Memory` + `skip_reason` (`build.rs:746-753`); commits `0ef2b5a`, `b70f2a1` dated 2026-08-27; SPEC II.3.3
  row (i) agrees (SPEC.md:3737).
- Proposed action: **none** (keep as is).

---

#### Summary

| doc | class | #stale | action | judgement? |
|---|---|---|---|---|
| CORPUS_DIFFERENTIAL_SWEEP.md | STALE | 14 | fix in place (3 legs + relation, 6 tables, G-D text, SKIP classes, notes on retired hold form) | yes — point to SPEC §III.2 rather than restate (recommend point) |
| ANCHORING_A_MODULE.md | STALE | 5 | fix in place (`initial` guidance reversed, combinational `clk`, rename "reference leg") | minor — terminology rename |
| LEVELIZED_SCHEDULING_SCOPE.md | HISTORICAL (partly stale) | 8 | add dated historical header + inline dated notes; keep in place | yes — header+notes (rec.) vs OUTDATED/ |
| ARRAY_PORT_ABI.md | CURRENT | 0 | none | no |
| RECEIVED_MEMORY_ABI.md | CURRENT | 0 | none | no |

Side finding (not one of my docs): `build.rs:65-77`'s comment "The claim ledger … Each module below is a
KNOWN-WRONG lowering with a working twin" now heads only `two_domain_top` and `mod8`, neither a known-wrong
lowering — stale comment in build.rs.

### Group D (D_timing.md)

#### Audit D_timing — the cycle-dataflow derivation docs

Tree: c81b59f (2026-11-09). Greps over `*.rs`, `*.sh`, `Cargo.toml`, `build.rs`, excluding `target/`
and `paper/`. "MISSING" means `fn <name>` has 0 hits in the tree. No repo file was edited. The
`derivation-audit` bin was not run (I only read its source).

Shared context for all four docs:
- Git: all four were last touched 2026-10-29 (`e78fe89`) or earlier (`TIMING_COVERAGE_MATRIX.md`
  2026-10-23, `00ea33c`). None was touched by Phase C step 3 (D-8, 2026-11-06, `9e41b61`) or later.
- D-8 deleted most of what these docs cite as live. `tests/sequential_forwarding_divergence.rs:1-14`
  says the file "held the measured two-job divergence probes — 167 tests over 160 modules … the
  tests went with them". It now holds one test (`the_registered_output_form_agrees`, line 138).
  `copper-analysis/tests/pretick_alignment_corpus.rs` was deleted in `9e41b61` (2026-11-06).
- CLAUDE.md lists these four under "the cycle-dataflow model and its derivation" without a
  historical label. The same list does label `SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md` "historical
  plan", and its pre-tick bullet calls `PRETICK_ALIGNMENT_GUARDRAIL.md` and
  `CYCLE_DATAFLOW_SEMANTICS.md` §9–§10 "the historical record".

---

##### design_docs/DERIVATION_TABLE.md (325 lines)
- Classification: **HISTORICAL**. It records the 2026-08-26 per-module derivation under the two-job,
  plain-`Out`-on-clocked model, with status notes running to 2026-10-30. Every hazard class it
  derives (D1, V8, mixed anchor, input-fed plain `Out`) is unexpressible under R8 / D-8.
- Claims normative?: No. L3-9 says "Status: COMPLETE (2026-08-26) — SUPERSEDED as a work list by
  `PAIRED_IMPLEMENTATION_SCOPE.md` … Nothing in this file changes behavior; it is kept as the
  derivation record." But `SYNCHRONOUS_SEMANTICS.md:10` cites it as "the per-module derivation
  record" that grounds that doc's claims.
- Header present?: Partial. There is a "SUPERSEDED as a work list" status (2026-09-01). There is no
  pre-SPEC / pre-D-8 header.
- Inbound refs: CLAUDE.md:382; copper-analysis/src/cfg.rs:923, :1023;
  copper-codegen/src/bin/derivation-audit.rs:179; SYNCHRONOUS_SEMANTICS.md:10, :72;
  SPEC_DECISIONS.md:2590 (cites the "memory row", M8–M10); CYCLE_DATAFLOW_SEMANTICS.md:4, 72, 169,
  299, 371; PAIRED_IMPLEMENTATION_SCOPE.md:91; PRETICK_ALIGNMENT_GUARDRAIL.md:1390;
  review/INVENTORY_DOCS.md:10.
- Stale claims:
  - L13-27: "The mechanical columns regenerate … `cargo run … --bin derivation-audit`", which prints
    "today's guard verdicts". The bin exists (`copper-codegen/src/bin/derivation-audit.rs`, 300
    lines, last touched 2026-11-07 `b91ca42`). But its guard column is hard-wired empty:
    derivation-audit.rs:167-170 says "The guards this column reported (D1, mphase, mwrite, V8) were
    the pre-tick alignment family, retired by D-8 … `let guards: Vec<&str> = Vec::new();`". Since R8,
    the plain-`Out` columns it reports (`+outN`, `input-fed`, `sv-changes`) cannot fire on a
    clocked module. So the bin still runs (not verified by running it), but the table it
    "regenerates" no longer exists in its output. Its own comment at :175-176 still cites the
    deleted `pretick_out_write_before_update`.
  - L19: "`Cfg::derivation_facts` … the same CFG authority the timing rules read". The function
    exists (cfg.rs:924; lib.rs:164), but the "timing rules" are gone. Its doc comment (cfg.rs:908)
    and body comment (:927) link `Self::multi_phase_out_write`, and that function has 0 `fn`
    definitions in the tree (dangling intra-doc link; deleted per copper-codegen/tests/phase_sensitive_checks.rs:13).
  - L86-91: "exact-set pins in `copper-analysis/tests/pretick_alignment_corpus.rs` … four
    `EXPECTED_*` sets". The file was deleted 2026-11-06 (`9e41b61`).
  - L119-126: V8 battery `v8a_…`, `v8b_…`, `v8c_…`, `v8t_trailing_stage_shape_verdict` in
    `tests/sequential_forwarding_divergence.rs`. All MISSING. That file's header (L1-14) records
    their deletion with D-8.
  - L134-136: "Derived rule (confirmed): … a plain-`Out` write placed after the leading read must
    come after every update". The rule (`pretick_out_write_before_update`) has 0 `fn` hits, and its
    only mention is a comment at derivation-audit.rs:176.
  - L159-160: "`Cfg::unprotected_pretick_out_write`'s register-reading clause now requires
    `leading_read_reaches`". `unprotected_pretick_out_write` has 0 `fn` hits (only comments, e.g.
    vlir_lower.rs:3349). `leading_read_reaches` has 0 hits.
  - L184: `add_then_write`, `fast_counter` witness, `pre_tick_update_forwarding_agrees_end_to_end`,
    `d_narrowing_battery_verdicts`: all MISSING.
  - L185: `trailing_update` row, "`vlir_lower::opening_hold_outputs` … m2 test's final assertion is
    `assert_eq`". `trailing_update` and `m2_model_forwarded_lowering_…` are MISSING.
    `opening_hold_outputs` survives only in a comment (tests/fixtures/control_extraction_dut.rs:109).
    The function and the hold form were deleted in Phase C step 4.
  - L186: `pc_arm_write`, `pc_arm_toggle` are MISSING. `branch_merge_explicit` survives in
    tests/fixtures/control_extraction_dut.rs, but the mux + hold form the row cites as the reason
    it agrees is gone (Phase C step 4).
  - L187: `probe_fsm` "path-dependent region boundary … the shared write". The fixture now drives
    `out: RegOut<Bits<8>, MainClk>` (tests/fixtures/probe_timing_dut.rs:47), so the plain-`Out`
    analysis no longer describes it. `w4_mixed_alignment` is MISSING. `probe_fsm_sim_matches_verilog`
    exists (tests/probe_timing_investigation.rs).
  - L188: `pulse_plain` is MISSING, as are `EXPECTED_MULTI_PHASE` (0 hits) and `multi_phase_out_write`
    (no fn). "the opt-out stays for `out_first_driven_late`": the opt-out was retired (refusal
    message at copper-analysis/src/front_end.rs:53-54), and `out_first_driven_late` has no fn (only
    comments, cfg.rs:2028, :2919).
  - L189: `ram_prewrite` "keeps its `allow_pretick_alignment` attribute … `EXPECTED_FLAGGED`". It now
    drives `dob: RegOut` (tests/mem_latency_probe.rs:49). The opt-out is refused. The only remaining
    `EXPECTED_FLAGGED` is an unrelated empty set in copper-analysis/tests/register_initializer_corpus.rs:72.
  - L209-231: the "Printout of 2026-09-01" totals and the line "its source … is unchanged since
    2026-08-27". The bin was edited 2026-11-07 (`b91ca42`), and its output can no longer reproduce
    those buckets (see L13-27).
  - L304-321: "`Cfg::pretick_out_write_before_update` … macro-enforced with the
    `allow_pretick_alignment` opt-out … `copper-macros/tests/ui/fail/write_before_update.rs` /
    `ui/pass/pretick_alignment_ok.rs`" and `out_phase_dut.rs::out_from_reg_before_commit`. The fn,
    both ui files and `out_from_reg_before_commit` are MISSING. `write_before_update_flags_exactly_…`
    is MISSING.
  - L322-325: "stated normatively in `SYNCHRONOUS_SEMANTICS.md`". Since 2026-11-03..09 SPEC.md is
    the normative document (per the brief), so this is a pre-SPEC claim.
- Accurate claims checked (12): `copper-codegen/src/bin/derivation-audit.rs` exists, and its
  `collect` skips `old/` (L18-19). `Cfg::derivation_facts` exists and carries the three documented
  approximations, including the `RegOut`-commit blind spot (cfg.rs:902-923). `control_input_read`
  exists (cfg.rs:192, :1017). The fold-artifact comment is in the function body (cfg.rs:1018-1026),
  as L265-266 says. The m3 tests `m3_rom_from_fn_…` and `m3_dual_port_ram_…` exist in
  tests/cycle_dataflow_memory_derivation.rs. `m4_uart_sampling_edges_…` exists in
  tests/cycle_dataflow_uart_derivation.rs. `a_bit_select_on_a_just_updated_register_matches_verilog`
  and `a_match_scrutinee_reads_the_registers_own_update` exist.
  `tests/fixtures/branch_on_updated_reg_dut.rs` exists. `single_loop_local_ok.rs::accum` exists.
  `lfsr`, `mac_fsm` and `counter`-family fixtures exist.
- Proposed action: **add a dated header**: "HISTORICAL (pre-D-8). Derivation record for the two-job
  executor's plain-`Out` timing. The rules, tests and witnesses it names were deleted by Phase C
  steps 3–4 (2026-11-06; see `tests/sequential_forwarding_divergence.rs` header and
  `review/MEASUREMENTS.md`). The surviving measurements m3/m4 live in
  `tests/cycle_dataflow_{memory,uart}_derivation.rs`. Normative: `SPEC.md`." Then move the doc to
  `OUTDATED/` and re-label it in CLAUDE.md.
  JUDGEMENT on whether to move it:
  - (a) Header only, kept in place. This keeps live links working: `SPEC_DECISIONS.md:2590` cites
    its memory row, and code comments cite it (cfg.rs:923, :1023; derivation-audit.rs:179).
  - (b) Move to `OUTDATED/` and update those 4 code comments plus CLAUDE.md:382.
  - I recommend (a) now and (b) together with a decision on the bin.
  Related code JUDGEMENT, out of scope for a doc edit: `derivation-audit` is now vestigial. Its guard
  column is constant-empty and its plain-`Out` columns cannot fire. Options: retire the bin (and
  `Cfg::derivation_facts`, "reporting only", cfg.rs:902) or re-purpose it. Either way, fix the
  dangling `multi_phase_out_write` doc links at cfg.rs:908 and :927.

##### design_docs/PAIRED_IMPLEMENTATION_SCOPE.md (317 lines)
- Classification: **HISTORICAL**. It is the scope and execution record of the 2026-08-26 migration
  (phases A–E), with status notes through 2026-10-30. Everything it implemented was itself
  replaced by D-8 and Phase C step 4 (one output form, no trailing fold, no plain `Out` on a clocked
  module).
- Claims normative?: No. L3: "Status: EXECUTED 2026-08-26 … Original scope follows, kept for the
  record" (L76).
- Header present?: Yes: "EXECUTED", with status markers "reflect 2026-09-01" plus later dated notes.
  It does not say the result itself is superseded.
- Inbound refs: CLAUDE.md:382; SYNCHRONOUS_SEMANTICS.md:14 (calls it "the migration's execution
  record"), :52; FORMAL_SEMANTICS.md:320; FORMAL_SEMANTICS_SURVEY.md:227; DERIVATION_TABLE.md (5×);
  CYCLE_DATAFLOW_SEMANTICS.md (10×); PRETICK_ALIGNMENT_GUARDRAIL.md (5×); review/INVENTORY_DOCS.md:11.
  No code refs.
- Stale claims (as statements about the tree):
  - L21-24: "the `Forwarding` walk's `forward_opening_drives` / `seen_in_read` state … uses
    `edge_value`". `forward_opening_drives`, `seen_in_read` and `edge_value` each have 0 hits.
    `struct Forwarding` still exists (shir_lower.rs:198). `SHIRStmt` drives now carry
    `value`/`next_value` (copper-core/src/shir.rs:154, :165), not `edge_value`.
  - L28-31: `pre_tick_update_forwarding_agrees_end_to_end` and
    `independent_hardware_anchors_the_corrected_spelling` (`ref_fast_counter`): both MISSING, as is
    `fast_counter_corrected`.
  - L44-60: `Cfg::unprotected_trailing_out_write`, `has_nested_tick`, `EXPECTED_TRAILING`,
    `linear_trailing_probe`, `branch_trailing_probe`, `d1_in_the_trailing_segment_is_an_unguarded_gap`,
    `vlir_lower::opening_hold_outputs`, `trailing_update`, `branch_trailing`, `pulse_plain`: all
    MISSING or comment-only. Two comments still call `unprotected_trailing_out_write` live:
    examples/cpu/rv32i_cpu_pipelined.rs:227 ("which `unprotected_trailing_out_write` now rejects",
    a stale code comment) and tests/fixtures/trailing_statements_dut.rs:23.
  - L61-74: `d_narrowing_battery_verdicts`, `ui/pass/pretick_alignment_ok.rs::forwarded_opening_drive`,
    `ui/fail/pretick_alignment.rs::mixed_alignment`, `pretick_alignment_hazard_flags_exactly_…`,
    `EXPECTED_FLAGGED` (in that sense): all MISSING.
  - L127-130: "`SHIRPortDrive` carries `value` (unforwarded) **and** `edge_value` (forwarded)".
    `edge_value` has 0 hits. `vlir_lower::split_output_regs` exists (vlir_lower.rs:1453).
  - L219-220: "the m2 test and `trailing_segment_register_update_agrees` are the regression tests".
    Both MISSING.
  - L226-231: rules listed as "unchanged, with derived justifications"
    (`pretick_out_write_before_update`, `multi_write_collapse`, `multi_phase_out_write`). None has a
    `fn` in the tree. Retirement is recorded at copper-codegen/src/lib.rs:583 and
    copper-macros/src/lib.rs:404.
  - L252-254: "`e8` is refused (`Cfg::trailing_out_reads_input` …)". 0 hits.
  - L273-275: "No executor changes … barrier machinery … untouched". The barrier machinery was later
    deleted by D-8 (copper-sim/src/lib.rs:57: "`PreEdgeBarrier` / `pre_edge_barrier` …").
  - L278-287 / L313-315: `probe_fsm`, `pc_arm_*`, V8a/V8d status. `pc_arm_*` and V8 are MISSING.
    `probe_fsm` is now `RegOut`. "`program_counter: RegOut` as of 2026-09-01" is still true
    (rv32i_cpu_pipelined.rs:230), but the "back-migration to plain `Out`" decision point (L312) is
    now impossible under R8.
- Accurate claims checked (7): `copper-codegen/src/bin/sv-baseline.rs` exists, writes under
  `target/`, and is not run by tools/regression.sh (L7-17). `shir_lower.rs::find_promoted_wires`
  exists (:1330). `split_output_regs` exists. The three derivation-pin files of L266-268 exist, but
  `sequential_forwarding_divergence` was gutted to one test. `rv32i_cpu_pipelined.rs` keeps
  `program_counter: RegOut`. `copper-codegen/tests/phase_sensitive_checks.rs` exists.
- Proposed action: **add a dated header**: "HISTORICAL (pre-SPEC, pre-D-8). The migration recorded
  here (forwarded opening emission, mux + hold, the narrowed pre-tick rules) was superseded by D-8
  and Phase C steps 3–4 (2026-11-06): one resumption per tick, R8, one output form. Normative:
  `SPEC.md`." Also re-label it in CLAUDE.md:382.
  JUDGEMENT: it has the most inbound doc refs of the four (≈25) and no code refs, so moving it to
  `OUTDATED/` costs only doc-link edits. I recommend a header now, and moving it together with
  `DERIVATION_TABLE.md` and `CYCLE_DATAFLOW_SEMANTICS.md` as one batch if that batch is moved. Also
  fix the stale comment at examples/cpu/rv32i_cpu_pipelined.rs:227 (code, out of scope here).

##### design_docs/TIMING_MODEL_UNIFICATION.md (195 lines)
- Classification: **HISTORICAL** (2026-08-25 scoping memo). Its durable finding still holds and
  matches the tree: phases are invisible downstream of `control_extract`, so phase checks live on
  the source, policed by `phase_sensitive_checks.rs`. CLAUDE.md states that finding itself
  ("Any new phase-sensitive rule belongs here…"). Everything else in the memo is pre-D-8 rule
  history.
- Claims normative?: No. L3: "Status: measured; one fix landed … No refactor undertaken … brought up
  to date 2026-09-01".
- Header present?: Status line only. No historical/superseded marker.
- Inbound refs: CLAUDE.md:383; review/INVENTORY_DOCS.md:11. Nothing else, so it is almost an orphan.
- Stale claims:
  - L20-29: the table "agree 58 / disagree 8 / not lowerable 18" and "asserts the disagreeing set
    equals `EXPECTED_DISAGREEMENTS` (the eight modules named below) and only that at least 40
    modules agreed". `EXPECTED_DISAGREEMENTS` (tests/timing_model_derivations.rs:33) now has **47**
    entries. The original 8 are all still in it. The lower bound is `agreed > 40` (:224). The doc
    does say the counts drift, but "the eight modules" is wrong as a description of the pin. The
    same 58/8 figure is also stale in the code comment at copper-codegen/tests/phase_sensitive_checks.rs:7-8.
  - L58-61: "`Cfg::multi_phase_out_write` … `Cfg::check_memory_staging` and
    `Cfg::memory_result_drives_plain_out` … called from `transpile_fir`". Of the three, only
    `check_memory_staging` exists (cfg.rs:505; lib.rs:226). The other two were retired by D-8
    (copper-codegen/src/lib.rs:605; copper-macros/src/lib.rs:405; phase_sensitive_checks.rs:13).
  - L96-110: "`SHIRStmt::PortDrive` carries both `value` and `edge_value` … `vlir_lower::split_output_regs`
    picks". `edge_value` has 0 hits. `split_output_regs` still exists, and per CLAUDE.md it is now
    the one output form.
  - L112-118 and L188-195: "What is actually left is **D1's phase alignment** … What remains
    genuinely open in this family is **D1 itself** … the residue of five rules is listed … in
    `SYNCHRONOUS_SEMANTICS.md` under *Output timing*". D1 and the whole family were retired by D-8.
    `unprotected_pretick_out_write` has no `fn`. The "two jobs" problem was dissolved by one
    resumption per tick, not restricted away.
  - L120-131: "The standing decision (2026-08-25) … No current/next distinction … reject the shape".
    D-8's one-job body is the outcome; whether it is "the same decision" is a judgement call, but
    the decision as phrased ("a more complex executor … is the thing being traded away") predates
    the executor rewrite of 2026-11-06.
  - L133-143: the §5.5 constant-write exemption and
    `pretick_alignment_corpus.rs::pretick_alignment_hazard_flags_exactly_…`. The file was deleted
    (`9e41b61`). `pc_arm_*` and `w4_mixed_alignment` are MISSING.
  - L159-166: `Cfg::unprotected_trailing_out_write`, `has_nested_tick`,
    `trailing_out_write_flags_exactly_the_demonstration_modules`. All MISSING.
  - L72-74: "pinned by `…unsupported_constructs.rs::trailing_combinational_statements_in_a_multi_tick_loop_lower_into_phase_zero`".
    The test exists. Accurate.
- Accurate claims checked (8): `copper_analysis::clock_phase_count` exists (lib.rs:197; cfg.rs:584).
  `tests/timing_model_derivations.rs::the_two_phase_derivations_agree_except_where_extraction_hides_them`
  exists. `no_new_phase_sensitive_check_in_the_transpiler` exists, and its `shir_lower.rs::lower_seq_body`
  entry is present (phase_sensitive_checks.rs:88). `shir_lower::lower_seq_body` exists (:564).
  `tests/fixtures/regout_forwarding_dut.rs` has 4 `#[hardware]` modules, matching "all four
  witnesses". `tests/regout_forwarding_equivalence.rs` exists. `SEED_CYCLES = 200` (build.rs:249).
  `tests/fixtures/trailing_statements_dut.rs` exists. `TODO` exists.
- Proposed action: **add a dated header** ("HISTORICAL, 2026-08-25 scoping memo, pre-D-8; its lasting
  conclusion, that phase-sensitive checks belong on the source, is stated in CLAUDE.md and pinned by
  `copper-codegen/tests/phase_sensitive_checks.rs`"), then **move to `OUTDATED/`**. There is only
  one inbound doc ref besides CLAUDE.md, and no code refs.
  JUDGEMENT: I recommend moving it rather than fixing it in place. Fixing in place would mean
  rewriting §1b, §2 and §4 around a rule family that no longer exists. Separately, the stale 58/8
  count in the code comment at phase_sensitive_checks.rs:7-8 needs a code edit.

##### design_docs/TIMING_COVERAGE_MATRIX.md (216 lines)
- Classification: **STALE**. It is a gate-G1 audit (2026-08-21/25, verified 2026-09-01, addendum
  2026-10-23). Its six-pattern matrix of independent anchors is still mostly true, and the anchoring
  concept is still current (G-E, `build.rs` `REFERENCE`, `ANCHORING_A_MODULE.md`). But pattern 3's
  framing, the register-inference section and the whole addendum are pre-D-8. It is less historical
  than the other three: its main content, which anchors exist, can be checked against the tree.
- Claims normative?: Implicitly. L49-50: "Every check named in the 'anchored by' column exists in
  the tree and does what the cell says (verified 2026-09-01)". L190: "All 6 timing patterns are now
  anchored to independent hardware."
- Header present?: No status header. The intro block says it is "Groundwork for
  `SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md` gate G1".
- Inbound refs: CLAUDE.md:383; TODO:2181; tests/golden_traces.rs:11 (code: "The designs span the
  timing-pattern matrix (see `TIMING_COVERAGE_MATRIX.md`)"); review/MEASUREMENTS.md:123;
  review/INVENTORY_DOCS.md:12.
- Stale claims:
  - L58, L84-87: pattern 3 is "Moore-`RegOut` vs Mealy-`Out` … comb-out: `det_110101`, `det_010`". Both
    now drive `out_o: RegOut<Logic, MainClk>` (examples/sequential/pattern_detector.rs:21-25;
    examples/sequential/pattern_detector_2.rs:18-22). R8 makes a clocked "Mealy-`Out`" axis
    unexpressible. The comb-output axis is now only the combinational examples. The goldens
    themselves are unchanged (`assign out = (cur_state == D)` in pattern_detector_010.sv:32), so
    the row still anchors, but under a different output-timing story.
  - L105-107: "`two_domain_hierarchy_cdc.rs` used (and still uses) a private copy of the body,
    `flag_sync`". It still does (tests/two_domain_hierarchy_cdc.rs:88), but in a `RegOut` one-job
    spelling (:89), so "copy of the body" is no longer literally true. Minor.
  - L122: `synchronizer_behaves_as_two_flops_not_one` is MISSING. It was removed 2026-11-07
    (`6ae104e`, Phase C step 10 part 2), and tests/cdc_synchronizer_anchor.rs:27, :81 still mention
    it in comments. The same file now has `a_one_flop_synchronizer_is_refused_by_both_front_ends`
    (:271), `sync_2ff_latency_is_measured_from_the_rise_not_from_reset` (:162) and
    `source_domain_ticks_do_not_advance_the_synchronizer` (:189).
  - L128-143: "`Cfg::registers` now has a **back-edge clause** alongside the tick clause". This was
    deleted in Phase C step 6. copper-analysis/src/cfg.rs:365-372 says "**No back-edge clause.**
    From 2026-08-21 to Phase C step 6 (2026-11-07) … `sync_2ff_ref.sv`'s second flop is the `RegOut`
    `q`." `register_inference_matches_the_independent_reference` still exists (:295), now passing
    for a different reason.
  - L201-216 (Addendum 2026-10-23): "Its independent golden is the m2 hand-lowered module …
    `m2_model_forwarded_lowering_matches_the_simulator_for_trailing_update` … Phase C made the
    transpiler match it too … (`vlir_lower::opening_hold_outputs`), so `trailing_update`,
    `branch_trailing` … now sweep". The m2 test, `trailing_update`, `branch_trailing` and
    `opening_hold_outputs` are all MISSING (deleted by D-8 / Phase C step 4). review/MEASUREMENTS.md:120-123
    already calls the m2 anchor "Circular as an anchor". `regout_trailing_single_tick` exists
    (tests/fixtures/out_phase_dut.rs). The "seventh pattern" claim no longer has its anchor.
  - L12-13, L35-37: "`branch_merge_explicit` as a one-cycle timing divergence … (`PRETICK_ALIGNMENT_GUARDRAIL.md`
    §5.5)". The history is accurate as history, but the doc offers it as current motivation.
- Accurate claims checked (≈25): `HardwareTest::with_verilog` (copper-sim/src/testing.rs:109) and
  `finish_with_expected` (:239) exist. `with_verilog` is used in examples/basejump/sipo_block.rs,
  bsg_counter_up_down.rs, bsg_dff_en.rs, examples/sequential/pattern_detector.rs and
  examples/memory/dual_port_ram.rs. It is also used in exactly the five named combinational examples
  (mux, priority_encode, one_bit_comparator, ripple_carry_adder, rotate_right). Every golden `.sv`
  named exists: sipo_block.sv, dual_port_ram.sv, sync_2ff_ref.sv, two_domain_hierarchy.sv,
  pattern_detector_010.sv, pattern_detector.sv, tests/fixtures/timing_probe_sv/mac_fsm.sv and ram1.sv.
  These tests exist: `det_010_matches_independent_verilog`, `det_010_awaits_matches_independent_verilog`,
  `independent_verilog_matches_sim`, `transpiled_sync_2ff_matches_sim`,
  `independent_reference_matches_sim_under_verilator`, the four latency/pulse tests,
  `mac_fsm_sim_matches_transpiled_verilog` (uses `with_reference_registers`,
  tests/mac_fsm_equivalence.rs:28) and `mac_pipeline_sim_matches_transpiled_verilog`.
  `probe_registered_timing` and `probe_mem_latency` exist and are `#[ignore]`d diagnostic printouts.
  `accum_2_sim_matches_verilog` exists. `det_010_variants_match_transition_table` exists. One update
  is needed at L152-154: "its `[[example]]` entry … sets no `test = true`" is now FALSE.
  Cargo.toml:78-84 sets `test = true` for `pattern_detector_2`, with a comment saying it was never
  run until 2026-09-01. The `synced_read` retirement is confirmed by comments at
  pattern_detector_2.rs:122 and det_010_independent_golden.rs:202.
- Proposed action: **fix in place**. Rewrite pattern 3 as "RegOut (all clocked outputs, R8) vs
  combinational `Out`", re-pointing `det_110101` / `det_010` to the RegOut side. Replace
  `synchronizer_behaves_as_two_flops_not_one` with the current test names. Mark the back-edge-clause
  section as historical, citing cfg.rs:365. Correct L152-154 (`test = true` is set). Strike the
  2026-10-23 addendum or label it "retired by D-8". Add a dated status line pointing to G-E /
  `ANCHORING_A_MODULE.md` as the standing anchoring mechanism.
  JUDGEMENT:
  - (a) Fix in place, as above. This keeps the six-pattern anchor audit that
    tests/golden_traces.rs:11 cites.
  - (b) Fold the matrix into `ANCHORING_A_MODULE.md` / the G-E report and move this doc to `OUTDATED/`.
  - I recommend (a): the matrix is still accurate in substance, and only about 6 claims are wrong.

---

#### Short summary
- design_docs/DERIVATION_TABLE.md | HISTORICAL | 15 stale | add dated pre-D-8 header (move to OUTDATED/ later) | JUDGEMENT: move vs keep (live refs from SPEC_DECISIONS.md:2590 and 3 code comments); `derivation-audit` bin vestigial (guard column hard-wired empty), dangling `multi_phase_out_write` doc links in cfg.rs:908/927
- design_docs/PAIRED_IMPLEMENTATION_SCOPE.md | HISTORICAL | 11 stale | add dated "superseded by D-8 / Phase C 3–4" header | JUDGEMENT: move with DERIVATION_TABLE/CYCLE_DATAFLOW as a batch; stale code comment examples/cpu/rv32i_cpu_pipelined.rs:227
- design_docs/TIMING_MODEL_UNIFICATION.md | HISTORICAL (near-orphan) | 9 stale | header + move to OUTDATED/ | JUDGEMENT: move (recommended) vs rewrite; stale 58/8 also in phase_sensitive_checks.rs:7-8
- design_docs/TIMING_COVERAGE_MATRIX.md | STALE | 6 stale | fix in place (pattern 3, back-edge, test name, `test = true`, addendum) | JUDGEMENT: fix in place (recommended) vs fold into ANCHORING_A_MODULE

### Group E (E_review_priorart.md)

#### Audit E — design_docs/review/ (except MEASUREMENTS.md), design_docs/prior_art/, OUTDATED/README.md

Tree: c81b59f (worktree docs-audit-332085). Read-only. All six review/ files were added in one commit,
649ea17 (2026-11-06, "spec: Copper's semantics, specified from the coroutine (D-1..D-20)"), and none has
been touched since (`git log -- design_docs/review/<f>`). The baseline commit they cite, ea54d26, exists
(2026-11-03 15:45, "probes: e16i/e16k pin ...").

**Cross-cutting finding for the coordinator (outside my scope, but it changes how several claims below read):**
`design_docs/SPEC.md:3-5` still says "**Status: UNDER CONSTRUCTION (started 2026-11-03). Not yet normative.**
Until this document is adopted, `SYNCHRONOUS_SEMANTICS.md` remains the reference and wins where the two
disagree." and `SYNCHRONOUS_SEMANTICS.md:3-4` calls `CYCLE_DATAFLOW_SEMANTICS.md` "*normative*". CLAUDE.md
also says SYNCHRONOUS_SEMANTICS.md wins. The brief assumes SPEC.md is normative. So the prior_art lines that
call SYNCHRONOUS_SEMANTICS / CYCLE_DATAFLOW normative (spade.md:198, cummings.md:244, verismith.md:227,
arch.md:280) match what the tree says today. They are stale only if SPEC.md's status line is stale. Someone
needs to decide that once; I have not marked those lines stale for that reason alone.

---

#### design_docs/review/

##### design_docs/review/README.md (16 lines)
- Classification: STALE (an index for the directory, so it is meant to be current).
- Claims normative?: No. L15-16: "They are snapshots. They are not updated as the review proceeds; `SPEC.md` and the decision log are."
- Header present?: Yes, dated: L1 "the 2026-11-03 baseline", L3 "taken on 2026-11-03 at commit `ea54d26`".
- Inbound refs: none by filename (`grep -rn 'review/README'` 0 hits). SPEC.md:8 points at the directory: "The baseline the review runs against is `review/` (the 2026-11-03 inventories)".
- Stale claims:
  - L3 "Three read-only inventories": the table (L9-14) has four rows. The fourth, MEASUREMENTS.md, is not an inventory.
  - L13 MEASUREMENTS.md described as "the measurements Phase A owed before the first clause packet: the `probe` fixture, the pre-loop-statement extraction question, and the anchor-debt list": MEASUREMENTS.md is 6067 lines with sections §A-§AF (`grep '^## '`) running to 2026-11-09 (§AF, D-30). It is now the whole Phase C implementation history, and CLAUDE.md cites it as such (§H, §L, §P, §U.5, §X.1, §AA.4 ...).
  - L15-16 "They are snapshots. They are not updated": wrong for MEASUREMENTS.md, which gets a new section with every change (§AF is in c81b59f's parent chain).
  - Missing rows: `DECISION_AUDIT.md` and `Q1_TRADEOFFS.md` are in the directory but not in the table.
  - L7-8 "the conformance ledger in `../SPEC.md` is built from them": overstated. The ledger is keyed on SPEC clauses. `tests/conformance_ledger.rs:7,59` reads the table out of `design_docs/SPEC.md` §III.3 and never opens review/. Only ledger line II.3.5 (SPEC.md:4296) touches an inventory, through INVENTORY_RULES's pointer line. SPEC.md does cite inventory items (INVENTORY_RULES §(1) at SPEC.md:3470, 3624, 3659, 3774; INVENTORY_CODE at :807, :832), so "the clause packets cite them" (L7) is accurate.
- Accurate claims checked: 4. Date and commit ea54d26 exist. INVENTORY_DOCS really has 254 clauses / 28 contradictions in its summary. INVENTORY_CODE says "~200 behaviors ... 36". INVENTORY_RULES has 16 rules / 159 modules.
- Proposed action: fix in place. Say "three inventories (snapshots) plus three living or decision records". Add rows for DECISION_AUDIT.md (the 2026-11-04 audit of D-1..D-7, with its after-D-8 table) and Q1_TRADEOFFS.md (the D-8 decision packet, historical). Describe MEASUREMENTS.md as the dated implementation log §A-§AF that keeps growing, and exempt it from "not updated". Change "the conformance ledger is built from them" to "SPEC II.3.2/II.3.5 cite INVENTORY_RULES §(1)".

##### design_docs/review/INVENTORY_RULES.md (181 lines)
- Classification: HISTORICAL, and labelled as such: a 2026-11-03 snapshot that has a pointer to its successor.
- Claims normative?: No. L3-5: "Read-only inventory ... Taken at commit `ea54d26`."
- Header present?: Yes. Title "2026-11-03 baseline". L7 dated pointer: "**Pointer (2026-11-05, D-13):** the SR/LL column of §(1) is the 2026-11-03 snapshot; its re-cut ... is `SPEC.md` §II.3.2 (D-13 A5), which is the authority." SPEC.md:3833 and ledger line II.3.5 (SPEC.md:4296) require that pointer line, so the file is load-bearing for G-K.
- Inbound refs: SPEC.md (:3470, :3624, :3659, :3774, :3833, :4296), SPEC_DECISIONS.md, review/README.md, review/Q1_TRADEOFFS.md (§5 table "The 16 refusal rules of `INVENTORY_RULES.md` §1").
- Stale-as-of-today (expected for a snapshot; listed so the reader knows how far it has drifted, not to be fixed):
  - Rules 7-15 (L94-102): `memory_result_drives_plain_out`, `multi_write_collapse`, `unprotected_pretick_out_write`, `pretick_out_write_before_update`, `trailing_regout_reads_input`, `trailing_out_reads_input`, `linear_out_write_anchor_mismatch`, `multi_phase_out_write`, `out_first_driven_late`. Each has 0 `fn <name>` definitions in `*.rs`, and the remaining mentions are deletion comments.
  - L13 `classify_reads`: no definition. Callers say "(retired with the barrier, Phase C steps 3-4)" (tests/read_timing_equivalence.rs:3 etc.).
  - L83 "OO = honours `allow_pretick_alignment`": the opt-out is retired (CLAUDE.md; now itself refused).
  - L162 `hold_form_audit.rs`: absent from copper-codegen/tests/.
  - L21 `tests/sequential_forwarding_divergence.rs` still exists. `copper-analysis/tests/pretick_alignment_corpus.rs` (L44, L168, L172) does not.
  - Still present: `check_admissible` (copper-analysis/src/lib.rs:822), `derivation_facts` (lib.rs:164), `clock_phase_count` (lib.rs:197), `check_reachability`, `check_nested_loop_shape`, `check_awaits`, `check_register_initializers`, `check_memory_staging`, `check_definite_assignment`, `infer_registers`, `register_reconciliation.rs`, `phase_sensitive_checks.rs`, `TODO`, `tests/fixtures/probe_timing_dut.rs`.
- Accurate claims checked: 15 identifiers or files. All accurate as of ea54d26 as far as their present or deleted status shows.
- Proposed action: none needed. Optional: extend the L7 pointer with one dated sentence, "rules 7-15 and the opt-out were deleted by D-8 (Phase C steps 3-4, 2026-11-06); `classify_reads` with them". JUDGEMENT: leave it, or add that sentence. I recommend adding it, because the pointer currently talks only about the SR/LL column.

##### design_docs/review/INVENTORY_CODE.md (321 lines)
- Classification: HISTORICAL. It is labelled with a date and a commit, but unlike INVENTORY_RULES it has no pointer saying the implementation has since changed.
- Claims normative?: No. L3-4: "Read-only inventory of every point where the implementation decides meaning, taken at commit `ea54d26`".
- Header present?: Partial. Title "2026-11-03 baseline" plus the commit. There is no "superseded" note, and no line saying the file:line references (EX:813-829, MA:686-729, ...) refer to ea54d26.
- Inbound refs: SPEC.md:807 and :832 ("`review/INVENTORY_CODE.md` unstated 22"), SPEC_DECISIONS.md, review/README.md, review/MEASUREMENTS.md.
- Stale-as-of-today (drift, not errors of the snapshot):
  - §(1) items 2, 8, §(4) whole group, §(9) 3-4: `PreEdgeBarrier`, `pre_edge_barrier`, `POLL_PHASE`. All deleted. copper-sim/src/lib.rs:57-60: "`PreEdgeBarrier` / `pre_edge_barrier` ... and the per-domain `POLL_PHASE` it read were deleted with the two-job executor (Phase C step 3, D-8)". `TICK_RESOLVING` survives (copper-core/src/types.rs:982).
  - §(1) items 1-3, 9-10: tick_clock still calls pre_edge_settle then post_edge_settle (copper-sim/src/executor.rs:851-853), but post-edge now resumes no coroutine (executor.rs:878-883, `poll_set(PollSet::Network)` only). "post-tick code runs after the edge" is gone.
  - §(1) 13 / §(9) 5 / unstated 20 "`exec.cycle` is a single global counter": replaced by per-domain `cycle::<Domain>()` (executor.rs:886-891, "Replaces the former global counter").
  - §(5) 11 `inject_synced_reads`: 0 hits in *.rs. §(5) 1 / 10, unstated 28 `allow_pretick_alignment`: retired.
  - §(10) 10, 18 `spawn_child`: deleted (copper-sim/src/lib.rs:62-63, "deleted in Phase C step 11b ... (SPEC I.13.5, D-23)").
  - §(7) 5, unstated 10, Summary L37 `ReadMode::Async`: `ReadMode` has 0 hits in *.rs.
  - §(11) items 13-21: the plain-Out drive forms. copper-codegen/src/vlir_lower.rs:333-339: "Until Phase C step 4 (2026-11-06) a plain `Out` on a clocked body had five more ... R8 made all of them unreachable".
  - §(9) 8 "One-argument `In<T>` ... domain `()`": D-22 (no default domain).
  - Unstated 29 (pre-loop statements in state 0): fixed in Phase C step 5 (CLAUDE.md, MEASUREMENTS §B/§J).
  - Still present: `spawn_untracked` (executor.rs:457), `wrap_in_module` (copper-macros/src/lib.rs:254), `TickAwaitVisitor` (now copper-analysis/src/front_end.rs:503, moved from MA), `find_unprotectable_in_uses` (front_end.rs:607, moved from MA).
- Accurate claims checked: 16 identifiers spot-checked against the tree, present or deleted as above. The snapshot is internally consistent, and the deletions are dated by code comments.
- Proposed action: add a dated header line in INVENTORY_RULES's style, for example: "Pointer (2026-11-09): file:line references are to ea54d26. The barrier (§4), the two-job phase facts (§1, §9), `spawn_child`, `ReadMode`, and the plain-Out forms of §11 were removed in Phase C steps 3-4 and 11b (D-8, D-23). SPEC.md is where the meaning lives now." JUDGEMENT: header vs. leave as is. I recommend the header, because SPEC.md cites this file by item number and a reader following a citation lands on text that describes deleted code without warning.

##### design_docs/review/INVENTORY_DOCS.md (380 lines)
- Classification: HISTORICAL. Same kind of labelled snapshot, also without a pointer.
- Claims normative?: No. L3-7 "Read-only inventory of every semantic statement in `design_docs/*.md` ... taken at commit `ea54d26`."
- Header present?: Partial (title date plus commit; no superseded note).
- Inbound refs: SPEC_DECISIONS.md, review/README.md. Not cited by SPEC.md (`grep INVENTORY_DOCS design_docs/SPEC.md` 0).
- Stale-as-of-today: the clauses restate the pre-D-8 docs. For example L57 T8 (four-phase tick with "leading reads sample here"), L62 T13 (`PreEdgeBarrier`), L63 T16 ("`pre_edge_barrier` is injected by the macro"), L71 S4 (read-site barrier), §6 RO/§14 RR (the pretick family). These are faithful to the 2026-11-03 docs, which is what the file says it records. Its §(d) "implementing functions" column names deleted functions: `PreEdgeBarrier` and `PollPhase` (0 definitions; `POLL_PHASE` deleted per copper-sim/src/lib.rs:57-60). `set_tick_resolving` and `TICK_RESOLVING` survive.
- Accurate claims checked: summary counts and section headings match (17+22+11+13+19+8+9+11+16+6+9+7+20+46+16+24 = 254). 4 identifiers checked.
- Proposed action: the same one-line dated pointer as INVENTORY_CODE ("records the docs as of ea54d26; their pre-D-8 clauses are superseded by SPEC.md"). Low priority, since nothing normative cites it.

##### design_docs/review/DECISION_AUDIT.md (106 lines)
- Classification: HISTORICAL (a dated audit of D-1..D-7, with an after-D-8 addendum and a D-15 note). One internal inconsistency.
- Claims normative?: No.
- Header present?: Dated title, L1 "Audit of D-1 … D-7 against `SPEC_OVERVIEW.md` (2026-11-04)". There is no "historical" label, but the dated sections make its status clear.
- Inbound refs: SPEC.md:10 ("`review/DECISION_AUDIT.md` sorts D-1…D-7"), SPEC.md:2315 (rows 53 and 97), :2593, :2887; SPEC_OVERVIEW.md:5 ("sorts D-1…D-8"); SPEC_DECISIONS.md; Q1_TRADEOFFS.md:11.
- Stale claims:
  - L103-104 "The two pending accidents are exactly as before." This contradicts the same section's table, L97-98, which marks both **resolved** by D-17 and D-18 (2026-11-05). The rows were updated later and the closing paragraph was not.
  - L64-65 "two remain pending (per-domain defaults, executor re-poll facts)" and L68 "No decision needs reversing today": accurate as of 2026-11-04 (first section), superseded by the L71ff addendum. Acceptable in a dated document.
  - L97 "today's any-domain first settle is a measured divergence, Phase C": the any-domain first settle is gone. Coroutines resume only on their own domain's `pre_edge_settle` (executor.rs:863-871, `PollSet::NetworkAnd(TypeId::of::<Domain>())`).
  - Minor: SPEC.md:10 says it sorts "D-1…D-7" and SPEC_OVERVIEW.md:5 says "D-1…D-8". The file covers D-1..D-7, with D-8's effect in the addendum. Both descriptions are defensible.
- Accurate claims checked: 6. SPEC_OVERVIEW.md exists with §4/§5 ("## 4. The design choices", "## 5. The three principles"). D-17 and D-18 exist and are dated 2026-11-05. D-15 is packet 15, 2026-11-05. `allow_pretick_alignment` is retired.
- Proposed action: fix in place, one sentence. Change L103-104 to "Both pending accidents were later resolved (D-17, D-18, 2026-11-05)". Optionally note at L97 that the any-domain first settle was removed in Phase C step 3. Otherwise keep it as the historical record that SPEC.md cites.

##### design_docs/review/Q1_TRADEOFFS.md (301 lines)
- Classification: HISTORICAL (the D-8 decision packet, 2026-11-04). Its status line is stale.
- Claims normative?: No. It is a tradeoff packet.
- Header present?: L3 "**Status: DECIDED — B, one job (user, 2026-11-04) ... To be recorded in `SPEC_DECISIONS.md` by the next packet; `SPEC.md`/`SPEC_OVERVIEW.md` unchanged here.**"
- Inbound refs: heavy. SPEC.md (:40, :46, :140, :477, :618, :653, :741, :957, :1102, :1568, :1995, :2154, :2166), SPEC_OVERVIEW.md:28, SPEC_DECISIONS.md, MEASUREMENTS.md, copper-ref/README.md:109, tests/fixtures/q1_b_rewrites_dut.rs, tests/fixtures/array_forwarding_dut.rs, tests/fixtures/det_010_dut.rs, examples/sequential/pattern_detector_2.rs. Its §4 numbers are evidence that SPEC.md quotes.
- Stale claims:
  - L3 "To be recorded in `SPEC_DECISIONS.md` by the next packet": it has been recorded, as SPEC_DECISIONS.md:1166 "### D-8 — The one-job body (Q1) (2026-11-04)".
  - §1/§6 present tense "A — two jobs, dynamic boundary (today)" and the file:line refs at L240-242 (`copper-sim/src/lib.rs:101-117`, `executor.rs:813-853`, `copper-macros/src/lib.rs:686-729` `ReadRewriter`, vlir_lower line ranges): they describe ea54d26-era code that B removed (PreEdgeBarrier deleted per copper-sim/src/lib.rs:57-60; `ReadRewriter` 0 hits). That is expected in a packet written before the decision.
  - L7: the temporary files are indeed gone: `tests/q1*` and `tests/fixtures/q1_rewrites_dut.rs` are absent, and `q1_facts`/`q1_scan` have 0 hits. Accurate. (`tests/fixtures/q1_b_rewrites_dut.rs`, added later, exists.)
- Accurate claims checked: 5. D-8 exists. The q1 temp files are absent. q1_b_rewrites_dut.rs exists. SPEC_OVERVIEW §2 "The one choice, decided" exists. prior_art/prost.md §3-§5, §8.3 sections exist.
- Proposed action: fix the status line in place, e.g. "...DECIDED — B (D-8, SPEC_DECISIONS.md, 2026-11-04). Realized in Phase C steps 2-4 (2026-11-06, MEASUREMENTS §F-§H). 'Today' below means ea54d26." Keep the file: SPEC.md cites it as evidence.

**Are the review/ inventories historical snapshots, and do they say so?** Yes, all three are. Each title says "2026-11-03 baseline" and each names commit ea54d26. review/README.md:15-16 says "They are snapshots. They are not updated". Only INVENTORY_RULES carries a dated pointer to what replaced it (L7). INVENTORY_CODE and INVENTORY_DOCS do not, and the README wrongly extends "snapshot" to MEASUREMENTS.md.

---

#### design_docs/prior_art/

**Count:** the directory has **20** files (README.md plus 19 notes), not 17 as briefed: anvil, arch, basejump, bluespec, chisel, clash, cummings, esterel, filament, golden_age, kami, koika, myhdl, prost, pymtl3, rhdl, spade, systemc, verismith.

##### design_docs/prior_art/README.md (31 lines)
- Classification: CURRENT, as an index.
- Claims normative?: No.
- Header present?: L3-4 dates the notes: "written 2026-11-02 for `../FORMAL_SEMANTICS_SURVEY.md`". L30-31 date kami/koika at 2026-11-05.
- Inbound refs: FORMAL_SEMANTICS.md:8, FORMAL_SEMANTICS_SURVEY.md:6, SPEC.md:25 (the `[paper §section]` citation convention), SPEC_DECISIONS.md:3423/3463 ("two rows in `prior_art/README.md`").
- Index completeness: **all 19 notes are indexed** (each filename appears exactly once in the table, L13-31). No orphans.
- Stale claims: none in the README itself. It does not mention that every "Transfer to Copper" section (the ninth question, L7) describes Copper as of 2026-11-02, before D-8. That is the one gap.
- Paths: `paper/references/` is gitignored (`.gitignore:29` `paper/*`), as the README says. Several notes also cite `scratchpad/papers/*.txt` (e.g. clash.md:3, prost.md:3, spade.md:3, chisel.md:3), a session scratchpad that is not in the repo. Harmless provenance, but it does not resolve.
- Proposed action: add one dated sentence: "The 'Transfer to Copper' sections of the 2026-11-02 notes describe Copper before D-8 (2026-11-04): plain `Out` on clocked bodies, the read barrier and Immediate/deferred reads, opening/closing anchors and the pre-tick alignment rule family, all since retired. Read their survey content as current and their Copper statements as historical. SPEC.md is the authority on Copper." JUDGEMENT: (a) that one README sentence (recommended), (b) a header line in each of the 17 affected notes, or (c) rewriting the transfer sections. Do not do (c): SPEC.md cites these notes by section for their content on *other* HDLs, and the Copper-side text is a dated record of what the comparison looked like before D-8.

##### Per-note findings (survey content about other HDLs was not audited, per the brief; only statements about Copper)

Common stale vocabulary in the 2026-11-02 notes (each verified once, cited per line below):
- Pretick/alignment family names (`unprotected_pretick_out_write`, `pretick_out_write_before_update`, `trailing_*_reads_input`, `linear_out_write_anchor_mismatch`, `multi_phase_out_write`, `out_first_driven_late`, `multi_write_collapse`): 0 `fn` definitions in *.rs, deleted by D-8 (CLAUDE.md; vlir_lower.rs:333-339).
- Plain `Out` written in a sequential body / "holds when unwritten" for a plain `Out`: refused by R8 (`copper_analysis::check_no_plain_out` at copper-analysis/src/lib.rs:101, `check_clocked_outputs` :78; `VLIRLowerError::PlainOutOnClockedBody`, vlir_lower.rs:341-343).
- `<port>_hold` / phase-C hold form: removed (vlir_lower.rs:333-339).
- Immediate vs deferred reads, the D2 fix, the barrier: removed (copper-sim/src/lib.rs:57-60; `classify_reads` retired).
- Opening/closing anchors as Copper's model: retired with the second instant (SPEC.md:140, D-8).
- "post-edge settle" as the place post-tick code runs: post_edge_settle resumes no coroutine (executor.rs:878-883). The phrase "pre-edge settle → edge → post-edge settle" is still accurate as executor vocabulary (executor.rs:851-883).

##### design_docs/prior_art/esterel.md (622 lines)
- Classification: HISTORICAL (2026-11-02 note). Survey content current; §9 Copper statements stale.
- Inbound refs: SPEC.md cites `[esterel §3, §9]`, and Q1_TRADEOFFS.md:253.
- Stale: L511-516 (plain `Out` hold; opening/closing drive `S⁺ₒ`/`S⁺꜀`), L519-533 (causality ↔ pretick family; `multi_phase_out_write`, D1, `trailing_out_reads_input`, `linear_out_write_anchor_mismatch`), L536-542 (anchor of a wait-loop test / opening vs closing `In` reads), L559-566 ("the whole D1/D2/multi-phase family exists because an `In` ..."). L511 also cites a private memory note `copper_out_hold_semantics`, which is not in the repo. 4 stale claims.
- Proposed action: covered by the README note.

##### design_docs/prior_art/filament.md (527 lines)
- HISTORICAL. Stale: L406 (`multi_write_collapse`, alignment family), L432-435 (`trailing_regout_reads_input`/`trailing_out_reads_input`), L459-471 (D1/multi-phase; opening vs closing anchor "the entire pre-tick alignment"), L495-498 ("the `_hold` register of phase C is its emission"), L513-527 (§9.3 "Concrete suggestion": an anchor-tagged read model, a design proposal overtaken by D-8). 5. Action: README note.

##### design_docs/prior_art/anvil.md (658 lines)
- HISTORICAL. Inbound: SPEC.md:1511, :1742, :1829 ("`prior_art/anvil.md` item 4"). Stale: L534 (anchor rule), L549 (trailing-drive rules), L564-575 (`multi_phase_out_write`, D1 clause A "mixed anchors"), L600-602 ("Copper's plain `Out` is a *mutable* port written from a [sequential body]"; "solves D1/multi-phase/linear-anchor"), L628 (`multi_write_collapse`), L640-658 (§9.3 suggestion `Drive = (port, value, anchor)`, opening/closing). 6. Action: README note. SPEC.md:1829 cites "item 4" of §9, which concerns memory, not anchors, so that citation is unaffected.

##### design_docs/prior_art/arch.md (301 lines)
- HISTORICAL. Stale: L263-268 ("Copper already has more (reachability, register-by-liveness, alignment rules)"; the opening/closing anchor ambiguity as motivation), L297 ("opening/closing anchoring of an `Out`"). L278-281 ("a design doc denotation (`CYCLE_DATAFLOW_SEMANTICS.md`) the paper-of-record could make normative") is now arguably SPEC.md's role. See the cross-cutting note on normativity. L270-271 (pre-edge settle → edge; levelized order) is accurate. 2-3. Action: README note.

##### design_docs/prior_art/basejump.md (134 lines)
- HISTORICAL, mostly still accurate. L117 (G-E anchoring, ANCHORING_A_MODULE.md): accurate (the file exists). L126-128 "combinational-through handshakes that Copper refuses (Mealy ready)": partly stale. Under D-8 a Mealy `ready` is expressible as a `combinational` module wired to `RegOut`s (Q1_TRADEOFFS §4 (d'); SPEC.md:2154). It is refused only inside a clocked body. L130 "3 of 6 anchored designs match the reference cell count exactly, per memory": sourced to a private memory, not checkable in the tree. 1 stale and 1 unverifiable. Action: README note.

##### design_docs/prior_art/bluespec.md (211 lines)
- HISTORICAL. Inbound: SPEC.md:1511, :1742. Stale: L164-166 ("The pre-tick alignment family is precisely 'a property of one segment'"), L168-176 (D1 forwarded guard `d4a`, `pretick_out_write_before_update`, "the rule family refuses the shapes ..."), L190 ("`Out` hold, or when an `In` is sampled"), L199 ("the pretick family is *about* phases"). 4. L183 (levelized/fixpoint scheduler) and L204-206 (no guard concept; Moore wait loop; option 1 registered handshakes) are accurate. Action: README note.

##### design_docs/prior_art/chisel.md (180 lines)
- HISTORICAL. Inbound: SPEC.md:2166 (`[chisel §2.4]`). Stale: L129-135 ("register-backed plain `Out`"; "phase-C `<port>_hold` register ... applied to a plain `Out`"), L151-156 (`pretick_out_write_before_update`, D1 `d4a`), L159-162 ("`Immediate` vs deferred reads (D2), opening vs closing anchors"), L165-168 ("Copper's sequential-module plain `Out` *holds when unwritten*"). 4. L137-138 (`RegOut` takes effect at the first edge after the write), L170-176 (`control_extract`, `match pc`, phase-sensitive checks upstream) and L180 (levelized SCC iteration) are accurate. Action: README note.

##### design_docs/prior_art/clash.md (210 lines)
- HISTORICAL. Stale: L173-180 (plain `Out` written several times; the pretick family list; "opening and a closing anchor"; "that is the content of CYCLE_DATAFLOW_SEMANTICS.md"), L192-193 (`Immediate` vs deferred reads, D2 fix, anchor rules), L196-199 ("Copper's default plain `Out` *holds when unwritten* in a sequential module"; "Copper phase C `<port>_hold`"). 3. Action: README note.

##### design_docs/prior_art/cummings.md (277 lines)
- HISTORICAL. Inbound: SPEC.md:1511, :1769. Stale: L230 ("Guideline 6 ... = Copper's `multi_write_collapse`"), L234-236 ("Copper's fixed D2 ... now `Immediate`"), L240-245 (`unprotected_pretick_out_write`, `multi_phase_out_write`, `linear_out_write_anchor_mismatch`; "phase C hold-form, drive-level mux"), L259 (opening/closing anchors). L210-213 (pre-edge settle → edge → post-edge settle as a stratified queue) is broadly accurate but describes the post-edge settle as running "re-triggered combinational propagation" only; under D-8 that is now exactly true. L244 "`CYCLE_DATAFLOW_SEMANTICS.md`: 'denotation NORMATIVE'": see the cross-cutting note. 4. Action: README note.

##### design_docs/prior_art/golden_age.md (245 lines)
- HISTORICAL. Stale: L199 ("The 'variable ≠ register' gap class is the pretick alignment family"), L216 ("Copper's plain `Out` ..." as the Mealy carrier), L227-237 ("a plain `Out` written mid-segment ... opening/closing anchor machinery"). 3. L184 (phased ticks), L192 (`check_nested_loop_shape`, `check_awaits` exist in copper-analysis) are accurate. Action: README note.

##### design_docs/prior_art/myhdl.md (373 lines)
- HISTORICAL. Inbound: Q1_TRADEOFFS.md:255. Stale: L295 ("the whole pre-tick alignment ..."), L315-321 ("Copper's plain `Out` written inside a [clocked body]"; "D1/multi_phase/forwarding"; reads anchored to opening vs closing edge), L347-353 (deferred/immediate split; "`out_first_driven_late`, D2 fix"), L360 ("the pre-tick alignment family is the cost of ..."). 4. L331 (combinational `loop { ...; delta_yield().await }`, levelized scheduler) and L345 (only `clk.tick().await` may be awaited) are accurate. Action: README note.

##### design_docs/prior_art/prost.md (242 lines)
- HISTORICAL, and **the most overtaken**: D-8 adopted Prost's model (Q1_TRADEOFFS.md:250, "Prost (LATTE 2026) | **B**"). Inbound: Q1_TRADEOFFS.md:250 (§3-§5, §8.3).
- Stale: L16-18 ("no plain combinational output driven from inside a clocked task, no opening/closing distinction, and no pre-first-edge execution — the three things Copper's pretick/forwarding rules exist for"), L199-201 (`trailing_regout_reads_input`/`trailing_out_reads_input`), L203-206 ("Copper executes the opening segment at time 0 before any edge (pre-tick)"; "eliminates D1 by construction"), L210-216 (§8.3 "where Copper is richer": "Prost has **no plain `Out`**. Copper's plain `Out` inside a sequential body ... is the source of the entire D1/multi_phase/linear-route rule family"). Copper now has no plain `Out` on a clocked body either (R8). L220 (no simulator phase structure). 5.
- Proposed action: README note, plus a one-line dated header in this note: "Since D-8 (2026-11-04) Copper's sequential body is Prost's one-job model; §8.3's 'where Copper is richer' predates that. The differences that remain are listed in Q1_TRADEOFFS.md §8 / SPEC_OVERVIEW.md §2." JUDGEMENT: header here only, or rely on the README note. I recommend a header here, because a reader comparing Copper with Prost for the paper would be misled by §8.3.

##### design_docs/prior_art/pymtl3.md (205 lines)
- HISTORICAL. Stale: L186-189 ("Copper's `Immediate` read at a point in a [segment]"; "the `d*`/`e*` batteries and the anchor rules exist because Copper permits it"), L201 (opening drives). L192 ("Copper has one normative ...") is consistent with the tree; see the cross-cutting note. 2. Action: README note.

##### design_docs/prior_art/rhdl.md (207 lines)
- HISTORICAL. Inbound: SPEC.md:1511. Stale: L179 ("the pretick/forwarding family"), L184-186 ("`In` reads anchored to opening vs closing edges, or about plain `Out` written in several phases"). 2. L196 (`verify_with_verilator`, corpus sweep, BaseJump anchoring) is accurate: `verify_with_verilator` is in copper-sim/src/verification.rs. Action: README note.

##### design_docs/prior_art/spade.md (211 lines)
- HISTORICAL. Inbound: Q1_TRADEOFFS.md:251. Stale: L172-176 (plain-`Out` hold "by a `<port>_hold` register"; "`linear_out_write_anchor_mismatch` restated by form"; "a *typing* presentation of the pretick family"), L195 ("Opening/closing anchors, `Immediate` reads, `trailing_*_reads_input`"), L206-210 ("Copper's plain `Out` is a port written by [a clocked body]"; private memory note "copper_out_hold_semantics"). L197-200 ("Copper's current normative document (SYNCHRONOUS_SEMANTICS.md...)") matches SPEC.md:3-5 today; see the cross-cutting note. 3. Action: README note.

##### design_docs/prior_art/systemc.md (355 lines)
- HISTORICAL. Inbound: Q1_TRADEOFFS.md:254. Stale: L281-282 ("Copper runs the opening segment at time 0 before any edge and lets it drive plain `Out`s combinationally (the D1 problem)"), L296 (`multi_write_collapse`), L317-321 ("Copper's plain `Out` in a sequential body"; "phase C hold form ... `unprotected_pretick_out_write`, `multi_phase_out_write`, `linear_out_write_anchor_mismatch`"), L341-342 ("`CYCLE_DATAFLOW_SEMANTICS.md` two-anchor segment model"), L352 (opening/closing anchors). 5. L277-280 (pre-loop prefix with `check_register_initializers`; constant initializer → SV `initial`, else refused) is accurate (R11; copper-analysis has `check_register_initializers`). Action: README note.

##### design_docs/prior_art/verismith.md (239 lines)
- HISTORICAL. Inbound: SPEC_DECISIONS.md:3423 (`[verismith §3.1]`). Stale: L230 ("opening/closing anchors"; "the pre-tick ..."). L224-228 (CYCLE_DATAFLOW "denotation normative"): see the cross-cutting note. L210 (three-way anchoring sim vs SV vs independent) is accurate, and is now four-way with the copper-ref reference leg (G-F), so slightly incomplete. 1-2. Action: README note.

##### design_docs/prior_art/kami.md (42 lines)
- CURRENT (written 2026-11-05, after D-8 and keyed to SPEC). It cites SPEC I.0.3, I.12.1, III.2, P1 and P8, all present (SPEC.md:3837 "### III.1", :3864 P1 row, :3921 "### III.2"). 0 stale. Action: none.

##### design_docs/prior_art/koika.md (47 lines)
- CURRENT (2026-11-05). It cites P1, P8, I.0.4, D-20 and I.13.2(a) (29 hits in SPEC.md), and "Copper's one-job body (D-8) has one resumption per module per cycle", which is accurate (executor.rs:863-871). 0 stale. Action: none.

---

#### design_docs/OUTDATED/README.md (190 lines) — label check only

- **It does not label OUTDATED/ as historical.** The file is an old copy of the project's top-level README, last touched in aa25fda (2026-07-27, "docs: archive superseded design docs to OUTDATED/, add CLAUDE.md"). It presents itself as current: "Copper is in active development ... (targeting PLDI 2027)", "[TRANSPILATION_ROADMAP.md] - **Current status, progress log, decisions, and notes (canonical)**", "[ASYNC_AWAIT_SEMANTICS.md] - Clock/tick/emit runtime semantics (canonical reference)", and "Copper is organized into several crates" listing 4 crates. Its relative links (`examples/verilog_pitfalls.rs`, `examples/inverter.rs`) resolve neither from OUTDATED/ nor from the root: both files are absent. The only "historical" label for the directory is CLAUDE.md:386 ("`design_docs/OUTDATED/` is historical, not authoritative").
- **It does not list all 31 entries.** Of the 31 entries other than README.md (30 .md files plus `diagrams/`), it links **18**: ASYNC_AWAIT_SEMANTICS, BINARY_TESTING_GUIDE, CHIR_DESIGN, EMISSION_DESIGN, FIR_DESIGN, LATENCY_INSENSITIVE_DESIGN, MEMORY_DESIGN, MODULE_COMPOSITION, RV32I_CPU_DESIGN, SHIR_DESIGN, TEST_RESULTS, TRANSPILATION_COVERAGE_MAP, TRANSPILATION_PLAN, TRANSPILATION_ROADMAP, VALIDATION_DESIGN, VERILATOR_VERIFICATION, VERILOG_OUTPUT_STANDARDS, VLIR_DESIGN. It also links `PROGRESS.md`, which exists neither in OUTDATED/ nor at the root.
  - **Missing (13):** ATOMIC_INSTANT_EXECUTOR.md, AUDIT_2026-08-26.md, CONTROL_EXTRACTION.md, DIAGRAMS.md, EXECUTION_MODEL.md, EXECUTION_MODEL_RECONCILIATION.md, EXECUTOR_CONVENTION_EXPERIMENT.md, HDL_SEMANTICS_COMPARISON.md, LOOP_SEMANTICS.md, REGISTERED_OUTPUTS.md, SYNCHRONOUS_SEMANTICS.md, TRANSPILATION_TODO.md, `diagrams/` (architecture.md, class.md, pipeline.md, simulation, verification.md, workspace.md).
  - Note: `OUTDATED/SYNCHRONOUS_SEMANTICS.md` (a 2026-07-24 "sketch") has the same name as the live `design_docs/SYNCHRONOUS_SEMANTICS.md`, an easy confusion with no index entry to disambiguate.
- Proposed action: JUDGEMENT. (a) Prepend a dated banner: "This directory is historical (archived 2026-07-27 onward); nothing here is authoritative; the semantics is SPEC.md / SYNCHRONOUS_SEMANTICS.md. Below is the project README as of 2026-07-27, kept verbatim." Then add an index of all 31 entries with their archive date. (b) Rename the old README to `OUTDATED/README_2026-07.md` and write a short new index README. I recommend (b): the old text claims "canonical" and "current" status in a directory whose job is to say the opposite.

---

#### Short summary (path | class | #stale | action | judgement?)

- review/README.md | STALE | 5 | fix in place (add DECISION_AUDIT/Q1 rows, re-describe MEASUREMENTS as the living §A-§AF log, drop "ledger built from them") | no
- review/INVENTORY_RULES.md | HISTORICAL (labelled, has pointer) | 0 errors (drift as expected) | optional: extend L7 pointer with the D-8 deletions | JUDGEMENT (recommend extend)
- review/INVENTORY_CODE.md | HISTORICAL (dated, no pointer) | 0 errors (much drift: barrier, spawn_child, ReadMode, plain-Out forms) | add dated pointer header | JUDGEMENT (recommend header; SPEC cites it)
- review/INVENTORY_DOCS.md | HISTORICAL (dated, no pointer) | 0 errors | add dated pointer header (low priority) | minor
- review/DECISION_AUDIT.md | HISTORICAL | 2 (L103 contradicts L97-98; L97 any-domain first settle gone) | one-sentence fix in place | no
- review/Q1_TRADEOFFS.md | HISTORICAL | 1 (L3 "to be recorded", recorded as D-8) | fix status line | no
- prior_art/README.md | CURRENT index | 0 (indexes all 19 notes; dir has 20 files, not 17) | add one sentence: transfer sections are pre-D-8 | JUDGEMENT (README note vs per-file headers; recommend README note)
- prior_art/{esterel,filament,anvil,arch,basejump,bluespec,chisel,clash,cummings,golden_age,myhdl,pymtl3,rhdl,spade,systemc,verismith}.md | HISTORICAL (survey current, "Transfer to Copper" pre-D-8) | 4,5,6,2-3,1(+1 unverifiable),4,4,3,4,3,4,2,2,3,5,1-2 | covered by the README note | no
- prior_art/prost.md | HISTORICAL, most overtaken (D-8 adopted Prost's model) | 5 | README note plus a one-line header on §8.3 | JUDGEMENT (recommend header)
- prior_art/kami.md, koika.md | CURRENT | 0 | none | no
- OUTDATED/README.md | not a historical label (an old 2026-07-27 project README claiming "canonical"); lists 18 of 31 entries, plus a dead PROGRESS.md link | — | replace with a dated index (or banner) | JUDGEMENT (recommend rename the old one and write a new index)
- Cross-cutting: SPEC.md:3-5 still says "Not yet normative ... SYNCHRONOUS_SEMANTICS.md remains the reference", which contradicts the brief's premise. Flag to whoever audits SPEC.md and CLAUDE.md.

### Group F (F_mine.md)

##### README.md (root, 628 lines) — STALE (heavily)
- Claims normative?: L342–343 and L579 point at `SYNCHRONOUS_SEMANTICS.md` as "the full semantics" / "start here"; SPEC.md is named nowhere in README (grep `SPEC` → 0 hits).
- Stale claims:
  - L10: `In<Logic, ()>` / `Out<Logic, ()>` — `()` is not a clock domain (D-22, Phase C step 11); the file itself is now `fn one_bit_comparator<D: ClockDomain>(i0: In<Logic, D>, …)` (examples/combinational/one_bit_comparator.rs:7).
  - L64–121: the `det_110101` example has `out_o: Out<Logic, MainClk>` on a sequential module, refused by R8; the file now has `RegOut` (examples/sequential/pattern_detector.rs:25), so the SV shown (`always_comb out_o = …`) is not what the CLI emits today. L123 claims "the CLI's output verbatim".
  - L43–44, L387–388: "34 of 34 as of 2026-09-01" — `tools/transpile_coverage.sh --quiet` measured 2026-11-09: `transpiler coverage: 40/40 modules`.
  - L185–193: "five wiring guards" G-A..G-E — the driver enforces ten (G-A..G-G, G-I..G-K; G-H retired), tools/regression.sh:36–66. L369 likewise "G-A through G-E".
  - L268–274: "Use `RegOut` for write-before-tick Moore outputs; plain `Out` otherwise. A sequential `Out` holds…" — plain `Out` is combinational-only (R8, D-8).
  - L303: "Five crates" — six workspace members (Cargo.toml:3 adds `copper-ref`); the table omits copper-ref.
  - L309: copper-macros "per-read freshness guards" — the per-read barrier injection was deleted with D-8 (copper-sim/src/lib.rs:57 comment).
  - L328: "A tick has four phases: pre-edge settle → edge → post-edge settle → observation" — today: settle, resume each coroutine once, commit the edge, settle (SPEC I.0.2; CLAUDE.md "One resumption per tick"). Wording to re-check against SPEC I.0.2.
  - L338: `iterate_scc` — 0 hits in code (executor.rs has `tarjan_scc`, `OSCILLATION_THRESHOLD`).
  - L374–381: ignored tests "keep `allow_pretick_alignment` on purpose" — the opt-out is itself refused (copper-macros/tests/ui/fail/retired_pretick_opt_out.rs).
  - L428–487: the "pre-tick alignment family", `trailing_regout_reads_input`, `trailing_out_reads_input`, `linear_out_write_anchor_mismatch`, `out_first_driven_late` listed as live refusals — all deleted by D-8 (CLAUDE.md conventions; 0 hits for `trailing_out_reads_input` in code).
  - L490–492: "`continue` inside a nested loop — refused by the transpiler with its own diagnostic" — since D-23 the `continue` of a tick-bearing `for` is refused by both (front_end rules); other nested `continue` is lowered (tests/fixtures/nested_continue_dut.rs).
  - L495–497: `memory_result_drives_plain_out` — deleted (D-8).
  - L499–501: "Two writes to one array register in a cycle … refused" — lowered since Phase C step 5 (copper-codegen/src/chir_lower.rs:3290–3291).
  - L502: "`while` loops (write `loop { … }`)" refused — a tick-bearing `while` is desugared (`desugar_tick_waits`, chir_lower.rs:5131); only a tick-free `while` is refused (chir_lower.rs:5139).
  - L508–509: "the division operator `/`" is a gap — `CHIRBinOp::Div` lowers (chir_lower.rs:6881, 6904).
  - L518–563: "Pinned divergences", the e8/e9 narrative, `vlir_lower::opening_hold_outputs` (0 hits), and "Copper carries both forms" (edge-sampled vs continuous assign for plain Out) — the five plain-Out forms were removed at Phase C step 4 (MEASUREMENTS §H).
  - L575–591: the Documentation table omits SPEC.md, SPEC_DECISIONS.md, SPEC_OVERVIEW.md, review/MEASUREMENTS.md, copper-ref/README.md; describes PRETICK_ALIGNMENT_GUARDRAIL as "its compile-time rules" (retired).
- Accurate (checked): CLI flags table L234–240 (copper-codegen/src/main.rs), `HardwareTest::with_params`, `tests/rv32i_pipelined_verilator.rs`, `tests/poll_order_fuzz.rs`, `OSCILLATION_THRESHOLD`, `VERILATOR_NOT_INSTALLED`, license section paths, `examples/counter_enable.sv` exists.
- Proposed action: fix in place (largest single edit of phase 2): replace both code examples with today's CLI output, replace the Limitations section with a short current list pointing at SPEC II.1/II.3 and the ledger, rewrite the Documentation table around SPEC.md.

##### copper-ref/README.md (110 lines) — STALE (partly)
- Stale claims:
  - L55–56: "`obs_0` is not recorded by the harness today, a III.2 item" — observation 0 is compared on all three legs since 2026-11-06 (tests/common/relation.rs:9, :238).
  - L61–67: `Deferred` list includes "a plain `Out` on a clocked module" (still a Deferred refusal in copper-ref/src/module.rs:224 — accurate) but its message says "awaits the Phase C rewrite"; README wording ok. `combinational` is no longer deferred (module.rs:127 accepts it) — README already says so in L36; consistent.
  - L70: `/` (R15) listed as Unsupported — the interpreter evaluates `BinOp::Div` (copper-ref/src/interp.rs:1890, 1930).
  - L85: the sim = SV ≠ ref row explained as "a two-job residue" — the two-job executor is gone; text is a dated aside, fine as history but should say so.
  - L91–101: "The constant evaluator" paragraph — compares against `is_constant_expr` and `fold_const_init` and says the interpreter "does not fold an associated const, a constant free function" — D-21 B2 / §N and 2026-11-08 widened grammar (`const_evaluators_agree.rs::the_widened_constant_grammar_agrees_with_rustc`) fold `T::K` and `const fn` calls. Needs re-measuring against copper-ref/src before rewording.
  - L103–110: "The one-job spelling and today's corpus … the two-job executor reads `x_N`" — describes the pre-Phase-C corpus; the two-job executor is deleted (D-8, Phase C step 3).
- Proposed action: fix in place (L55–56, L70, L91–110).

##### copper-ref/src/lib.rs `//!` (L1–41) — STALE
- L30–31: "`combinational` / `structural` modes (all *deferred*…)" — combinational is interpreted (`Module::evaluate`, module.rs:127); structural is `Design` (design.rs).
- L33: "`/`" refused — implemented (interp.rs:1890).
- Proposed action: fix in place (doc comment in a `.rs` → bare `tools/regression.sh` required in phase 2).

##### copper-analysis/src/lib.rs `//!` (L1–~60) — STALE (framing)
- L3–19: written as the G6 feasibility answer ("The open feasibility question was…"), item-2 scope; accurate as to facts (`infer_registers`, `check_reachability`, `cfg`), but describes "THIS module (item 2)" and omits what the crate now holds (`front_end`, `consts`, `data`, `memory_rules`, `check_memory_staging`, parity rules). Not wrong; dated.
- Proposed action: JUDGEMENT — leave (accurate, historical framing) or rewrite as a current crate overview. Recommend a short current overview paragraph on top, keep the rest. `.rs` change → regression.

##### copper-core, copper-macros, copper-codegen, copper-sim `src/lib.rs` — no `//!` crate docs (0 lines each). CURRENT by absence; no action (adding crate docs is new work, not an audit fix).

##### src/lib.rs (root `copper`) `//!` — CURRENT. D-30 doctest and clock doctests checked; `spawn_wired`, `registered_wire`, `dirty_handle` exist.

##### tools/stats/README.md — CURRENT (paper-facing). `SUMMARY.md`/`paper/00_claims_audit.md` are under `paper/` (outside audit; `paper/00_claims_audit.md` does not exist — L56 dangling). M7 numbers dated 2026-09-01 and say "regenerate rather than quote". Action: one-line fix of the dangling path only, if the user wants; it is paper tooling.

##### tools/regression.sh header (L1–66) — CURRENT (G-A..G-K described, G-H retirement recorded).
##### tools/regression-remote.sh header — CURRENT (flags `--setup --ref --sync --host --dir --max-load --force` match).
##### tools/transpile_coverage.sh header — CURRENT (L5 "28/34" is quoted as TODO's historical number).
##### tools/coverage_audit.py, refusal_reachability.py, clean_mermaid.js, stats/*.py, stats/collect.sh, stats/paper_numbers.sh — headers CURRENT as far as checked (no references to retired mechanisms).

##### copper-core/src/memory.md (32 lines) — ORPHAN
- An early brainstorm outline ("Would simulating latency be useful?", "dont think about how many accesses at a time"); referenced by nothing (`grep -rn memory.md copper-core/src` → 0; not `include_str!`d).
- Proposed action: delete (or move to design_docs/OUTDATED/). JUDGEMENT for the user.

##### examples/**/README* — none exist. tests/fixtures/ — no README/notes file; per-fixture `//` headers are code comments (out of scope). tests/fixtures/reference_sv/ — no notes file.

##### CLAUDE.md (548 lines) — STALE (phase 2 only; batch J is editing it)
- L208: "### The five crates" — six (Cargo.toml:3, `copper-ref`); copper-ref is undescribed in the crate list.
- L221: `E_comb`/`E_tick` — the edge kinds are `EdgeKind::Comb` / `EdgeKind::Tick` (copper-analysis/src/cfg.rs:25–48).
- L321: "read `design_docs/SYNCHRONOUS_SEMANTICS.md`" for the semantic core — SPEC.md is normative (batch J's item).
- L373–386: roles list — calls SYNCHRONOUS_SEMANTICS.md normative; omits SPEC.md, SPEC_DECISIONS.md, SPEC_OVERVIEW.md, review/ (MEASUREMENTS), prior_art/, FORMAL_SEMANTICS*.md; lists PRETICK/CYCLE_DATAFLOW/DERIVATION/PAIRED/TIMING_* as live roles (see their rows).
- L525: names `copper-analysis/tests/pretick_alignment_corpus.rs` — deleted (path missing); the lesson stands, the example should say "(since deleted)".
- History-heavy paragraphs to trim to current state (pointing at MEASUREMENTS): L40–49 (the G-F count history 213→388), L409–522 (the front-end parity paragraph, ~110 lines of §U–§AF narrative), L305–313 (the "until Phase C step 4" five-forms history), L248–265 (macro history).
- Not re-measured here: the G-F count "388, 3 skipped" (needs a full run; not measured in this audit).
- Proposed action: fix in place in phase 2, after J merges.

##### Out-of-scope stale code comments found in passing (follow-ups, not fixed here)
- copper-analysis/tests/front_end_parity.rs:21 — "`allow_pretick_alignment` (the opt-out both front-ends honour)" — retired.
- tests/two_domain_hierarchy_cdc.rs:32 — names `copper_analysis::unprotected_pretick_out_write` as live.
- copper-codegen/src/vlir_lower.rs:3349 — "`unprotected_pretick_out_write` refuses the module in both front-ends".

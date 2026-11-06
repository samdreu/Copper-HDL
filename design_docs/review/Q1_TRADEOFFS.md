# Q1 — two jobs or one? The tradeoff packet (2026-11-04)

**Status: DECIDED — B, one job (user, 2026-11-04), after this packet and the question whether B is too close to Prost (answer: the model is shared and must be cited; the realization, the register inference, the written semantics and the measured, anchored sweep are the daylight). To be recorded in `SPEC_DECISIONS.md` by the next packet; `SPEC.md`/`SPEC_OVERVIEW.md` unchanged here.** Every number below
was measured in this packet (the method is named next to it); every verdict was predicted
before it was run and the prediction is printed beside the result. Temporary files (a
`Cfg::q1_facts` method, a corpus-scan test, `tests/fixtures/q1_rewrites_dut.rs`,
`tests/q1_anchor.rs`, `tests/q1_tmp/`) were removed after the runs; the tree is as before.

The question, from `SPEC_OVERVIEW.md` §2: keep the two-job sequential body, make it
one-job as Prost does, or keep two jobs with a static boundary. The audit
(`DECISION_AUDIT.md`) found that D-1's refusal of the one-job body rested on the
passthrough anchor that D-4 later removed, so the refusal has no reason on record and the
question is open.

## 1. The three candidates, precisely

**A — two jobs, dynamic boundary (today).** As `SPEC_OVERVIEW.md` §2 and `SPEC.md`
§I.0–§I.6 state it. After edge N the coroutine's post-edge resumption runs the trailing
region and the head up to the first deferred read at `t_N⁺`, reading `x_N`; it parks at
that read's barrier; before edge N+1 the pre-edge resumption runs the rest of the segment
at `t_{N+1}⁻`, reading `x_{N+1}`. A read site is deferred iff some path from it reaches a
`tick` before the loop's back edge (`classify_reads`), so *which* instant a statement runs
at is dynamic, per execution path. A plain `Out` may be written at either instant; codegen
realizes a drive with one of five forms (F1 assign, F2 every-path comb mux, F3 edge flop,
F4 mux + `<port>_hold`, F5 phase-scoped flop) and refuses, as dated lowering limitations,
the shapes no form realizes (the eight alignment rules of `INVENTORY_RULES.md` §1).

**B — one job (Prost).** The body resumes once per cycle, at the edge. Every read sees the
input at that edge: `x_{N+1}` for the resumption that ends at edge N+1. Every output is a
`RegOut`; a plain `Out` is not a parameter of a sequential module. A combinational output
is a separate `combinational` module wired to `RegOut`s. The barrier disappears; a cycle
has one instant; the trailing region is just the head of the next resumption.
*Not recoverable by an extra flop:* a Mealy output — one that is a combinational function
of the current input — inside the clocked body: a flop of it shows the input one edge
late, so the shape moves to a `combinational` module (§4 (d) measures this).
*Recoverable at the cost of flops and spelling:* a Moore output `o = f(state)` written
after the tick becomes `q.write(f(next_state))` written before it, with the identical trace
(§4 (a)); a "current input" read in the trailing region becomes the value sampled at the
previous edge, stored, or — when the read only feeds this cycle's next state — simply the
same read moved before the tick (§4 (c), identical trace, identical hardware).

**C — two jobs, static boundary.** Both instants stay, and (i) a plain `Out` may be
written only in the comb job — in the trailing region, or in a head segment before any
deferred read on every path — so every plain-`Out` drive is an opening drive (F1 assign or
F4 mux + hold; F3 edge-sampled flop and F5 phase-guarded flop disappear); (ii) the first
deferred read of a segment is at the same program point on every path (every node has a
definite instant: "May-flipped" equals "Must-flipped"). Registers and `RegOut` may be
written in either job.

## 2. Corpus measurement

Method: a temporary `Cfg::q1_facts` walking the analysis graph (`copper-analysis/src/cfg.rs`)
and a temporary `copper-analysis/tests/q1_scan.rs` over every clocked `#[hardware]` module
(`sequential` or `synchronizer`) in `examples/`, `tests/fixtures/` and `src/`, nested
ticking loops included. 135 modules, as the overview counted. Definitions, each the
project's own helper: a **closing write** is a plain-`Out` write node that an `In` read
reaches over `Comb` edges (its own operand included) where that read also reaches a `tick`
(`deferred_read_reaches`); an **immediate read** is an `In` read with no `tick`
comb-reachable from it; **path-dependent** means some node is reached by a deferred read
on one path and not on another (a May/Must dataflow over `Comb` edges from every segment
entry). "Feeds" is path-based (the read comb-reaches a node assigning a register, writing a
`RegOut`, or writing a plain `Out`), an over-approximation.

| | shipped: `examples/` + `src/` (30) | probes: `tests/fixtures/` (105) | all (135) |
|---|---|---|---|
| (1) has a plain `Out` port (every one is written) | **19** | 69 | 88 |
| (2) has a closing plain-`Out` write — refused under C, impossible under B | **9** (4 register-free) | 9 (2 register-free) | 18 |
| (3) plain-`Out` writes only in the comb job — allowed under C; `RegOut` of the next state or a `combinational` module under B | **10** | 60 | 70 |
| (3b) …of which some write is reached by an immediate input read (enabled register / passthrough / trailing Mealy) | 3 | 27 | 30 |
| (4) has a trailing (immediate) read feeding a register / a `RegOut` | **5** / 0 | 13 / 1 | 18 / 1 |
| has any immediate read / any deferred read / both | 6 / 19 / **0** | 34 / 65 / 0 | 40 / 84 / 0 |
| (5) path-dependent first deferred read (every hit is at an effect node) | **1** | 11 | 12 |
| (6) `allow_pretick_alignment` / `build.rs` `SKIP` | 0 / 1 | 3 / 4 | 3 / 5 |
| **refused under C** = (2) ∪ (5) | **10** | 18 | 28 |
| **rewritten under B** = (1) | **19** | 69 | 88 |

No module in the corpus has both an immediate and a deferred read: every module is wholly
"trailing-read" or wholly "deferred-read" today.

**Shipped modules each option touches.**
- *C refuses (closing write, 9):* `sipo_block` (`w0_dbg`…`w3_dbg`), `det_110101`
  (`out_o`), `lfsr` (`o`), `shift_register` (`out`), `simple_counter` (`count_o`), and the
  four register-free passthroughs `event_source`, `event_sink`, `slow_consumer` ×2 (which
  D-4 A2-i already sends to the trailing form). All nine are the same edit: move the write
  after the tick (the `det_010` spelling; §4 (a), (c') measure it). *C refuses (path-dependent,
  1):* `mac_fsm` — its `match stage` arms read `a`/`b`/`c` in one arm only; it has no plain
  `Out`, so only rule (ii) bites, and the fix is a read hoisted above the `match`.
- *C leaves unchanged (20):* `bsg_dff_en`, `bsg_counter_up_down`, `det_010`,
  `traffic_light`, `fast_counter` ×2, `flag_sync`, `dual_port_ram`, `rv32i_cpu_transpilable`,
  `sync_2ff`, and the 10 whose outputs are all `RegOut` (`det_010_awaits`, `det_for`,
  `mac_pipeline`, `serializer`, `handshake`, `simple_handshake`, `rx`, `uart_tx`, `uart_rx`,
  `rv32i_cpu_pipelined`).
- *B rewrites (19):* every module in (1): the nine above plus `bsg_dff_en`,
  `bsg_counter_up_down`, `det_010`, `traffic_light`, `fast_counter` ×2, `flag_sync`,
  `dual_port_ram`, `rv32i_cpu_transpilable` (3 plain `Out`s), `sync_2ff`. The edit is the
  port kind plus moving the tick to the end of the loop; the measured cases are 2–8 body
  lines each (§4).
- *Probe fixtures C refuses (18):* `branch_merge`, `branch_merge_explicit`, `lfsr`,
  `det_110101`, `plain_out_multiphase`, `conditional_plain_out`, `shift_register`,
  `vr_sink_mealy`, `vr_skid` (closing); `branch_on_updated_reg`, `branch_on_updated_reg_bit`,
  `if_tick_explicit`, `match_tick_explicit`, `ex_regfile`, `ex_reg_then_wire`, `mac_fsm`,
  `match_on_updated_reg`, `probe_fsm` (path-dependent). The 62 refused probes of
  `tests/sequential_forwarding_divergence.rs` are outside the corpus directories and were
  not scanned; under B all of them are unexpressible (each has a plain `Out`), under C those
  whose refusal needs a closing drive are (§5).

## 3. What the two jobs buy, re-examined

Under the harness convention — inputs driven for cycle N before edge N, outputs observed
after it — a Moore output written after the tick (`tick; o.write(f(state))`) and a `RegOut`
of the next state written before it (`q.write(f(next)); tick`) have the same trace: both
show `f(state_N)` at `obs_N`. §4 (a) measures this against the independent goldens. The
same holds for the trailing read into a register (§4 (c)) and the enabled register (§4 (b)).
So the harness trace does **not** separate A from B for Moore outputs, enabled registers or
counters; what separates them is the hardware (a flop per output bit under B, unless the
output is a `combinational` module of the exposed state) and the spelling.

The trace **does** separate them in composition. A's trailing plain-`Out` drive runs once,
at `t_N⁺`, and is not re-run when the input changes before edge N+1; the F1 `assign` the
transpiler emits for it follows the input. §4 (e) measures a two-module composition (a
trailing passthrough feeding a register) in the simulator against the same two emitted
modules under Verilator. This is the one measured difference where A's own two sides
disagree and B has no such shape.

## 4. The rewrites, measured

Each rewrite was run (i) through the corpus sweep as a temporary fixture (simulator vs its
own transpiled SV, five stimulus sequences) and (ii) against its independent anchor with the
anchor's own port names (`HardwareTest::with_verilog`, 200 cycles of LCG stimulus). Cell
counts are Yosys `synth` to generic cells, the method of `tools/stats/qor.py`, run on the
emitted SV and on the anchor; FF = flip-flop cells. "Lines" = body lines changed, comments
stripped (`diff`, added/removed).

| rewrite | anchor | predicted | sweep sim≡SV | vs anchor | cells/FF: anchor · A · rewrite | lines |
|---|---|---|---|---|---|---|
| (a) `det_010` → B (`RegOut` of next state) | `pattern_detector_010.sv` | identical | PASS | **identical** (200 cycles) | 14/4 · 9/2 · **10/3** | 3/3 |
| (a') `det_110101` → B | `pattern_detector.sv` | identical | PASS | differs at cycles 15, 271 — **so does the shipped spelling** (control below): the golden, not the rewrite | 23/3 · 24/3 · **24/4** | 2/2 |
| (a'') `det_110101` → C (write after the tick) | `pattern_detector.sv` | identical | PASS | same two cycles as the control | 23/3 · 24/3 · **24/3** | 2/2 |
| (b) `bsg_dff_en` → B | BaseJump `bsg_dff_en.sv` | identical | PASS | **identical** | 8/8 · 8/8 · **8/8** | 2/2 |
| (b') `bsg_dff_en` → C | — | unchanged source | (= A) | (= A, shipped) | 8/8 | 0 |
| (c) `bsg_counter_up_down` → B | BaseJump `bsg_counter_up_down.sv` | identical | PASS | **identical** | 15/3 · 16/3 · **16/3** | 4/12 |
| (c') `bsg_counter_up_down` → C | — | unchanged source | (= A) | (= A, shipped) | 16/3 | 0 |
| (c'') `sipo_block` → B (`wN_dbg` as `RegOut`) | BaseJump `sipo_block.sv` (`data_o`) | identical | PASS | **identical** | 36/30 · 37/30 · **37/30** | 8/8 |
| (c''') `sipo_block` → C (dbg writes after the ticks; `w3` carried across the back edge) | BaseJump `sipo_block.sv` (`data_o`) | identical | **FAIL** (`data_o`, from cycle 3) — a transpiler bug, see below; the variant `q1_c_sipo_block2` PASSES | **identical** (the sim side; `q1_c_sipo_block2` also identical) | 36/30 · 37/30 · **41/34**; `q1_c_sipo_block2` **37/30** | 7/6 |
| (d) `vr_sink_mealy` → B (`RegOut` of next `full` and `drain`) | the refused module's own emitted SV (`assign ready`) | identical trace | PASS | **identical** (`ready` and `sum`) | — · 54/17 · **57/18** | 8/3 |
| (d') `vr_sink_mealy` → B with a boundary (`q1_b_vr_sink_core` + comb `q1_b_vr_ready`) | — | both compile and sweep | PASS (both) | n/a (composition) | — · 54/17 · **54/17** | 8/3 + 7 |
| (d'') `vr_sink_mealy` → C, `ready` written in the trailing region **after** `full`'s update | the refused module's emitted SV | admitted; identical (revised before the run: `sum`, a `RegOut` written in the trailing region, one late) | PASS | `ready` identical; `sum` one cycle late (re-predicted before the run, see note) — the faithful spelling `q1_c_vr_sink_after2` (`sum` as plain `Out`) is **identical** on both ports | — · 54/17 · 62/25 (`sum` a delayed copy); `q1_c_vr_sink_after2` **54/17** | 7/3 |
| (d''') `vr_sink_mealy` → C, `ready` written **before** `full`'s update | — | REFUSED by `pretick_out_write_before_update` (trailing clause) | refused as predicted (CLI) | — | — | 7/3 |
| control: `det_110101` as shipped (`q1_a_det_110101`) | `pattern_detector.sv` | run after (a') failed; predicted: fails at the same cycles 15, 271 | PASS | fails at cycles 15 and 271, as predicted | 24/3 | 0 |

Readings:
- **The Moore output costs B one flop per output bit** (`det_010` 9/2 → 10/3, `det_110101`
  24/3 → 24/4): Yosys does not fold a flop of `f(next_state)` back into the state register.
  Under C the output stays an `assign` of the state (`det_110101` → C is cell-identical to A).
- **The enabled register and the counter cost B nothing**: the emitted SV is the same
  `always_ff` as A's, because A already realized those trailing drives as the register
  itself. The paper's two exact cell-count matches among clocked designs (`bsg_dff_en`,
  `bsg_counter_up_down`) survive B and C unchanged; `sipo_block` (37 vs 36) is unchanged
  under B — Yosys merges the `wN_dbg` flops with the `wN` registers — and unchanged under C in
  the faithful spelling (`q1_c_sipo_block2`, 37/30: the carried `w3` register merges with
  `data_o`'s top nibble); the first C spelling's 41/34 is the divergent one.
- **The Mealy handshake is expressible under A and C** with `ready` written after the
  register it reads is updated (d''): the rules admit it and the emitted form is the same
  `assign ready = !full || drain` the refused module produced. `vr_sink_mealy`'s refusal is a
  refusal of a *spelling* (write before the update), not of the shape. Under B the shape
  needs a module boundary (d'), at no cell cost.
- **Lines changed** are 2–8 per module for B and 0–7 for C; the C rewrites of
  `bsg_dff_en` and `bsg_counter_up_down` are empty.

**(e) Composition.** `q1_a_pass` = `loop { tick; o.write(x.read()) }` (A's trailing
passthrough, emitted as `assign o = x`), `q1_a_pass_en` = the same under `if en` (emitted as
a flop), `q1_b_pass` = `loop { o.write(x.read()); tick }` with `o: RegOut`; each feeds
`q1_sink` = `loop { let v = i.read(); r.write(v); tick }` (`r: RegOut`). Simulator (the two
coroutines hand-wired, as every multi-module example is) against a hand-written SV wrapper
instantiating the two emitted modules, 40 cycles, `x` distinct each cycle.

| composition | predicted | result |
|---|---|---|
| A passthrough (assign) → sink | **diverges from cycle 0**: sim `r_N = x_{N-1}` (the trailing drive ran at `t_N⁺` and is not re-run at `t_{N+1}⁻`), hardware `r_N = x_N` | **diverges from cycle 0** as predicted: `r` expected (sim) 0, got (hardware) 1; then 1 vs 2, 2 vs 3, … — the simulator lags the emitted hardware by one cycle on every cycle |
| A enabled register (flop) → sink | agrees (the flop samples `x_N` at edge N, which is what the `t_N⁺` read saw) | **agrees** as predicted (40 cycles) |
| B registered passthrough → sink | agrees (`r_{N+1} = x_N` on both sides) | **agrees** as predicted (40 cycles) |

Three readings. (1) **A's trailing F1 `assign` of an input is not the simulator's meaning in
composition.** The sweep passes `q1_a_pass` (PASS above) because the harness holds the input
across the edge; wire the same module to a consumer and the simulator shows the value the
trailing drive sampled at `t_N⁺` while the hardware wire follows the input to the next edge.
This is exactly SPEC §I.5's meaning (the wire keeps the written value until the next write),
so the transpiler's F1 form is wrong for this shape, no rule refuses it on the single-tick
route (`trailing_out_reads_input` exempts it), and the shipped `event_source`/`event_sink`/
`slow_consumer` passthroughs have the same emitted form. The flop form (A enabled register)
agrees, because a flop sampling `x_N` at edge N is what the `t_N⁺` read saw. (2) Under B the
shape does not exist: a comb path through a clocked body is impossible, and `combinational`
modules are re-evaluated at every settle. (3) Under C the shape is legal (a trailing drive is
the comb job) and would need one more rule or a forced flop form — the single-tick extension
of `trailing_out_reads_input` — to close.

**Two findings beside the question.** (i) `pattern_detector.sv`'s golden and the shipped
`det_110101` disagree at a 1 after `S110101` (the golden's "Bug 2" arm goes to `S1`, the
Rust `_ => IDLE`); the example's hand-listed stimulus never reaches it, the 300-cycle LCG
stream does at cycles 15 and 271, and the shipped spelling (`q1_a_det_110101`, the control)
fails at the same two cycles. Not a Q1 matter; owed to whoever owns the example.
(ii) `q1_c_sipo_block` is **admitted and diverges**: `w3`, a register assigned in the last
segment and read in the same segment through `bits[12+k] = w3[k]`, is read as its committed
`Q` in the emitted `always_comb` (`bits[(32'd12 + k)] = w3[k]` beside `w3 <= data_i`) while the
simulator forwards the new value; `data_o` shows the previous block's `w3` from cycle 3 on.
Reading the fresh `let` instead (`q1_c_sipo_block2`) agrees. The shape is legal under A
today (nothing in it is C-specific), so this is a live forwarding gap in the transpiler for a
register read through an array-element assignment, found by the rewrite.

## 5. Rules under each option

The 16 refusal rules of `INVENTORY_RULES.md` §1, by what each needs.

| rule | A (today) | B | C |
|---|---|---|---|
| `check_admissible`, `check_reachability`, `check_nested_loop_shape`, `check_awaits`, `check_register_initializers`, `check_memory_staging`, `check_definite_assignment` (comb) | shape / design rules | unchanged | unchanged |
| `multi_write_collapse` (the dead closing write, D-5 A2-i) | semantic (I.5.5) | gone: no plain `Out` | stays as the dead-write rule, now reachable only through the comb job's two writes with a tick between |
| `memory_result_drives_plain_out` | lowering | gone | stays (comb-job drive from a memory result) |
| `unprotected_pretick_out_write` (D1) clause A, mixed anchors | lowering | gone | **gone** — needs a closing drive |
| D1 clause B, forwarded guard (`d4a`) | lowering | gone | stays — an opening drive under a guard on a register assigned earlier |
| `pretick_out_write_before_update` (V8): pre-tick and guard editions | lowering | gone | **gone** — need a read-reached write |
| V8: trailing and single-tick trailing-update clauses | lowering | gone | stays — the comb job reads a register the same resumption updates; the linear fold realizes trailing updates at the opening edge (I.6.9), so the emitted `assign` shows the new value where the simulator showed the old (d''' measured) |
| `trailing_regout_reads_input` | lowering (fix: carrier flop) | gone: the read moves before the tick | stays |
| `trailing_out_reads_input` | lowering | gone | stays |
| `multi_phase_out_write` (all clauses) | lowering | gone | **gone** — every clause needs a read-reached write or a register-free read before an opening write |
| `linear_out_write_anchor_mismatch` | lowering | gone | clauses A (two drivers), B (conditional trailing), 2 (unconditional wire), 3a (direct input operand) stay; 3b and the deferred-read halves of C **go** |
| `out_first_driven_late` | lowering | gone | stays |
| C's own: "plain `Out` written after a deferred read"; "path-dependent first deferred read" | — | — | **two new semantic rules** (28 corpus hits, §2) |

Count of rules that reason about alignment: A **8** (all lowering limitations); B **0**;
C **5 survive + 2 new** = 7, of which the two new ones are semantic and the five are the
comb-job limitations of the forms that remain. C therefore removes three rules (D1-A,
multi-phase, the closing half of the linear rule) and the two forms F3/F5, not the family.
B removes the family and the forms.

## 6. Implementation change

| | A | B | C |
|---|---|---|---|
| simulator | none | `PreEdgeBarrier` and the pre-edge resumption go (`copper-sim/src/lib.rs:101-117`, `executor.rs:813-853`: a tick becomes edge → resume-to-next-tick); plain `Out` wiring for sequential modules removed | none |
| macro | none | `ReadRewriter` (`copper-macros/src/lib.rs:686-729`) no longer injects barriers; signature check refuses `Out` in sequential mode; `classify_reads` becomes dead | `classify_reads` stays; two new checks (closing write, path-dependent instant), both on `Cfg` |
| transpiler | none | forms F1–F5 for sequential plain `Out` go (`vlir_lower.rs` 738–741, 369–372, 519–541, 1579–1600, 1530–1566 per `packet-5` notes); every port is the `split_output_regs` flop; `shir_lower`'s opening/closing tagging (335–386) goes | F3 and F5 go; F1/F2/F4 stay; the opening walk stays |
| spec | — | D-1 A (instants collapse to one), D-4 A1/A2-i (barrier placement), D-5 A1 (hold is the `RegOut` hold, D-6) are re-derived; II.3's alignment list empties; D-2, D-3 (segments stay, instants merge), D-6, D-7 (one suspension kind) stand with vocabulary edits | D-4 gains the static-boundary clause; D-5 loses F3/F5 from A4; II.3 loses three rows |
| sweep / anchors | — | 19 shipped + 69 probe modules rewritten; the probe battery's 62 refused modules become unexpressible, so the measured-divergence evidence base shrinks to what `RegOut` can express | 10 shipped + 18 probe modules rewritten or hoisted |

## 7. Prior art

| system | where it sits | note |
|---|---|---|
| Prost (LATTE 2026) | **B** | all outputs `.next`-written registers visible "after the next wait", inputs read in the edge-evaluated advance function, no comb output in a task, opening segment runs at the first edge (`prior_art/prost.md` §3–§5, §8.3) |
| Spade | equational: one comb output expression of registers, no point-of-read | `reg` + body-as-expression; an output is a function of registers, which is A/C's trailing Moore drive and B's `combinational` module; no imperative sequencing (`spade.md` §3, §9) |
| Chisel, CλaSH, Bluespec | Mealy step function `f : (S, I) → (S, O)` evaluated once per cycle; comb outputs allowed, no point-of-read | one value per signal per cycle, all reads of the cycle's settled value (`chisel.md` §3, §9; `clash.md` §3, §9; `bluespec.md` §3) — a one-instant model *with* combinational outputs, i.e. neither A nor B |
| Esterel | one instant per reaction, outputs synchronous with inputs (Mealy) | the synchrony hypothesis; no registered-output restriction; the barrier has no analogue (`esterel.md` §3, §9) |
| SystemC synthesizable subset | thread `wait()` = edge; `sc_out.write()` commits at the delta, visible before the next edge — A's plain-`Out` hazard, left unspecified; HLS tools register thread outputs in practice (B) | (`systemc.md` §3.1, §8.2) |
| MyHDL | `always_seq`: `.next` writes commit after the body, reads see committed values — B at the boundary; general generators may `yield` anywhere | (`myhdl.md` §3.4, §5) |

No surveyed system has A's dynamic two-instant body. C (static boundary) has no direct
precedent either; the nearest is SystemC's "first `wait` statically placed" shape rule.

## 8. Paper impact (flagged only; the paper is the user's)

The two-job body is the daylight from Prost: under B the kernel claim ("a coroutine *is*
the circuit; every output a register") is Prost's §2 sentence with liveness-based register
inference and the measured sweep as the delta. Under A or C the delta also includes the
plain-`Out` model and its rules. The evaluation's cell-count rows for clocked anchors
(`bsg_dff_en`, `bsg_counter_up_down`, `sipo_block`) are unchanged under B and under C.

## 9. Summary table

| row | A — two jobs, dynamic | B — one job | C — two jobs, static |
|---|---|---|---|
| Mealy output in the clocked body | yes, as a trailing drive after the update (d''); the F1 form diverges in composition (e) | no; `combinational` module + exposed `RegOut` (d') | as A |
| Moore output from committed state | `assign` of the state, no flop | `RegOut` of the next state, +1 flop per output bit (a, a') | as A |
| current-input sampling into state | trailing read (c) | the same read before the tick, identical trace and SV (c) | as A |
| enabled register / held output | plain `Out` hold (b) | `RegOut` hold, identical SV (b) | as A |
| multi-cycle algorithm | yes | yes (Prost's whole point) | yes; the path-dependent instant is refused (`mac_fsm` hoists a read) |
| output flops vs the six anchored references | 3 of 6 exact today | the clocked three unchanged (measured) | the clocked three unchanged (measured) |
| alignment rules | 8, lowering limitations | 0 | 5 lowering + 2 semantic |
| simulator change | — | barrier and pre-edge resumption go | none |
| macro change | — | barrier injection goes; `Out` refused in sequential mode | two checks added |
| transpiler change | — | five plain-`Out` forms go | two forms go |
| spec | D-1…D-7 as recorded | D-1 A, D-4 A1/A2-i, D-5 A1 re-derived | D-4, D-5 amended |
| corpus cost, shipped | — | 19 rewritten (2–8 lines each) | 10 rewritten (0–7 lines each) |
| corpus cost, probes | — | 69 rewritten; 62 refused divergence witnesses unexpressible | 18 rewritten or hoisted |
| prior art | none | Prost, MyHDL `always_seq`, HLS practice | none |
| open hazards measured here | (e): the single-tick trailing `assign` of an input is not re-run in composition, no rule covers it; (ii) the array-element forwarding bug | (e) cannot arise; (ii) is a register-forwarding bug independent of the port kind, so it stays | as A: (e) needs a sixth rule, (ii) stays |

## 10. Recommendation

**B.** Every shipped rewrite measured trace-identical to its independent anchor at zero cell
cost on the anchored designs, while the one hazard this packet measured (the trailing
combinational drive sampled once, wrong in composition) and all eight alignment rules exist
only because a clocked body may drive a wire — and a Mealy output, the one thing B cannot
spell inside the body, is already what the project decided against for handshakes
(2026-10-21) and is expressible as a `combinational` module at no cost (d'). The price is
one flop per Moore output bit written inside a body (or the module boundary), the 19 shipped
and 69 probe rewrites, and a smaller daylight from Prost in the paper, which is the user's
call. C is the alternative if the Moore `assign` form and the two-instant model are worth
keeping: it halves the corpus cost and keeps three fewer rules, but keeps five, adds two,
needs a sixth to close the composition hazard, and its first rewrite found a forwarding bug
the single-module sweep had not.

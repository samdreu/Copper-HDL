# Decision log for `SPEC.md`

One entry per clause, in the order decided. An entry is never deleted; a reversal is a
new entry that names the one it reverses, and the old entry gains a "reversed by" line.
The baseline every entry cites is `review/` (the 2026-11-03 inventories: DOCS = clause
inventory of the design documents, CODE = implementation-defined behaviors, RULES =
rules, probes and open items).

## Entry template

```
### D-<n> — <clause title>                                  (<date>)
Clause: SPEC §<ref>
Question: <the semantic question in one sentence>
Docs say: <DOCS items; contradictions if any>
Code does: <CODE items, file:line>
Anchor: <independent hardware that fixes the answer, or "none — decided">
Prior work: <[paper §section] …, or "no precedent">
Options:
  A. <option> — changes: <sim / macro / codegen / refuse / none>; corpus cost: <measured or n/a>
  B. …
Recommendation: <A/B/…, one sentence why>
Decision: <A/B/… as the user decided; rationale in the user's words where given>
Changes: <which implementation changes, or none>
Obligations: <tests to add, pins to re-derive, measurements owed>
```

## Entries

### D-1 — Time, instants and the environment                      (2026-11-03)
Clause: SPEC §I.1
Question: Within a clock domain, how is time structured, at which instants does a
module's code execute, when are outputs observed, and what may the environment do to
inputs and when?
Docs say: DOCS T3–T9 (edges from 1; cycle N = [edge N, edge N+1); observation after the
post-edge settle; x_N driven before edge N; four tick phases; tick resolves post-edge).
Contradiction 5 (edges numbered from 1 vs from 0). Gap 22 (mid-cycle input changes
undefined).
Code does: CODE (1).1–16 — pre-edge settle, `advance` with listeners, post-edge settle,
global `exec.cycle`; a tick resolves only post-edge; a deferred read parks post-edge and
resumes pre-edge; futures are first polled in the pre-edge settle of edge 1, so pre-loop
statements and the first segment run before any edge and a deferred read there does not
park; `poll_tasks` may be called outside a tick; inputs may be written at any moment.
Anchor: the drive–tick–observe convention of every hardware-anchored check
(MEASUREMENTS §C) fixes the sampling instants for inputs changed between an observation
and the next tick. Nothing anchors a mid-cycle change or the first instant (no harness
observes before edge 1).
Prior work: Esterel one instant per reaction [esterel §3.3, §6.3]; Verilog stratified
event queue — RHS evaluated from pre-edge values, NBA update after [cummings §4.0, §6.0];
SystemC threads resume after the edge in the next evaluation phase [systemc §4.2.2];
Prost advances at posedge to the next wait, reads at the edge [prost §2–3]; Clash and
Anvil one value per cycle, no sub-cycle structure [clash eq. 11; anvil C.2]; Arch's
"register commit barrier" and "DAG level barrier" [arch §7.3.4]. No language has two
execution instants inside a cycle for one sequential body; the event queue is the only
precedent and is not a language semantics.
Options:
  A. Two instants per cycle (post-edge of N, pre-edge of N+1), observation at the end of
     the post-edge instant, environment = drive/tick/observe with drives only between an
     observation and the next tick, edges from 1, cycle 0 with the single instant t_1⁻ —
     changes: none to sim or codegen; convention becomes a precondition of III.2;
     polling outside a tick and mid-cycle drives become outside the language.
     A1. Include obs_0 (after the first pre-edge settle) in the trace — changes: harness
         observes before the first tick (Phase C); startup rules re-derived in I.9.
     A2. Per-domain edge numbering; the global counter is an artifact — changes: executor
         counter per domain or renamed (Round 3 / Phase C).
  B. One instant per cycle (Esterel/Prost): every plain output registered — refuted by the
     hardware-anchored passthrough (D2); would remove or re-mean plain `Out`.
  C. Continuous time with events (Verilog) — more general than anything Copper emits.
Recommendation: A with A1 and A2 — it is what the anchored patterns measured, the only
option no anchor refutes, and costs no code change now.
Decision: **A, with A1 (include obs_0) and A2 (per-domain counting).** User, 2026-11-03.
Changes: none now. Phase C: harness observes obs_0; executor cycle counter per domain;
refusal of out-of-tick polling and mid-cycle drives to be decided when I.13/III.2 are.
Obligations: I.9 must give obs_0 and the first segment a meaning (and restate
`out_first_driven_late` from it); III.2 must state the environment precondition;
`late_constant_one_edge` and the `out_first_driven_late` witnesses to be re-measured
with obs_0 observed.
Coroutine terms (D-7): the two instants of cycle N are its two resumptions — `t_N⁺` the
post-edge resumption of edge N, `t_{N+1}⁻` the pre-edge resumption of edge N+1; cycle 0's
one instant is the first resumption, from the entry; `obs_N` is read after the post-edge
resumption (→ I.0.2, I.0.3).
Reversed by D-8 (2026-11-04): A's two instants per cycle — one instant per cycle, `t_{N+1}⁻`,
the one resumption that ends at edge N+1, reading `x_{N+1}`; the drive–tick–observe
environment, A1 (`obs_0` in the trace) and A2 (per-domain numbering) stand; option B, refused
here on the passthrough anchor that D-4 A2-i later removed, is the model D-8 adopts.

### D-2 — The kernel language                                      (2026-11-04)
Clause: SPEC §I.2
Question: What object does the specification give meaning to, and what is its syntax —
the desugared surface Rust both front-ends hold (`syn::ItemFn`), with register inference
and read classification as defined functions on it, or the post-inference,
post-extraction IR with the lowering proven meaning-preserving — and, for every
construct the macro accepts, is it in the kernel, desugared into it, outside it, or not
yet given a meaning?
Docs say: DOCS T1 (the simulator runs plain Rust and never interprets an IR; the c2
decision of 2026-07-29), T2 (an IR interpreter only as a validation backend), T16 (the
only await is `clk.tick().await`), T11 (every tick is a cycle boundary), REG1–REG5 (a
register is a local defined in the loop and live across a tick; the minimal set, not
rustc's layout), REG7 and A3 (one CFG, both front-ends consume it), SC10
(statement-granular CFG), SC13–SC15 (a tick-bearing nested loop is a repeating wait; a
tick-free one unrolls), SC17 and SC19 (control extraction is codegen's FSM; phase rules
stay on the source), IN8 (test-before-tick is the only wait order), PO2 (the
constant-initializer grammar), CM1 (one instant per combinational evaluation), H1–H2
(structural = wiring only), RR2–RR4 (the shape rules), RR43 (a module that compiles also
transpiles). No document states a grammar (survey §0). Gap 27 (constructs accepted with
no stated meaning: `break`/`continue` beyond the CFG, word-indexed array registers,
`wire(init)` in structural parents, `delta_yield`, host-side Rust, file-scope `const`);
gaps 17 (zero-iteration tick-bearing loop) and 18 (a pre-loop `Out` write).
Code does: CODE (5).1–(5).4 (mode is the first attribute token, one flag; no return
type; params typed `Clock`/`In`/`Out`/`RegOut`/`Memory`; at least one output; every
clock ticked; exactly one top-level `loop`, last); (5).6–(5).7 (clock construction,
`Option`, `?`, the panic family refused); (5).14–(5).16 (the combinational wrapper,
`HardwareModule` wrapping, structural emitted unchanged); (5).17–(5).18 (synchronizer =
sequential minus CDC; single-bit rule unenforced); (5).20 and (9).7 (CDC skipped at two
or more clocks: a two-clock sequential module compiles); (4).1–(4).2, (4).11 (an `In`
appears only as `p.read()`, rewritten per static tag); (4).6 (reads inside macro tokens
neither classified nor rewritten; `matches!` expanded at capture, `parser.rs:831`);
(12).3 (`control_extract::desugar_tick_waits`: tick-bearing `while c {b}` →
`loop { if !c { break } b }`); `control_extract::desugar_counted_loops` (tick-bearing
`for i in a..b` → counter register + loop + hoisted tick; a literal-empty range dropped;
a non-literal one undecidable, RULES open 5); `chir_lower.rs:3286` (tick-free `for` →
SV `for`, Verilator unrolls); `:3401` (tuple assignment: projections into wires first,
then the assigns — simultaneous); `:3467` (`arr[i] = v` on an array local → one-port
memory write; a second write per cycle refused); `:383` (helper fns and methods inlined,
`let`s plus a tail only, recursion refused); `:2458` (struct locals one wire per field;
`..rest` refused); `file_consts.rs` (integer file-scope `const` → `localparam`); `:3606`
(`break`/`continue` escaping the module loop refused by the transpiler only; the
simulator panics at run time, CODE (1).15); `:3891` and
`copper-codegen/tests/unsupported_constructs.rs` (any other construct — `/`, a statement
macro, a closure outside a preload — refused by the transpiler, admitted by the macro).
RULES rule 1 (`check_admissible`, `copper-analysis/src/lib.rs:824`: port payloads and
annotated `let`s only; unannotated lets and array lengths unchecked; macro only, never
the transpiler; one-way calibration `copper-codegen/tests/admissible_calibration.rs`; no
ui case; RULES open 10). `frontend_ir.rs` captures 33 expression kinds including
`Closure`, `Async`, `Yield`, `Try`, `Return`, `Macro`. MEASUREMENTS §B: a pre-loop
expression statement runs once in the simulator and every iteration under extraction;
no rule refuses it. Corpus (94 files under `examples/`, `tests/fixtures/`, `src/`; 176
modules: 140 sequential, 26 combinational, 8 synchronizer, 2 structural): `while` 10
modules, `for` 33 files, tuple assignment 2, file-scope `const` 12, `struct` 23, `enum`
15, indexed write 15, `match` 22, `const { assert! }` 7, helper fn 12, `impl` 23,
`Memory::` 9, `break` 4, `continue` 3, `Bits::x()` 2, closures 14 (all preloads),
`if let` 0, labelled loop 0, `return` 3; non-`let` pre-loop statements 2 modules (both
CPUs); macros inside bodies: `const { assert! }`, `matches!`, `vec![]` only.
Anchor: none — decided (a syntax clause). Meanings the kernel must carry unchanged
are anchored at their own clauses: tick and segment (`det_010`, `sipo_block`, `lfsr`,
`shift_register`); the test-before-tick wait (`det_010_awaits` vs
`pattern_detector_010.sv` — the `while` desugaring is a Rust identity, so the anchor
covers both spellings); hold (`bsg_dff_en`); `RegOut` (`sipo_block`); memory
(`dual_port_ram`); the synchronizer (`sync_2ff_ref`); combinational modules (the five
goldens). Sim ≡ SV only: the counted `for` (`det_for`), tuple assignment
(`traffic_light`), word-indexed registers (the pipelined CPU), struct locals, inlined
helpers.
Prior work: Esterel [esterel §1, §3.2, §4] — a 14-statement basic kernel, plain Esterel
by macro-expansion, the semantics extends the kernel syntax to carry state; "a loop body
cannot terminate instantly" is the compile-time shape rule [esterel §3.4]. SystemC
synthesis subset [systemc §1.3.3, §4.2, §8.3] — every construct Supported / Ignored /
Not Supported / Supported with Restrictions; a Supported construct's meaning is the host
LRM's. MyHDL [myhdl §1, §10.4] — the convertible subset as lists of types and
statements, strictly wider than "synthesizable". Prost [prost §1–§3] — no grammar, one
sentence of meaning, every local a register. Filament [filament §1, A.1; note §9.2
item 4] — a simplified core (one event per component), proofs over Low Filament, the
compiler's explicit form, not the surface. Anvil [anvil §1, Fig. 7 vs Fig. 10] — a
two-layer core calculus dropping `recursive`, `spawn`, data types and `;`, "not quite
closed". Golden Age — a formally specified core as the common target. Spade — the
generated SV is normative (the contrast).
Options:
  A. The kernel is the desugared surface: the `syn::ItemFn` after a stated syntactic
     desugaring D; registers (I.7), read classification (I.4) and segments (I.3) are
     defined functions on it; FIR/CHIR/SHIR/VLIR and the `match pc` of control
     extraction are implementation forms (SC17; Esterel's derivative enumeration). The
     simulator runs the surface as written, so the spec owes one statement — D
     preserves Rust meaning — true by construction for every construct except the
     non-literal empty `for` range (carried) — changes: none to sim or codegen now;
     II.1 gains the parity holes the classification exposes (`/`, statement macros,
     top-level `break`/`return`, a two-clock sequential module, reads inside macro
     tokens, `check_admissible` absent from the transpiler); corpus cost: 0 modules
     change meaning; refusing every NOT-YET row costs 0 modules except the pre-loop
     statements (2, both CPUs), which I.9 decides.
     A1. Classification IN / DESUGARED / ERASED / OUTSIDE plus one carried NOT-YET list;
         no "Ignored" class (`const { assert! }` is the one erased construct).
     A2. The type grammar is I.14's; I.2 says only that every port and local has a type
         of I.14. Rule 1 goes to II.1 with the obligation that the transpiler run it.
     A3. One kernel, three instantiations: sequential is the kernel; synchronizer is the
         kernel with the single-domain clause replaced (I.12); combinational is the
         tick-free statement language evaluated at every instant (I.11), the
         `loop { body; delta_yield().await }` wrapper an implementation form. Structural
         is not this kernel but a wiring language (I.13).
     A4. The pre-loop region is `s_pre ; loop { s }` with `s_pre` tick-free,
         syntactically; what a non-`let` pre-loop statement means is I.9's.
  B. Meaning on the post-inference, post-extraction IR, the lowering proven
     meaning-preserving (Filament's Low Filament move). Registers and `pc` become
     primitives rather than consequences, so REG5 must be re-justified; P4 ("the
     simulator implements the spec") becomes unmeasurable in principle, since the
     simulator never computes that IR; the transpiler's own intermediate form becomes
     the reference the transpiler is checked against — the circularity T6 names (IMPL
     item 2, rejected 2026-07-29 as option b) — changes: none in code; a reversal
     entry against T1/T2; corpus cost: n/a.
  C. The surface with no desugaring, one rule per sugar — changes: none; corpus cost:
     every sugar needs its own rule and parity pin; no precedent in the survey.
Recommendation: A with A1–A4 — the object the c2 decision already fixed, the only
option under which "the simulator runs the user's Rust" stays visible in the definition
and P4 stays measurable, at no code change; the NOT-YET rows are II.1's first worklist,
not blockers.
Decision: **A, with A1–A4.** User, 2026-11-04.
Changes: none now. II.1 (Round 1) takes the parity holes as shape rules or refusals:
`/`, statement macros outside `const`, `break` of the module loop and `return`, a plain
sequential module with two or more clocks, a read inside macro tokens, and
`check_admissible` run by the transpiler. I.9 takes the non-`let` pre-loop statements.
Obligations: a ui/fail case per parity hole once II.1 refuses it, and the front-end
parity audit extended to each; a test that D is the identity on Rust meaning for the
`while` and counted-`for` rows (both spellings simulate identically) and a decision for
the run-time-empty range (RULES open 5); I.12 to decide the unit-domain `In<T>` and the
multi-clock sequential module; I.14 to state the value grammar I.2.6 defers to; the
corpus counts above to be re-derived by `tools/stats` rather than by hand before III.3.
Coroutine terms (D-7): the kernel is the coroutine's body; `tick` and a deferred `read`
are its two suspension kinds; the IRs and the `match pc` FSM are encodings of its
projection H(M) (→ I.0.1, I.0.4).
Note (D-8, 2026-11-04): stands. A `sequential` or `synchronizer` module's outputs are `RegOut`
only — `Out<T, D>` is a parameter of a `combinational` module (I.2.2; one new shape rule of
II.1); `allow_pretick_alignment` is retired with the rules it opted out of; the coroutine-terms
line above reads "one suspension kind, `tick`".

### D-3 — Segments, the trailing region and the residue           (2026-11-04)
Clause: SPEC §I.3
Question: What is a segment — a function on the kernel's executions, a Comb-component of
the CFG with the loop head in or out, or the textual split of the loop body at ticks —
at which of I.1.2's two instants does each statement of a segment execute, what is the
trailing region (the statements after an iteration's last tick) and which of its
consequences are I.3's, what is the residue a module carries across an edge and what do
`break`/`continue` do to it, where do "two iterations suffice" and the zero-iteration
loop live, and what does I.3 owe I.9 about cycle 0?
Docs say: DOCS S1/S3 (SS denotation, CDS §2: a segment is a maximal tick-free region of
an execution, trailing statements merged with the head; the segment opening at edge N
denotes cycle N), T11 (a clock cycle is a maximal tick-free region of an execution), T12
(an FSM state is a node with an incoming `E_tick`), SC10/SC18 (statement-granular CFG
with head `H`; a phase is a Comb-connected component), S2 (program order with
forwarding, both implementations owe it), S4 (opening prefix / closing suffix at the
read-site barrier), S16 (the simulator's anchor is dynamic: closing iff a deferred read
parks the task first), IN11 (every read after the last tick is immediate), S5/DT §1
(anchoring is per region, pre-tick vs trailing), TR1–TR9 (C and the next A share the
head's cycle, decided 2026-08-25; C lowers into phase 0 and is emitted before A; C's
register updates commit at the last tick's edge with a shared forwarding map; the
single-tick body's trailing statements share the head's phase, a multi-tick body's do
not; `crosses_more_than_one_tick` counts a folded loop as more than one), S7 (a
trailing region's commits anchor to the opening edge, its reads are Immediate), REG9 (a
value from C into A is a register, `sync_2ff`'s `ff2`), SC12–SC16, SC20, IN8, gap 17 (a
tick-bearing loop running zero iterations at run time has no stated meaning), gap 18.
FORMAL_SEMANTICS §3.2–3.4: the residue `s′ ; loop { s }` run per cycle with a phase flag
that flips at the first deferred read executed; §5 P6 / §6 G6: "two iterations suffice"
is unstated. Contradiction 6 (SS/CDS §2 merge the trailing statements with the head; CDS
§10.7–10.8: "the Comb-component with the loop head cut out, so the trailing statements
are a segment of their own"). Contradiction 2 (a trailing `RegOut` commits at the
opening edge in SS's denotation, DT §1 and CDS §2; at the next edge in SS's trailing
section and RO3). Contradiction 22 (SS calls the C-before-A order unobservable through
ports because `multi_write_collapse` refuses the straddle, yet admits the straddle
without a leading read). Contradiction 14 (CDS §3's "no barrier" cell vs PIS §0).
Code does: `copper-analysis/src/cfg.rs:287-306` (`H` is node 0; the body's last
statement falls through to `H` by a `Comb` edge; a trailing tick's `Tick` edge targets
`H`), `:3044-3058` (top-level `continue` → `Comb` edge to `H`; nested `break` → the
sub-CFG's exit sink), `:2896-2920`, `:3070-3140` (a tick-bearing nested loop is one
folded node with a `Tick` out-edge in the parent and a real sub-CFG), `:539`
(`pre_tick_region` = `Comb`-reachable from `H`), `:1216`/`:1271`/`:2383` (the two
trailing rules' region = nodes that `tick_free_reaches` `H`), `:2321-2350`
(`comb_phases`: union-find over `Comb` edges, "the trailing statements merge with the
head"), `:1334` (`crosses_more_than_one_tick`), `:3827-3835` (`classify_reads` walks the
loop end with `tail_has_tick = false`, making trailing reads Immediate). Two facts read
off the code that no document states: (a) every `Comb` edge into a tick node is
unioned, so two execution segments ending at one tick are one component — measured by
`derivation-audit` on `branch_merge` (two tick nodes, two extracted states, one phase);
(b) a head statement on a path to a module-loop `continue` tick-free-reaches `H` and so
counts as trailing for the two trailing rules. `copper-codegen/src/shir_lower.rs:583-588`,
`:875-885` (`split_at_ticks`: N ticks → N+1 textual segments, N = 0 refused), `:642-709`
(single tick: trailing lowered first, `opening = false`, merged before the head, one
shared forwarding map), `:760-845` (multi-tick: the trailing segment's register updates
in phase N−1 with the shared map, its combinational statements lowered under the last
phase's renames and prepended to phase 0; a trailing `RegOut` takes the committed value,
`trailing_reg_outs`, `:681`/`:787`). `copper-codegen/src/control_extract.rs:1210-1215`
(a trailing tick emits `pc = head`, no extra state), `:1351-1357` (the module loop's
fall-through is a zero-time goto, `FallThrough::ZeroTime`), `:1226-1233` (`continue` of
the module loop → zero-time goto), `:1236-1255` (`break` inlines the enclosing
continuation in the same cycle, lowered once per loop), `:1276-1288` (a nested loop's
head state holds the body rotated around its tick), `:1768-1783` (`continue` inside a
nested loop declined — on the macro side the same shape is a zero-time cycle refused by
`check_reachability`, `cfg.rs:6857`, since a nested body must end at its tick,
`shape.rs:107-150`), `:526-549` (pre-loop expression statements moved into state 0,
which every iteration re-enters: MEASUREMENTS §B, divergent, unrefused). Simulator: as
I.1.6 — a tick resolves at `t⁺`, the coroutine runs plain Rust through C, the back edge
and A until a deferred read parks it; nothing is injected at the loop end (CODE unstated
14). Corpus (grep script over `examples/`, `tests/fixtures/`, `src/`; 135 clocked
modules, the audit bin agreeing on the count): single-tick 89 (trailing region
non-empty 61, empty 28); linear multi-tick 15 (non-empty 3: `slow_counter`,
`trailing_out`, `trailing_local_into_head`); extraction 31 (non-empty 17); tick-bearing
nested loops 32; `break` 4; `continue` 4, all of the module loop (`rx`, `uart_rx`,
`uart_rx_dut`, `skip_on_halt`).
Anchor: for the single-tick body, `sync_2ff` vs `examples/cdc/sv/sync_2ff_ref.sv`
(`tests/cdc_synchronizer_anchor.rs`: one destination cycle of observable latency — C and
the next A in one cycle, C first, `ff2` a flop) and the tick-first BaseJump anchors
`bsg_counter_up_down` and `bsg_dff_en` (the whole body a trailing region, Moore output
visible at `obs_N`); the head segment at the closing instant: `det_010`/`det_010_awaits`
vs `pattern_detector_010.sv`, `lfsr`, `shift_register`, `sipo_block`. Sim ≡ SV only: the
trailing region of a body with more than one tick (`trailing_out`,
`trailing_local_into_head`, `linear_trailing`, `trailing_update`, `branch_trailing`,
`uart_rx`), the C-before-A order through a local (`trailing_local_into_head`), a trailing
`RegOut` at the next edge (`regout_trailing_single_tick`; MEASUREMENTS §C debts 2, 3), the
residue as `pc` (every extraction fixture). No anchored module has a non-empty trailing
region after more than one tick. The vocabulary, P6's placement and cycle 0's single
instant are decided.
Prior work: Esterel [esterel §6.5.6, §3.4, §9] — the sequence rule's two cases ("the
segment continues past `s₁`" / "the segment ends inside `s₁`, residue `s₁′ ; s₂`"), the
loop rule unfolds within the instant so the end of one iteration and the start of the
next are one reaction, "a loop body cannot terminate instantly" is reachability, states
are derivatives, finitely many; one observation point per instant. Prost [prost §2–§3,
§5] — a cycle is one advance of the coroutine to its next `wait`, the state is program
counter plus locals, no trailing notion, "each loop contains at least one wait and runs
at least once". SystemC [systemc §4.2.2, §10.5] — a CTHREAD resumes after the edge and
runs to the next `wait()`; `continue` supported, `break` out of the infinite loop not.
Anvil [anvil §4.5 E-Wait, C-Extend, Lemma C.19 / App. D.4] — `t₁ ⇒ t₂` overlaps the
last cycle of `t₁` with the first of `t₂` (the precedent for "C and A share a cycle"); a
loop is checked on `t ⇒ t`, two iterations suffice because the loop head is a cut
vertex. Filament [filament §6, Def. 6.1] — the back edge as the union of two shifted
copies of the log at distance 1. Clash [clash eq. 11], Spade [spade §III-A] — one step
per cycle, `reg;` as a stage boundary, no trailing region. No precedent for a segment
with two execution instants, for the trailing-region/head split, or for a residue model
combined with sub-cycle instants.
Options:
  A. One segment per cycle, defined on executions (a maximal tick-free run of an
     execution; C and the next A one segment; nested loops, `break`, `continue` and
     branch-nested ticks are statements of the execution, no special case); the trailing
     region is the named leading part of its segment; the residue is the continuation at
     the tick (one per tick site, T12); statements execute in program order with
     forwarding, at `t_N⁺` until the first deferred read of the segment executes, then at
     `t_{N+1}⁻` (which reads are deferred → I.4; where a write lands → I.5), so the
     trailing region always begins at `t_N⁺` and S7/TR6/IN11 become corollaries; the
     static objects (textual split, Comb-component, `pre_tick_region`, the trailing
     rules' region) are implementation forms and approximations with facts (a)/(b)
     carried; contradiction 6 resolved, 2 dissolved by removing "commit edge" from Part
     I, 22 resolved by definition (C then A, last write wins, observable through a
     straddled port without a leading read) — changes: none; corpus cost 0.
     A1. The dynamic flip, per execution path, is the definition of the instant; I.4
         defines "deferred".
     A2. "Commit edge" vocabulary leaves Part I: registers are I.7's carriers, `RegOut`
         is I.6's, the rules' trailing clauses are II.2's derived forms.
     A3. P6 ("two iterations suffice") is III.1's; I.3 states only the lemma it rests on
         (a segment crosses the back edge at most once).
     A4. A nested loop running zero iterations is I.8's (it contributes no boundary;
         admissibility and the empty-range NOT-YET row are II.1's).
  B. CDS §10.7's vocabulary: segments are the Comb-components with `H` cut out, the
     trailing region a segment of its own sharing the head's cycle, "cycle" a derived
     union; every clause about "the segment" needs a trailing sibling (today's rule
     structure) and contradictions 2 and 22 survive as special cases — changes: none;
     corpus cost 0.
  C. Residue only (Prost/SystemC): Part I defines the residue and the per-cycle run;
     segment, region and phase appear only in Part II as the rules' static
     approximations; I.4–I.6 then have no named object for the statements between two
     ticks — changes: none; corpus cost 0.
Recommendation: A with A1–A4 — the sentence T11 and the simulator already embody, what
Esterel's sequence + loop rules and Anvil's overlap say in their own terms; it makes
S7/TR6/IN11 corollaries and dissolves contradictions 2, 6 and 22 at zero code cost; the
one thing it carries forward is that the multi-tick trailing region is anchored sim ≡ SV
only.
Decision: **A, with A1–A4.** User, 2026-11-04.
Changes: none now. Part I uses "segment" (per cycle, on executions), "trailing region"
(the leading part of a segment after an iteration's last tick) and "residue" (the
continuation at a tick); "phase", "pre-tick region", "textual segment" and "commit edge"
are Part II/III words from here on. CDS §10.7–10.8's "segment" is read as "textual
segment" where the two differ.
Obligations: I.4 must define the deferred read (the flip trigger of I.3.4) and give
cycle 0's reads their value under I.3.7; I.5 must say where an opening-instant and a
closing-instant write of a plain `Out` land, with the trailing region's writes as
opening-instant writes and the C-then-A order as last-write-wins; I.6 must settle the
edge at which a `RegOut` written at `t_N⁺` takes effect (RO3; contradiction 2) with only
the instant supplied here; I.7 must give the register carrier of a value assigned in C
and read in the next A (REG9's back-edge clause) and state what a register's "value in
cycle N" is relative to C's assignments; I.8 takes loops, `break`, `continue`, the
rotated nested-loop state and the zero-iteration loop (gap 17; the counted-`for`
empty-range NOT-YET row of I.2.8); I.9 takes the pre-loop statements and `obs_0`
(MEASUREMENTS §B remains an admitted, unrefused divergence until then). II.2 must name
the static object each rule reads (Comb-component, `pre_tick_region`, tick-free-reaches
`H`, `crosses_more_than_one_tick`) and the direction in which it approximates the
execution segment, carrying facts (a) and (b) — a unit pin for (a)
(`clock_phase_count` = 1 on `branch_merge`'s shape) and for (b) (a head write before a
module-loop `continue` is reported trailing by `trailing_out_reads_input`'s region),
measured verdicts to be decided there. III.1 states P6 as the "two iterations suffice"
proposition over `body ; body` with I.3.6 as its premise (G6 closed as a statement, its
pin owed). III.2 carries the anchor debt: an independent reference for one body whose
trailing region follows more than one tick (user-written, per
`ANCHORING_A_MODULE.md`), and for the C-before-A order through a port. The corpus
counts above are to be re-derived by `tools/stats` before III.3.
Coroutine terms (D-7): a segment is the statements of one cycle's two resumptions; the
residue is the tick suspension site, a barrier suspension an intra-cycle residue; the
trailing region is the opening part of the post-edge resumption (→ I.0.1, I.0.2).
Note (D-8, 2026-11-04): stands, by vocabulary — a segment is the statements of one resumption
(one per cycle); the trailing region is the leading part of the next resumption and executes
with it at `t_{N+1}⁻`; A1's per-execution instant is vacuous (one instant); (c)'s carrier
across the edge is not needed — `sync_2ff`'s `ff2` is a wire of one resumption, the second
flop is the `RegOut` `q` (D-6); I.3.3's item (v), the plain-`Out` wires, leaves with D-5.

### D-4 — Reading an input                                         (2026-11-04)
Clause: SPEC §I.4
Question: What value does the kernel read `let t = read x` yield — the settled value of its
wire at the instant the read executes (by position), or the settled input of its consumer's
window (by value, IN1) — what makes a read site deferred (the flip trigger of I.3.4), can a
read in the trailing region be deferred, what do cycle 0's reads see, is the register-free
passthrough exception a clause of meaning and in which of its four scopes, and which of the
implementation's read facts are approximations for Part II/III rather than meaning?
Docs say: DOCS IN1 (by value: the closing-edge sample when it feeds a commit, the opening
value when it feeds only combinational logic), IN2/IN3 (deferred iff a tick follows within
the iteration; samples at the next pre-edge settle; else immediate), IN4–IN6 and
contradiction 4 (the D2 passthrough: SS "a segment assigning no register", CDS §9.1/§10.1 "a
register-free body", CDS §10.8 (c) "the direct argument of `o.write(…)` only", IMPL item 3
no exception), IN5 (never a condition read), IN7 and DT §2.2 calibration 2 (a condition read
steering the next suspension point feeds the implicit `pc`), IN8 (the tick-first wait is
refused, so no choice between the two samplings is made), IN9 and S20 (sim and hardware
differ only for an input changing mid-cycle; an `In` driven by a clocked module in the same
domain "is stable"), IN11 (every read after the last tick is immediate), IN12 (a folded
loop's read is closing evidence), IN13 (the impl plan's exception-free rule). S4/S16 (the
simulator's anchor is dynamic: closing iff a deferred read parks the task first), S8 (closing
iff a committed value depends on a same-cycle read; never implemented), S21 and gap 1 (the
dual-consumer read "open as a refinement"), gap 7 (a `RegOut` with an immediate read in a
trailing guard, unmeasured), gap 22 (mid-cycle input changes). FORMAL_SEMANTICS §3: Read-D
yields `x_{N+1}` and flips `φ` to C, Read-I yields `x_N` under O and `x_{N+1}` under C, `cls`
static; §6 G1 (two authorities for the anchor). Two things no document says: IN1 is false for
a trailing read (`bsg_counter_up_down` assigns a register from a read anchored at `x_N`),
and the direct-argument scope is not expressible on D-2's kernel (both spellings of the
passthrough desugar to `let t = read x; write o t`).
Code does: `copper-analysis/src/cfg.rs:3775` (`ReadTiming`), `:3804-3833`
(`classify_reads_with`: deferred iff a tick follows in the iteration; the loop end is walked
with `tail_has_tick = false`; "register-free" is the whole module's `registers()`),
`:3880-3887` (`passthrough_read`: in a register-free module the arguments of
`<plain Out>.write(…)` are classified with no tick after), `:3973-3985` (an `if` condition is
deferred if either branch ticks), `:3987-3998` (a `match` scrutinee likewise; a **guard** gets
the match's own continuation only, `:3994-3995`), `:4003-4010` (a `while` condition is deferred
if its body ticks; the body is a fresh iteration), `:4012-4016` (`for`/`loop` bodies fresh
iterations), `:3922-3925` (macro statements skipped), `:2945-2949` (the CFG counts every
identifier in a macro's tokens as a use, so the rules see a read the classifier never tags).
`copper-macros/src/lib.rs:686-729` (`ReadRewriter`: tags consumed by index in visit order;
`Deferred` → `{ pre_edge_barrier::<D>().await; x.read() }`; any other tag, including a
missing one, → a plain read, `:724-725`), `:841-848` (the count check is `debug_assert_eq!`
only: CODE (4).7, unstated 16), `:620-660` (the use restriction, a compile error: CODE (4).11),
`:731-737` (doc names a wrap counter and a tracker; neither exists — the per-read "freshness
guard" is the barrier and nothing else). `copper-sim/src/lib.rs:101-117` (`PreEdgeBarrier`
Ready on any poll in the domain's pre-edge phase, Pending in post-edge), `:47-52` (a never-set
domain is pre-edge). `copper-sim/src/executor.rs:364-416` (nothing polled at spawn), `:813-853`
(a tick = pre-edge settle, `advance`, post-edge settle; so every future's first poll is the
first pre-edge settle and a deferred read there does not park: CODE (1).11), `:545-556` (the
levelized settle polls producer components before consumers: CODE (4).9).
`copper-core/src/port.rs:120-124` (`In::read` clones the live cell), `:86-92` (`Out::write`
immediately visible: CODE (2).2). Corpus (a `syn` walk re-implementing the classifier over
`examples/`, `tests/fixtures/`, `src/`, agreeing with `classify_reads` on every read of every
module; 135 clocked modules, 130 with `In` params): 86 modules have a deferred read, 44 an
immediate one, none both; the passthrough exception fires in 4 (`event_sink`, `event_source`,
`slow_consumer` in `two_domain_counter.rs` and `two_domain_hierarchy.rs`); a register-free
hoisted `let t = x.read()` before a tick, deferred today: 1 (`plain_out_multiphase`); a
register-free condition read before a tick, deferred: 6; a module with registers and a
direct-argument plain-`Out` read before a tick: 0 (the SS and CDS scopes classify the same
set today); trailing immediate reads: 44; immediate read at a nested loop's tail: 0; deferred
reads inside nested loops: 9; `while`-condition reads: 16; deferred reads in `if` tests or
scrutinees: 37; reads in `match` guards: 0; deferred by may-reach only (a tick-free path to
the iteration end exists): 1 (`det_010_awaits`, its inner `if in_i.read() == Zero { … tick }`
with no `else`); dual-consumer reads (a `let` whose value feeds a commit and a plain `Out`):
4, all `sipo_block` (`w0..w3` → `data_o` and `wN_dbg`). Measured (simulator, the D2 chain
clocked producer → `sync_2ff` → consumer): `o.write(q.read()); tick` tracks the producer
(out at the synchronizer's cycle), `let t = q.read(); o.write(t); tick` lags one cycle,
`tick; o.write(q.read())` tracks — one kernel program, two traces.
Anchor: a pre-tick read feeding a commit samples at the closing edge — `det_010`,
`det_010_awaits` vs `pattern_detector_010.sv` (`tests/det_010_independent_golden.rs`), `lfsr`
(`bsg_lfsr`), `shift_register`, `sipo_block` (BaseJump; also the read between two ticks); a
trailing read is immediate even when it feeds a register — `bsg_counter_up_down`, `bsg_dff_en`
(BaseJump), `sync_2ff` vs `sync_2ff_ref.sv`; may-reach — `det_010_awaits`'s inner test, a tick on
one path and the loop end on the other, the corpus's one such read; the passthrough tracks a
clocked producer — `two_domain_ref` (hand-written, `tests/two_domain_hierarchy_cdc.rs`; the D2
pin `d2_is_fixed_and_d1_still_demonstrates_the_hazard` is simulator-only). Sim ≡ SV only: the
`Out` leg of the dual-consumer read (`sipo_block`'s `wN_dbg`, excluded from its BaseJump
check). Unanchored: the parking rule as a definition (MEASUREMENTS debt 12 — every shape where
it matters is refused; V8c measured sim against the transpiler's own assign). Decided: cycle 0
(no harness observes before edge 1).
Prior work: Esterel [esterel §2.2, §3.3, §3.5] — an input's value changes only at a tick, one
value per signal per instant (the degenerate case of "by position", one instant). Prost
[prost §3] — inputs read inside `advance` at the edge, every read closing. SystemC [systemc
§4.2.2] — a thread resumes after the edge and reads the committed value, every read opening.
Cummings [cummings §4.0, §6.0] — a clocked block's RHS is evaluated from pre-edge values;
Guideline 3's `ao4` lag is the D2 shape. Anvil [anvil §4.5, T-RegEval] — a read at `e_c` sees
the value at `e_c`, no sub-cycle point. Filament [filament §6, Def. 6.1], Clash [clash §II-F,
eq. 11], Spade [spade §III-A], Chisel [chisel §5], PyMTL3 [pymtl3 §5], Bluespec [bluespec §5],
RustHDL [rhdl §3] — one value per signal per cycle, no point of read. MyHDL [myhdl §12.2.4,
§6.3.2] — reads see the committed value; `posedge; delay(1)` is the only post-edge read. Arch
[arch §4.6, Listing 9] — when a wait's condition is sampled is left unstated. No precedent for
a static per-site classification by position relative to the cycle boundary, nor for the
passthrough exception.
Options:
  A. By position: a read yields the settled value of its wire at the instant it executes
     (`x_N` at `t_N⁺`, `x_{N+1}` at `t_{N+1}⁻` for an environment input; a producer's value
     at the instant for a module-driven wire, → I.13); a read has one value, so the
     dual-consumer read is dissolved (gap 1); cycle 0's reads see `x_1` as a consequence —
     changes: none; corpus cost 0.
     A1. A read site is deferred iff some kernel path from it reaches a `tick` before the
         module loop's back edge (may-reach, static, per site; condition, scrutinee, guard and
         desugared-`while` reads by the same rule, the arm's or body's tick counting); no
         read after an iteration's last tick is deferred, so the trailing region runs wholly
         at `t_N⁺` — changes: the guard tagging in `classify_reads` (Phase C); corpus cost 0.
     A2-i. No passthrough exception in I.4; II.2 refuses a plain `Out` written from a deferred
         read's value in a body with no register, on every route; the sanctioned spelling is
         the trailing read — changes: sim (`passthrough_read` retired), one II.2 rule, rule
         13's two register-free clauses retire; corpus cost 4 modules rewritten to
         `tick; o.write(x.read())` plus the test-only `slow_consumer`, the two CDC anchors
         re-run.
     A2-ii. Keep the exception, restated by value on the kernel: in a module with no register
         a read whose temporary reaches only plain-`Out` writes and steers no tick is not
         deferred — changes: sim (the `let` spelling becomes immediate), `plain_out_multiphase`
         re-measured or refused on its two-tick route, rule 13 re-derived; corpus cost 1.
     A2-iii. Today's direct-argument scope: not expressible on the kernel; would reverse D-2.
         Not offered.
  B. By value (IN1 as written) — refuted by the trailing anchors (`bsg_counter_up_down` feeds a
     register from a read anchored at `x_N`); gap 1 stays open — changes: sim; corpus cost 44.
  C. Deferred per segment (by I.3.2 a tick always follows within the segment, trailing reads
     included) — refuted by the same anchors — changes: sim; corpus cost 44.
Recommendation: A with A1 and A2-i — the one definition every anchor fits, one sentence on the
kernel, dissolving gap 1 and contradiction 4; the only thing given up is a spelling whose
meaning the simulator already gets wrong for one of its two forms.
Decision: **A, with A1 and A2-i.** User, 2026-11-04.
Changes: none now. Part I uses "deferred" and "immediate" for read sites (static, per site) and
"the instant of the read" for its value; "opening/closing sample", "leading read" and
"passthrough" are Part II/III words from here on. Phase C: `passthrough_read` retired together
with the II.2 rule and the rewrites of `event_sink`, `event_source`, both `slow_consumer`
examples and the test-only `slow_consumer` to the trailing form; `classify_reads` tags a
`match` guard by its arm's tick; the release-mode index fallback becomes a hard error; one
authority for "reads an input" (the CFG's macro-token uses vs the classifier's skip, resolved
by II.1 refusing the construct); the stale macro doc comment removed.
Obligations: unit pins for I.4.2 on a guard, a scrutinee, a `while` test and a nested-loop
read (today only `pretick_reads_are_deferred` / `trailing_read_is_immediate`); a ui/fail case
and an exact-set corpus pin for the new II.2 rule (the four modules before their rewrite,
empty after); the D2 chain measurement kept as a test of the three spellings after the
rewrite (direct and hoisted refused, trailing tracks); `tests/two_domain_hierarchy_cdc.rs` and
`flag_crossing` re-run after the rewrite; a pin that the two spellings of one kernel read
classify alike. I.5 owes where a write at `t_N⁺` and at `t_{N+1}⁻` lands, the hold, and the
observation rule the II.2 refusal rests on (a closing write from a deferred read is shown
from the next observation); I.6 owes the edge at which a `RegOut` written from a deferred read
takes effect (RO3); I.7 owes the carrier of a trailing read's `x_N` into the next cycle and
what a register's value in cycle N is relative to the closing sample; I.9 owes cycle 0's reads
under `obs_0`; I.13 owes the value of a module-driven wire at each instant (the composition in
which the passthrough question is observable: the `two_domain_ref` anchor, and a producer
writing at the closing instant, unmeasured); II.1 takes the macro-token read (refuse) and the
use restriction (`find_unprotectable_in_uses`) as a shape rule; II.2 derives the new rule,
states the direction of `leading_read_reaches`, `deferred_read_reaches` and codegen's
`seen_in_read` walk against "a deferred site executed earlier in the same segment on this
path", retires rule 13's two register-free clauses, and reads rules 11–12's "no tick
comb-reachable from the site" as I.4.2's immediate; III.2 states the levelized precondition (a
parked read sees its producers settled), the condition under which codegen's walk agrees with
the simulator's flip (FORMAL_SEMANTICS G1), and carries the `Out` leg of `sipo_block`'s
dual-consumer read and the parking rule's definition as anchor debt. The corpus counts above
are to be re-derived by `tools/stats` before III.3.
Coroutine terms (D-7): a deferred site is a barrier site; its read executes in the
resumption that passes the barrier — the next pre-edge one when reached post-edge
(parked), the current one when reached pre-edge (→ I.0.2).
Reversed by D-8 (2026-11-04): A1 (a site is deferred iff some path reaches a tick before the
back edge; the barrier at it) — no site is deferred and no barrier exists: every read of the
resumption that ends at edge N+1 yields `x_{N+1}`, trailing reads included; A2-i (the
register-free passthrough refused, the trailing read its sanctioned spelling) — vacuous: a
plain `Out` is not a parameter of a sequential module, and a passthrough is a `combinational`
module (same cycle, I.11) or a `RegOut` (one edge later, I.6). A (by position: the wire's
settled value at the instant of the read) stands, with one instant; cycle 0's reads see `x_1`
as before. The Phase C items of this entry's Changes (retire `passthrough_read`, tag a guard
by its arm's tick, the index fallback) are subsumed: `classify_reads` and `ReadRewriter`'s
injection go entirely.

### D-5 — Writing a plain output; observation and hold           (2026-11-04)
Clause: SPEC §I.5
Question: What does the kernel statement `write o e` do to a plain `Out` at the instant it
executes, which of a cycle's writes does `obs_N(o)` show and how do several writes in one
segment (the trailing region's before the head's) resolve, what does a port show in a cycle no
write reaches and before any write at all, is the closing write that the next opening write
overrides a clause of meaning or a refusal, what is the one meaning behind codegen's five
emitted forms and which are anchored, what frame does II.2 derive its plain-`Out` rules from,
and what separates a plain `Out` from a `RegOut` and from a combinational output?
Docs say: DOCS OUT1 (a combinational Mealy output; an unwritten path holds — an enabled
register; MEASURED on `bsg_dff_en`), OUT2 (a continuous drive from the write instant),
OUT3/OUT4 (forwarded opening drive, committed closing drive), OUT5–OUT13, OUT18–OUT19 (the
forms), T7 (the write-window invariant: a write at `t` is observed for `[t, next write)`), S10
(drives anchor to the opening edge; S4: a closing-suffix drive is seen at N+1), S16, S19 (a
closing write followed by the next iteration's opening write is never observed; MEASURED,
`d2b`), CM6 (a sequential `Out` may hold; definite assignment is combinational-only), PO6
(synthesized state powers on at 0: the edge register of a conditional `Out`, `_hold`), PO8 (a
wire starts at zero, harness convention), PO10 (`out_first_driven_late`; no initial-value rule
for `Out`). FORMAL_SEMANTICS §3.4: `obs_N(o) = last of Δ_{N−1}^C ++ Δ_N^O`, else
`obs_{N−1}(o)`; `obs_0(o)` the wire's initial value; §4's form table; §7 D-c (the dynamic
anchor as the definition, adopted by D-3/D-4). Gaps 18 (a pre-loop `Out` write and the value
before the first edge undefined), 19 (several plain-`Out` writes in one segment only
implicit), 9 and 13 (the hold form on wire-valued ports / the linear route unmeasured, not
recorded). Contradiction 22 (C-before-A "unobservable through ports" vs the admitted read-free
straddle — the order is D-3's, the value is I.5's), 11 (CDS §4's "Landed" note names the
retired `written_on_all_paths` clause; stale, nothing inherited). One statement no inventory
row carries under OUT: SS "Output timing" says of the write-around-a-tick collapse that "the
synthesized combinational Mealy output is correct; the simulator loses a cycle" — the one
place a document takes the hardware's side against the instants of D-1.
Code does: simulator — `copper-core/src/port.rs:86-92` (`Out::write`: lock, store if changed,
set dirty; immediate; no buffering), `:120-124` (`In::read` clones the live cell), `:135-148`
(`wire(initial)`: the initial value is the environment's argument; `build.rs:687` creates every
sweep output wire at `Bits::zero()`, the examples at `Bits::zero()` / `Logic::Zero`); CODE
(2).4 (the last write in a settle wins); hold is the absence of a call — nothing in the
simulator distinguishes positions. Transpiler — the anchor tag `SHIRPortDrive::opening =
forward_opening_drives && !seen_in_read` (`copper-codegen/src/shir_lower.rs:335-339`,
`:374-386`; arm-granular `:399-404`, `:435-443`; trailing segments never opening `:665`,
`:790`), `value` / `edge_value` (`:363-378`); five forms in `copper-codegen/src/vlir_lower.rs`:
F1 a module-level continuous `assign` for an unconditional top-level drive (`:738-741`;
forwarded if opening, committed if closing); F2 an every-path `always_comb` mux (a
single-phase default stays in the comb copy in program order `:369-372`, `nested_port_drives`
`:1520`; the Moore hoist `:1781-1857`; a multi-phase every-path port takes the held forms
`:438-453`); F3 the edge-sampled flop for a conditional port with no opening drive
(`split_output_regs` `:519-541`; power-on 0 through `port_inits`, `emit.rs:418-432`); F4 the
comb mux + `<port>_hold` for a conditional port with at least one opening drive
(`opening_hold_outputs` `:1579-1600`, `strip_closing_drives` `:1646`, `shadow_hold_ff`
`:1705`, the `o = o_hold` default in `comb_defaults` outside every phase guard `:316-320`,
`:448-517`; `_hold` init 0 `:468-470`; a port with a drive inside a `for` excluded `:1590`,
unstated 31); F5 the phase-scoped flop on the linear route (`phase_scoped_output_ports`
`:1530-1566`). Pinned: `copper-codegen/tests/hold_form_audit.rs` (20 mixed-anchor F4 ports).
Corpus (a `syn` scan over `examples/`, `tests/fixtures/`, `src/`, static approximation): 135
clocked modules, 88 with a plain `Out`, 114 plain-`Out` ports; held (some execution segment
does not write the port) 34 ports in 20 modules (`bsg_dff_en`, `sipo_block`, `traffic_light`,
`branch_merge`, `mac_pipeline`, `probe`, `trailing_out`, …); every-cycle ports 80; ports with
two or more write sites in one tick-free run 24 in 18 modules, every one arm-exclusive (no
plain-`Out` default-then-override on one path in the corpus; `e16j` is probe-only, and
`det_010_awaits`'s is on a `RegOut`); pre-loop plain-`Out` writes 0; ports written both before
the first tick and after the last 0; undriven plain `Out` 0. Probe verdicts read from
`tests/sequential_forwarding_divergence.rs`: `d2a` sim `[0, 5, 5, …]` — its closing write of
9 is never observed on any stimulus (the write that stores 9 also sets the state that fires the
opening 5 before the next observation), agrees under F4; `d2b` sim `[2, 2, …]`, SV
`[2, 1, 2, 1, …]` under F2, refused; `e18j` sim `[0, 1, 1, 3, 3, …]` — a closing write of the
committed register, F3 samples it before the edge, agrees, refused by V8's value clause;
`multi_write_collapse`'s ui case is the `d2b` shape on an unconditional port; `fast_counter`
sim ≡ SV under forwarded F1 while `ref_fast_counter` anchors the other spelling.
Anchor: a write at `t_N⁺` is in `obs_N` and an unwritten cycle holds — `bsg_dff_en` vs
BaseJump `bsg_dff_en.sv` (a tick-first single-tick body whose whole segment is a trailing
region; condition and value immediate reads of `x_N`; `if en { o.write(d) }` with no `else`;
forced hold patterns across `en = 0`), so hold is anchored at the opening position only, and F3
with it; also `bsg_counter_up_down`, `det_010` (a trailing Moore write from committed state).
A write at `t_{N+1}⁻` with forwarded locals is in `obs_{N+1}` — `det_110101` vs
`pattern_detector.sv` (a head every-path write of `state` assigned earlier in the segment
behind deferred reads), `lfsr` (`bsg_lfsr`), `shift_register`: the committed-form F1. Sim ≡ SV
only: hold at a closing-instant write (`d2c`, `e18a`); F4 (MEASUREMENTS debt 2, the
model-derived m2/m3 hand models); the forwarded opening drive (debt 1; `ref_fast_counter`
anchors the other spelling — the forwarded value is I.3.4's, not I.5's); F2 (debt 11, `e15y`,
`e16j`); F1/F5 on the linear route (debt 5); the never-observed closing write (`d2a`, `d2b`);
power-on (debt 6); startup (debt 7). Decided: the initial value (no harness observes before
edge 1).
Prior work: Esterel [esterel §6.1, §3.3] — the persistence law (`S⁻(v) ∈ Ê_{n+1} ⇒ Ê_n(S)
= v`) and one value per signal per instant: the one-instant form of "last write at or before,
sampled once". Chisel [chisel §2.6, Fig. 1] — last-connect with a seeded mux chain and an OR-ed
enable, the canonical hardware form of "last write on the taken path, else hold"; a static
driver priority with no forwarding, so it anchors the form, not the value. SystemC [systemc
§8.2] — an `sc_out` written by a thread holds and is visible in the same delta; the subset
declines to specify the pre-edge visibility. Cummings [cummings §15.3] — last nonblocking
assignment wins. MyHDL [myhdl §12.2.2], RustHDL [rhdl §3] — last `.next` write wins, hold when
unwritten, one instant. Anvil [anvil §9 item 3] — no value for an output in a cycle in which
nothing was sent; hold must be stated separately. Prost [prost §3] — no plain combinational
output. Filament [filament Def. 6.1] — an unwritten port is invalid, not held. Spade [spade
§IV] — a `&mut` wire set exactly once per cycle; forbids the shape. Verismith [verismith §3.3]
— compare the internals too. No precedent for a write at one of two instants inside a cycle
with one observation per cycle, nor for the never-observed closing write as a defined value.
Options:
  A. Last write at or before, sampled once: `write o e` at an instant sets the wire to `⟦e⟧`
     (forwarded, I.3.4) until the next write (I.1.3); `obs_N(o)` is I.1.4's sample — a
     write at `t_N⁺` is in `obs_N`, a write at `t_{N+1}⁻` in `obs_{N+1}`; within a segment
     the last write in program order wins, the trailing region's before the head's (gap 19;
     contradiction 22's value); FORMAL_SEMANTICS §3.4's formula and T7 are one law —
     changes: none (the simulator is this law verbatim); corpus cost 0.
     A1. Hold is a corollary, in every position; anchored at the opening position, carried
         sim ≡ SV elsewhere (debts 2, 4, 5, 11). Corpus: 34 held ports keep their meaning.
     A2. The never-observed closing write (a write at `t_{N+1}⁻` that a write at `t_{N+1}⁺`
         follows on the same execution) is a consequence of D-1, defined and never sampled.
         A2-i: II.2 refuses it for a design reason citing I.5 (a dead write; the forms are
         the write before the first deferred read, the write after the tick, or `RegOut`);
         `multi_write_collapse` and D1's every-path mixed-anchor clause become that citation
         — corpus cost 0 (`front_end_parity` pins the collapse corpus-clean; `EXPECTED_FLAGGED`
         holds witnesses only). A2-ii: admitted as dead code; `d2b` becomes a II.3 limitation
         with the known fix (an every-path mixed port through F4, as §10.9 did on the linear
         route, `e16q`).
     A3. The initial value. A3-i: `init(o)` is the zero of its type, decided here (PO6, PO8,
         the sweep); `wire(zero)` a III.2 precondition (closes unstated 8 / debt 6 by
         precondition, not by a sim change); the F1 startup transient — an `assign` has no
         "before the first write" state and shows its expression from time 0, so a port first
         written after `obs_0` shows `init(o)` in the simulator where the SV shows the
         expression (today refused only for the constant multi-tick case, PO10; register-valued
         late writes agree only because the register's initial is 0; predicted, unmeasured:
         `loop { tick; o.write(r) }` with `r` initialized to 5 differs at `obs_0`) — goes to
         I.9 with `obs_0` observed (D-1 A1). A3-ii: undefined until I.9/I.14; `obs_0` of an
         unwritten port outside the agreement relation until then. Corpus cost 0 either way.
     A4. One meaning, five implementation forms F1–F5 (III.2), as I.3.8 did for state
         encodings; the anchor status of each carried. No change.
     A5. The frame for II.2. Under A every plain-`Out` shape has a defined meaning; only
         `init(o)`'s realization at startup (→ I.9) and X (→ I.14) are open, so the category
         "a shape whose meaning the clause does not give" is empty for plain outputs — V8's
         "previous generation" is the carried register written at `t_{N+1}⁻` and shown at
         `obs_{N+1}` (F3 realizes it; `e18j` agrees), the path-dependent instant is I.3.4's
         per-execution instant. A5-i: a plain-`Out` rule cites I.5 (the dead write) or II.3
         as a dated lowering limitation with its fix direction; the opt-out names a
         limitation, never a meaning; `e18j`-type over-refusals are admitted when II.2 sorts.
         A5-ii: the static approximation's reach is the language — a path-dependent instant is
         outside the admitted language as a design rule; this makes I.3.4's per-execution
         instant a definition nothing admitted exercises, against the reason D-3 A1 chose it.
     A6. One line each for I.6 and I.11: a plain `Out` is a wire (a write takes effect at its
         instant and the wire keeps it); a `RegOut` is a register port (a write takes effect
         at an edge, from the next observation; several before one edge, last wins → I.6); a
         combinational output is a wire with one instant per settle, written on every path,
         no cycle to hold across (→ I.11, CM6).
  B. No hold: an unwritten port takes a default each cycle (Esterel's absent, Spade's and
     Chisel's wire) — refuted by `bsg_dff_en` as written (its plain-`Out` spelling matches
     BaseJump only under hold) — changes: sim (a reset of every plain `Out` at each `t_N⁺`),
     codegen (F3/F4 replaced by a defaulted mux); corpus cost 34 ports in 20 modules.
  C. Observe at both instants, so the closing write of A2 is shown for one cycle (SS's
     reading of the collapse) — reverses D-1; the simulator becomes wrong on every closing
     write — changes: sim (a second observation, a hold register per port), harness; corpus
     cost unmeasured.
  D. Every output registered (Prost, Anvil) — refuted by D-1 B. Not offered again.
Recommendation: A with A1, A2-i, A3-i, A4, A5-i, A6 — one sentence the simulator already is,
hold and the dead write as corollaries, the initial value fixed where PO6/PO8 put it with the
startup transient handed to I.9, and a II.2 frame under which a plain-`Out` refusal must cite a
clause or a dated limitation; zero code change and zero corpus cost now.
Decision: **A, with A1, A2-i, A3-i, A4, A5-i, A6.** User, 2026-11-04.
Changes: none now. Part I uses "write at an instant", "shown from `obs_N`", "hold" (a
corollary) and "the unobserved write"; "drive", "anchor", "opening/closing drive", "hold form",
"enabled register" and "continuous drive" are Part II/III words from here on. SS "Output
timing"'s "the synthesized combinational Mealy output is correct; the simulator loses a cycle"
is reversed by I.5.5 and is retired when SPEC is adopted. II.2 (Round 1) restates
`multi_write_collapse` and the every-path mixed-anchor clauses of `unprotected_pretick_out_write`
and `multi_phase_out_write` as citations of I.5.5, chooses the static direction of that rule
(the CFG is value-blind, S22: `d2a`'s closing write is dead on every execution yet the CFG sees
a conditional opening write — admitted today, agreeing), and measures its corpus cost; every
other plain-`Out` rule is sorted into II.3 with a date and a fix direction (the path-dependent
instant: no known form; the forwarded guard: gap 11; V8's value and guard clauses: the
edge-sampled flop for a closing drive, `e18j` admitted; `trailing_out_reads_input`: a form that
samples the input at the opening edge, CDS §9.4; the linear-route forms) or to its owning
section (`out_first_driven_late` → I.9; `memory_result_drives_plain_out` → I.10); the opt-out
`allow_pretick_alignment` is re-documented as a II.3 opt-out in Phase C.
Obligations: a unit property for I.5.2 on the executor (FORMAL_SEMANTICS G3: a two-write module,
closing in cycle N and opening in cycle N+1, asserting only the opening write is observed) and a
pin that a plain `Out` written twice on one path shows the last write (today only `e16j`, sim ≡
SV); a ui/fail case and an exact-set corpus pin for the I.5.5 rule once II.2 states it, and the
`multi_write_collapse` and D1 messages rewritten to cite it; `hold_form_ports` extended to the
linear route (gap 13) and the `for`-loop exclusion (unstated 31) and the wire-valued hold form
(gap 9) carried in II.3 with dates; III.2 states the environment precondition `wire(zero)` for
every port and carries the anchor debt — hold at the closing position, F4, F2, F5 and the
forwarded F1 are sim ≡ SV only (an independent reference for a held port written at the closing
instant and for one mux + hold port is owed; the user writes references, `ANCHORING_A_MODULE.md`);
I.6 owes the edge at which a `RegOut` write takes effect (RO1–RO3) and last-write-wins before
one edge (gap 19's other half; `det_010_awaits` is its anchor); I.9 owes `obs_0` for a port
first written after it (the F1 transient; `out_first_driven_late` restated from I.9 and I.5.6;
the `loop { tick; o.write(r) }`, `r ≠ 0` prediction measured with `obs_0` observed) and the
pre-loop `Out` write (gap 18's first half; its instant is `t_1⁻` by I.3.7 and it is in `obs_0`
by I.5.2; corpus 0; MEASUREMENTS §B's re-execution under extraction); I.11 cites I.5.7 for
"no hold, every path writes"; I.13 owes the value a consumer reads from a plain-`Out` wire
written at the closing instant of the same cycle (I.4.8); I.14 owes a write of X to a port; the
corpus counts above are to be re-derived by `tools/stats` before III.3.
Coroutine terms (D-7): a plain-`Out` write is a resumption's effect on a wire; `obs_N`
shows the wire after the post-edge resumption of edge N (→ I.0.1, I.0.3).
Reversed by D-8 (2026-11-04): A1–A6 as a whole — a plain `Out` is not a parameter of a
sequential module, so I.5 has no sequential subject. What moves to I.11 (the combinational
module's wire) rather than dies: A's "last write at or before" within one evaluation
(last-write-wins, I.5.3), A3-i's `init(o)` = the zero of the payload type with `wire(zero)`
a III.2 precondition (I.5.6), and A6's three-kind line (I.5.7) with the first kind
combinational-only. A1 (hold) is D-6's `RegOut` hold (I.6.3), anchored by `bsg_dff_en` in
its `RegOut` spelling (`review/Q1_TRADEOFFS.md` §4 (b): identical SV). A2-i (the dead closing
write), A4 (the five forms) and A5-i (the II.2 frame) are vacuous — no plain-`Out` rule
remains. The Phase C items of this entry's Changes are subsumed by D-8's.

### D-6 — Registered outputs                                       (2026-11-04)
Clause: SPEC §I.6
Question: At which edge does `write q e` on a `RegOut` take effect, given only the instant
I.3.4 supplies (an opening-instant write, the trailing region included, or a closing-instant
one); what do several writes before one edge and a cycle with no write mean; what are
`obs_N(q)`, `obs_0(q)` and `init(q)`, and where does a first-segment or pre-loop write land;
what does an input read at `t_N⁺` that reaches a `RegOut` write mean, and how is its refusal
sorted under I.5.8's frame; and what does I.6 owe I.10, I.12, I.13 and II.1?
Docs say: DOCS RO1 (a write takes effect at the first clock edge after it executes; "one
clause, no regions"), RO2 (a write before a tick appears one cycle later; MEASURED), RO3 (a
trailing write commits at the next edge, not the opening one; MEASURED 2026-10-23;
`trailing_reg_outs`), RO4 (immune to the phase question, not to forwarding; the commit is
computed from forwarded values), RO5 (a tick-first `q.write(acc)` is a copy one edge later),
RO6 (commits through the clock-edge listener; a phase source with no comb edge), RO7 (the
universal remedy), RO8 (rule 11 reads value sources only; a guard read unmeasured), S6
(commits, `RegOut` writes included, anchor to the segment's closing edge), S7 (a trailing
region's commits anchor to the edge that opens its cycle), S13, M9 (`q = data()` a trailing
commit at the producing edge), PO6 (a `RegOut` flop powers on at 0 in the SV). Contradiction
2 (SS denotation, DT §1, CDS §2: the trailing `RegOut` commits at the opening edge; SS
trailing and CDS §5 PC#2: at the next edge, "the paper assignment was wrong for `RegOut`").
Gap 7 (a `RegOut` with an immediate read in a trailing guard, unmeasured and unrefused), gap
19 (several writes to one `RegOut` before one edge only implicit). X9 concerns `sync_2ff`'s
`ff2`; `sync_2ff`'s `q` is a plain `Out` (`src/sync.rs:78`), so it says nothing here.
FORMAL_SEMANTICS §3.3 the (RegOut) rule `Π[q ↦ ⟦e⟧ρ]`; §3.4 `Q_{N+1}(q) = Π_N(q) if q ∈
dom Π_N else Q_N(q)`, `obs_N(q) = Q_N(q)`, "a write at either instant of cycle N−1 is
observed from N on; the anchor is unobservable"; §4 row `RegOut | always_ff | Q_{N+1}`; the
ledger pins. CODE unstated 8 (the simulator's initial is the wire's, the SV's is 0), 22
(listener order = registration order), 25 (the edge-set dirty flag).
Code does: simulator — `copper-core/src/port.rs:194-196` (`RegOut::write` stores `Some(v)`
into `pending`; no dirty flag; invisible to readers), `:173-183` (`on_posedge`:
`pending.take()` into `committed`, dirty only on change, no pending → hold), `:220-235`
(`registered_wire`: `committed` seeded with the argument, the listener registered at
construction), `:188-191` (`RegOut` has no `Clone`), `:198-204` (dirty handle
`Registered`); `copper-core/src/types.rs:886-900` (`Clock::advance`: cycle += 1, then every
listener in registration order); `copper-sim/src/executor.rs:813-853` (a tick = pre-edge
settle, `advance`, post-edge settle), `:364-416` (nothing polled at spawn: the first
segment runs in the first pre-edge settle). Hence a write executed at `t_N⁺` (the trailing
region included) or at `t_{N+1}⁻` is committed by the `advance` of edge N+1; a write at
`t_1⁻` by edge 1; a second write before the edge overwrites `pending` (CODE (3).1); a write
after the edge is a fresh `pending`, so no `RegOut` write is unobserved; listener order is
unobservable for `RegOut` (`pending` holds a copied value). Transpiler — every `RegOut` is
in `hold_outs` (`copper-codegen/src/vlir_lower.rs:329-333`) and `cond_outs` (`:402-405`);
an unconditional drive is kept as a `PortAssign` (`:716-734`); `split_output_regs` emits
`q <= edge_value` in `always_ff` under the phase or `pc` guard (`:519-531`, `:2107-2135`);
`port_inits` → `initial q = 0` (`emit.rs:418-432`); excluded from the mux + hold form
(`:1573-1586`); `edge_value` is the forwarded value (`shir_lower.rs:374-386`) except for a
drive in a trailing textual segment, the committed register (`:363-367`,
`trailing_reg_outs` `:205-215`, `:681`, `:791`). Transpiled this session:
`regout_trailing_single_tick` → `q <= acc; acc <= acc + d;` in one `always_ff`, `initial q
= 0`; `uart_rx_dut` → `rx_dv <= 1'b1` / `<= 1'b0` under `case (pc)` arms. Rule 11
`trailing_regout_reads_input` (`copper-analysis/src/cfg.rs:1214-1236`): value sources only,
guards not examined (`:1240-1242`), both front-ends, `EXPECTED = []`. Branch
`claude/trailing-regout-guard-67cf37` (commit `3412805`, not in `paper`; evidence of
measured shapes, not the tree's state): the guard clause, fifteen `e19*` probes every one
predicted before measuring — the seven input-guard rows diverge on every route, the
single-tick body included (`e19a`–`e19d`, `e19g`, `e19h`, `e19j`); a committed-register
guard, a deferred guard, a read before the ticks on the two-tick route and a declared
carrier register agree (`e19i`, `e19f`, `e19m`, `e19n`); three single-tick rows diverge,
admitted and recorded: `e19l` (`let v = x.read(); tick; o.write(v)`) and `e19e` (the same
read in a trailing guard) — the single-tick route emits the carrier `let` as a wire, so the
flop at edge N+2 reads the next iteration's read; `e19k` (`tick; cnt += 1; if cnt[0] { … }`)
— the trailing drive's value is committed but its guard is still forwarded. Pre-loop:
`control_extract.rs:526-549` moves pre-loop statements into state 0, re-entered every
iteration (`review/MEASUREMENTS.md` §B, measured divergent on a `RegOut`, unrefused).
Corpus (a `syn` scan over `examples/`, `tests/fixtures/`, `src/`, static approximation): 50
of 135 clocked modules declare a `RegOut` (15 single-tick, 7 linear multi-tick, 28
extraction), 61 ports, 91 write sites; before a tick 73 (9 with a direct input read in the
value, deferred); trailing 11 in 10 modules, none from an immediate input read in value or
guard (rule 11's pin agrees); path-dependent 0; inside a tick-bearing nested loop 4;
pre-loop 3 (the pipelined CPU's `program_counter`, `halted`, `a0_out`; §B's re-execution
masked by its halt path, which never returns to state 0); two writes on one path before
one edge 1 (`det_010_awaits.out_o`, the loop-top default and the reset arm, both `Zero`);
trailing + next-head pairs 0; ports with one site 42.
Anchor: a write before a tick, from forwarded locals the last of which is a deferred read,
is shown from the next observation and HOLDS across unwritten cycles — `sipo_block` vs
BaseJump `sipo_block.sv` (`examples/basejump/sipo_block.rs`, `tests/sipo_block_equivalence.rs`;
`data_o` written once per four cycles; the reference's `data_o <=` fires in state 3 only).
A write before a tick as a one-cycle pulse, cleared by the next iteration's write —
`det_010_awaits` vs `pattern_detector_010.sv` (golden: `assign out = (cur_state == D)`); its
two writes before one edge both carry `Zero`, so it anchors the shape, not last-write-wins
by value. An opening-instant write — the segment's first statement, before the deferred
`req` read — shown from `obs_{N+1}`: `handshake` (`tests/fixtures/handshake_dut.rs`) vs the
`REFERENCE` `handshake.sv`, hand-written from the English description by the module's
author ("a second spelling, not a third-party opinion"). `review/MEASUREMENTS.md` §C's row
naming `bsg_counter_up_down` and `bsg_dff_en` as `RegOut` anchors is wrong: both declare
plain `Out` (`examples/basejump/bsg_counter_up_down.rs:32`, `bsg_dff_en.rs:23`). Sim ≡ SV
only: the trailing position (`regout_trailing_single_tick`, debt 3; `rx`'s trailing
`rx_dv.write(Zero)`; the pipelined CPU's trailing `program_counter` against its own
Verilated self), forwarding into a write before a tick
(`tests/regout_forwarding_equivalence.rs`), last-write-wins by value (nothing), power-on
(debt 6), the pre-loop write (§B). Decided: `init(q)`, cycle 0.
Prior work: Cummings [cummings §4.0, §15.3] — the right-hand side of a nonblocking
assignment is evaluated when the statement executes, the left-hand side updated at the end
of the time step, "last nonblocking assignment wins": the `pending`/`take()` pair verbatim.
Chisel [chisel §2.6, Fig. 1] — "updated at the end of the current clock cycle"; last-connect
is a static priority with no forwarding; an unwritten register holds. Filament [filament
§2.3] — `Register: in@[G,G+1) → out@[G+1,L)` is this clause as a type signature. Anvil
[anvil §4.5, T-RegAssign] — a write at `e_c` takes effect at `e_c ▷ #1`; several writes in
one cycle not addressed. Prost [prost §3] — `.next` "affects all `.val` accesses after the
next wait", last write wins; every output is a `RegOut` (the contrast with I.5). RustHDL
[rhdl §3] — the last write to `.next` becomes `.val` in the next cycle. PyMTL3 [pymtl3 §4]
— `<<=` in `@update_ff` is a flop. Bluespec [bluespec §5] — two writes to one register in
one rule is a compile error; reads see pre-state. SystemC [systemc §5, §8.2] — an
`sc_signal` write is visible after the update phase of the current delta, not at the edge
(the contrast); `SC_ONE_WRITER`. Clash [clash §II-F], Spade [spade §IV] — one value per
register per cycle, no second write. Esterel — no register construct. No precedent for a
register port written at either of two sub-cycle instants with the instant unobservable.
Options:
  A. `write q e` at any instant of cycle N sets `q`'s pending value to `⟦e⟧` (forwarded,
     I.3.4); at edge N+1 the pending value, if any, becomes `q`'s value, shown from
     `obs_{N+1}`; the instant is unobservable through `q`; contradiction 2 dissolved (the
     SS denotation's "opening edge" precedes the write's execution; "first edge after" and
     "next edge" are one statement under D-3); FORMAL_SEMANTICS §3.4's `Q_{N+1}` kept —
     changes: none; corpus cost 0.
     A1. Several writes before one edge: the last in program order, across both instants
         and across the trailing region and the next head (one segment); gap 19 closed;
         anchored in form only (`det_010_awaits`), by value sim ≡ SV only — carried.
     A2. `init(q)`. A2-i: the zero of the payload type (as D-5 A3-i); `registered_wire(clock,
         zero)` a III.2 precondition (`build.rs:934`; every example); closes unstated 8 /
         debt 6 for `RegOut` by precondition. A2-ii: undefined until I.9/I.14.
     A3. Cycle 0: a write in the first segment, the pre-loop region included, executes at
         `t_1⁻` and is in `obs_1`; `obs_0(q) = init(q)`; the extraction route's
         re-execution of pre-loop statements → I.9 NOT-YET.
     A4. The input-sampling consequence: `tick; q.write(x.read())` means `q` takes `x_N` at
         edge N+1 (I.4.3 + A); the input-guard variant fires iff `x_N` satisfies the test;
         the SV flop samples `x_{N+1}`. A4-i: both refusals are II.3 lowering limitations
         with a fix direction — a carrier flop loading the input at edge N (`cfg.rs:1207`'s
         "staging flop"; `e19n` is it written by hand, and agrees) — dated, the rules kept
         until the form lands. A4-ii: a design refusal citing a new I.6 clause (a `RegOut`'s
         value at edge N+1 is a function of the state at edge N and the inputs at edge N+1
         only; Prost's rule). Corpus cost 0 either way.
     A5. `e19e`, `e19l`, `e19k`: defined under A (`e19l` shows `x_{N+1}` from `obs_{N+2}`).
         A5-i: II.3 rows with the branch's fix directions (promote the carrier `let` on the
         single-tick route as the two-tick route does; emit a trailing drive's guard
         unforwarded like its value). A5-ii: refuse by rule. Probe-only, corpus cost 0.
     A6. Frame lines: a `RegOut` changes only at its clock's edge, a source for I.13's
         settle (RO6, M15); one driver (not `Clone`, W6 → II.1); `q.write(m.data())` → I.10
         (M9); listener order and the edge-set dirty flag are unobservable artifacts.
     A7. No `RegOut` write is unobserved (writes at `t_{N+1}⁻` and `t_{N+1}⁺` land in
         different observations); I.5.5's naming of `RegOut` as a form stands.
  B. A write visible at its instant (SystemC's signal) — refuted by `sipo_block` (the
     reference registers `data_o`) and by D-1 B. Not offered again.
  C. S7's per-position edge (a trailing write takes effect at the edge that opened its
     cycle) — the write executes after that edge (I.1.2); the lowering that realized it
     diverged until 2026-10-23. Dead.
  D. One write per cycle (Spade, Bluespec, Anvil) — changes: a new Part II rule; corpus
     cost 1 anchored, agreeing module (`det_010_awaits`), for a hazard no implementation has.
Recommendation: A with A1, A2-i, A3, A4-i, A5-i, A6, A7 — the simulator's `pending`/`take()`
verbatim, Cummings' rule and Filament's `Register` signature, anchored at both instants
with hold anchored, zero code change and zero corpus cost; refusing the input-sampling
shapes by design would carve the language to a lowering.
Decision: **A, with A1, A2-i, A3, A4-i, A5-i, A6, A7.** User, 2026-11-04.
Changes: none now. Part I uses "pending value", "takes effect at edge N+1" and "shown from
`obs_{N+1}`"; "commit", "commit edge", "trailing commit", "anchor" and "staging flop" are
Part II/III words from here on. SS denotation's and DT §1's "a trailing region's commits
anchor to the edge that opens its cycle" (S7; contradiction 2) is reversed for `RegOut` by
I.6.1 and retired when SPEC is adopted; `review/MEASUREMENTS.md` §C's `RegOut` row is to be
corrected. Phase C: none to the simulator; II.3 gains, each dated 2026-11-03 with the
branch's measurement, the carrier-flop form for an input read at `t_N⁺` reaching a `RegOut`
drive's value or guard, the promoted carrier `let` on the single-tick route (`e19e`,
`e19l`) and the unforwarded trailing guard (`e19k`), with rule 11 and the branch's guard
clause kept as their refusals until the forms land; the sweep's and the examples'
`registered_wire(clock, zero)` becomes the III.2 precondition.
Obligations: an executor property for I.6.2 (two writes with distinct values before one
edge, the trailing region and the next head included: the last is observed and the first
never) and its sim ≡ SV probe, carried as anchor debt until a reference exists (the user
writes references, `ANCHORING_A_MODULE.md`); a unit pin that a write at `t_1⁻` is in `obs_1`
and that `obs_0(q) = init(q)`, once the harness observes `obs_0` (D-1 A1); the guard clause
of rule 11 merged from `claude/trailing-regout-guard-67cf37` or re-derived, with its ui/fail
case, its exact-set pin (`EXPECTED_TRAILING_REGOUT_INPUT`), the fifteen `e19*` probes and
their verdict table, and the three `…_diverges_admitted_recorded` pins flipped on purpose
when the II.3 forms land; `MEASUREMENTS.md` §C's row corrected (`sipo_block`, `handshake`;
not `bsg_counter_up_down`, `bsg_dff_en`); D-5's obligation line on `det_010_awaits` read as
"the shape, not the value". I.7 owes the register carrier and its edge — on the linear
routes a trailing-region assignment is realized at edge N, which is what makes the
committed-register form of a trailing `RegOut` drive an implementation of I.6.1 — and what
a register's value in cycle N is; I.9 owes `obs_0`, the pre-loop `RegOut` write under
extraction (`MEASUREMENTS.md` §B; corpus: the pipelined CPU's three sites) and the `initial
q = 0` transient; I.10 owes `q.write(m.data())` (M9) and the staging edge; I.12 owes the
value a module in another domain reads from a `RegOut`; I.13 owes the value a consumer
reads from a `RegOut` wire at each instant (the value committed at the last edge; a source
of the settle) and listener order stated as unobservable; II.1 takes one driver (W6) and
`RegOut` as an output for the signature check; II.2 restates rule 11 and the guard clause
as II.3 citations, never a clause of meaning, and reads "immediate site" as I.4.2's; II.3
carries the four dated rows above with their fix directions; III.2 states the precondition
`registered_wire(clock, zero)` and carries the anchor debt (the trailing position,
last-write-wins by value, forwarding into a write before a tick, power-on, the pre-loop
write). The corpus counts above are to be re-derived by `tools/stats` before III.3.
Coroutine terms (D-7): a `RegOut` write is a resumption's effect on the pending value;
the edge step between the two resumptions commits it (→ I.0.2).
Note (D-8, 2026-11-04): stands, and is now the output rule — every output of a sequential
module is a `RegOut`. A's "at any instant of cycle N" (I.6.1) reads "at the one instant
`t_{N+1}⁻`"; A1 (last write before one edge, trailing region and next head one segment)
stands; A4's input-sampling consequence is re-derived: `tick; q.write(x.read())` means `q`
takes `x_{N+1}` at edge N+1 — the flop's own sample — so A4-i's carrier flop and
`trailing_regout_reads_input` (its guard clause included) are vacuous and retired; A5's
`e19e`/`e19l` (a `let` carried across the tick) are registers of I.7 and `e19k` (a trailing
guard) is the one forwarded phase of the resumption — the three II.3 rows close with Phase
C's one-phase lowering, to be re-measured there; A7 stands trivially.

### D-7 — The module as a coroutine                                (2026-11-04)
Clause: SPEC §I.0 (new, before §I.1); one-line readings added to §I.1–§I.6 and D-1–D-6;
vocabulary edits to I.3.3 and I.4.2.
Question: What is the object whose behaviour a sequential module's meaning is — the
kernel function run as a coroutine with two suspension kinds (the `tick` and the barrier
before a deferred read) under the executor's resumption protocol, with the hardware
state machine as a projection of it; what are its state, resumptions, effects and trace;
what does each of D-1…D-6 read as in those terms; which of its propositions (registers
suffice, the projection, realizability) are definitions and which obligations; and where
are the decided text or the code inconsistent with it?
Docs say: SS "Execution model — c2 + just Rust" (`SYNCHRONOUS_SEMANTICS.md:18-26`: the
simulator executes plain Rust, "rustc's own coroutine lowering *is* the FSM", never an
IR; load-bearing for T6), `:740-744` (an FSM state is a node with an incoming `E_tick`,
defined independently of rustc's lowering — T1), `:766-767` (the register set is not
read off rustc's `Future` layout, "a conservative superset" — asserted, never measured);
IMPL `:77-97` (c2; an IR-interpreter simulator rejected for circularity). D-2 I.2.1 (the
object is the desugared `syn::ItemFn`; `match pc` an implementation form), I.2.5(b)
("the only suspension is the cycle boundary"). D-3 I.3.3 (residues "one per `tick`
site"; rustc's coroutine state, `pc`, `phase_r` implementation forms of the residue),
I.3.4 (the per-execution flip), I.3.8 ("until a deferred read parks it"). D-4 I.4.2 (the
deferred site is static), I.4.7 (the barrier is "the per-read freshness guard and
nothing else"). I.1.6 ("a deferred read parks at `t⁺` and resumes at `t⁻`").
FORMAL_SEMANTICS §3.2 (the barrier as the phase flag `φ`, "the device that realizes the
two instants", flipping once per cycle), §3.3 (`(Tick) ⇒ pause`), §3.4 (`K_N`), Lemma
2.1 "to be stated". CODE (1).5–11, (1).15–16, (3).1–4, (4).1–2, (4).8–10, (12).5–6,
(12).11, (12).13–14; MEASUREMENTS §B. No document states the module as a coroutine with
two suspension kinds; the barrier is everywhere an executor device, a realization or a
guard; `pc` is related to tick sites only by example (`branch_merge`).
Code does: `copper-macros/src/lib.rs:966`, `:977`, `:980` (every check on the pristine
fn; `check_awaits` admits only `clk.tick().await`, `copper-analysis/src/shape.rs:192-200`),
`:1365` (reads rewritten after the checks), `:878-888` (the body wrapped unchanged in
`HardwareModule::__new(async move #body)`; `copper-sim/src/executor.rs:48-63`),
`:712-725` (a `Deferred` site → `{ pre_edge_barrier::<D>().await; port.read() }`, tags by
source-order index, a missing tag a plain read) — so the coroutine rustc compiles is the
kernel body plus one `.await` per deferred read site. `copper-core/src/types.rs:913-921`
(`target = cycle + 1` fixed at `tick()`), `:1002` (Ready iff `cycle >= target` and
resolving), `:988-996` (no waker), `:981` (never-set domain resolving);
`copper-sim/src/lib.rs:101-117` (`PreEdgeBarrier`: Ready iff the domain's phase is
pre-edge, stateless), `:49` (never-set domain pre-edge), `:70-81` (`DeltaYield`).
`copper-sim/src/executor.rs:364-416` (nothing polled at spawn), `:836-840`
(`pre_edge_settle`), `:845-853` (`post_edge_settle`: `advance`, then poll),
`copper-core/src/types.rs:886-899` (`advance`: cycle += 1, listeners in registration
order), `copper-core/src/port.rs:173-183` (`RegOut` commit), `copper-core/src/memory.rs:73`
(memory listener), `executor.rs:546-558` (levelized: each singleton polled once per
settle), `:561-569` (asserts Pending: "a module's body is a `loop` that never
terminates"), `:520-531` (every domain's tasks polled in every settle). Read off `:1002`
and `lib.rs:107-114`: post-edge, the coroutine parked at a tick with `target = N` is
resumed and runs to a barrier (Pending: post-edge) or the next `tick()` (Pending:
`target = N+1`); pre-edge, one parked at a barrier is resumed (Ready) and runs through
any further barrier (Ready) to the next tick (Pending: resolving off); one parked at a
tick runs nothing; the first resumption is the first pre-edge settle, from the entry; a
fixpoint re-poll of a Pending leaf runs no statement. Effects: `port.rs:86-92`
(`Out::write`), `:194-196` (`RegOut::write` → `pending`), CODE (7).8–9 (stagings).
`copper-analysis/src/cfg.rs:399-411` (`compute_registers`: defined-in-loop ∧ live-out at
a tick node or at the head, `:409`), `:2600-2632` (liveness), `:287-305` (the CFG from
the pristine fn; the barrier is not a node), `:3082-3117` (a tick-bearing nested loop
folded). `src/sync.rs:69-83` (`q.write(ff2); tick; ff2 = ff1; ff1 = d.read();` — `ff2`
assigned and consumed within one post-edge resumption; live at the tick suspension:
`{ff1}`). `copper-codegen/src/control_extract.rs:1208-1358` (`lower_into`), `:1217` (a
state per tick encountered), `:1213-1215` (a trailing tick is `pc = head`), `:1310`,
`:1315`, `:1333` (the continuation after a diverging `if`/`match` appended to every arm
and lowered per arm — one site, one state per arm: `pc` per occurrence, many-to-one),
`:914-960`, `:928` (hoisting by `stmts_mention`, not liveness), `:526-549` (pre-loop
expression statements into state 0; MEASUREMENTS §B). `copper-codegen/src/shir_lower.rs:184-196`,
`:315-338` (`seen_in_read`: a static arm-granular walk; `classify_reads` consumed nowhere
in codegen — G1), `:873-883` (`split_at_ticks`). `copper-codegen/tests/register_reconciliation.rs:19-23`,
`:72` (flops ≡ `infer_registers` + {`phase`, `pc`, `_hold`}); no test compares `pc`
states with tick sites; the sweep compares ports. `tests/common/mod.rs:827`, `:860` (the
third leg's insertion point).
Anchor: none — decided (a definition). Its observable consequences are I.1–I.6's
anchors: `sync_2ff` vs `sync_2ff_ref.sv` (one cycle's two resumptions composed: two
flops, one cycle of latency); `bsg_dff_en`, `bsg_counter_up_down`, `det_010` (the
post-edge resumption's effects at `obs_N`); `det_010_awaits`, `lfsr`, `shift_register`,
`sipo_block` (the pre-edge resumption reads `x_{N+1}`, commits at edge N+1). The
protocol as such has no anchor: every shape where a park would be observable through a
port is refused (I.4.8, debt 12). H(M) against the emitted FSM is sim ≡ SV only (the
sweep, `register_reconciliation.rs`, the four G2 structural matches).
Prior work: Prost [prost §2–§3] — a coroutine "is" a single-clocked synchronous
circuit, one resumption per edge, every local a register, every output `.next`
(= `RegOut`), `advance : State × In → State`; no plain output, no pre-first-edge
execution, no sub-cycle instant; "not perfect", no semantics; de Moura & Ierusalimschy's
asymmetric stackless coroutine cited for the host construct. Esterel [esterel §6.5.6,
§9; note items 2–3] — states are derivatives (the residue after each reaction), FSM
extraction is derivative enumeration, a value boxed in the derivative is state (the
declarative "register" I.0.5 adopts). SystemC [systemc §4.2.2] — a thread resumes after
the edge and runs to the next `wait()`, one suspension kind. MyHDL [myhdl §3.1, §4.2] —
a generator resumes once per `yield`; the synthesizable subset allows one, at the loop
head, so no local crosses a suspension. Golden Age (the Silica paragraph) —
"restricting the coroutine semantics to describe only FSMs" is the open problem the
shape principle answers. Survey §5 item 1 — one deterministic coroutine, no
interleaving. No precedent for a coroutine with two suspension kinds of which one is
intra-cycle, nor for a cycle transition that composes two resumptions.
Options:
  A. Coroutine-as-definition: a sequential module is the kernel run as a coroutine
     whose suspension sites are its `tick` sites and its barrier sites (one per deferred
     read site), state = (site, locals, control position), a resumption runs to the next
     site, effects = `Out` writes, `RegOut` pending values, memory stagings; the
     protocol per tick = pre-edge resumption from a barrier to the next tick (a barrier
     reached pre-edge suspends nothing), the edge (pending values commit, memories
     advance), post-edge resumption from the tick to a barrier (park) or the next tick;
     the first resumption is the pre-edge step of edge 1 from the entry; `obs_N` after
     the post-edge resumption, `obs_0` after the first; H(M) = one state per tick site,
     variables registers ∪ `RegOut` ∪ memories ∪ the plain-`Out` wires' values, the
     cycle transition the composition of the two resumptions; `pc`/`phase_r` encodings
     of it, many-to-one; the register proposition (a)/(b)/(c); the three admissibility
     principles; rustc's lowering the simulator's exact realization; the interpreter
     the executable form — changes: none to sim, macro or codegen (the macro passes the
     body through, the executor already is the protocol); SPEC gains §I.0, six
     cross-reference lines, the I.3.3 and I.4.2 vocabulary edits; corpus cost 0 — no
     rule's verdict and no module's trace changes, since I.0 names the simulator's
     behaviour as it is and refuses nothing new.
     A1-i. The register boundary: a register is a local live at a `tick` suspension of
         the coroutine; `sync_2ff`'s `ff2`, the `<port>_hold` registers and the input
         carrier D-6 A4-i owes are carriers of the realization (I.0.5(b)), named in III
         for the forms the transpiler has and in II.3 for the one it lacks; REG9's
         back-edge clause and `infer_registers` compute (a) ∪ the trailing carriers; the
         poison run decides `ff2` empirically — changes: none; I.7 states the split;
         corpus cost 0.
     A1-ii. A register is `infer_registers`' set with the back-edge clause as the
         definition (`ff2` a definitional flop); the sufficiency proposition is then
         stated over the CFG, not the coroutine, and the barrier carriers remain a
         separate, unexplained category — changes: none; corpus cost 0.
     A2. I.3.3's list of carried state gains "(v) the values of its plain `Out` wires"
         (I.5.4), which H(M) needs and I.3.3 omitted — a minimal in-place edit marked
         D-7; or I.0.4 alone carries it.
  B. The barrier as an instant flip only (FORMAL_SEMANTICS §3.2's `φ`): one suspension
     kind, the deferred read a flag flip inside a big-step cycle — contrary to the
     user's sub-decision 1 (2026-11-04); the model has no state for a coroutine parked
     mid-segment, so it cannot express the first resumption from the entry, a barrier
     reached pre-edge (Ready, no flip back), a barrier keyed to the port's domain, or
     the `Barrier` state the interpreter needs — changes: none; corpus cost 0.
  C. Projection by `pc` occurrence: H(M)'s states are the extracted FSM's — reverses
     D-2 (the `match pc` becomes meaning) and I.3.3; makes H(M) depend on extraction's
     continuation duplication (one site, several states) and undefined for the 104 of
     135 clocked modules on the linear route, which have no `pc` — changes: none; corpus
     cost 0.
Recommendation: A with A1-i and A2 — the sentence the macro and the executor already
are; D-1…D-6 re-read as six consequences of one protocol with no reversal; the
interpreter gets its state space; what the coroutine carries (registers) is separated
from what a realization must add (carriers), which is where every remaining II.3 row
lives.
Decision: **A, with A1-i and A2 (the I.3.3 edit).** User, 2026-11-04. B refused (contrary
to sub-decision 1; cannot express a parked coroutine); C refused (reverses D-2).
Changes: none to sim, macro or codegen now. SPEC: §I.0 inserted before §I.1 (I.0.1–I.0.8,
no TBD); a one-line "In coroutine terms" reading after the `Decided:` paragraph of each of
§I.1–§I.6 and a `Coroutine terms (D-7):` line appended to each of D-1–D-6; I.3.3 edited in
place — residues "one per suspension site of either kind" (a barrier residue intra-cycle,
never a hardware state) and the wires' values added as (v); I.4.2 edited in place — a
deferred site is a barrier site, static, whose park is dynamic. Part I uses "coroutine",
"suspension site" (tick site, barrier site), "resumption" (pre-edge, post-edge), "park",
"effect", "projection H(M)", "register" (I.0.5(a)) and "carrier" (I.0.5(b)) from here on;
"FSM state", "`pc` state", "phase", "freshness guard" and "flop" are Part II/III words.
I.2.5(b) is read as a statement about what the user writes (the only `await` is `tick`);
not edited. FORMAL_SEMANTICS §3.2's `φ` is superseded by I.0.2 when SPEC is adopted. The
pre-loop re-execution under control extraction is reclassified from an open meaning to a
transpiler defect (I.0.4), owed to I.9 and Phase C; the NOT-YET rows of I.2.8, I.3.9,
I.5.10 and I.6.10 close when I.9 is written.
Obligations: III.1 — a static test that the extracted `match pc` encodes H(M) (`pc`
states against tick sites, beside `tests/timing_model_derivations.rs`, plus the
interpreter's reached-site set); a measurement that `hoist_cross_state_locals`' set ⊆
registers ∪ carriers on the corpus (today bounded only against `infer_registers` by
`register_reconciliation.rs`); the poison run for I.0.5(a) — at every tick suspension
overwrite every local not in the register set, a poisoned read is an under-inference —
with `sync_2ff`'s `ff2` the first case; rustc's layout as a superset of (a)
(`-Zprint-type-sizes` on a few fixtures, recorded in `review/MEASUREMENTS.md`). III.2 —
the reference interpreter (`copper-ref`: state `Entry | Tick(site) | Barrier(site)`,
steps `pre_edge`/`edge`/`post_edge`/`observe`, barrier sites from `classify_reads` and
an independent may-reach walk asserted equal) as the executable form of I.0.1–I.0.3 and
the third sweep leg with a guard G-F, its first measurements the pre-loop re-execution
probe (ref ≡ sim ≠ SV), the `pc`-vs-site test and the poison run; the levelized
precondition that a re-poll runs no statement; `poll_tasks` outside a tick. I.7 (packet
8) — instantiate I.0.5: "register" = (a), the carriers of (b) named with the form that
realizes each (`ff2` and the trailing-region carriers on the linear routes at the
opening edge, `_hold` at the edge, the D-6 input carrier owed to II.3), power-on values,
`Bits::x()`, what "a register's value in cycle N" is relative to the post-edge
resumption's assignments (D-3/D-4/D-6's obligations). I.8 — loops, `break`, `continue`
and the zero-iteration loop as resumptions that reach or do not reach a tick site. I.9 —
`s_pre` as part of the first resumption, `obs_0`, the F1 transient, and the extraction
re-run as a Phase C defect. I.12 — per-domain gating (every domain's coroutines polled
in every settle; the never-ticked default; a barrier keyed to the port's domain). I.10
— memory stagings in the effect set and the edge step. I.11 — the combinational
coroutine, one evaluation per settle. II.1 — the shape principle's four clauses as the
rules' citations (`check_awaits`, `check_reachability`, finite state, never returns).
II.2 — the design principle; the direction of each static predicate against the park
(`leading_read_reaches`, `deferred_read_reaches`, `seen_in_read`; G1). II.3 — every
residual refusal sorted as a dated limitation with its fix direction under the
realizability principle. III.1 — the propositions above; III.2 — the agreement relation
over the three legs.
Reversed by D-8 (2026-11-04) in one part: A's two suspension kinds — one kind, the `tick`
site; the barrier site, the pre-edge/post-edge pair and the park go (I.0.1, I.0.2). What
stands: coroutine-as-definition, state = (tick site, locals, control position), H(M) with
one state per tick site and the cycle transition now one resumption over `x_{N+1}` (I.0.4;
the plain-`Out` wires leave its variables and I.3.3(v) — A2 reversed with D-5), the register
proposition A1-i with the barrier carrier of I.0.5(b) deleted — and, by the one-resumption
reading, `_hold` (a `RegOut` holds) and the D-6 input carrier gone too, `sync_2ff`'s `ff2`
a wire of one resumption rather than a carrier (dead at the `tick`; the second flop is the
`RegOut`); the three admissibility principles, realizability read as "the resumption's
effects"; the interpreter's state space `Entry | Tick(site)`. Option B of this entry (the
barrier as a flag) is not what D-8 adopts: D-8 removes the second instant, it does not
re-encode it.

### D-8 — The one-job body (Q1)                                    (2026-11-04)
Clause: SPEC §I.0–§I.6 re-derived in place (every edit marked `(D-8)`); §I.5 superseded as
a whole; §I.11's heading takes the wire rules of §I.5. Reverses D-1 A, D-4 A1 and A2-i,
D-5 A1–A6, D-7 A (the barrier); notes on D-2, D-3, D-6. `SPEC_OVERVIEW.md` §2 rewritten.
Question: Does a sequential body do two jobs in one cycle — drive outputs from the state the
edge produced at `t_N⁺`, then sample inputs for the next state at `t_{N+1}⁻`, the boundary
set per execution by the first deferred read (A, today) or fixed statically (C) — or one job:
one resumption per cycle, at the edge, every read seeing that edge's input, every output a
register (B, Prost)?
Docs say: SPEC_OVERVIEW §2 (the two-job body as "the one choice that makes Copper hard";
Q1 open) and §4 choices 1–3 (two instants, the barrier placement, the plain-`Out` hold);
SPEC §I.0–§I.6 as decided by D-1…D-7 — I.0.1 two suspension kinds, I.0.2 the two
resumptions and the park, I.1.2 two instants, I.3.4 the per-execution instant, I.4.2
deferred reads, I.5 the plain-`Out` wire; DOCS T3–T9 (four tick phases), S4/S16 (opening
prefix / closing suffix at the barrier), IN2/IN3 (deferred iff a tick follows), OUT1–OUT19
(the forms), the D1 family (PRETICK_ALIGNMENT_GUARDRAIL §5.8, CDS §9–§10.12).
`review/DECISION_AUDIT.md`: D-1's refusal of the one-job body (D-1 option B) rested on the
passthrough anchor that D-4 A2-i removed, so the refusal had no reason on record.
Code does: the barrier — `copper-sim/src/lib.rs:101-117` (`PreEdgeBarrier`: Ready in the
pre-edge phase), `copper-sim/src/executor.rs:813-853` (a tick = pre-edge settle, `advance`,
post-edge settle: the two resumptions); the macro — `copper-macros/src/lib.rs:686-729`
(`ReadRewriter` injects `pre_edge_barrier` before each deferred read; `classify_reads`,
`copper-analysis/src/cfg.rs:3804`, places them); the transpiler — the opening/closing
tagging `copper-codegen/src/shir_lower.rs:335-386` and the five plain-`Out` forms in
`copper-codegen/src/vlir_lower.rs` (F1 `:738-741`, F2 `:369-372`, F3 `:519-541`, F4
`:1579-1600`, F5 `:1530-1566`); the eight alignment rules of `review/INVENTORY_RULES.md` §1
(`unprotected_pretick_out_write`, `pretick_out_write_before_update`,
`multi_phase_out_write`, `linear_out_write_anchor_mismatch`, `trailing_out_reads_input`,
`trailing_regout_reads_input`, `out_first_driven_late`, `multi_write_collapse`) and the
opt-out `allow_pretick_alignment`. `review/Q1_TRADEOFFS.md` §1 and §6 (measured
2026-11-04; the temporary scan, fixtures and tests were removed after the runs).
Anchor: every shipped B rewrite is trace-identical to its independent reference
(`review/Q1_TRADEOFFS.md` §4, 200 cycles of LCG stimulus, the anchor's own port names;
every verdict predicted before the run): `det_010` → `RegOut` of the next state vs
`pattern_detector_010.sv`; `bsg_dff_en` vs BaseJump `bsg_dff_en.sv` (the enabled register:
identical SV, 8 cells / 8 FF on both); `bsg_counter_up_down` vs BaseJump
`bsg_counter_up_down.sv` (the trailing read moved before the tick: identical SV, 16/3);
`sipo_block` with `wN_dbg` as `RegOut` vs BaseJump `sipo_block.sv` (`data_o`; 37/30,
unchanged); `vr_sink_mealy` as `RegOut` of the next `full` and `drain` vs the refused
module's own emitted `assign ready` (identical on `ready` and `sum`), and with a module
boundary (`q1_b_vr_sink_core` + the `combinational` `q1_b_vr_ready`, 54/17 = the Mealy
hardware, at no cell cost). `det_110101` → B differs from `pattern_detector.sv` at cycles 15
and 271 — and so does the shipped spelling (the control, `q1_a_det_110101`): the golden's
"Bug 2" arm, not Q1 (obligation below). The one measured A-only divergence: the
composition of §3/§4 (e) — A's trailing `tick; o.write(x.read())` passthrough, emitted as
`assign o = x`, feeding a register in a second module diverges from cycle 0 (the simulator
shows `x_{N−1}` where the hardware shows `x_N`; the harness holds the input across the edge,
so the single-module sweep passes it and no rule refuses it); the flop form and the B
registered passthrough agree. Under B the shape cannot be written.
Prior work: Prost [prost §2–§5, §8.3] — one resumption per positive edge, every local a
register, every output a `.next`-written register visible after the next `wait`, inputs read
in the edge-evaluated `advance` function, the opening segment run at the first edge: the
one-job model verbatim. Spade [spade §3, §9] — `reg` + body as an expression, an output a
combinational function of registers with no point of read (A's trailing Moore drive, B's
`combinational` module). Chisel [chisel §3, §9], CλaSH [clash §3, §9], Bluespec
[bluespec §3] — a Mealy step function `(S, I) → (S, O)` evaluated once per cycle, one value
per signal per cycle, combinational outputs allowed: one instant *with* comb outputs, neither
A nor B. Esterel [esterel §3, §9] — one instant per reaction, outputs synchronous with
inputs; the barrier has no analogue. SystemC's synthesizable subset [systemc §3.1, §8.2] —
`wait()` = edge, an `sc_out` write visible before the next edge (A's hazard, left
unspecified; HLS tools register thread outputs: B in practice). MyHDL [myhdl §3.4, §5] —
`always_seq` with `.next` writes is B at the boundary. No surveyed system has A's dynamic
two-instant body; C has no precedent either.
Options:
  A. Two jobs, dynamic boundary (today; `Q1_TRADEOFFS` §1): after edge N the post-edge
     resumption runs the trailing region and the head to the first deferred read at `t_N⁺`
     reading `x_N`, parks at that read's barrier, and the pre-edge resumption runs the rest
     at `t_{N+1}⁻` reading `x_{N+1}`; which instant a statement runs at is per execution
     path; a plain `Out` may be written at either instant, realized by one of five forms,
     the unrealizable shapes refused by the eight alignment rules as dated lowering
     limitations — changes: none; corpus cost: 0; keeps the composition hazard of (e) (no
     rule covers the single-tick trailing `assign`), 8 rules, 5 forms, the barrier, and the
     per-execution instant as the definition of a write's time.
  B. One job (Prost): the body resumes once per cycle; the resumption that ends at edge N+1
     reads `x_{N+1}` at every read site; every output of a sequential module is a `RegOut`,
     a plain `Out` is not a parameter of a sequential module; a combinational output is a
     separate `combinational` module wired to `RegOut`s; the barrier disappears, a cycle has
     one instant, the trailing region is the head of the next resumption — changes (§6
     column B): sim — `PreEdgeBarrier` and the second resumption go (`lib.rs:101-117`,
     `executor.rs:813-853`: one resumption per tick, from the tick site to the next), plain
     `Out` wiring for sequential modules removed; macro — `ReadRewriter` injects nothing,
     the signature check refuses `Out` in sequential mode, `classify_reads` dead; codegen —
     F1–F5 for sequential plain `Out` go, every port the `split_output_regs` flop,
     `shir_lower`'s opening/closing tagging goes; refuse — the eight alignment rules and the
     opt-out retired, one new signature rule; corpus cost: 19 shipped + 69 probe modules
     rewritten (88 of 135 clocked modules, every one with a plain `Out`; the shipped
     rewrites 2–8 body lines each), the 62 refused probes of
     `tests/sequential_forwarding_divergence.rs` unexpressible; cost in hardware: one flop
     per Moore-output bit written inside a body (`det_010` 9/2 → 10/3, `det_110101`
     24/3 → 24/4) or a module boundary; the enabled register, the counter and `sipo_block`
     unchanged (the paper's two exact clocked cell-count matches survive). Not recoverable
     by an extra flop: a Mealy output inside the clocked body (it moves to a `combinational`
     module, measured at no cost, §4 (d')).
  C. Two jobs, static boundary: both instants stay; a plain `Out` may be written only in
     the comb job (the trailing region, or a head segment before any deferred read on every
     path), so F3/F5 disappear and F1/F4 stay; the first deferred read of a segment is at the
     same program point on every path — changes: macro gains two semantic checks (closing
     write; path-dependent instant) on `Cfg`; codegen loses F3/F5; D-4 gains the static
     clause, D-5 loses two forms; rules — three go (D1 clause A, `multi_phase_out_write`,
     the closing half of the linear rule), five survive as comb-job limitations, two new
     semantic ones, plus a sixth needed to close (e) (the single-tick extension of
     `trailing_out_reads_input`); corpus cost: 10 shipped + 18 probe modules rewritten or
     hoisted (28 of 135); the first C rewrite (`sipo_block`, `w3` carried across the back
     edge) was admitted and diverged — a transpiler forwarding bug for a register read
     through an array-element assignment, reachable under A today.
Recommendation: B — every shipped rewrite measured trace-identical to its independent anchor
at zero cell cost on the anchored designs, while the one hazard the packet measured (the
trailing combinational drive sampled once, wrong in composition) and all eight alignment
rules exist only because a clocked body may drive a wire; the Mealy output, the one thing B
cannot spell inside the body, is what the project already decided against for handshakes
(2026-10-21) and is a `combinational` module at no cost; the price is one flop per Moore
output bit, the 19 + 69 rewrites, and a smaller daylight from Prost in the paper.
Decision: **B, the one-job body.** User, 2026-11-04, after the packet and the question
whether B is too close to Prost — the user's answer: the one-job model is shared with Prost
and must be cited; Copper's daylight is the realization (rustc's own async lowering as the
FSM, the simulator runs plain Rust), the liveness-based register inference, the written
semantics, and the measured, anchored sweep.
Changes: Phase C obligations (`Q1_TRADEOFFS` §6 column B), none now:
  simulator — `PreEdgeBarrier` and the second resumption go; a tick becomes one resumption
  per cycle, from the tick site the coroutine is parked at to the next tick, run after the
  environment's drive and before the edge it ends at (the instant `t_{N+1}⁻` of I.1.2; every
  measured B rewrite ran under today's executor with every read deferred, so the trace it
  must reproduce is the pre-edge one); the post-edge settle resumes no coroutine; plain
  `Out` wiring for sequential modules removed;
  macro — `ReadRewriter` injects nothing; the signature check refuses `Out` as a parameter
  of a `sequential` or `synchronizer` module (the one new shape rule, II.1), with
  `allow_pretick_alignment` refused as a flag; `classify_reads` and the eight alignment
  rules are dead and removed, together with their ui/fail cases and corpus pins
  (`copper-analysis/tests/pretick_alignment_corpus.rs`, `hold_form_audit.rs`,
  `phase_sensitive_checks.rs` re-scoped to what is left);
  transpiler — forms F1–F5 for a sequential plain `Out` go; every port is the
  `split_output_regs` flop (`q <= edge_value` under the state guard, `initial q = 0`);
  `shir_lower`'s opening/closing tagging (`:335-386`) and `trailing_reg_outs` (the
  committed-value drive of a trailing textual segment) go — one phase per resumption, every
  drive forwarded; whether the linear route keeps its trailing fold as an encoding is III's;
  corpus — the 19 shipped modules (`bsg_dff_en`, `bsg_counter_up_down`, `det_010`,
  `det_110101`, `lfsr`, `shift_register`, `simple_counter`, `sipo_block`, `traffic_light`,
  `fast_counter` ×2, `flag_sync`, `dual_port_ram`, `rv32i_cpu_transpilable`, `sync_2ff`,
  `event_source`, `event_sink`, `slow_consumer` ×2) and 69 probe fixtures rewritten by port
  kind plus the tick at the loop's end, a Mealy output as a `combinational` module; the 62
  refused probes of `tests/sequential_forwarding_divergence.rs` become unexpressible and the
  battery is retired with the rules (its agreeing `RegOut` rows kept where they still
  compile); the `d*`/`e*` verdict tables and `MEASUREMENTS.md` §C's plain-`Out` debts
  (1, 2, 4, 5, 7, 11, 12) close as vacuous;
  spec — D-1 A reversed (one instant per cycle, `t_{N+1}⁻`; the drive–tick–observe
  environment, `obs_0` in the trace and per-domain numbering stand); D-4 A1 and A2-i
  reversed (no site is deferred — every read sees the edge's input; the passthrough refusal
  vacuous); D-5 A1–A6 reversed as a whole (the plain `Out` leaves the sequential module; the
  wire's last-write-wins and its zero initial value move to I.11 for the combinational
  module; hold is D-6's I.6.3); D-7 A reversed in one part (one suspension kind; the
  barrier site goes; H(M) and the register proposition stand, the barrier carrier of
  I.0.5(b) deleted, the plain-`Out` wires leave H(M)'s variables and I.3.3); D-2, D-3 and
  D-6 stand with notes (a segment is one resumption; the trailing region is the head of the
  next resumption; D-6 is the output rule and "any instant" reads "the one instant").
  II.3's alignment list empties; the realizability principle of I.0.6 reads "the emitted
  text must realize the resumption's effects". SPEC_OVERVIEW §2 is "The one choice,
  decided"; §4 loses choices 2 and 3 and choice 1's dependence on Q1.
Obligations: the 19 + 69 rewrites measured again under the sweep after Phase C (sim ≡ SV,
five stimulus sequences each; the five anchored ones against their references again);
`det_110101` vs `pattern_detector.sv` at cycles 15 and 271 — the golden's "Bug 2" arm goes
to `S1`, the Rust `_ => IDLE`; owed to the example, not to Q1; the `sipo_block`
static-boundary transpiler bug of `Q1_TRADEOFFS` §4 (c''') — a register assigned in a
segment and read in the same segment through `bits[12+k] = w3[k]` is read as its committed
`Q` in the emitted `always_comb` while the simulator forwards it — reachable under A today
and independent of the port kind, carried to Phase C (a register-forwarding bug of I.7's
realization) even though C was not chosen; the trailing-combinational composition hazard of
§4 (e) (`q1_a_pass` → `q1_sink`: sim lags hardware from cycle 0) kept as the pin that B
closes — re-run after Phase C as a composition case that must now be refused at the
signature (a plain `Out` on a sequential module) and agree in its `RegOut` form; the paper's
citation of Prost for the one-job model, with the daylight as the user stated it (flag only;
the paper is the user's); I.7 (packet 9) to instantiate the register proposition under one
resumption (`sync_2ff`'s `ff2` is a wire of one resumption, not a register; the second flop
is the `RegOut`); I.11 to take the wire rules of I.5; `review/INVENTORY_RULES.md` §1 to be
re-cut after Phase C (eight rows retired, one signature rule added).
Coroutine terms: the coroutine has one suspension kind; a cycle is one resumption; the
one-job body is Prost's model, realized by rustc's own lowering and checked by the sweep.

### D-9 — Registers                                                (2026-11-04)
Clause: SPEC §I.7 (new, replacing the TBD heading); `(D-9)` notes appended to §I.0's
Decided paragraph and to I.0.5(c).
Question: Under the one-job body, which locals of the kernel are registers — what "live at
a `tick` suspension" means on an execution, and what is instead a wire of one resumption or
a constant; what a register's value is in the resumption that ends at edge N+1; what its
power-on value is for a constant initializer, for `Bits::x()` and for a register born
inside the loop; whether `infer_registers`' back-edge clause still computes anything of
I.0.5(a); and what I.7 owes I.9, I.10, I.11, I.12, I.14, II.1 and III.
Docs say: DOCS REG1 (a local is a register iff defined in the loop and live across some
`E_tick`), REG2 and REG9 (the back-edge clause, 2026-08-21: a value defined after the last
tick and read before the first is a register — `sync_2ff`'s `ff2`), REG3 (a pre-loop
binding never assigned in the loop is a wire), REG4 (a same-cycle temp is not a register),
REG5 (the minimal synthesizable set, not rustc's `Future` layout — "a conservative
superset", asserted and never measured), REG6 (registers stay plain locals, no
current/next split), REG7 (codegen consumes the shared set plus the names it synthesizes;
promotes in-loop wires live across a tick itself), REG8 (validated reg-for-reg against four
hand-written SV references and against codegen's flops over the fixtures), REG10 (control
extraction hoists a cross-state local into a flop), REG11 (tuple targets and loop-born
registers across an interior tick); PO1 (power-on = the binding's initializer; `initial` in
the SV), PO2 (a foldable constant), PO3 (`Bits::x()` undefined; a reset needed; the sim
carries X where Verilator reads 0), PO4 (a non-constant initializer refused, both
front-ends, no opt-out), PO5 (a loop-born register "has no power-on value"), PO6
(synthesized state at 0), PO9 (`initial` honoured by simulators and FPGA tools, ignored by
most ASIC flows), PO10–PO11 (the plain-`Out` startup rule, retired with D-8); X9 (a
synchronizer's trailing `ff2 = ff1` "must be a flop"); A3 (one CFG, both front-ends). SS
"Registers — the liveness rule", "Register power-on values"; FORMAL_SEMANTICS §2 (the CFG
definition "including the back edge"; Lemma 2.1 "to be stated"). Contradictions: REG2,
REG9 and X9 against I.0.5(b) as re-derived by D-8 (`ff2` a wire of one resumption) —
settled by measurement below, the docs right for the two-job body and wrong for the
one-job one; PO5 says neither X nor "unreadable"; contradiction 8 (AAM: no `initial`
blocks) and 9 (`lfsr`'s initializer "not settled") are superseded by the 2026-10-29
decision; gap 18 (the value before the first edge is undefined except for register
initializers) → I.9; gap 27 ("word-indexed array registers" without a stated meaning) is
the array-local case. §I.0's Decided paragraph says `sync_2ff` was re-measured in its
`RegOut` spelling in `review/Q1_TRADEOFFS.md` §4; §4 measures five modules and `sync_2ff` is
not among them — measured in this packet instead (Anchor).
Code does: `copper-analysis/src/cfg.rs:394-415` (`registers` / `compute_registers`:
`defined_in_loop` ∩ (live-out at every tick node ∪ live-out at the head); the head clause
`:408-409` is REG2), `:355-393` (its docstring: the clause "newly admits only values
defined in a post-tick segment and read before the next tick"), `:3402-3440`
(`DefinedInLoop`: `let` bindings, assignment targets through `Index`/`Field`/`Tuple`
(`assign_targets` `:3443-3455`), a ticking `for`'s binding; match-arm patterns excluded),
`:3062-3119` (a tick-bearing nested loop is ONE folded node, `defs` empty, interior reads
as `uses`, out-edge `Tick`), `:2600-2632` (backward liveness, edge kind ignored),
`:6503-6525` (the `sync_2ff` unit pin `["ff1", "ff2"]`). `copper-analysis/src/lib.rs:95-98`
(`infer_registers`), `:51-56` (`check_register_initializers`: pre-loop `let`s in
`registers()` only), `cfg.rs:3519-3550` (`is_constant_expr`), `lib.rs:391-425`
(`assert_source_registers_match_reference_sv`, `NameExact` / `StorageEquivalent`).
Consumers: the macro — `copper-macros/src/lib.rs:990` (`check_register_initializers_with`)
and `:1357` (a debug log); nothing else, the simulator needs no register set (rustc keeps
every local, I.0.5(c)). The transpiler — `copper-codegen/src/parser.rs:68` and
`lib.rs:552` (`fir.registers` = the shared inference, consumed by `chir_lower` for the
pre-loop `let mut`s); loop-born registers are realized by two mechanisms independent of
the inference: `shir_lower::find_promoted_wires` (`copper-codegen/src/shir_lower.rs:1495`;
a wire referenced in another phase, promoted to `<name>_r`, powered on at 0 when it has no
literal initializer, `:612-625`, CODE 33) and `control_extract::hoist_cross_state_locals`
(`copper-codegen/src/control_extract.rs:914-961`: a local mentioned in another state is
hoisted to a pre-loop `let mut` whose initializer is kept for its type only,
`HOISTED_CROSS_STATE_ATTR`; by name mention, not liveness). Power-on:
`chir_lower::lower_register_init` (`copper-codegen/src/chir_lower.rs:1158-1198`: the
initializer folded, `Bits::x()` → no init, a non-foldable one refused with the rule-5
message, a value that does not fit refused), `emit::reg_inits` (`emit.rs:418-432`: one
`initial` block, registers with an init then `port_inits` at 0; a register with no init
gets no line — CODE 27, 34), `build.rs:206-230` (`RESET`: `shift_register`,
`simple_counter` from `Bits::<8>::x()`, `rv32i_cpu_transpilable`), `COPPER_VERILATOR_X_INITIAL`.
`copper-codegen/tests/register_reconciliation.rs`: fixtures only, sequential and
synchronizer; codegen flops − inferred ⊆ {`phase`, `pc`, `pc_<n>`, `__copper_ctr…`,
`<port>_hold`, memory pipeline stages, struct fields}; a single-phase module may keep an
inferred register as a comb wire ("absorption"); WITNESSES empty. **Run 2026-11-04: 105
clocked modules, codegen ≡ shared inference, no violation.**
Measured 2026-11-04 (a temporary `Cfg` method splitting the two clauses, a scan test, two
fixtures through the sweep and a temporary anchor test — all removed, `cfg.rs` restored;
every verdict predicted before the run):
  (1) the back-edge clause's surplus over the tick clause, corpus-wide (135 clocked
      modules in `examples/`, `tests/fixtures/`, `src/`) — predicted at most ten modules,
      every surplus local assigned after the iteration's last tick, read in the next head
      before its first tick, dead at every tick; measured **7 modules, 8 locals, all that
      shape**: `ff2` (`src/sync.rs::sync_2ff`, `two_domain_hierarchy.rs::flag_sync`,
      `sync_2ff_dut.rs::sync_2ff_concrete`), `w` (`trailing_local_into_head`, whose own
      doc comment calls it "a wire and not a register"), `latch` (`struct_latch`,
      `struct_latch_cond`), `latch` and `target` (`tuple_destructure`);
  (2) probes (tick-clause set / head-clause surplus): `sync_2ff` in both port spellings
      {ff1} / {ff2}; a trailing assignment read in the head {} / {v}, also when the head
      reads it on one path only; an accumulator with a trailing temp {acc} / {}; a
      loop-born `let` across an interior tick {p} / {}; an array element write
      `arr[1] = w; tick; q.write(arr[0])` {arr} / {}; `tick; (a, b) = (b, x.read())` read
      in the head {b} / {a}; a pre-loop `mask` never assigned: not a register; a local
      both head-read and tick-live {v} / {} (the clause adds nothing it did not have);
  (3) two defects of the nested-loop fold, neither of the clause: `loop { let v =
      x.read(); for i in 0..4 { q.write(v + i); tick } }` infers {i} and MISSES `v`, which
      is live at the inner tick (the transpiler emits a flop `v` by hoisting, so sim ≡ SV
      holds; no fixture has the shape, so the reconciliation test never met it); `loop {
      q.write(v); tick; for i in 0..2 { tick; v = x.read() } }` infers `v` although it is
      dead at every tick (over-inference, harmless);
  (4) `sync_2ff` in its one-job `RegOut` spellings under TODAY's simulator and transpiler:
      B1 `q.write(ff2); tick; ff2 = ff1; ff1 = d.read()` — inferred {ff1, ff2}, emitted
      flops `q`, `ff2`, `ff1`, sweep PASS (both legs read `d` at `t_N⁺`), against
      `sync_2ff_ref.sv` **one cycle late at six of sixteen cycles**; B2 `ff2 = ff1; ff1 =
      d.read(); q.write(ff2); tick` — inferred {ff1}, emitted `q <= ff1; ff1 <= d`, sweep
      PASS, against the reference **identical**. B1 is trace-identical only under Phase C's
      resumption at `t_{N+1}⁻`, where `ff2` is the wire B2 already emits.
Anchor: the register set against independent hand-written SV — `mac_fsm` (`NameExact`),
`det_010`, `det_110101`, `lfsr` (`StorageEquivalent`), through
`assert_source_registers_match_reference_sv` in their equivalence tests; `sync_2ff` vs
`examples/cdc/sv/sync_2ff_ref.sv` (today `NameExact` {ff1, ff2}; under the decision the
reference's flops are the registers ∪ the `RegOut` ports = {ff1} ∪ {q}, and the one-job
spelling B2 is identical to the reference over the anchor schedule, measured above);
`lfsr`'s `Bits::from_u32(1)` at the first observation (`tests/lfsr_equivalence.rs`). Sim ≡
SV only: `register_reconciliation.rs`; loop-born and tuple registers; array locals; the
Q1 §4 (ii) array-element forwarding bug (`bits[12+k] = w3[k]` reads `w3`'s committed `Q` in
the emitted `always_comb` while the simulator forwards it). B (below) is refuted by (4).
Prior work: no precedent for liveness-inferred registers whose constant initializers are
stated as `initial`. Prost [prost §2, §8.2] — every local a register, dead locals re-zeroed
(Listing 2: the cost of not inferring). Esterel [esterel note items 2–3] — the registers are
the values the derivative boxes; liveness is an implementation of that declarative
criterion (the form I.7.1 takes, on executions). Spade [spade §4] — `reg;` registers every
value visible above it, use-before-ready refused: visibility, a superset of liveness.
Chisel [chisel §4] — explicit `Reg`, `resetVal`; without one the Verilog is X at power-on.
CλaSH [clash §II-F] — the `State` wrapper decides; the initial value is a simulation
parameter, not part of the circuit. Anvil [anvil §4] — explicit `reg`; how a multi-cycle
`let` is held is not described. Filament [filament §4] — `Register` an extern with a
latency signature; `Prev[SAFE]` the only initial-value notion ("whether the first read
produces an undefined value"). Bluespec [bluespec §4] — explicit `mkReg`; a rule-local is a
wire. SystemC [systemc §4.2.2] — everything before the first `wait()` is reset behaviour,
"reset values shall be statically determinable constants", C++ constructor values a
simulation artefact: the nearest precedent for `s_pre`'s constants as power-on values.
MyHDL [myhdl §4.3] — initial values only in the reset branch, a pre-reset X window. RustHDL
[rhdl §4] — explicit `DFF<T>`, initial values not discussed. Golden Age [golden_age §3.1]
— "the precise choice of semantics for abstracting the concept of a register" as the open
design question. Verismith [survey] — registers declared with initializers, FPGA honours
them.
Options:
  A. Liveness at a tick, on executions (I.0.5(a) instantiated): a register is a local
     assigned inside the loop and live at some `tick` of some execution — assigned before
     it, read after it before any reassignment; everything else a constant (a pre-loop
     `let` never assigned) or a wire of one resumption; a register's value in the
     resumption ending at edge N+1 is its last assignment earlier in that resumption, else
     the value the previous resumption left (committed at edge N); the resumption's
     assignments commit at edge N+1 with the `RegOut` pending values; a register is
     observable only through a `RegOut` or a `combinational` module; power-on per the
     2026-10-29 decision; `pc`, `phase_r`, hoisted and promoted locals and counters are
     realization state (III) — changes: none now; Phase C: delete the head clause
     (`cfg.rs:408-409`), compute (a) through a nested loop's own CFG (the two fold
     defects), retire REG2/REG9/X9, the `sync_2ff` unit pin and the anchor's `NameExact`
     set (restated as registers ∪ `RegOut`), rewrite `sync_2ff` in the tick-last spelling
     (B2), add fixtures for the two nested-loop shapes to the reconciliation test; corpus
     cost: 8 locals in 7 modules change from register to wire, every one of them a D-8
     rewrite already.
     A1. A pre-loop `let` assigned in the loop but never live at a tick (B1's `ff2`):
         A1-i admitted — a wire of each resumption whose initializer is read, if at all,
         by the first resumption only, as a constant of `s_pre` (→ I.9); A1-ii refused (a
         pre-loop `let` must be a register or a constant) — costs the same 7 modules and a
         style constraint with no semantic content.
     A2. `Bits::x()` / `Logic::X`: A2-i unstated — X in the simulator (→ I.14), no `initial`
         in the SV, the module outside the agreement relation until every such register
         has been assigned on every path, the `RESET` table being III.2's declaration of
         that, `COPPER_VERILATOR_X_INITIAL=1` the measurement of what is left unstated;
         A2-ii refuse `Bits::x()` initializers — costs `simple_counter` and the `RESET`
         mechanism.
     A3. A loop-born register's value before its first assignment: A3-i none, unreadable
         by construction (Rust scoping: a `let` opens the name after it); A3-ii X.
     A4. Realization state (`pc`, `phase_r`, hoisted locals, promoted wires, counters,
         memory stages) powers on at 0 (PO6) with no meaning of its own — III only.
  B. The back-edge clause as the definition (D-7's rejected A1-ii; `ff2` a register) —
     refuted by (4): realizing `ff2` as a flop is one cycle late against the reference, and
     under one job a stored `ff2` read as its `Q` in the next resumption is the same
     program as B1's SV; changes: none; corpus cost 0.
  C. Every local a register [prost] — changes: the transpiler emits a flop per local; the
     four register-set anchors and the reconciliation test break; the paper's exact
     cell-count matches are at risk; contrary to the daylight the user stated at D-8.
  D. Visibility [spade]: every local in scope at a tick is a register — a superset of A
     at a cell cost, the same direction as C, smaller.
Recommendation: A with A1-i, A2-i, A3-i, A4 — it is I.0.5(a) written down, the measured
`sync_2ff` anchor agrees with it and refutes B, and its only implementation change is
deleting a clause that today adds exactly eight wires.
Decision: **A, with A1-i, A2-i, A3-i (A4 with A).** User, 2026-11-04 (the recommended
option and sub-options, chosen as offered; no further rationale given).
Changes: none now. SPEC: §I.7 written (I.7.1–I.7.7, no TBD); a `(D-9)` note appended to
§I.0's Decided paragraph (the `sync_2ff` measurement that paragraph attributed to Q1 §4)
and to I.0.5(c) (the back-edge clause re-derived). Part I uses "register" (I.7.1), "wire
of a resumption", "constant", "live at a tick" and "initializer" from here on; "flop",
"promoted", "hoisted", "carrier", "`initial` block" and "absorption" are Part II/III words.
Phase C: the head clause of `compute_registers` goes; the inference descends into a
tick-bearing nested loop's own CFG so that a local live at an interior tick is a register
and one dead at every tick is not (the two fold defects); REG2, REG9, X9, SS's back-edge
sentence, the `post_tick_def_read_pre_tick_is_a_register` pin and the `sync_2ff` anchor's
`NameExact` {ff1, ff2} are retired or restated (reference flops = registers ∪ `RegOut`
ports); `sync_2ff`, `flag_sync` and the fixture twin take the tick-last spelling (B2,
measured identical to the reference); the array-element forwarding defect of Q1 §4 (ii)
is fixed as a defect of I.3.4's realization; rule 5 cites I.2.3 through II.1.
Obligations: Phase C — the above; two reconciliation fixtures (a local assigned before a
ticking `for` and read inside it; a local assigned inside a ticking `for` and read only
after it) and the B1/B2 pair as a sweep fixture with the reference. II.1 — rule 5
(`check_register_initializers`) restated as the shape rule for I.2.3's `s_pre` grammar,
its scope widened from "in `registers()`" to every pre-loop `let` (corpus cost to be
measured there; `register_initializer_corpus` EXPECTED = [] today). I.9 — the first
resumption reads a pre-loop initializer as a constant of `s_pre` (A1-i's wire, and every
register's initial value), `obs_0`, and the realization of the entry resumption as a state
of its own on the extraction route. I.10 — `Memory` bindings and memory arrays are not
locals of I.7.1 (the reconciliation test filters them); word-indexed array locals are
I.7.2(h). I.11 — a `combinational` module has no register. I.12 — the synchronizer's
registers under one job (`ff1` a register, `q` a `RegOut`). I.14 — the X algebra a
`Bits::x()` register's reads follow before its assignment, and a pre-loop `let v =
Bits::x()` never assigned (a constant X). III.1 — the poison run for I.0.5(a) with `ff2`
(B2 spelling) the first negative case and the tick-last `sync_2ff` the first positive;
rustc's `Future` layout as a superset (`-Zprint-type-sizes`, a few fixtures); hoisted
locals ⊆ registers and promoted wires ⊆ registers on the corpus; the hoist's claim that "a
registered output can forward the flop before" its assignment (`control_extract.rs`
comment), which the kernel cannot do and is therefore a realization defect if true. III.2
— the `RESET` rows as the declaration of which modules the agreement relation covers only
from reset; the precondition that the emitted text states every constant initializer
(`initial`); the three-flop B1 emission as the Phase C regression case (must become B2's
two flops). I.8 inherits the nested loop's ticks as suspension sites of the kernel (the
fold is an encoding) and the zero-iteration loop's effect on liveness.
Coroutine terms: a register is a local the coroutine must carry across a `tick`
suspension; with the `RegOut` values and the memories it is a variable of H(M) (I.0.4); the
transpiler's flops are its realization, checked by the reconciliation and the reference
matches.

### D-10 — Loops, control flow and the cycle boundary                (2026-11-05)
Clause: SPEC §I.8 (new, replacing the TBD heading); `(D-10)` notes on I.2.3 (the nested-loop
sentence), the I.2.4 rows for the nested `loop`, `while`, the counted `for`, nested
`break`/`continue` and the labelled `break`, the I.2.8 NOT-YET rows for the empty-range
`for` and the labelled `break` and its parity list, the I.3.9 and I.7.7 rows that pointed to
I.8, and I.7.2(g)/(j) (the `for` counter).
Question: Under one resumption per cycle (D-8) and liveness registers (D-9), what do the
module loop, a nested tick-bearing loop, `break` and `continue`, a loop that runs zero
iterations and the counted `for` mean in coroutine terms; which of the loop rules enforced
today are shape rules of II.1, which are design rules of II.2 and which are lowering
limitations of II.3; and what is the counted `for`'s desugaring `D`, undefined until now for
a range empty only at run time?
Docs say: SS "Reachability well-formedness" (every CFG cycle crosses a tick; the
`FallThrough` discrimination), "Nested loops" (folded in the parent, a real sub-CFG inside,
"rejection-sound"), "A repeating wait must test before its tick (2026-08-24)" (the ordering
rule, justified by the post-edge sampling window of the two-job body); SPEC I.2.3 (a nested
loop must reach a tick on every path and end at one), I.2.4 (the `loop`/`while`/`for`
rows, the counted `for` as "a counter register, a `loop`, the tick hoisted to the loop's
end", the empty-at-run-time range NOT YET, labelled `break` NOT YET), I.2.5(c)(d), I.2.8 and
I.3.9 (the NOT-YET rows), I.3.2 (a zero-iteration loop contributes no boundary), I.3.3
(`break`/`continue` select the next statement), I.3.6 (the back edge at most once), I.7.2(g)
(the counter a register; the `_` counter "realization state"), I.7.5 (the fold's two
defects), I.7.7 (the nested loop's ticks as suspension sites owed to I.8); DOCS gap 17 (the
zero-iteration tick-bearing loop) and gap 27 ("`continue`/`break` semantics beyond CFG
modelling"); RULES open 5 (a literal-empty `for` range treated as a boundary).
Contradictions: I.2.4 classes `break`/`continue` of a nested loop IN while the transpiler
refuses a nested `continue` (measured on a probe) — a parity break absent from I.2.8's list;
I.7.2(g)/(j) call the `_` counter realization state while `D` needs a counter register
whatever the pattern; the ordering rule's reason is a two-job sentence D-8 retired.
Code does: `copper-analysis/src/shape.rs` (`check_nested_loop_shape`: a nested loop must
contain a tick and end at one — the last statement a bare tick or a nested loop ending at
one; ticks only in arms refused "no clock boundary of its own"; a tick-free `while`/`loop`
refused, a tick-free `for` unrolls; both front-ends), `cfg.rs:2405-2464` (`check_reachability`,
a DFS with Comb edges and the folded node's zero-tick exit as zero-time edges; two messages,
one naming the nested loop), `:2896-2920` (`nested_loop_cfg`: `break` → exit sink,
`continue` → head), `:3062-3119` (the fold: one node, `defs` empty, interior reads as `uses`,
out-edge `Tick`, `may_exit_without_tick`; the measured livelock `:3078-3090`), `:3572-3604`
(`may_exit_without_tick`: `while` always, `loop`/`for` iff no tick before some `break`; an
empty range unseen), `:3607-3640` (`ticks_before_any_break`: returns false at a statement that
ticks only on some path — over-approximates a `loop` with no `break`), `:3641-3655`
(`breaks_enclosing_loop`: a labelled `break` counts as the innermost loop's).
`copper-codegen/src/control_extract.rs:135-158` (`while` → `loop { if !c { break } … }`),
`:339-470` (`expand_counted_for`: exclusive range, tick last; a literal-empty range dropped;
counter `pat` or `__copper_ctr<n>`, type `u32`; `loop { exit_test; work; incr; exit_test;
tick }` then the hoisted tick, guarded on `ctr >= end` iff the body breaks), `:1126-1158`
(`LoopCtx`, the cached lowered continuation), `:1162-1196` (`FallThrough::AfterTick` /
`ZeroTime`, the module context), `:1208-1358` (`lower_into`: a tick allocates a state per
occurrence, a trailing tick is `pc = head`, a module-loop `continue` is a `goto_marker`, a
`break` inlines the continuation, a nested loop's head state holds the rotated body, the
ticks-only-in-arms arm clones the lowered body), `:777` (`splice_zero_time_gotos`),
`:1768-1820` (`unflattenable_in_expr`: nested `continue` refused "re-entering would re-run
the statements that follow the tick", labelled and valued `break` refused, a nested loop not
ending at a boundary refused). `copper-codegen/src/shir_lower.rs:873-883` (`split_at_ticks`,
the linear route). `copper-macros`: no loop rule of its own beyond the shared ones; a nested
`continue` and a labelled `break` compile (measured). Tests: `tests/det_for_probe.rs`,
`tests/wait_loop_equivalence.rs`, `tests/nested_tick_equivalence.rs`,
`tests/counted_delay_equivalence.rs`, `tests/counted_repetition_equivalence.rs`,
`copper-codegen/tests/while_loops.rs`, ui/fail `for_break_before_tick.rs`, `tickless_loop.rs`,
`partial_tick_path.rs`.
Measured 2026-11-04 (four probe modules in the session's scratchpad through
`EquivalenceTest`, sim vs a hand model vs the transpiled SV under Verilator; a syn scan of
the corpus; a macro/transpiler parity probe; every verdict predicted before the run):
  (i) `brk_land` — `loop { loop { if go { break } tick }; n = n + 1; q.write(n); tick;
      q.write(n + 16); tick }`: predicted agree (the break's continuation runs in the
      resumption that tested `go`; its write takes effect at the edge ending that
      resumption); measured agree, sim = hand model at 24/24 cycles, Verilator PASS.
  (i-b) `brk_trailing` — `loop { tick; loop { if go { break } tick }; n = n + 1; q.write(n) }`
      (the continuation is the trailing region, ended by the next head tick): predicted
      agree (the extraction route's zero-time goto to state 0 splices it); measured agree.
  (ii) `for_empty` — `let cnt: u32 = c.read().as_u32(); for _ in 0..cnt { n = n + 1;
      q.write(n); tick }; q.write(255); tick`: predicted divergent — both front-ends accept
      it, the simulator runs zero iterations and no cycle, the FSM takes the hoisted tick
      after the first exit test, one extra cycle per zero-count pass with `q` holding;
      measured divergent — both accepted; the emitted SV's entry state writes nothing and
      goes to the post-loop state when `0 >= c`, so a zero-count pass costs two states
      against one resumption; Verilator FAIL at cycles 0, 15 and 16 of 24, the rest masked
      by consecutive zero-count passes showing the same value; sim = hand model at every
      cycle.
  (iii) `while_skip` — `loop { while go == 0 { tick }; q.write(n); n = n + 1; tick }` with
      `go` high on entry: predicted agree; measured agree.
  (iv) corpus (`examples/`, `tests/fixtures/`, `src/`; 135 clocked modules): 28 modules with a
      nested tick-bearing loop (51 loops); **20 with a zero-iteration path** (a nested loop
      that can be left before its first tick: every `while` wait, the `loop { if c { break }
      tick }` waits, `det_for`'s mismatch exit); 0 with a run-time `for` bound; 0 with a
      nested `continue`; 0 with a labelled `break`. (Packet 3's "32 nested tick-bearing
      loops" was counted by another definition.)
  (v) parity — `nested_continue` (a `continue` after a tick in a nested loop that ends at a
      tick, the enclosing loop with a tick of its own) and `labelled_break` (`break 'w` from
      a counted `for` inside loop `'w`): the macro compiles both; the transpiler refuses
      both (`:1777`, `:1792`). Found in passing: `Bits::from_lit::<0xFF>()` is refused by the
      transpiler ("non-hardware function calls") and accepted by the macro (decimal works);
      `may_exit_without_tick` reports a `loop` with no `break` as exitable when its first
      statement is an `if` containing a tick (a false livelock refusal of the probe's first
      spelling).
Anchor: `tests/det_for_probe.rs` — the counted `for` with a `break` before its tick, sim =
a hand model of the source = SV cycle for cycle on a chosen stream (the 2026-09-02 fix of the
hoisted tick's charge to a `break`); `det_010_awaits` vs `pattern_detector_010.sv` (a `while`
wait loop, the test before the tick); the `sync_2ff` anchor of I.3 for the back edge. Sim ≡
SV only: probes (i), (i-b), (iii) and every other nested-loop module; no independent
reference contains a nested loop other than the `while` wait (III.2 owed). Probe (ii) is
measured divergent and is the Phase C defect below. B (Prost's rule) is refuted by cost,
not by an anchor: 20 modules, among them anchored ones.
Prior work: Prost [prost §2, §4, §5, §8.1] — the task runs to its next `wait` at each edge,
"any control flow construct" by unrolling into the next-state function, and the heuristic
"each loop to contain at least one wait statement and to run for at least one iteration",
acknowledged as a heuristic to be formalised; a skippable `while … wait` is refused there.
Esterel [esterel §3.4, §6.5.7] — `loop` unfolds instantaneously and a body that "cannot
terminate instantly when started" is refused at compile time: Copper's reachability rule in
rule form, about the body, not the trip count. SystemC [systemc §4.2.2, §10.5] — the thread
is an infinite loop of five syntactic forms; `wait()` inside inner loops and `break` from
them are supported; `break` out of the infinite loop, `return` and `goto` are "Not
Supported"; `continue` supported. Anvil [anvil Lemma C.19] — two iterations suffice, the
premise of I.3.6; nothing on a skipped iteration. Spade [spade §4 note] and MyHDL [myhdl §4,
§5 notes] — no tick inside a nested loop at all (one `yield` at the loop head). No surveyed
language gives a zero-iteration clocked loop a meaning other than by refusing it (Prost) or
by not expressing it.
Options:
  A. The coroutine's control flow as written, with each rule given its principle: I.8.1
     restated; I.8.2 the nested loop's ticks as suspension sites, rule (a) shape, rule (b)
     re-derived (A1); I.8.3 `break`/`continue` as statements of the resumption, nested
     `continue` (A2), labelled/valued `break` → II.1 refuse in both (A5); I.8.4 the
     zero-iteration loop (A3) and the run-time-empty `for` (A4); I.8.5 `D` for the counted
     `for` as `let mut c = a; let e = b; loop { if c >= e { break }; let v = c; c = c + 1;
     s }`, `c` a register — changes: none now; Phase C obligations below; corpus cost 0.
     A1. "A nested loop ends at its tick": A1-i a design rule of II.2 (one spelling per
         meaning: `loop { tick; C }` ≡ `tick; loop { C; tick }` by rotation, tick-last the
         canonical spelling; the two-job sampling-window reason retired with D-8); ticks
         only inside arms a dated II.3 limitation (the transpiler's cloning arm exists,
         gated off; measured or re-justified in Phase C); corpus cost 0 (`tick_first_waiter`
         is a ui/fail case). A1-ii the whole rule a II.3 limitation, Phase C measuring the
         tick-first wait under one resumption.
     A2. A nested `continue`: A2-i IN, its Rust meaning (a zero-time return to the nested
         loop's head), the transpiler's refusal a dated II.3 limitation with the fix
         direction "lower it as the loop's entry copy `W ; pc = head`"; A2-ii OUTSIDE in both
         front-ends (II.1); corpus cost 0 either way.
     A3. The zero-iteration `while` / `loop`: A3-i admit — a zero-time exit the resumption
         continues through, legal iff the enclosing path reaches a tick (Copper's rule, what
         `check_reachability` tests; corpus cost 0); A3-ii Prost's rule — refuse a nested
         loop with a zero-iteration path; measured corpus cost 20 of 135 modules:
         `handshake`, `simple_handshake`, `det_010_awaits`, `det_for` (×2), `serializer`, `rx`,
         `uart_rx` (×2), `uart_tx`, `capture_after_wait`, `rom_gated`, `handshake`
         (`handshake_dut`), `vr_read_after_delay`, `vr_sink_moore`, `vr_src_regout`,
         `vr_src_same_cycle`, `handshake` (`wait_loop_dut`), `waiter`, `while_waiter`.
     A4. The run-time-empty counted `for`: A4-i its Rust meaning (zero iterations, case
         A3-i), `D` defined, the NOT-YET rows closed; the transpiler's extra cycle a Phase C
         defect (guard the hoisted tick on the counter having advanced, or lower `D`'s form
         through the rotation), the probe a sweep fixture; no refusal; A4-ii the same meaning
         with a II.3 refusal of a non-constant bound until the fix lands (a check that does
         not exist today; corpus cost 0); A4-iii a permanent design refusal — no principle
         names it, since the `while` spelling of the same loop is admitted under A3-i.
     A5. A labelled or valued `break`: NOT YET → II.1 refuses it in both front-ends (the
         macro admits it today; the CFG's verdicts for one are a different program's); not
         optioned.
  B. Prost's loop rule as the language (A3-ii with A4-iii): every nested loop runs at least
     once — refuted by cost (20 modules, every shipped handshake and UART, `det_010_awaits`
     anchored); changes: a new check in both front-ends; corpus cost 20.
  C. Esterel's rule only (the instantaneous-body check): already Copper's rule (a); gives
     the zero-iteration loop no meaning of its own — not an option, a reading of A.
Recommendation: A with A1-i, A2-i, A3-i, A4-i (A5 with A) — every clause is the
simulator's Rust meaning written down, the alternatives cost 20 modules (A3-ii) or a
refusal check no principle names (A4-ii/iii), and the only code changes are Phase C fixes
with known directions.
Decision: **A, with A1-i, A2-i, A3-i, A4-i (A5 with A).** User, 2026-11-05 (the recommended
option and sub-options, chosen as offered; no further rationale given).
Changes: none now. SPEC: §I.8 written (I.8.1–I.8.7, no TBD); `(D-10)` notes on I.2.3, the
I.2.4 rows (nested `loop`, `while`, counted `for`, nested `break`/`continue`, labelled
`break`), I.2.8 (two rows closed or confirmed, the parity list extended), I.3.9, I.7.2(g)/(j),
I.7.7. Part I uses "nested loop", "zero-iteration exit", "counter" and "rotation" from here
on; "head state", "hoisted tick", "`pc`", "`goto`" and "fall-through" are Part II/III words.
Phase C: the hoisted tick guarded on the counter having advanced (or `D`'s form lowered
through the rotation), probe (ii) as a sweep fixture; a nested `continue` lowered as the
entry copy; the macro refusing a labelled or valued `break` (parity); the ticks-only-in-arms
lowering arm measured or its refusal re-justified; the two-job sentence of SS "A repeating
wait must test before its tick" retired and the shape rule's message restated as the
one-spelling rule.
Obligations: II.1 — the shape rule (a) with its principle; the labelled/valued `break` and
the data-dependent tick-free `for` refusals in both front-ends; the literal-empty range as
a boundary (RULES open 5: evaluate a literal range); the `may_exit_without_tick`
over-approximation stated as a direction or narrowed; the `from_lit::<0xFF>()` parity
break. II.2 — rule (b) as the one-spelling rule. II.3 — two dated rows (ticks only inside
arms; nested `continue`) with their fix directions. III.1 — `pc` states against tick sites
through nested loops (the rotation, the inlined continuation, the cloned entry); the fold's
two defects (D-9). III.2 — an independent reference with a nested loop other than the
`while` wait (the user writes references). III.3 — probe (ii) and the parity probes as
fixtures once Phase C lands. I.10 — memory accesses inside a nested loop
(`entry_cycle_mem`). I.9 — nothing: a nested loop never precedes the module loop.
Coroutine terms: a loop is the coroutine's control structure between suspension sites;
every tick site inside a nested loop is a state of H(M); `break` and `continue` select the
resumption's next statement; a loop that runs zero iterations is part of the resumption
that entered it; the counted `for`'s counter is a register of I.7.1.

### D-11 — Startup                                                 (2026-11-05)
Clause: SPEC §I.9 (new, replacing the TBD heading); `(D-11)` notes on I.0.8 (the pre-loop
re-execution bullet), I.1.4, the I.2.4 row "any other pre-loop statement", I.2.8 and I.3.9's
pre-loop bullets, I.3.7, I.4.4, I.5.10's `obs_0` and pre-loop-`Out` bullets, I.6.5, I.6.10's
pre-loop and `obs_0` bullets, I.7.7's entry-resumption bullet.
Question: Under one resumption per cycle (D-8), liveness registers (D-9) and a zero-time back
edge (D-10), what does the coroutine's first resumption mean — what `s_pre` and a pre-loop
statement that is not a `let` mean, what the entry is in H(M), what `obs_0` is for a
sequential module — and which of today's startup behaviours are defects of a realization
rather than meanings?
Docs say: SPEC I.0.2 (the first resumption from the entry, before edge 1, reading `x_1`),
I.0.4 and I.0.8 (the extraction route's pre-loop re-execution "a transpiler defect owed to
I.9"), I.1.2 (cycle 0's one instant `t_1⁻`), I.1.4 (`obs_0` in the trace, "given a meaning in
I.9"), I.2.3 (`s_pre ::= let v = c | let m = Memory(init)`), I.2.4 ("any other pre-loop
statement" NOT YET → I.9), I.2.8 / I.3.9 (the pre-loop NOT-YET bullets), I.3.7 (cycle 0's
segment; pre-loop meaning and `obs_0` I.9's), I.4.4 (every read sees `x_1`; `obs_0` I.9's),
I.5.6 / I.5.10 (the plain-`Out` startup items owed to I.9: `init(o)`, the assign transient,
`out_first_driven_late`, gap 18), I.6.5 / I.6.6 / I.6.10 (a pre-loop `RegOut` write takes
effect at edge 1; `obs_0(q) = init(q)`; the extraction route's re-execution → I.9), I.7.2(c),
I.7.4 (initializers: constant → `initial`, `Bits::x()` unstated), I.7.7 (the first
resumption's reading of an initializer and "the entry resumption as a state of its own on
the extraction route" → I.9); `review/MEASUREMENTS.md` §B (the re-execution, measured
2026-11-03 on a `RegOut`), §C debts 6 and 7 (power-on; startup before the first edge); D-1
A1 and its obligation ("I.9 must give obs_0 and the first segment a meaning and restate
`out_first_driven_late` from it"); D-2's corpus count ("non-`let` pre-loop statements 2
modules, both CPUs"); `control_extract.rs:526-549`'s comment ("state 0 runs once"); SS
"Reachability" (the pre-loop prefix is tick-free).
Contradictions: the comment's "runs once" against the measured re-execution; D-2's count
attributes both CPUs to one route while one is linear; I.2.3's `s_pre` grammar admits no
expression statement while I.2.4 classes one NOT YET and the macro admits it.
Code does: `copper-sim/src/executor.rs:364-416` (`spawn_*` push the future, poll nothing),
`:813-828` (`tick_clock`), `:836-840` (`pre_edge_settle`: the first poll of every coroutine),
`:845-853` (`post_edge_settle`); `copper-core/src/port.rs:173-183`, `:220-235` (the committed
cell is the environment's initial until `on_posedge`); `copper-codegen/src/control_extract.rs:526-549`
(pre-loop expression statements moved into state 0's body; state 0 = the loop head, re-entered
on every iteration), `:1208-1226` (`lower_into`: a trailing tick is `pc = head`; statements
after a non-trailing last tick get a state of their own); `copper-codegen/src/chir_lower.rs:1958-1972`
(the linear route's pre-loop scan: `Loop` → the body, `While` refused, every other
expression statement `_ => {}` — dropped silently); `shir_lower.rs:873-883`
(`split_at_ticks`: the trailing textual segment folded into phase 0, run at every edge
including the first); `chir_lower.rs:1158-1198` (`lower_register_init`); `emit.rs:418-432`
(`reg_inits`: registers' constants, `pc`/`phase_r`, every `always_ff` port at 0);
`copper-analysis/src/lib.rs:51-56` (`check_register_initializers`), `:185`
(`out_first_driven_late`); `copper-sim/src/verification.rs:255-299` (the testbench compares
after the first posedge — `obs_0` never compared), `:505-560` (`COPPER_VERILATOR_X_INITIAL`);
`tests/common/mod.rs:827-856` (`record` after `tick_clock`), `:577-581` (a `RESET` port
asserted on cycle 0); `build.rs:162-171`, `:206-230` (the `RESET` table and its rationale).
Measured 2026-11-05 (eight probe modules in the session's scratchpad through
`EquivalenceTest::differential_only`, 24 cycles from `obs_1`; a direct Verilator build of each
probe's SV printing the port after the `clk = 0` eval — `obs_0`, before any edge — and after
four edges, under zero-fill and `--x-initial unique`; the simulator's `obs_0` read between
`pre_edge_settle` and `post_edge_settle`; a syn scan of the corpus; every verdict predicted
before the run):
  (i) `preloop_probe` (§B verbatim, extraction route): predicted divergent (re-execution at
      every return to state 0); measured divergent at 10 of 24 cycles — `obs_0..4` sim
      0,7,1,1,2 vs SV 0,7,1,7,2.
  (i-b) `preloop_linear` (the same pre-loop write, linear route): predicted divergent (the
      write dropped); measured divergent from cycle 0 (`expected 7 got 0`), sim 0,7,1,2,3 vs
      SV 0,0,1,2,3.
  (ii) `preloop_masked` (pre-loop write, every-path head write, extraction route — the
      pipelined CPU's shape): predicted agree; measured agree 24/24, both 0,1,101,2,102.
  (iii) `obs0_write` (`loop { q.write(5); tick; q.write(6); tick }`): predicted agree with
      `obs_0 = 0` on both sides; measured 0,5,6,5,6 on both, zero-fill and randomized alike
      (every emitted port carries `initial q = 0`).
  (iii-b) `obs0_x_reg` (`let mut r = Bits::x(); loop { q.write(r); r = 9; tick }`): predicted
      divergent at `obs_1` (the RESET class); measured sim `obs_1 = X` — unrecordable by the
      harness, `as_u8` panics on X (`types.rs:297`) — SV `obs_1 = 0` zero-fill, 77 randomized;
      `obs_0 = 0` everywhere.
  (iv) `trailing_const` (`let r = 5; loop { tick; q.write(r) }`, linear): predicted divergent
      at `obs_1` only (SV 5, sim 0); measured divergent at cycle 0 only, sim 0,0,5,5,5 vs SV
      0,5,5,5,5.
  (iv-b) `trailing_const_x` (extraction twin, a branch-nested tick before the last tick):
      predicted divergent at `obs_1` (low confidence); measured **agree** 24/24 — the
      prediction missed: the extraction route lowers the statements after the last tick into
      states entered only after a tick, and the entry state runs the head alone.
  (iv-c) `trailing_reg_init` (`regout_trailing_single_tick` with `acc` initialized to 5):
      predicted divergent at `obs_1` only; measured divergent at cycle 0 only, sim 0,0,8,11,14
      vs SV 0,5,8,11,14 — the sweep's fixture agrees only because its register's initial
      equals `init(q)`.
  (v) corpus (`examples/`, `tests/fixtures/`, `src/`; 135 clocked modules with a module loop,
      31 on the extraction route): non-`let` pre-loop statements in **2 modules** —
      `rv32i_cpu_pipelined` (extraction: `program_counter.write(pc)`, `halted.write(Zero)`,
      `a0_out.write(zero)`, `memory.read_port::<0>().read(pc >> 2)`; masked by the head's
      every-path writes and restaging) and `rv32i_cpu_transpilable` (**linear**:
      `memory.read_port::<0>().read(0)`, dropped; masked by its reset on cycle 0 and the head's
      restaging) — D-2's "both CPUs" stands, "both under control extraction" does not; modules
      whose `pc` returns to state 0 with pre-loop expression statements: 1 (every extracted
      module returns to state 0 each iteration); linear-route modules with an effectful
      trailing region: 64, of which with a non-zero power-on operand: **0** (five enum
      initializers at their first variant, encoded 0).
Anchor: none independent for `obs_0` or for a pre-loop statement — every hardware-anchored
module (`lfsr` with its constant initializer, `det_010`, `sipo_block`, `shift_register` from
reset) is compared from `obs_1`, and no reference exercises a pre-loop expression statement
or a non-zero power-on operand in a trailing drive. Probes (i), (i-b), (iv), (iv-c) are
measured divergent and are the Phase C defects below; (ii), (iii), (iv-b) agree.
Prior work: Prost [prost §2, §4, §8.2] — "we begin execution at the entry point on reset";
the RESET state "corresponds to the initial state at the task entry", entered on reset, and
"on the next positive clock edge, it updates all registers according to the code between
the task entry point and the first wait statement": the entry as a state of its own, run at
the first edge, no pre-first-edge phase; the note records Copper's difference (the opening
segment before edge 1, constants as `initial`). SystemC [systemc §4.2.2] — everything before
the first `wait()` is reset behaviour, "reset values shall be statically determinable
constants", the default constructor's value "not part of the reset behavior" (a simulation
artefact). Esterel [esterel §3.3, §6.1] — initial values ⊥ for variables and signals; the
persistence law `S⁻ ∈ Ê0 implies Ê0(S) = ⊥`: the first reaction is a reaction like any
other. CλaSH [clash eq. 12] — the initial state is `run`'s argument, a simulation parameter
with no VHDL counterpart. Chisel [chisel §2.6, §10.2] — `resetVal` gives a synchronous reset
branch; a `Reg` without one is X at power-on. MyHDL [myhdl §4.3, §6.3.1] — the simulator
starts from `Signal(val)`, the converted HDL has no `initial` and is X until reset. Spade
[spade §4] — `reset(rst: v)` in the declaration. Bluespec, Cummings, Anvil, RustHDL: not
covered (bluespec.md: "not discussed in the slides"; cummings.md: reset pattern only, "no
discussion of power-on values"; anvil.md: "not discussed anywhere in the formal part";
rhdl.md: "not discussed in either paper"). No surveyed language runs the opening segment
before the first edge with its constants stated in the emitted HDL; no precedent for the
combination, and Prost's RESET state is the precedent for the entry as a realization state.
Options:
  A. The Rust meaning throughout: I.9.1 the first resumption restated; I.9.2 an initializer
     read once as a constant, the entry H(M)'s initial state and no tick site, a realization
     owing an entry that runs the first resumption exactly once (A2); I.9.3 a non-`let`
     pre-loop statement IN as a statement of the first resumption, executed once (A1); I.9.4
     `obs_0(q) = init(q)` a consequence of I.6.5/I.6.6, the plain-`Out` items to I.11,
     `out_first_driven_late` retired with D-8 (A4); I.9.5 pointers only; the linear route's
     trailing fold at edge 1 a Phase C defect (A3) — changes: none now; Phase C obligations
     below; corpus cost 0.
     A1. Pre-loop statements that are not a `let`: A1-i (recommended) their Rust meaning,
         the extraction route's re-execution and the linear route's drop Phase C defects,
         probes (i)/(i-b) sweep fixtures, cost 0; A1-ii refuse the shape (II.1 shape rule;
         cost: both CPUs rewrite; no principle names it); A1-iii desugar into the head under
         a synthesized first-iteration register — rejected: a second spelling of A1-i, and the
         coroutine has no such register (a started flag is a realization choice of A2).
     A2. The entry: A2-i (recommended) I.9 states the obligation only — a realization runs
         the first resumption exactly once; today's encodings (state 0 on the extraction
         route, phase 0 on the linear route) are defects where they differ; the form (an
         entry value of `pc`/`phase_r` as Prost's RESET arm, or a started carrier of
         I.7.2(j)) is III's, chosen in Phase C by measurement; A2-ii name the form — an entry
         state, a carrier not admitted.
     A3. The linear route's trailing region at edge 1 (probes (iv), (iv-c)): A3-i
         (recommended) a Phase C realization defect under A2's obligation, the probes sweep
         fixtures, no refusal (corpus cost 0, no shipped module shows it); A3-ii a dated II.3
         refusal of a linear-route trailing drive whose operand has a non-zero power-on value
         until the entry lands (a new check in both front-ends; cost 0 today).
     A4. `obs_0`: A4-i (recommended) stated as a consequence of I.6.5/I.6.6, the harness
         observing it in Phase C, the plain-`Out` items routed to I.11 as vacuous under D-8,
         `out_first_driven_late` retired; A4-ii the same content as a clause of I.9 in its own
         right.
  B. Prost's startup as the language: the opening segment runs at the first edge after reset,
     a reset port required — reverses D-1 (cycle 0 and `obs_0`) and the 2026-10-29 power-on
     decision (constants as `initial`), costs a reset port on every module; refused by cost
     and by the anchors (`lfsr` shows its constant at `obs_1`, before any reset).
  C. SystemC's startup: the pre-loop region is a reset segment run under a reset signal —
     the same reversal with a different clock; not an option on a design without a reset.
Recommendation: A with A1-i, A2-i, A3-i, A4-i — every clause is the simulator's Rust meaning
written down, every divergence measured is a realization defect with a fix direction and
zero corpus cost, and the one real design question (the entry's form) is a measurement
Phase C can make.
Decision: **A, with A1-i, A2-i, A3-i, A4-i.** User, 2026-11-05 (the recommended option and
sub-options, chosen as offered; no further rationale given).
Changes: none now. SPEC: §I.9 written (I.9.1–I.9.7, no TBD); `(D-11)` notes on I.0.8, I.1.4,
I.2.4, I.2.8, I.3.7, I.3.9, I.4.4, I.5.10, I.6.5, I.6.10, I.7.7. Part I uses "the entry",
"the first resumption" and "`obs_0`" from here on; "state 0", "phase 0", "entry state" and
"started carrier" are Part II/III words. Phase C: an entry on both routes that runs the
first resumption exactly once (the extraction route's state 0 not re-running `s_pre`'s
expression statements on return; the linear route lowering them and not running the trailing
fold at edge 1), its form chosen by measurement; probes (i), (i-b), (iv), (iv-c) as sweep
fixtures, (ii), (iii), (iv-b) as agreeing twins; the harness observing `obs_0` (D-1 A1); the
`control_extract.rs:526-549` comment corrected; `out_first_driven_late` removed with the eight
rules (D-8); `review/MEASUREMENTS.md` §B extended by the linear route's drop and D-2's route
attribution corrected.
Obligations: II.1 — `s_pre` tick-free (a `tick` before the loop OUTSIDE), nothing else from
this clause. II.2 — nothing. II.3 — nothing: both defects are Phase C fixes, not dated
limitations (A1-i, A3-i). III.1 — the emitted machine's initial state encodes the entry
(I.0.8's `pc`-vs-sites measurement extended to the initial state); the entry's form measured
(entry state vs carrier). III.2 — `obs_0` as a compared observation (`init(q)` both sides);
`RESET`-row coverage from the first assignment on every path, the reset on cycle 0; the
precondition that the emitted text states every constant initializer; the simulator's X
unrecordable by the harness (probe (iii-b)); an independent reference with a pre-loop
expression statement and one with a trailing drive over a non-zero power-on operand (the
user writes references). III.3 — the four divergent probes as fixtures once Phase C lands.
I.10 — the pre-loop staging as the first resumption's staging; the memory's initial array.
I.11 — `init(o)`, the assign transient, `out_first_driven_late`'s class, gap 18. I.12 — the
never-ticked domain. I.14 — a read of a `Bits::x()` register before assignment; a pre-loop
constant X.
Coroutine terms: startup is the first resumption, from the entry to the first tick reached,
before edge 1; the entry is H(M)'s initial state and no tick site, re-entered by no
execution; `s_pre` and every pre-loop statement are statements of that one resumption;
`obs_0` is the trace's first observation, taken after it and before any edge, and shows
`init(q)` on every port.

### D-12 — Shape rules and rules derived from Part I (packet II-a)   (2026-11-05)
Clause: SPEC §II.1 and §II.2 (new, replacing the two TBD headings); `(D-12)` notes at every
Part I pointer into II.1 or II.2 — I.0.6, I.2.2, I.2.3, the I.2.4 table, I.2.5, I.2.6, I.2.8's
parity list, I.3.6, I.3.8, I.3.9, I.4.3, I.4.5, I.4.6, I.4.8, I.5.5, I.5.8, I.5.9, I.5.10,
I.6.8, I.7.4(c), I.7.6, I.8.2, I.8.3, I.8.4(b), I.8.5, I.8.6, I.8.7, I.9.3. II.3 is not
written (packet II-b).
Question: What is a shape rule and what is its basis; which refusals are shape rules of II.1,
each citing which clause of the shape principle, enforced where today and with what parity;
what does the parity obligation require and which breaks are open; which refusals survive D-8
as design rules of II.2, and what becomes of `multi_write_collapse` and the dead write; and in
which direction does each static predicate the surviving rules consume approximate the
coroutine property it stands for?
Docs say: SPEC_OVERVIEW §3 (compiles iff transpiles; the message says which principle), §5
(the shape principle's sentence: five predicates — one suspension kind, every loop path reaches
a `tick`, finite state, never returns, outputs are registers; the design principle: every
effect observable, one spelling per meaning), §6 (the two refusal kinds; the II.3 list empty);
SPEC I.0.6 (the same five with pointers), I.2.2–I.2.3 (the header and grammar), I.2.4's
OUTSIDE/NOT-YET rows, I.2.5 (four reasons and a list of "remaining" forms with no principle),
I.2.6 (types are II.1's rule), I.2.8 (the NOT-YET rows and the parity list, D-10's three
additions), I.3.6, I.4.8, I.5.5/I.5.8 (the dead write and the per-rule sort owed to II.2),
I.6.8 (one driver → II.1), I.7.4(c) (rule 5 → II.1, widened), I.8.2–I.8.7 (D-10's placements:
reaching a tick II.1, tick-last II.2, arms-only II.3, nested `continue` II.3, labelled `break`
II.1; the `may_exit_without_tick` over-approximation; the literal-empty range), I.9.2–I.9.3;
D-8 (the eight retired rules, `multi_write_collapse` eighth; the one new signature rule), D-10,
D-11. `review/INVENTORY_RULES.md` §(1): sixteen rules, six justified "SR" (admissible,
reachability, nested shape, awaits, register initializers, definite assignment; memory staging
"Copper's memory model"), the rest "LL"; open item 10 (`check_admissible` macro-only, no ui
case); the macro's rule order. The task's "four clauses" is five in the overview's sentence.
Code does (commit ea54d26): the macro (`copper-macros/src/lib.rs`) refuses in order — the
header (`validate_hardware_fn` :1403-1576), `Clock::new` (:911/:295), the `Option`/`?`/panic
family (:917/:506), `check_admissible` (:932), parameter shadowing (:937/:383), CDC (:947),
reachability (:963), nested-loop shape (:977), awaits (:980), register initializers (:990),
memory staging (:999), the eight alignment rules (:1065-1322), the `In`-handle rule (:1365),
definite assignment on the combinational arm (:1378); the transpiler
(`copper-codegen/src/lib.rs::vlir_for_target`) runs reachability :358, nested shape :364,
awaits :365, definite assignment :371, register initializers :377, the alignment rules
:385-460, collapse :462, memory staging :531, memory result :537, and refuses the rest by
lowering (`control_extract.rs:1768-1820`; `chir_lower.rs:3603`, `:3812`, `:3892`, `:4822`;
`parser.rs:275`). `front_end_parity.rs` runs five rules in one direction (test 1) and the
collapse/opt-out assertions (test 2). `ticks_before_any_break` (`cfg.rs:3611-3640`) returns
"exitable" at the first statement that contains a tick without being one (`:3634-3636`).
`multi_write_collapse` (`cfg.rs:439-480`) ranges over `comb_outputs` only.
Measurements (every verdict predicted first; `scratchpad/predictions.md`, probes outside the tree):
  (i) `cargo test -p copper-analysis --test front_end_parity` — predicted PASS; measured PASS,
      2 tests. Covered: `check_nested_loop_shape`, `check_awaits`, `check_register_initializers`,
      `check_memory_staging`, `memory_result_drives_plain_out` (macro-side direction only);
      `multi_write_collapse` empty; the seven alignment rules opt-out-gated. Not covered:
      `check_reachability`, `check_admissible`, `check_definite_assignment`, the header rules,
      every syntactic OUTSIDE/NOT-YET shape, the transpiler-admits direction.
  (ii) Corpus refusal count (syn scan; 364 modules in examples/ src/ tests/: 319 sequential,
      5 synchronizer, 39 combinational, 1 structural; 162 in the sweep dirs, 135 clocked) —
      predicted 0 for every rule; measured 0 for `check_admissible`, `check_reachability`,
      `check_nested_loop_shape`, `check_awaits`, `check_register_initializers`,
      `multi_write_collapse`, `check_memory_staging`, `memory_result_drives_plain_out`,
      `check_definite_assignment`. Syntactic: labelled/valued `break` 0/0, module-loop `break`
      0, `return` 0, `/` 0, macro-token read 0, data-dependent tick-free `for` 0, hex
      `from_lit` 0, two-clock sequential 0 (all predicted 0); statement macros predicted ≤ 5,
      measured 1 (a `vec!` inside a `from_contents` preload — DESUGARED, not a statement
      macro); plain `Out` on a clocked module predicted 88 (sweep), measured 88 (273 in all);
      `allow_pretick_alignment` predicted 15–30, measured 78 (3 in the sweep dirs, the rest the
      battery's); non-`let` pre-loop statements predicted 2, measured 2 (the CPUs; two
      `const { assert!(…) }` hits are ERASED).
  (iii) `may_exit_without_tick`: `livelock_a` (packet 10's shape — a nested `loop` with no
      `break`, an `if` containing a tick before the bare tick, the enclosing loop with no
      boundary of its own) — predicted refused by both front-ends; measured refused by both
      ("this loop's only clock boundary is a nested loop that can be left before it ticks …").
      `livelock_b` (an `if`/`else` whose both arms tick, then `if done { break }`, then the bare
      tick, a write after the loop) — predicted refused by both; measured refused by both. Every
      execution of either reaches a tick on every loop path: over-refusals, parity holding. The
      control `livelock_c` (the standard wait) — predicted admitted, measured admitted by both.
      A first `nested_continue` draft with `if b == 0 { tick; continue }` first and no `break`
      was refused the same way — the over-approximation reaches a `continue`-carrying wait.
  (iv) `dbl_write` (`q.write(a); q.write(a+1); tick`) — predicted AGREE (obs_N = x_N + 1);
      measured sim = hand model 24/24, Verilator PASS. `straddle` (rule 8's shape on a
      `RegOut`: `let a = x.read(); q.write(a); tick; q.write(a+0x10); tick`) — predicted AGREE;
      measured sim = hand model 24/24, Verilator PASS. `multi_write_collapse` corpus set —
      predicted empty, measured empty. Corpus cost of refusing a straight-line dominated write
      of any port (syntactic) — predicted 0, measured 0.
  (v) Parity probes (macro = `cargo build` of a bin holding the module; transpiler =
      `transpile_source`), predicted → measured: `two_clocks` admits/REFUSES → admits/ACCEPTS
      (miss); `division` admits/refuses → as predicted (`chir_lower.rs:4822`); `stmt_macro`
      and `macro_read` admits/refuses → as predicted ("macro invocation is not supported");
      `module_break` and `module_return` admits/refuses → as predicted; `labelled_break`,
      `valued_break`, `nested_continue` (rewritten with a `break` and an outer tick)
      admits/refuses → as predicted (`control_extract.rs:1792`, `:1797`, `:1777`); `hex_lit`
      admits/refuses → as predicted (`:3812`); `datadep_for` admits/REFUSES → admits/ACCEPTS
      (miss: the SV is an `always_comb` `for (int _ = 0; _ < n; _++)`, a loop with a run-time
      bound); `plain_out_seq` and `opt_out_flag` admits/admits → as predicted; `tuple_local`
      refuses/refuses → as predicted (`check_admissible` / "cannot resolve type");
      `nonconst_init` refuses/refuses → as predicted (one message); `clock_new` refuses/? →
      refuses/refuses by accident ("cannot infer bit width"); `option_none` refuses/? →
      refuses/refuses ("cannot resolve type", `check_admissible` fires too); `panic_macro`
      refuses/? → refuses/refuses (macro invocation); `shadow_param` refuses/? →
      refuses/ACCEPTS.
Anchor: none of II.1's own (definitions); the zero-time cycle R3 refuses was measured on the
simulator before the rule existed (`cfg.rs:3078-3090`); `bsg_dff_en` anchors the `RegOut`
hold the dead-write question touches; (iv) is sim ≡ SV only.
Prior work: Prost refuses a loop without a `wait` and one that may run zero iterations
[prost §5, §8.1] — the first is S2, the second was refused by D-10; combinational loops "not
expressible" by construction [prost §5]; `.next` last-write-wins implicit [prost §5].
Esterel refuses an instantaneous loop body at compile time [esterel §3.4, §6.5.7] — S2 —
and cyclic dependencies by a static graph that "sometimes rejects correct programs"
[esterel §7], the precedent for stating a predicate's direction; several emissions of one
signal in an instant combined by `comb`, a variable "x := 1; x := x+1" ending at 2
[esterel §3.4, §5]. Spade: explicit `reg`, use-before-ready a type error, no `reg` in a
loop [spade §4, §9]; a `&mut` wire set twice a type error, set conditionally a latch
[spade §9]. SystemC: `break` of the thread loop "Not Supported", `continue` supported,
`wait()` in inner loops [systemc §4.2.2, §10.5]. CλaSH: declared state, no sequencing, no
shape rule of this kind [clash §9] — not covered. No surveyed language states its grammar as a
refusal principle (S0) or refuses a dominated write of a register.
Options:
  1. The basis of a shape rule —
     A. the five clauses S1–S5 and the kernel grammar as S0 (a construct outside I.2.2–I.2.3
        with no Part I meaning is refused citing the production); I.2.5(a)'s `Option`/`?`/panic
        family is II.2's one-spelling rule — changes: none now; corpus cost 0.
     B. as A, with the `Option` family a sixth shape clause ("every resumption produces a
        value") — changes: SPEC_OVERVIEW §5 gains a predicate; cost 0.
     C. the five clauses only; grammar-only refusals (labelled `break`, macros, shadowing, the
        hex literal) are dated II.3 limitations — changes: II.3 non-empty; I.2.4's OUTSIDE rows
        for those forms become NOT-YET; cost 0.
  2a. `check_admissible` — A. II.1 rule R16 with the parity obligation (the transpiler calls
     it, same message, a ui/fail case; direction stated: under-approximation) / B. a II.3 row
     (macro-only until I.14 writes the type set). Cost 0 either way.
  2b. The NOT-YET shapes with no rule in at least one front-end — A. II.1 refuses now, in
     both: a second `Clock` on a `sequential` module (R10, until I.12 gives it a meaning), `/`
     (R15, until I.14 does), the data-dependent tick-free `for` (R5, I.2.5(c)); cost 0+0+0
     (ii) / B. leave them NOT-YET, admitted, outside the agreement relation / C. refuse R10
     and R5, give `/` to I.14.
  3. The parity rule — restated from I.2.8 and `SPEC_OVERVIEW` §3 with the measured list of
     open breaks and the fix per row; not optioned.
  4. `multi_write_collapse` and the dead write — A. retire rule 8 as vacuous with D-8's eight
     (its predicate names a plain `Out`); a `RegOut` written twice before one edge keeps D-6 A1
     (the effect is the pending value at the edge, always observed); I.5.5's dead-write
     refusal vacuous for sequential modules, its combinational case → I.11 — changes: none now
     (the rule is in D-8's list; its ui case and parity assertion go with it); cost 0 /
     B. re-derive it as a II.2 design rule for `RegOut`: a write dominated by a later
     unconditional write before any tick is in no observation and refused — changes: a new
     rule in both front-ends, a ui/fail case; cost 0 (iv); narrows D-6 A1's reading.
  5. The direction table of II.2.2 and the retired-predicate list — stated, not optioned.
Recommendation: 1 A, 2a A, 2b A, 4 A — the overview says "shape" is the coroutine's form and
I.2.3 is that form written down; II.3 is for forms the transpiler lacks, not forms the kernel
lacks; `check_admissible` cites I.2.6 and its verdict parity is already proven by the
calibration test; every NOT-YET shape is measured at zero cost and "admitted with no meaning"
is what §3 forbids; D-6 A1 and D-8 decided the multi-write question and the measured shape
agrees on both sides.
Decision: **1 A, 2a A, 2b A, 4 A** — user, 2026-11-05, the recommended options as offered.
Changes: none to code now. Phase C obligations recorded in II.1.4 and II.2.3: the macro gains
R5, R7, R8, R9, R10, R12, R13/R14, R15; the transpiler gains R16, R17, R18 and D3 as the
shared functions with one message, and parses hex const generics; every message names its
clause (S0–S5, D1–D3) and, for S0, the production; R11 widened to every pre-loop `let` (D-9);
R3's two directions fixed (`ticks_before_any_break` walking every arm, literal ranges
evaluated), `livelock_a`/`livelock_b` as `ui/pass` cases and the literal-empty `for` as a
`ui/fail` case; `front_end_parity.rs` extended to R2, R16, `check_definite_assignment`, the
header rules and the transpiler-admits direction (D-12 §0 (v)'s probe set as its seed);
`multi_write_collapse`, its ui/fail case and the parity test's `collapsing` assertion removed
with D-8's eight (`ui/pass/multi_write_regout_ok.rs` stays as D-6 A1's pin); the nested
`continue` lowering (D-10). Spec: I.2.4's OUTSIDE/NOT-YET rows that pointed to II.1 have rows
of II.1.2; I.2.8's parity list is closed into II.1.3; I.5.5/I.5.8's sort is II.2.1's (the
rules retired, the combinational case I.11's); I.8.7's four II.1 items placed (R12, R3's
under-admission, R3's over-refusal in II.2.2, the hex literal a defect).
Obligations: II-b — the sixteen rules of `review/INVENTORY_RULES.md` §(1) sorted into II.1 /
II.2 / II.3 / retired with II.1.5's placements; II.3 written (the nested `continue`, the
arms-only loop, the run-time-empty counted `for`'s Phase C defect, what Phase C finds); R6
(recursion), R19's transpiler side and R21 (a `tick` before the loop) measured on probes;
I.10 — the memory rules' principles (incl. the dominated staging); I.11 — D2 as a clause with
the wire's dead write and last-write-wins; I.12 — CDC, the unit-domain port, the two-clock
module's eventual meaning; I.14 — `/`, the constant set behind R11, the type set behind R16;
III.1 — the poison run that decides `infer_registers`' direction, the two fold defects;
Phase C — the list under Changes. `review/INVENTORY_RULES.md` §(1)'s SR/LL column to be
re-cut after II-b.
Coroutine terms: a shape rule refuses a function that is not a hardware coroutine (it could
suspend other than at a `tick`, run forever inside one resumption, hold unbounded state,
return, or drive a wire from a clocked body); a derived rule refuses a hardware coroutine for a
reason Part I states (an effect no trace shows, a second spelling); a static predicate
approximates a property of the coroutine's executions and says in which direction.

### D-13 — Lowering limitations and the sort of the sixteen rules (packet II-b)   (2026-11-05)
Clause: SPEC §II.3 (new, replacing the TBD heading: II.3.1–II.3.7); `(D-13)` notes at every
pointer into II.3 — I.0.6, I.0.7, the I.2.4 table (the nested `continue`, `arr[i] = e` and
`Bits` methods rows), I.5.8, I.5.10, I.6.7, I.6.10, I.7.7, I.8.2, I.8.3, I.8.4(b), I.8.6,
I.8.7, I.9.2, I.9.3, II.1.2, II.1.5, II.2.4; one pointer line in `review/INVENTORY_RULES.md`.
Part II is complete.
Question: What is a row of II.3 and what must it carry; where does each of the sixteen rules
of `review/INVENTORY_RULES.md` §(1) go (II.1 / II.2 / II.3 / retired / I.10 / I.11) and does
any plain-`Out` rule keep a residue for the combinational module; which rows exist today,
with what status; what do the three rows II.1.2 left unmeasured (R6, R19, R21) do in each
front-end; and is the inventory's SR/LL column rewritten or pointed at?
Docs say: SPEC_OVERVIEW §5 (realizability: a shape the forms do not realize is refused as a
dated limitation with a fix direction, never a meaning), §6 (two kinds of refusal; the
limitation list "none today" — the alignment rules went with D-8), §2 (the 62 refused probes
unexpressible); SPEC I.0.6, I.0.7 ("II.3 owes no form"), I.2.4 (three rows pointing at II.3),
I.5.8 (the per-rule sort owed), I.5.10 (gaps 9, 13, unstated 31 → II.3), I.6.7/I.6.9/I.6.10
(the carrier rows, vacuous under D-8), I.7.5/I.7.7 (the fold defects, the array-element
defect), I.8.2(b) (the arms-only loop: "measure the arm or keep the refusal with that
measurement as its reason"), I.8.3 (nested `continue` → II.3), I.8.4(b) (the hoisted tick a
Phase C defect), I.8.7, I.9.2–I.9.4 (the entry; the linear route's drop and fold), II.1.2
(R6, R19, R21 unmeasured; the non-shape rows placed for II-b), II.1.5 and II.2.4 (the carried
lists this packet closes); D-8 (eight retired rules), D-10 and D-11 ("II.3 — nothing: both
defects are Phase C fixes"), D-12 (the placements). `review/INVENTORY_RULES.md` §(1): the
sixteen rules with their predicates, pins and SR/LL justification; §(2) the probe ledger (62
refused-divergent, 92 agreeing, 3 lowering refusals, 2 over-refusals); §(3) the `build.rs`
tables and codegen pins. `review/Q1_TRADEOFFS.md` §4 (ii) and (c''') (the array-element
forwarding bug), §5 (the rules under option B: none reason about alignment).
Code does (commit ea54d26): rule 11 `trailing_regout_reads_input` fires at a trailing
`RegOut` write whose value reaches an input read site with no tick comb-reachable from it
(`copper-analysis/src/cfg.rs:1218-1234`, the site test `:1229`); the guard edition on branch
`claude/trailing-regout-guard-67cf37` (`:1236-1241`) keys on the same site; rule 7 requires
≥ 2 comb phases and a plain-`Out` write observing a memory result (`lib.rs:537` in the
transpiler, macro `:999`); the nested `continue` is refused at `control_extract.rs:1777-1783`
and the entry copy exists at `:1278-1281`; the arms-only loop is refused by
`shape.rs:154-162` in both front-ends and its lowering arm `control_extract.rs:1290-1300` is
gated off at `:1804`; an array local with an inferable element width > 1 that is a register
is an internal memory with one write port (`chir_lower.rs:1721-1745`), a second element write
refused at `:3467-3481`; the register initializer is folded by `lower_register_init`
(`:1158-1198`); `lower_method_call` (`:4842-4937`) passes through `read`, `as_*`, `lock`,
`unwrap`, `clone`, lowers `arithmetic_shift_right`, `wrapping_*` and the memory `data`/`is_ready`,
refuses `saturating_*`/`checked_*` by name and every other method; recursion is refused by
the inliner's `inlining` set (`:389`, `:2391`); the pre-loop statements that are not a `let`
are dropped on the linear route (`:1958-1972`) and moved into state 0 on the extraction route
(`control_extract.rs:532-549`); `IndexAssign` carries only a committed value
(`shir_lower.rs:478-483`, emitted `vlir_lower.rs:2029`); the hex literal is unread at
`chir_lower.rs:3812`'s parser. Pins: 39 ui/fail cases, 8 ui/pass; `pretick_alignment_corpus.rs`
seven arrays; `hold_form_audit.rs`; `front_end_parity.rs` two tests; the battery 167 tests,
148 opt-outs; `cfg.rs` 165 unit tests; `build.rs` `SKIP` seven rows.
Measurements (every verdict predicted first; `scratchpad/predictions.md` of session
spec-packet-II-b, probes in its `probes/` and a patched toolchain in `patched/`, nothing in
the tree; macro = `cargo build` of a bin holding the module, transpiler = `transpile_source`,
sim vs SV = `EquivalenceTest::differential_only`, 48 cycles, seeded LCG stimulus):
  (i) R6 `r6_recursion` — predicted macro ADMITS / transpiler REFUSES; measured as predicted
      ("cannot inline recursive function `dec` into hardware"). R19 `r19_rebind` (`let h = x`),
      `r19_clone` (`x.clone().read()`), `r19_helper` (`fn rd(p: &In<…>)`, `rd(&x)`) — predicted
      macro REFUSES all three, transpiler ADMITS the first two and refuses the third by
      accident; measured macro REFUSES all three ("used in a way … can't verify is protected"),
      transpiler ADMITS **all three** (the helper is inlined with `&x` substituted) — one miss.
      R21 `r21_linear` — predicted macro ADMITS, transpiler ADMITS dropping the tick; measured
      as predicted (the SV is `always_ff q <= a + 1`, no state); sim vs SV predicted divergent
      at obs_1 only, measured DIVERGES at obs_1 only (simulator 0, SV x_1 + 1 = 17), agreeing
      from obs_2. `r21_extracted` (the same before a loop with a nested wait) — predicted
      macro ADMITS, transpiler REFUSES; measured as predicted ("clk.tick().await inside a
      conditional branch is not supported": the tick is moved into state 0's body).
  (ii) The ticks-only-in-arms loop. Original toolchain: `b1_arms_wait` (`loop { if c { tick }
      else { break } }` inside a loop with its own tick) and `b2_arms_both` (`loop { if c
      { n += 1; tick } else { n += 2; tick } if n[3] { break } }` then `q.write(n); tick`) —
      predicted refused by both front-ends with clause 4's message; measured as predicted.
      Patched toolchain (scratchpad copies of `copper-analysis`, `copper-macros`,
      `copper-codegen` with clause 4 and the `:1804` gate turned off): `b1` predicted
      AGREES, measured AGREES 48/48 (Verilator PASS, 2 states); `b2` predicted AGREES,
      measured **DIVERGES** from cycle 5, 41 of 48 cycles (4 states) — one miss. Mechanism
      read off the emitted SV: each arm's tick allocates a state whose body is the post-tick
      continuation `if n[3] { break → q <= n; pc <= 0 } else { pc <= 1 }`; the fall-through to
      the nested head is a state transition costing a cycle, where the coroutine runs the
      continuation, the head test and the next arm's work in one resumption; `b1` agrees
      because its continuation is empty.
  (iii) Rule 11 and the e19 guard clause read against D-8 — predicted vacuous; confirmed by
      the predicate: both fire only at a read site with no tick comb-reachable from it, and
      under one resumption per cycle every read is at t_{N+1}⁻ and `q <= x` samples that edge.
      No probe.
  (iv) `build.rs` `SKIP` rows citing a retired rule or the retired opt-out — predicted 4 of 7;
      measured 4 (`probe` — stale, its source has no opt-out and it measures agreeing,
      `review/MEASUREMENTS.md` §A; `out_from_reg_before_commit`, `vr_sink_mealy`, `vr_skid` —
      rule 10); citing a II.3 row — predicted 0, measured 0.
  (v) Belonging to retired rules — ui/fail 18 of 39 (+ ui/pass `pretick_alignment_ok`; rule 7
      makes 19); `pretick_alignment_corpus.rs` 83 entries in seven arrays (10, 8, 4, 0, 11,
      36, 14); `hold_form_audit.rs` 20 entries; `front_end_parity.rs` test 2; the battery
      167 tests / 148 opt-outs / 62 refused-divergent modules; `cfg.rs` unit tests by name
      prefix about 109 of 165 — all as predicted (counted, not estimated).
  Other rows, message only: `c_arr2` (two element writes on `[Bits::<8>::zero(); 4]`) —
      predicted macro ADMITS / transpiler REFUSES "written twice in one cycle"; measured as
      predicted, with `c_arr1` (one write) admitted by both; a first spelling
      `[Bits::zero(); 4]` without a turbofish was refused by the transpiler as a latch on both
      forms — the array-register classification keys on the element's inferable width
      (`chir_lower.rs:1721-1731`), recorded in row (c). `d_const_expr_init`
      (`const START: u8 = 1 + 2` as an initializer) — predicted ADMITS / REFUSES "has no
      power-on value"; as predicted. `e_or_reduce`, `e_concat` — predicted ADMITS / REFUSES
      "method `…` is not supported in hardware expressions"; as predicted.
Anchor: none — every row is sim ≡ SV or a refusal; no row names a behaviour.
Prior work: no surveyed language keeps a dated list of shapes its compiler does not realize;
Prost refuses as a shape error only (a loop without a `wait`, a loop that may run zero
iterations) [prost §5, §8.1]; SystemC's synthesizable subset marks constructs "Not
Supported" without a fix direction [systemc §4.2.2, §10.5]; Esterel's cyclic-dependency
refusal is a predicate direction, stated as such [esterel §7]. No precedent for the
status-bearing row.
Options:
  1. What a row is —
     A. one table, two kinds of row, one status column (REFUSED-both / REFUSED-one /
        ADMITTED-AND-DIVERGENT, the last owing a sweep fixture); D-10's and D-11's "II.3 —
        nothing" read as "no refusal"; III.3's ledger reads one line per row — changes: none;
        SPEC_OVERVIEW §6's "none today" to be restated by the coordinator.
     B. refusals only; the defects as the Phase C fix list under III.3's seed with a pointer —
        changes: none; two lists to keep in step.
  2. Rule 7 `memory_result_drives_plain_out` — A. retire now as vacuous (its predicate needs a
     plain-`Out` write and two phases; a `combinational` module has neither and cannot observe
     a memory, I.2.7 and `check_memory_staging` (d)); I.10 may re-derive its own rule —
     changes: Phase C deletes it with its ui case and the vlir RULE pin / B. carry it to I.10
     with the vacuity recorded. Corpus cost 0 either way (D-12 §0 (ii)).
  3. Row (b), the arms-only loop — A. a dated II.3 row, REFUSED-both, with the measured reason
     (the arm agrees on `b1`, diverges on `b2`) and the fix direction (inline the head's
     lowered body at every post-tick fall-through) — changes: none now / B. admit the `b1`
     sub-shape (empty post-tick continuation) now — a new partial rule in both front-ends /
     C. re-justify as a II.2 one-spelling rule ("lift the tick") — names the wrong principle:
     the coroutine gives the shape a meaning (I.8.2) and `b2` shows the refusal is a
     lowering fact.
  4. The three unmeasured rows — placed by their measurements, not optioned: R6 II.1 S3
     (macro owes), R19 II.1 S0 (transpiler owes; its injection reason goes with D-8), R21
     II.1 S0 (both owe; row (f6) until then).
  5. The inventory's SR/LL column — A. leave `review/INVENTORY_RULES.md` as the 2026-11-03
     snapshot with one pointer line to II.3.2 / B. rewrite the column there.
Recommendation: 1 A, 2 A, 3 A, 5 A — one place answers "what does the transpiler not realize
today" and a defect differs from a refusal only in whether the user is told; a vacuous rule
on I.10's list is a rule without a principle; the measurement makes (b) a lowering fact and
a partial admit a rule nobody asked for; the inventory is the baseline every entry cites and
SPEC.md is the authority.
Decision: **1 A, 2 A, 3 A, 5 A** — user, 2026-11-05, the recommended options as offered.
Changes: none to code now. Spec: §II.3 written (II.3.1 the definition and the two rules of
the section; II.3.2 the sort, the no-residue reading, the sized deletion, the rows D-8
closed; II.3.3 the twelve rows — six refusals (a)–(e), (g), six defects (f1)–(f6); II.3.4
the three measured rows placed in II.1; II.3.5 the column; II.3.6 the map and the ordered
Phase C fix list; II.3.7 the carried list); `(D-13)` notes at the pointers; one pointer line
in `review/INVENTORY_RULES.md`. Phase C: the fix list of II.3.6 in order ((1) the entry of
I.9.2, (2) the hoisted-tick guard, (3) the array-element forwarding, (4) the nested-`continue`
entry copy, (5) the arms-only fall-through clone measured on `b1`/`b2`, (6) the array
write-port form, (7) the constant evaluator with I.14, (8) hex const generics, (9) the method
lowerings as I.14 lands, (10) R21 in both front-ends), then the deletions of II.3.2 (19 ui
cases, the two pin files, parity test 2, the battery's refused rows and verdict tables, the
~109 `cfg.rs` tests, 4 `SKIP` rows, rules 7 and 9–15 with their call sites) and the parity
fixes (R6 macro, R19 transpiler, R21 both). The probes `b1`, `b2`, `r21_linear` become sweep
fixtures when their fixes land.
Obligations: III.3 — the ledger's shape: one line per II.3.3 row (one per method for (e))
and per retired rule 7–15, closed when the fixture passes; III.2 — every defect row outside
the agreement relation until closed; I.10 — rule 6 with its principle and the dominated
staging; I.11 — D2; I.14 — the constant grammar behind (d) and R11, a meaning for every
method of (e) and the checked/saturating four, the hex literal (g), `/` (R15); III.1 — the
two fold defects and the hoisted-initializer claim (not rows); the coordinator —
`SPEC_OVERVIEW.md` §6 restated ("six dated refusal rows and six defect rows, every one a
transpiler form, none a meaning"); `review/INVENTORY_RULES.md` §(1) — the pointer line only.
Coroutine terms: a lowering limitation is a shape whose resumption Part I defines and whose
emitted machine does not reproduce today; the coroutine is never at fault, the row is the
realization's dated debt with the form that pays it, and a row without a date or a fix
direction is a meaning in disguise and is deleted.

### D-14 — Combinational modules                                   (2026-11-05)
Clause: SPEC §I.11 (new, replacing the TBD heading); `(D-14)` notes at every pointer into
I.11 — I.0.1, I.0.7 ×2, I.0.8, I.1.3 ×2, I.2.2, I.2.3, I.2.7, I.4.5, I.4.8, the I.5 header,
I.5.3, I.5.4, I.5.5, I.5.7, I.5.10 ×2, I.7.2, I.7.3, I.9.1, I.9.4, II.1.1 (S5), II.1.5, II.2.1
(D2, D4), II.2.3, II.2.4, II.3.2 (rows 15, 16), II.3.5, II.3.7. Nothing else changed.
Question: What does a `combinational` module mean as a coroutine — what one evaluation reads,
writes and keeps; which of I.5's wire rules survive for its wire and what becomes of the dead
write, `init(o)` and D-11's startup transient; what principle refuses the unwritten path and the
never-written port; what an evaluation may read, and whether two combinational modules may read
each other; what the Mealy composition D-8 routes here means and whether it is anchored; and
what I.13, I.10 and I.14 inherit?
Docs say: SPEC_OVERVIEW §2 (a Mealy output is a `combinational` module wired to the body's
`RegOut`s, measured at no cell cost), §4 choice 6 ("wires in I.11"), §5 (shape: "its outputs
are registers" — no wire clause; design: every effect observable, one spelling per meaning);
SPEC I.2.7 (the tick-free fragment, every path writes every output, "evaluated afresh at every
instant", the wrapper an implementation form), I.5 header (the wire rules routed here: I.5.1's
write, I.5.3's last-write-wins, I.5.6's `init(o)`, I.5.7's three kinds), I.5.4 ("a
combinational module's output does not hold"), I.5.5 (the dead write's combinational case),
I.5.7 (CM6: definite assignment applies), I.4.8's D-8 bullet (a wire a `combinational` module
drives in the same settle), I.6.8 (a `RegOut` is a settle source), I.7.2 (no `tick`, no
register), I.9.4 and D-11 (`init(o)`, the assign transient, `out_first_driven_late`'s class,
gap 18 routed here as vacuous), I.0.4 ("combinationally iff a function of the registers and
the inputs"), II.1.1 S5, II.2.1 D2 (the principle stated and owed to I.11 as its clause; the
default-then-override admitted), II.2.2 (`check_definite_assignment` *under*: folded loops,
never-written ports), II.2.4 and II.3.7 (D2, the dead write, the double write), II.3.2 rule 7
(no combinational residue). `LEVELIZED_SCHEDULING_SCOPE.md` phase 5: a convergent
combinational loop is legal and iterated, "detection is not rejection" (2026-07-30).
Code does (commit ea54d26): the macro wraps the pristine body in `loop { body;
::copper_sim::delta_yield().await }` (`copper-macros/src/lib.rs:853-868`), refuses `async`
(`:1411`) and a `Clock` parameter (`:1506`), runs `check_definite_assignment` first
(`:1371-1384`); `DeltaYield` is Pending once and Ready on the next poll
(`copper-sim/src/lib.rs:70-81`); the levelized settle polls a singleton SCC once
(`executor.rs:546-558`), iterates a multi-task SCC to a fixpoint and panics past the
oscillation threshold (`:577-632`), `comb_cycles` reports the SCCs (`:490`);
`Cfg::build_combinational` (`cfg.rs:332`) and `check_definite_assignment` (`:2514`: MAY − MUST
at the exit, folded loops write everything, a never-written port is not flagged — `TODO`
`:1366`); the transpiler runs the same rule (`copper-codegen/src/lib.rs:371`), `lower_comb`
(`vlir_lower.rs:256-271`) routes a port written unconditionally and inside a branch into
`always_comb` in program order (§10.12, `nested_port_drives`) and every other top-level drive
to a continuous `assign` (`lower_flat_stmts`, `:736-741`), `check_no_latches` (`:2384`) as
defence in depth, `emit.rs::comb_body` (`:185-197`); the sweep drives the truth table up to 8
input bits (`build.rs:256-262`) and the testbench drives, `eval()`s and compares
(`verification.rs:263-276`). The environment creates every port wire at `Bits::zero()`.
Measurements (every verdict predicted first; `scratchpad/predictions.md` then
`measurements.md`; probes in `scratchpad/i11probes/`, none in the tree):
  Corpus: 39 `#[hardware(combinational)]` modules (examples/ 10, tests/fixtures/ 16, src/ 0 — the
  26 of the sweep directories — and 13 sim-only in four tests/*.rs files).
  (i) `bsg_mux_one_hot` (the example's module text), 192 vectors, golden = OR of the selected
      inputs — predicted sim ≡ transpiled SV PASS and sim ≡ BaseJump reference PASS; measured
      PASS/PASS, sim vs golden 0 mismatches.
  (ii) default-then-override ports — predicted 5 (all in `comb_default_override_dut.rs`),
      measured 5; double top-level writes of one port — predicted 0, measured 0; the six
      `fx_comb_default_override_dut::*` sweep cases — predicted PASS, measured 6/6. Probe
      `cdo_dead` (`o.write(x); o.write(y)`) — predicted: macro admits, `check_definite_assignment`
      Ok, sim shows `y`, transpiler emits two `assign o`, Verilator MULTIDRIVEN fails the build;
      measured exactly that.
  (iii) `check_definite_assignment` over the 39 — predicted 0 refusals, measured 0; ui
      `combinational_latch`, `_second_port`, `_two_ifs` refused and `combinational_mux_valid`
      admitted (trybuild, predicted); `comb_definite_assignment_parity` 2/2. Probe
      `cdo_unwritten` (`p` declared, never written) — predicted: both front-ends admit, the SV
      has an undriven `output logic [7:0] p`, `verilator --lint-only -Wall` refuses UNDRIVEN;
      measured exactly that. Corpus never-written ports — predicted 0, measured 0.
  (iv) combinational cycles — predicted examples 0 (no example file holds two combinational
      modules; the six multi-spawn examples spawn none), measured 0; tests/ — predicted 2
      sim-only designs, measured 2 (`levelized_differential` or_gate+buf convergent, iterated:
      `diff_convergent_comb_loop` ok; `simulator_corner_cases` invert_a+buffer_a non-convergent:
      should-panic ok; `executor::tests::comb_cycles_*` 2 ok). (iv-b) the convergent pair
      transpiled under a hand top, `verilator --lint-only -Wall` — predicted UNOPTFLAT, refused;
      measured "%Warning-UNOPTFLAT: Circular combinational logic: 'out'".
  (v) Mealy pair `mealy_core` (`RegOut full` toggled by `x`) + `mealy_ready` (`ready = ~full |
      drain`), 64 LCG cycles, golden = a Rust model — predicted sim ≡ hierarchy SV (a hand top
      over the two transpiled leaves) PASS and sim ≡ independent reference `mealy_ref.sv`
      (`always_ff` toggle, `assign ready = ~f | drain`) PASS; measured PASS/PASS, sim vs golden
      0; `obs_N(ready) = ~full_N | drain_N`; before the first settle the simulator's wire shows
      its creation value, after it the evaluation.
  (vi) `obs_0` of `pass8` (`o = x`) — predicted: the simulator shows the creation zero only
      before the first settle, `obs_0` with `x = 0xA5` is `0xA5` on both sides, PASS; measured
      exactly that (3 cycles PASS).
  Side findings: a `Bits<1>` port indexed `x[0]` is emitted as a select on a scalar `logic`
  (Verilator "Illegal bit or array select"; probe (v)'s first draft) — a transpiler width-1
  defect → I.14 / Phase C; a module named `buf` collides with a Verilog primitive in a hand top
  (renamed; whether the legalizer renames keyword-named modules was not measured).
Anchor: `bsg_mux_one_hot` vs BaseJump (i) for the evaluation as a function and one write per
path; the Mealy pair vs its independent reference (v) and `vr_sink_mealy` → B with a boundary
(`Q1_TRADEOFFS` §4 (d')) for I.11.5; sim ≡ SV only — the default-then-override shape (ii) and
`obs_0` of a wire (vi); none for the dead write's straight-line form (the build refuses it),
the never-written port (the build refuses it) or the combinational cycle (the build refuses
it): decided.
Prior work: CλaSH — a combinational circuit is a pure function, the combinational part of a
Mealy machine [clash §II-F, eq. 10]; Spade — `fn` units are combinational only, a mutable wire
set conditionally "a latch could be inferred", set twice a conflict, both refused by its linear
type, loops "discouraged by scoping; allowed via `decl`" [spade §II, §IV]; Chisel — a
combinational output is a function of the cycle's input (`io.enq_rdy := !is_full`), an
undriven wire an error, multiple writes a static driver priority, loops not discussed
[chisel §2.4, §2.6]; Bluespec — rules and methods evaluated combinationally over the current
state, a method's `rdy` Mealy, no combinational module kind, loops not discussed [bluespec
slides 11, 16]; Esterel — the reaction instantaneous, one value per signal per instant,
several emissions combined by a declared `comb`, cyclic dependencies rejected even when the
solution is unique [esterel §1.4, §3.3, §2.2.1, §7]; Cummings — `always_comb` with blocking
assignments gives single-pass forwarding, last write wins, an incomplete assignment infers a
latch, loops treated only as the self-triggering oscillator [cummings §5.3, §5.4, §15.3];
MyHDL — `@always_comb` infers its sensitivity, `.next` last-write-wins, the time-0 mismatch
documented as a known issue, loops only the delta cap [myhdl §6.2.1, §10.14, §12.2.2]; Prost —
"modeling pure combinational circuits via regular functions is a goal, combinational cycles
are currently not expressible" [prost §5]; PyMTL3 — the latch lint over combinational blocks,
the static scheduler refusing any cyclic block dependency [pymtl3 §4, §5]; Arch — "no
combinational loops — static DAG, bounded settle" [arch §7.3.1]; RustHDL/RHDL — the
"assign default first" idiom plus a Yosys latch lint, RustHDL iterating to a fixpoint with a
cap, RHDL "feedback loops must be broken by registers" [rhdl §3, §4]; SystemC — complete static
sensitivity for a combinational method "to avoid accidental latching", last write wins by the
LRM, loops not discussed [systemc §4.1.1, §5]. No surveyed language refuses a dominated write
of a wire; the function languages (CλaSH, Chisel, Spade, Prost, PyMTL3 static, Arch) have no
combinational cycle, the event languages (Verilog, SystemC, RustHDL) iterate one.
Options:
  1. The dead write — a port written twice in one evaluation, the first write in no observation:
     A. admitted, last-write-wins: the effect of a write is the value the evaluation leaves on
        the wire, always observed (II.2.1 D4's reading for a `RegOut`); default-then-override
        is every surveyed language's idiom and transpiles since §10.12; the straight-line
        double top-level write is a II.3 row (two `assign`s, MULTIDRIVEN; fix: the §10.12
        routing for any port written more than once) — changes: none now; corpus cost 0.
     B. refused when dead on every evaluation (I.5.5's principle on the wire) — changes: a new
        rule in both front-ends; corpus cost 1 (`cdo_override_then_default`, a §10.12 pin); the
        lint D-12 D4 declined for a `RegOut`.
  2. The principle D2 (definite assignment, the latch) cites:
     A. design, D2 stays in II.2 — one spelling per meaning: an unwritten path is a hold, a
        hold is state, state is a `RegOut`'s spelling; I.11.3 supplies the clause (a wire's
        value is a function of the evaluation's reads and nothing else; an unwritten path gives
        it no value) — changes: none; no overview edit.
     B. shape, D2 moves to II.1 as S6 ("its wires are written on every path") — changes:
        `SPEC_OVERVIEW.md` §5 gains a predicate (the coordinator's edit), D-12's D2 row re-cut.
  3. The never-written port (both front-ends admit; Verilator refuses UNDRIVEN; corpus 0):
     A. refused in both front-ends as D2's degenerate case, one message, a ui/fail case; a port
        a child drives is wired, not written → I.13 — changes: `check_definite_assignment`
        widened (Phase C); cost 0.
     B. admitted as today (the `TODO`'s submodule reason), the build refusing it — a module
        that compiles and does not transpile.
  4. The combinational cycle:
     A. OUTSIDE — a shape refusal: every wire is a function of the registers and the inputs
        (I.0.5(b)); the settle is a function, not a fixpoint; refused statically at spawn in
        Phase C from `comb_cycles`, the SV side already refused by Verilator (UNOPTFLAT); the
        simulator's convergent iteration and non-convergent panic are realization behaviour
        outside III.2 until then — changes: the executor refuses at spawn, the three cycle
        tests become refusal pins, `LEVELIZED_SCHEDULING_SCOPE.md` noted (reverses item 6
        phase 5, 2026-07-30); corpus cost 0.
     B. admitted if convergent (today's policy as meaning: the wire's value is the fixpoint the
        iteration reaches) — changes: none; no anchor, no SV realization, convergence
        data-dependent (Esterel §7's argument); the admitting prior art is an event or fixpoint
        semantics, not a function.
  No fork (stated and written): I.11.1 the module is a function `f(reads) → writes` evaluated
  by the wrapper coroutine once per settle, the count a scheduler artifact, the wrapper's
  yield not a kernel suspension; I.11.2(b) last-write-wins; I.11.2(e) `init(o)` is the
  environment's unobserved creation value, D-11's transient vacuous; I.11.4 a `RegOut` is
  readable at its committed value, a settle source; I.11.5 the Mealy composition's meaning
  `f(obs_N(q), x_N)` at `obs_N`, `f(obs_N(q), x_{N+1})` before edge N+1; I.11.6 the map.
Recommendation: 1 A, 2 A, 3 A, 4 A — the wire's meaning is the value an evaluation leaves on
it, which every observation shows; the latch refusal is a spelling rule with its clause now
written; a port no path writes has no value on every evaluation and the build already refuses
it; and a combinational cycle gives no wire a value as a function, has never had an anchor or
an SV realization, and is refused by every function-based language surveyed.
Decision: **1 A, 2 A, 3 A, 4 A.** User, 2026-11-05, the recommended options as offered.
Changes: none now. Phase C obligations (I.11.6): `check_definite_assignment` refuses the
never-written port in both front-ends with one message and a ui/fail case, the folded-loop
and never-written directions pinned (II.2.2); the transpiler routes every write of a port
written more than once into `always_comb` in program order (II.3's new row, probe (ii) as the
fixture); the executor refuses a multi-task SCC at spawn from `comb_cycles` and the
transpiler's hierarchy emission refuses the same cycle, the three cycle tests
(`comb_cycles_reports_convergent_latch_without_rejecting_it`, `diff_convergent_comb_loop`,
`non_convergent_combinational_loop_is_detected`) rewritten as refusal pins,
`LEVELIZED_SCHEDULING_SCOPE.md` phase 5 noted as reversed by this entry; `front_end_parity.rs`
runs `check_definite_assignment` over the combinational corpus (II.1.3); every D2 message
cites I.11.3; the `Bits<1>` index emission fixed with I.14's width rules. Spec: I.5's wire
rules are taken (I.11.2); I.5.5's combinational case, II.2.4's question and the combinational
residue of `multi_write_collapse` close; D-11's startup items are vacuous for the wire; II.3
gains one row (II.3.7's note) and II.1 one shape row when the cycle rule lands.
Obligations: II.3's row for the double top-level write written into II.3.3 when that section
is next edited (its date is this entry's); I.13 — the settle, the wiring, dirty-driven
re-evaluation, order independence (I.11.4 states the contribution: an evaluation is a function
of its reads), and hierarchy inside a `combinational` module (the wired-port exemption of
I.11.3); I.10 — a memory reaching a `combinational` module; I.14 — X in a wire and the width-1
emission defect; III.2 — `obs_0` of a combinational wire observed by the harness and the
creation-at-zero precondition; the probes of this entry (`cdo_dead`, `cdo_unwritten`, the
Mealy pair with its reference, the convergent pair) as fixtures when their rules and forms land.
Coroutine terms: a `combinational` module is a function the wrapper coroutine evaluates once
per settle; it reads settled wires, writes its own wires, keeps nothing; its wire's value is
what the evaluation leaves on it, every path must leave one, and a cycle of such functions is
not a function and is not a hardware coroutine.

### D-15 — Consolidation of §I.0–§I.6 under D-8 (packet 15)          (2026-11-05)
Clause: SPEC §I.0–§I.6 rewritten in place to read under the one-job body, as §I.7 onward
does; §I.5 a forwarding stub; the preamble's sentence on D-8. Bookkeeping only — no clause
of meaning changes, no clause is renumbered, no citation is orphaned.
Question: How are §I.0–§I.6 — written under the two-job body by D-1…D-7 and re-derived by
annotation under D-8 (packet 8: "(D-8 …)" notes and "superseded" marks, bodies kept) — made
to read as one model without changing any meaning?
Docs say: SPEC §I.0–§I.6 as packet 8 left them — 43 `(D-8 …)` notes, 13 "superseded" marks,
and the two-job vocabulary counted under Changes; every one of the 59 clause headings
I.0.1–I.6.10 is cited somewhere in SPEC.md, SPEC_DECISIONS.md, SPEC_OVERVIEW.md or
review/DECISION_AUDIT.md, so none can be dropped or renumbered; §I.7–§II.3 cite I.5.4,
I.5.5, I.5.6, I.5.7 and I.5.10 (nine citations) and use "trailing region" in ten lines.
Code does: nothing — a documentation packet; no code, no probes.
Anchor: none — bookkeeping.
Prior work: none consulted — bookkeeping.
Options:
  Q1 §I.5. A. heading kept, body a stub: one paragraph plus a table mapping I.5.1–I.5.10 to
     where each former clause's surviving content lives — changes: none; 0 re-pointed
     citations. B. delete §I.5 and renumber I.6–I.14 — 813 references across the four files
     (SPEC.md 531, the log 273, OVERVIEW 2, AUDIT 7) plus eleven sibling notes; breaks the
     "never renumbered" convention and §I.7+ byte-identity.
  Q2 the read classes and the trailing region. A. restate — a read is by position at the one
     instant (I.4.1–I.4.3); the trailing region stays a defined position in the text (I.3.5)
     because §I.7+ uses the term; the linear route's fold is history in I.3.8. B. keep the
     deferred/immediate vocabulary as history inside I.4.2/I.4.3 — fails measurement (i).
  Q3 the `(D-8 …)` notes and "superseded" marks. A. removed; each section opens with one line
     "Rewritten 2026-11-05 under D-8 (D-15); the two-job text is in the decision log's D-1…D-7
     entries and the packet-15 snapshot"; a bare D-8 stays wherever it is the decision that
     produced a surviving clause. B. keep the notes inline.
  Q4 this entry and one pointer line in review/DECISION_AUDIT.md. yes / no.
Recommendation: A, A, A, yes.
Decision: **A, A, A, yes** — user, 2026-11-05 ("All recommended"), with the packet's five
bookkeeping choices: the opt-out flag's name leaves I.2.2/I.2.4 (II.1 R9 and D-8 keep it);
`t⁻`/`t⁺` stay as the environment's phase names (I.1.5, I.1.6, I.0.7) while `t_N⁺` as an
instant goes; I.4.2, I.4.3, I.4.5, I.4.6 and I.6.7 are restated one-job clauses, not stubs;
review-inventory codes attached to deleted content leave with it (S7, TR6/TR7, IN11, T7,
OUT1, PO8, PO10) and those attached to surviving content stay (RO1/RO3, RO6, W6, M9, M15,
PO6, T12); the pre-rewrite text is snapshotted verbatim outside the repo
(side-session notes, `packet-15-pre-rewrite-I0-I6-snapshot.md`, 1050 lines).
Changes: SPEC.md §I.0–§I.6 (1050 lines → 849) and one preamble sentence; §I.7 onward
byte-identical (diff: 0 lines); nothing else.
  Inventory, delete / restate / keep-as-history: I.0 8/7/5, I.1 2/5/1, I.2 0/4/0,
  I.3 4/8/2, I.4 3/9/1, I.5 every clause / 1 stub / 0, I.6 4/9/2.
  Deleted: the barrier sites (I.0.1); the two-resumption and plain-`Out`-wire notes (I.0.2,
  I.0.3, I.0.4, I.0.8); D-7's three carriers (I.0.5(b)); the predicate names in I.0.6; the
  second instant `t_N⁺` (I.1.2, I.1.5); the opt-out flag in the header and the table (I.2.2,
  I.2.4); the per-execution instant (I.3.4); the TR6/TR7/S7 sentence (I.3.5(d)); the
  deferred/immediate classes, the parking rule, the passthrough refusal and the static
  approximations as written (I.4 Decided, I.4.2, I.4.3, I.4.5, I.4.6, I.4.8); §I.5.1–I.5.10
  with their Decided and coroutine paragraphs; D-6 A4-i's two-instant text (I.6.7); the
  `t_N⁺` carrier-flop obligation (I.6.9).
  Restated: I.0 Decided (D-7 and D-8 named once; the D-9 note folded); I.0.1 ("the only
  `await` is `tick`"); I.0.5(b) ("no carrier is known"); I.0.5(c) (the back-edge clause by
  D-9's result); I.0.6 (the rules and forms retired with the second instant, II.2.2 names the
  predicates); I.1 Decided; I.1.2–I.1.5 (D-8 parentheticals folded); I.2.2 (no second
  attribute token; `Out` outside on a sequential module, II.1 R8); I.3 Decided (`ff1` a
  register, `q` a `RegOut`); I.3.3 (one suspension kind); I.3.4 (one instant, D-8 reversing
  D-3 A1); I.3.5(a)–(d); I.3.7; I.4 Decided (anchors by position; the passthrough to I.11.5);
  I.4.1 ("a register and a `RegOut` alike"); I.4.2 "One class of read"; I.4.3 "Trailing
  reads" (sees `x_{N+1}`, anchored in the `RegOut` spelling); I.4.4; I.4.5 "The passthrough";
  I.4.6 "No static approximation"; I.6 Decided ("the output rule since D-8"); I.6.1, I.6.2
  (not dead writes, II.2.1 D4), I.6.3 (a wire does not hold, I.11.2(d)), I.6.4, I.6.6
  (I.11.2(e)), I.6.7 "An input read after the tick", I.6.8, I.6.10.
  Kept as history, inside implementation maps and prefixed "Today (two-job implementation,
  Phase C rewrites)": I.0.7 (the `pre_edge_barrier` injection and `classify_reads`,
  `PreEdgeBarrier`, `tick_clock`'s two settles, the Phase C obligation list); I.1.6 (a tick
  resolves at `t⁺`, a deferred read parks); I.3.8 (the park; the linear route's split placement
  and the three static regions); I.4.7 (`classify_reads`, `passthrough_read`, `ReadRewriter`,
  `pre_edge_barrier`, the surviving obligations); I.6.9 (the mux + hold form,
  `trailing_reg_outs`, `trailing_regout_reads_input`, the three vacuous II.3 rows).
  The I.5 forwarding table (as in SPEC §I.5): I.5.1 → I.11.2(a) / I.6.1; I.5.2 → retired
  (I.1.4, I.6.4); I.5.3 → I.11.2(b) / I.6.2; I.5.4 → I.6.3, I.11.2(d); I.5.5 → vacuous
  (I.6.4, II.2.1 D4), the wire's case I.11.2(c); I.5.6 → I.11.2(e) / I.6.6; I.5.7 → I.2.2,
  I.11.1, I.6.1, I.11.3; I.5.8 → retired (II.2.1, II.3.2); I.5.9 → retired (D-8 "Code does",
  I.0.7); I.5.10 → closed (I.9.4, I.11.2(e), II.2.1, II.3.2, I.11.4, I.13, I.14).
  Measurements. (i) Two-job vocabulary in §I.0–§I.6, before → after: `barrier` 13 → 5,
  `deferred` 29 → 6, `immediate read` 2 → 0, `post-edge resumption` 1 → 0, `t_N⁺` 12 → 0,
  `allow_pretick` 4 → 0, `_hold` 7 → 0, plain(-)`Out` write 2 → 1; every remaining hit is in
  a keep-as-history sentence (I.0.7 lines 2, I.1.6 lines 2, I.3.8 line 1, I.4.7 lines 7);
  `(D-8 …)` notes 43 → 0 (21 bare D-8 decision citations remain), "superseded" 13 → 0.
  (ii) Citations: 70 distinct `I.[0-6].n` citations across the four files; every one resolves
  to a clause heading of SPEC.md, the ten I.5.n citations to rows of the §I.5 stub table; one
  historical sub-item citation, `I.3.3(v)` in D-7's reversal line, names the item D-8 deleted
  and is left as the record of that deletion. (iii) `diff` of §I.7 onward, before vs after:
  0 lines; re-pointed citations: none. (iv) The D-n reference set of §I.0–§I.6: before
  {D-1 … D-14}; after the same set plus D-15 (the per-section "Rewritten" line).
Obligations: none of meaning. Two notes for a later packet, not acted on here: (a) I.0's and
I.3's Decided paragraphs inherit packet 8's grouping of `det_010_awaits`, `lfsr` and
`shift_register` with the `RegOut` spellings of `review/Q1_TRADEOFFS.md` §4, whereas §4
(D-8's Anchor) measures `det_010`, `bsg_dff_en`, `bsg_counter_up_down`, `sipo_block`,
`vr_sink_mealy` and `det_110101` — the wording now says "the `RegOut` spellings measured in
§4" without asserting each module was; (b) the pre-rewrite text of §I.0–§I.6 exists only in
the side-session snapshot (the SPEC files are untracked) — copy it under `review/` if the
record should live in the repo.
Coroutine terms: unchanged — §I.0–§I.6 now state D-8's one-job coroutine once each, in the
vocabulary §I.7 onward already uses.

### D-16 — Memory                                                   (2026-11-05)
Clause: SPEC §I.10 (new, replacing the TBD heading); `(D-16)` notes at every pointer into
I.10 — I.0.1, I.0.2, I.0.4, I.0.8, I.1.3, I.2.2, I.2.3 ×2, I.2.4 ×2, I.3.3, I.6.8, I.6.10,
I.7.2, I.7.7, I.8.7, I.9.2, I.9.3, I.9.5, I.9.7, I.11.4, I.11.7, II.1.2, II.1.4 ×2, II.1.5
×2, II.2.4, II.3.2 (rows 6, 7), II.3.5, II.3.6, II.3.7. Nothing else changed.
Question: What is a `Memory` as a variable of the coroutine's state machine — what one
resumption may do to it and what the edge does; what a read delivered at an edge where the
same word is written is; what the initial array is and what the first resumption's staging
means; which principle each clause of rule 6 cites and whether a dominated staging is admitted;
what a received memory means when shared and what the bus ABI is; whether a `combinational`
module may touch a memory; and what the interpreter, I.12, I.13, I.14 and III.2 inherit?
Docs say: SPEC I.0.1 (stagings among the effects), I.0.2 (2) and I.1.2–I.1.3 ("every memory
advances"; memories change only at an edge), I.0.4 (iii) and I.3.3 (iv) (a variable of H(M)),
I.2.3 (`let m = Memory(init)`, `stage`, `stage_write`, `observe`), I.2.4 (the constructors
DESUGARED to a constant array, a run-time preload OUTSIDE; the port calls IN), I.2.7 (no
staging in the tick-free fragment), I.6.8/I.6.10 (M9: `q.write(m.data())`), I.7.2 (a `Memory`
binding is not a local; a memory pipeline stage is realization state), I.8.7
(`entry_cycle_mem`), I.9.2–I.9.5 (D-11: a pre-loop staging is the first resumption's; the
linear route drops it, the extraction route re-runs it), I.11.4/I.11.7 (rule 7's residue),
II.1.2 (the memory rules deferred), II.2.4 (the dominated staging), II.3.2 rows 6–7 (rule 6
deferred; rule 7 retired vacuous). `SYNCHRONOUS_SEMANTICS.md` "Memory — staging and read
latency" (M1–M7: staged not returned, the four rules, both front-ends, no opt-out), "state
starts at zero" (PO7: a `Memory::new` array zero, pipelines empty); `RECEIVED_MEMORY_ABI.md`
(M11–M14: the bus, `M_ADDR_W`, used ports only, `WRITE_LAT = 1`, the owner's policy);
`ARRAY_PORT_ABI.md` (array ports, option B); `DERIVATION_TABLE.md` memory row and
`CYCLE_DATAFLOW_SEMANTICS.md` §5 (M8–M10: a staging a closing commit, `q = data()` a trailing
commit, m3's denotation); `copper-core/src/memory.md` (M16: questions only); DOCS gap 14
("memory semantics without a timing meaning": `ReadMode::Async`, latencies above 1,
`WriteFirst` only named, write–write collisions unspecified); `memory.rs:324` ("called by the
macro when it infers WriteFirst ordering" — contradicted by CODE (7).6, "no macro code does").
Code does (commit 0138a87): `copper-core/src/memory.rs` — `read(addr)` stages (last call
wins, :290-298), `write` sets stage 0 (:353-361), `on_posedge` runs the read pipelines then
the write pipelines under ReadFirst and the reverse under WriteFirst (:73-91), capture of
`data[addr]` into stage 0 and the staging cleared (:140-160), commit of the oldest write stage
in ascending port order (:94-114), `is_ready()` = output stage `Some`, `data()` panics
otherwise (:303-314), the builders set an atomic flag (:322-335), `ReadMode` has no setter
(:12-16), the listener registered on the clock it is given (:263-264), out-of-range addresses
panic at the call (:116-138). Analysis: `memory_locals` collects declared and received
memories (`cfg.rs:3246`), `check_memory_staging` (a)–(d) over the module loop's CFG
(:2077-2207; the CFG is built over the loop, :287-290, so `s_pre` is no node),
`entry_cycle_mem` for a nested loop (:2352); both front-ends call it (macro :999, transpiler
`lib.rs:531`); the macro's parameter allowlist admits `Memory` in every mode (:1475) and its
CDC arm refuses a foreign-domain memory (:219-247); the combinational arm runs no memory rule
(:1371-1384). Transpiler: the builder read off the declaration (`chir_lower.rs:5736-5742`), a
`Memory::new` array stated as a zero fill (:1930-1936), preloads lowered as a fill loop or
words with a signal read or non-inline fn refused (:2000-2060), `received_memory_decls`
(:1448-1510; `WRITE_LAT ≠ 1` refused :1489, the child's `write_mode` "never consulted"
:1446), stagings take the edge form (`shir_lower.rs:497-511`), the address cast
(`vlir_lower.rs:782-830`), `MemPortUse`'s `data_comb`/`data_reg` by consumer position
(:2520-2620), pipelines and stage registers powered on at 0 (:2913-3024), the read net a plain
array read or a forwarding mux from the committing stage, highest port outermost
(:3077-3112), the guarded commit (:3121-3148), emission as `logic [w] m [0:depth-1]` with an
`initial` block and `assign` read nets (`emit.rs:316-400`), no `$readmemh`, no `ram_style`.
Sweep: `Kind::Memory` skipped with a sentence (`build.rs:693-699`), `REF_SKIP` memory rows
(:239-256), the CPU `SKIP` row (:59-66). Interpreter: `Memory` parameter, binding and port
access `Deferred` (`copper-ref/src/module.rs:199-201`, :527-535). The combinational lowering
registers no memory (`c_stage`, `c_obs` refused by the method registry).
Measurements (every verdict predicted first; `scratchpad/predictions.md` then
`measurements.md`; probes in `scratchpad/i10probes/`, none in the tree; a temporary driver
in `tests/` deleted after the run):
  (i) Corpus (examples/, tests/fixtures/, src/): modules with a `Memory` — predicted 20,
      measured **20** (19 declared, 1 received: `rv32i_cpu_pipelined`); sim-only `#[hardware]`
      memory modules in tests/*.rs: 5. Sweep-skipped for `Kind::Memory` — predicted 1 (the
      brief said "the two CPUs"), measured **1**: `rv32i_cpu_transpilable` declares its memory
      and sweeps under a `RESET` row. Latency pairs — predicted and measured (1,1)×17, (2,2),
      (3,1), (1,2) ×1 each (all three in `pipelined_ram_dut.rs`). Modes — predicted and
      measured `.write_first()` 5 (both CPUs — the pipelined one on the owner's object —,
      `ram_write_first`, `ram_wf_priority`, `ram_r1w2_wf`), `.read_first()` 1, default 14;
      inference sites in the macro or the analysis: **0** (the `memory.rs:324` comment is
      stale). `ReadMode::Async`: 0 uses. Reference leg: 20/20 memory cases ignored (4 REF_SKIP
      rows, 15 by the plain-`Out` class, 1 by the parameter class).
  (ii) Named fixtures — predicted all PASS; measured `write_first_memory_equivalence` 5/5,
      `dual_port_ram_equivalence` 2/2, `preloaded_memory_equivalence` 3/3,
      `pipelined_memory_equivalence` 5/5, `extracted_memory_equivalence` 3/3,
      `multiphase_memory_equivalence` 5/5, `received_memory_abi` 1/1, `front_end_parity` 2/2
      (rule 6 over every clocked corpus module: 0 refusals, predicted 0), `cfg.rs` memory unit
      tests 16/16 (11 rule 6, 5 rule 7), `unsupported_constructs` 33/33 (15 memory cases),
      `rv32i_pipelined_verilator` **PASS** (13 programs, WriteFirst owner).
  (iii) A read and a write of one address on different ports in one resumption, one-job
      spelling (`rdw_wf`, `rdw_rf`: `v`/`q` RegOuts written from `is_ready()`/`data()` at the
      head, then `write(a, d); read(a); tick`), 24 cycles, a hand model of I.10.1 — predicted
      sim = model = SV under each mode; measured **sim = model = SV for both** (trace PASS,
      verilator PASS). WriteFirst delivers `d_{N+1}` at `obs_{N+2}`, ReadFirst the pre-write
      word — the listener order and the emitted forwarding mux / plain read.
  (iv) A pre-loop staging on each route (`prestage_lin`: `read(5); loop { if go { read(addr) };
      tick; v.write(is_ready()); if ready { q.write(data()) }; tick }`; `prestage_ext`: the
      second tick inside `for _ in 0..1`), 16 cycles, `go_N = 1` iff `N mod 4 = 3`, a hand model
      of I.9.3 + I.10.1 — predicted admitted by both front-ends (the CFG ranges over the loop),
      sim = model, SV DIVERGENT: linear at `obs_2` (the drop), extraction at `obs_6` (the
      re-run); measured **exactly that**: both transpile (rule 6 Ok); sim = model on both (trace
      PASS); `prestage_lin` SV diverges at `obs_2`–`obs_3` (`v` 1→0, `q` 22→0; the emitted
      text stages address 5 nowhere); `prestage_ext` SV diverges at `obs_6/7`, `obs_10/11`,
      `obs_14/15` (`v` 0→1, `q` 52/16/28→22; state 0 emits `rom_rd0_addr = 4'(64'd5)` and
      restages it whenever `go` is low on the return to state 0).
  (v) Rule 6 — corpus refusals per clause predicted 0, measured 0. One probe per clause,
      predicted refused by BOTH front-ends with the clause's message; measured **refused by
      both, identical messages** (macro: `cargo check` of the probe crate; transpiler:
      `transpile_source`): `p_a` (a tick-free `for` staging reads) "accessed inside a nested
      loop with no `clk.tick().await`"; `p_b` (two stagings) "read port 0 is accessed 2 times
      in one cycle"; `p_c` (observe, no staging) "nothing stages a `read()` on that port";
      `p_d` (observe before the tick) "read before the `clk.tick().await` that produces it";
      `p_dom` (II.2.4's dominated staging) refused by (b). Direction witnesses: `p_pre_only`
      (a port staged only in `s_pre`, observed under an `is_ready()` guard in the loop) —
      predicted refused by both, over w.r.t. I.9.3; measured **refused by both** via (c);
      `p_guarded` (`if is_ready() { q.write(data()) }; read(a); tick`, the rotation of
      `read; tick; observe`) — predicted admitted by both; measured **admitted by both**.
  (vi) Combinational module and a memory — corpus use predicted 0, measured **0**. `c_obs`
      (`Memory` parameter, `o.write(m.read_port::<0>().data())`) and `c_stage`
      (`m.read_port::<0>().read(x)`) — predicted macro admits, transpiler refuses; measured
      **macro admits both** (no diagnostic), **transpiler refuses both** by accident of its
      registries ("method `data` is not supported in hardware expressions"; "one-argument
      `.read()` on something that is not a memory read port") — a parity break of II.1.3.
Anchor: `dual_port_ram` vs its author's hand SV (ReadFirst collision, one-edge write
visibility, hold when unstaged); `ram_read_first` vs the `REFERENCE` row `ram_read_first.sv`
(hand-written, this repository); `rv32i_cpu_pipelined` under a hand-written WriteFirst owner,
13 programs cycle-exact (the write-first collision and the bus ABI). Sim ≡ SV only: latency
above 1, the write-port priority, the preloads, the extracted and multi-phase shapes, the
address casts, probes (iii)–(iv). No third-party memory reference is vendored
(`bsg_mem_1r1w_sync` is named in `ram_read_first.sv`'s header); the vendor read-first /
write-first / no-change templates are in no prior-art note — a citation is the user's to
supply. Decided, not anchored: the unready observe, the latency generics, the sharing rule,
the combinational refusal.
Prior work: Chisel — `Mem(depth, target, readLatency)`, 0 a combinational read, 1 registered,
`hexInitFile` for contents [chisel §4.3]; Spade — explicit `clocked_memory` (fixed set of
write ports) and `read_memory` entities, "handled separately" by the compiler, never inferred,
motivated by the partial update of large state [spade §III-C]; BaseJump — the read-during-write
policy pinned as an interface parameter, `write_then_read_same_addr_p`, "the write appearing
to go first", realized by wrappers over hard blocks [basejump §3, fn 1]; explicit reset, never
initializers [basejump §3]; Bluespec — a rule is one atomic parallel action, a second write of
one register in one rule a compile error, FIFO ports arbitrated between rules [bluespec §5];
`RegFile`/BRAM not covered by the note; Anvil — the "for every timestamp function" quantifier
as the statement of rule 6's clauses over the CFG [anvil §C.4; note item 4]; Fig. 1's memory
timing hazard is motivation, not a memory semantics; MyHDL — lists of signals become a Verilog
memory array [myhdl §11.6]; Cummings — the nonblocking rule from which ReadFirst falls out
[cummings §4.0], RAM inference not covered; CλaSH `blockRam`, Filament, Prost, Esterel,
RustHDL, PyMTL3, SystemC: not covered. No surveyed language states a memory as a variable of a
coroutine's state machine or shares one memory port between two processes.
Options:
  1. I.10.1 — `data()` in a resumption where the port delivered nothing (sim panics; SV shows
     the stage register, `mem[0]` after a drain):
     A. outside — a run-time failure like an out-of-range address, outside the agreement
        relation, the interpreter's `Runtime` class; `if is_ready()` the spelling — changes:
        none; cost 0.
     B. defined as the hold of the last delivered word (the SV register) — changes: the
        simulator.
     C. defined as zero — changes: both sides.
  2. I.10.1 — `READ_LAT` / `WRITE_LAT`:
     A. kept as parameters of the instance (≥ 1), one pipeline clause for every pair;
        `ReadMode::Async` retired; `L = 0` outside — changes: `ReadMode` deleted (Phase C),
        the sim's `L ≥ 1` check owed; cost 0.
     B. restricted to (1, 1) — changes: the three `pipelined_ram_dut` modules refused, the
        pipelines deleted; cost 3.
  3. I.10.2 — the mode is a property of:
     A. the instance, declared by the builder, default ReadFirst; the inference sentence
        retired; for a received memory the owner's object (the type question to I.13) —
        changes: a doc comment; cost 0.
     B. the read port — changes: a per-port builder and lowering; cost 0 (no use).
     C. the module — no precedent.
  4. I.10.3 — the initial array:
     A. the constant array (`new` zero, `from_contents` words, `from_fn` an expression in the
        index); a run-time preload OUTSIDE; the X memory a question for I.14 — changes: none.
     B. also an X-initialized memory as a form (`Memory::x()` with a `RESET` row) — changes: a
        constructor and a sweep table entry.
  5. I.10.4 (a)/(b) — the dominated staging:
     A. kept as a design refusal (one staging per port per resumption, the conditional the one
        spelling; Bluespec's and Chisel's discipline), (a) narrowed to per port after unrolling
        — changes: (a)'s predicate; cost 0.
     B. admitted, last staging wins, as I.6.2 admits the second `RegOut` write (D-12 D4, D-14
        A1); (a) and (b) retired in Phase C, the tick-free counted `for` handled by `D` —
        changes: two clauses and three pins deleted; cost 0.
  6. I.10.4 (d) — the observe before the tick that delivers it:
     A. kept as a one-spelling design rule (the observe stands after the tick; the rotation
        is the spelling — D1's kind) — changes: none.
     B. retired — the observe reads what the last edge delivered wherever it stands; Phase C
        binds every observe to the delivered register — changes: codegen's `data_comb` form.
     (c): no fork — design, its `s_pre` blind spot fixed in Phase C.
  7. I.10.5 — a received memory shared between modules:
     A. one owner per port: the ports partitioned among the receivers, one domain, two
        modules on one port OUTSIDE — a structural shape rule I.13 states and the parameter type
        eventually carries (the port set and the mode); the bus ABI a dated realization;
        `WRITE_LAT ≠ 1` and the absent mode II.3 rows — changes: none now.
     B. shared ports with a priority (index or poll order) — order-dependent, no anchor.
  8. I.10.6 — the combinational module:
     A. OUTSIDE, both stage and observe; `Memory` not a parameter kind of its header (S0, both
        front-ends, Phase C, a ui case); the Mealy read spelled through a `RegOut` of the staging
        module — changes: a header rule (Phase C); cost 0.
     B. observe admitted (delivered state, read as a `RegOut` is), stage outside — needs the
        shared memory 7A refuses and a new child ABI; nothing anchors it.
  No fork (stated and written): I.10.1(c) the edge step and the write-port priority (highest
  index wins, sim ≡ SV); I.10.1(d) the observe reads what the last edge delivered (I.6.4's
  form); I.10.3's pre-loop staging as the first resumption's, no new II.3 row (f2, f3 close it);
  I.10.4 (c) design with its direction; I.10.7's map and the interpreter obligation.
Recommendation: 1A, 2A, 3A, 4A, 5B, 6A, 7A, 8A — the memory is a variable of H(M) with
effects consumed at the edge like a `RegOut`'s pending value and observations of committed
state; a dominated staging is the lint D-12 and D-14 declined for the other two port kinds,
while the observe's position is the spelling rule D1 kept; sharing a port has no
order-independent meaning; and a combinational module has no resumption to stage in.
Decision: **1A, 2A, 3A, 4A, 5B, 6A, 7A, 8A.** User, 2026-11-05, the recommended options as
offered.
Changes: none now. Phase C obligations (I.10.7): the interpreter's memory model (I.10.1(a)–(d),
I.10.2's order; `observe` a `Runtime` failure when not valid; the four `REF_SKIP` memory rows
deleted, `Deferred` narrowed to the parameter); rule 6 clauses (a) and (b) deleted with their
pins, (c) extended to `s_pre`, (d)'s message citing I.10.4; the I.10.6 header rule in both
front-ends with a ui/fail case; `ReadMode` deleted and `L ≥ 1` checked in the simulator;
`memory.rs:324` corrected; the pre-loop staging through the entry of I.9.2 with probes (iv) as
fixtures; `WRITE_LAT ≠ 1` on a received memory as a II.3 row (REFUSED-one, the macro owing
the refusal) and the mode owed to the parameter type (I.13); the `data_comb`/`data_reg`
binding reduced to the delivered register with the trailing fold; rule 7's code, ui case,
tests and vlir RULE pin deleted (II.3.2). Spec: I.0.8's memory item, I.6.10's M9, I.7.7's and
I.8.7's memory items, I.9.7's pre-loop staging item, I.11.7's rule-7 item, II.1.5's and
II.2.4's and II.3.7's memory items close; II.3.2 rows 6 and 7 are placed.
Obligations: II.3's two rows for the received memory (`WRITE_LAT ≠ 1`; the mode absent from
the type) written into II.3.3 when that section is next edited (dated 2026-08-27 and this
entry); I.12 — `Memory` in the CDC rule, a memory crossing domains; I.13 — the port set and
the mode in the parameter type, the one-owner-per-port rule enforced, the bus as a hierarchy
realization; I.14 — X in a memory, the address width, the out-of-range address, a port index
`k ≥ R`; III.2 — an independent reference for a RAM with a write (`bsg_mem_1r1w_sync`) and
for a two-cycle read (the user writes references), the interpreter's memory leg and G-F,
`obs_0` of a memory module; the probes of this entry (`rdw_wf`, `rdw_rf`, `prestage_lin`,
`prestage_ext`, `p_pre_only`, `p_guarded`, `c_obs`, `c_stage`) as fixtures when their rules and
forms land; `design_docs/RECEIVED_MEMORY_ABI.md` and `SYNCHRONOUS_SEMANTICS.md`'s memory
section gain a pointer to I.10 (the coordinator's edit); the brief's "the two CPUs" corrected
to one in whatever carries it.
Coroutine terms: a memory is a variable of H(M) that only an edge changes — an array and a
delivery pipeline per read port; a resumption's stagings are its effects on the ports,
consumed at the edge it ends at, the last per port winning; an observe reads what the last
edge delivered; the edge captures then commits (ReadFirst) or commits then captures
(WriteFirst); a received memory's ports are each one module's; a combinational module has no
resumption to stage in.

### D-17 — Synchronizers and clock domains                          (2026-11-05)
Clause: SPEC §I.12 (new, replacing the TBD heading); `(D-17)` notes at every pointer into
I.12 — I.0.2, I.0.8, I.1.1, I.1.6, I.2.4 (the port row), I.2.7, I.2.8 ×2, I.6.8, I.6.10,
I.7.2, I.7.7, I.9.5, I.9.7, I.10.1(c), I.10.5, I.10.8, II.1.2 ×2 (R10; the rule-placement
paragraph), II.1.4 ×2 (the macro map, the pins), II.1.5. `review/DECISION_AUDIT.md` rows 53 and 97 resolved in one
line each. Nothing else changed.
Question: What is a clock domain as the unit of time, and what may the environment do with
two of them; what has a module done before its own clock's first edge (the audit's pending
accident, D-7); what a `synchronizer` module means — what its read of a foreign wire yields,
where metastability lies, whether the two-flop structure is a rule; what may cross a domain
(a `RegOut` wire, a memory, a combinational module's ports, a structural parent's nets, a
multi-bit value); whether the two-clock sequential module stays refused; and what the
interpreter, I.13, I.14, III.2 and Phase C inherit?
Docs say: SPEC I.0.2 and I.0.8 (per-domain gating and the never-ticked domain deferred), I.1.1
(per-domain numbering, "no relation between the edges of two domains is assumed"), I.1.6 (the
global cycle counter an artifact), I.2.2/I.2.7 (the synchronizer: "the kernel with the
single-domain clause replaced by I.12's"), I.2.4/I.2.8 (unit-domain `In<T>` NOT YET; the
two-clock module NOT YET, refused by decision in D-12), I.3.6 and I.7.5 (`sync_2ff` anchored:
`ff1` a register, `q` a `RegOut`, `ff2` a wire), I.6.8/I.6.10 (the value a consumer in another
domain reads), I.7.2/I.7.7 (the synchronizer's registers by the same rule), I.9.5/I.9.7 (the
first settle of any domain), I.10.1(c)/I.10.5/I.10.8 (a memory advances on its own domain's
edges; a foreign memory refused; no dual-clock RAM form), I.11.1 (a combinational module has no
domain of its own), II.1.2 R10/R17, II.1.4, II.1.5, II.2.3, II.2.4. `copper-core/src/cdc.rs`
(the executable CDC specification: nominal domain mismatch E0308, one driver, the legal
synchronizer path, GAP 1 closed by `check_cdc`, GAP 2 by `check_no_clock_construction`);
`src/sync.rs`'s header ("restricted to single-bit `Logic`"; a user synchronizer "takes
responsibility for the timing correctness"); `examples/cdc/two_domain_counter.rs`'s prose
("two destination cycles" counted from a fast register, vs the anchor's one);
`review/DECISION_AUDIT.md` rows 53/97 ("per-domain gating, never-ticked default — accident,
pending; must be decided from §1, not described"). D-1 A2, D-7, D-8 (the barrier's domain
keying gone), D-9 (`ff2` a wire; tick-last rewrite owed), D-12 (R10), D-16 A7 (one domain per
memory).
Code does (commit 0138a87): `copper-core/src/types.rs` — `ClockDomain` a marker (:850);
`Clock<D>` an `Arc`'d cycle counter with listeners (:863-927), `advance` the edge (:886);
`ClockTick::poll` Ready iff `cycle ≥ target` and the domain is resolving (:985-1008);
`TICK_RESOLVING` per domain, **default true** for a domain never set (:966-983).
`copper-core/src/port.rs` — `Out<T, D = ()>` (:79), `In<T, D = ()>` (:109), no bound on `D`;
`registered_wire` requires `D: ClockDomain` (:220). `copper-sim/src/lib.rs` — `POLL_PHASE` per
domain, **default pre-edge** (:36-51); `PreEdgeBarrier` Ready iff pre-edge (:101-117).
`copper-sim/src/executor.rs` — nothing polled at spawn (:364-416); `poll_tasks` polls **every**
task (:520-575); `tick_clock::<D>` = pre-edge settle (phase, resolving off, poll) then advance,
post-edge settle (phase, resolving on, poll), global `cycle += 1` (:813-853).
`copper-macros/src/lib.rs` — `check_cdc` (:134-259): native domain = the one clock's, a port
or memory in another domain refused, **skipped when the clock count ≠ 1** (:207-210), `()`
treated as domain-agnostic (:145-176); called for `sequential` only (:943-947); the
combinational arm runs no domain rule (:1371-1384); the read-gating fallback to the one clock
(:811-825); `validate_hardware_fn` requires the loop in the `synchronizer` mode (:1936).
`copper-codegen` — `synchronizer` parsed as a mode and lowered as `sequential`
(`parser.rs:98`); the structural call-site domain check (`chir_lower.rs:2286-2310`).
`build.rs` — a second `Clock` port a `SKIP` (:702-708), `two_domain_top` a `SKIP` row
(:118-121), foreign domains declared by the wrapper (:884-931), `REF_SKIP`'s multi-clock class
(:362, :769-773) empty. `copper-ref/src/module.rs` — more than one clock deferred (:211-215).
Measurements (every verdict predicted first; `scratchpad/i12probes/predictions.md` then
`measurements.md`; probes in `scratchpad/i12probes/src/`, none in the tree):
  (i) The never-ticked domain — `b_reg` in `B` (`let t = x.read(); q.write(t); tick`, `q` a
      `RegOut`), a side-effect counter twin, a plain-`Out` twin, `fast_counter` in `A`; `x = 1`;
      `A` ticks ×3; `x = 2`; `B` tick. Predicted: resumptions 0 after spawn, 1 after `A`'s first
      tick and unchanged after the next two, `q = 0` until `B` ticks, then **`q = 1`** (the value
      read in `A`'s settle), the plain-`Out` twin showing 1 before `B` ever ticked; Verilator on
      the transpiled module `q = 2`. Measured **exactly that**: 0 / 1 / 1 / 1; `q = 0, 0, 0`;
      after `B`'s edge 1 resumptions 2, `q = 1`, `o = 1`; next edge with `x = 3`: `q = 3`;
      control with no `A` ticks `q = 2`; Verilator `q = 2` then 3. Today sim ≠ SV at startup
      whenever another domain ticks first and the input changes in between; no rule, pin or
      sweep case covers it (the sweep drives one clock).
  (ii) Anchors — predicted all PASS; measured `cdc_synchronizer_anchor` 10/10, `cdc_crossing`
      2/2, `two_domain_counter_behavioral` 5/5, `flag_crossing_behavioral` 5/5,
      `two_domain_hierarchy_cdc` 3/3 (sim ≡ transpiled hierarchy ≡ `two_domain_ref` at 2:1),
      `reference_leg_measurements` 6/6 (the tick-last poison run: `ff2` dead, `ff1` live);
      corpus `ex_cdc_*` and `fx_sync_2ff_dut` 8 passed, 10 ignored (every `*_reference_leg` by
      the plain-`Out` class; `two_domain_top` both legs by its `SKIP` row).
  (iii) Corpus — unit-domain ports in `examples/`, `src/`, `tests/fixtures/`: predicted 0,
      measured **0**; `tests/*.rs`: **0**; elsewhere 13 scaffolding sites (`copper-sim/src/
      executor.rs` unit tests, 6 sites in 4 helpers; `copper-macros/tests/ui` `function_typed_*`,
      7 sites in 6 files). `Bits<N>` ports on a synchronizer: predicted 0, measured **0**.
      Synchronizer-mode modules: 9 occurrences (`sync_2ff`, `flag_sync` ×2, `sync_2ff_concrete`,
      the `structural_hierarchy.rs` copy, `sync_3ff` doc test, `sync_2ff_tick_last` — both ports
      in one domain —, `sync_1ff`); bodies with fewer than two edges between the foreign read
      and the output: predicted 0 shipped + `sync_1ff`, measured **0 + `sync_1ff`**. A rule
      enforcing two flops: predicted none, measured **none**.
  (iv) One-flop synchronizers — `sync_1ff` (two-job spelling), `sync_1ff_oj` (`ff1 = d.read();
      q.write(ff1); tick`, `RegOut`), `sync_guard` (`if d.read() == 1 { ff1 = 1 }; q.write(ff1);
      tick`), `sync_2ff_oj` (control): predicted admitted by both front-ends; measured **macro
      admits all four, transpiler admits all four**; `sync_1ff_oj` emits `always_ff … q <= d`.
  (v) A combinational module and a foreign wire — `c_mixed(a: In<Logic, A>, o: Out<Logic, B>)`:
      predicted macro admits, transpiler admits (the brief's "type error" is the wiring's);
      measured **admitted by both** (`assign o = a`). Wiring an `In<Logic, A>` into a port
      `In<Bits<8>, B>`: predicted E0308; measured **E0308** ("expected `In<Bits<8>, B>`, found
      `In<Logic, A>`").
  (vi) Interleavings — `two_domain_counter` under P = [2;10] and Q = [1,3,1,3,2,2,3,1,2,2]
      (twenty fast, ten slow ticks): predicted the fast trace per fast edge equal and the slow
      trace per slow edge equal (the flag standing at each slow edge is the same); measured
      **equal / equal** (flag raised at slow 4, `sync_q` and the consumer at slow 5 under both;
      only the fast count seen at each slow edge differs). SV agrees under the harness's 2:1
      driving (`two_domain_hierarchy_cdc` PASS).
Anchor: `sync_2ff_concrete` vs the hand-written `examples/cdc/sv/sync_2ff_ref.sv` (one
destination cycle through two flops, rate independence, dropped and captured pulses — ten
cases) and `two_domain_top` vs `examples/cdc/sv/two_domain_hierarchy.sv` (2:1). No independent
reference covers the never-ticked startup, a multi-bit crossing, a two-domain design under
another interleaving, or a one-flop body (the hardware's answer is the flop's sample at its
own edge, measured on the transpiled probe). Decided, not anchored: the environment's
interleaving freedom, the never-ticked clause, the two-flop rule's statement, the unit domain's
retirement, the combinational module's one domain, the per-bit meaning.
Prior work: RHDL — `Signal<T, Red>` phantom domains, "Clock domain crossings require special
constructs provided in the RHDL core library" [rhdl §2.2], the ancestor; Arch — domain-tagged
`Clock<D>` [arch §3.4], "separate clock domains run on dedicated threads, synchronizing only at
CDC crossings" [arch §7.3.4], `--cdc-random` randomizes synchronizer latency [arch §7.1], no
formal model of multi-domain time; Filament — all events in one clock domain [filament fn. 2];
Prost — "coroutines are equivalent to single-clocked synchronous circuits", multi-clock not
supported, SystemC's domain splitting cited [prost §2, §5]; CλaSH — single-clock, "work is in
progress to add an abstraction mechanism that allows the modeling of asynchronous and
multi-clock systems" [clash §I]; Chisel — implicit single global clock [chisel §2.5]; Spade —
one explicit clock per register [spade Listing 1]; Bluespec — "No mention of clocks" [bluespec
slide 14]; BaseJump — CDC modules listed separately (`bsg_async_fifo`, `bsg_launch_sync_sync`,
`bsg_sync_sync`, `bsg_async_ptr_gray`, `bsg_async_credit_counter`) [basejump §6 items 9–10],
single-clock synchronous modules otherwise [basejump §3]. Not covered by the notes: Clash's
`Clock dom`/`Signal dom` model, Chisel's `withClock`, SystemC's domain splitting, the internals
of the BaseJump CDC modules, Cummings' CDC paper, metastability and multi-bit coherence. No
surveyed language states what a module has done before its clock's first edge.
Options:
  1. I.12.1 — the unit domain `()` (`In<T>`; 0 corpus uses, 13 scaffolding sites):
     A. retired — every port and clock names a domain; `In<T>` OUTSIDE (S0); Phase C drops the
        `D = ()` default, puts `D: ClockDomain` on every port type and gives the 13 sites a
        domain — changes: types, macro (`check_cdc`'s `()` clause); cost 0.
     B. kept as the domain of a module with no clock (combinational) — changes: a rule for
        what a `()` port may connect to; the type system today lets it connect to anything.
  2. I.12.2 — the never-ticked domain (measured: today the first settle of ANY domain runs
     every module's first resumption; sim 1 vs hardware 2):
     A. a module runs only at its own domain's instants; before its first edge it has executed
        nothing, its `RegOut`s show `init(q)`, `obs_0` of `D` exists once `D` ticks; the
        thread-local defaults are realization details — changes: the executor keys resumption
        on the domain (Phase C), probe (i) a two-clock pin; cost 0 corpus modules.
     B. the first settle of any domain runs every module's first resumption (today) — refused:
        the measured divergence, and §1 says the clock resumes the coroutine.
  3. I.12.3 — the two-flop structure:
     A. a realizability rule of II.1: no `RegOut` write may depend, by value or guard within
        one resumption, on a foreign read of that resumption — the value crosses a register,
        then the port — changes: a rule in `copper-analysis`, both front-ends, a ui case; refuses
        `sync_1ff`, `sync_1ff_oj`, `sync_guard`; cost 0 shipped, 1 test-only (`sync_1ff`, which
        becomes a refusal pin).
     B. any body; two flops a convention of `sync_2ff` — changes: none.
  4a. I.12.4(c) — a combinational module with ports in two domains (admitted by both today):
     A. all ports of a combinational module lie in one domain (S0); refused in both front-ends
        in Phase C — changes: `check_cdc` on the combinational mode, a ui case; cost 0.
     B. admitted as today; only the wiring checked.
  4b. I.12.4(e) — a `Bits<N>` foreign port on a synchronizer (corpus 0):
     A. admitted with the per-bit meaning, the coherence hazard documented, a coherent crossing
        the user's design (gray code, handshake) — changes: none; cost 0.
     B. `N > 1` refused (S0); a handshake the only multi-bit form — changes: a rule; cost 0.
  No fork (stated and written): I.12.1's environment (any interleaving, including none) and
  per-domain determinism (measured (vi)); I.12.3's meaning of the foreign read (the last value
  `D_src` committed before `D_dst`'s instant; metastability outside the model, the two flops
  the realization's agreement with it) and the one-job normative spelling (D-8, D-9); I.12.4's
  one crossing, the memory that does not cross (D-16 A7) and the dual-clock memory as a NOT-YET
  row of I.13, the structural parent as I.13's; I.12.5's confirmation of R10 (D-12).
Recommendation: 1A, 2A, 3A, 4aA, 4bA — the clock resumes the coroutine (§1), so a domain
that has not ticked has run nothing and a domain of no clock names nothing; the two-flop
structure is the realizability principle applied to the one asynchronous read the language
admits; a combinational path between domains is a crossing with no synchronizer; and the
per-bit crossing is what the reference admits, with its hazard said rather than hidden.
Decision: **1A, 2A, 3A, 4aA, 4bA.** User, 2026-11-05, the recommended options as offered.
Changes: none now. Phase C obligations (I.12.6): (1) the executor resumes a coroutine in its
own domain's settles only (tasks keyed by their clock's domain; the per-domain thread-locals
deleted with the barrier), probe (i) a two-clock pin expecting `q = 2`,
`source_domain_ticks_do_not_advance_the_synchronizer` kept; (2) the two-flop rule in
`copper-analysis`, both front-ends, ui case from `sync_1ff_oj`, `sync_1ff` a refusal pin; (3)
the combinational one-domain rule, both front-ends, ui case from `c_mixed`; (4) the unit
domain removed (`D = ()` default gone, `D: ClockDomain` on every port type, 13 scaffolding
sites, `check_cdc`'s `()` clause); (5) R10 refused in both front-ends (D-12); (6) `sync_2ff`,
`flag_sync` and the fixture twin tick-last (D-9), `src/sync.rs`'s header corrected; (7)
`REF_SKIP`'s multi-clock class and the sweep's second-clock `SKIP` restated; (8) the executor's
global `cycle` counter per domain or renamed (I.1.6). Spec: I.0.8's per-domain item, I.1.6's
counter item, I.2.4's and I.2.8's unit-domain and two-clock rows, I.6.10's foreign-reader item,
I.7.7's synchronizer item, I.9.7's never-ticked item, I.10.8's CDC item, II.1.5's CDC item
close; II.1.2 R10 is confirmed; the audit's two rows are resolved.
Obligations: I.13 — the structural wiring of domains and the dual-clock memory (a
synchronizer-kind primitive with no form); I.14 — X on a crossing wire; III.2 — the
environment of a two-domain design as a stated interleaving of per-domain `resume`/`edge`
calls, an independent reference for one beyond the 2:1 anchor and for the never-ticked startup
(the user writes references); the interpreter's multi-clock leg needs only I.12.1 (Phase C);
II.1 — the two-flop rule as a row of II.1.2 (realizability, its message naming I.12.3) and the
combinational one-domain rule as an S0 row when that section is next edited; II.3 — no row
(the never-ticked divergence is a simulator defect of the realization, not a transpiler form,
and is owed to Phase C (1) with its pin); the probes of this entry (`b_reg`, `sync_1ff_oj`,
`sync_guard`, `c_mixed`) as fixtures or ui cases when their rules and the executor change land;
`examples/cdc/two_domain_counter.rs`'s "two destination cycles" prose reconciled with the
anchor's one-cycle figure (the coordinator's edit, outside this packet).
Coroutine terms: a module's coroutine is resumed by its own domain's edges and by nothing
else, and before its first edge it has run nothing; the environment is any interleaving of
the domains' ticks; a synchronizer's foreign read yields the last value the source domain
committed before the synchronizer's instant, and a `RegOut` may be written from it only in a
later resumption than the one that read it; a combinational module lies in one domain; a
multi-bit crossing is N single-bit ones.

### D-18 — Structural modules, wiring and the settle                (2026-11-05)
Clause: SPEC §I.13 (new, replacing the TBD heading); `(D-18)` notes at every pointer into
I.13 — I.0.2, I.0.3, I.1.2, I.1.3, I.2.7, I.4.1, I.4.8, I.5.10, I.6.8 ×3, I.6.10, I.7.1,
I.10.5 ×4, I.10.7, I.10.8 ×2, I.11.1, I.11.3, I.11.4 ×4, I.11.6, I.11.7 ×2, I.12.2, I.12.4 ×3,
I.12.5, I.12.6, I.12.7 (36 lines). Nothing else changed.
Question: What is a design made of several modules — what a wire is, who drives it, how the
combinational network settles at an instant and in what order relative to the resumptions and
the edge, whether the simulator's evaluation order and count can be observed; what a
`structural` module means and which shapes it admits, how a net is spelled, whether a
behavioral body may instantiate a child; and what the emitted hierarchy and its ABIs owe?
Docs say: SPEC I.0.2–I.0.3 (the settle after the edge), I.1.2–I.1.3 (the network settles to a
fixpoint before the instant ends; every read sees settled values), I.2.7 (`structural`: "a
wiring language … I.13 gives it meaning"), I.4.1/I.4.8 (the settled value, sim ≡ SV only),
I.6.8/I.6.10 (a `RegOut` a source, never a combinational dependency; W6 one driver; the dirty
flag a scheduler artifact), I.10.5/I.10.8 (one owner per memory port, the bus ABI as a
hierarchy realization, the port set and mode owed to the type), I.11.1/I.11.4/I.11.6/I.11.7
(the evaluation count a scheduler artifact; the cycle OUTSIDE; "an evaluation is a function of
its reads" contributed; hierarchy inside a combinational body), I.12.4(d)/I.12.5/I.12.7 (the
structural parent wires domains; a two-clock design is a structural composition; the dual-clock
memory), II.1.2 R22. `review/INVENTORY_DOCS.md`: SC1–SC9 (poll-order independence pinned; the
levelized default, fixpoint the oracle; comb-only edges; a wire a shared cell; convergent loops
legal; both schedulers agree per phase), H1–H3 (a structural module transpile-only, inert, needs
a clock; `wire::<T,D>(init)` with `n.0`/`n.1`; deepest-first emission), H4 (the array port),
X2/X4/X7, DOCS 27 (the net's `init` has no stated meaning); `LEVELIZED_SCHEDULING_SCOPE.md`
(approach A: spawns declare reads; comb-only edges; phase 5 "detection is not rejection");
`ARRAY_PORT_ABI.md` (B, 2026-08-24), `RECEIVED_MEMORY_ABI.md` ((a), 2026-08-27);
`review/DECISION_AUDIT.md` rows 54/98 ("a fixpoint re-poll runs no statement" — pending). D-8
(one job; no plain `Out` on a sequential module), D-14 (the cycle OUTSIDE; the never-written
port; "hierarchy inside a combinational body" → I.13), D-16 A7 (one owner per port), D-17
(own-domain resumption; the structural parent and the dual-clock memory → I.13).
Code does (commit 0138a87): `copper-core/src/port.rs` — `WireId` the cell pointer (:14);
`WireKind::Comb`/`Registered` on every `DirtyHandle` (:28-61); `Out` not `Clone` (:64-105),
`In` `Clone` (:108-134), `wire` (:136-150), `RegOut` committing at the edge, dirty on change
(:168-218), `registered_wire` (:220-235). `copper-sim/src/executor.rs` — `spawn_wired` records
dirties and reads (:401-416); `spawn_child` untracked (:427-465); `comb_dependency_graph`
producer→consumer for `Comb` handles in `reads`, a later producer silently replacing an earlier
one (:657-687); `compute_scc_plan` Tarjan + Kahn (:700-751); `poll_tasks_levelized` one poll per
singleton, dirty flags discarded (:546-572); `iterate_scc` (:588-632); `poll_tasks_fixpoint`
re-polls until no dirty, `PollOrder` (:753-803, :226-335); `comb_cycles` detection only
(:490-496); `tick_clock` = pre-edge settle, advance, post-edge settle (:813-854).
`copper-macros/src/lib.rs` — the structural arm emits the fn unchanged, no `HardwareModule`
(:1385-1398); the clock rule (:1496-1503); R19 refuses an `In` passed on; the combinational
wrapper loop (:853-868). `copper-codegen/src/chir_lower.rs` — `lower_structural_body` nets and
instantiations only (:2129-2183); `parse_wire_net` matches the substring `wire::<` and ignores
`init` (:2187-2213); `lower_structural_inst` arg count and call-site domain check (:2243-2333);
name validation (:5234-5252); the legacy expression form `lower_hardware_call` with a `.out`
port (:4961-5008, from :3785). `emit.rs` instances and nets (:436-490); `lib.rs`
`transpile_source_hierarchy` deepest-first (:173-196, :304-318). `build.rs` — `two_domain_top` a
`SKIP` row (:118-121).
Measurements (every verdict predicted first; `scratchpad/i13probes/predictions.md` then
`measurements.md`; probes in `scratchpad/i13probes/src/bin/`, emitted SV in `sv/`, none in the tree):
  (i) Scheduler agreement — predicted all PASS, identical traces; measured: default
      `levelized_differential` 6/6, `poll_order_fuzz` 2/2, `golden_traces` 6/6,
      `module_composition_hybrid` 3/3, `received_memory_abi` 1/1, `two_domain_hierarchy_cdc` 3/3,
      codegen `structural_hierarchy` 5/5; under `COPPER_SCHEDULER=fixpoint` `golden_traces` 6/6
      (byte-identical), `levelized_differential` 6/6, `two_domain_hierarchy_cdc` 3/3; a 65-case
      corpus sample under fixpoint **65 passed, 0 failed, 34 ignored** (the same `SKIP`/`REF_SKIP`
      rows as under the default).
  (ii) Evaluation counter — a `RegOut` counter → `cnt_comb` (counting its evaluations) → sink,
      10 ticks: predicted levelized 20, fixpoint 30, traces equal; measured **20 / 40 / equal**
      (the oracle's pre-edge settle also takes two passes: the `RegOut`'s edge-set dirty flag is
      consumed one settle late — the count missed, the direction held).
  (iii) Structural shape probes, both front-ends (M predicted → measured / T predicted →
      measured): `s0` control admit/admit ✓; `s1` statement, `s2` port read: M admit ✓, T refuse
      ✓; `s3` unconnected port: refuse E0061 ✓ / refuse ✓; `s4` two drivers: refuse E0382 ✓ /
      **admit** ✓ (Verilator MULTIDRIVEN); `s5` foreign net: refuse E0308 ✓ / refuse ✓; `s6a`
      `RegOut` port through `wire`: refuse E0308 ✓ / admit ✓ (no port-kind check); `s6b` through
      `registered_wire`: admit ✓ / predicted refuse, **admitted** (`parse_wire_net`'s substring
      match; `logic n;` emitted correctly); `s7` one `Memory` to two children: refuse E0382 ✓ /
      predicted admit, **refused** (a `Memory` parameter is no signal of the lowering); `s8b` no
      clock: refuse ✓ / **admit** ✓; `s9` undriven net: admit ✓ / admit ✓ (UNDRIVEN); `s10` an
      `In` moved twice: refuse E0382 ✓ / admit ✓; `s11` with `.clone()`: admit/admit ✓; `s12` a
      plain fn call: admit ✓ / refuse ✓; `s13` self-instantiation: predicted refuse E0733,
      **admitted** (the dropped future is no recursion) / admit ✓ (Verilator UNSUPPORTED).
      Misses: 2 of 16 T verdicts, 1 of 16 M verdicts.
  (iv) A behavioral body instantiating a child — `b1` (an `In` passed on): M predicted admit,
      **refused** by R19; `b1b` (a `wire()` net in the body): M admit ✓, T refuses "cannot infer
      bit width" before any instantiation; `b2` (`one(o)` moving a parent `Out`): comb parent M
      predicted admit, **refused** E0382 by the wrapper loop; seq parent M admit ✓ (the child's
      future dropped, never runs); T admits both ✓ — comb emits `.out(one_0_out)` (Verilator
      PINNOTFOUND), seq **drops** the pre-loop call (`o` UNDRIVEN). Corpus: predicted 0 behavioral
      bodies calling a `#[hardware]` fn, measured **0** (two same-name plain helpers).
  (v) Anchors — predicted PASS; measured `two_domain_hierarchy_cdc` 3/3, `received_memory_abi`
      1/1, `structural_hierarchy` 5/5, `module_composition_hybrid` 3/3.
  (vi) Corpus — structural modules **1**; `.spawn_wired(` sites examples **40**, tests **185**
      (+43 executor unit tests, +3 copper-sim docs); `spawn_child!` **2** (one test), method calls
      elsewhere **0**; `.spawn_untracked(` **7**; files with ≥ 2 `spawn_wired` **41** (examples 6).
Anchor: `two_domain_top` vs `examples/cdc/sv/two_domain_hierarchy.sv` (hand-wired simulation ≡
transpiled hierarchy ≡ independent reference, 2:1) for the structural module and the emitted
hierarchy; the Mealy composition of D-14 (v) for the settle's reading of a `RegOut`. Sim ≡ SV
only: the received-memory bus, the two schedulers' agreement. Decided, not anchored: the
structural module's meaning, the structural-only instantiation form, the kind-agnostic net, the
order of settle and resumptions (a consequence of D-8 and RO6/M15).
Prior work: Esterel — causality by a static acyclic signal-dependency graph, a unique solution
not enough (P6) [esterel §7]; Arch — a static DAG across comb blocks and instance connections,
a bounded settle "not a fixpoint iteration", single-driver rule, level barriers [arch §3.8,
§7.3.1, §7.3.4]; PyMTL3 — static topological schedule vs event-driven pass, `connect` [pymtl3
§2–§3, Fig. 2]; Bluespec — linearizable parallel firing, composition by methods [bluespec slides
14–15]; Chisel — `:=` wiring with exactly one connection, one value per wire per cycle, loops
not discussed, instances as graph nodes [chisel §2.2, §2.4, §2.5]; CλaSH — application is
instantiation, no settling model [clash §II-A, §II]; Spade — `inst` sub-pipelines [spade Listing
3]; Filament — `x := new C`, connections as continuously active wires, parallel composition,
Lemma C.18 [filament §3.5, §6.1]; Prost — no instantiation, a sub-task call is inlined [prost §1,
§4]; RustHDL capped fixpoint vs RHDL registers-break-loops [rhdl §3–§4]; MyHDL inferred
sensitivity [myhdl §6.2.1]; Cummings — the event queue and NBA update, `always_comb`
re-evaluation [cummings §4.0, §5.3, §6.0]; SystemC — the delta cycle is IEEE 1666's, not the
subset's [systemc §4.2.2]. Not covered by the notes: FIRRTL's combinational-loop check, Clash's
`Signal` composition, SystemC port binding, Bluespec's method-level combinational paths.
Options:
  1. I.13.4(a) — the structural module's meaning (corpus 1; anchor PASS):
     A. a name for the design it denotes — its instances and wires; hand wiring and the emitted
        hierarchy are two realizations that must denote the same design (III.2); sim-as-unit a
        realization form deferred with a date; the shape rules S0 rows enforced in both
        front-ends — changes: Phase C rules (T: one driver, every net driven, port kind, clock;
        M: body form); cost 0.
     B. simulate as a unit now — the macro lowers the body into spawns and a per-domain tick
        driver — changes: a macro lowering, wire-id plumbing, a two-clock driver; cost 0 modules,
        41 hand-wired test designs unchanged.
  2. I.13.5 — instantiation inside a behavioral body (corpus 0; measured: the sim never runs
     the child, the transpiler emits a missing port or drops the call):
     A. a structural-only form (S0): no instantiation and no net in a behavioral body; a body
        that needs a child is a structural parent over two modules — changes: both front-ends
        refuse; `lower_hardware_call`, the `submodules` of behavioral bodies, `spawn_child`
        retired; cost 0.
     B. admitted in the kernel — a new grammar form, a macro lowering that spawns and wires, a
        transpiler form — Phase C cost of its own, no corpus demand.
  3. I.13.4(c) — the net's spelling (`wire::<T,D>(init)`; `registered_wire` admitted by accident;
     `init` ignored by the transpiler; under D-8 every sequential child port is a `RegOut`):
     A. one kind-agnostic net with no initial value, `net::<T, D>()`, the cell kind following the
        driver; until Phase C `init` is a III.2 precondition (the zero of `T`) and
        `registered_wire` the sanctioned spelling for a `RegOut` net — changes: `copper-core`,
        both front-ends, the one structural module; cost 0.
     B. two spellings by cell kind, `init` required to be zero and checked by both — changes: a
        rule in both front-ends.
  No fork (stated and written): I.13.1 (a design as coroutines and functions over wires; one
  driver, any readers; the three driver kinds; type-checked wiring), I.13.2 (the settle a
  function of the sources; the order drive → settle → resumptions → edge → settle → observation;
  a settle resumes no coroutine; order independence a theorem and III.1's proposition), I.13.3
  (the evaluation count unobservable; the dirty flag the oracle's rule), I.13.6 (the hierarchy
  and the two ABIs as dated realizations; III.2's one obligation).
Recommendation: 1A, 2A, 3A — a structural module that executes nothing can only name the
design its parts already mean; one spelling per meaning puts instantiation in the mode built for
it and gives a net, which has no value of its own, no initial value; and every rule the two
front-ends enforce unevenly today is a parity break of II.1.3 to be closed, not a meaning.
Decision: **1A, 2A, 3A.** User, 2026-11-05, the recommended options as offered.
Changes: none now. Phase C obligations (I.13.7): (1) the structural shape rules in both
front-ends (T: one driver per net, every net driven, the port kind, the clock rule; M: the body
form, the plain fn call; both: self-instantiation), ui cases from `s1`, `s4`, `s6a`, `s8b`,
`s9`, `s13`; (2) `net::<T, D>()` in `copper-core`, both front-ends and `two_domain_top`,
`wire`/`registered_wire` retired from the structural grammar, `parse_wire_net`'s substring match
deleted; (3) instantiation and nets refused in behavioral bodies in both front-ends (ui case from
`b2`), `lower_hardware_call`, the behavioral `submodules` fields, `spawn_child`/`spawn_child!`
deleted, `module_composition_hybrid.rs` rewritten to `spawn_wired`; (4) the D-14 cycle refusal
at spawn and over the emitted hierarchy; (5) the executor's `tick_clock::<D>` in I.13.2(b)'s
order with the own-domain resumption (D-17) and no barrier (D-8); (6) the received memory's port
set and mode in the parameter type, checked at the structural call site (I.10.8); (7) sim-as-unit,
if ever built, a III realization with a differential test against the hand wiring; (8)
`LEVELIZED_SCHEDULING_SCOPE.md` and `SYNCHRONOUS_SEMANTICS.md`'s poll-order section pointed at
I.13.2 (the coordinator). Spec: I.0.8's "poll_tasks outside a tick" item stays with III.2;
I.6.10's source item, I.11.7's settle and hierarchy items, I.12.7's structural-wiring item close;
the dual-clock memory is I.13.8's row.
Obligations: III.1 — order independence as the levelized scheduler's proposition, with the
differential harness, the fuzzer and the goldens as its evidence; III.2 — the design as the
environment of several interpreter instances over shared values (the functions evaluated per
I.13.2), hand wiring ≡ hierarchy as the structural obligation, the net's zero initial value and
the input wire's creation value as preconditions, an independent reference for a hierarchical
design beyond the 2:1 anchor (the user writes references); I.14 — X through the settle (a
function over an X input, a `RegOut` holding X read by a function, X on a net); II.1 — the
structural S0 rows of I.13.4(d) and the behavioral-body refusal of I.13.5 as rows when next
edited, with the parity column; II.3 — no row (every structural gap is a shape rule one
front-end lacks, owed to Phase C (1)–(3), not a transpiler form); `review/DECISION_AUDIT.md`
rows 54/98 resolved by I.13.2(b) (the coordinator's edit); the probes of this entry (`s1`, `s4`,
`s6a`, `s8b`, `s9`, `s13`, `b2`) as ui cases when their rules land.
Coroutine terms: a design is coroutines and functions sharing wires, each wire driven by one
`RegOut`, one function or the environment; at every instant the functions are evaluated over the
wires to the one settled network, which every resumption of that instant reads and no resumption
changes; a `structural` module is no coroutine but a name for such a design, its body nets and
instantiations only; a behavioral body instantiates nothing.

### D-19 — Data: `Logic`, `Bits`, X, widths                         (2026-11-05)
Clause: SPEC §I.14 (new, replacing the TBD heading: I.14.1–I.14.8); `(D-19)` notes at every pointer
into I.14 — I.2.2, I.2.3 ×4, I.2.4 ×2, I.2.6, I.2.8, I.5 (table), I.6.6, I.6.10, I.7.1, I.7.4 ×2, I.7.7,
I.9.2, I.9.5, I.9.7 ×2, I.10.1 ×4, I.10.3 ×2, I.10.8, I.11.2, I.11.7 ×2, I.12.7, I.13.8, II.1.1 ×3,
II.1.2 ×4, II.1.5, II.2.2 ×2, II.2.4, II.3.3 ×4, II.3.6 ×2, II.3.7 (50 lines). Nothing else
changed. Part I has no TBD left.
Question: What types may a port, register, wire, memory word and expression carry, and with which
widths and SV types; what is a value and what does each operator do with X, normatively; how does the
2-state realization show X and what does the agreement relation assume about it; may a body write X;
what do the operators, casts and every `Bits` method mean and which does the transpiler realize; what
is a constant; what width has each expression, what is a `Bits<1>`, what is a generic width
constraint; and what are a memory's address width, out-of-range address, port index and X words?
Docs say: SPEC I.2.3 (`c a constant of I.14`, `e pure (→ I.14)`, `p a pattern of I.14`), I.2.4 (`/`
NOT YET; the `Bits` methods row), I.2.6 (types → I.14; R16 an admissibility rule), I.2.8 (`/` → I.14
or II.1), I.6.6/I.11.2(e) (the zero of the payload type), I.6.10/I.11.2/I.11.7 (a write of X; the
width-1 emission defect), I.7.4(b) (`Bits::x()` unstated, the `RESET` rows), I.9.5/I.9.7 (what a
read of an X register computes; the simulator's X unrecordable), I.10.1/I.10.3/I.10.8 (address width,
out-of-range address, `k ≥ R`, X-initialized memory), I.12.7/I.13.8 (X across a crossing, on a net,
through the settle), II.1.2 R11/R15/R16, II.2.2 (the two constant over-approximations), II.3.3 rows
(d), (e), (g). `review/INVENTORY_CODE.md` (6).1–32 (the X rules as the code has them), (7).2, (7).8,
unstated 3–7 and 34. `review/MEASUREMENTS.md` §D (the interpreter: `RefBits` pinned; `all_ones`
without a width a parity item; inclusive `for`). D-9 A2-i (`Bits::x()` unstated), D-12 (R15 refused in
both by decision; R16 rule parity), D-13 (rows (d), (e), (g)), D-14 (the wire's zero), D-16 (run-time
failure of `data()`; one owner per port), D-17 (the crossing), D-18 (the settle a function of the
sources). Contradiction: II.3.3's cross-reference calls `/` "refused in both front-ends by D-12" while
R15's clause says "until I.14 gives one" — resolved by 3A below.
Code does (commit 0138a87): `copper-core/src/types.rs` — `Logic {Zero, One, X}`, no Z (:14-19);
`as_bool` panics on X (:28-35); per-bit `! & | ^` (:57-115, :626-672); `Bits<N>` = `[Logic; N]`
(:145-147); constructors `zero`/`all_ones`/`x`/`from_uN`/`from_usize`/`from_lit` (:155-259, the last
with a post-mono width assert :247-251); `as_*` views post-mono width asserts and a panic on X
(:261-301); `get`/`set` (:304-314); shifts, X moving, ≥ N zero (:332-348); reductions (:367-383); asr
(:388-395); `*_logic` comparisons X on any X operand bit (:400-426); width conversions with run-time
asserts (:431-488); `mux` (:494-506); `+ - * / %` and `-x` all-X on any X, `/`/`%` by zero all-X,
wrapping at N, N ≤ 128 (:676-746); derived `PartialEq` (:144); `From` both ways for `Bits<1>`/`Logic`
(:804-810). `copper-analysis` — `check_admissible`'s type set incl. `i8…isize` (`lib.rs:773-799`,
`:824-870`); `is_constant_expr` syntactic (`cfg.rs:3519-3550`). `copper-codegen/src/chir_lower.rs` —
`resolve_type`, `usize`/`isize` → 32 bits (:117-146); `classify_value_ctor` with a decimal-only
`trailing_const_param` (:654-707); `lower_register_init`/`fold_const_init`, no `all_ones` (:1158-1300);
`lower_index`'s constant select a one-bit `Slice` (:4640-4690); `eval_const_usize` literals only
(:4688-4699); `expr_width`/`width_of_chir_expr`/`resize_to_target`, the e5d103b element width and
port-write cast (:4390-4560); `lower_binop` with no `/` (:4802-4827); `lower_method_call`'s passthroughs
`as_*` (:4885-4890), asr (:4846-4854), `wrapping_*`, the checked/saturating four refused by name
(:4919), the rest "not supported" (:4934). `emit.rs:753-760` (`range_str`: width 1 a scalar).
`vlir_lower.rs:782-830` (cause Q's address cast). `copper-core/src/memory.rs:116-138` (out-of-range
panic), `:195-201` (no port-index check). `copper-sim/src/verification.rs:330-340` (`logic_vec_to_int`:
X compared as 1, a `u64`), `:497-560` (power-on modes). `copper-ref/src/bits.rs` (RefBits, width
1..=128, copied from `types.rs`), `value.rs`, `types.rs` (integer ports refused), `interp.rs:1240-1300`
(the constant constructors; `all_ones` needs a width).
Measurements (every verdict predicted first; `scratchpad/i14probes/predictions.md` then
`measurements.md` of session spec-packet-i14; probes in `src/probes/{xprop,widths}.rs`, runners in
`src/bin/`, emitted SV in `sv/`, none in the tree; the macro admits every probe — the crate builds):
  (i) Simulator vs interpreter, 20 % X per input bit, 48 cycles: `xp_bitwise`, `xp_not`, `xp_arith`,
      `xp_rem`, `xp_eq`, `xp_methods` (reductions, `eq_logic`, `lt_logic`, `mux`, `concat`,
      `sign_extend`, `Logic` ops), `xp_cond_eq`, `xp_reg`, `xw_port`, `w1_index`, `k_hex_body`,
      `k_hex_init`, `k_underscore`, `k_ones_width`, `k_ones_init`: predicted AGREE → AGREE (up to 258
      X-valued observations per probe). `xp_shift`, `c_narrow`, `c_signed`, `usize_width`, `xp_cond`:
      predicted both fail at the first X view → sim panics "Cannot convert X to integer|bool", the
      interpreter errs at the same cycle ✓; at 0 % X all five AGREE. Misses: `xp_div` (predicted
      AGREE; the interpreter refuses `/` by R15), `int_wrapping`/`int_overflow` (the interpreter
      refuses `u8` ports; the simulator panics "attempt to add with overflow" on `int_overflow` ✓).
      `w1_from` (interpreter refuses `Logic::from`) ✓, `k_ones_nowidth` (interpreter refuses) ✓,
      `mem_oob` (both fail) ✓.
  (i-b) `Bits<8>`/`Logic` vs IEEE 1800 4-state (Icarus Verilog 12.0), 400 X-sprinkled rows per
      operator (X rate 0/10/25/50 %): predicted EQUAL → EQUAL 400/400 for `& | ^ ~ + - * / %`, unary
      `-`, `<< >>` by 0..10, asr vs `>>>`, derived `==` vs `===`, `lt/le/gt/ge_logic` vs `< <= > >=`,
      `and/or/xor/nand` reductions, `mux` vs `?:`, `Logic & | ^ !`, sign extension, concatenation,
      `if (l == One)` vs `if (l)`. `eq_logic`/`ne_logic` vs `==`/`!=`: predicted Copper-more-X →
      209/400 more X, 0 less, 0 conflict ✓. Derived `==` vs SV `==`: predicted less-X "on every X row"
      → 70/400 (a row with a known differing bit is 0 on both) — partly missed.
  (i-c) SV leg, X-free inputs, 64 cycles, zero-fill then `COPPER_VERILATOR_X_INITIAL=1`: PASS in both
      modes as predicted — `xp_bitwise`, `xp_not` (`!` → `~`), `xp_arith`, `xp_eq`, `xp_shift`
      (amounts 0..15 on 8 bits), `xp_cond`, `xp_cond_eq`, `c_signed`, `int_wrapping`. `xp_rem`: FAIL
      exactly the 9/64 zero-divisor cycles ✓ ("expected 255 got 0": X read as 1, Verilator `% 0` = 0).
      `xp_reg`: FAIL 9 checks (zero-fill: "q_r expected 255 got 0"; random: "got 185") ✓. Refused as
      predicted: `xp_div` ("binary operator '/' not supported"), `xp_methods` (`and_reduce`), `xw_port`,
      `w1_from`, `k_hex_body`, `k_underscore`, `k_ones_nowidth`, `k_ones_width` ("non-hardware function
      calls cannot appear in hardware expressions" — the messages were mispredicted), `k_hex_init`,
      `k_ones_init` ("cannot infer bit width"). `int_overflow`: transpiles, the simulator panics ✓.
  (ii) The `RefBits` pin's coverage (`copper-ref/tests/bits_property.rs`): covers `! & | ^ + - * / %`,
      unary `-`, shifts (n ≤ N+1), asr, `with_lsb/msb`, `get`/`set`, the six reductions, derived `==`,
      the six `*_logic`, `is_valid`, `as_u128`, `mux`, the five width conversions, `concat`,
      `replicate`, `Logic & | ^`; not covered: the constructors, `as_u8…as_u64`/`as_usize`, shifts
      > N+1, `Logic` `!`/`as_bool`, `From`, `as_array`, other widths and N > 128, the integer types,
      `bool`, arrays (→ III.1).
  (iii) Width 1: `w1_index` (`q.write(v[0])`, `v: Bits<1>`) and `w1_comb` (I.11.7's combinational
      draft): predicted transpiles + Verilator refuses → ✓ both ("Illegal bit or array select; type does
      not have a bit range … 'logic'"); `w1_from` refused by the transpiler ✓.
  (iv) Hex and `all_ones`: macro admits all ✓; sim = interpreter ✓ (bar the unsized `all_ones`);
      transpiler refuses all six spellings ✓ — `all_ones` in every spelling, not only without a width.
  (v) Shifts ≥ N agree sim ≡ SV ✓. Casts: `c_narrow` predicted PASS → BUILD ERR (-Wall UNUSEDSIGNAL:
      `let v = a.read().as_u32()` declared 8 bits) — missed; `c_narrow2` WIDTHTRUNC ✓; `usize_width`
      (`as_usize`, `<< 30 >> 30`) predicted FAIL by `usize` = 32 → FAIL 64/64 by a different mechanism
      (the passthrough keeps 8 bits); `usize_annot` (`let v: usize`, `<< 40 >> 40`) as emitted a -Wall
      BUILD ERR, with one cast patched FAIL 64/64 ✓ (`usize` 32 in the SV).
  (vi) `mem_oob` (depth 10, 4-bit address): the simulator panics at the first address ≥ 10 ✓; the SV
      has 4-bit address nets ✓; vs a hand model (out-of-range read 0, write dropped, 19 such cycles):
      zero-fill PASS ✓, randomized FAIL 19/19 ✓.
  (vii) Corpus (374 `#[hardware]` fns in `examples/`, `src/`, `tests/`: 328 sequential, 6
      synchronizer, 39 combinational, 1 structural): signed port payloads 0 (pred. 0); signed in
      bodies 6 sites in 3 modules (pred. 5–15); X written to a port 0 (pred. 0–3); `/` 0 (pred. 0);
      `%` 3 (pred. ≤ 10); non-decimal or underscore `from_lit` 0 of 264 (pred. 0); `all_ones` without a
      width 0 (pred. 2–5 — missed: `all_ones` has 0 uses); inclusive ranges 0; ports wider than 64
      bits 0 (widest `Bits` anywhere 64). Port payloads `Bits<lit>` 651, `Logic` 237, `Bits<param>` 40,
      `u8` 5, arrays 2, `bool` 0; `Bits::x()` 5 (initializers); `as_usize` 51 call sites; every row-(e)
      method 0; `const { assert! }` in 7 files; `PARAMS` 7 rows.
Anchor: IEEE 1800 4-state operator semantics as executed by Icarus Verilog 12.0 ((i-b): equal on every
row of every operator but `eq_logic`/`ne_logic`, which 2a-B makes IEEE-exact). Sim ≡ interpreter on
every operator class ((i)). Sim ≡ SV only: modular arithmetic, shifts ≥ N, `!` → `~`, the
out-of-range address under zero-fill. Decided, not anchored: the type set and signed types, `usize`'s
64 bits (a consequence of `SPEC_OVERVIEW.md` §1), the constant grammar, the width-constraint rule, the
out-of-range address, the port index.
Prior work: Arch — `Bit`, `UInt<N>`, `SInt<N>`, `Bool` [arch §3.1 Table 1], 2-state "no per-bit X/Z
state" [arch §7.2], "no X from uninitialized state" [arch §6.1, Table 6]; Clash — Bit, Bool,
Signed/Unsigned with wrap-around overflow, Vector, Index [clash §II-C]; Spade — widening arithmetic with
a mandatory `trunc`, enum payload "undefined otherwise" [spade §III-B, Fig. 2]; Chisel — width inference
to a fixpoint, growing [chisel §10.4], a `Reg` without `resetVal` X in the Verilog [chisel §2.6, §10.2];
MyHDL — `bool`/`intbv`/`int`/lists as the convertible types [myhdl §10], HDL registers X until reset vs
the simulator's initial value [myhdl §6.3.1, §11.2]; Verismith — power-on initialisation so that no X
arises [verismith §3.1 p.279], division by zero and out-of-range selects guarded [§3.3 p.280],
signedness the richest bug source [Fig. 1 p.277]. Not covered by the notes: X propagation as such
(`cummings.md` is silent), Clash `BitVector`/`undefined`, Chisel `UInt`/`SInt`/`DontCare`, MyHDL
`intbv` bounds, Spade's integer types, an out-of-range `Mem` read.
Options:
  1. I.14.1 signed types (corpus: 0 ports, 3 bodies):
     A. admitted wherever an unsigned integer is, ports included, Rust's meaning — changes: the
        interpreter's integer ports (Phase C); cost 0.
     B. body-local only, a signed port payload refused (R16 narrowed) — cost 0.
     C. refused everywhere, `arithmetic_shift_right` the one signed operation — cost 3 modules.
  2a. I.14.2 `eq_logic`/`ne_logic` (209/400 rows more X than IEEE; corpus 0; transpiler refuses):
     A. the simulator's rule as is, a recorded sound deviation — changes: none.
     B. IEEE-exact — changes: `types.rs:400-405`, `RefBits::eq_logic`, the pin re-run; cost 0.
  2b. I.14.2 a write of `Bits::x()`/`Logic::X` to a `RegOut` or wire (corpus 0):
     A. admitted, the port is X, outside the agreement relation by the X precondition; the
        transpiler's refusal a II.3 row (emit the X literal) — changes: Phase C lowering; cost 0.
     B. refused as design (II.2) in both front-ends — changes: a rule.
  3. I.14.3 `/` on `Bits` (corpus 0; IEEE-equal 400/400):
     A. truncating unsigned division, X on X or a zero divisor; refused in both front-ends until the
        transpiler lowers it, R15 a dated II.3 row — changes: R15's principle; Phase C one arm; cost 0.
     B. OUTSIDE (no combinational divider), R15 an S0 refusal, the simulator's `Div` removed.
  4. I.14.4 the constant grammar (three evaluators disagree; corpus cost 0 either way):
     A. Rust's constants over literals in any base, the `Bits` constructors incl. `all_ones`, enum
        variants, `const` items, const generics, associated consts, `const fn` calls and operators,
        casts, arrays — one evaluator in `copper-analysis` for both front-ends and the interpreter;
        rows (d), (g) close.
     B. today's transpiler grammar, the rest an S3 refusal in both front-ends.
  5. I.14.5 a generic width constraint (7 files; rustc refuses a violating instance, the SV states
     nothing):
     A. a shape rule at instantiation; the emitted parametric module owes an elaboration-time check;
        a III.2 precondition until then — changes: Phase C emission.
     B. a III.2 precondition only.
  6a. I.14.6 an out-of-range memory address (sim panics; SV 0 / random; IEEE X):
     A. OUTSIDE, a run-time failure, as `data()` undelivered (D-16) — changes: none.
     B. IEEE's meaning (read X, write dropped) — changes: the simulator.
  6b. I.14.6 a port index `k ≥ R`/`≥ W`:
     A. a shape rule, refused at compile time in both front-ends — changes: Phase C.
     B. a run-time failure like 6a.
  No fork (stated and written): the type table with widths, zeros and SV types (I.14.1); `usize`/
  `isize` 64 bits, the transpiler's 32 a defect (I.14.1(b)); `Bits<N>` 1 ≤ N ≤ 128 (I.14.1(c)); per-bit
  X, no Z, the operator table (I.14.2(a)–(b)); X in a condition or a view a run-time failure
  (I.14.2(c)); the X precondition handed to III.2 (I.14.2(f)); X through a design (I.14.2(g)); the
  operator meanings, Rust integer overflow a run-time failure, `checked_*` OUTSIDE, `saturating_*` IN,
  `is_valid` OUTSIDE, the method table (I.14.3); patterns (I.14.3(e)); the port-write cast a
  realization (I.14.3(f)); expression widths Rust's, no growth (I.14.5(a)); the passthrough and width-1
  defects (I.14.5(b)–(c)); the address width and X words (I.14.6(a), (d)).
Recommendation: 1A, 2a-B, 2b-A, 3A, 4A, 5A, 6a-A, 6b-A — the meaning is the Rust coroutine's
wherever rustc already gives one (signed types, constants, widths, `/`), the X algebra is the one
measured equal to IEEE once `eq_logic` is made exact, a value no realization can hold is a value of the
kernel with a stated precondition rather than a refusal, a failure only reachable at run time ends the
trace, and a fact known at the instance is checked at the instance.
Decision: **1A, 2a-B, 2b-A, 3A, 4A, 5A, 6a-A, 6b-A.** User, 2026-11-05, the recommended options as
offered.
Changes: none now. Phase C obligations (I.14.7): (1) `eq_logic`/`ne_logic` IEEE-exact in `types.rs`
and `RefBits`, the pin re-run, an IEEE table pin; (2) one constant evaluator over I.14.4's grammar in
`copper-analysis` for R11, the transpiler's fold and `eval_const_usize`, and the interpreter — closes
(d), (g), `all_ones`, underscores; (3) the `as_*` passthroughs as zero-extensions to the integer's
width and `usize`/`isize` as 64 bits; (4) the width-1 select and `From`; (5) `/` lowered, the macro's
R15 refusal until then; (6) `Bits::x()`/`Logic::X` in an expression emitted; (7) the row-(e) lowerings
incl. `eq_logic`, `ne_logic`, `as_array_mut`, `from_array`, `from_slice`, `saturating_*`, and
`checked_*`/`is_valid` refused as OUTSIDE in both front-ends; (8) width constraints emitted as
elaboration-time checks; (9) the memory port-index rule in both front-ends; (10) the interpreter's
integer and `bool` ports, `From`, `/`, the unsized `all_ones`. Each with its D-19 probe as the fixture.
Obligations: III.1 — the pin's uncovered operators (measurement (ii)), the IEEE comparison as a
recorded standing check; III.2 — the X precondition and its form (the harness reads an expected X as 1
today), X-free inputs, ≤ 64-bit harness ports, the generic-width precondition until (8), run-time
failures (an out-of-range address, X in a condition or view, integer overflow) outside the relation;
II.3 — rows for `/` (from R15), `Bits::x()` in an expression, the passthrough/`usize` widths
(ADMITTED-AND-DIVERGENT: `usize_width`, `usize_annot`), the width-1 select (`w1_index`, `w1_comb`), row
(e) widened by seven names, row (g) by underscores (the next II.3 edit); II.1 — rows for the memory port
index, `checked_*`/`is_valid`, R16's N bound, R15 restated; Phase C — the inclusive counted `for`
(interpreter parity; corpus 0); `SPEC_OVERVIEW.md` §6's II.3 count (the coordinator's edit).
Coroutine terms: a value is what the coroutine computes, typed by rustc — every integer, `usize`
included, at its host width, every `Bits<N>` at N bits modulo 2^N; X is a third bit value the coroutine
can hold and the 2-state realization cannot, so it is a kernel value whose operator table is IEEE's and
whose observation the agreement relation excludes; whatever needs a definite value from an X, or an
address the memory lacks, ends the trace; and whatever is known at the instance — a constant, a width,
a constraint, a port index — is decided at the instance.

### D-20 — Propositions and their evidence; the agreement relation (Round 5)   (2026-11-05)
Clause: SPEC §III.1 (new, replacing the TBD heading: III.1.1–III.1.6) and §III.2 (new: III.2.0–
III.2.13); `(D-20)` notes at every pointer into III.1 or III.2 in Parts I–II (75 lines; `grep -n 'D-20' SPEC.md`), I.0.4's "many-to-one" and I.0.7's "per tick occurrence" corrected by notes. Nothing else changed. Only §III.3 is TBD.
Question: Which claims do Parts I–II make about an implementation, and what evidence does each rest
on; how is the projection stated; which propositions are proposed for mechanization; and what exactly
does "sim ≡ SV ≡ reference" mean for one module instance under one stimulus — which observations,
which bit equality, which preconditions, how a run-time failure, an X, several domains and a hierarchy
are read, and what the anchors add?
Docs say: SPEC I.0.4 (H(M), "many-to-one", "unmeasured"), I.0.5 (the register proposition, the poison
run), I.0.8, I.7.7, I.8.7, I.9.7 (the III.1 list), I.1.5–I.1.6 (the environment, outside-the-language
drives), I.3.6 (two iterations, [anvil Lemma C.19]), I.6.6 and I.11.2(e) (`wire(zero)`), I.7.4(b) and
I.9.5 (the `RESET` rows), I.12.1–I.12.2, I.12.7 (interleavings, the never-ticked startup), I.13.2(c)
(order independence), I.13.4 (hand wiring ≡ hierarchy), I.14.2(f), I.14.5(d), I.14.6(b), I.14.8 (the X
precondition, ≤ 64-bit ports, generic widths, run-time failures), II.1.3 (parity), II.2.2 (directions),
II.3.3/II.3.7 (defect rows outside the relation). `SPEC_OVERVIEW.md` §7 (consistency vs correctness).
`FORMAL_SEMANTICS.md` P6/G6 and D-e (the proof bar). `copper-ref/README.md` (the disagreement table).
`review/MEASUREMENTS.md` §C (anchors and debt) and §D (the interpreter's four measurements, D.6's
list for the coordinator). The obligations of D-1, D-3, D-6, D-7, D-9, D-10, D-11, D-12, D-13, D-14,
D-16, D-17, D-18, D-19 naming III.1 or III.2. Contradiction: I.0.4 and I.0.7 say `pc` is allocated per
tick occurrence and is many-to-one onto sites; §D.3 measured many-to-many — resolved by III.1.3.
Code does (commit 0138a87): `copper-sim/src/verification.rs:250-300` (the testbench drives, toggles,
then compares: from `obs_1`), `:330-340` (`logic_vec_to_int`: X written as 1, a `u64`), `:497-560`
(power-on modes); `tests/common/mod.rs:577-600` (`stim`: the reset asserted on cycle 0), `:853`
(`record_differential`, after `tick_clock`), `:1233` (an interpreter stop is a leg failure), `:1245`
(the reference leg compares `Logic` vectors exactly, X = X); `build.rs:52-139` (`SKIP`, 7), `:152`
(`PARAMS`, 7), `:194-205` (`REFERENCE`, 3), `:206-230` (`RESET`, 3), `:239-270` (`REF_SKIP`, 5 named;
the four memory sentences still name I.10 as unwritten), `:702-708` (a second clock is a `SKIP`), the
anchoring and reference-leg ledgers; `copper-ref/src/module.rs:211-215` (one clock); `tools/regression.sh`
G-D, G-E, G-F.
Measurements (every verdict predicted first; `scratchpad/predictions.md` of session spec-round-5):
  (i) `tools/regression.sh --no-examples`: build and CLI ok; `cargo test --workspace` stopped on
      "No space left on device (os error 28)" (523 MiB free; `.claude/worktrees` 75 GiB, `target/` 33 GiB;
      nothing deleted) — the test-phase counts of G-C/G-D/G-F were NOT measured. From the build-time
      ledger of the same tree: sweep 166 modules (predicted ~166 ✓); reference leg 46 run, 113 excused —
      4 `Memory`, 25 combinational, 1 measured disagreement, 83 plain `Out` (predicted 46 / ~113 ✓);
      anchoring 5 modules, 161 transpiler-only (predicted 4 ✗ — `handshake` has three copies,
      `examples/sequential/handshake.rs`, `tests/fixtures/handshake_dut.rs`, `tests/fixtures/wait_loop_dut.rs`).
      Re-measured 2026-11-06 after the worktree build folders were deleted (73 GiB free): bare
      `tools/regression.sh` at 0138a87 → `REGRESSION OK` (7344 s); G-C 115 test files, G-D 159 cases
      + 7 ignored, G-E 5 anchored / 161 transpiler-only, G-F 46 run / 120 skipped — every ledger count ✓.
  (ii) `REFERENCE` rows: predicted 6–8 ✗, refined after reading `build.rs` 3 ✓; anchored in the sweep 5.
      Golden `.sv` files in the tree: `examples/` 20, `tests/fixtures/reference_sv/` 7,
      `tests/fixtures/timing_probe_sv/` 6; MEASUREMENTS §C's independent set 23 (7 BaseJump, 4
      sequential, 5 combinational, 1 memory, 2 CDC, 3 `REFERENCE`, `mac_fsm` register set) — predicted
      "~12–15 modules", not comparable as stated (files were counted, not modules).
  (iii) `SKIP` 7 ✓, `RESET` 3 ✓, `REF_SKIP` 5 named ✓; sorted by precondition in III.2.10.
  (iv) The X form, read off the code with no probe needed: the SV pair writes an expected X as 1, the
      reference pair compares X exactly ✓ (both predicted).
  Recovered, not re-measured: the interpreter's §D.3 (`pc` 34 extracted modules, states/sites 0.67–4.00),
  §D.4 (poison run 47 clean, fold probe flagged), §D.5 (layout ⊇ registers + iterators).
Anchor: none of its own; III.2.11 lists the anchors. Kôika's compiler theorem and Kami's refinement are
precedents for the statement's form, not anchors.
Prior work: Kôika — the one-rule-at-a-time theorem of a log-based cycle semantics [koika §4, Theorem 1];
a Coq-verified compiler to circuits, `∀R, s, r. δs(R)[r] = δ⟨s⟩r(R)`, a per-cycle equality of next-state
functions, with "a thin unverified pretty-printing layer" to Verilog [koika §6]. Kami — modules as
labeled transition systems [kami §3, Fig. 6], refinement as trace inclusion `m ⊑ m'` [kami §3.3],
modular lemmas [kami §5], an unverified Bluespec compiler trusted for hardware [kami §4, p. 24:3]. Anvil's
two-iteration lemma [anvil Lemma C.19]; Verismith's power-on initialisation so that no X arises
[verismith §3.1] (the X precondition's practice). Notes `prior_art/koika.md`, `prior_art/kami.md` added
by this entry (option 5A).
Options:
  1. P1, the projection (I.0.4; `pc` many-to-many):
     A. a bisimulation of the emitted machine with H(M) under a many-to-many relation `R` between `pc`
        values and continuations; trace equality follows — owes a static test that builds `R`.
     B. trace equality only, no structural claim — owes nothing new.
  2. III.2(b), an X observation:
     A. X inside sim ≡ ref (exact), outside every SV pair: a recorded X against the SV puts the case
        outside the relation and the harness says so — changes: the harness (Phase C).
     B. mask X bits against the SV with a printed count — restores `mod8`; its `SKIP` row recorded this
        as rejected (silent passes).
     C. today's form (X as 1) stated as the precondition — changes: none.
  3. III.2(d), a run-time failure:
     A. the trace ends; compare up to the failing resumption; sim and ref must fail at the same one;
        the SV is outside from there and the case is reported truncated — changes: the harness.
     B. any failure fails the case (today) — changes: none.
  4. III.1 P9, two iterations suffice:
     A. retired as vacuous — its consumers were the trailing rules (D-8); the surviving analyses are CFG
        fixpoints and unroll nothing (grep of `copper-analysis`, `copper-codegen`); I.3.6's lemma stays.
     B. kept as THEOREM, no pin.  C. kept, G6's pin owed to Phase C.
  5. Mechanization (the 2026-11-04 direction):
     A. P1 and P8 proposed, dated, not started; Kôika and Kami cited, notes added to `prior_art/`.
     B. the same without the notes.  C. none proposed (FORMAL_SEMANTICS D-e's bar).
  6. III.2(f), several domains:
     A. two dedicated pins (D-17 probes (i) and (vi)); the sweep stays one-clock.
     B. the sweep generates interleavings for a k-clock design.
  No fork (stated and written): the four evidence kinds, consistency never correctness; the 24 rows and
  their kinds; the three pairwise relations; `obs_0` compared (D-1 A1, Phase C); "from reset" as the
  observations after the edge at which the reset asserted on cycle 0 takes effect, the row's sentence
  the unchecked assertion; X-free inputs, ≤ 64-bit ports, generic widths, `wire(zero)`; the power-on
  statement a realization obligation; hand wiring ≡ hierarchy as a design; the disagreement table; the
  stimulus policy as coverage; the excusing rows by precondition; the anchors and the owed references.
Recommendation: 1A, 2A, 3A, 4A, 5A, 6A — state the projection in the form the measurement found and a
proof would need, keep X a kernel value the 2-state leg cannot show rather than a silent pass or a
misleading failure, end a trace where the coroutine ends it, retire what nothing consumes, propose for
mechanization only the two claims about the emitted text, and pin the multi-domain cases the
measurements already found.
Decision: **1A, 2A, 3A, 4A, 5A, 6A.** User, 2026-11-05, the recommended options as offered.
Changes: none to code now. Spec: §III.1, §III.2 written; the notes; `prior_art/koika.md`,
`prior_art/kami.md` and two rows in `prior_art/README.md`.
Obligations — the Phase C list as Part III sees it, in order (the input of III.3; each item's fixture
lands with it):
  (1) The harness, first, because it is the instrument: observe `obs_0` on all three legs (III.2.1); the
      "left the relation" verdict for an X against the SV (III.2.2); the truncated verdict at a run-time
      failure, sim and ref required to fail at one resumption (III.2.4); the per-pair reading of
      III.2.8 in the failure report; the four stale `REF_SKIP` memory sentences restated.
  (2) The simulator: one resumption per tick at `t_{N+1}⁻`, `PreEdgeBarrier` removed (D-8, I.0.7) — P12;
      `regout_trailing_single_tick`'s pin flips and its `REF_SKIP` row is deleted; the own-domain
      resumption (D-17, I.12.6 (1)) — P11 — with D-17's probe (i) as a two-clock pin and probe (vi)'s
      interleavings P/Q (III.2.6); the global cycle counter per domain (I.12.6 (8)).
  (3) The macro: no `Out` on a `sequential`/`synchronizer` module; the injection, `classify_reads` and
      the eight alignment rules with their opt-out removed; the 19 shipped and 69 probe rewrites, the 88
      plain-`Out` and 78 opt-out modules (D-8, D-12); the deletions sized in II.3.2; the reference leg's
      plain-`Out` class empties and P2's poison run is re-run over it.
  (4) The transpiler's one port form (`split_output_regs`), the trailing fold and the opening/closing
      tagging removed (I.0.7, I.6.9).
  (5) The II.3.6 fix list: the entry of I.9.2 — (f2), (f3), (f4), P15; the hoisted-tick guard (f1); the
      array-element forwarding (f5); the nested-`continue` entry copy (a); the arms-only fall-through
      clone (b); the array write port (c); the constant evaluator with I.14.7 (2) — (d), (g); the method
      lowerings (e); R21 — (f6).
  (6) Register inference: the head clause deleted, the nested loop's CFG descended, two reconciliation
      fixtures (I.7.6) — P3; P13 measured on the corpus; P14 probed.
  (7) The static test of P1's relation `R` per module (beside `pc_states_vs_tick_sites`), the initial
      state included (P15).
  (8) I.14.7 (1)–(10): `eq_logic`/`ne_logic` IEEE-exact with the IEEE table as a standing pin (P6), the
      `as_*`/`usize` widths, the width-1 select, `/`, the X literal, the width-constraint check, the
      memory port-index rule, the interpreter's integer and `bool` ports; `RefBits`' uncovered operators
      pinned (P7).
  (9) The interpreter's legs: the memory model (D-16; the first interpreter item), the combinational
      module, the design over shared values (I.13.2) — the excused classes of III.2.10 shrink to the
      rows with sentences.
  (10) The rules: the II.1.3 parity fixes and the audit's extension (P10); the CDC two-flop rule, the unit
      domain removed, R10 (I.12.6 (2)–(5)); the static combinational-cycle refusal at spawn (I.11.6,
      I.13.7 (4)) — P5's open item; the structural shape rules and `net::<T, D>()` (D-18); the II.2.2
      directions narrowed or pinned (P22).
  (11) III.3, the ledger: one line per clause and per II.3.3 row, each closed when its fixture passes
      under the sweep's policy.
  Outside Phase C: the eight references of III.2.11 (the user writes them); the mechanization of P1 and
  P8 (III.1.4, proposed 2026-11-05, not started); the G-C/G-D/G-F counts — read into III.2.9
  on 2026-11-06 (done).
Coroutine terms: the coroutine is the one object with a trace; the simulator, the interpreter and the
emitted machine are three ways of computing it, and the relation says when they compute the same
thing — X included where both sides can hold it, up to where the coroutine stops, per domain under a
stated interleaving. That they compute the right thing is what the anchors, and only they, decide.

### D-21 — Constants: rustc judges R11, one grammar with two evaluators, R11 for registers only   (2026-11-07)
Clause: SPEC §I.2.3 (`s_pre` gains the first resumption's wire, and an OPEN note), §I.7.4(c), the
I.7.6 obligation list, §I.14.4 (evaluator (1), the direction), §I.14.7 (2), II.1's R11 row, II.2.2's
R11 row, II.3.2's sort row 5, II.3.3 row (d) (closed). Nothing else normative changed.
Question: (A) What decides that a register's initializer is a constant — a syntactic test the macro
can run on one `ItemFn`, or rustc? (B) Should the interpreter and the front-ends share one constant
evaluator? (C) Is R11 a rule about registers, or about every pre-loop `let`?
Docs say: I.14.4 (D-19 4A: Rust's constants; "one evaluator … used by R11, the transpiler's fold and
the interpreter"); I.2.3 (`s_pre ::= let v = c`); I.7.4(c) / D-12 R11 ("widens … to every pre-loop
`let`"); `review/MEASUREMENTS.md` §K.12 item 6 and §M.10 (the packet).
Code does (c1735ab): `copper-analysis/src/init.rs` (R11 = `is_constant_expr` on the register set,
both front-ends); `chir_lower.rs` `lower_register_init` / `fold_const_init` over `ConstEnv`;
`copper-ref` `Exec::eval_const` (typed `Value`s); `copper-core/src/types.rs` constructors not `const`.
Anchor: none — decided. The sweep compares the simulator (rustc's value) against both evaluators.
Prior work: no surveyed HDL lets the host compiler's constant evaluator decide a register's reset value;
Rust's inline `const { }` (stable 1.79) is the mechanism.
Measurements (§M.10, before): 195 clocked modules, 243 pre-loop `let` initializers, all constant-shaped,
0 non-constant; 93 const items in 35 hardware files; 8 `const fn`s; R11 under-admits a single-segment
`const fn` call (corpus 0), over-admits the non-`const` `T::bubble()` (7 initializers, 5 fn definitions);
step 6's throwaway wire fixture 6/6 PASS on all three legs.
Options: (A) A1 keep the syntactic test; A2 admit a single-segment call with constant arguments; A3 the
macro emits each register initializer in a `const { }` block (rustc the judge); A4 give the macro the
file's items. (B) B1 the interpreter's typed evaluator shared; B2 one grammar, two evaluators, pinned
against each other; B3 status quo. (C) C1 widen R11 to every pre-loop `let`; C2 widen I.2.3 — a
non-register pre-loop `let` is a wire of the first resumption; C3 status quo.
Recommendation: A3 (A2 the fallback), B2, C2 (§M.10).
Decision: **A3, B2, C2** — the user, as recommended (relayed by the coordinator, 2026-11-07).
Changes (branch `claude/phase-c-consts-867296`, `review/MEASUREMENTS.md` §N): every `Bits`/`Logic`
constructor a `const fn` (A3's prototype held on rustc 1.92: no fallback to A2); the 5 `bubble()`
definitions `const fn`; the macro wraps each pre-loop register initializer in `const { }`; R11's shared
syntactic half refuses only a run-time read, with its own message; the transpiler adds a call half
(every callee a core constructor or a `const fn` of the file) before its fold — the `T::bubble()`
over-admission closed, `f(3)` admitted by both. `copper_ref::ConstScope` and `ConstEnv::eval_expr` (entry
points, no shared code) and `tests/const_evaluators_agree.rs`, guard G-J. Fixtures
`tests/fixtures/const_init_dut.rs` (3) and `tests/fixtures/preloop_wire_dut.rs` (2, step 6's shape
made permanent); ui `register_init_reads_input`, `register_init_non_const_ctor`,
`register_init_rust_constants_ok`; `copper-codegen/tests/register_init_parity.rs`.
Measured basis: (A) emitted SV byte-identical corpus-wide (sv-baseline against c1735ab: 267 modules,
no changes — the pipelined CPU included); the new spellings agree on all three legs; the rustc error a
user sees for a non-`const` callee is E0015 pointing at the call, and for a run-time read R11's own
message. (B) the two evaluators agree on every corpus constant compared — 68 const items, 221 stated
register initializers (vs the transpiler's SHIR value) + 42 vs `ConstEnv`, 91 counted-`for` bounds, 68
indices — 0 DISAGREE; 7 initializers one-evaluator-only (the interpreter's `Self { .. }` gap, the
`REF_SKIP` rows' class), pinned; an off-by-one injected into `ConstEnv`'s `+` gives 16 DISAGREE. (C) the
wire shape agrees on all three legs (both fixtures PASS).
Found, not decided (stopped): **the held pre-loop read** — `let x = a.read(); loop { q.write(x); tick }`.
I.7.1 (i) calls `x` a constant (never assigned in the loop), the analysis agrees (not a register), both
front-ends admit it, the transpiler emits a wire of each resumption, and the SV diverges: 6/6 sequences
`sim=ref≠SV` (corpus 0). C2's wire is I.7.2(c)'s (reassigned in the loop); this is I.7.2(b)'s shape with
a non-constant initializer, which neither C2 nor R11 then covered; options in §N.4, both H1 and H2
prototyped and measured (§N.4.1). **DECIDED H1 (user, 2026-11-07): refused**, R11 widened to it in both
front-ends (`check_held_preloop_values`, one message, ui/fail `held_preloop_read`, corpus 0); H2's branch
is not merged. Obligations: the follow-ups of §N.5.
Coroutine terms: a register's power-on value is the value the coroutine's first resumption finds in it,
and rustc is the one tool that computes Rust's constants; the transpiler and the interpreter each
compute it again, and the sweep and G-J say whether all three agree.

### D-22 — R6 the one named parity exception; the unit domain removed by a generic domain   (2026-11-07)
Clause: SPEC §II.1.3 (the "iff" gains its one exception; a D-22 note on the state after Phase C step
10), II.1.2's R6 row (and, as step 10's SPEC items, the Parity cells of R5, R7, R10, R12–R20), II.1.4,
II.2.1 D2 and D3, II.2.2 (R3, D2, four new rows), §I.12.1, I.12.4(c), I.12.6 (the map and obligation
(4)), I.13.4(b), I.11.6, I.13.7, and III.1's P5, P11, P20, P22. Nothing else normative changed.
Question: (1) The macro cannot see a helper `fn`'s body, so it cannot refuse recursion through one,
which the transpiler refuses when it inlines: how is this break of II.1.3 resolved? (2) D-17 A1 retired
the unit domain `()` believing no corpus port used it; 32 combinational modules (100 ports) do, through
the default of `In<T, D = ()>`: how are they written once `()` is gone?
Docs say: II.1.3 ("compiles iff it transpiles"); II.1.2 R6 ("unmeasured"); I.12.1 / I.12.6 (4) (D-17 A1,
"measured 0"); I.12.4(c) (a combinational module lies in the domain of its ports);
`review/MEASUREMENTS.md` §P.8 (the packet), §P.2 (the 32 modules).
Code does (6c17e6e): the macro runs the shared `check_front_end_rules` on the module's `ItemFn`; the
transpiler's inliner refuses a recursive helper ("cannot inline recursive function"); `In<T, D = ()>`,
`Out<T, D = ()>` with no bound, the 32 modules written `In<T, ()>` / `In<T>`.
Anchor: none for (1) — decided; the witness is checked against `u8::count_ones`. For (2) the four BaseJump
combinational anchors (`bsg_adder_one_hot`, `bsg_encode_one_hot`, `bsg_gray_to_binary`,
`bsg_mux_one_hot`) and the five `examples/combinational/sv` goldens, re-run on the generic spelling.
Prior work: (1) not covered by the notes (no surveyed language's notes state how an embedded front-end
treats host-level recursion it cannot see). (2) RHDL's and Arch's domain-tagged signals (I.12.1); a
function generic over a phantom domain parameter is that type's ordinary use.
Options: (1) (a) record R6 as the one named exception to II.1.3 — a shape only the inliner can see; (b)
an attribute on every helper plus a marker trait the body's calls are checked against (mutual recursion
still invisible; every corpus helper annotated); (c) give the macro the file (D-21's A4). (2) (A1) each
module generic over its domain, `D: ClockDomain`; (A2) each in one concrete domain; (B) amend D-17 A1 to
keep `()` as "domain-polymorphic" on combinational modules.
Recommendation: (a), (A1) (§P.8).
Decision: **(a) and (A1)** — the user, as recommended (relayed by the coordinator, 2026-11-07).
Changes (branch `claude/phase-c-domains-f9b023`, `review/MEASUREMENTS.md` §Q): (1) the transpiler's
refusal names its clause (`copper_analysis::front_end::RECURSIVE_HELPER_MESSAGE`); the witness
`tests/recursive_helper_r6.rs` (simulated, refused with R6's message); ui/pass `recursive_helper_r6`
with `front_end_parity_ui.rs`'s `NAMED_PARITY_EXCEPTION` (refused with exactly that message);
`front_end_parity.rs::the_one_named_parity_exception_is_r6` (the tree's helper call graph: the witness
is the only module that reaches a recursive helper). (2) `In`, `Out`, `RegOut` bound `D: ClockDomain`,
no default; the shared `front_end::check_port_domains` refuses a port, clock or memory naming no domain
or `()` in both front-ends with one message (ui/fail `unit_domain_port`, `missing_domain_port`;
copper-core ui `unit_domain_refused`, rustc's E0277 in a testbench); the 32 modules take a leading
`D: ClockDomain` and their testbenches name `MainClk` (`_` in a const-generic turbofish); the sweep
names `SweepDomain` for a module generic over its domain; the transpiler's structural call-site CDC
check binds a child's domain generic per instance (it falsely refused a generic child as a crossing —
found, fixed, pinned by `structural_hierarchy.rs::a_domain_generic_child_takes_the_domain_of_its_nets`).
Measured basis: (1) predicted and measured — the macro admits, the transpiler refuses with R6's
message, the simulator agrees with `count_ones` on all 256 inputs, corpus 0 other modules reach a
recursive helper. (2) predicted 32 modules / 100 ports, measured 32; the emitted SV **byte-identical**
for every module the CLI lists (270 transpiled, each alone and with `--hierarchy`; the three refused
modules refused with the same messages), so no SV parameter for `D`; the four BaseJump anchors and the
five combinational goldens pass; bare regression on `5b22062`: REGRESSION OK — G-G 1622 PASS / 12 OUTSIDE / 0 DISAGREE, reference 1634 / 0, G-F 213, G-I 162, G-J 0, examples 32/32; `sv-baseline diff` vs `6c17e6e`: only the new witness (refused).
Coroutine terms: a combinational module is a function, evaluated wherever it is wired; its domain is
the domain of the wires it reads and drives, so it is a parameter of the module, bound at each
instance. A helper `fn` is a function the coroutine calls; the macro sees the coroutine, the inliner
sees the call graph, and only the inliner can refuse a cycle in it.

### D-23 — Instantiation refused in behavioural bodies by rustc; `continue` of a ticking `for` and `== X` refused in both   (2026-11-08)
Clause: SPEC §I.13.5 (a D-23 note: the realization), I.13.7 (3) (discharged), I.8.5 (the `continue`
note), I.14.3(d) (a new `==`/`!=` row) and (e) (the `Logic::X` pattern, recorded open), II.1.3 (the
re-verification), II.3.3 (row (h)), and III.3.4's lines. Nothing else normative changed.
Question: (1) by what test does each front-end refuse a child instantiated in a `sequential`,
`synchronizer` or `combinational` body, given that the macro sees one `ItemFn` and cannot tell a
`#[hardware]` callee from a helper `fn`? (2) Is a `continue` of a tick-bearing counted `for` realized,
or refused in both as a dated II.3 limitation? (3) Is `v == Logic::X` a value of the kernel, or the
`is_valid()` question — OUTSIDE?
Docs say: (1) I.13.5 (D-18 A2: instantiation and nets are structural-only; `lower_hardware_call`, the
`submodules` fields, `spawn_child`, `spawn_child!` deleted; a ui case from `b2`); D-18 (iv) (`b2`
admitted by both, the child dropped in the simulator). (2) I.8.3, I.8.5 (D-10 A2-i: increment, then
test — IN); II.1.3. (3) I.14.2 (the derived `==` is structural, X equals X), I.14.3(d) (`is_valid()`
OUTSIDE), III.2.2. The packets: `review/MEASUREMENTS.md` §R.6 (1)–(3).
Code does (c0f2a3c): (1) neither front-end refuses; `lower_hardware_call` instantiates; `spawn_child`
stands. (2) the macro admits, the transpiler refuses (`control_extract.rs` ~:1990). (3) the macro
admits, the transpiler refuses (`chir_lower.rs` the `XLit` comparison arm). Corpus 0 for all three.
Anchor: none — decided.
Prior work: (1) as I.13.5 (Prost's call-is-inlining declined; Spade's `inst`, Chisel's instances
admitted where the body is elaboration). (2) Prost's `for` (a counter-and-state form). (3) IEEE 1800's
`===` is the 4-state question a 2-state realization cannot ask — not covered further by the notes.
Options: (1) (a) rustc decides — a denied `#[must_use]` or a type with no use but `__instance`; the
transpiler refuses from its registry; (b) a syntactic port-argument rule (misses a child taking only a
clock, exempts `&clk` by name); (c) a second named exception. (2) (a) realize the lowering; (b) refuse
in both now, dated row, (a) its fix direction; (c) leave the break. (3) (a) OUTSIDE in
`check_outside_methods`; (b) lower to constant false; (c) leave the break.
Recommendation: (1) (a); (2) (b), (a) its dated fix; (3) (a) (§R.6).
Decision: **(1) (a), (2) (b), (3) (a)** — the user, all three as recommended (relayed by the
coordinator, 2026-11-08).
Changes (branch `claude/phase-c-r6-decisions-5895af`, `review/MEASUREMENTS.md` §S): (1) the macro's
`wrap_in_module` puts `#[must_use = BEHAVIOURAL_INSTANCE_MESSAGE]` and `#[deny(unused_must_use)]` on
every behavioural fn — the denied `#[must_use]` rather than a type, because it names the callee
("unused return value of `child`") and carries the clause as its note, and a statement-position call
is the shape `b2` measured; the transpiler runs `front_end::check_no_behavioural_instance` (its
registry) after the memory rules, and `lower_expr` refuses with the same message as a backstop;
`front_end_parity_ui.rs` gains `LINT_JUDGED`, whose transpiler message must equal the `.stderr`'s
note. Deleted: `lower_hardware_call`, `LowerCtx::{submodules, inst_counters, next_inst_name}`,
`CHIRCombBody`/`CHIRSeqBody`/`SHIRCombBody`/`SHIRSeqBody`/`VLIRCombBody`/`VLIRSeqBody::submodules`,
`{CHIR,SHIR,VLIR}SubmoduleInst::{inputs, output_wire, output_ty/output_width, output_port}`, emit's
legacy instance branch, four `chir_lower` unit tests of the deleted form (one test of the refusal in
their place), `HardwareExecutor::{spawn_child, module_info, module_infos, ensure_module}`, the
`modules` map, `ModuleInfo`, `spawn_child!`. `spawn_child` and `spawn_child!` are deleted rather than
kept: their only use was `tests/module_composition_hybrid.rs`, a testbench, which wires the same
children with `spawn_wired` (same expected stream). (2) `front_end::check_ticking_for_continue` in
`check_front_end_rules` (after R12), `TICKING_FOR_CONTINUE_MESSAGE`; `control_extract`'s arm keeps the
refusal as a backstop with the shared message; II.3.3 row (h). (3) `check_outside_methods` refuses
`==`/`!=` when an operand is `Logic::X`, `Bits::x()` or `Bits::<N>::x()` (`X_COMPARISON_MESSAGE`, the
span on the X operand); the transpiler's `XLit` arm uses the same message. ui/fail
`behavioural_instantiation`, `ticking_for_continue`, `outside_x_comparison`.
Measured basis: predicted and measured (§S.1–§S.3) — corpus 0 for all three (the workspace, every
example and test target, builds with the deny on; `front_end_parity.rs` passes every corpus module);
the SV of all 217 CLI-listed modules byte-identical before and after; the three ui cases refused by
both with one message. Found, not settled by D-23 (§S.4, §S.5): a bound instance (`let _ = child(..)`
— rustc's own help line for the refusal suggests it — or `let m = child(..)`) compiles, its child never
running; a `wire`/`registered_wire` call in a behavioural body; a `Logic::X` pattern; a `continue` of a
tick-free `for` — each admitted by the macro and refused by the transpiler, each an OPEN III.3.4 line.
Coroutine terms: a `#[hardware]` fn's value is a coroutine *not yet started*; only a testbench spawn
or a structural instance starts one, so a behavioural body that makes one and drops it has made a
coroutine that never runs. A `continue` of a ticking `for` is a zero-time jump to the counter's
increment and test, which the coroutine takes and the emitted state machine has no state for yet. An
`== X` asks the coroutine's third value, which the emitted machine does not hold.
Obligations: the four found lines (two decisions — §S.5); the lowering of row (h); III.3.6's counts.

### D-24 — The let-bound instance the second named parity exception, with a drop guard; a `Logic::X` pattern OUTSIDE   (2026-11-08)
Clause: SPEC §II.1.3 (two named exceptions), I.13.5 (a D-24 note: the drop guard), I.14.3(d) (a new
row: the X pattern) and (e) (an X scrutinee matches `_` only), II.1.2's R6 row (the pin renamed), and
III.3.4's lines (`II.3 found let-instance`, `II.3 found x-pattern` CLOSED; `II.3 found const-pattern`
opened). II.3.3 unchanged: neither shape is a lowering limitation. Nothing else normative changed.
Question: (1) should `let _ = child(..)` / `let m = child(..)` in a `sequential`, `synchronizer` or
`combinational` body be refused by the macro, and how? (2) Is `match v { Logic::X => … }` the OUTSIDE
question D-23 (3) refused for `==`, or a value?
Docs say: (1) I.13.5 (D-18 A2, D-23 (1): a behavioural body contains no instantiation; rustc decides);
II.1.3 (a shape one front-end admits and the other refuses is a break; R6 the one named exception).
(2) I.14.3(e) (an X scrutinee matches `Logic::X` or `_`, structurally, as the derived `==`); I.14.3(d)
and D-23 (3) (`==`/`!=` with an X literal OUTSIDE). The packets: `review/MEASUREMENTS.md` §S.5 (1), (2).
Code does (7c9483f): (1) the denied `unused_must_use` fires on an unused *statement* only — a `let`
binding is a use — so the macro admits both spellings and the child's coroutine is dropped unstarted;
the transpiler refuses with I.13.5's message. (2) the macro admits (a structural match); the
transpiler refuses ("unsupported pattern: Logic :: X"). Corpus 0 for both.
Anchor: none — decided.
Prior work: not re-surveyed (§S.5; the questions are the front-ends' reach, not a semantics).
Options: (1) (a) a run-time refusal: `HardwareModule` gains a drop guard that panics when it is
dropped unconsumed by a spawn or `__instance`; (b) the macro refuses, syntactically, a `let` bound to
a bare call of a path not known to be a non-module (over-refuses helper calls, or needs an exemption
list by name); (c) a second named parity exception beside R6. (2) (a) OUTSIDE: `check_outside_methods`
also refuses a `Logic::X` pattern (and an X-valued const it can see); (b) a value: lower the arm as
never taken; (c) leave the break.
Recommendation: (1) (c) with (a); (2) (a) (§S.5).
Decision: **(1) (c) + (a), (2) (a)** — the user, both as recommended (relayed by the coordinator,
2026-11-08).
Changes (branch `claude/phase-c-s5-decisions-c662aa`, `review/MEASUREMENTS.md` §T): (1) (a)
`copper-sim/src/executor.rs`: `HardwareModule::future` is an `Option`; `into_future` and `__instance`
take it; `impl Drop` panics with `UNSTARTED_MODULE_MESSAGE` and the module's name when it is still
`Some` and `!std::thread::panicking()` (a panic in a drop during unwinding aborts). (c) II.1.3 names two
exceptions; `front_end_parity_ui.rs`'s `NAMED_PARITY_EXCEPTION` becomes `NAMED_PARITY_EXCEPTIONS`, two
rows each required to fail with its message and to exist; `front_end_parity.rs`'s
`the_one_named_parity_exception_is_r6` is renamed `the_named_parity_exceptions_are_r6_and_the_let_instance`
and gains a second scan — every behavioural `#[hardware]` fn whose body binds, in a `let`, a call of
a `#[hardware]` fn of the same file (any depth) — whose set must equal the two witnesses of
`tests/let_instance_d24.rs`; ui/pass `let_instance_d24`. (2) `check_outside_methods` visits patterns:
a `Logic::X` path pattern anywhere (`match`, or-patterns, `if let`, `let`, tuple sub-patterns, the
pattern of `matches!`, which it parses), and an identifier pattern naming a `const` item of the body
whose initializer is an X literal (`X_PATTERN_MESSAGE`, citing I.14.3(d)); ui/fail `outside_x_pattern`.
The constant evaluator (`ConstEnv`) is integer-valued and cannot hold X, so "a const whose value is X"
is decided syntactically on the body's own `const` items; a file-level `const` lies outside the
`ItemFn` both rules hold.
Measured basis (§T): predicted and measured — the drop guard tripped on one legitimate path, not the
predicted zero: `tests/reference_leg_measurements.rs::future_size_bounds_the_register_storage` built
seven module values only to measure `size_of_val` and dropped them; it now spawns each into an
executor that is never ticked (a consumption, no design choice), and the measured sizes grow by 8
bytes each (`Option`'s discriminant; 112 → 120 for `counted_repetition`). With that, the bare
regression on `7589204` is REGRESSION OK: every swept case, test, example and `copper-ref` leg passes; `let _ = child(..)` panics
at the parent's first resumption, `let m = child(..)` before the loop when the parent's coroutine is
dropped (the binding is live across the module loop); the X pattern refused by both with one message
in six positions, corpus 0. Found: a pattern naming a non-integer `const` (`const ONE: Logic =
Logic::One`, file-level or in the body; `const UB: Bits<2>`) is admitted by the macro and refused by the
transpiler with a tuple-pattern message (it reads the identifier as an enum variant) — `II.3 found
const-pattern`, OPEN, copper-codegen.
Coroutine terms: a `#[hardware]` fn's value is a coroutine not yet started; dropping it unstarted is
now an observable failure of the simulation, at the instant the value is dropped, rather than a
coroutine that silently never runs. An X pattern asks the coroutine's third value, which the emitted
machine does not hold.
Obligations: `II.3 found const-pattern` (the lowering; how a file-level X-valued `const` pattern is
then refused is a decision for that change); III.3.6's counts.

### D-25 — Memory: one receiver per memory, the mode the owner's, the latency and placement rules in both front-ends   (2026-11-08)
Clause: SPEC I.10.1(a)–(b), I.10.4, I.10.5, I.10.6, I.10.7; I.13.4(d)–(e), I.13.7 (6), I.13.8; II.3.3 rows (i)
and (j) (new); II.3.7's I.10 item; III.3's lines I.10.1, I.10.4, I.10.5, I.10.6, I.13.4, II.3.3 (i), II.3.3 (j).
Question: the memory OPEN lines of III.3 (the open-lines memory batch): how D-16 A7's "ports partitioned
among the receivers, the port set and mode eventually in the parameter type" is realized; whether the mode
is owed to the type; how `L ≥ 1` is checked; whether rule 6 (d) orders against a pre-loop staging; and
whether a `Memory` parameter may leave its latencies defaulted.
Docs say: D-16 1A–8A and its Phase C obligations (I.10.7 (2)–(7)); I.10.5 and I.2.2 — a `Memory` is a
parameter of a `sequential` or `synchronizer` module only; I.13.4(e) — a structural parent's memory "is
owned by the one child it reaches" (the two disagree on the structural parent); `RECEIVED_MEMORY_ABI.md` §3.
Code did (at `e4a9aae`): `ReadMode` a field, no latency check in the simulator (an index panic at the first
edge, `memory.rs:156`/`:360`); rule 6 (a)/(b) refusing in both; (c) blind to `s_pre`; no I.10.6 header rule
(the transpiler refused by a registry accident); `WRITE_LAT ≠ 1` and a latency 0 refused by the transpiler
only, a received `READ_LAT = 0` a transpiler crash (`vlir_lower.rs:2092`); a defaulted `Memory` parameter
admitted by the macro and refused by the transpiler; a structural memory parameter admitted by the macro
for one child.
Measured basis: `review/MEASUREMENTS.md` §W.1 (17 probes, every verdict predicted first; rule 6's retirement
measured on three legs under a temporary patch: 16/16 cases, corpus 0, four pins).
Options and recommendations: §W.3 (1)–(5).
Decision: **(1) A, (2) M1, (3) a3, (4) (i), (5) (ii)** — the user, each as recommended (relayed by the
coordinator, 2026-11-08), with the mechanical items of §W.2.
(1) **One receiver per memory.** The partition of D-16 A7 is realized in its trivial case: `Memory` is not
`Clone` and nothing yields a second handle, so rustc's move refuses a memory handed to two modules; a
`structural` module receives no `Memory` (I.10.5, I.2.2 kept; I.13.4(e) corrected); the port set needs no
type. (2) **The mode is the owner's**, stated by the owner on each leg; the receiving module's text is
mode-free; not carried in the type, and not a limit (II.3.3 row (j)). (3) The simulator's type asserts
`READ_LAT ≥ 1 ∧ WRITE_LAT ≥ 1` at construction, and both front-ends refuse a literal 0 in either latency
position, one message. (4) Rule 6 (c) counts the stagings of `s_pre`; (d) does not order against them.
(5) The transpiler reads a received memory's omitted latencies as the type's defaults, 1 and 1.
Changes (branch `claude/open-lines-memory-71b72a`, §W.5): `copper-core` — `ReadMode` deleted,
`latency_message`, the assert in `Memory::from_data`; `copper-analysis` — `check_memory_staging` without (a)
and (b), `Cfg::pre_staged` for (c), and `memory_rules::check_memory_shapes` (placement, latency, received
`WRITE_LAT`) called once from `front_end::check_front_end_rules`; `copper-codegen` — `received_memory_decls`
admits four to six generics; fixtures `tests/fixtures/memory_staging_dut.rs` (eight swept modules), ui/fail
`memory_combinational_observe`, `memory_combinational_stage`, `memory_structural_parameter`,
`memory_zero_latency`, `memory_zero_latency_parameter`, `memory_received_write_latency`, ui/pass
`memory_parameter_default_latencies`, coreui/fail `memory_one_receiver`, the construction asserts, the
interpreter's I.10.6 refusal pinned; the four rule-6 pins rewritten as admitted shapes.
Coroutine terms: a memory is a variable of H(M) that one coroutine stages and observes; its mode is a
property of the instance its owner built, outside the receiving coroutine; a resumption's several stagings of
one port are its effects in program order, the last consumed at the edge.
Obligations: III.3.6's counts; the coordinator's notes in `RECEIVED_MEMORY_ABI.md` (done in this change:
`WRITE_LAT` REFUSED-both, the defaulted spelling).

### D-26 — An array-register write in a const-generic-bound `for` refused in both; the file-level X-const pattern the third named parity exception   (2026-11-08)
Clause: SPEC §II.1.3 (three named exceptions), I.2.4's `arr[i] = e` and tick-free `for` rows (the
residue of the unrolling), and III.3.4's lines (`II.3 found array-for-generic`, `II.3 found
file-x-const-pattern` CLOSED; I.2.4's line CLOSED; II.1.3's line updated). II.3.3 unchanged: (1) is a
refusal of a shape the unrolling cannot carry, not a dated lowering row; (2) is not a lowering
limitation. Nothing else normative changed.
Question: (1) an element write to an array register inside a tick-free `for` whose bound names a const
generic (`for j in 0..N { arr[j] = … }`) — the transpiler cannot unroll it (the emitted module is
parametric), the macro admits it: lower it some other way, or refuse it in both? (2) a pattern naming a
FILE-level `const` whose value is X (`const UX: Logic = Logic::X; … UX =>`) — OUTSIDE by D-24 (2), refused
by the transpiler once it folds the const (§U.2), admitted by the macro, which cannot see the value: a
third named exception, a macro-side check, or an over-refusal?
Docs say: (1) I.2.4 (a tick-free constant-bound `for` is unrolled), II.3.3 (c) (one write port per
element-write statement); (2) I.14.3(d)–(e), D-24 (2) (an X pattern OUTSIDE), II.1.3 (two named
exceptions; any other one-sided shape is a break). The packets: `review/MEASUREMENTS.md` §U.4, §U.2.
Code does (9c0272a): (1) the transpiler refuses with its lowering message (an element write to array
register … inside a combinational `for`), the macro admits; (2) the transpiler refuses with
`X_PATTERN_MESSAGE`, the macro admits. Corpus 0 for both.
Anchor: none — decided.
Options: (1) (a) refuse in both with one message; (b) a second array-register form (the cycle's words
computed into an array copy in `always_comb`, committed whole); (c) emit per instance. (2) (a) the third
named exception, pinned like R6 and the let-instance; (b) a macro-side `const` assertion per pattern
identifier (needs a trait-based X probe, since the macro cannot tell a const from an enum variant);
(c) refuse every pattern naming a `Logic`/`Bits` file const in both (over-refuses 2-state consts).
Decided (the user, 2026-11-08, as recommended): (1) (a), (2) (a).
Realized (MEASUREMENTS §U.11): (1) `copper_analysis::front_end::check_array_write_in_generic_for`, run
by both front-ends from `check_front_end_rules`, with `ARRAY_WRITE_GENERIC_FOR_MESSAGE`: an array
register is a pre-loop `let mut x = [e; n]` (or an array literal) whose element is not one bit wide
(`Logic::…`, `bool`, `Bits::<1>`: a packed vector) and that `infer_registers` calls a register — the
transpiler's own classification; the rule fires on an element write (plain or compound) to one inside
a tick-free `for` whose range names a const generic of the module. ui/fail `array_write_generic_for`;
witnesses and twins (literal bound, packed vector, ticking `for`) in
`front_end_parity.rs::the_2026_11_08_rules_refuse_their_witnesses_and_admit_their_twins`; the
transpiler's pin requires the shared message. (2) `front_end_parity_ui.rs`'s `NAMED_PARITY_EXCEPTIONS`
gains `("file_x_const_pattern_d26.rs", X_PATTERN_MESSAGE)` (ui/pass); `front_end_parity.rs`'s audit is
renamed `the_three_named_parity_exceptions_have_exactly_their_witnesses` and gains a third scan — every
`#[hardware]` fn whose body has an identifier pattern naming a file `const` whose initializer is
`Logic::X` or `Bits::x()` / `Bits::<N>::x()` — whose set must equal `tests/file_x_const_pattern_d26.rs`'s
`fx_logic`, `fx_bits` (that file pins the transpiler's refusal and the simulator's structural match).
Measured basis (§U.11): corpus 0 for both, as predicted; sv-baseline no corpus module changed.
Coroutine terms: (1) an unrolling is a rewrite of the coroutine's body that needs the trip count; a
parametric module has none to give it. (2) the X pattern asks the coroutine's third value; the macro
admits the question only where it cannot see that it is one.
Obligations: III.3.6's counts; CLAUDE.md's front-end parity paragraph (three named exceptions; the
audit's new name) — the user's to approve.

# Formal-esque semantics of Copper — DRAFT kernel, denotation, rules, and conformance ledger

> **Superseded by `SPEC.md`, normative since 2026-11-09 (D-31).** This was the 2026-11-02 draft of
> a formal semantics. Its two-instant cycle relation (§3.2's `φ`) was rejected by D-8, the one-job
> body, and SPEC Part I is the semantics that replaced it. Its deference to
> `SYNCHRONOUS_SEMANTICS.md` is historical too. Kept because SPEC_DECISIONS.md and
> `FORMAL_SEMANTICS_SURVEY.md` cite it.

**Status: DRAFT PROPOSAL (2026-11-02). Nothing here is decided.** §7 lists the forks
that are the user's to settle; everything before it is written under the recommended
choice so there is a concrete object to react to. Where this draft and
`SYNCHRONOUS_SEMANTICS.md` disagree, the latter wins until this is adopted. The
survey that motivates each device is `FORMAL_SEMANTICS_SURVEY.md`; the per-paper
notes are in `design_docs/prior_art/`.

The aim is the shape the field's one adequate precedent has (Esterel, SCP 1992): a
**kernel** language, a **per-cycle big-step relation** that is the definition, an
**effective implementation** (the simulator) and a **second realization** (the
emitted SystemVerilog), and **stated propositions** relating them, each with its
evidence named honestly (argued and measured, not mechanized). What no precedent has
and this draft adds is the two-instant structure inside a cycle — the opening and
closing anchors — because every divergence Copper has measured lives there.

---

## 1. The kernel — "basic Copper"

Surface Rust accepted by `#[hardware(sequential)]` is desugared into the kernel
below, the way plain Esterel is macro-expanded into basic Esterel and SystemC's
synthesizable subset is a fenced fragment of C++. The desugaring is syntactic:

- A `.read()` of an `In` port inside an expression is hoisted into its own statement
  (`let t = read x;`), in evaluation order, so kernel expressions are pure.
- `while e { s }` ≡ `loop { if e { s } else { break } }`;
  `for v in a..b { s }` ≡ `let v = a; loop { if v < b { s; v = v + 1 } else { break } }`.
- `clk.tick().await` is `tick`. Only this await exists (W2 below).
- Tuple assignment, `match` with a wildcard, `else if` chains: the obvious expansions.

```
Module   M ::= module m ⟨clk : Clock<D>⟩ (x̄ : In, ō : Out, q̄ : RegOut, μ̄ : Memory)
               { let r̄ = c̄ ; loop { s } }                     -- c̄ constant (W5)
Stmt     s ::= skip
             | let v = e | v = e                                 -- local bind / assign
             | let v = read x                                    -- In read (hoisted)
             | o.write(e)                                        -- plain Out drive
             | q.write(e)                                        -- RegOut write
             | tick                                              -- clock boundary
             | s₁ ; s₂
             | if e { s₁ } else { s₂ } | match e { κ₁ ⇒ s₁, …, κ_n ⇒ s_n }
             | loop { s } | break | continue
             | stage_read μ k e | stage_write μ k e₁ e₂          -- memory port k
Expr     e ::= c | v | op(ē) | e[e] | data μ k | is_ready μ k | …  -- pure
```

Values are `Logic` (four-state) and `Bits<N>`; `Bits::x()` is the one non-value
(§3.1). Combinational, synchronizer and structural modules are treated in §5.

### 1.1 Static well-formedness (admissibility), in the normative register

A kernel module is **admissible** iff every rule below holds. A violation is a
compile error in **both** front-ends (front-end parity, 2026-10-23; audit
`copper-analysis/tests/front_end_parity.rs`). Following the SystemC subset's
convention: a construct is either Supported, Supported with the restriction named
here, or Not Supported; **nothing is silently ignored** (Copper has no "Ignored"
class — every accepted statement has a meaning in §3).

| # | Rule (shall) | Implementation |
|---|---|---|
| W1 | A sequential module shall have exactly one `Clock<D>` parameter; every `In`/`Out`/`RegOut` it reads or writes shall carry the same `D` (phantom domain). | `copper-macros` signature check; `copper-core/src/cdc.rs` (`compile_fail` doctests are the executable spec) |
| W2 | The only `await` in a body shall be `clk.tick().await` on that clock. | `copper_analysis::check_awaits` |
| W3 | Deleting every `E_tick` edge, the CFG reachable from the loop head shall be acyclic (every loop path reaches a `tick`; recursively for nested tick-bearing loops). | `Cfg::check_reachability` |
| W4 | A nested loop containing a `tick` shall end at a clock boundary (its test precedes its tick). | `check_nested_loop_shape` |
| W5 | Every pre-loop `let` that becomes a register (§2) shall be initialized by a constant (`is_constant_expr`) or `Bits::x()`. | `check_register_initializers` |
| W6 | A wire shall have one driver: `Out`/`RegOut` are not `Clone`. | `copper-core/src/port.rs` (type system) |
| W7 | A memory port shall be staged at most once per cycle; a result shall be observed only after the edge that produces it; no access inside a tick-free nested loop. | `Cfg::check_memory_staging` |
| W8–W14 | The alignment residue (§4): `multi_write_collapse`, `unprotected_pretick_out_write`, `pretick_out_write_before_update`, `multi_phase_out_write`, `linear_out_write_anchor_mismatch`, `trailing_regout_reads_input`, `trailing_out_reads_input`, `out_first_driven_late`, `memory_result_drives_plain_out`. | `Cfg::*`, both front-ends; `allow_pretick_alignment` silences W9–W14 for demonstration fixtures only |

W1–W7 are **shape** rules: without them the dynamic semantics (§3) is not defined (no
clock to anchor to, a zero-time cycle, an unstated power-on value). W8–W14 are
different in kind: the dynamic semantics *is* defined on the shapes they refuse; they
refuse them because no emitted SystemVerilog realizes it (§4).

## 2. Register inference — a defined function, not a heuristic

**Definition (CFG).** The CFG of `loop { s }` has one node per kernel statement plus
the head `H`; edges `E_comb` (same cycle) and `E_tick` (out of a `tick`); every
fall-off-the-end and trailing tick is a back edge to `H`. (`Cfg::build`.)

**Definition (register).** A local `v` is a **register** iff (1) `v` is assigned
inside the loop and (2) `v` is live across some `E_tick` edge, including the back
edge — the backward-liveness fixpoint with transfer `use ∪ (out − def)`.
(`Cfg::registers`, `copper_analysis::infer_registers`.) Everything else is a wire
(a pre-loop constant, or a same-cycle temporary).

**Lemma 2.1 (sufficiency, to be stated).** The §3 relation carries the whole
environment `ρ` across a tick; restricting the carried environment to the registers
yields the same observation trace, because every non-register local is redefined
before it is read on every path that crosses the edge. *Status: standard liveness
argument; the empirical pin is `register_reconciliation.rs` (codegen's flops ≡ this
set + `pc`) and the four reference-SV structural matches
(`assert_source_registers_match_reference_sv`).*

**Definition (read classification).** An `In` read is **deferred** iff a `tick` node is
`E_comb`-reachable from it within the iteration (without passing `H`), unless it feeds
only a plain `Out` in a body with no registers (the passthrough case, D2); otherwise
it is **immediate**. (`classify_reads`.) This is static and is what both the macro
(barrier injection) and the transpiler consume.

## 3. Dynamic semantics — one cycle as a big-step relation

### 3.1 Time, instants, streams

Edges are numbered 1, 2, …; **cycle N** is `[edge N, edge N+1)`. Inside each cycle
there are exactly **two instants** at which statements execute:

- `t_N⁺` — the post-edge settle of edge N (the executor's `PostEdge` phase);
- `t_{N+1}⁻` — the pre-edge settle of edge N+1 (`PreEdge`).

In time, `t_N⁻ < t_N⁺ < t_{N+1}⁻ < t_{N+1}⁺ …`. **Observation N** is the end of `t_N⁺`.
The environment supplies an **input stream** `x_1, x_2, …` per `In` port: `x_N` is
the value present on the wire during `[t_N⁻, t_{N+1}⁻)` — driven before edge N and held
through cycle N (the harness convention: drive, tick, observe). A wire's value at an
instant is the **last write at or before that instant**, else its initial value
(`wire(init)`). This sentence is the write-window invariant of
`CYCLE_DATAFLOW_SEMANTICS.md` §1 stated as a definition.

### 3.2 Configurations

*(Superseded 2026-11-09 by D-31, as D-7 provided: the two-instant flag `φ` below is replaced by
SPEC I.0.2's one resumption per tick — D-8.)*

A cycle executes a **residue** `s` (the statement left after the last `tick`;
initially `loop { s }`) in a store

```
σ = (ρ, φ, Δ, Π, μ)
  ρ : local environment            φ ∈ {O, C} : the current instant (O = t_N⁺, C = t_{N+1}⁻)
  Δ : list of plain-Out drives (o, value, φ) issued this cycle, in order
  Π : RegOut ⇀ value, the pending writes      μ : memory state (stagings this cycle)
```

The phase flag `φ` is the device that realizes the two instants: it starts at `O`
and **flips to `C` at the first deferred read executed**, never back. This is exactly
what the macro's injected `pre_edge_barrier` does in the simulator, and it is dynamic
on purpose — on a path with no deferred read the whole segment runs at `t_N⁺`.

### 3.3 Statement rules

Judgment: `⟨s, σ⟩ ⇓ ⟨σ′, k⟩`, with completion kind `k ∈ {done, pause(s′), break, continue}`
(Esterel's termination flag, with a residue, plus the two loop exits). `⟦e⟧ρ` is the
pure value of `e` in `ρ`. The rules are deterministic.

```
(Skip)      ⟨skip, σ⟩ ⇓ ⟨σ, done⟩

(Assign)    ⟨v = e, σ⟩ ⇓ ⟨σ[ρ ↦ ρ[v ↦ ⟦e⟧ρ]], done⟩                (let v = e likewise)

(Read-D)    cls(read x) = deferred
            ───────────────────────────────────────────────────────
            ⟨let v = read x, σ⟩ ⇓ ⟨σ[ρ ↦ ρ[v ↦ x_{N+1}], φ ↦ C], done⟩

(Read-I)    cls(read x) = immediate      val = x_N if φ = O, x_{N+1} if φ = C
            ───────────────────────────────────────────────────────────────────
            ⟨let v = read x, σ⟩ ⇓ ⟨σ[ρ ↦ ρ[v ↦ val]], done⟩

(Out)       ⟨o.write(e), σ⟩ ⇓ ⟨σ[Δ ↦ Δ ++ [(o, ⟦e⟧ρ, φ)]], done⟩

(RegOut)    ⟨q.write(e), σ⟩ ⇓ ⟨σ[Π ↦ Π[q ↦ ⟦e⟧ρ]], done⟩

(Tick)      ⟨tick, σ⟩ ⇓ ⟨σ, pause(skip)⟩

(Seq-done)  ⟨s₁, σ⟩ ⇓ ⟨σ₁, done⟩     ⟨s₂, σ₁⟩ ⇓ ⟨σ₂, k⟩
            ─────────────────────────────────────────────
            ⟨s₁ ; s₂, σ⟩ ⇓ ⟨σ₂, k⟩

(Seq-pause) ⟨s₁, σ⟩ ⇓ ⟨σ₁, pause(s₁′)⟩
            ─────────────────────────────────────
            ⟨s₁ ; s₂, σ⟩ ⇓ ⟨σ₁, pause(s₁′ ; s₂)⟩

(Seq-exit)  ⟨s₁, σ⟩ ⇓ ⟨σ₁, k⟩    k ∈ {break, continue}
            ───────────────────────────────────────────
            ⟨s₁ ; s₂, σ⟩ ⇓ ⟨σ₁, k⟩

(If)        ⟦e⟧ρ = true ⟹ as ⟨s₁, σ⟩ ;   false ⟹ as ⟨s₂, σ⟩        (match: the arm whose pattern matches)

(Loop)      ⟨s, σ⟩ ⇓ ⟨σ₁, k⟩
            k ∈ {done, continue} ⟹ ⟨loop { s }, σ⟩ ⇓ ⟨loop { s }, σ₁⟩'s result   (unfold: run the body again, same cycle)
            k = pause(s′)        ⟹ ⟨loop { s }, σ⟩ ⇓ ⟨σ₁, pause(s′ ; loop { s })⟩
            k = break            ⟹ ⟨loop { s }, σ⟩ ⇓ ⟨σ₁, done⟩

(Break)     ⟨break, σ⟩ ⇓ ⟨σ, break⟩        (Continue) ⟨continue, σ⟩ ⇓ ⟨σ, continue⟩

(StageR)    ⟨stage_read μ k e, σ⟩ ⇓ ⟨σ[μ ↦ μ.stage_k ↦ ⟦e⟧ρ], done⟩      (StageW likewise with (addr, value))
```

Notes. (Loop) unfolds within the cycle; W3 guarantees the unfolding reaches a `pause`.
A residue `s′ ; loop { s }` is why the **trailing region** (statements after the last
tick) and the next iteration's head run in one cycle at `t_N⁺`, with no rule needed:
it is the Esterel sequence rule. (Read-I) in phase `C` reads `x_{N+1}` because at
`t_{N+1}⁻` that is what is on the wire; this is why an immediate read after a
deferred one agrees with it. Expressions `data μ k` / `is_ready μ k` read the memory
state the opening edge produced (§3.5).

### 3.4 The cycle step and the observation function

A module's **state** at edge N is `K_N = (s_N, ρ_N, Q_N, M_N)`: the residue, the
locals (registers, by Lemma 2.1), the committed `RegOut` values, the memory array.
`K_1 = (loop { s }, ρ₀ from the pre-loop lets, Q_init, M_init)`.

**Cycle N** (edge N to edge N+1), given `x_N` and `x_{N+1}`:

```
⟨s_N, (ρ_N, O, [], ∅, M_N.pipeline)⟩ ⇓ ⟨(ρ′, φ′, Δ_N, Π_N, μ′), pause(s_{N+1})⟩
ρ_{N+1} = ρ′                                                   -- registers commit at edge N+1
Q_{N+1}(q) = Π_N(q) if q ∈ dom Π_N, else Q_N(q)                 -- RegOut: first edge after the write, hold otherwise
M_{N+1}  = M_N updated by μ′'s stagings at edge N+1             -- §3.5
```

`pause` is the only possible kind at the top (the body is `loop`, W3). A `done`/`break`
at the top level would be a module that halts; the kernel has none.

**Observation (plain `Out`).** Split `Δ_N` into `Δ_N^O` (drives issued with `φ = O`,
i.e. at `t_N⁺`) and `Δ_N^C` (at `t_{N+1}⁻`). Then

```
obs_N(o) = last value in  Δ_{N−1}^C(o) ++ Δ_N^O(o)   if that list is non-empty
         = obs_{N−1}(o)                             otherwise  (hold)
obs_0(o) = the wire's initial value
```

because the last write before observation N is the last opening drive of cycle N if
there is one, else the last closing drive of cycle N−1 (issued at `t_N⁻`), else
nothing changed. This is the persistence law (Esterel §6.1) with Copper's two anchors.
It also states, as a definition, the fact the D1 re-derivation measured: *the
simulator never observes a closing write that an opening write follows*.

**Observation (`RegOut`).** `obs_N(q) = Q_N(q)`. A write at either instant of cycle
N−1 is observed from N on; the anchor is unobservable, which is why `RegOut` is the
universal remedy.

**Trace.** `Obs(M, x) = (obs_1, obs_2, …)`. Two realizations of `M` **agree** iff their
traces are equal for every input stream; the sweep samples this relation (§6).

### 3.5 Memory (summary; the four rules are in `SYNCHRONOUS_SEMANTICS.md`)

A staging at cycle N drives the port's bus for cycle N and takes effect at edge N+1;
`is_ready`/`data` on that port read what that edge produced, so the earliest cycle
that observes it is N+1 (`READ_LAT = 1`); collisions follow `WriteMode`. W7 makes a
second staging of one bus in one cycle, or an observation before the producing edge,
inadmissible. Measured against hand-derived traces (m3,
`tests/cycle_dataflow_memory_derivation.rs`).

### 3.6 Several modules in one domain

Modules are composed by wiring an `Out`/`RegOut` to `In`s (W6: one driver). At each
instant the wires settle **combinationally**: a module's `In` at instant `t` is the
driver's wire value at `t` (its last write at or before `t`, §3.1). The settle is the
least fixpoint over the producer→consumer graph, computed by the levelized scheduler
(SCC-condensed topological order; a convergent SCC iterated to its fixpoint; a
non-convergent one is a runtime error, the one runtime check — `LEVELIZED_SCHEDULING_SCOPE.md`).
`RegOut`, memory and synchronizer outputs change only at edges and so induce no
combinational edge. **Proposition 3.1 (order independence):** the settled values at
every instant are independent of the order tasks are polled. *Evidence:* the
poll-order fuzzer pinned to the fixpoint scheduler, and the levelized-vs-fixpoint
differential (`tests/levelized_differential.rs`). This is Cummings §2.0's race
definition, required to be vacuous.

A **combinational** module is a function `In → Out` evaluated at every instant
(definite assignment on every path, else a latch — refused). A **synchronizer** is a
sequential module exempt from W1's single-domain clause. A **structural** parent is
wiring only.

## 4. The alignment residue — rules W8–W14 as sufficient conditions

The emitted SystemVerilog realizes §3 with **static** forms chosen per drive:

| form | emitted as | realizes |
|---|---|---|
| opening drive (no `In` read reaches it in its arm) | forwarded `assign` / comb mux + `<port>_hold` | `Δ^O` at `t_N⁺` |
| closing drive (a read reaches it) | edge-sampled flop / committed-register assign | `Δ^C` at `t_{N+1}⁻`, visible from N+1 |
| `RegOut` | `always_ff` | `Q_{N+1}` |

Codegen's anchor is a **static** per-drive predicate ("does a read comb-reach this
drive on *any* path", arm-granular); the definition's anchor is **dynamic** (`φ` on
*this* path). They coincide unless the two can differ — and each rule W8–W14 is one
way they differ, written as a shape. Stated as a proposition:

**Proposition 4.1 (agreement, conjectured).** For every admissible module `M` and every
input stream `x`, `Obs_sim(M, x) = Obs_SV(M, x)`, where `Obs_SV` is the Verilated
module's post-edge value trace under the harness convention.

*Status.* Not proven. Argued per rule in `CYCLE_DATAFLOW_SEMANTICS.md` §§3–4, 9–10
(each rule derived from the write-window invariant before it was measured), and
measured by the corpus differential sweep on every module in the tree across five
stimulus sequences. Every counterexample found to date became either a new rule
(a shape made inadmissible) or a new emission form (the shape given its §3 meaning
in SV); the doc records which, with the witness. The honest statement of
Proposition 4.1 is therefore: *no admissible module in the corpus is a counterexample,
and the rule set is the derived set of shapes for which the static form cannot equal
the dynamic anchor.* Each rule's mechanism, in §3's terms:

- **W8 `multi_write_collapse`** — a drive in `Δ^C` of cycle N and one in `Δ^O` of N+1
  on the same port with the pre-tick write intended for observation N: §3.4's
  observation shows only the later; no SV shows the earlier.
- **W9 `unprotected_pretick_out_write`** — a drive that is in `Δ^O` on one path and
  `Δ^C` on another (path-dependent anchor), or an every-path port with mixed anchors
  (the comb mux has no hold slot for the `Δ^C` drive), or a `Δ^O` drive under a guard
  that tests a register assigned earlier in the segment (the comb copy tests
  `ρ_N`, the definition tests the forwarded `ρ`).
- **W10 `pretick_out_write_before_update`** — a `Δ^C` drive computed from `ρ` before the
  segment updates the register it reads: the value is `f(x_{N+1}, ρ_N)`, a mixed
  generation no `assign` shows at any observation.
- **W11 `multi_phase_out_write`**, **W12 `linear_out_write_anchor_mismatch`** — the same
  mismatch on the extraction route (a port driven in several `pc` states) and on the
  linear route (where the form is chosen by the operand's kind and the drive's shape).
- **W13 `trailing_regout_reads_input` / `trailing_out_reads_input`** — a drive in the
  trailing region whose value is an immediate read `x_N`; the SV samples `x_{N+1}`.
- **W14 `out_first_driven_late`**, **`memory_result_drives_plain_out`** — a startup
  transient (first drive after edge 2 against an `assign` live from time 0) and a
  plain `Out` wired to a memory result that re-captures every edge.

## 5. Propositions and their evidence, collected

| | Statement | Status | Evidence |
|---|---|---|---|
| P1 | §3.3 is deterministic and total on admissible modules (every cycle derives exactly one `pause`). | argued | W3 + structural induction on `s`; no pin |
| P2 (Lemma 2.1) | Carrying only the registers across an edge preserves `Obs`. | argued | liveness; `register_reconciliation.rs`, reference-SV reg matches |
| P3 (Prop 3.1) | Settled values are poll-order independent. | measured | `tests/poll_order_fuzz.rs`, `levelized_differential.rs` |
| P4 | The simulator implements §3: for every admissible `M`, `Obs_exec(M, x) = Obs(M, x)`. | **scoping finding, not measured** | `PAIRED_IMPLEMENTATION_SCOPE.md` §0 ("the executor already implements the model"); no executable reference to compare against (gap G2) |
| P5 (Prop 4.1) | For every admissible `M`, `Obs_sim = Obs_SV`. | conjectured, measured | the sweep (G-D), the d*/e*/v8*/e1x* batteries, the independent anchors (G-E) |
| P6 | Two iterations suffice: every W8–W14 verdict is decided on `body ; body`. | **unstated** | Anvil Lemma C.19's cut-vertex argument transfers to `Cfg`'s back edge; worth stating because the trailing rules reason about exactly that pair |
| P7 | Admissibility is the same predicate in both front-ends. | measured | `front_end_parity.rs` |

## 6. Conformance ledger — does Copper implement §3 as written?

Each clause of §3, where the two implementations realize it, and what pins it.
**Gaps are marked G1–G6.**

| Clause | Simulator | Transpiler | Pin |
|---|---|---|---|
| Two instants per cycle (§3.1) | `tick_clock`: `pre_edge_settle` / `post_edge_settle`, `PollPhase` per domain | phase guards in `always_ff` / `always_comb` | `tests/det_010_independent_golden.rs` and every equivalence test |
| `φ` flips at the first deferred read (§3.2) | `pre_edge_barrier` injected by the macro before each deferred `.read()` | `shir_lower.rs` `seen_in_read` walk (static, per arm) | **G1: no audit that the static anchor equals the rules' `leading_read_reaches`** |
| (Read-D) `x_{N+1}`, (Read-I) `x_N` | barrier + plain `In::read` | edge-sampled vs combinational operand | `analysis_semantics.rs::pretick_reads_are_deferred`, D2 fix test |
| (Out): wire write visible at the instant | `Out::write` (immediate, dirty flag) | the three forms of §4 | `hold_form_audit.rs`, `sv-baseline` |
| `obs_N` = last of `Δ_{N−1}^C ++ Δ_N^O`, else hold (§3.4) | falls out of instants + last-write wires | mux + `<port>_hold`, enabled register | **G3: stated only by the d*/e* probes; no unit property names the closing-then-opening overwrite** |
| (RegOut): `Q_{N+1}` from `Π_N` (§3.4) | `RegOutShared::pending` committed in `Clock::advance` | `always_ff` | `regout_forwarding_equivalence.rs`, `regout_trailing_single_tick` |
| Registers commit at edge N+1; forwarding within the segment | plain Rust locals | forwarded `edge_value` | `sequential_forwarding_divergence.rs` (m2, V-battery) |
| Residue `s′ ; loop { s }` = trailing region in the head's cycle | rustc's coroutine | `shir_lower::lower_seq_body`, `control_extract` | `trailing_statements_dut.rs` in the sweep |
| Power-on `K_1` from the pre-loop lets (W5) | the `let` values | `initial` block | `register_initializer_corpus.rs`, `COPPER_VERILATOR_X_INITIAL=1` |
| Memory staging (§3.5) | `Memory` pipelines on `on_posedge` | staged ports ABI | m3 tests |
| Combinational settle, order independence (§3.6) | levelized / fixpoint schedulers | n/a (SV is continuous) | P3 pins; **G4: non-convergent SCC panic unpinned** |
| Proposition 4.1 | — | — | the sweep; **G5: ports only — internals (registers, `pc`) are not compared, Verismith §3.3's strengthening** |
| P4: the executor computes §3 | — | — | **G2: no executable reference semantics; a kernel interpreter would be the third leg** |
| P6: two iterations suffice | — | — | **G6: unstated; the trailing rules assume it** |

**What the ledger says.** The semantics as written is implemented, clause by clause,
and nearly every clause has a measured pin. The gaps are not behaviors but
*authorities and statements*: the anchor is computed twice with no cross-check (G1);
the proposition that the simulator is a correct implementation rests on a scoping
argument because there is no third thing to measure it against (G2); two definitional
facts are carried by probe batteries rather than named properties (G3, G6).

### 6.1 Closing the gaps — proposed work, smallest first

1. **G1 (one anchor authority).** Expose `Cfg::drive_anchor(node) → {Opening, Closing,
   PathDependent}` in `copper-analysis` (it exists in pieces: `leading_read_reaches`,
   `read_free_path_reaches`), and add an audit test in `copper-codegen` that, for every
   corpus module, the `opening` flag `shir_lower` attaches to each `SHIRPortDrive`
   equals the analysis verdict. If they ever differ, one of the two is wrong *today*.
   Then, optionally, make `shir_lower` consume the analysis — the c2 discipline.
2. **G3, G6 (name the facts).** Two unit properties over `Cfg` and the executor: the
   observation rule as a hand-traced test on a two-drive module (closing in N−1,
   opening in N, assert only the opening value is observed), and a statement of P6 with
   a test that each W8–W14 rule returns the same verdict on `body` and on `body ; body`
   unrolled.
3. **G4.** Pin the non-convergent-SCC panic with a two-inverter loop.
4. **G5.** Extend the sweep's compared vector with the module's registers and `pc`
   (Verismith's "concatenate the internals"); divergences hidden behind a port that
   happens not to expose them become visible.
5. **G2 (the reference interpreter).** Implement §3.3–3.4 directly as an interpreter
   over the kernel (FIR is already source-shaped and the CFG exists), run it as a third
   leg of the sweep, and compare `Obs_ref` with `Obs_sim` on every corpus module. This
   is impl-plan item 7's "validation-only backend", never the default. It is the only
   way P4 becomes a measurement; it is also the largest item and should be decided, not
   assumed.

## 7. Decisions that are the user's

- **D-a Object.** Kernel over the *surface* (desugared Rust, what both front-ends see)
  with inference as a defined function — recommended — versus meaning defined on the
  post-inference IR with inference proven meaning-preserving.
- **D-b Primary style.** Big-step per cycle with residue and completion kind
  (Esterel's form, deterministic) — recommended — with the valued cycle-indexed trace
  `Obs` as the *derived* observation (Anvil/Filament's carrier). The alternative is
  the trace as the primary definition, which cannot say where inside a cycle a read
  happens without re-introducing the instants anyway.
- **D-c The anchor.** Dynamic `φ` as the definition (what the simulator does on every
  path; every measurement confirmed it), codegen's static form as the realization,
  W8–W14 as the conditions for their equality — recommended. The alternative, a static
  anchor as the definition, declares the simulator wrong on every path-dependent shape
  it handles today.
- **D-d Explicit anchor syntax.** Esterel resolved its off-by-one with two syntactic
  forms (`await` vs `await immediate`). Copper could offer `read_now()` / `read_at_edge()`
  instead of inferring deferred vs immediate from position. Flagged only; the inference
  is anchored to independent hardware (`det_010`) and changing it is a language change.
- **D-e Proof bar.** Statements with argued derivations and measured evidence (the
  field's own bar: Esterel, Filament and Anvil mechanize nothing) — recommended — versus
  mechanizing P1/P2/P6 in a proof assistant.
- **D-f The reference interpreter (G2).** Build it as a third leg, or accept P4 as a
  scoping finding. This is the one item that changes the project's shape.

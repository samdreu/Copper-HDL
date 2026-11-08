# The Copper language specification

**Status: UNDER CONSTRUCTION (started 2026-11-03). Not yet normative.** Until this
document is adopted, `SYNCHRONOUS_SEMANTICS.md` remains the reference and wins where
the two disagree. Every clause below is decided in `SPEC_DECISIONS.md`, one entry per
clause, with the options considered, the prior work consulted, the independent anchor
if any, which implementation changes, and the date. A clause marked **TBD** has no entry
yet. The baseline the review runs against is `review/` (the 2026-11-03 inventories).
**`SPEC_OVERVIEW.md` is the generating page (2026-11-04):** every clause here must follow
from it or from a design choice it lists; `review/DECISION_AUDIT.md` sorts D-1…D-7 by that
test. Its question Q1 (two jobs or one) was decided 2026-11-04 — **D-8, the one-job body**
— and §I.0–§I.6, first re-derived in place, were consolidated under it 2026-11-05 (D-15):
they read as one model; §I.5 keeps its number as a forwarding stub, and the two-job text is
in the decision log's D-1…D-8 entries.

How this document is organized. Part I is what a program *means*: the kernel, the
per-cycle relation, the observation function. Part II is what programs are *admitted*:
every rule gives a reason that is a clause of Part I, never "the emitted form cannot
show it". Part III is what the two implementations *owe* and how it is checked: the
propositions, the agreement relation, and the conformance ledger that maps every clause
to the simulator, the transpiler, the reference interpreter and the test that pins it.

Conventions. Clauses are numbered `§<part>.<section>.<n>` and never renumbered; a
superseded clause keeps its number and gains a "superseded by" note. Prior work is cited
as `[paper §section]` with the paper's note in `prior_art/`. "Anchored" means measured
against independent hardware (BaseJump, a `REFERENCE` golden, a `with_verilog` example);
"decided" means a recorded design choice with its rationale; a clause is one or the other.

---

## Part I — Meaning

### I.0 The module as a coroutine

Rewritten 2026-11-05 under D-8 (D-15); the two-job text is in the decision log's D-1…D-7
entries and the packet-15 snapshot.

Decided: D-7 (2026-11-04) and D-8 (2026-11-04) — one suspension kind, one resumption per
cycle. Anchor: none of its own — a definition. Its observable consequences are the anchors
of I.1–I.6, the `RegOut` spellings measured in `review/Q1_TRADEOFFS.md` §4: `bsg_dff_en`,
`bsg_counter_up_down` and `det_010` (the resumption's `RegOut` writes shown at the next
observation, identical to their references); `det_010_awaits`, `lfsr`, `shift_register` and
`sipo_block` (the resumption's reads see `x_{N+1}`, its `RegOut` writes take effect at edge
N+1); `sync_2ff` vs `examples/cdc/sv/sync_2ff_ref.sv` (two flops, one destination cycle of
latency: `ff1` a register, `q` a `RegOut` — measured by packet 9, I.7.5; D-9 note,
2026-11-04: `review/Q1_TRADEOFFS.md` §4 measures five modules and `sync_2ff` is not among
them). The projection H(M) against the emitted state machine is sim ≡ SV only (the sweep,
`copper-codegen/tests/register_reconciliation.rs`, the four G2 structural matches). Decided:
the one suspension kind (D-8), the projection by tick site, the register boundary of I.0.5.
Each of I.1–I.6 carries a one-line reading in this section's terms after its `Decided:`
paragraph.

**I.0.1 The coroutine.** A sequential module instance is the kernel function of I.2 run
as a **coroutine**: `async move { s_pre ; loop { s } }`. Its **suspension sites** are its
`tick` sites — **one kind** (D-8). Its **state** is (the suspension site, the values of
its locals, its control position). A **resumption** runs the kernel, with the Rust meaning
I.2 gives it, from one `tick` site to the next it reaches. Its **effects** are the
`RegOut` pending values it sets (I.6), the memory stagings it performs (→ I.10; D-16: I.10.1(b) — one staging per port, an effect the edge consumes, the last per resumption winning) and the
locals it leaves; a sequential module has no plain `Out` (D-8; the combinational module's
wire → I.11, D-14: I.11.2). rustc's `async` lowering is the simulator's realization
of this coroutine, exact by construction: the body is wrapped unchanged and the macro
injects nothing into it (I.0.7); control extraction's `match pc` is the transpiler's
encoding of its projection (I.0.4). The residue of I.3.3 is the `tick` site. I.2.5(b)'s
"the only suspension is the cycle boundary" is literally the definition: the only
`await` is `tick`. The definition is on the kernel, not on rustc's `Future` layout, which
stays an implementation form (I.3.3): meaning is never read off it.

**I.0.2 The resumption protocol.** Per tick of its clock's domain `D` (I.1.5), in order:
(1) **the resumption** (`t_{N+1}⁻`) — after the environment's drive of `x_{N+1}` (I.1.5),
the coroutine parked at a `tick` is resumed and runs to the next `tick`, where it parks;
every input it reads is `x_{N+1}`, the value present before the edge it ends at (I.4.1).
(2) **The edge** (N+1) — every pending `RegOut` value takes effect (I.6.1) and every memory
advances (→ I.10; D-16: I.10.1(c) — capture then commit, or the reverse, in the instance's mode order); no statement runs (I.1.2). (3) The settle of the combinational network
(→ I.13; D-18: I.13.2(b) — the network settles on the committed values after the edge, for the observation) and the observation `obs_{N+1}` (I.0.3). The **first resumption** of a coroutine
precedes edge 1, from its entry: it runs `s_pre` and the loop's head to the first `tick`,
reading `x_1` (I.3.7, I.4.4). A coroutine is therefore resumed exactly once per cycle, at
the one instant of I.1.2 — the resumption that ends at edge N+1 is cycle N's — and nothing
parks mid-resumption. Seen from the coroutine the order is the edge, then one run to the
next `tick` [prost §2–§3]; the two readings are one protocol because no statement runs at
the edge. Per-domain gating and the never-ticked domain → I.12 (I.0.8). (D-17: I.12.1–I.12.2 — a coroutine is resumed by its own domain's edges only and has executed nothing before its first; today's any-domain first settle is a measured startup divergence, Phase C.)

**I.0.3 The trace.** `obs_N` (N ≥ 1) is the value of every port after edge N and the
settle of I.13 (D-18: I.13.2); `obs_0` is the value of every port after the first resumption and its
settle (I.1.4) — for a sequential module's ports `init(q)` (I.6.6), since the first
resumption's writes take effect at edge 1. The environment's drive (I.1.5) falls between
`obs_N` and the resumption that ends at edge N+1: the inputs that resumption reads are
`x_{N+1}` (I.4.1). One resumption, one observation per cycle.

**I.0.4 The hardware projection H(M).** H(M) is the state machine with one **state per
`tick` site** of the kernel and the **variables** (i) the registers (→ I.7), (ii) the
`RegOut` values (I.6), (iii) the memories (→ I.10; D-16: I.10.1(a) — the array and the delivery pipelines). Its **cycle-N transition** is the one
resumption that ends at edge N+1, with inputs `x_{N+1}`: a function from (site, variables,
`x_{N+1}`) to (next site, variables), the edge committing the variables and `obs_{N+1}`
read after it (I.0.3) — Prost's `advance : State × In → State` [prost §3].
The transpiler's `pc` register (the extraction route) and `phase_r` register (the linear
route) are **encodings** of H(M)'s state: `pc` is allocated per tick *occurrence* in the
lowered form and is many-to-one onto tick sites (D-20: corrected — many-to-many: `pc` is allocated per continuation, sites whose continuation is the head merge into state 0 and one site's continuation is cloned per arm, `review/MEASUREMENTS.md` §D.3; III.1.3) (the continuation after a branch that
ticks is lowered once per arm), `phase_r` is a position in the textual split; neither
carries meaning of its own (I.2.1, I.3.3). That the emitted machine is an encoding of
H(M) — the same transitions over the same sites — is a proposition of III.1 (D-20: P1, a bisimulation over continuations, MEASURED as consistency only), unmeasured
today (I.0.8). The extraction route's re-execution of pre-loop statements (I.2.8, I.3.9)
is a projection of no resumption — the coroutine runs `s_pre` once, in its first
resumption — and is a transpiler defect owed to I.9 and Phase C, not a meaning this
section leaves open.

**I.0.5 The register proposition.** (a) A local that is **live at a `tick` suspension**
— assigned before it and read after it, on some execution, before being reassigned — is
a **register**; replacing every other local by an arbitrary value at every tick
suspension leaves the trace unchanged. This is a proposition about the coroutine
(FORMAL_SEMANTICS Lemma 2.1, now stated), checked by a poison run (→ III.1; D-20: P2, measured). (b) A value
one resumption computes and a later point of the same cycle uses is realized
combinationally iff it is a function of the registers and the inputs present *at that
point*; otherwise a realization stores it in a **carrier flop**, which is not a register
of (a). Under one resumption per cycle that point is the edge the resumption ends at, so
every such value is a function of the registers and `x_{N+1}`, and no carrier is known
(D-8). `sync_2ff`'s `ff2` (`src/sync.rs`) is assigned and consumed within one resumption
and is dead at the `tick`: a wire of that resumption, neither a register of (a) nor a
carrier — the second flop `sync_2ff_ref.sv` anchors is the `RegOut` `q` (D-6). The
category is kept, empty, for what Phase C finds. (c) rustc's `Future` layout is a superset
of (a) by construction; `infer_registers` (`copper-analysis/src/cfg.rs`, the tick clause
and the back-edge clause) is an implementation computing (a) together with the back-edge
clause's surplus over (a) — eight wires of one resumption in seven modules (D-9:
re-derived in I.7.5; the clause is retired in Phase C). **Decided (D-7 A1-i; D-8):**
"register" in this document names (a); a carrier is realization state, named in III if
Phase C finds one; II.3 owes none; I.7 instantiates the split.

**I.0.6 The three admissibility principles.** Every Part II rule is an instance of one
of three principles, and a rule must say which. **Shape** (→ II.1): the coroutine has
one suspension kind (the only `await` is `tick`; D-8); every path of the loop reaches a
`tick`, so every resumption ends at a suspension; its state is finite (I.2.5(c)); it never
returns (I.2.5(d)); its outputs are `RegOut` (D-8). **Design** (→ II.2): every effect is
observable in some trace (every `RegOut` write lands in an observation, I.6.4; the dead
write of I.5.5 has no sequential instance, D-8); one spelling per meaning (a Mealy output
is a `combinational` module, never a drive inside a clocked body; D-8). **Realizability**
(→ II.3): the emitted text must realize **the resumption's** effects in every cycle — its
reads of `x_{N+1}`, its register updates and its pending values, shown from `obs_{N+1}` —
and a shape the emitted forms do not realize is refused as a **dated lowering limitation
with a fix direction**, never as a clause of meaning. The eight alignment rules and the
five plain-`Out` forms of the two-job body existed to realize two instants with one set of
logic and are retired with the second instant (D-8; `review/Q1_TRADEOFFS.md` §5), together
with the static predicates that approximated it (II.2.2 names them). (D-12: II.1.1 numbers the shape clauses S0–S5, the grammar being S0; II.2.1 lists the surviving derived rules D1–D3 and retires `multi_write_collapse`; II.3 is II-b's.) (D-13: II.3 written — six dated refusal rows and six defect rows, every one a transpiler form, none a meaning; rules 7 and 9–15 retired, 11 vacuous by its predicate.)

**I.0.7 Implementation map.** Simulator: the macro wraps the body unchanged in
`HardwareModule::__new(async move { … })` (`copper-macros/src/lib.rs:878-888`) after
running every check on the pristine function; two futures and no wakers — `ClockTick<D>`
(`copper-core/src/types.rs:948-1008`: target fixed at `tick()`, Ready iff `cycle ≥ target`
and the domain is resolving) and `DeltaYield` (`copper-sim/src/lib.rs:70-81`, the
combinational wrapper's, → I.11; D-14: I.11.6); the edge is `Clock::advance`
(`types.rs:886-899`: cycle += 1, then the listeners — `RegOut` commit
`copper-core/src/port.rs:173-183`, memory `copper-core/src/memory.rs:73`); nothing is
polled at spawn (`copper-sim/src/executor.rs:364-416`), so the first resumption is the
first pre-edge settle; the levelized settle polls each task once (`:546-558`) and asserts
the future never completes (`:561-569`); a fixpoint re-poll of a parked coroutine runs no
statement. Effects: `RegOut::write` (`port.rs:194-196`), memory stagings (`Out::write`,
`port.rs:86-92`, is the combinational module's, → I.11; D-14: I.11.1, I.11.6). Analysis:
`infer_registers` (`cfg.rs:399-411`) is I.0.5(c)'s implementation. Transpiler:
`control_extract::lower_into` (`copper-codegen/src/control_extract.rs:1208-1358`)
allocates `pc` per tick occurrence (D-20: per continuation, III.1.3) (`:1217`), duplicates the continuation of a ticking
branch into every arm (`:1310`, `:1315`, `:1333`), emits a trailing tick as `pc = head`
(`:1213-1215`), hoists a local mentioned in another state (`:914-960`, by name) and
moves pre-loop expression statements into state 0 (`:526-549`);
`shir_lower::split_at_ticks` (`copper-codegen/src/shir_lower.rs:873-883`) is the linear
route's `phase_r`. Reference: a kernel coroutine interpreter in Rust — state `Entry |
Tick(site)`, steps `resume`, `edge`, `observe` — is the executable form of I.0.1–I.0.3
and the third leg of the sweep (→ III.2; D-20: III.2.0, the reference leg G-F); a task of its own.
Today (two-job implementation, Phase C rewrites): the macro rewrites a deferred read site
to `{ pre_edge_barrier::<D>().await; x.read() }` (`copper-macros/src/lib.rs:712-725`),
placed by `classify_reads` (`cfg.rs:3804`); a third future, `PreEdgeBarrier<D>`
(`copper-sim/src/lib.rs:101-117`: Ready iff the domain is pre-edge, stateless), parks it;
`tick_clock` (`copper-sim/src/executor.rs:813-853`) runs `pre_edge_settle` (phase
pre-edge, resolving off, poll), `Clock::advance`, `post_edge_settle` (phase post-edge,
resolving on, poll), so a coroutine is resumed in both settles. Phase C makes this one
resumption per tick — the coroutine parked at a `tick` is resumed in the settle that
follows the environment's drive and precedes `advance`, and runs to its next `tick`; the
post-edge settle resumes no coroutine. Obligations carried to Phase C (D-8): the
simulator's one resumption per tick and the removal of `PreEdgeBarrier`; the macro's
signature rule (no `Out` on a `sequential` or `synchronizer` module) and the removal of
the injection, of `classify_reads` and of the eight alignment rules with their opt-out;
the transpiler's single port form (`split_output_regs`) with the five plain-`Out` forms
and the opening/closing tagging removed; the pre-loop re-execution fix (I.9); II.3 owes
no form. (D-13: II.3.3 lists the forms it owes today.)

**I.0.8 Carried NOT-YET list.**
- Per-domain gating: `poll_tasks` resumes every domain's coroutines in every settle,
  gated by per-domain phase and resolving flags; a module whose clock has never ticked
  reads pre-edge and resolving-on, so its first resumption runs during the first settle
  of any domain (→ I.12; D-17: I.12.2 — a realization defect, measured divergent, Phase C; closed).
- Memory stagings as effects and the memory's edge step (→ I.10). (D-16: I.10.1(b)–(c) — closed.)
- The pre-loop re-execution under control extraction — a defect of the projection,
  defined by I.0.2's first resumption (→ I.9, Phase C). (D-11: I.9.3 — on both routes: the
  extraction route re-runs `s_pre`'s expression statements at every return to state 0, the
  linear route drops them; both owed to the entry obligation of I.9.2.)
- The propositions of III.1 (D-20: P1, P13, P2, P4) this section states and does not measure: the emitted
  machine encodes H(M) (a static test of `pc` states against tick sites beside
  `tests/timing_model_derivations.rs`, plus the interpreter's reached-site set); hoisted
  locals ⊆ registers ∪ carriers; the poison run for I.0.5(a), with `sync_2ff`'s `ff2`
  the first case (expected poisonable — a wire of one resumption, not a register);
  rustc's layout as a superset (`-Zprint-type-sizes` on a few fixtures,
  recorded in `review/MEASUREMENTS.md`).
- `poll_tasks` outside a tick and a mid-cycle drive (I.1.6 → III.2; D-20: III.2.3 — outside the language).
- The combinational module's coroutine, one evaluation per settle (→ I.11). (D-14: I.11.1 — a
  function evaluated by the wrapper coroutine, one evaluation per settle, the count a scheduler
  artifact.)

### I.1 Time, instants and the environment

Rewritten 2026-11-05 under D-8 (D-15); the two-job text is in the decision log's D-1…D-7
entries and the packet-15 snapshot.

Decided: D-1 (2026-11-03); its instant structure by D-8 (2026-11-04) — one instant per
cycle; the convention, `obs_0` and per-domain numbering are D-1's. Anchor: the
drive–tick–observe convention shared by every hardware-anchored check
(`review/MEASUREMENTS.md` §C); the first instant and observation 0 are decided, not
anchored.

In coroutine terms (D-7, D-8): the one instant of cycle N is its one resumption —
`t_{N+1}⁻`, the resumption that ends at edge N+1; cycle 0's instant is the first
resumption, from the entry; `obs_N` is read after edge N (→ I.0.2, I.0.3).

**I.1.1 Domains and edges.** Each clock domain `D` has an infinite sequence of edges
numbered `1, 2, 3, …`. Numbering is per domain; no relation between the edges of two
domains is assumed (→ I.12; D-17: I.12.1 — any interleaving of the domains' ticks, including none). **Cycle N** of `D` is the interval `[edge N, edge N+1)`.
**Cycle 0** is the interval before edge 1.

**I.1.2 Instants.** Within cycle N (N ≥ 1) a module's statements execute at exactly
**one** instant, `t_{N+1}⁻`: the resumption that ends at edge N+1, after the
environment's drive for that edge and before it. In time, `edge N < t_{N+1}⁻ < edge N+1`.
Cycle 0 has one instant, `t_1⁻`, at which the pre-loop statements and the first segment
execute (→ I.9); nothing executes before it. No statement executes at an edge, and none in
the settle after it, which is the combinational network's (→ I.13; D-18: I.13.2(b) — the settle resumes no coroutine).

**I.1.3 State and wires.** Registers, `RegOut` ports and memories change value only
at an edge (→ I.6, I.7, I.10; D-16: I.10.1(c) — a memory changes only at an edge of its own domain). Wires — the outputs of combinational modules (→ I.11,
D-14: I.11.1; a sequential module has none, D-8) — change at instants: a wire's value at
an instant is its last write at or before that instant, else its initial value (→ I.11.2,
D-14: a combinational wire's value is its evaluation's at every point, its creation value
unobserved). At each instant the combinational network settles to a fixpoint before the
instant ends (→ I.13; D-18: I.13.2(a) — a function of the sources, unique, one topological pass); every read and every observation sees settled values.

**I.1.4 Observation.** `obs_N`, for N ≥ 1, is the value of every port after edge N and
the settle that follows it (for a sequential module's port, a `RegOut`, what the
resumption ending at edge N committed, I.6.4). `obs_0` is the value of every port after
the settle at `t_1⁻`, before any edge. The **trace** of a module under an input stream is
`obs_0, obs_1, obs_2, …`. Observation 0 is part of the trace: a startup transient is
observable and is given a meaning in I.9, not hidden by the harness. (D-11: I.9.4 —
`obs_0(q) = init(q)` for every port of a sequential module, measured equal on both sides;
the harness observes it in Phase C.)

**I.1.5 The environment.** The environment has exactly three operations on a domain:
**drive** (set the value of an `In` wire), **tick** (advance `D` through `t⁻`, the edge,
and `t⁺`), and **observe**. A drive is permitted only between an observation and the
next tick — before `t_1⁻`, or after `obs_N` and before `t_{N+1}⁻` for N ≥ 1. The **input
stream** `x_N` is the value present on an input at `t_N⁻`, held through edge N; every
read of the resumption that ends at edge N+1 sees `x_{N+1}`. A drive at any other time,
and a settle requested outside a tick, are **outside the language**: no meaning is
defined and the agreement relation (III.2; D-20: III.2.3) does not cover them.

**I.1.6 Implementation map.** The simulator realizes `t⁻` as `pre_edge_settle`, the
edge as `Clock::advance` (listeners commit), and `t⁺` as `post_edge_settle`
(`copper-sim/src/executor.rs`). Today (two-job implementation, Phase C rewrites): a tick
resolves only at `t⁺` and a deferred read parks at `t⁺` and resumes at `t⁻`; Phase C
resumes the coroutine once per tick at `t⁻`, after the drive, and removes the barrier
(I.0.7). Obligations carried to Phase C: the executor's global cycle counter is an
implementation artifact and must become per domain or be renamed (→ I.12; D-17: I.12.6 obligation (8)); `poll_tasks`
outside a tick and a mid-cycle drive are candidates for refusal; the harness must observe
`obs_0` once III.2 adopts it (D-20: adopted, III.2.1; Phase C).

### I.2 The kernel

Rewritten 2026-11-05 under D-8 (D-15); the two-job text is in the decision log's D-1…D-7
entries and the packet-15 snapshot.

Decided: D-2 (2026-11-04). Anchor: none — decided (a syntax clause). The meanings the
kernel carries are anchored at their own clauses (→ I.3–I.12); the kernel adds none.

In coroutine terms (D-7, D-8): the kernel is the coroutine's body; `tick` is its one
suspension kind; the IRs and the `match pc` FSM are encodings of its projection H(M)
(→ I.0.1, I.0.4).

**I.2.1 The object.** The object this specification gives meaning to is a `#[hardware]`
function as the user wrote it — the `syn::ItemFn` both front-ends hold — after the
desugaring `D` of I.2.4. `D` is syntactic and preserves Rust meaning: running the
surface function and running its kernel form are the same computation (the one
construct for which `D` is undefined is carried in I.2.8). The **kernel** is the
desugared function. Register inference (→ I.7), read classification (→ I.4) and
segmentation (→ I.3) are functions *defined on* the kernel, not primitives of it. The
transpiler's intermediate forms — FIR, CHIR, SHIR, VLIR — and the `match pc` state
machine that control extraction builds are implementation forms: they carry no meaning
of their own, and the transpiler is correct iff the text it emits agrees with the
kernel's meaning (→ III.2; D-20: III.2.0, P8). The simulator runs the surface function as written; by the
identity above that is running the kernel.

**I.2.2 The module header.** A module is

```
#[hardware(mode)]
async fn name<generics>(params) { s_pre ; loop { s } }
```

`mode` is one of `sequential`, `combinational`, `synchronizer`, `structural`
(→ I.2.7); the attribute takes no second token (the two-job body's opt-out flag is
retired with the rules it opted out of, D-8; II.1 R9 refuses it). `generics` are
const generics and `D: ClockDomain` parameters; the object given meaning is a module
**instance** at concrete parameters, and a generic module means the family of its
instances. `params` are: exactly one `Clock<D>` for a `sequential` module (I.2.8 carries
the two-clock case); ports `In<T, D>` and `RegOut<T, D>` (→ I.4, I.6), and on a
`combinational` module only `Out<T, D>` (→ I.11, D-14: I.11.1; on a `sequential` or
`synchronizer` module a plain `Out` is outside — a shape rule of II.1, D-8); `Memory`
parameters (→ I.10; D-16: I.10.5 — on a `sequential` or `synchronizer` module only, I.10.6). There is no return type and at least one output. Every
payload `T` is a type of I.14 (→ I.2.6; D-19: I.14.1). The body is one tick-free prefix `s_pre`
followed by exactly one `loop`, last (→ I.9 for `s_pre`, → I.8 for the loop). (D-12: the header is S0 of II.1.1 — R8 the plain `Out`, R9 the flag, R10 the second `Clock`, R20 the rest, R21 a `tick` before the loop.)

**I.2.3 The kernel grammar.** Over locals `v`, read temporaries `t`, inputs `x`, plain
outputs `o`, registered outputs `q`, memories `m` with port index `k`, constants `c`:

```
module ::= header { s_pre ; loop { s } }
s_pre  ::= ε | s_pre ; let v = c                        c a constant of I.14, or Bits::x()  (D-19: I.14.4)
         | s_pre ; let t = read x | s_pre ; let v = e   v not a register: a wire of the first resumption (D-21 C2)
         | s_pre ; let m = Memory(init)                 init a constant array (→ I.10; D-16: I.10.3)
s      ::= ε | s ; s
         | let v = e | v = e                            e pure (→ I.14; D-19: I.14.3)
         | let t = read x                               the only form a read takes
         | write o e | write q e
         | tick                                         clk.tick().await
         | if e { s } else { s }
         | match e { p => s , … }                       first matching arm; p a pattern of I.14 (D-19: I.14.3(e))
         | loop { s } | break | continue                unlabelled; innermost loop
         | stage m k a | stage_write m k a e | observe m k   (→ I.10; D-16: I.10.1(b), (d))
e      ::= pure expressions over v, t, c and observe results (→ I.14; D-19: I.14.2–I.14.3)
```

A `read` appears only as the hoisted `let t = read x`; an input is never read inside
an expression. (D-21 C2) A pre-loop `let v = e` whose `e` is not a constant is in the kernel
when `v` is not a register — I.7.2(c)'s wire, reassigned in the loop before any read that
follows a tick, so `e` is read by the first resumption alone (D-11: the pre-loop statements are
the first resumption's); a register's initializer is a constant (I.7.4(c), R11). **REFUSED (D-21 H1, user decision 2026-11-07):** the shape I.7.2(b) with a non-constant
initializer — a pre-loop `let v = e` the loop never reassigns and reads after a tick (`preloop_held_read`:
`let x = a.read(); loop { q.write(x); tick }`) — would carry the first resumption's value into
every later one, a register whose power-on value is an input's; R11's reason refuses it in both
front-ends (`copper_analysis::check_held_preloop_values`, one message; ui/fail `held_preloop_read`;
corpus 0; before the rule the SV diverged 6/6 sequences `sim=ref≠SV`, `review/MEASUREMENTS.md` §N.4). `write o e` on a plain output appears only in a `combinational` module's
body (I.2.7, → I.11; D-14: I.11.2); in a sequential module every output is a `q` (D-8). `break` and `continue` target the innermost enclosing loop; `continue`
of the module loop is in the kernel, `break` of it is outside (I.2.5). A nested `loop`
must reach a `tick` on every path and end at one — a shape rule (→ II.1), stated here
because the grammar admits the text and the rule gives it its only meaning (→ I.8). The
tick-free counted loop of I.2.4 is unrolled by `D` and so does not appear. (D-10: the two
halves of that sentence have different principles — reaching a tick on every path is II.1's
shape rule, ending at one is II.2's one-spelling rule, I.8.2; the counted `for`'s `D` is
I.8.5's.) (D-12: R3/R4 and II.2 D1; the grammar's other productions are S0 — R12–R15, R18, R19.)

**I.2.4 The desugaring `D` and the classification.** Every construct the macro accepts
is in exactly one class: **IN** (a kernel form), **DESUGARED** (rewritten by `D` to a
kernel form, Rust meaning preserved), **ERASED** (no run-time effect on either side),
**OUTSIDE** (refused; I.2.5), or **NOT YET** (admitted by at least one front-end with no
meaning stated; I.2.8). There is no "ignored" class: a construct either has its Rust
meaning or is refused. The evaluation order of `D`'s hoisted reads is Rust's evaluation
order of the expression they were hoisted from.

| Surface construct | Class | Kernel form or note |
|---|---|---|
| `#[hardware(mode)]` | IN | header; a second attribute token (the retired opt-out flag): OUTSIDE (D-8, → II.1 R9) |
| const generics, `D: ClockDomain` generics | IN | a module instance at concrete parameters |
| one `Clock<D>` | IN | two or more on a `sequential` module: NOT YET |
| `In<T,D>`, `Out<T,D>`, `RegOut<T,D>`, `Memory` parameter | IN | `Out<T,D>` on a `sequential` or `synchronizer` module: OUTSIDE (D-8, → II.1); unit-domain `In<T>`: NOT YET (→ I.12; D-17: OUTSIDE — the unit domain is retired, I.12.1) |
| pre-loop `let v = c`, `let v = Bits::x()` | IN | → I.7, I.9 |
| pre-loop `Memory::new`, `from_contents(vec![…])`, `from_fn(closure)` | DESUGARED | the initial array (→ I.10; D-16: I.10.3); a preload computed at run time: OUTSIDE |
| `const { assert!(…) }` | ERASED | a compile-time check |
| body-local `const`, `enum`, `struct` items | DESUGARED | a constant; a value set of I.14 (D-19: I.14.1); one local per field |
| any other pre-loop statement | NOT YET | → I.9 (`review/MEASUREMENTS.md` §B) (D-11: IN — a statement of the first resumption, executed once, I.9.3; no longer NOT YET; the extraction route's re-execution and the linear route's silent drop are Phase C defects) |
| `let v = e`, `v = e` | IN | |
| `v op= e` | DESUGARED | `v = v op e` |
| `let (a, b) = e`, `(a, b) = e` | DESUGARED | one assignment per element; every projection evaluated before any assignment (simultaneous) |
| `x.read()` in any expression position | DESUGARED | `let t = read x` hoisted before the statement, in evaluation order; inside macro tokens: NOT YET |
| `o.write(e)`, `q.write(e)` | IN | `write`; `o` in a `combinational` module only (D-8) |
| `clk.tick().await` | IN | `tick`; any other `await`: OUTSIDE |
| `if`/`else`; `else if` | IN; DESUGARED | `else if` is a nested `if` |
| `match` with literal, enum, tuple, range, wildcard patterns and guards | IN | first matching arm; an or-pattern that binds: OUTSIDE |
| `matches!(e, p)` | DESUGARED | a `match` with a `bool` in each arm |
| nested `loop { … tick }` | IN | must end at a tick (→ II.1); a tick-free nested `loop`: OUTSIDE (D-10: every path reaches a tick is II.1's shape rule; ends at a tick is II.2's one-spelling rule, I.8.2) |
| `while c { … tick }` | DESUGARED | `loop { if !c { break } ; … }`; a tick-free `while`: OUTSIDE (D-10: a test false on entry runs zero iterations in zero time, I.8.4(a)) |
| `for v in a..b { … tick }` | DESUGARED | a counter register, a `loop`, the tick hoisted to the loop's end; exclusive ranges only; a range empty only at run time: NOT YET (D-10: `D` is I.8.5's `let mut c = a; let e = b; loop { if c >= e { break }; let v = c; c = c + 1; s }`, the loop ending at `s`'s tick — the hoisted tick is the transpiler's realization; a range empty only at run time runs zero iterations, I.8.4(b), no longer NOT YET) |
| `for v in a..b { … }` tick-free, constant bounds | DESUGARED | unrolled `b − a` times; a data-dependent bound: NOT YET |
| `break`/`continue` of a nested loop; `continue` of the module loop | IN | (D-10: a nested `continue` is refused by the transpiler today, a dated II.3 limitation, I.8.3; D-13: row (a)) |
| `break` of the module loop; `return` | OUTSIDE | the simulator panics at run time; a parity obligation (→ II.1) |
| labelled or valued `break` | NOT YET | no use in the corpus (D-10: → II.1 refuse in both front-ends, I.8.3; the macro admits it today) |
| `read_port::<k>().read(a)`, `write_port::<k>().write(a, e)`, `is_ready()`, `data()` | IN | `stage`, `stage_write`, `observe` (→ I.10; D-16: I.10.1(b), (d)) |
| `arr[i] = e` on an array or `Bits` local | IN | an element assignment to a local; one such write per cycle is a lowering limitation (→ II.3; D-13: row (c), the one-write-port array register) |
| struct locals, literals, field access, whole-struct assignment | DESUGARED | one local per field; `..rest`: OUTSIDE |
| helper `fn` and `impl` method calls from the body | DESUGARED | inlined (`let`s and a tail); recursion: OUTSIDE |
| `+ - * %`, bitwise, shifts, comparisons, casts, `Bits` methods | IN | pure expressions of I.14 (D-19: I.14.3(a)–(d) — every method's meaning stated, realized or row (e)); the transpiler's unsupported-method list is II.3's (D-13: row (e), enumerated) |
| `/` | NOT YET | the simulator computes it, the transpiler refuses it |
| statement macros (`println!`, `assert!` outside `const`) | NOT YET | the macro admits them with unrewritten reads; the transpiler refuses them |
| closures outside a preload; `let … else`; `if let`; `unsafe`; nested `async`; `yield` | OUTSIDE | |
| `Option`, `?`, the panic family, `unwrap`; `Clock::new`; a parameter shadowed by a local | OUTSIDE | |
(D-12: every OUTSIDE or NOT-YET row that pointed to II.1 has a row of II.1.2 — the flag R9, the two-clock module R10, the plain `Out` R8, the other `await` R1, the tick-free nested loop R4, the module-loop `break`/`return` R7, the labelled or valued `break` R12, the data-dependent tick-free `for` R5, `/` R15, statement macros and macro-token reads R13/R14, `Clock::new` R17, the shadowed parameter R18; `Option`, `?`, the panic family and `unwrap` are II.2's D3.)

**I.2.5 Outside the kernel, and why.** A construct is outside the kernel for one of four
reasons, each a clause of Part I. (a) **No cycle to fail in:** `Option`, `?`, the panic
family and `unwrap` describe a computation that may not produce a value, and every
instant of I.1 produces one. (b) **Time is received, never made:** a module may clone
or pass the clock it was given but never construct one (`Clock::new`), and the only
suspension is the cycle boundary (`tick`), so every other `await`, nested `async` and
`yield` are outside. (c) **No unbounded work inside an instant:** a tick-free nested
`loop` or `while`, recursion, and a tick-free `for` with a data-dependent bound would
iterate within one instant without a bound the grammar can see; the constant-bound
tick-free `for` is the one bounded form and is unrolled. (d) **No control that leaves
the cycle structure:** `break` of the module loop and `return` end the module, which has
no meaning after I.1's infinite edge sequence. The remaining outside forms (closures
outside a preload, `let … else`, `if let`, an or-pattern that binds, `..rest`, `unsafe`,
shadowing a parameter) have no corpus use and no hardware reading the kernel would need;
they are refused by one front-end or both, and where only one refuses, II.1 carries the
parity obligation. (D-12: II.1.3, with every break measured; I.2.5(a)'s family is II.2 D3, (b) S1/S2 and R17, (c) S3, (d) S4.)

**I.2.6 Types.** Every port payload, local, constant and expression has a type of
I.14 (D-19: I.14.1); I.2 states no type grammar of its own. The admissible-type rule of the macro
(`check_admissible`: port payloads and annotated `let`s) is an admissibility rule of
II.1, not a clause of meaning, and II.1 carries the obligation that the transpiler run
it too. (D-12: R16, with the obligation and its direction in II.2.2.)

**I.2.7 The four modes.** There is one kernel and three instantiations of it.
A **`sequential`** module is the kernel of I.2.3 as written. A **`synchronizer`** is the
kernel with the single-domain clause of I.2.2 replaced by I.12's: its ports may lie in
different domains and the crossing has the meaning I.12 gives it (D-17: I.12.3 — one `Clock<D_dst>`, foreign `In` ports, the two-flop rule; I.12.4 what may cross). A **`combinational`**
module is the tick-free fragment of `s` — no `tick`, `loop`, `break`, `continue` or
memory staging; every path writes every output — evaluated afresh at every instant of
I.1 (→ I.11; D-14: I.11.1); the simulator's `loop { body; delta_yield().await }` wrapper is an
implementation form of that evaluation, not a kernel loop. A **`structural`** module is
not an instance of this kernel: its body is a wiring language — `wire::<T, D>(init)`,
child instantiation and port connection — and I.13 gives it meaning (D-18: I.13.4 — a name for a design of I.13.1; nets and instantiations only); it is
transpile-only and the simulator wires the hierarchy by hand.

**I.2.8 Carried NOT-YET list.** Each row below is admitted by at least one front-end
today, has no meaning in this document, and is owed to the section named; until that
section decides, it is outside the agreement relation (→ III.2; D-20: III.2.8).

- a `sequential` module with two or more `Clock` parameters (the CDC check is skipped;
  → I.12); (D-17: closed — refused by decision, II.1.2 R10, I.12.5;)
- `In<T>` with the unit domain (bypasses the domain check; → I.12); (D-17: closed — OUTSIDE, the unit domain retired, I.12.1;)
- any pre-loop statement that is not a `let` (runs once in the simulator and every
  iteration under control extraction, measured divergent, unrefused; → I.9); (D-11: closed —
  IN with its Rust meaning, I.9.3; the extraction route's re-execution and the linear route's
  silent drop, measured 2026-11-05, are Phase C defects, not NOT-YET rows;)
- an input read inside macro tokens (neither classified nor rewritten; → II.1, refuse);
- a tick-bearing `for` over a range empty only at run time (`D` is undefined: the
  counter form is derived for a non-empty range and a literal-empty range is dropped;
  → II.1, refuse, RULES open 5); (D-10: closed — `D` is defined in I.8.5 and the empty range
  runs zero iterations, I.8.4(b); the transpiler's extra cycle is a Phase C defect, measured,
  not a NOT-YET; the literal-empty range counted as a boundary by the analysis stays with
  II.1, I.8.7);
- a tick-free `for` with a data-dependent bound (→ II.1, refuse);
- a labelled or valued `break` (→ II.1, refuse); (D-10: confirmed, I.8.3; the macro admits
  it today, the transpiler refuses it);
- `/` (→ I.14 for its meaning on `Bits`, or II.1 to refuse); (D-19: closed — I.14.3(b): truncating unsigned division, X on an X operand or a zero divisor; refused in both front-ends as a dated II.3 limitation until the transpiler lowers it;)
- a statement macro outside `const` (→ II.1, refuse).

Parity obligations the classification exposes, owed to II.1: `break` of the module
loop and `return`, `/`, statement macros, the two-clock sequential module, reads inside
macro tokens, and `check_admissible` — each refused by one front-end and admitted by
the other, against the rule that a module compiles iff it transpiles. (D-10 adds three,
measured 2026-11-04: a `continue` of a nested loop and a labelled `break`, both admitted by
the macro and refused by the transpiler, I.8.3; and a hex literal in a `from_lit` turbofish,
`Bits::from_lit::<0xFF>()`, refused by the transpiler, I.8.7.) (D-12: closed into II.1.3's measured list — the macro owes R5, R7, R8, R9, R10, R12, R13/R14, R15, the transpiler R16, R17, R18 and D3, and the hex literal is a transpiler defect; the two-clock module, `/` and the data-dependent `for` are refused by decision, II.1.2.)

### I.3 Segments, the trailing region and the residue

Rewritten 2026-11-05 under D-8 (D-15); the two-job text is in the decision log's D-1…D-7
entries and the packet-15 snapshot.

Decided: D-3 (2026-11-04); read under D-8 (2026-11-04): a segment is one resumption.
Anchor: for the single-tick body, `sync_2ff` vs `examples/cdc/sv/sync_2ff_ref.sv`
(`tests/cdc_synchronizer_anchor.rs`: one destination cycle of latency — the trailing
region and the next head are one resumption, the trailing region first; `ff1` a register
and `q` a `RegOut`) and the tick-first BaseJump anchors `bsg_counter_up_down` and
`bsg_dff_en`; the head segment, `det_010` / `det_010_awaits` vs `pattern_detector_010.sv`,
`lfsr`, `shift_register`, `sipo_block` (the `RegOut` spellings measured in
`review/Q1_TRADEOFFS.md` §4). Sim ≡ SV only: the trailing region of a body with more than
one tick, the trailing-before-head order observed through a local, the residue as `pc`
(`review/MEASUREMENTS.md` §C debts 2, 3, 12; carried in I.3.9). The vocabulary, the
placement of "two iterations suffice" and cycle 0 are decided.

In coroutine terms (D-7, D-8): a segment is the statements of one resumption; the residue
is the tick suspension site; the trailing region is the opening part of the next
resumption (→ I.0.1, I.0.2).

**I.3.1 Executions.** An **execution** of a module instance under an input stream
(I.1.5) is the sequence of kernel statements the function performs with the Rust meaning
I.2 gives it, from the first pre-loop statement on: an `if` or `match` performs the arm
its test selects, a nested `loop` repeats its body until a `break`, a `continue`
re-enters its loop's head, and the module loop re-enters its head when its body ends
(→ I.8). Every statement of an execution other than `tick` performs its effect at an
instant of I.1.2; a `tick` is a cycle boundary and performs nothing.

**I.3.2 Segments.** A **segment** is a maximal `tick`-free run of an execution: the
statements performed between one `tick` and the next. When the module loop's body ends
between two ticks, the statements after the last `tick` of iteration k and the
statements of iteration k+1 up to its first `tick` are **one** segment — re-entering the
module loop costs no time, and neither does a `break` or `continue` of a nested loop or a
`continue` of the module loop: each selects the next statement, never the next cycle. A
nested loop that runs zero times contributes no boundary (→ I.8). Segmentation is a
function of the execution, hence of the kernel and the input stream; it is not a textual
partition of the loop body, and a statement that lies in several executions' segments
has no segment of its own. **Cycle N's segment** (N ≥ 1) is the segment whose opening
boundary is edge N; it runs within `[edge N, edge N+1)` and its closing boundary is edge
N+1. Cycle 0's segment is I.3.7's.

**I.3.3 The residue.** The state a module carries across edge N is: (i) the **residue**,
the continuation of the execution at the `tick` that is edge N — identified by that
tick's site in the kernel, so a module has finitely many residues, one per `tick` site
(D-7, D-8: one suspension kind; the tick sites are the suspension points, T12's FSM
states); (ii) the values of its registers (→ I.7); (iii) the committed values of its
`RegOut` ports (→ I.6); (iv) its memories (→ I.10; D-16: I.10.1(a)).
Cycle N runs the residue from edge N through cycle N's segment to whichever `tick` the
segment reaches; that tick is edge N+1 and its site is the next residue. A `break` or
`continue` is a statement of the segment: it changes which tick the segment reaches, not
when. The implementations' state encodings — rustc's coroutine state, the transpiler's
`phase_r` and `pc` registers and the rotated nested-loop state — are implementation
forms of the residue (I.2.1) and carry no meaning of their own.

**I.3.4 Instants.** Within a segment, statements execute in **program order with
forwarding**: a read of a local yields the last value assigned to it earlier in the
segment, else the value carried across the segment's opening edge (→ I.7); a read of an
input yields the input's value at the instant of the read (→ I.4); a write to a port
takes effect as I.6 says. Every statement of cycle N's segment executes at I.1.2's one
instant, `t_{N+1}⁻`, in the resumption that ends at edge N+1 (D-8, reversing D-3 A1's
per-execution instant).

**I.3.5 The trailing region.** In `loop { A ; tick ; B ; tick ; C }` the statements an
iteration performs after its last `tick`, up to the end of the loop body, are the
**trailing region** of the segment that contains them; by I.3.2 that segment continues
with the next iteration's `A`. What follows is I.3's; the rest is other sections'.
(a) The trailing region is the opening part of the resumption that begins at the tick it
follows (edge N) and ends at the next tick (edge N+1): it executes at `t_{N+1}⁻` with the
rest of its segment, and a read inside it sees `x_{N+1}` like every other read (I.4.1).
It names a position in the text, not a region with an instant of its own.
(b) The trailing region **precedes `A` in program order** within one segment: `A` sees
the region's assignments by forwarding, and a port written in both takes `A`'s write
(last write wins, I.6.2). This order is a definition. It is observable: through a local
assigned in the trailing region and read in `A` (`trailing_local_into_head`, sim ≡ SV),
through the synchronizer's latency (anchored), and through a `RegOut` written in both the
trailing region and `A` (I.6.2: the last write before the edge).
(c) A value assigned in the trailing region and read in the next `A` is read in the same
resumption it was assigned in; it needs no carrier across an edge. `sync_2ff`'s `ff2` is
such a value — a wire of one resumption, dead at the `tick`; the flop `sync_2ff_ref.sv`
has for it is the `RegOut` `q` (D-6; → I.7 for the register set). "Commit edge" is not a
Part I word: a trailing `RegOut` write is a write of the resumption that ends at edge N+1
and takes effect there (I.6.1).
(d) A **single-tick body** has one segment per cycle like any other: in `loop { A ;
tick ; C }` the segment is `C` then `A`, one resumption; in `loop { tick ; C }` the
segment is `C` alone. A single-tick and a multi-tick body differ in no clause of meaning:
the distinction (`crosses_more_than_one_tick`) was the gate of two rules retired by D-8.

**I.3.6 The back edge.** A segment crosses the module loop's back edge **at most once**:
the head's statements reach a `tick` on every path (reachability, → II.1), so the segment
that contains iteration k's trailing region ends at or before iteration k+1's first
tick. Hence every obligation about the trailing region concerns one pair — the trailing
region of iteration k and the head of iteration k+1 — and the proposition that every
Part II verdict is decided on `body ; body` ("two iterations suffice", [anvil Lemma
C.19]; FORMAL_SEMANTICS P6/G6) is III.1's, with this clause as its premise (→ III.1; D-20: P9 retired as vacuous, the lemma stays). (D-12: R2.)

**I.3.7 Cycle 0.** The first segment of an execution is the pre-loop statements followed
by iteration 1's head up to its first `tick`. It executes at `t_1⁻` (I.1.2), with no
trailing region before it: every read sees `x_1` (I.1.5: a resumption's reads see the
input of the edge it ends at), and the residue at edge 1 is the first `tick` reached.
What a pre-loop statement means, and `obs_0`, are I.9's (→ I.9). (D-11: I.9.3 and I.9.4.)
The first segment is the one segment of the execution not preceded by a trailing region
(otherwise like every other); every later head segment is preceded in its cycle by the
previous iteration's trailing region.

**I.3.8 Implementation map.** Simulator: Rust's own control flow performs the execution;
nothing is injected at the loop's end. Transpiler, linear route: the loop body is split
textually at ticks (`shir_lower::split_at_ticks`, N ticks → N+1 **textual segments**, an
implementation form); the trailing textual segment's combinational statements are placed
before the head's in phase 0 and its register updates in the last phase with a shared
forwarding map — an encoding: under one resumption the trailing segment and the head are
one phase, every value forwarded (→ III.2; D-20: an encoding, P8). Extraction route: a trailing tick is
`pc = head` with no extra state, the module loop's fall-through and `continue` are
zero-time gotos, a nested `break` inlines its continuation in the same cycle, a nested
loop's head state holds its body rotated around its tick (`control_extract.rs`).
Today (two-job implementation, Phase C rewrites): a tick resolves at `t⁺` and the
coroutine runs until a deferred read parks it at `t⁻` (I.1.6); in Phase C the coroutine
runs the trailing region, the back edge and the next head in one resumption at `t⁻`
(I.0.2). Analysis: three static regions — `pre_tick_region` (`Comb`-reachable from `H`),
the trailing rules' region (tick-free-reaches `H`), the Comb-component (`comb_phases`, in
which the trailing statements merge with the head) — each an approximation of I.3.2. Two
recorded approximations: every `Comb` edge into a tick node is unioned, so two segments
ending at one tick are one component (`branch_merge`: two tick sites, two extracted
states, one phase, measured by `derivation-audit`); a head statement on a path to a
module-loop `continue` tick-free-reaches `H` and counts as trailing for the two trailing
rules. The two trailing rules and the consumers of `pre_tick_region` are retired (D-8),
and the approximations recorded here matter only to the transpiler's phase encoding
(→ III.2; D-20: P8). (D-12: II.2.2 names `pre_tick_region` and `comb_phases` among the retired predicates; no direction is owed.)

**I.3.9 Carried NOT-YET list.**
- The trailing region of a body with more than one tick, the trailing-before-head order
  through a port, and the residue as `pc` are agreed sim ≡ SV only; an independent
  reference for one such body is owed to III.2 (D-20: III.2.11 item 1) (`ANCHORING_A_MODULE.md`; the user writes
  references).
- A pre-loop statement that is not a `let` (I.2.8; `review/MEASUREMENTS.md` §B — run
  once by the simulator, re-run at every return to state 0 by the extraction route,
  divergent and unrefused; → I.9). (D-11: settled — I.9.3; the linear route drops it,
  measured 2026-11-05; both Phase C defects.)
- A tick-bearing `for` over a range empty only at run time (I.2.8; `D` undefined; the
  zero-iteration meaning → I.8, the refusal → II.1). (D-10: settled — I.8.4(b), I.8.5; zero
  iterations in zero time, no refusal; the transpiler's extra cycle a Phase C defect.) (D-12: II.2.2 states R3's literal-empty under-admission.)

### I.4 Reading an input

Rewritten 2026-11-05 under D-8 (D-15); the two-job text is in the decision log's D-1…D-7
entries and the packet-15 snapshot.

Decided: D-4 (2026-11-04); its read classes reversed by D-8 (2026-11-04) — a read is by
position at the one instant. Anchor: a read before a tick feeding a `RegOut` samples the
value present before that edge — `det_010` / `det_010_awaits` vs `pattern_detector_010.sv`,
`lfsr`, `shift_register`, `sipo_block` (BaseJump; also a read between two ticks), and a
read in a test whose arm ticks — `det_010_awaits`'s inner test; a read after the last
tick, measured in the `RegOut` spelling with the read before the tick —
`bsg_counter_up_down`, `bsg_dff_en` (BaseJump, identical SV; `review/Q1_TRADEOFFS.md` §4
(b), (c)), `sync_2ff` vs `sync_2ff_ref.sv`. Sim ≡ SV only: the second consumer of a read
that feeds two ports (`sipo_block`'s `wN_dbg`, a `RegOut` since D-8). A passthrough is a
`combinational` module's (I.4.5, → I.11.5). Decided: cycle 0.

In coroutine terms (D-7, D-8): a read executes in the one resumption of its cycle and
yields the input present before the edge that resumption ends at (→ I.0.2).

**I.4.1 The value.** A read `let t = read x` executed at an instant yields the settled value
of `x`'s wire at that instant (I.1.3). For an input the environment drives that is
`x_{N+1}` for every read of the resumption that ends at edge N+1 (I.1.5): the value present
before the edge, wherever the read stands in the segment — before the tick, after the
previous one, in a guard, in a nested loop. For a wire another module drives it is that
module's value at the instant, settled — after the producer's statements at that instant
(→ I.6 for when a `RegOut`'s write is on the wire, I.11 for a combinational wire; → I.13 (D-18: I.13.1–I.13.2)
for the settle and the composition). A read has **one value**: a result that feeds a
register and a `RegOut` alike carries the same sample to both, so a read is never "by
consumer" — the instant, not the use, decides (CDS §8-1's dual-consumer refinement is not a
case; `sipo_block`'s `w0..w3`). One value per signal per instant is every surveyed
language's rule [esterel §3.5; clash eq. 11; anvil §4.5], and Copper's (D-8): a sequential
body has one instant to read at [prost §3].

**I.4.2 One class of read.** Every read site of a sequential module is alike: the value a
read yields is fixed by the resumption it executes in (I.4.1), not by its position in the
kernel — before the tick, after the previous one, in the test of an `if` or `match` whose
arm ticks, in the test of a nested loop, on a path to a `break` or `continue`. The value a
test read gets cannot depend on the arm its own value selects, and the FSM the test steers
samples it at the edge whichever arm it takes (`det_010_awaits`, anchored). No read site is
classified (D-8, reversing D-4 A1). Prost reads every input in the edge-evaluated step
[prost §3]; that is Copper's rule.

**I.4.3 Trailing reads.** A read after an iteration's last tick is a read of the
resumption that ends at the next edge (I.3.5(a)) and yields `x_{N+1}` like every read
(I.4.1); a trailing read that feeds a `RegOut` yields the sample the flop itself takes at
that edge — measured in the `RegOut` spelling with the read before the tick, identical SV
(`bsg_counter_up_down`, `bsg_dff_en`, `review/Q1_TRADEOFFS.md` §4 (b), (c); `sync_2ff`).
The body of a nested loop ends at a tick (→ II.1), so it has no trailing region of its
own. (D-12: II.2 D1.)

**I.4.4 Cycle 0.** The first segment executes at `t_1⁻` (I.3.7); every read in it yields
`x_1` (as in every cycle). A consequence of I.4.1, not a clause; `obs_0` is I.9's. (D-11: I.9.4 — `obs_0` shows no
read: `init(q)`.)

**I.4.5 The passthrough.** A sequential module has no plain `Out` (I.2.2), so a read has
no wire to pass through to. A passthrough is a `combinational` module — the same settle
(→ I.11; D-14: I.11.5) — or a `RegOut` written from the read, one edge later (I.6.1).
D-4 A2-i's refused shape and its sanctioned spelling are vacuous (D-8). (D-12: vacuous, II.2.1.)

**I.4.6 No static approximation.** A read's value is a function of the resumption it
executes in and of nothing else, so no static predicate approximates it; the predicates
with which the two-job rules and the transpiler asked "does a read reach this drive" are
retired (D-8; D-12: II.2.2 names them).

**I.4.7 Implementation map.** Simulator: a read clones the live cell
(`copper-core/src/port.rs:120`) in the pre-edge settle, after the environment's drive and
after its producers (`copper-sim/src/executor.rs:545`, the levelized order); the first
poll of every future is the first pre-edge settle, which is I.4.4. Today (two-job
implementation, Phase C rewrites): `classify_reads` (`copper-analysis/src/cfg.rs:3804`)
tags each site deferred or immediate by D-4 A1, with two departures — a `match` guard is
tagged by the match's continuation alone (`:3994`; no corpus instance), and
`passthrough_read` (`:3880`) tags the direct argument of a plain-`Out` write in a
register-free module immediate, which, being keyed on the surface spelling, gives the two
spellings of one kernel read different traces (D-4, measured on the D2 chain);
`ReadRewriter` (`copper-macros/src/lib.rs:686`) wraps a deferred site in
`pre_edge_barrier` (`copper-sim/src/lib.rs:101`: ready in the pre-edge settle, pending
after the edge), matched by index with a release-mode fallback to a plain read; the
per-read "freshness guard" is that barrier and nothing else; a parked site resumes in the
next pre-edge settle; the transpiler's static walk makes a deferred test read the FSM's
next-state input. Phase C removes `classify_reads`, `passthrough_read`, `ReadRewriter`'s
injection and `pre_edge_barrier` (D-8); the rewrites of `event_sink`, `event_source` and
the `slow_consumer` examples are in D-8's corpus list. Obligations that outlive them: one
authority for "reads an input" (the CFG counts identifiers inside macro tokens as uses,
`cfg.rs:2945`, while the classifier skips the statement, `:3922` — resolved by II.1
refusing the construct); remove the stale doc comment (`copper-macros/src/lib.rs:731`).

**I.4.8 Carried NOT-YET list.**
- A read inside macro tokens (I.2.8; → II.1, refuse). (D-12: R14.)
- The value a read sees on a wire a `combinational` module drives in the same settle:
  I.4.1 gives it (the settled value), measured sim ≡ SV only (→ I.13, III.2 (D-20: III.2.7); D-18: I.13.2 — the settled value is the network's unique fixpoint). (D-14: I.11.4 — a
  `combinational` module reads a `RegOut` at its committed value; the Mealy composition anchored,
  I.11.5.)

### I.5 Writing a plain output; observation and hold

Rewritten 2026-11-05 under D-8 (D-15) as a forwarding stub; the two-job text is in the
decision log's D-5 entry and the packet-15 snapshot.

Decided: D-5 (2026-11-04); reversed as a whole by D-8 (2026-11-04). This section was the
plain-output section of the two-job body: a plain `Out` written inside a clocked body, its
observation at either of two instants, its hold, its five emission forms and the frame for
the eight alignment rules. Under D-8 a plain `Out` is not a parameter of a sequential
module (I.2.2); every output of a sequential module is a `RegOut` and I.6 is the output
rule; the wire's rules are the combinational module's (I.11.2, D-14). The section keeps
its number so that every citation of an I.5 clause resolves; the table says where each
former clause's surviving content lives.

| Former clause | Surviving content |
|---|---|
| I.5.1 The write | the wire's write: I.11.2(a); the `RegOut` write: I.6.1 |
| I.5.2 Observation (two instants) | retired — one observation per cycle (I.1.4); a `RegOut` write is shown from `obs_{N+1}` (I.6.4) |
| I.5.3 Several writes | last write wins on the wire: I.11.2(b); for a `RegOut`: I.6.2 |
| I.5.4 Hold | hold is the `RegOut`'s: I.6.3; a combinational wire does not hold: I.11.2(d) |
| I.5.5 The unobserved write | vacuous for a sequential module — no `RegOut` write is in no observation (I.6.4; II.2.1 D4); the wire's case is admitted, last write wins (I.11.2(c), D-14) |
| I.5.6 The initial value | the wire is created at the zero of its payload type, a III.2 precondition (D-20: III.2.3), and no observation shows it: I.11.2(e); `init(q)`: I.6.6 |
| I.5.7 Three kinds | `Out` is a `combinational` parameter only (I.2.2, I.11.1); `RegOut` is the register port (I.6.1); definite assignment is the wire's rule (I.11.3) |
| I.5.8 The frame for Part II | retired — the rules are sorted in II.2.1 (D-12) and II.3.2 (D-13), none left for any mode |
| I.5.9 Implementation map (the five forms) | retired — the forms Phase C removes are listed in D-8's "Code does" and in I.0.7's obligations; its predicates are retired, II.2.2 (D-12) |
| I.5.10 Carried NOT-YET list | closed — the startup items are vacuous (I.9.4, D-11; I.11.2(e), D-14); the dead-write and hold-form items are vacuous (II.2.1, D-12; II.3.2, D-13); the consumer's read of a wire → I.11.4, I.13 (D-18: I.13.2 — closed); a write of X → I.14 (D-19: I.14.2(e) — admitted, the port is X) |

### I.6 Registered outputs

Rewritten 2026-11-05 under D-8 (D-15); the two-job text is in the decision log's D-1…D-7
entries and the packet-15 snapshot.

Decided: D-6 (2026-11-04), the output rule since D-8 (2026-11-04): every output of a
sequential module is a `RegOut`. Anchor: a write before a tick, from forwarded locals the
last of which is an input read, is shown from the next observation and the port holds
across the cycles no write reaches — `sipo_block` vs BaseJump `sipo_block.sv`; a write
before a tick as a one-cycle pulse, cleared by the next iteration's write —
`det_010_awaits` vs `pattern_detector_010.sv`; a write at the head of the resumption,
before any read, shown from the next observation — `handshake` vs the `REFERENCE`
`handshake.sv` (hand-written by the module's author: a second spelling, not a third
party); the `RegOut` spellings of `bsg_dff_en`, `bsg_counter_up_down`, `det_010` and
`sipo_block`'s `wN_dbg`, each identical to its reference (`review/Q1_TRADEOFFS.md` §4).
Sim ≡ SV only: the trailing position (`regout_trailing_single_tick`, `rx`'s `rx_dv`;
`review/MEASUREMENTS.md` §C debt 3), forwarding into a write before a tick
(`tests/regout_forwarding_equivalence.rs`), several writes before one edge by value, the
pre-loop write (§B); carried in I.6.10. Decided: the initial value and cycle 0.

In coroutine terms (D-7, D-8): a `RegOut` write is the resumption's effect on the pending
value; the edge the resumption ends at commits it (→ I.0.2).

**I.6.1 The write.** `write q e`, executed at cycle N's one instant `t_{N+1}⁻` (the
trailing region included, I.3.5(a)), sets `q`'s **pending value** to `⟦e⟧`, the value
of `e` with forwarding at that instant (I.3.4: a local as last assigned earlier in the
segment, else as carried across the opening edge; an input as I.4.1). At **edge N+1** the
pending value, if there is one, becomes `q`'s value and is cleared. A `RegOut` is therefore
the register port of I.1.3: its value changes only at an edge. "The first edge after the
write executes" (RO1) and "the next edge" (RO3) are one statement: the trailing region
executes in the resumption that ends at edge N+1 (I.3.5(a)), so its next edge is
N+1. The statement that a trailing region's writes take effect at the edge that
opens its cycle (S7, DT §1; contradiction 2) named an edge before the write executes and is
reversed. FORMAL_SEMANTICS §3.4's `Q_{N+1}(q) = Π_N(q) if q ∈ dom Π_N, else Q_N(q)` is this
clause. The nonblocking assignment's rule — the right-hand side evaluated when the statement
executes, the left-hand side updated at the end of the step [cummings §4.0] — and
Filament's `Register: in@[G,G+1) → out@[G+1,L)` [filament §2.3] say the same in their own
time models; so do Chisel's "updated at the end of the current clock cycle" [chisel §2.6]
and Anvil's `e_c ▷ #1` [anvil §4.5].

**I.6.2 Several writes.** Writes of one `RegOut` before one edge take effect in program order
(I.3.4): the pending value at edge N+1 is that of the **last** write executed in cycle N
(one instant, I.1.2), whether it lies in the trailing region or in the
head of the next iteration (one segment, I.3.2; the trailing region first, I.3.5(b)). Gap 19
is closed. The earlier writes are not dead writes: they set a pending value that a later
write replaces before any edge, and no observation samples a pending value (II.2.1 D4).
"Last nonblocking assignment wins" [cummings §15.3] is the same rule; Chisel's
last-connect [chisel §2.6] shares the form without forwarding; Prost's and RustHDL's `.next`
[prost §3; rhdl §3] are it inside one instant; Bluespec forbids the second write [bluespec
§5]. Anchored in form only: the corpus's one such pair (`det_010_awaits`, the loop-top
default and the reset arm) writes the same value twice; by value the rule is sim ≡ SV only
(I.6.10).

**I.6.3 Hold.** A `RegOut` that no write of cycle N reaches keeps its value through edge N+1:
there is no pending value, so nothing changes at the edge. A corollary of I.6.1, and
position-blind; a combinational wire does not hold (I.11.2(d)). Anchored: `sipo_block`
holds `data_o` across the three cycles of every four that do not write it. Chisel's
deasserted load enable [chisel Fig. 1] is its hardware form.

**I.6.4 Observation.** `obs_N(q)`, N ≥ 1, is `q`'s value after edge N (I.1.4): the pending
value set in cycle N−1 if there was one, else `obs_{N−1}(q)`. Hence a write executed in
cycle N (at `t_{N+1}⁻`) is **shown from `obs_{N+1}`**. `obs_0(q)` is `init(q)`
(I.6.6): no edge precedes it. A write in cycle N followed by one in cycle N+1 lands in
`obs_{N+1}` and `obs_{N+2}` respectively: no `RegOut` write is in no observation (there is
no dead `RegOut` write; II.2.1 D4). FORMAL_SEMANTICS §3.4 (a write in cycle N−1 is
observed from N on) is this clause in its numbering.

**I.6.5 Cycle 0.** A write in the first segment — the pre-loop statements and iteration 1's
head up to its first `tick` (I.3.7) — executes at `t_1⁻` and takes effect at edge 1: it is
shown from `obs_1`. A pre-loop `RegOut` write is a write of the first segment and nothing
more; what the extraction route does with a pre-loop statement is I.9's
(`review/MEASUREMENTS.md` §B, measured on a `RegOut`; → I.9). (D-11: I.9.3 — the Rust
meaning stands; the extraction route's re-execution and the linear route's silent drop are
Phase C defects.)

**I.6.6 The initial value.** `init(q)`, `q`'s value before edge 1, is the **zero of its
payload type** (→ I.14; D-19: I.14.1(e) — the table's zero column), the zero a combinational wire is created at (I.11.2(e)). Decided:
it is what the SV powers the port on at (PO6), what the sweep's testbench and every example
create a registered wire at, and `obs_0` is the only observation that shows it. The
environment creates every registered wire at `init(q)` — a precondition of the agreement
relation (→ III.2; D-20: III.2.3) that closes the recorded power-on mismatch (`review/INVENTORY_CODE.md`
unstated 8; `review/MEASUREMENTS.md` debt 6) by precondition, not by a simulator change. A
module whose first write of `q` executes after `obs_0` shows `init(q)` there (→ I.9 for
`obs_0` observed).

**I.6.7 An input read after the tick.** `tick; q.write(x.read())` means: `q` takes
`x_{N+1}` at edge N+1 — the read is in the resumption that ends at edge N+1 (I.4.1), and
that is the flop's own sample; `tick; if x.read() == c { q.write(v) }` fires iff
`x_{N+1} == c`. Both agree with the hardware with no carrier — measured in the `RegOut`
spelling with the read before the tick (`bsg_counter_up_down`, `bsg_dff_en`, identical
SV; `sync_2ff`). D-6 A4-i's lowering limitation (a carrier register loading the input at
the opening edge), its rule `trailing_regout_reads_input` and the rule's guard clause
(measured on `claude/trailing-regout-guard-67cf37`, `e19a`–`e19j`) are retired (D-8);
gap 7 and RO8 are closed by this clause (D-13: the II.3 row is vacuous under D-8, II.3.2
rule 11). Prost reads every input at the edge and so has no such case [prost §3]; no
precedent.

**I.6.8 Composition and one driver.** A `RegOut`'s wire changes only at its clock's edge
(I.1.3): a consumer that reads it in cycle N reads the value `obs_N(q)` shows (one
instant, I.1.2), and the settle of I.13 (D-18: I.13.2(b)) treats it as a source, never as a combinational
dependency (RO6, M15; → I.13 for the value another module reads at each instant — D-18: I.13.2(b), the committed value, a source; → I.12
for a reader in another domain; D-17: I.12.3 — the last value `D_src` committed before the reader's instant). A `RegOut` has one driver — the handle is not `Clone`
(W6; → II.1 as a shape rule). A memory result written to a `RegOut` (`q.write(m.data())`)
is a write of this clause at the instant of the observation; what the observation yields
is I.10's (M9; → I.10; D-16: I.10.1(d) — the word the last edge delivered). The order in which the simulator's clock calls its listeners
(`review/INVENTORY_CODE.md` unstated 22) is unobservable for a `RegOut`, whose pending value
is a copy, and the dirty flag it sets at the edge (unstated 25) is a scheduler artifact
(→ I.13; D-18: I.13.1 — one driver per wire, the type system's). (D-12: R22.)

**I.6.9 Implementation map.** Simulator: `RegOut::write` stores `Some(v)` into `pending`
(`copper-core/src/port.rs:194-196`), invisible to readers and setting no dirty flag;
`on_posedge` takes it into the committed cell (`:173-183`), dirty only on change, hold when
`None`; the listener is registered at `registered_wire(clock, initial)` (`:220-235`) and
called by `Clock::advance` between the two settles (`copper-core/src/types.rs:886-900`;
`copper-sim/src/executor.rs:813-853`); `init(q)` is the environment's argument, zero in the
sweep (`build.rs:934`) and in every example — the III.2 precondition of I.6.6 (D-20: III.2.3). Transpiler:
every `RegOut` is a hold output (`copper-codegen/src/vlir_lower.rs:329-333`, `:402-405`),
never a continuous assign (`:716-734`, `:1573-1586`); its drives become `q <= edge_value`
in `always_ff` under the phase or `pc` guard (`split_output_regs` `:519-531`,
`:2107-2135`), powered on at zero through `port_inits` (`emit.rs:418-432`); `edge_value`
is the forwarded value (`shir_lower.rs:374-386`). Today (two-job implementation, Phase C
rewrites): a `RegOut` is never the mux + hold form of a plain `Out`; for a drive in a
trailing textual segment of the linear routes `edge_value` is the committed register
(`:363-367`, `trailing_reg_outs` `:205-215`) — those routes realize the trailing region's
register updates at edge N (→ I.7), so the flop already holds the forwarded value when the
drive is sampled at edge N+1, an encoding of I.6.1, not a second rule; on the extraction
route the trailing region is a `pc` arm whose drives and updates share the edge;
`trailing_regout_reads_input` (`copper-analysis/src/cfg.rs:1214-1236`; value sources only
in `paper`, the guard on the unmerged branch) refuses the input read after the tick of
I.6.7. Obligations carried to Phase C (D-8): none to the simulator; `trailing_reg_outs`
and the committed-value drive go with the trailing fold — one phase per resumption, every
drive's value `edge_value` (whether the linear route keeps the fold as an encoding is
III's); `trailing_regout_reads_input` is retired (I.6.7); `split_output_regs` is the one
port form of every sequential module; the three II.3 rows D-6 A5-i recorded — the carrier
flop for the input read of I.6.7, the carrier `let` of a single-tick body emitted as a
wire (`e19e`, `e19l`: the flop at edge N+2 reads the next iteration's read), a trailing
drive's guard forwarded while its value is committed (`e19k`), each dated 2026-11-03 with
the branch's measurement — are vacuous (I.6.10) and are re-measured after Phase C;
`review/MEASUREMENTS.md` §C's `RegOut` row corrected (`sipo_block`, `handshake`;
`bsg_counter_up_down` and `bsg_dff_en` declare plain `Out`).

**I.6.10 Carried NOT-YET list.**
- The trailing position (a write after the last tick, at `t_{N+1}⁻`; `regout_trailing_single_tick`,
  `rx`'s `rx_dv`), forwarding into a write before a tick (`regout_forwarding_equivalence.rs`)
  and several writes before one edge by value are sim ≡ SV only: an independent reference
  for one trailing `RegOut` write and for one port written twice with distinct values before
  one edge is owed to III.2 (D-20: III.2.11 item 2) (`ANCHORING_A_MODULE.md`; the user writes references). The
  `handshake` reference is by the module's author.
- A pre-loop `RegOut` write under the extraction route (`review/MEASUREMENTS.md` §B; corpus:
  the pipelined CPU's `program_counter`, `halted`, `a0_out`, masked by its halt path) → I.9.
  (D-11: I.9.3; masked by the head's every-path writes, probe (ii); the linear route drops the
  transpilable CPU's pre-loop staging, masked by its reset.)
- `obs_0(q) = init(q)` observed, and the `initial q = 0` form at startup → I.9, with `obs_0`
  observed (D-1 A1). (D-11: I.9.4 — measured equal on both sides in both power-on builds;
  the harness observes `obs_0` in Phase C.)
- The II.3 rows D-6 A5-i recorded (I.6.9) are vacuous (D-8): the carrier flop with I.6.7;
  the single-tick carrier `let` (`e19e`, `e19l`) is a register of I.7 (live at the `tick`);
  the forwarded trailing guard (`e19k`) is the one forwarded phase of the resumption. Each
  to be re-measured after Phase C. (D-13: recorded closed in II.3.2.)
- The value a consumer in another domain reads from a `RegOut` wire → I.12 (D-17: I.12.3 — closed); the settle's
  treatment of a `RegOut` as a source → I.13. (D-18: I.13.2(b) — closed.)
- `q.write(m.data())` → I.10 (M9; D-16: I.10.1(d) — closed); a write of `Bits::x()` or `Logic::X` to a `RegOut` → I.14. (D-19: I.14.2(e) — admitted: the port shows X from the observation the write reaches, outside the agreement relation by I.14.2(f) — closed.)

### I.7 Registers

Decided: D-9 (2026-11-04). Anchor: the register set against independent hand-written SV
— `mac_fsm` (name-exact), `det_010`, `det_110101`, `lfsr` (by storage count), through
`assert_source_registers_match_reference_sv` in their equivalence tests; `sync_2ff` vs
`examples/cdc/sv/sync_2ff_ref.sv` in its one-job spelling (`ff2 = ff1; ff1 = d.read();
q.write(ff2); tick` with `q` a `RegOut`, measured 2026-11-04 in today's simulator:
identical to the reference over the anchor schedule, and its register set {`ff1`} plus the
`RegOut` `q` is the reference's two flops); `lfsr`'s constant initializer at the first
observation (`tests/lfsr_equivalence.rs`). Sim ≡ SV only: the reconciliation of codegen's
flops with the inference over the fixtures (`copper-codegen/tests/register_reconciliation.rs`,
105 modules, no violation), loop-born and tuple registers, array locals, the two
nested-loop inference defects of I.7.5. Decided: the definition on executions rather than
on the CFG, the pre-loop wire of I.7.2(c), the `Bits::x()` reading and the loop-born
power-on reading of I.7.4.

In coroutine terms (D-7, D-8): a register is a local the coroutine must carry across a
`tick` suspension; with the `RegOut` values and the memories it is a variable of H(M)
(I.0.4), and the transpiler's flops are its realization.

**I.7.1 Definition.** On an execution of a module instance under an input stream (I.3.1),
a local `v` is **live at a tick** `T` iff some assignment to `v` precedes `T` on that
execution and some read of `v` follows `T` with no assignment to `v` between `T` and the
read; a read in a later resumption, after the loop's back edge or inside a nested loop
counts like any other. A local is a **register** iff (i) it is assigned inside the module
loop — a `let` or an assignment in the loop body or in a nested loop; `s_pre` does not
count — and (ii) it is live at some tick of some execution. A local that fails (i) is a
**constant**: a pre-loop `let v = c` never assigned in the loop keeps `c` forever and is not
state (REG3; `c = Bits::x()` is a constant X, → I.14; D-19: I.14.2(d)). A local that fails (ii) is a **wire of
a resumption**: every read of it sees an assignment of the same resumption (I.3.4), or it is
never read. This is I.0.5(a) instantiated, and I.0.5(a)'s proposition — replacing every
non-register by an arbitrary value at every tick leaves the trace unchanged — is III.1's (D-20: P2, measured) to
check (the poison run). The definition quantifies over executions and is therefore a
property of the kernel; the analysis of I.7.5 approximates it, and the direction of each
approximation is stated there. The declarative form is Esterel's: the registers are the
values the residue after a reaction must carry [esterel §6.5.6, note items 2–3]; the
liveness pass is its implementation. Spade's `reg;` registers every value *visible* above
it [spade §4], Prost every local [prost §2] — supersets of (ii) by visibility; Chisel,
CλaSH, Anvil, Bluespec and RustHDL declare registers and infer nothing [chisel §4; clash
§II-F; anvil §4; bluespec §4; rhdl §4]. No surveyed language infers its registers by
liveness; no precedent.

**I.7.2 What is a register and what is not.** By I.7.1:
(a) a pre-loop `let v = c` reassigned in the loop and read after a tick before its
reassignment — a register, with `c` its initial value (I.7.4);
(b) a pre-loop `let` never assigned in the loop — a constant (REG3);
(c) a pre-loop `let v = c` assigned in the loop but never live at a tick — a wire of each
resumption; `c` is read, if at all, by the first resumption alone, before the first
assignment, as a constant of `s_pre` (→ I.9), and never by a later one (D-9 A1-i). The `ff2`
of `q.write(ff2); tick; ff2 = ff1; ff1 = d.read()` is this case: assigned and consumed within
one resumption from the tick site, dead at the `tick`; the second flop of `sync_2ff_ref.sv`
is the `RegOut` `q` (I.0.5(b), I.3.5(c));
(d) a `let` born inside the loop and read after a later tick before its reassignment — a
register (REG11; `mac_pipeline`'s `product`, `c_s`, `sum`), with no initial value (I.7.4);
(e) a same-cycle temporary, assigned and consumed before the next tick — a wire (REG4);
(f) a tuple target `(a, b) = e` — one local per element (I.2.4), each judged alone: in
`tick; (a, b) = (b, x.read())` read in the next head, `b` is a register and `a` a wire;
(g) the counter of a tick-bearing `for` — the desugared counter register of I.2.4 (REG11;
`_` binds nothing and the transpiler's counter for it is realization state); (D-10: under
I.8.5's `D` the counter `c` is a register of the kernel whatever the pattern, `_` included —
the iteration count is state the residue carries — and `__copper_ctr<n>` is the transpiler's
name for it, not realization state of its own; read (j)'s `__copper_ctr` accordingly);
(h) an array local — one local holding an array value; `arr[i] = e` is an assignment to it
(I.2.4), forwarded within the resumption like any other (I.3.4: a later read of `arr[j]`,
or of `arr`, in the same resumption sees it); the local is a register iff live at a tick,
whichever elements each resumption touches. The word-indexed register file of the CPU is
this case (DOCS gap 27). The transpiler's reading of a register's committed `Q` instead of
its forwarded value inside an element write (`bits[12+k] = w3[k]`, `review/Q1_TRADEOFFS.md`
§4 (ii)) is a defect of I.3.4's realization, carried to Phase C;
(i) a struct local — one local per field (I.2.4), each judged alone (`struct_latch`'s
`latch.valid`, `latch.data`);
(j) `pc`, `phase_r`, a hoisted cross-state local, a promoted `_r` wire, `__copper_ctr`, a
memory pipeline stage — realization state, encodings of H(M)'s site and of (a)–(i)
(I.0.4), not registers of the kernel; III states what each must be a subset of (I.7.6).
A `Memory` binding is not a local of this clause (→ I.10; D-16: I.10.1(a) — a variable of H(M) of its own kind). A `combinational` module has no
`tick` and so no register (→ I.11; D-14: I.11.1). A synchronizer's registers are judged by the same rule
(→ I.12; D-17: I.12.3 — `ff1` a register, `q` a `RegOut`, `ff2` a wire; closed).

**I.7.3 The value of a register.** In the resumption that ends at edge N+1, a read of a
register `v` yields the last value assigned to `v` earlier in that resumption, else the
value `v` had when the previous resumption ended (I.3.4, the one-instant form) — which is
the value the edge N committed (I.3.3(ii)): the last assignment of the resumption that
ended at edge N, else of the one before it, …, else its initial value (I.7.4). The
resumption's own assignments are committed at edge N+1, together with the `RegOut` pending
values (I.6.1) — the one commit of the cycle — so a register's committed value changes only
at an edge (I.1.3), and a register the resumption does not assign keeps its value (a
corollary, as I.6.3 for a `RegOut`). A register is never observed directly: an observation
sees a `RegOut` (I.6) or the wire of a `combinational` module that reads an exposed `RegOut`
(→ I.11, I.13; D-14: I.11.5; D-18: I.13.2); every statement about registers in this document is therefore a statement
about traces, and I.0.5(a) is the proposition that the trace is a function of the registers,
the `RegOut` values, the memories and the input stream alone. A realization may store a wire
of a resumption, but a read of that wire after its assignment in the same resumption yields
the assigned value, never the stored one (I.3.4) — the two-cycle `sync_2ff` of I.7.5 is what
the stored one gives. The linear route's trailing fold (a trailing region's updates
realized at the edge that opens its resumption, I.6.9) is an encoding of this clause,
III's to keep or drop. Prost's "local variables correspond to registers" with every effect
at the edge [prost §2] is this clause with (ii) taken as "every local"; Chisel's register
"updated at the end of the current clock cycle" [chisel §2.6], Anvil's `e_c ▷ #1` [anvil
§4.5] and Filament's `Register` signature [filament §2.3] say the commit in their own time
models.

**I.7.4 Power-on.** (a) A register of I.7.2(a) has, before its first assignment, the value
of its initializer `c` — a constant of I.14 (D-19: I.14.4) by I.2.3's grammar — and the emitted text
states it (an `initial` assignment, PO1, PO2; a realization obligation of III.2 (D-20: III.2.5), not an
environment precondition: a register is not a wire the environment creates, and the only
one it creates is a port's, I.6.6). (b) `Bits::x()` or `Logic::X` as the initializer means
**unstated** (D-9 A2-i): the register's value before its first assignment is X in the
simulator (→ I.14 for what a read of it computes; D-19: I.14.2(b)–(c) — the operator table over X; a condition or an `as_*` view of X a run-time failure), nothing is stated in the emitted text
(PO3), and a module with such a register is outside the agreement relation (→ III.2; D-20: III.2.2, III.2.5) until
every such register has been assigned on every path — "until reset"; the sweep's `RESET`
table is III.2's declaration (D-20: III.2.2, "from reset") of that for each such module, and `COPPER_VERILATOR_X_INITIAL=1`
measures what a design leaves unstated. (c) A register's initializer that is not a constant is not in
the kernel (I.2.3: a register's `s_pre` is `let v = c` or `Bits::x()`) — the shape rule R11 of
II.1, for **registers only** (D-21 C2: a pre-loop `let` that is a wire of the first resumption,
I.7.2(c), may have any pure initializer, I.2.3). "Constant" is rustc's (D-21 A3): the macro
evaluates every pre-loop register initializer in an inline `const { }` block, so rustc admits
exactly I.14.4's constants and refuses the rest (E0015 for a call to a non-`const` fn); both
front-ends share R11's syntactic half `check_register_initializers` — an initializer reading a
run-time value (a parameter, an earlier pre-loop binding) is refused with R11's own message —
and the transpiler, which cannot see rustc's verdict, adds `check_register_initializer_calls_in`
(every callee a `Bits`/`Logic` constructor or a `const fn` of the file) before its fold. No
opt-out (PO4). (D-12: R11, S3; D-21 A3, C2.) (d) A register born inside the loop (I.7.2(d)) has no value
before its first assignment and cannot be read before it: a `let` opens its name after it
(Rust scoping), so no execution reads it unassigned (D-9 A3-i; PO5 made precise — not X).
What a realization powers its storage on at is III's (0 today, PO6, CODE 33). (e) Realization
state of I.7.2(j) powers on at 0 (PO6) — a III fact with no meaning here. SystemC's reset
behaviour — everything before the first `wait()`, "reset values shall be statically
determinable constants", a constructor's value a simulation artefact [systemc §4.2.2] — is
the nearest precedent for (a) and (c); Chisel's `Reg` without `resetVal` is X in the
Verilog [chisel §4], MyHDL's converted HDL is X until reset where its simulator holds the
initial value [myhdl §4.3], CλaSH's initial value is a simulation parameter [clash §II-F],
Filament's `Prev[SAFE]` the only initial-value notion [filament §4]. No surveyed language
states an inferred register's power-on value in the emitted HDL; no precedent.

**I.7.5 The back-edge clause, re-derived (measured 2026-11-04).** `infer_registers` takes,
besides the locals live out of a tick node, those live out of the loop head (REG2, the
2026-08-21 clause for `sync_2ff`'s `ff2`). D-7 named that surplus "the trailing carriers";
D-8 left it to this clause. Measured corpus-wide (135 clocked modules in `examples/`,
`tests/fixtures/`, `src/`; predicted first): the clause adds **8 locals in 7 modules** and
every one has one shape — assigned after the iteration's last tick, read in the next head
before its first tick, dead at every tick: `ff2` in `sync_2ff`, `flag_sync` and
`sync_2ff_concrete`; `w` in `trailing_local_into_head`; `latch` in `struct_latch` and
`struct_latch_cond`; `latch` and `target` in `tuple_destructure`. Under I.7.1 each is a
wire of one resumption (I.7.2(c)), so the clause is **over-inference** and goes in Phase C
(D-9). The anchor decides it: `sync_2ff` with `q` a `RegOut`, spelled `q.write(ff2); tick;
ff2 = ff1; ff1 = d.read()` and realized with `ff2` a flop (today's inference and emission:
`q <= ff2; ff2 <= ff1; ff1 <= d`, three flops), is one cycle late against `sync_2ff_ref.sv`
in today's simulator and today's SV alike (sweep PASS, reference DIFFERS at six of sixteen
cycles); spelled `ff2 = ff1; ff1 = d.read(); q.write(ff2); tick` — the one-job spelling, in
which `ff2` is a wire on every route — the inference gives {`ff1`}, the emitted text is
`q <= ff1; ff1 <= d`, and the trace is identical to the reference. REG9 and X9 ("`ff2 =
ff1` must be a flop") were true of the two-job body and are retired with it. Two defects
of the implementation that are not the clause's, measured on probes: a tick-bearing
nested loop is one folded node of the CFG with no definitions, so a local assigned before
it and read inside it after an interior tick is **missed** (`loop { let v = x.read(); for i
in 0..4 { q.write(v + i); tick } }` infers only `i`; the transpiler emits a flop for `v` by
hoisting, so sim ≡ SV holds and no fixture has the shape), and a local assigned inside it
and read only after it is **added** though dead at every tick. Both are III.1 obligations (D-20: P3):
the inference must compute I.7.1 through the nested loop's own CFG, and the reconciliation
test gains a fixture of each shape. `anvil`'s two-iteration lemma [anvil Lemma C.19] is the
premise under which (ii) is decided on `body ; body` (I.3.6).

**I.7.6 Implementation map.** Analysis: `Cfg::registers` / `compute_registers`
(`copper-analysis/src/cfg.rs:394-415`): `defined_in_loop` ∩ (live-out at every tick node ∪
live-out at the head, `:408-409` — the clause of I.7.5); `DefinedInLoop` (`:3402-3440`:
`let` bindings, assignment targets through `Index`/`Field`/`Tuple`, `assign_targets`
`:3443-3455`, a ticking `for`'s binding; match-arm patterns excluded); the folded nested
loop (`:3062-3119`: `defs` empty, interior reads as `uses`, out-edge `Tick`); backward
liveness (`:2600-2632`); `infer_registers` (`lib.rs:95-98`); `check_register_initializers`
(`lib.rs:51-56`, pre-loop `let`s in `registers()` only; `is_constant_expr` `cfg.rs:3519-3550`);
`assert_source_registers_match_reference_sv` (`lib.rs:391-425`, `NameExact` /
`StorageEquivalent`). Consumers: the macro only for rule 5 (`copper-macros/src/lib.rs:990`)
and a debug log (`:1357`) — the simulator needs no register set, rustc's lowering keeps
every local (I.0.5(c), a superset to be measured); the transpiler consumes `fir.registers`
for the pre-loop `let mut`s (`copper-codegen/src/parser.rs:68`, `lib.rs:552`, `chir_lower`)
and realizes loop-born registers by two mechanisms of its own — `find_promoted_wires`
(`shir_lower.rs:1495`: a wire referenced in another phase becomes `<name>_r`, powered on at
0 without a literal initializer, `:612-625`) and `hoist_cross_state_locals`
(`control_extract.rs:914-961`: a local mentioned in another state becomes a pre-loop `let
mut` whose initializer is kept for its type, by name mention rather than liveness). Power-on:
`lower_register_init` (`chir_lower.rs:1158-1198`: folds the constant, `Bits::x()` → no init,
refuses a non-foldable or ill-fitting one), `reg_inits` (`emit.rs:418-432`: one `initial`
block; a register with no init gets no line), the `RESET` table (`build.rs:206-230`),
`COPPER_VERILATOR_X_INITIAL`. Checks: `register_reconciliation.rs` (fixtures; codegen flops
− inferred ⊆ {`phase`, `pc`, counters, `<port>_hold`, memory stages, struct fields}; an
inferred register kept as a comb wire on a single-phase route allowed) and the four
reference matches. Obligations carried to Phase C (D-9): delete the head clause; descend
into the nested loop's CFG; retire REG2/REG9/X9, SS's back-edge sentence, the unit pin
`post_tick_def_read_pre_tick_is_a_register` and the `sync_2ff` anchor's `NameExact`
{ff1, ff2} (restated: reference flops = registers ∪ `RegOut` ports); `sync_2ff`,
`flag_sync` and the fixture twin in the tick-last spelling; the array-element forwarding
defect fixed; two reconciliation fixtures for the fold's shapes; rule 5 cited from II.1 as
I.2.3's shape rule for registers (D-12: R11; its widening to every pre-loop `let` retired by
D-21 C2).

**I.7.7 Carried NOT-YET list.**
- The poison run for I.0.5(a) (→ III.1; D-20: P2 — measured, 47 modules clean): at every tick, every local not in the register set
  overwritten; `ff2` in the tick-last `sync_2ff` the first negative case, `ff1` the first
  positive.
- rustc's `Future` layout as a superset of I.7.1 (`-Zprint-type-sizes` on a few fixtures;
  REG5, asserted and never measured) → III.1 (D-20: P4 — measured on seven fixtures).
- Hoisted locals ⊆ registers and promoted wires ⊆ registers on the corpus (today bounded
  only by the reconciliation test, fixtures only) → III.1 (D-20: P13); the hoist's claim that "a
  registered output can forward the flop before" its assignment (`control_extract.rs`
  comment on the hoisted initializer) names something the kernel cannot do (I.7.4(d)) and
  is a realization defect if it is observable → III.1 to measure (D-20: P14, ASSERTED).
- The two nested-loop fold defects of I.7.5 → III.1 / Phase C (D-20: P3); the nested loop's ticks as
  suspension sites of the kernel and the zero-iteration loop's effect on (ii) → I.8. (D-10:
  I.8.2 states the sites; I.8.4(a) the zero-iteration loop — a skipped loop contributes no
  tick, so a local is live at its ticks only on the executions that enter it, which I.7.1's
  quantification over executions already covers; the fold's empty `defs` is the direction of
  the approximation.)
- The array-element forwarding defect (`review/Q1_TRADEOFFS.md` §4 (ii)) → Phase C, as a
  defect of I.3.4's realization. (D-13: row (f5).)
- The first resumption's reading of a pre-loop initializer — a register's initial value
  and I.7.2(c)'s constant of `s_pre` — and the entry resumption as a state of its own on the
  extraction route → I.9. (D-11: I.9.2 — read once as a constant; the entry is H(M)'s initial
  state and no tick site; a realization owes an entry that runs the first resumption exactly
  once, on both routes.)
- What a read of a `Bits::x()` register computes before its assignment, and a pre-loop
  constant X → I.14 (D-19: I.14.2(b)–(d) — closed); which modules the agreement relation covers only from reset (the
  `RESET` rows), and the precondition that the emitted text states every constant
  initializer → III.2 (D-20: III.2.2, III.2.5).
- `Memory` bindings and memory arrays → I.10 (D-16: I.10.1(a)); the synchronizer's register set → I.12. (D-17: I.12.3 — closed.)
- The sim ≡ SV-only cases above (loop-born, tuple, array registers): an independent
  reference with a loop-born register is owed to III.2 (D-20: III.2.11 item 3) (`ANCHORING_A_MODULE.md`; the user
  writes references).

### I.8 Loops, control flow and the cycle boundary

Decided: D-10 (2026-11-05). Anchor: `tests/det_for_probe.rs` — the counted `for` with a
`break` before its tick, sim = a hand model of the source = SV cycle for cycle (the extra
cycle the hoisted tick once charged a `break` was measured and fixed 2026-09-02); the
`sync_2ff` anchor of I.3 for the back edge (trailing region and next head in one cycle);
`det_010_awaits` vs `examples/sequential/sv/pattern_detector_010.sv` for a `while` wait
loop (the test before the tick, a variable number of cycles). Sim ≡ SV only: every other
nested loop (28 modules, I.8.6), the `break` landing (probes (i), (i-b)), the zero-iteration
`while` (probe (iii)); the run-time-empty counted `for` is measured **divergent** (probe (ii))
and carried as a Phase C defect. Decided: the principle of each loop rule, the zero-iteration
meaning, the counted `for`'s desugaring, the nested `continue`.

In coroutine terms (D-7, D-8): a loop is the coroutine's control structure between its
suspension sites; every tick site inside a loop, nested or not, is a state of H(M); a
`break` or `continue` is a statement of the resumption that selects its next statement, and
a loop iteration that reaches no tick is part of the resumption that entered it.

**I.8.1 The module loop.** (Restated from I.2.2, I.2.5, I.3.2, I.3.5, I.3.6; nothing new is
decided here.) `loop { s }` is the coroutine's cycle structure: a resumption runs from the
tick site it is parked at to the next tick site it reaches; when the body ends, the back
edge is taken in zero time and the resumption continues into the next iteration's head —
the trailing region of iteration k and the head of iteration k+1 are one resumption,
ending at the first tick the head reaches (I.3.2, I.3.5, I.3.6). `continue` of the module
loop is a zero-time goto to the head: the resumption continues at `s`'s first statement
(I.3.2). `break` of the module loop and `return` are OUTSIDE (I.2.5(d)): the coroutine
never returns. Prost's task runs "until its next suspension point" at each edge and
unrolls all control flow into the next-state function [prost §2, §4]; SystemC's thread is
an infinite loop whose `break` is "Not Supported" and whose `continue` is supported
[systemc §4.2.2, §10.5]; Esterel's `loop` unfolds instantaneously and its body must not be
instantaneous [esterel §6.5.7, §3.4] — the same shape as `check_reachability` (I.8.2).

**I.8.2 Nested tick-bearing loops.** A `loop`, a desugared `while` (I.2.4) or a counted
`for` (I.8.5) inside the module loop whose body contains a `tick` is a nested loop. Its tick
sites are **suspension sites of the kernel** like any other (I.0.1): H(M) has a state per
each, a resumption parked at one runs the rest of the nested body, its back edge or its
exit, and whatever follows, to the next tick site it reaches — which may be the same site
(another iteration), a later site of the nested body, or a site after the loop (after a
`break` or a fall-through exit). A nested loop adds no state of its own: its iteration
count, if any, is a register (I.8.5; D-9), and which site the coroutine is parked at is
the residue (I.3.3). Two rules are enforced on a nested loop today, and they have
**different principles**:
(a) **Every path of the nested loop reaches a tick** — a **shape** rule (II.1; I.0.6):
a cycle of the kernel with no tick on it is a resumption that never ends, and the
coroutine's state is then not finite in time. The implementation is `check_reachability`
on the nested loop's own CFG (`Cfg::nested_loop_cfg`, `copper-analysis/src/cfg.rs:2896-2920`,
built with `break` → exit and `continue` → head; the parent sees the loop as one folded
node, `:3062-3119`), and on the parent with the folded node's zero-tick exit counted as a
zero-time edge (`may_exit_without_tick`, `:3572-3604`; the DFS `:2405-2464`). The
livelock it refuses was measured on the simulator (99.5 % CPU, no progress, `:3078-3090`)
before the rule existed; `tests/ui/fail/for_break_before_tick.rs` pins it.
(b) **The nested loop ends at its tick** — the body's last statement is a bare `tick` or
a nested loop that itself ends at one; a tick only inside `if`/`match` arms gives no
boundary (`check_nested_loop_shape`, `copper-analysis/src/shape.rs`). Its recorded reason
(2026-08-24, SS "A repeating wait must test before its tick") is that
`loop { tick; if go.read() == One { break } }` read `go` in the post-edge window of the
two-job body while the flip-flop sampled before its own edge, measured a cycle apart. Under
one resumption per cycle (D-8) both spellings read `x_{N+1}`, and `loop { tick; C }` is the
same coroutine as `tick; loop { C; tick }` (rotate the body around its tick: the first
iteration's tick is taken once before the loop, every later iteration runs `C` then ticks).
The rule's principle is therefore **design — one spelling per meaning** (II.2; I.0.6, as
D-5 and D-8 for the Mealy output): the tick-last form is the canonical spelling of a
repeating wait, and a body `W ; tick ; C` is written `W ; tick ; loop { C ; W ; tick }`
(D-10 A1-i). A body whose ticks lie only inside arms is not reducible to that form without
a register and is a **dated lowering limitation** (II.3, 2026-11-05): the transpiler's
lowering has an arm for it (`control_extract.rs:1290-1300`, cloning the lowered body into
the entry) that `unflattenable_in_expr` gates off (`:1804`, `:1814`); the fix direction is
to measure that arm under Phase C and admit the shape, or keep the refusal with that
measurement as its reason. Neither half of (b) is a shape rule, and the two-job sentence
that justified it is retired with D-8. (D-12: (a) is R3, the tick-last spelling II.2 D1, the arms-only loop II.3.) (D-13: row (b), measured — the gated arm agrees on the wait shape and diverges on a two-arm body.)
The transpiler's realization of a nested loop is one head state per loop holding the body
**rotated** around its tick (`W ; tick ; C` lowered as `C ; W`, `control_extract.rs:1265-1281`),
entry running `W` in the current cycle, the fall-through standing for the removed tick
(`FallThrough::AfterTick`, `:1162-1188`), and a `break` **inlining** the continuation in the
same cycle (`:1236-1252`, lowered once per loop). These are encodings of the residue
(I.3.3): `pc` is allocated per tick occurrence and is many-to-one onto tick sites (I.0.4),
the obligation that it encodes H(M) is III.1's (I.0.8; D-20: P1 — `pc` is many-to-many, III.1.3). The simulator realizes a nested
loop by running the Rust loop (I.0.7). Prost supports "any control flow construct" by the
same unrolling [prost §4]; Spade and MyHDL have no tick inside a nested loop at all
[spade §4 note; myhdl §4 note — one `yield` at the loop head]; SystemC allows `wait()` in
inner loops and `break` from them [systemc §10.5].

**I.8.3 `break` and `continue` of a nested loop.** Both are statements of the resumption
that select the next statement, never the next cycle (I.3.3): a `break` continues with the
statement after the loop, a `continue` with the loop's head test, in the same resumption,
and the resumption ends at whichever tick that path reaches. Measured (D-10, probes (i) and
(i-b)): a `RegOut` written after a wait loop's `break` takes effect at the edge that ends
the resumption which tested the exit condition, and a `break` whose continuation is the
trailing region runs it and the next head in the same resumption — sim = hand model = SV.
A **nested `continue`** has its Rust meaning (a zero-time return to the nested loop's head)
and is IN (D-10 A2-i); the transpiler refuses it today (`control_extract.rs:1777-1783`: the
rotated head state holds `C ; W` and re-entering it would run `C`), the macro admits it
(measured 2026-11-04 on a probe), and the refusal is a **dated lowering limitation** (II.3,
2026-11-05) with the fix direction "lower a nested `continue` as the loop's entry copy,
`W ; pc = head`" — the same statements `lower_into` already emits for entering the loop
(`:1278-1281`). No corpus module uses one. A **labelled or valued `break`** is NOT YET →
II.1 refuses it in both front-ends (D-10 A5): the transpiler refuses it (`:1792-1801`), the
macro admits it (measured on a probe), no corpus module uses one, and the CFG models a
labelled `break` as a `break` of the innermost loop (`breaks_enclosing_loop`,
`cfg.rs:3641-3655`), so every verdict the analysis gives such a module is the verdict of a
different program. `break` of the module loop: I.8.1. (D-12: R12; the nested `continue` is II.3's; `break` of the module loop and `return` are R7.) (D-13: row (a).)

**I.8.4 The zero-iteration loop.** (a) A nested `while c { … tick }` whose test is false on
entry, and a nested `loop` that `break`s before its first tick, run **zero iterations**: a
zero-time exit that the entering resumption continues through, with no tick and no
boundary (I.3.2). It is legal iff the enclosing path still reaches a tick — exactly what
`check_reachability` tests (I.8.2(a)); an enclosing loop whose only tick is inside such a
loop is refused (`cfg.rs:2419-2425`, the `may_exit_without_tick` clause). Measured (D-10,
probe (iii)): a `while` false on entry followed by a `RegOut` write and a tick — the write
takes effect at the edge that ends the resumption which skipped the loop, sim = hand
model = SV. **Decided (D-10 A3-i):** Copper keeps this rule. Prost requires "each loop to
contain at least one wait statement and to run for at least one iteration" [prost §5,
§8.1], so a skippable `while … wait` is refused there; the measured cost of that rule on
Copper's corpus is **20 of 135 clocked modules** — every `while` wait loop, every
`loop { if c { break } tick }` wait and `det_for`'s mismatch exit (I.8.6 names them) —
including every shipped handshake, the UART receiver and transmitter and `det_010_awaits`,
an anchored module. Esterel refuses an *instantaneous loop body* [esterel §3.4] — the body
of `loop` must take time — which is rule (a) of I.8.2, not a trip-count rule: an Esterel
`loop` cannot be skipped because it has no exit test, and the skippable Copper loop is
its `present S then … else loop … end` form. Anvil's two-iterations lemma [anvil Lemma
C.19] is the premise of I.3.6 and says nothing about a skipped iteration. On liveness
(I.7.1): a skipped loop contributes no tick, so a local is live at the loop's ticks only on
the executions that enter it; I.7.1's quantification over executions already covers that,
and the folded node of I.7.5 (empty `defs`, interior reads as `uses`) is the direction in
which the implementation approximates it (it treats the skipped path as reading what the
body reads).
(b) A tick-bearing counted `for v in a..b { … tick }` over a range **empty only at run
time** (`a >= b` at entry, with a non-constant bound) runs zero iterations by `D` (I.8.5)
and is case (a): zero time, no boundary, the enclosing path must reach a tick. **Decided
(D-10 A4-i):** this is its meaning; it is no longer NOT YET (I.2.8, I.3.9 closed). Measured
(D-10, probe (ii), `for _ in 0..cnt { … tick }` with `cnt` read from an input, a tick after
the loop): both front-ends accept it; the simulator runs zero iterations; the transpiled
FSM spends **one extra cycle** per zero-count pass — the tick hoisted after the loop
(`control_extract.rs:411-449`) is taken after the first exit test breaks, so a zero-count
pass costs two states (the entry state with no write, then the state after the loop)
against the simulator's one resumption; divergent at 3 of 24 cycles, the rest masked by
consecutive zero-count passes showing the same value; sim = hand model at every cycle.
This is a **Phase C defect of the realization**, not a limitation to refuse: the fix
direction is to guard the hoisted tick on the counter having advanced past its start (the
guard the author's-`break` case already has, `:435-449`, extended to `c != a`), or to lower
I.8.5's kernel form through the rotation of I.8.2; the probe becomes a sweep fixture
(III.3). A **literal-empty** range (`0..0`) is dropped by the transpiler (`:359-365`), which
agrees with `D`; the analysis counts it as a tick boundary (`may_exit_without_tick` answers
"no exit without a tick" for a `for` without a `break`, `:3598`), so an enclosing loop whose
only tick is a literal-empty `for` passes the check and livelocks the simulator — RULES open
5, carried to II.1 (I.8.7). (D-12: R3's under-admission, II.2.2.) (D-13: the extra cycle is row (f1).)

**I.8.5 The counted `for` as a desugaring.** `D` (I.2.4) rewrites a tick-bearing
`for v in a..b { s }`, `s` ending at the tick (I.8.2(b)), as

```
let mut c = a ; let e = b ;
loop { if c >= e { break } ; let v = c ; c = c + 1 ; s }
```

with `a` and `b` evaluated once at entry (Rust evaluates the range once), `c` the counter,
`v` the iteration's value (for `_` no `v` is bound and the counter exists all the same),
exclusive ranges only (I.2.4). The nested loop ends at the tick `s` ends at; a `break` in
`s` leaves to the statement after the `for`; a `continue` in `s` returns to the test with
the next `c` — Rust's meaning of both. (D-23, 2026-11-08: a `continue` that targets a tick-bearing
`for` — not one of a loop nested in it — is **refused in both front-ends** with one message,
`front_end::check_ticking_for_continue`, a dated lowering limitation, II.3.3 row (h): the
transpiler's counted `for` has no state for "increment, then test", and its fix direction is that
lowering. The meaning above stands; the refusal is the realization's.) Under D-9 `c` is a **register** (assigned in the
loop, live at the tick: the next iteration's test reads it); `v` is a wire of its
resumption unless `s` reads it after the tick, when it is a register too. The transpiler
realizes `D` with one counter, named by the pattern or `__copper_ctr<n>` for `_`,
incremented before the tick, a second exit test after the increment and the loop's tick
hoisted after the loop (`control_extract.rs:339-470`): the same executions as `D` except
the zero-iteration entry of I.8.4(b). The hoisting preserves the resumption boundaries of
the source form on every entered iteration: the body's tick is the hoisted one on the last
iteration and the loop's own otherwise, and a `break` the author wrote leaves before the
body's tick in the source, so the hoisted tick is guarded on the counter having reached
the end (`:425-449`) — `tests/det_for_probe.rs` is the anchor (sim = a hand model of the
source = SV on a stream with a mismatch at every bit position; before the 2026-09-02 fix
the RTL lost three detections to the unguarded tick). The counted `for` is counted
repetition, not unrolling: a UART bit period of 434 cycles is one state and one counter
(`:228-233`). A tick-free `for` with constant bounds is unrolled by `D` (I.2.4; shape
(c) of I.2.5); a tick-free `for` with a data-dependent bound is NOT YET → II.1 refuses it
(I.2.5(c): unbounded work in an instant). (D-12: R5, refused in both front-ends by decision; both admit it today.) Prost's `for` is the same counter-and-state
form, its counter a register re-zeroed on exit [prost §4, Listing 2]; SystemC's
synthesizable subset allows loops with `wait()` inside and counts them the same way
[systemc §4.2.2].

**I.8.6 Implementation map.** Simulator: Rust's own control flow performs every loop,
`break` and `continue` (I.0.7; nothing injected). Transpiler, linear route:
`split_at_ticks` (`copper-codegen/src/shir_lower.rs:873-883`) cuts the module loop's body
at its top-level ticks into `phase_r` positions; a nested loop sends the module to the
extraction route. Extraction route (`copper-codegen/src/control_extract.rs`): the `while`
desugar (`desugar_tick_waits`, `:135-158`), the counted-`for` desugar (`expand_counted_for`,
`:339-470`), `lower_into` (`:1208-1358`: a tick allocates a state per occurrence `:1217`; a
trailing tick is `pc = head` `:1213-1215`; a module-loop `continue` and a zero-time
fall-through are `goto_marker`s spliced by `splice_zero_time_gotos` `:777`; a nested loop's
head state holds the rotated body `:1265-1281` with `FallThrough::AfterTick`; a `break`
inlines the cached continuation `:1236-1252`), `hoist_cross_state_locals` (`:914-961`, by
name — I.7.6), the refusals `unflattenable_in_expr` (`:1768-1820`: nested `continue`,
labelled and valued `break`, a nested loop not ending at a boundary). Analysis:
`check_nested_loop_shape` and `check_awaits` (`copper-analysis/src/shape.rs`, both
front-ends), `check_reachability` (`cfg.rs:2405-2464`) over `nested_loop_cfg`
(`:2896-2920`) and the fold (`:3062-3119`), `may_exit_without_tick` / `ticks_before_any_break`
(`:3572-3640`). Tests: `tests/det_for_probe.rs`, `tests/wait_loop_equivalence.rs`,
`tests/nested_tick_equivalence.rs`, `tests/counted_delay_equivalence.rs`,
`tests/counted_repetition_equivalence.rs`, `copper-codegen/tests/while_loops.rs` (the
`while` and `loop` spellings emit identical SV), the ui/fail cases
`for_break_before_tick.rs`, `tickless_loop.rs`, `partial_tick_path.rs`, the `cfg.rs` unit
tests named in SS "Nested loops", and the sweep. Nested tick-bearing loops in the corpus
(28 of 135 clocked modules, 51 loops; syn scan 2026-11-04): shipped —
`rv32i_cpu_pipelined` (`examples/cpu/`), `handshake`, `det_010_awaits`, `det_for`,
`serializer`, `simple_handshake` (`examples/sequential/`), `rx` (`examples/uart/rx.rs`),
`uart_rx`, `uart_tx` (`examples/uart/system.rs`); fixtures — `skip_on_halt`,
`counted_delay`, `counted_repetition`, `capture_after_wait`, `det_for`, `ex_reg_then_wire`,
`ex_regfile`, `rom_gated`, `rom_paced`, `handshake` (`handshake_dut.rs`), `nested_boundary`,
`uart_rx_dut`, `vr_read_after_delay`, `vr_sink_moore`, `vr_src_regout`, `vr_src_same_cycle`,
`handshake` and `waiter` (`wait_loop_dut.rs`), `while_waiter`; none in `src/`. Of these, 20
have a zero-iteration path (I.8.4(a)): every one except `rv32i_cpu_pipelined`,
`skip_on_halt`, `counted_delay`, `counted_repetition`, `ex_reg_then_wire`, `ex_regfile`,
`rom_paced` and `nested_boundary`. No corpus module has a run-time `for` bound, a nested
`continue` or a labelled `break`. Obligations carried to Phase C (D-10): the hoisted tick's
zero-iteration guard and the probe as a sweep fixture; a nested `continue` lowered as the
entry copy; the two parity breaks (nested `continue` → II.3 form, labelled `break` → II.1
refusal in the macro); the ticks-only-in-arms arm measured or its refusal re-justified. (D-12: the two parity breaks are rows of II.1.3.) (D-13: rows (a), (b).)

**I.8.7 Carried NOT-YET list.**
- The two nested-loop fold defects of I.7.5 (a local assigned before a ticking `for` and
  read inside it is missed; one assigned inside and read only after it is added) → III.1 (D-20: P3) /
  Phase C (D-9).
- The `pc`-vs-tick-site measurement: that the extraction route's states encode H(M)'s
  sites, nested loops included (the rotation, the inlined `break` continuation and the
  cloned entry are the shapes that make `pc` many-to-one) → III.1 (I.0.8; D-20: P1 — many-to-many, III.1.3).
- A labelled or valued `break` → II.1, refuse in both front-ends (I.8.3); a tick-free
  `for` with a data-dependent bound → II.1 (I.8.5).
- The literal-empty range counted as a boundary by the reachability check (RULES open 5,
  I.8.4(b)) → II.1: a `for` over a literal range the analysis can evaluate is a boundary
  iff the range is non-empty.
- `may_exit_without_tick` over-approximates: a nested `loop` with no `break` whose first
  statement is an `if` containing a tick is reported as exitable before a tick
  (`ticks_before_any_break` returns at "ticks only on some path", `cfg.rs:3634-3636`), so an
  enclosing loop whose only boundary it is gets a false livelock refusal (measured
  2026-11-04 on a probe). A direction of approximation II.1 must state, or narrow. (D-12: stated in II.2.2 — over-refusal, measured on `livelock_a`/`livelock_b`, narrowed in Phase C; the labelled `break` is R12, the literal-empty range R3's under-admission, the hex literal a transpiler defect of II.1.3.)
- The ticks-only-in-arms nested loop: a II.3 limitation until the gated lowering arm is
  measured (I.8.2(b)). (D-13: measured, row (b).)
- A nested `continue`: a II.3 limitation until the entry-copy lowering lands (I.8.3). (D-13: row (a).)
- The run-time-empty counted `for`: a Phase C defect of the hoisted tick (I.8.4(b)); until
  it lands the shape is divergent and unrefused, and the sweep has no fixture of it. (D-13: row (f1).)
- A hex literal in a `from_lit` turbofish (`Bits::from_lit::<0xFF>()`) is refused by the
  transpiler and admitted by the macro (found 2026-11-04 while probing) → II.1 parity. (D-13: row (g).)
- An independent reference for a module with a nested tick-bearing loop (`det_010_awaits`
  anchors the `while` wait only) → III.2 (D-20: III.2.11 item 4) (`ANCHORING_A_MODULE.md`; the user writes
  references).
- Memory accesses inside a nested loop (`entry_cycle_mem`, `cfg.rs:3101-3106`: only the
  accesses before the loop's first tick belong to the entering cycle) → I.10. (D-16: I.10.4 — the
  folded-node attribution stands for (c) and (d); (a) and (b) are retired.)

### I.9 Startup

Decided: D-11 (2026-11-05). Anchor: `obs_1` onward of every hardware-anchored module starts
from the first resumption's effects — `lfsr` vs BaseJump `bsg_lfsr` (a constant register
initializer at the first observation, `tests/lfsr_equivalence.rs`), `det_010` /
`det_010_awaits` vs `pattern_detector_010.sv`, `sipo_block` (BaseJump) — and `shift_register`
from its reset (`RESET` row). Sim ≡ SV only: `obs_0` itself (measured equal on every probe,
never compared by a harness), a pre-loop expression statement (probes (i), (i-b), (ii)), the
entry's realization on either route (probes (iv), (iv-b), (iv-c)). Decided: the meaning of a
non-`let` pre-loop statement, the entry as H(M)'s initial state, `obs_0` as a consequence,
the routing of the plain-`Out` startup items.

In coroutine terms (D-7, D-8): startup is the coroutine's **first resumption** — the one
resumption of an execution not preceded by a tick, from the entry to the first `tick`
reached, during the settle before edge 1 (I.0.2); the entry is H(M)'s initial state and no
tick site; `obs_0` is the trace's first observation, taken after that resumption and before
any edge (I.0.3).

**I.9.1 The first resumption.** (Restated from I.0.2, I.1.2, I.3.7, I.4.4, I.6.5; nothing
new is decided here.) Nothing executes before it: the simulator polls nothing at spawn
(`copper-sim/src/executor.rs:364-416`), and the first poll of every coroutine is the first
pre-edge settle (`:836-840`, I.0.7). The first resumption runs `s_pre` once, then the loop
head to the first `tick` it reaches, every read seeing `x_1` (I.4.4); its `RegOut` writes
take effect at edge 1 and are shown from `obs_1` (I.6.5); its register assignments are
committed at edge 1 (I.7.3); the residue at edge 1 is the first `tick` reached (I.3.7). It is
the cycle-0 instant `t_1⁻` of I.1.2 and the one segment not preceded by a trailing region
(I.3.7). Under one job (D-8) it is a resumption like every other except for where it starts.
Prost runs the same segment, entry to the first `wait`, **at the first edge after reset**
[prost §4] and SystemC while reset is asserted [systemc §4.2.2]; Copper runs it before edge 1,
and the difference is unobservable through a `RegOut` (both show the segment's writes from
`obs_1`) and visible only in `obs_0` of a wire, which a sequential module no longer has
(D-8; → I.11; D-14: I.11.2(e) — `obs_0` of a combinational wire is its first evaluation, measured).

**I.9.2 `s_pre`, the initializers and the entry.** `s_pre` is `let v = c` and `let m =
Memory(init)` only (I.2.3; `c` a constant of I.14 or `Bits::x()`; D-19: I.14.4). The first resumption reads
an initializer **once, as a constant**: for a register of I.7.2(a) it is the value before the
first assignment (I.7.4(a)), stated in the emitted text as an `initial` assignment; for the
wire of I.7.2(c) it is the value the first resumption reads before its first assignment and no
later resumption reads; for a constant of I.7.2(b) it is the value forever. `Bits::x()` is
unstated: the register is X until assigned, the emitted text says nothing, and the module is
outside the agreement relation until every such register is assigned on every path
(I.7.4(b); the `RESET` rows, → III.2; D-20: III.2.2). A memory's `init` is the memory's initial array
(→ I.10; D-16: I.10.3). The **entry** — the coroutine's state before its first resumption: the function's
start, every local at its initializer — is the **initial state of H(M)** (I.0.4): a state
that is not a tick site, whose one transition is the first resumption, and which no
execution re-enters. It coincides with a tick site's transition in no general case (the head
from the top with `s_pre` is not the head after a trailing region), so a realization **owes
an entry that runs the first resumption exactly once** — a realization obligation of III,
whatever form it takes (an entry value of `pc` or `phase_r` whose arm is `s_pre` and the head
to the first tick, Prost's RESET state [prost §4], `pc` being already many-to-one onto sites,
I.0.4; or a one-bit started carrier of I.7.2(j)); D-11 fixes the obligation and leaves the
form to Phase C's measurement. Measured 2026-11-05 (D-11): today's encodings differ from the
entry in two ways, both corpus-masked. The **extraction route** encodes the entry as state 0,
the loop head, with `s_pre`'s expression statements prepended (`control_extract.rs:526-549`):
right at edge 1, wrong at every return to state 0 (I.9.3). The **linear route** encodes it as
phase 0, the trailing textual segment folded before the head (I.3.8): at edge 1 the
`always_ff` runs a trailing region the coroutine has not executed, showing the trailing
drive's power-on operand one observation early — `loop { tick; q.write(r) }` with `r = 5`
shows `obs_1 = 5` in the SV and `0` in the simulator (probe (iv)), and
`regout_trailing_single_tick` with its accumulator initialized to 5 shows `obs_1 = 5` against
`0` (probe (iv-c)); the sweep's fixture agrees only because its register's initial equals
`init(q)`. The extraction route does not share this defect: the statements after a body's last
tick are lowered into states entered only after a tick (probe (iv-b) agrees; the entry state
runs the head alone). Corpus: 64 linear-route modules have an effectful trailing region and
**none** reads a non-zero power-on operand there (the five enum-initialized ones start at
their first variant, encoded 0), so no shipped module shows the defect; it is a **Phase C
defect of the realization** (D-11 A, sub-clause 7), not a refusal, and probes (iv) and (iv-c)
become sweep fixtures. Prior work: Prost's RESET state is the entry as a state of its own,
entered on reset and left at the first edge [prost §4]; SystemC's reset behaviour is
everything before the first `wait()`, with "reset values … statically determinable constants"
and the C++ power-on value "not part of the reset behavior" [systemc §4.2.2]; Esterel's first
reaction is a reaction like any other, every signal's previous value ⊥ [esterel §3.3, §6.1];
CλaSH's initial state is the argument of `run` [clash eq. 12], Chisel's `resetVal` a reset
branch and a register without one X [chisel §2.6, §10.2], MyHDL's simulator starts from the
initial value while its HDL is X until reset [myhdl §4.3, §6.3.1], Spade's `reset(rst: v)`
syntactic [spade §4]; Bluespec, Cummings, Anvil and RustHDL do not cover power-on
(`prior_art/bluespec.md`, `cummings.md`, `anvil.md`, `rhdl.md`: not covered). No surveyed
language runs the opening segment before the first edge with its constants stated in the
emitted HDL; no precedent for the combination. (D-13: the linear route's fold is row (f4).)

**I.9.3 Pre-loop statements that are not a `let`.** **Decided (D-11 A):** an expression
statement before the loop — a `RegOut` write, a memory staging, any statement of `s` the
grammar admits there — has its **Rust meaning**: it is a statement of the first resumption,
executed **once**, in program order before the head. A pre-loop `RegOut` write is a write of
the first resumption and takes effect at edge 1 (I.6.1, I.6.5), overridden by any later write
of the same port before edge 1 (I.6.2: the head's every-path write masks it, probe (ii)). A
pre-loop memory staging is the first resumption's staging, answered as any staging is
(→ I.10; D-16: I.10.3 — measured on both routes, probes (iv): dropped by the linear route,
re-run by the extraction route; no new row). I.2.4's "any other pre-loop statement" row is IN, no longer NOT YET; I.2.3's
`s_pre` grammar is widened to `s_pre ::= … | s_pre ; s_nt` where `s_nt` is a tick-free
statement of `s` (a `tick` before the loop is OUTSIDE: I.2.2's prefix is tick-free). Both
transpiler routes realize this wrongly today, measured 2026-11-05 (D-11; MEASUREMENTS §B):
the **extraction route** moves the statements into state 0 and **re-runs them at every
return to state 0** — every extracted module returns there each iteration — so §B's probe
(i) diverges at 10 of 24 cycles (`q` re-written to 7 whenever `pc` returns to 0); the
**linear route** `_ => {}`s them (`copper-codegen/src/chir_lower.rs:1958-1972`) and **drops
them silently**, so probe (i-b), the same write on the linear route, diverges from cycle 0
(`obs_1 = 0` where the simulator shows 7). Corpus: **two modules**, both CPUs —
`rv32i_cpu_pipelined` (extraction: `program_counter`, `halted`, `a0_out` written before the
loop and the first fetch staged, masked because the head writes the three ports on every
path and restages the fetch) and `rv32i_cpu_transpilable` (linear: the first fetch staged
before the loop, dropped, masked because its reset is held on cycle 0 and the head restages).
Both are **Phase C defects of the realization**, fixed by the entry of I.9.2, with probes (i)
and (i-b) as sweep fixtures (III.3). Option B, refusing the shape (II.1), was costed at both
CPUs and names no principle (the coroutine runs the statements); option C, a desugaring into
the head under a synthesized first-iteration register, is a second spelling of this meaning
and names a register the coroutine does not have (a started flag is a realization choice of
I.9.2, never a construct of the kernel). SystemC's rule — everything before the first
`wait()` is reset behaviour, run once [systemc §4.2.2] — and Prost's RESET arm [prost §4]
are this clause in their time models; the `control_extract.rs:526-549` comment ("state 0
runs once") states the intended meaning and the measured realization contradicts it. (D-12: II.1.2 R21 refuses only a `tick` before the loop.) (D-13: rows (f2), (f3); R21 measured — the macro admits, the linear route drops the tick, row (f6).)

**I.9.4 Observation 0.** Every output of a sequential module is a `RegOut` (D-8) and a
`RegOut` write takes effect at an edge (I.6.1), so **`obs_0(q) = init(q)`** — the zero of the
payload type (I.6.6) — **for every port of every sequential module, whatever the first
resumption writes**: a consequence of I.6.5 and I.6.6, not a clause of this section. The
first resumption's writes are shown from `obs_1`. Measured 2026-11-05 (D-11, probe (iii)): a
first resumption that writes `q` before its tick shows `obs_0 = 0` in the simulator (the
committed cell is the environment's `registered_wire` initial until an edge) and in the
SystemVerilog (the port's `initial q = 0`, `emit.rs:418-432`), under Verilator's zero-fill and
under `--x-initial unique` alike — every emitted port has an `initial`, so randomized power-on
reaches no port; a `Bits::x()` register has none and shows the randomized value from its
first observation (probe (iii-b): the `RESET` class, outside III.2 until assigned (D-20: III.2.2), and the
simulator's X cannot even be recorded by today's harness). The harness compares from `obs_1`:
the generated testbench drives, toggles the clock and then compares
(`copper-sim/src/verification.rs:255-299`), and `EquivalenceTest::record` is called after
`tick_clock` (`tests/common/mod.rs:827-856`); **Phase C observes `obs_0`** (D-1 A1) — for a
sequential module a check of `init(q)` on both sides, which is III.2's precondition stated (D-20: III.2.1)
(I.6.6). The plain-`Out` startup items this section was owed — `init(o)` at startup, the
continuous-assign transient (`loop { tick; o.write(r) }` with `r ≠ 0` differing at `obs_0`),
`out_first_driven_late` (`copper-analysis/src/lib.rs:185`) and gap 18's pre-loop `Out` write —
are **vacuous for sequential modules** (D-8: no plain `Out` in a clocked body) and go to
I.11 with the combinational module's wire (D-14: vacuous there, I.11.2(e)); `out_first_driven_late` is retired with the plain
`Out` and the eight alignment rules (D-8, `review/Q1_TRADEOFFS.md` §5), not re-derived.
Esterel's persistence law gives every output ⊥ before the first reaction [esterel §6.1] —
Copper's `init(q)` is the typed zero instead; no surveyed language observes before its first
edge (Prost's first observation follows the first edge after reset [prost §4]).

**I.9.5 State the module did not declare.** Each has its clause; this section decides none.
`pc`, `phase_r`, a hoisted cross-state local, a promoted `_r` wire, a `for` counter and a
memory pipeline stage are realization state (I.7.2(j)) and power on at zero in the emitted
`initial` block (`emit.rs:418-432`; `initial begin cnt = 0; pc = 0; q = 0; end` on probe (i));
what a realization powers its storage on at is III's (I.7.4(e)). A module whose clock has
never ticked reads pre-edge and resolving-on, so its first resumption runs during the first
settle of any domain → I.12 (I.0.8; D-17: I.12.2 — a realization defect, measured, Phase C). A `RESET` row declares that the agreement relation
covers the module from its first assignment on every path, the sweep asserting the reset on
cycle 0 (`tests/common/mod.rs:577-581`; `build.rs:162-171`, "comparing undefined behaviour
against undefined behaviour is a test of nothing") → III.2 (D-20: III.2.2, "from reset"). A memory's initial array and the
first resumption's staging → I.10 (D-16: I.10.3); what a read of an X register computes → I.14 (D-19: I.14.2(b)–(c) — closed).

**I.9.6 Implementation map.** Simulator: `spawn_untracked` / `spawn_wired` / `spawn_child`
push the future and poll nothing (`copper-sim/src/executor.rs:364-416`, `:427`); `tick_clock`
(`:813-828`) is `pre_edge_settle` (`:836-840`: phase pre-edge, resolving off, `poll_tasks`)
then `post_edge_settle` (`:845-853`: `advance`, phase post-edge, resolving on, `poll_tasks`);
the first resumption is the first `pre_edge_settle`'s poll, so `obs_0` is readable between
the two halves (how probe (iii) read it); `RegOut`'s committed cell is the environment's
initial until `on_posedge` (`copper-core/src/port.rs:173-183`, `:220-235`); the register
initializer is the Rust `let` (rustc's lowering keeps it, I.0.5(c)). Transpiler, extraction
route: pre-loop expression statements → state 0's body ahead of the head
(`copper-codegen/src/control_extract.rs:526-549`), `pc` allocated per tick occurrence with
state 0 the head (`:1208-1358`), the statements after a body's last tick in a state of their
own (`:1213-1226`), `pc` powered on at 0; linear route: the pre-loop scan takes `let`s and the
`loop`, refuses `while` and drops every other statement (`chir_lower.rs:1958-1972`),
`split_at_ticks` (`shir_lower.rs:873-883`) makes `phase_r` with the trailing textual segment
folded into phase 0 (I.3.8, I.6.9), `phase_r` powered on at 0; both: `lower_register_init`
(`chir_lower.rs:1158-1198`: folds the constant, `Bits::x()` → no init, refuses an unfoldable
or ill-fitting one), `reg_inits` (`emit.rs:418-432`: one `initial` block, each register's
constant, every `always_ff` port at zero). Analysis: `check_register_initializers`
(`copper-analysis/src/lib.rs:51-56`, both front-ends; I.7.4(c)); `out_first_driven_late`
(`lib.rs:185`, retired with D-8). Harness: `obs_0` is never compared
(`verification.rs:255-299`, `tests/common/mod.rs:827-856`). Sweep: the `RESET` table
(`build.rs:206-230`), the reset asserted on cycle 0 (`tests/common/mod.rs:577-581`);
`COPPER_VERILATOR_X_INITIAL` (`verification.rs:505-560`). Obligations carried to Phase C
(D-11): an **entry** on both routes that runs the first resumption exactly once — the
extraction route's state 0 no longer re-running `s_pre`'s expression statements on return,
the linear route lowering them and not running the trailing fold at edge 1 — its form
(entry value of `pc`/`phase_r`, or a started carrier) chosen by measurement; probes (i),
(i-b), (iv), (iv-c) as sweep fixtures (III.3), (ii), (iii), (iv-b) as agreeing twins; the
harness observing `obs_0` (D-1 A1); the `control_extract.rs:526-549` comment corrected;
`out_first_driven_late` removed with the eight rules; `review/MEASUREMENTS.md` §B extended
by the linear route's drop.

**I.9.7 Carried NOT-YET list.**
- `obs_0` in the harness — observed on both sides from Phase C on (D-1 A1; for a sequential
  module a check of `init(q)`, III.2's precondition; D-20: III.2.1).
- The §B probe and its linear twin as sweep fixtures, divergent until the entry lands
  (Phase C); the trailing-fold startup probes (iv), (iv-c) likewise.
- The pre-loop memory staging's meaning and the memory's initial array → I.10. (D-16: I.10.3 —
  closed.)
- The never-ticked domain (I.0.8) → I.12. (D-17: I.12.2 — closed.)
- What a read of a `Bits::x()` register computes before its assignment, and a pre-loop
  constant X → I.14. (D-19: I.14.2(b)–(d) — closed.)
- `RESET`-row coverage (the agreement relation from the first assignment on every path; the
  reset asserted on cycle 0) and the precondition that the emitted text states every constant
  initializer → III.2 (D-20: III.2.2, III.2.5); the simulator's X, unrecordable by today's harness (probe (iii-b)) →
  III.2 with I.14 (D-20: III.2.2 — X outside the SV pairs, reported as having left the relation). (D-19: I.14.2(f) — the X precondition stated and handed to III.2: the harness compares an X bit as 1.)
- An independent reference that exercises a pre-loop expression statement, and one with a
  trailing drive whose operand has a non-zero power-on value → III.2 (D-20: III.2.11 item 5)
  (`ANCHORING_A_MODULE.md`; the user writes references).
- The entry's realization form (entry state vs started carrier) and the proposition that the
  emitted machine's initial state encodes the entry → III.1 (D-20: P15, measured false) (I.0.8's `pc`-vs-sites
  measurement, extended to the initial state).

### I.10 Memory

Decided: D-16 (2026-11-05). Anchor: `dual_port_ram` vs `examples/memory/sv/dual_port_ram.sv`
(hand-written by the module's author — the ReadFirst collision, one-edge write visibility,
the held output across unstaged cycles; `tests/dual_port_ram_equivalence.rs`); `ram_read_first`
vs the `REFERENCE` row `tests/fixtures/reference_sv/ram_read_first.sv` (hand-written for this
repository: a read-first 1r1w RAM from the nonblocking-assignment rule, not from the
transpiler's nets); the pipelined CPU — `rv32i_cpu_pipelined` receiving a WriteFirst memory,
its 13 architectural programs cycle-exact against the transpiled core under a hand-written
owner (`tests/rv32i_pipelined_verilator.rs`, measured PASS 2026-11-05) — for the write-first
collision and the received-memory bus. Sim ≡ SV only: the preloads (`preloaded_rom_dut`), the
three latency pairs of `pipelined_ram_dut`, the write-port priority (`ram_wf_priority`), the
extracted shapes (`extracted_rom_dut`), the multi-phase shapes (`multiphase_rom_dut`), the
address-width casts (`mem_address_width_dut`), and D-16's probes (iii)–(iv). No third-party
memory reference is vendored (`bsg_mem_1r1w_sync` is named in `ram_read_first.sv`'s header
as the stronger form) → III.2 (D-20: III.2.11 item 6). Decided: the unready observe, the latency generics, the mode's
owner, the initial array, the dominated staging, rule (d), the sharing rule, the combinational
module.

In coroutine terms (D-7, D-8): a `Memory` is a variable of H(M) (I.0.4 (iii), I.3.3 (iv)) —
an array of words and, per read port, a delivery pipeline — that only an edge changes
(I.1.3). A staging is a resumption's effect on one port, consumed at the edge the resumption
ends at, as a `RegOut`'s pending value is (I.6.1); an observe reads what the last edge
delivered, as a `RegOut` is read at its committed value (I.6.4, I.11.4).

**I.10.1 The memory as a variable of H(M).** (a) **The instance.** `Memory<T, R, W, D,
READ_LAT, WRITE_LAT>`: `T` a type of I.14 (D-19: I.14.1), `R` read ports and `W` write ports indexed from 0,
`D` the domain of the clock it is given (I.10.5), `READ_LAT = L ≥ 1` and `WRITE_LAT = M ≥ 1`
— **decided (D-16 2A):** kept as parameters of the instance, one pipeline clause for every
`L`, `M`; `L = 0` is OUTSIDE (a combinational read is Chisel's `readLatency = 0` [chisel §4.3]
and no form of this document: the transpiler refuses it, `zero_latency_memory_is_rejected`,
the simulator's type does not check it — a parity item of II.1.3); `ReadMode::Async` is
retired (no setter, no meaning, 0 uses; `ReadMode` goes in Phase C). Its state: an array `A`
of `depth` words (the depth a run-time argument of the constructor; its address width → I.14; D-19: I.14.6(a) — ⌈log₂ depth⌉ bits)
and, per read port `k`, a delivery pipeline `P_k` of `L` stages each holding a word or
nothing; per write port `j`, a write pipeline `Q_j` of `M` stages each holding `(a, e)` or
nothing. In the kernel it is `let m = Memory(init)` in `s_pre` (I.2.3; `init` → I.10.3) or a
`Memory` parameter (I.10.5). It is not a local of I.7.1 and not a register (D-9): a variable
of H(M) of its own kind, which the simulator realizes as the `Arc`-shared object of
`copper-core/src/memory.rs` and the transpiler as an array with per-port nets and stage
registers (I.10.7). Corpus (D-16 §0 (i)): 20 modules in the sweep directories hold one (19
declared, 1 received), 17 at `(1, 1)`, one each at `(2, 2)`, `(3, 1)`, `(1, 2)`; every pair
transpiles and agrees.
(b) **The effects.** In the resumption that ends at edge N+1, `stage m k a` sets port `k`'s
staged address `s_k := ⟦a⟧` (an index of I.14 — D-19: I.14.6(a), a `usize` — evaluated with forwarding, I.3.4; an input in it
is `x_{N+1}`, I.4.1); `stage_write m j a e` sets `w_j := (⟦a⟧, ⟦e⟧)`. A staging is an effect
of the resumption exactly as a `RegOut` write is (I.6.1): it is not state — `s_k` and `w_j` are
consumed and cleared at the edge — and a resumption that does not stage a port stages nothing
on it. Several stagings of one port in one resumption take effect in program order and the
**last wins** — **decided (D-16 (b) B):** the port captures one address per edge, and the
earlier staging is not a dead effect but a value a later staging replaces before any edge,
as I.6.2 reads the second `RegOut` write and I.11.2(b) the second write of a wire; the
simulator's `pending_read_addr[k]` and `write_pipeline[j][0]` are overwritten by the later
call (`memory.rs:290-298`, `:353-361`) and the emitted text's later blocking assignment to
the port's address net in one `always_comb` is the same mux. An out-of-range `⟦a⟧` is a
run-time failure (→ I.14; D-19: I.14.6(b) — OUTSIDE, decided): the simulator panics at the call (`memory.rs:116-138`), the emitted
address net truncates (cause Q's cast, `vlir_lower.rs:782-830`) — outside the agreement
relation, as every run-time failure is.
(c) **The edge step** (I.0.2 (2), I.1.2: "every memory advances"). At edge N+1, in the
instance's mode order (I.10.2): the **read step** shifts every `P_k` one stage toward its
output and captures `A[s_k]` into stage 0 if `s_k` is set, else nothing, and clears `s_k`; the
**write step** takes `w_j` into stage 0, shifting `Q_j`, and commits the stage that reaches
`M-1` — `A[a] := e` — in ascending port order, so that of two ports committing one address the
**higher index wins** (decided; sim ≡
SV on `ram_wf_priority`, the rule at four ports in `tests/memory_multiport_arbitration.rs`,
simulator only; no anchor). (Phase C step 11, wording only — `review/MEASUREMENTS.md` §O.7
item 1: the earlier text committed the oldest stage *before* shifting `w_j` in, which read
literally lands a write at edge N+M+1 and contradicts the consequence below; the interpreter's
mutation of that literal order disagrees with the simulator on 8 RAMs, §O.1. The meaning — a
write in `A` from edge N+M, so at M = 1 at the very edge that consumes it — is unchanged.) Hence a word staged in the resumption ending at edge N+1
reaches the output stage at edge N+L and is observable in the resumption ending at edge
N+L+1, and — if the port is not restaged — in no later one: with nothing behind it the
output stage is empty one edge on ("the address must be re-presented every cycle", CODE
(7).14). A write staged in that resumption is in `A` from edge N+M. The step is the
listener the memory registers on its clock at construction (`memory.rs:263-264`; `:73-160`),
called by `Clock::advance` between the two settles (I.0.7): a memory advances on its own
domain's edges only (→ I.12; D-17: I.12.1 — closed).
(d) **The observation.** `observe m k` in the resumption that ends at edge N+1 yields the
pair (valid, word) of port `k`'s output stage after edge N: `is_ready()` is valid, `data()`
the word. It reads **committed state** — what edge N delivered — exactly as I.6.4 reads a
`RegOut` after edge N: a resumption never observes its own staging (I.10.4 (d)), and the
word it observes was staged `L` resumptions earlier. `q.write(m.data())` (M9) is therefore a
`RegOut` write of I.6.1 whose value is the word edge N delivered, shown from `obs_{N+1}`: for
the single-latency ROM `loop { read(a); tick; q.write(data()) }`, `obs_{N+1}(q) = rom[a_N]`,
`a_N` the address staged by the resumption that ended at edge N — the trace
`tests/cycle_dataflow_memory_derivation.rs` derives by hand and both implementations match
(m3, M10). **Decided (D-16 1A):** `data()` when valid is false is a **run-time failure** —
the program has no word to read, as at an out-of-range index — outside the agreement
relation, reported by the interpreter as `Runtime`; the guarded spelling `if is_ready() { … }`
(`dual_port_ram`, `rom_from_fn`, both CPUs) and the unconditional restaging (`rom_paced`) are
the two idioms, and rule (c) refuses the static case (I.10.4). The emitted text shows the
stage register there — zero at power-on, `mem[0]` after a drain (CODE (7).19) — a
realization value with no meaning, neither a hold nor a zero. Measured (D-16 §0 (iii)): the
one-job spelling of a 1r1w RAM reading and writing one address in one resumption (`rdw_wf`,
`rdw_rf`, 24 cycles) agrees three ways — simulator = hand model of this clause = transpiled
SV — under each mode.
Prior work: Chisel `Mem(depth, target, readLatency)`, `readLatency` 0 combinational and 1
registered, `hexInitFile` for contents [chisel §4.3]; Spade's `clocked_memory` with a fixed
set of write ports and `read_memory`, instantiated explicitly and never inferred, motivated by
the partial update of large state [spade §III-C]; Bluespec `RegFile`/BRAM — not covered by
`prior_art/bluespec.md`; CλaSH `blockRam`, Filament, Anvil (its Fig. 1 is a memory timing
hazard, not a memory semantics), Prost, Esterel, RustHDL, PyMTL3, SystemC, Cummings — not
covered; MyHDL's lists of signals become a Verilog memory array [myhdl §11.6], a realization
note. No surveyed language states a memory as a variable of a coroutine's state machine; the
staging-then-observe-after-the-edge discipline is Chisel's `readLatency = 1` and Spade's
`read_memory` stated over resumptions.

**I.10.2 Read-during-write.** **Decided (D-16 A):** the mode is a property of the memory
**instance**, declared by the builder — `Memory::new` / `from_contents` / `from_fn` is
ReadFirst, `.write_first()` and `.read_first()` set it — and nothing infers it: no macro code
does (D-16 §0 (i): 0 hits), and `memory.rs:324`'s "called by the macro when it infers
WriteFirst ordering" is stale (Phase C). Meaning: the order of the two steps of I.10.1(c).
**ReadFirst** — the read step, then the write step: a read port staging `a` at an edge where
a write port commits `(a, e)` delivers the word `A` held before the edge. **WriteFirst** — the
write step, then the read step: it delivers `e` (the highest committing port's `e` when two
commit `a`). The modes differ at such an edge only (`read_first_and_write_first_actually_differ`).
At `M > 1` the committing stage is the pipeline's last, and WriteFirst forwards from it
(`ram_r1w2_wf`). There is no third mode: an unstaged port delivers nothing (I.10.1(c)), and
the "no-change" of vendor RAM templates is not a spelling of this document. Realization:
ReadFirst is the plain continuous array read beside a non-blocking commit — the form of the
reference `ram_read_first.sv`, where it falls out of the nonblocking rule [cummings §4.0];
WriteFirst a forwarding mux from the committing stage, highest port outermost
(`vlir_lower.rs:3077-3112`). For a received memory the policy is the owner's object's and the
parameter type does not carry it (→ I.10.5). Anchor: BaseJump pins the same choice as a
parameter of the interface, `write_then_read_same_addr_p` — "the write appearing to go first",
realized by wrappers when a hard block lacks it [basejump §3, footnote 1]; the ReadFirst
reference row; the CPU under a WriteFirst owner (13 programs). The vendor read-first /
write-first / no-change templates are in no `prior_art/` note (`cummings.md` does not cover RAM
inference); a citation, if wanted, is the user's to supply. Measured: (iii) above;
`write_first_memory_equivalence` 5/5, `dual_port_ram_equivalence` 2/2. Prior work: Chisel and
Spade do not name a read-during-write policy [chisel §4.3; spade §III-C]; the others — not
covered.

**I.10.3 Initial contents and the first resumption.** **Decided (D-16 A):** the memory's
initial array `init` is a **constant array** of I.14 (D-19: I.14.4), read once by the entry (I.9.2):
`Memory::new(clk, depth)` is the all-zero array (the zero of `T`; `SPEC_OVERVIEW.md` §4 choice
6, as I.6.6 and I.7.4 apply it); `from_contents(vec![c_0, …])` the listed words;
`from_fn(|i| e)` the array `A[i] = ⟦e⟧(i)` with `e` a constant expression in the index alone.
A preload that reads one of the module's own signals, a non-inline function, or any contents
computed at run time is OUTSIDE (I.2.4; `chir_lower.rs:2000-2060` refuses the first two with
their own messages). An X-initialized memory is no form of this document (→ I.14 — D-19: I.14.6(d): a word may hold X, no constructor leaves the array unstated —, with the
`RESET` rows). `obs_0` shows no memory (no port exposes the array, I.1.4): a staging in the
first resumption is delivered at edge 1 and observable from the resumption ending at edge 2,
through a `RegOut` from `obs_2` — the pre-loop staging of I.9.3 (D-11) is the first
resumption's staging and nothing more, answered by I.10.1(c)–(d). Realization: one `initial`
block per memory — a fill loop for `new` (stated as zero since 2026-10-29, `chir_lower.rs:1930-1936`)
and for `from_fn`, one assignment per word for `from_contents` (`emit.rs:347-383`); every
delivery and write stage register powered on at 0 (`vlir_lower.rs:2977-3024`); `$readmemh`
is not emitted (the CPU's hand-written owner uses it for the program image,
`rv32i_pipelined_verilator.rs`); `COPPER_VERILATOR_X_INITIAL=1` therefore covers memories —
nothing of a memory is left unstated. BaseJump's stance — reset lines, never initializers
[basejump §3] — is the portability limitation I.7.4 states for registers, stated here for
memories too. Measured: `preloaded_memory_equivalence` 3/3 (sim ≡ SV); probes (iv), one per
route (D-16 §0): on both the simulator matches the hand model of I.9.3 with this section
(`obs_2(v) = 1`, `obs_2(q) = rom[5]`); the **linear route drops** the staging
(`chir_lower.rs:1958-1972`; the emitted text stages address 5 nowhere; diverges at
`obs_2`–`obs_3`) and the **extraction route re-runs** it at every return to state 0
(`control_extract.rs:526-549`; `rom_rd0_addr = 4'(64'd5)` restaged whenever the loop's own
staging is off; diverges at `obs_6`, `obs_10`, `obs_14` and the hold cycle after each) — the
two defects I.9.3 recorded (II.3 rows f2, f3), now measured on a staging; the entry of I.9.2
closes both, and this section adds no row. Prior work: Chisel `hexInitFile` [chisel §4.3];
MyHDL's memory initialization — not covered; BaseJump as above; the rest — not covered.

**I.10.4 The memory rules and their principles.** Rule 6 (`check_memory_staging`,
`copper-analysis/src/cfg.rs:2077-2207`; both front-ends, macro `copper-macros/src/lib.rs:999`,
transpiler `copper-codegen/src/lib.rs:531`, one message — measured on one probe per clause in
both, D-16 §0 (v); corpus refusals 0 per clause, `front_end_parity` test 1) has four clauses,
each sorted here by I.0.6's principles with its direction in II.2.2's format.

| clause | predicate | principle | direction, witness | verdict (D-16) |
|---|---|---|---|---|
| (a) | a memory access inside a tick-free nested loop (a folded node with no tick) | (b)'s case after `D`'s unrolling of the constant-bound `for` (I.2.4) | **over**: `for i in 0..1 { m.read(i) }` and a loop touching two distinct ports once each are refused | **retired** with (b), Phase C |
| (b) | two stagings of one (memory, port) that reach each other tick-free (through the loop head) | the overridden staging is an effect no observation shows — the dominated-write question D-12 D4 and D-14 A1 answered for the `RegOut` and the wire: a lint, not a refusal; the hardware realizes last-wins for free | exact on its predicate; `p_b`, `p_dom` (II.2.4's dominated staging), `two_stagings_of_one_bus_in_one_cycle_are_rejected` | **retired (D-16 B)**: the last staging wins (I.10.1(b)); the conditional and the sequence are two spellings of one meaning, both realized; corpus cost 0 |
| (c) | an observe of a port staged nowhere in the module loop (top level only; off inside a nested loop's sub-CFG) | **design** (II.2): an observation no effect feeds — valid constantly false, `data()` a run-time failure on every execution (I.10.1(d)) | **over** by one blind spot: a port staged only in `s_pre` (`p_pre_only`: refused by both, though I.9.3 gives it a meaning — valid once, in the resumption ending at edge 2); the CFG is built over the loop (`cfg.rs:287-290`); `p_c`, `an_unstaged_port_is_rejected` | **kept**, design; Phase C counts `s_pre`'s stagings |
| (d) | an observe that every staging of its port reaches tick-free | **design, one spelling** (II.2): the observe stands after the tick that delivers it; the rotation `observe; stage; tick` (`p_guarded`, admitted by both) is the same coroutine and is the spelling — D1's kind (I.8.2(b)); the refused text is the one a Verilog reader misreads as a combinational read | exact on its predicate (a mixed shape — one staging across a tick, one before the observe — is admitted and reads the delivered word, I.10.1(d)); `p_d`, `a_same_cycle_observation_is_rejected_even_when_extracted` | **kept (D-16 A)** |

Every clause ranges over the module loop's CFG: a nested tick-bearing loop contributes the
accesses before its first tick to the node that reaches it (`entry_cycle_mem`, `cfg.rs:2352`;
I.8.7's pointer) and is checked in its own sub-CFG with (c) off — the CPU's `read(addr);
loop { tick; if is_ready() { break } }` wait. A received memory is checked like a declared one
(`memory_locals`, `cfg.rs:3246`; `a_received_memory_is_checked_like_a_declared_one`). Rule 7
(`memory_result_drives_plain_out`) is retired vacuous (D-13, II.3.2) and its combinational
residue closes in I.10.6 — nothing is re-derived. Prior work: Bluespec — a rule is one atomic
parallel action and a second write of one register in one rule is a compile error [bluespec
§5]: (b)'s discipline, not taken, as D-12 D4 did not take it for the `RegOut`; Chisel `Mem`'s
one read per port per cycle is structural, a port being a node [chisel §4.3]; Spade's fixed
port set [spade §III-C]; Anvil's "for every timestamp function consistent with the graph"
quantifier is the statement of (d) over the CFG [anvil §C.4; `prior_art/anvil.md` item 4].

**I.10.5 A received memory, sharing and the bus.** A `Memory<…, D, …>` **parameter** (legal
since 2026-08-26) is a variable of H(M) the instantiating context owns: the module's effects
on it are stagings on the ports it uses, its observations are of those ports, and its domain
`D` is the module's clock domain — a memory in another domain is refused (`check_cdc`,
`copper-macros/src/lib.rs:219-247`, ui `cdc_memory_parameter`: "a memory is clocked on its own
domain and cannot be synchronized across one"; → I.12; D-17: I.12.4(b) — a memory does not cross; the dual-clock memory → I.13; D-18: I.13.8 — a NOT-YET row, a synchronizer-kind primitive with no form). It is a parameter of a `sequential`
or `synchronizer` module only (I.10.6). **Decided (D-16 A), the sharing rule:** a memory's
ports are **partitioned** among the modules that receive it — each port is staged and observed
by exactly one module — and a memory lies in one domain; two modules staging one port is
OUTSIDE: it has no meaning, because the simulator's result would depend on the poll order,
which I.0 forbids (the object is shared by `Arc` with no port ownership, `memory.rs:263`). It
is a structural shape rule that I.13 states and enforces (D-18: I.13.4(d) — one owner per memory port, refused by both front-ends on the probe; the port set and mode owed to Phase C) — the port set a module uses, and
the mode of I.10.2, carried in the parameter type (a Round 3 form); nothing enforces it today,
and no corpus module shares a memory (the pipelined CPU's owner is its testbench). **The bus
ABI** — per used port `m_rd<i>_addr` (out) and `m_rd<i>_data` (in), `m_wr<j>_en`,
`m_wr<j>_addr`, `m_wr<j>_data` (out); `M_ADDR_W` a module parameter defaulting to 1; no read
enable; the delivery pipeline child-side; the array, its preload, the continuous read and the
guarded commit owner-side; the collision policy the owner's — is a **realization** recorded
here with its date and not a meaning: `RECEIVED_MEMORY_ABI.md`, DECIDED 2026-08-27, option (a)
of its fork (ports, not a hierarchy edge); `chir_lower::received_memory_decls`
(`chir_lower.rs:1448-1510`), `vlir_lower::lower_mem_decls`, `mem_write_commits`,
`lower_to_vlir`, `mem_net_defaults`; verified by `tests/received_memory_abi.rs` and the CPU
lane. Nothing above changes if I.13 realizes the edge another way (D-18: I.13.6 — recorded as a dated realization). Its two limits are dated
rows of II.3 (→ II.3.7): `WRITE_LAT ≠ 1` on a received memory is refused by the transpiler
only (`chir_lower.rs:1489`; the macro admits it — REFUSED-one; fix: expose the committing
stage on the bus), and the mode is absent from the type (a WriteFirst child under a ReadFirst
owner is two memories; the type owes the mode, I.13; D-18: I.13.7 (6)). The sweep skips **1** module for a
`Memory` parameter (`rv32i_cpu_pipelined`; `build.rs:693-699`): the harness would have to
invent the depth and the contents, both design decisions; it is anchored outside the sweep by
its 13 programs under a hand-written WriteFirst owner (`rv32i_pipelined_verilator.rs`,
measured PASS). `ARRAY_PORT_ABI.md` (DECIDED 2026-08-24, option B: a packed 2-D port with
element select, `[ELS-1:0][WIDTH-1:0]`) is the array-**port** realization, likewise III's and
not a memory. Prior work: Spade's memory entities are "handled separately" by the compiler
with a fixed set of write ports [spade §III-C]; BaseJump's memory modules are instantiated,
one owner per port, never shared [basejump §3]; no surveyed language shares one memory port
between two processes; Bluespec's one-port FIFO arbitration is between rules of one module
[bluespec §5].

**I.10.6 Memory and the combinational module.** **Decided (D-16 A):** a `combinational`
module neither stages nor observes a memory, and `Memory` is not a parameter kind of its
header — OUTSIDE, a row of S0 (II.1.1; I.2.7 already excludes staging from the tick-free
fragment, this clause adds the observe and the parameter). Reason: an evaluation has no
resumption whose effect a staging could be (I.11.1), and an observe of a port no coroutine
stages is (c)'s case on every evaluation; the one coroutine that could stage it is another
module's, which I.10.5 refuses to share a port with. The Mealy read of a delivered word is
spelled as D-8 spells every Mealy output: the staging module writes `m.data()` to a `RegOut`
(one flop per bit, I.10.1(d)) and the `combinational` module reads that (I.11.4, I.11.5). Rule
7's residue (D-13, II.3.2) closes here: vacuous, and nothing reaches one. Measured (D-16 §0
(vi)): corpus 0; `c_obs` (a `Memory` parameter observed) and `c_stage` (staged) are admitted by
the macro — no header rule, and the combinational arm runs no memory rule — and refused by
the transpiler by accident of its registries ("method `data` is not supported in hardware
expressions"; "one-argument `.read()` on something that is not a memory read port") — a parity
break of II.1.3; Phase C gives both front-ends the header rule with one message and a ui/fail
case. Prior work: Chisel's `readLatency = 0` `Mem` is the combinational read Copper does not
have [chisel §4.3]; Spade's `read_memory` is an entity, not a function [spade §III-C]; the
rest — not covered.

**I.10.7 Implementation map.** Simulator: `copper-core/src/memory.rs` — the type (:18-35),
`from_data` registers the listener on the clock it is given (:263-264), `on_posedge` with the
mode flag (:73-91), `advance_write_pipelines` (:94-114), `advance_read_pipelines` (:140-160),
`read` / `is_ready` / `data` (:290-314), the builders (:322-335), `write` (:353-361);
`copper-core/src/memory.md` is a question list with no clause (M16) — superseded by this
section. Analysis: `memory_locals` (`cfg.rs:3246`, both spellings), `mem_accesses` (:3169),
`MemAccess` / `MemAccessKind` (:53-90), `check_memory_staging` (:2077-2207), `entry_cycle_mem`
(:2352), `staged_ports` (:2365), `stages_memory` (:198; M8's reporting); the macro's call
(`copper-macros/src/lib.rs:999`), its parameter allowlist (:1455-1480) and CDC arm (:219-247);
the transpiler's call (`copper-codegen/src/lib.rs:531`), `has_hardware_signature` (:596-610).
Transpiler: `parse_memory_decl` with the builder (`chir_lower.rs:5732-5742`), the declaration
(:1914-1936), the preloads (:2000-2060), `received_memory_decls` (:1448-1510),
`parse_mem_port` (:3357-3375), the word-indexed array register lowered as an internal
one-write-port memory (:1708-1801; I.7.2(h), II.3 row (c)); SHIR: stagings take the edge form
(`shir_lower.rs:497-511`) and a staging-only branch's enable too (:405-431), `lower_mem_init`
(:1839); VLIR: `lower_mem_access` with cause Q's address cast (`vlir_lower.rs:782-830`),
`MemPortUse` / `mem_binding` (:2520-2620: the `data_comb` / `data_reg` split by consumer
position, a two-job form), the pipelines (:2913-3024), `lower_mem_decls` / `read_net_value` /
`mem_write_commits` (:3024-3150), `forward_ff_mem_index` (:3151),
`reject_memory_driven_comb_outputs` (:1465; rule 7's copy); emission `mem_decls` /
`mem_inits` / `mem_read_assigns` (`emit.rs:316-400`): `logic [w-1:0] m [0:depth-1]`, no
`ram_style` attribute and no lint pragma. Sweep: `Kind::Memory` (`build.rs:607-610`,
`:693-699`, `:766-767`, `:1119`, `:1168`), the four `REF_SKIP` memory rows (`:239-256`), the
CPU's `SKIP` row (`:59-66`), the `rv32i_cpu_transpilable` `RESET` row. Interpreter:
`copper-ref` defers a `Memory` parameter (`module.rs:199-201`) and a binding or port access
(`:527-535`). Pins: `cfg.rs` — 11 rule-6 tests (:7195-7406) and 5 rule-7 tests (:7433-7511);
`copper-codegen/tests/unsupported_constructs.rs` — 15 memory cases; the equivalence tests
`dual_port_ram`, `write_first_memory`, `preloaded_memory`, `pipelined_memory`,
`extracted_memory`, `multiphase_memory`, `received_memory_abi`, `rv32i_pipelined_verilator`;
`cycle_dataflow_memory_derivation.rs` (m3); `ui/pass/memory_parameter_ok`,
`ui/fail/cdc_memory_parameter`, `ui/fail/memory_result_drives_plain_out`. Obligations carried
to Phase C (D-16): (1) **the interpreter's memory model** — I.10.1(a)–(d) and I.10.2's order
and nothing more: an array, per read port an optional staging and an `L`-stage delivery
pipeline, per write port an `M`-stage pipeline, `edge` in the mode's order, `observe` a
`Runtime` failure when not valid; the four `REF_SKIP` memory rows deleted and `Deferred`
narrowed to the parameter (whose storage the harness cannot supply, I.10.5); (2) clauses (a)
and (b) and their pins (`two_stagings_of_one_bus_in_one_cycle_are_rejected`, the two
`two_accesses_*` cases of `unsupported_constructs.rs`) deleted, (c)
extended to `s_pre`, (d) kept with its message citing this clause; (3) the header rule of
I.10.6 in both front-ends, one message, a ui/fail case; (4) `ReadMode` deleted, `L ≥ 1` checked
where the simulator's type is built (parity with the transpiler's refusal); (5) `memory.rs:324`'s
inference comment corrected; (6) the pre-loop staging — the entry of I.9.2 (rows f2, f3),
probes (iv) as sweep fixtures; (7) `WRITE_LAT ≠ 1` on a received memory as a II.3 row with the
macro's refusal, and the mode owed to the parameter type (II.3.7, I.13; D-18: I.13.7 (6)); (8) the
`data_comb` / `data_reg` binding reduced to the delivered register when the trailing fold goes
(I.6.9, I.10.1(d)); (9) rule 7's code, ui case, `cfg.rs` tests and the vlir RULE pin deleted
(II.3.2).
(Phase C step 11, 2026-11-07, read off the tree at `924793e`; the line numbers above are the
2026-11-05 snapshot and are kept as history.) **Done:** (1) the interpreter's memory model (step 9,
`review/MEASUREMENTS.md` §O.1 — 18 memory modules on three legs; `Deferred` narrowed to the
parameter, `copper-ref/src/module.rs:223-225`, and a `combinational` memory refused, `:195-197`);
(8) the binding is the delivered register only (`data_reg`, step 4, §H.5 item 5); (9) rule 7's code,
ui case and pins deleted (step 3, §G.4). **Open (III.3's ledger lines I.10.1, I.10.4–I.10.6):**
(2) rule 6's clauses (a) and (b) still refuse (`check_memory_staging`, `cfg.rs:491`, its "accessed 2
times in one cycle" arm) and (c) does not count `s_pre`'s stagings; (3) no I.10.6 header rule in
either front-end; (4) `ReadMode` is still a field (`memory.rs:13`, `:38`, `:215`) and the
simulator's type checks no `L ≥ 1`; (5) the inference comment at `memory.rs:323-325` stands;
(6) no memory pre-loop-staging fixture (the entry itself landed, §J.1); (7) `WRITE_LAT ≠ 1` on a
received memory refused by the transpiler only. Moved since the snapshot: `check_memory_staging`
`cfg.rs:491-650`, `mem_accesses` :1644, `memory_locals` :1721; the macro's call
`copper-macros/src/lib.rs:295`, the parameter allowlist and the CDC arm now
`copper-analysis/src/front_end.rs` (`check_cdc` :188); `parse_memory_decl` `chir_lower.rs:6640`,
`received_memory_decls` :1564; `lower_mem_access` `vlir_lower.rs:611`, `forward_ff_mem_index`
:2361; `reject_memory_driven_comb_outputs` and `ui/fail/memory_result_drives_plain_out` gone.

**I.10.8 Carried NOT-YET list.**
- `Memory` in the CDC rule, and a memory crossing domains (a dual-clock RAM has no form) → I.12. (D-17: I.12.4(b) — closed; the dual-clock memory is a NOT-YET row of I.13. D-18: I.13.8 — carried with its direction.)
- The shared-memory structural form: the port set and the mode in the parameter type, the
  one-owner-per-port rule enforced, the bus ABI as a hierarchy realization → I.13. (D-18: I.13.4(d), I.13.6 — closed; the port set and mode → Phase C.)
- X in a memory (an X address, a word of X, an X-initialized memory), the address width, the
  out-of-range address (the simulator panics, the emitted text truncates by cause Q's cast) and
  a port index `k ≥ R` (a run-time panic today, CODE (7).2) → I.14. (D-19: I.14.6 — closed: the address width ⌈log₂ depth⌉, an out-of-range address OUTSIDE, `k ≥ R` a shape rule, X per word.)
- The pre-loop staging's realization → Phase C (II.3 rows f2, f3; I.9.6).
- Latency above 1, the write-port priority and the preloads are sim ≡ SV only; an independent
  reference for a RAM with a write (`bsg_mem_1r1w_sync`, named in `ram_read_first.sv`'s header
  and not vendored) and for a two-cycle read → III.2 (D-20: III.2.11 item 6) (`ANCHORING_A_MODULE.md`; the user writes
  references).
- The interpreter's memory leg and its guard count (G-F) → III.2 (D-20: III.2.12 (5)).
- `obs_0` of a memory module is `init(q)` (I.9.4), observed by the harness in Phase C → III.2 (D-20: III.2.1).

### I.11 Combinational modules

Decided: D-14 (2026-11-05). Anchor: `bsg_mux_one_hot` vs BaseJump `bsg_mux_one_hot.sv`
(192 vectors — every select pattern over 24 random data rounds; sim ≡ transpiled SV ≡
reference) for the evaluation as a function and the write of a value accumulated in a local;
the Mealy composition — a `RegOut`-writing core beside a `combinational` reader,
`ready = ~full | drain` — vs an independent hand-written Mealy machine (D-14 probe (v), 64
cycles, sim ≡ hierarchy SV ≡ reference); `vr_sink_mealy` → B with a boundary
(`review/Q1_TRADEOFFS.md` §4 (d'), the Mealy hardware at no cell cost). Sim ≡ SV only: the
default-then-override shape (the six `cdo_*` sweep cases) and `obs_0` of a wire (probe (vi)).
Decided: the dead write (admitted, last-write-wins), the principle D2 cites and its
placement (design, II.2), the never-written port (refused), the combinational cycle (outside,
refused statically in Phase C).

In coroutine terms (D-7, D-8): a `combinational` module is not a coroutine of I.0.1 but a
**function** the simulator evaluates by one — `loop { body; delta_yield().await }` resumes
the body once per settle; the body has no `tick`, no suspension site of its own and no
state, and its effects are the values it leaves on its `Out` wires (I.0.8's last item).

**I.11.1 The combinational module as a function.** A `combinational` module instance is the
tick-free fragment of the kernel (I.2.7): `s` without `tick`, `loop`, `break`, `continue` or
memory staging, over inputs `x`, plain outputs `o` and locals `v`. Its meaning is a function
`f` from the settled values of the wires it reads to the values of its `Out` wires. An
**evaluation** runs `body` once, with the Rust meaning I.2 gives it: every read yields the
settled value of its wire at that point of the settle (I.1.3, I.4.1), every write sets an
`Out` wire (I.11.2), and nothing survives it — every local is a wire of that evaluation
(I.7.2(e)) and none is a register, there being no `tick` to be live at (I.7.1, I.7.2). The
wire's value is therefore defined at every point in time as `f` of the current settled
values of its reads: the hardware is continuous, and the simulator evaluates `f` whenever a
settle reaches the module (I.11.6). The macro's `loop { body; delta_yield().await }` is the
simulator's realization of "evaluated at every settle": the wrapper's yield is the one
suspension of that coroutine and is **not** a suspension site of the kernel — the user
writes no `await` (I.2.7) — and the number of evaluations a settle performs (one under the
levelized scheduler, one per delta pass under the fixpoint oracle) is a scheduler artifact,
unobservable (→ I.13; D-18: I.13.3 — measured 20 vs 40 evaluations, traces identical). A `combinational` module has no instant of its own — I.1.2 numbers the
instants at which a *sequential* module's statements execute — and is evaluated at every
settle: the one before each edge, after the environment's drive (I.1.5), and the one after
it (I.0.2 step 3); `obs_N(o)` is `f` applied to the settled values after edge N (I.1.4).
Prior work: CλaSH — "pure functions are a perfect match for combinational circuits"
[clash §II-F], the combinational part of a Mealy machine [clash eq. 10]; Spade's `fn` unit is
combinational only [spade §II]; Chisel's elaborated graph is combinational by default, an
output a function of the cycle's input [chisel §2.4]; Esterel's reaction is instantaneous,
one value per signal per instant [esterel §1.4, §3.3]; Cummings' `always_comb` is
re-evaluated on any change of what it reads, blocking assignments giving single-pass
forwarding [cummings §5.3]; MyHDL's `@always_comb` infers its sensitivity from its reads
[myhdl §6.2.1]; Bluespec has no combinational module kind — rules and methods are evaluated
combinationally over the current state, a method's `rdy` a Mealy function of it [bluespec
slides 11, 16] — not covered as a module; Prost names "modeling pure combinational circuits
via regular functions" as a goal [prost §5] — not covered.

**I.11.2 The wire rules.** (Taken from I.5, re-derived for the wire of one evaluation.)
(a) **The write.** `write o e` in an evaluation sets `o`'s wire to `⟦e⟧` — every local as
last assigned earlier in the same evaluation (there is no earlier generation to carry) and
every input at its settled value (I.11.1). (b) **Last write wins.** Several writes of one
port in one evaluation take effect in program order; the wire's value at the end of the
evaluation is the last write executed on the path taken. Anchored for the
one-write-per-path shape (`bsg_mux_one_hot`, whose `for` accumulates into a local and writes
once; the BaseJump comparators and encoders); sim ≡ SV for a port written more than once on
a path — the default-then-override shape (`cdo_default_then_override`,
`cdo_override_then_default`, `cdo_nested_overrides`, `cdo_match_default_arm`,
`cdo_for_override`: six sweep cases, measured 2026-11-05) and the straight-line pair of probe
(ii). Every surveyed language's rule [cummings §15.3; chisel §2.6 — a static driver priority
that shares the form; myhdl §12.2.2; rhdl §3; prost §5]; Spade refuses the shape by a linear
type [spade §IV]. (c) **The dead write — decided (D-14 A1).** A write that a later write of
the same port on the same path overrides is in no observation; it is **admitted**, and the
wire's meaning is the value the evaluation leaves on it. The design principle's "every
effect is observable" (I.0.6) is read as II.2.1 D4 reads it for a `RegOut`: the effect of a
write is the port's value at the end of the evaluation, and that value is always observed
(`obs_N`, I.1.4); a rule refusing a write dominated by a later one is a lint, not a semantic
refusal, and was declined for the `RegOut` at a cost of 0 (D-12 §0 (iv)). Measured:
default-then-override is the idiom (five ports in the corpus, all in the §10.12 fixture; 0
double top-level writes); `cdo_override_then_default` — the conditional write dead on every
evaluation — agrees and would have been the one refusal. I.5.5's combinational case,
II.2.4's question and the combinational residue of `multi_write_collapse` close here. What
the realization owes: a port written twice at the top level of the body (`o.write(x);
o.write(y)`) is emitted as two continuous assigns — `lower_flat_stmts` sends every top-level
drive to `output_assigns` (`copper-codegen/src/vlir_lower.rs:736-741`) — which Verilator
refuses as MULTIDRIVEN (probe (ii)): admitted by both front-ends, meaning defined, text
unbuildable — a II.3 row (→ II.3.7, I.11.7), fixed by routing every write of a port written
more than once into `always_comb` in program order, the §10.12 routing
(`nested_port_drives`, `:265`) widened from "written unconditionally and inside a branch" to
"written more than once". (d) **No hold.** A wire has no value a write of this evaluation
did not give it: I.5.4's hold is a `RegOut`'s (I.6.3), and definite assignment is the wire's
rule (I.11.3); a combinational module's output does not hold (I.5.7). (e) **The initial
value.** A combinational wire has no initial value of its own: at every point in time its
value is its evaluation's. The environment creates every port wire at the zero of its
payload type (`wire(zero)`, I.5.6's precondition, → III.2; D-20: III.2.3) and no observation shows that
value: `obs_0` is taken after the settle at `t_1⁻` (I.1.4), which evaluates every
combinational module. Measured (probe (vi)): the simulator's wire shows the creation zero
only before its first settle, the emitted `assign` shows `f` from time 0, and `obs_0` is
`f(x_1)` on both sides. D-11's startup items — `init(o)` at startup, the continuous-assign
transient (`loop { tick; o.write(r) }`), `out_first_driven_late`'s class, gap 18's pre-loop
write — are **vacuous** here as they were for the sequential module (I.9.4): a combinational
module has no edge for a first write to follow and every evaluation writes every port
(I.11.3). A write of `Bits::x()` or `Logic::X` to a wire → I.14. (D-19: I.14.2(e) — admitted, the wire is X.)

**I.11.3 Definite assignment.** Every path of `body` writes every `Out` of the module. The
clause this document supplies, which II.2 D2 cites: a wire's value is a function of the
evaluation's reads and nothing else (I.11.1), so a path that leaves a port unwritten gives
it **no value** — there is no previous evaluation's value to keep (I.11.2(d)) and no state
to hold one. The spelling of a value kept across evaluations is state, and state is a
register of a sequential module shown through a `RegOut` (I.6, I.7): refusing the unwritten
path is **one spelling per meaning** — the design principle — and D2 stays in II.2 (D-14
A2; II.2.1 D2's sentence stands, this clause being what it cites). The alternative, a sixth
shape clause "its wires are written on every path", was not taken: `SPEC_OVERVIEW.md` §5's
shape sentence names the coroutine's suspensions, paths, state, return and outputs, and a
combinational module fails none of them — it is refused for spelling state the wrong way.
The refusal is a latch in the hardware reading (Cummings' incomplete assignment
[cummings §5.3]; PyMTL3's CheckInferredLatchPass lint [pymtl3 §4]; SystemC's complete static
sensitivity "to avoid accidental latching" [systemc §4.1.1]; Spade's conditionally-set wire
"a latch could be inferred", refused by linearity [spade §IV]; Chisel's undriven wire an
error [chisel §2.4]; RustHDL's "assign default first" idiom plus a Yosys lint [rhdl §3]),
and Copper's message says "would infer a latch"; the meaning is the clause above, not the
hardware's. **The never-written port — decided (D-14 A3):** a declared `Out` no path
writes is the degenerate case — no evaluation gives it a value — and is **refused in both
front-ends** with D2's message. Today both admit it (`check_definite_assignment` flags only a
port written on some path but not all, `copper-analysis/src/cfg.rs:2514`; its `TODO` at
`:1366` names a submodule as the possible driver) and Verilator refuses the emitted undriven
output (UNDRIVEN under `-Wall`; probe (iii)) — a module that compiles and does not build,
the parity II.1.3 forbids. A port a child module drives inside a `combinational` module is
wired, not written, and is I.13's (D-18: I.13.5 — closed, no exemption: instantiation is a structural-only form) (hierarchy inside a combinational body; no corpus
instance). Measured: 0 refusals over the 39 corpus modules; the ui trio
`combinational_latch`, `combinational_latch_second_port`, `combinational_latch_two_ifs`
refused and `combinational_mux_valid` admitted; `comb_definite_assignment_parity.rs` 2/2; 0
never-written ports. The folded loop (II.2.2: a `for` is taken to write every port) stays an
*under* direction, pinned in Phase C with the never-written case.

**I.11.4 Reads and the settle.** An evaluation reads inputs the environment drives and
wires other modules drive, each at its settled value at that point of the settle (I.4.1): a
`RegOut` of a sequential module at its committed value, `obs_N(q)` after edge N (I.6.4,
I.6.8 — a source of the settle, never a combinational dependency; I.4.8's D-8 bullet is
answered: readable, at the committed value); another `combinational` module's wire at the
value that module's evaluation left — after its producer, which the settle orders
(→ I.13; D-18: I.13.2(a)–(b)); a memory result — none: a combinational body stages nothing (I.2.7; rule 7's
residue, retired as vacuous by D-13, is I.10's to re-derive only if a memory ever reaches
one; D-16: I.10.6 — none does, and a memory on a `combinational` module is OUTSIDE). **The combinational cycle — decided (D-14 A4):** two or more combinational modules
whose reads reach each other with no `RegOut`, memory or synchronizer between them are
**outside the language** — a shape refusal: every wire is a function of the registers and
the inputs (I.0.5(b): "realized combinationally iff a function of the registers and the
inputs"), and in a cycle the settle is a fixpoint problem, not a function — its value, if
there is one, depends on the iteration and on the data (Esterel's argument for rejecting a
cyclic dependency even when the solution is unique [esterel §7]). Today the simulator
detects the cycle structurally (`comb_cycles`, `copper-sim/src/executor.rs:490`), iterates a
convergent SCC to a fixpoint and panics on a non-convergent one (`iterate_scc`, `:577-632`;
`LEVELIZED_SCHEDULING_SCOPE.md` phase 5, "detection is not rejection"), and the transpiler
emits the `always_comb`/`assign` network as written, which Verilator refuses as UNOPTFLAT
under `-Wall` (probe (iv-b): the convergent `or_gate` + `buf` latch): a convergent cycle has
therefore never been in the sweep and has no anchor. This clause **reverses** item 6 phase
5's policy (2026-07-30) at a corpus cost of 0 — no example wires two combinational modules
into each other; the two sim-only test designs (`levelized_differential`'s set-dominant
latch, `simulator_corner_cases`' inverter pair) become refusal pins in Phase C, which refuses
the cycle statically at spawn from the same SCC computation, in the simulator and over the
hierarchy the transpiler emits (→ I.13 for the wiring it is checked over; D-18: I.13.7 (4)), with a message
naming the cycle's modules; until then the simulator's iteration and panic are realization
behaviour outside the agreement relation (→ III.2; D-20: P5's open obligation). Refusing is Prost's position ("not
expressible" [prost §5]), PyMTL3's static schedule [pymtl3 §5], Arch's "no combinational
loops — static DAG" [arch §7.3.1], RHDL's "feedback loops must be broken by registers"
[rhdl §3] and Esterel 1992's [esterel §7]; admitting it is an event or fixpoint semantics —
Verilog's queue [cummings §5.4], SystemC [systemc §5], RustHDL's capped iteration [rhdl §4],
Spade's `decl` [spade §IV] — none of which gives the wire a value as a function. What I.11
owes I.13 (D-18: I.13.2(a) takes it): an evaluation is a function of its reads and writes only its own `Out` wires, so
in an acyclic network the settle's result is independent of the order and the number of
evaluations — the fact I.1.3's order independence rests on for combinational modules; the
order itself, the wiring and the dirty-driven re-evaluation are I.13's. (D-18: I.13.2, I.13.3 — closed.)

**I.11.5 Composition with a sequential module: the Mealy output.** D-8's rewrite of a Mealy
output is a `combinational` module reading the sequential body's `RegOut`s and the input
(`SPEC_OVERVIEW.md` §2; `review/Q1_TRADEOFFS.md` §4 (d')). Its meaning by I.11.1 and I.11.4:
the wire's value is `f(q, x)` of the current settled values; at `obs_N` that is
`f(obs_N(q), x_N)` — the `RegOut` as committed at edge N and the input the environment
drove before edge N and holds through it (I.1.4, I.1.5); after the environment's drive of
`x_{N+1}` and the settle before edge N+1 it is `f(obs_N(q), x_{N+1})`, which is what a
sequential module reading the wire in its resumption sees (I.4.1) and what the hardware
shows between the drive and the edge. A drive at any other time is outside the language
(I.1.5). Anchored: `mealy_core` (a `RegOut` `full` toggled by `x`) with `mealy_ready`
(`ready = ~full | drain`) vs an independent hand-written Mealy machine (`always_ff` toggle,
`assign ready = ~f | drain`), 64 cycles, sim ≡ hierarchy SV ≡ reference, every `obs_N(ready)`
equal to `~full_N | drain_N` (D-14 probe (v)); `vr_sink_mealy` → B with a boundary
(`q1_b_vr_sink_core` + `q1_b_vr_ready`, 54/17 cells — the Mealy hardware at no cell cost,
`Q1_TRADEOFFS` §4 (d')). Chisel's `io.enq_rdy := !is_full` [chisel §2.4] and Bluespec's
method `rdy` [bluespec slide 11] are this composition with the boundary implicit; Prost
cannot write it [prost §5]; Spade's output expression over a `reg` is the same function with
the boundary at the unit [spade §3].

**I.11.6 Implementation map.** Macro: `wrap_combinational` (`copper-macros/src/lib.rs:853-868`)
wraps the pristine body in `loop { body; ::copper_sim::delta_yield().await }` and marks it
`async`; `validate_hardware_fn` refuses an `async` combinational fn (`:1411`) and a `Clock`
parameter (`:1506`); the combinational arm (`:1371-1384`) runs `check_definite_assignment` on
the pristine fn and then `wrap_in_module`. Simulator: `DeltaYield`
(`copper-sim/src/lib.rs:70-81`: Pending once, Ready on the next poll) — one evaluation per
poll; the levelized settle polls a singleton SCC once (`poll_task_once`,
`copper-sim/src/executor.rs:546-558`) and iterates a multi-task SCC (`iterate_scc`,
`:577-632`; `comb_cycles` `:490`); the fixpoint oracle polls every task per delta pass until
no dirty flag; `Out::write` sets the shared cell and the dirty flag
(`copper-core/src/port.rs:86-92`), `In::read` clones the live cell (`:120-124`); the dirty
flag is what re-evaluates a consumer (→ I.13; D-18: I.13.3 — a scheduler artifact). (D-22: `iterate_scc`
is deleted — a multi-task component is refused at spawn by `refuse_comb_cycle` under both schedulers,
Phase C step 10, §P.3; the levelized settle's multi-task branch is unreachable.) Analysis: `Cfg::build_combinational`
(`copper-analysis/src/cfg.rs:332`: `head → body → exit`, no back-edge),
`check_definite_assignment` (`:2514`, MAY − MUST at the exit; `lib.rs:274` returns `Ok` on a
clocked body). Transpiler: `check_definite_assignment` at `copper-codegen/src/lib.rs:371`;
`lower_comb` (`vlir_lower.rs:256-271`) — `nested_port_drives` keeps a port's unconditional
write in `always_comb` when the port is also written inside a branch (§10.12),
`lower_flat_stmts` (`:699-752`) sends every other top-level drive to a continuous `assign`
and every `if`/`match`/`for` into `always_comb`, `hoist_branch_local_defaults`,
`check_no_latches` (`:2384`, defence in depth over the lowered statements);
`emit.rs::comb_body` (`:185-197`) prints the wire declarations, one `always_comb` block and
the assigns. Sweep: a combinational module is driven by its whole truth table up to 8 input
bits, else 64 random vectors per seed plus the pattern list (`build.rs:256-262`), the
testbench driving, calling `eval()` and comparing (`copper-sim/src/verification.rs:263-276`).
Pins: `copper-codegen/tests/comb_definite_assignment_parity.rs`, the sweep's `cdo_*` cases,
ui `combinational_latch*` and `combinational_mux_valid`,
`tests/randomized_combinational_equivalence.rs`, the BaseJump examples;
`executor::tests::comb_cycles_*`, `tests/levelized_differential.rs`
(`diff_convergent_comb_loop`), `tests/simulator_corner_cases.rs`. Obligations carried to
Phase C (D-14): the never-written port refused by `check_definite_assignment` in both
front-ends (one message, a ui/fail case; the folded-loop and never-written directions
pinned, II.2.2); the double top-level write routed into `always_comb` (the II.3 row of
I.11.2(c), probe (ii) as its fixture); the combinational cycle refused at spawn by the
executor from `comb_cycles` and over the transpiler's hierarchy emission, the three cycle
tests rewritten as refusal pins, `LEVELIZED_SCHEDULING_SCOPE.md` noted;
`front_end_parity.rs` running `check_definite_assignment` over the combinational corpus
(II.1.3); every D2 message citing this clause.

**I.11.7 Carried NOT-YET list.**
- The settle, the wiring, the dirty-driven re-evaluation and order independence → I.13 (D-18: I.13.2–I.13.3 — closed)
  (I.11.4 states what this section contributes: an evaluation is a function of its reads).
- Hierarchy inside a `combinational` module (`SHIRCombBody::submodules`; a port a child
  drives, exempt from the never-written-port rule as wired) → I.13; no corpus instance. (D-18: I.13.5 — closed; instantiation is a structural-only form.)
- A memory reaching a `combinational` module (rule 7's retired residue) → I.10. (D-16: I.10.6 —
  OUTSIDE; closed.)
- Types and X in a wire — `Bits::x()` written to a port, a read of an X input → I.14 (D-19: I.14.2(b), (e) — closed); the
  transpiler's emission of `x[0]` on a `Bits<1>` port as a select on a scalar `logic`
  (probe (v)'s first draft, refused by Verilator) → I.14 / Phase C, a width-1 defect. (D-19: I.14.5(c) — measured on the sequential and the combinational spelling; I.14.7 (4).)
- The static cycle refusal → Phase C (I.11.6) and II.1 (a shape row beside S5 when the rule
  lands; D-14).
- `obs_0` of a combinational wire observed by the harness, and the precondition that every
  port wire is created at its payload's zero → III.2 (D-20: III.2.1, III.2.3).
- The double top-level write → II.3 (the row I.11.2(c) describes; II.3.7).

### I.12 Synchronizers and clock domains

Decided: D-17 (2026-11-05). Anchor: `sync_2ff_concrete` vs `examples/cdc/sv/sync_2ff_ref.sv`
(`tests/cdc_synchronizer_anchor.rs`: one destination cycle of latency through two flops, the
source rate irrelevant, a pulse that stands at no destination edge dropped) and the dual-clock
hierarchy vs `examples/cdc/sv/two_domain_hierarchy.sv` (`tests/two_domain_hierarchy_cdc.rs`,
2:1). Decided, not anchored: the never-ticked domain (I.12.2 — a flop samples at its own edge,
measured on the transpiled probe; no independent two-domain reference covers the startup), the
two-flop rule's statement, the unit domain's retirement, the per-bit meaning of a multi-bit
crossing, the combinational module's one domain.

In coroutine terms (D-7, D-8): a module's coroutine is resumed by the edges of its one clock's
domain and by nothing else; a synchronizer is such a coroutine whose reads include a wire
another domain's coroutine drives, read at the synchronizer's own instants; the environment is
any interleaving of the domains' ticks, including none.

**I.12.1 The clock domain as the unit of time.** A domain `D` is a type implementing
`ClockDomain`; a module instance belongs to the domain of its one `Clock<D>` parameter
(I.2.2), and every cycle, edge, instant and observation of I.1 is that domain's: edges numbered
per domain from 1 (I.1.1, D-1 A2), the instant `t_{N+1}⁻` of I.1.2 and `obs_N` of I.1.4
indexed by `D`'s edges. The coroutine of a module in `D` is **resumed by `D`'s edges only**
(I.0.2): a tick of another domain runs none of its statements, commits none of its `RegOut`s
(I.6.1) and advances none of its memories (I.10.1(c)). **The environment** of a design with
domains `D_1 … D_k` performs the three operations of I.1.5 on each domain, and the ticks of
different domains may be interleaved in **any** order: a sequence in which one domain ticks ten
times per tick of another, one in which it ticks once, and one in which it never ticks are all
environments. No relation between two domains' edge sequences is assumed (I.1.1) — no ratio, no
phase, no ordering. The trace of a module in `D` is therefore a function of the input stream
`x_N` standing on its inputs at `D`'s instants (I.1.5) and of nothing else; the interleaving
enters a trace only through the values foreign-driven wires hold at those instants (I.12.3),
and a module none of whose inputs another domain drives has one trace under every
interleaving. Measured (D-17 (vi)): `two_domain_counter` under two interleavings of twenty fast
and ten slow ticks gives the same fast-domain trace per fast edge, and the same slow-domain
trace per slow edge because the flag standing at each slow edge is the same. The simulator's
`tick_clock::<D>` call is one tick of `D` (I.1.6); the testbench's sequence of calls is the
interleaving.
**Decided (D-17 A1): the unit domain is retired.** `()` — the default `D` of `In<T, D = ()>`
and `Out<T, D = ()>` (`copper-core/src/port.rs:79`, `:109`), which implements no `ClockDomain`
and which `check_cdc` treats as domain-agnostic (`copper-macros/src/lib.rs:145-176`) — names no
domain: every port and every clock of a `#[hardware]` module names a domain, and a port written
`In<T>` is OUTSIDE (S0, II.1.1; the rows of I.2.4 and I.2.8 close). A `combinational` module
has no clock and no domain of its own (I.11.1), but its ports carry one and all of them carry
the same one (I.12.4(c)): the module lies in the domain of its ports. Corpus (D-17 (iii)): no
port in `examples/`, `src/`, `tests/fixtures/` or `tests/*.rs` uses the unit domain (measured
0); the thirteen sites that do are the executor's own unit tests (`copper-sim/src/executor.rs`,
four helper functions) and the macro's `function_typed_*` ui cases — scaffolding, given a
domain in Phase C when the default is removed. (D-22: **wrong** — D-17 (iii) counted the explicit `()`
only; the default reached **32 corpus `combinational` modules, 100 ports** — 10 in `examples/`, four of
the five BaseJump combinational anchors among them, 16 in `tests/fixtures/`, 6 in `tests/*.rs`
(`review/MEASUREMENTS.md` §P.2). Option (A1): each is now generic over its domain, `fn m<D:
ClockDomain>(a: In<T, D>, …)`, and lies in the domain of its ports (I.12.4(c)); its SystemVerilog is
byte-identical (§Q).) Prior work: a phantom domain on the signal type
is RHDL's [rhdl §2.2] ("a Signal type indicates the clock domain it belongs to"; crossings
through library constructs) and Arch's domain-tagged `Clock<D>` [arch §3.4]; Arch runs domains
on separate threads "synchronizing only at CDC crossings" [arch §7.3.4] with no model of
multi-domain time; Filament has one domain [filament fn. 2]; Prost one clock, multi-clock
unsupported [prost §5]; CλaSH is single-clock with multi-clock "work in progress" [clash §I];
Chisel has an implicit single clock [chisel §2.5]; Spade one explicit clock per register
[spade Listing 1]; Bluespec's semantics has no clock at all [bluespec slide 14]. Clash's
`Clock dom` type, Chisel's `withClock` and SystemC's domain splitting are not covered by the
notes.

**I.12.2 The never-ticked domain.** Decided (D-17 A2), from §1 of `SPEC_OVERVIEW.md` (the
clock resumes the coroutine): **a module runs only at its own domain's instants.** Before
`D`'s first edge the coroutine of a module in `D` has executed nothing — not `s_pre`, not the
loop's head — so its `RegOut`s show `init(q)` (I.6.6) and its memories their initial arrays
(I.10.3); its first resumption is `D`'s `t_1⁻`, run in the settle before `D`'s edge 1 after the
environment's drive of `x_1` (I.0.2, I.9.2); `obs_0` of a module in `D` is defined once `D` has
been asked to tick (I.1.4: the observation after the settle at `t_1⁻`). A domain the environment
never ticks has no trace, and the modules in it hold their initial state for the whole run —
observable through their `RegOut` wires, as `init(q)`, by a synchronizer in another domain
(I.12.3). **Today's simulator does not realize this** (measured, D-17 (i)): `poll_tasks` polls
every task in every settle of every domain (`copper-sim/src/executor.rs:520-575`, `:836-853`),
the thread-local phase and resolving flags of a domain with no entry default to pre-edge and
resolving-on (`copper-sim/src/lib.rs:47-51`, `copper-core/src/types.rs:979-983`), and nothing
is polled at spawn (`executor.rs:364-416`) — so the first resumption of a module in `B` runs in
the **first settle of any domain**. With `x = 1` standing, three ticks of `A`, then `x = 2`
driven and one tick of `B`, a module `loop { let t = x.read(); q.write(t); tick }` in `B` shows
`q = 1` after `B`'s edge 1, its side-effect counter having advanced to 1 at `A`'s first tick and
to 2 at `B`'s; the transpiled module under Verilator shows `q = 2` (a flop samples what stands
at its own edge), and so does the simulator when `A` never ticks. This is a startup divergence of
the realization — sim ≠ SV whenever another domain ticks before a module's first edge and the
module's input changes in between — that no rule, pin or sweep case covers (the sweep drives
one clock). The per-domain defaults are realization details with no meaning in this document;
the barrier that read them goes with D-8, and Phase C keys the executor's resumption on the
domain: `tick_clock::<D>` resumes the coroutines whose clock is in `D` and evaluates the
combinational network (I.11.6, I.13; D-18: I.13.2(b), I.13.7 (5)), and no other domain's settle resumes them. The probe is
owed as a two-clock pin (expected `q = 2`). The audit's pending accident
(`review/DECISION_AUDIT.md` rows 53 and 97) closes here. Prior work: Prost resets every
coroutine "to its initial suspension point on reset" and advances it only at its clock's edge
[prost §2]; Arch's domain threads run independently [arch §7.3.4]; no surveyed language states
what a module whose clock has not yet ticked has done, because none has a second domain to
tick first.

**I.12.3 The synchronizer module.** A `#[hardware(synchronizer)]` module is the kernel of
I.2.3 (I.2.7) with I.2.2's single-domain clause replaced: exactly one `Clock<D_dst>`, its
`RegOut`s in `D_dst`, and its `In` ports in `D_dst` **or in any other domain** (a free
`Src: ClockDomain` parameter in `sync_2ff`, a concrete one in `flag_sync`). Its coroutine is
resumed by `D_dst`'s edges only (I.12.1) and has the protocol of I.0.2 unchanged: every rule of
Part I and Part II applies to it as to a `sequential` module — registers by I.7.1 (`ff1` a
register, `q` a `RegOut`, `ff2` a wire of one resumption under the tick-last spelling, I.7.5),
the one-job body (D-8), the memory rules with a memory in `D_dst` only (I.10.5). **The foreign
read.** A read of `In<T, D_src>` in the resumption that ends at `D_dst`'s edge N+1 yields the
value standing on that wire at that instant (I.4.1). The wire is a `RegOut<T, D_src>` of a
module in `D_src` (the only driver that may cross, I.12.4(a)), which changes value only at
`D_src`'s edges (I.1.3, I.6.1); so the value read is **the last value `D_src` committed before
`D_dst`'s instant**. Which `D_src` edge that is, is the environment's interleaving (I.12.1);
under the simulator it is the sequence of `tick_clock` calls. The model is deterministic: no
metastable value, no X, no randomized latency. The hardware's sampling of a wire that changes
near the destination edge is outside the model, and **the two-flop structure is what makes the
hardware agree with the model from the second flop on**: stage 1 may resolve to either
neighbouring value, stage 2 presents a settled one, and the model's last committed value is one
of the two the hardware can present after it. This is the textbook idiom `sync_2ff_ref.sv` is
written from and the one BaseJump ships as `bsg_sync_sync` [basejump §6 items 9–10, listed by
name; its internals are not in the note]; Arch models the same hazard as a randomized
synchronizer latency, `--cdc-random` [arch §7.1], a nondeterminism this document does not adopt
— the agreement relation needs one trace, and the deterministic model under the two-flop rule is
what the anchor measures. The anchor: `sync_2ff_concrete` vs `sync_2ff_ref.sv` — `q` after
destination edge n is `d` as it stood at destination edge n−1, one destination cycle of
observable latency through two flops, independent of the source rate (1:1 to 8:1), a pulse that
stands at no destination edge dropped and one that does captured
(`tests/cdc_synchronizer_anchor.rs`, ten cases; measured PASS, D-17 (ii)). **The normative
spelling** is the one-job one (D-8, D-9; measured identical to the reference in I.7.5):
`ff2 = ff1; ff1 = d.read(); q.write(ff2); tick` with `q` a `RegOut` — `ff1` the register that
samples the foreign wire at edge N+1, `q` the `RegOut` that presents it at edge N+2; today's
`src/sync.rs` spells it with a plain `Out` and a trailing update and is rewritten in Phase C
(D-9; `tests/ref_probes/sync_2ff_tick_last.rs` is the spelling, in one domain). **Decided
(D-17 A3), the two-flop rule — a realizability rule of II.1:** in a `synchronizer` module **no
`RegOut` write may depend on a foreign read of the same resumption** — neither its value (by
forwarding through the resumption's locals, I.3.4) nor its guard (an `if`/`match` condition
the write stands under). A foreign read's value may be assigned to a local; the local is then a
register at the `tick` (I.7.1), and the next resumption may write it to a `RegOut`. The value
therefore crosses two edges of `D_dst` before it is observed — the register's at N+1, the
port's at N+2 — which is the two flops in series. Principle: realizability (I.0.6) — a
`RegOut` written from a foreign read of the same resumption is one flop sampling an
asynchronous wire, and the hardware cannot realize the model's deterministic value at that
flop's output in every cycle; the rule refuses the shape as one the emitted text cannot
realize, with its direction (assign the local in this resumption, write the port in the next).
Measured (D-17 (iii), (iv)): no rule enforces this today (`check_cdc` is skipped for the mode,
`copper-macros/src/lib.rs:943-947`, and gates port kinds only); every shipped synchronizer
satisfies it; the one-flop probes `sync_1ff` (the anchor test's deliberate neighbour,
`q.write(ff1); tick; ff1 = d.read()` — one job: `ff1 = d.read(); q.write(ff1)`), `sync_1ff_oj`
(emitted `q <= d`) and `sync_guard` (`if d.read() == 1 { ff1 = 1 }; q.write(ff1)`) are admitted
by both front-ends and are what the rule refuses; the three-flop `sync_3ff` of `src/sync.rs`'s
doc test is admitted. Phase C: the rule in `copper-analysis` (a taint of the resumption's
locals from foreign reads, cleared at the `tick`, checked at every `RegOut` write and its
guards), called by both front-ends (II.1.3), a ui/fail case from `sync_1ff_oj`; the anchor
test's `sync_1ff` becomes a refusal pin (its two-flops-not-one check is then the rule's). A
native-domain read (an enable in `D_dst`) is unconstrained.

**I.12.4 What may cross.** (a) **The one crossing.** A wire driven by a `RegOut<T, D_src>`
and read by a `synchronizer` module in `D_dst` through an `In<T, D_src>` port. Nothing else
crosses: wiring a `D_src` wire into a `D_dst` port of any module is a type error (E0308;
`copper-core/src/cdc.rs`, pinned in `copper-core/tests/ui/fail/` — the four domain cases;
measured D-17 (v)), a foreign-domain port on a `sequential` module is refused by `check_cdc`
(`copper-macros/tests/ui/fail/cdc_crossing`), and a `structural` parent that wires a foreign
net into a regular child is refused at the call site by the transpiler
(`copper-codegen/src/chir_lower.rs:2286-2310`). The value read is I.12.3's. (b) **A memory
does not cross.** A `Memory<…, D, …>` parameter in a domain other than the module's clock is
refused on `sequential` and `synchronizer` modules alike (`check_cdc`'s memory arm, `lib.rs:
219-247`, ui `cdc_memory_parameter`; I.10.5): a memory lies in one domain and each port is one
module's (D-16 A7), so there is no synchronizer form for it. A dual-clock memory — one port in
each domain — is a synchronizer-kind primitive of its own with no form today: a NOT-YET row
carried to I.13 with that direction (D-18: I.13.8 — carried). (c) **The combinational module.** Decided (D-17 A4a): all
ports of a `combinational` module lie in **one** domain, the domain the module is evaluated in
(I.11.1 — a function of its reads, settled per I.13, D-18: I.13.2; its read of a `RegOut` is the committed
value, I.11.4). A `combinational` module with ports in two domains is a crossing with no
synchronizer — one evaluation would make a `D_dst` wire a function of a `D_src` register, a
combinational path between domains that no flop samples — and is OUTSIDE (S0, II.1.1).
Measured (D-17 (v)): both front-ends admit it today (`c_mixed(a: In<Logic, A>, o: Out<Logic,
B>)` compiles under the macro — the combinational arm runs no domain rule,
`lib.rs:1371-1384` — and transpiles to `assign o = a`); only the wiring is checked. Phase C:
`check_cdc`'s port-domain clause on the `combinational` mode in both front-ends, the module's
domain being its ports', with a ui case from `c_mixed`. (D-22: done — `front_end::check_combinational_one_domain`,
ui `combinational_two_domains`. **A module generic over its domain** — `fn m<D: ClockDomain>(…)`, every
port in `D` — is the spelling of a combinational module that serves any domain: each **instance** lies
in the domain of the wires it is connected to; the testbench names `D` through its wires' types, a
`structural` parent's call site binds `D` to the domain of the instance's first net and checks every
other `D` port against it (`chir_lower.rs::lower_structural_inst`), and two instances may lie in two
domains. The domain is phantom: no SV parameter, the text the concrete module's; the sweep names one
concrete domain for it. One instance wired across two domains is still the crossing refused above —
`structural_hierarchy.rs::a_domain_generic_child_takes_the_domain_of_its_nets`.) (d) **The structural parent.** A
`structural` module receives several clocks and wires children in several domains
(`two_domain_top`); I.13 gives the wiring its meaning (D-18: I.13.4 — a name for the design of its instances and wires). This section states only that every
crossing instance in it is a `synchronizer` child — which the call-site check already enforces
— and that the parent has no coroutine of its own (I.2.7). (e) **Multi-bit crossings.** Decided
(D-17 A4b): a `synchronizer` module's foreign port may be `Bits<N>`, with the meaning **N
independent per-bit synchronizers**: each bit of the value read is the last-committed value of
that bit (I.12.3), and nothing relates the bits — a word that changes in `D_src` between two
`D_dst` instants may be read with some bits old and some new. The coherence hazard is outside
the model and is documented here, not refused: a coherent multi-bit crossing is a gray-coded
counter or a handshake built from single-bit crossings (BaseJump's `bsg_async_ptr_gray` and
`bsg_launch_sync_sync` [basejump §6 items 9–10, listed by name; the forms are not covered by
the notes]), a design of the user's. Corpus (D-17 (iii)): every synchronizer port is `Logic`
(0 multi-bit). `src/sync.rs`'s header says synchronizers "are restricted to single-bit `Logic`"
— a statement of `sync_2ff`'s signature, not a rule; no rule refuses `Bits<N>`.

**I.12.5 The two-clock sequential module.** A `sequential` module with two or more `Clock`
parameters is refused by decision (D-12, II.1.2 R10): S0 — I.2.2 says exactly one — and S2 — a
`tick` on a second clock is a second resumption protocol, and I.0.2 has one. Both front-ends
admit it today (`check_cdc` skips a module with several clocks, `lib.rs:207-210`; measured
accepted, D-12) and both owe the refusal (II.1.3). A design that needs two clocks is a
`structural` composition (I.13; D-18: I.13.4) of single-domain modules and synchronizers; the three CDC
examples are that design hand-wired (`examples/cdc/`). The macro's read-gating fallback for a
port whose domain no clock of the module ticks (`lib.rs:811-825`) is the two-job
implementation's and goes with the barrier (D-8). Closed; nothing to decide.

**I.12.6 Implementation map.** Types: `ClockDomain` (`copper-core/src/types.rs:850`, a
marker), `Clock<D>` (`:863-927`: an `Arc`'d cycle counter and its listeners; `advance` (`:886`)
is the edge, `tick()` (`:913`) a `ClockTick<D>` with target cycle + 1), `ClockTick::poll`
(`:985-1008`: Ready iff `cycle ≥ target` and the domain is resolving), `TICK_RESOLVING` per
domain, defaulting to true (`:966-983`); ports `In<T, D = ()>`, `Out<T, D = ()>`
(`port.rs:109`, `:79`; the default goes in Phase C; D-22: gone — `In`, `Out` and `RegOut` bound
`D: ClockDomain`, no default, and `wire` requires it), `RegOut<T, D>` with `registered_wire`
requiring `D: ClockDomain` (`:188-235`); `cdc.rs` — no code: its guarantees are pinned in
`copper-core/tests/ui/fail/` (`in_port_foreign_domain`, `out_port_foreign_domain`,
`clock_foreign_domain`, `second_driver_foreign_domain` — E0308; `out_is_not_clone`,
`out_not_clone_single_driver` — the single driver; `regout_is_not_clone` — E0599), its legal
path and GAP 1/2 are doctests. Macro: `check_cdc` (`copper-macros/src/lib.rs:134-259`: native
domain = the one clock's; a port or memory in another domain refused; skipped when the clock
count is not one; `()` not a crossing), called for `sequential` only (`:943-947`); the
read-gating fallback (`:811-825`); `validate_hardware_fn` admits `Memory` and requires the loop
in the `synchronizer` mode (`:1936`); ui `cdc_crossing`, `cdc_memory_parameter`. Simulator:
`tick_clock::<D>` = `pre_edge_settle::<D>` (phase pre-edge, resolving off, `poll_tasks` over
**every** task) then `post_edge_settle::<D>` (`advance`, phase post-edge, resolving on,
`poll_tasks`) (`copper-sim/src/executor.rs:813-853`); `POLL_PHASE` per domain, defaulting to
pre-edge (`copper-sim/src/lib.rs:36-51`); the global `cycle` counter (`executor.rs:852`,
D-1 A2's artifact). Transpiler: a `synchronizer` is lowered as a `sequential` module
(`copper-codegen/src/parser.rs:98`; one `clk` input, the foreign port a plain input); the
structural call-site domain check (`chir_lower.rs:2286-2310`); the hierarchy emits each child's
`.clk(…)` (`examples/cdc/sv/two_domain_hierarchy.sv` is its reference). Sweep: one clock per
module — a second `Clock` port is a `SKIP` (`build.rs:702-708`: "the ratio between two domains
is a design decision the sweep must not invent") and `two_domain_top` a named `SKIP` row
(`:118-121`); a synchronizer's foreign port is driven like any input, its domain declared by
the wrapper (`:884-931`); `REF_SKIP`'s "multi-clock (I.12)" class (`:362`, `:769-773`) has no
member — every synchronizer has one clock and is deferred by the plain-`Out` class instead.
Interpreter: one `Clock` parameter, more deferred (`copper-ref/src/module.rs:211-215`); a
synchronizer is a one-clock module to it and needs nothing of this section. Corpus: `sync_2ff`
(`src/sync.rs`), `flag_sync` (`examples/cdc/two_domain_hierarchy.rs`, copied in
`tests/two_domain_hierarchy_cdc.rs`), `sync_2ff_concrete` (`tests/fixtures/sync_2ff_dut.rs`),
the `copper-codegen/tests/structural_hierarchy.rs` copy, `sync_3ff` (doc test),
`sync_2ff_tick_last` (ref probe), `sync_1ff` (anchor test); `two_domain_counter`,
`flag_crossing`, `two_domain_hierarchy` (`examples/cdc/`). Tests: `cdc_crossing` (delay,
hold), `cdc_synchronizer_anchor` (latency, rate independence, quiescence under source ticks,
dropped and captured pulses, two flops not one, the G2 register match, the reference and the
transpiled arms), `two_domain_counter_behavioral` (per stage at 1:1, 2:1, 3:1),
`flag_crossing_behavioral`, `two_domain_hierarchy_cdc` (sim ≡ transpiled hierarchy ≡
independent reference, 2:1). **Phase C obligations:** (1) the executor resumes a coroutine in
its own domain's settles only — tasks keyed by their clock's domain, `tick_clock::<D>` polling
`D`'s sequential tasks and the combinational network; the per-domain thread-locals deleted with
the barrier (D-8); probe (i) as a two-clock test (expected `q = 2`), and
`source_domain_ticks_do_not_advance_the_synchronizer` kept; (2) the two-flop rule of I.12.3 in
`copper-analysis`, both front-ends, ui case from `sync_1ff_oj`; the anchor test's `sync_1ff` a
refusal pin; (3) the combinational one-domain rule of I.12.4(c) in both front-ends, ui case
from `c_mixed`; (4) the unit domain removed — no `D = ()` default, `D: ClockDomain` on every
port type, the thirteen scaffolding sites given a domain, `check_cdc`'s `()` clause deleted (D-22:
**discharged**, Phase C step 11 — the bound and the default as stated; the scaffolding given domains in
step 10; the 32 corpus modules of I.12.1's D-22 note generic over theirs; the refusal is rustc's in a
testbench (E0277, copper-core ui `unit_domain_refused`) and, on a module, the shared
`front_end::check_port_domains` in both front-ends with one message — a port, clock or memory naming no
domain or `()` (ui/fail `unit_domain_port`, `missing_domain_port`; `front_end_parity_ui.rs`); the
domain readers behind it (`check_cdc`, the one-domain and two-flop rules) meet named domains only, so
their `()` arm is unreachable);
(5) R10 refused in both front-ends (D-12); (6) `sync_2ff`, `flag_sync` and the fixture twin in
the tick-last `RegOut` spelling (D-9), `src/sync.rs`'s header corrected (the single-bit sentence
is a signature, the structural requirement is I.12.3's rule); (7) `REF_SKIP`'s multi-clock
class and the sweep's second-clock `SKIP` restated as this section's (one clock by S0; a
`structural` parent is I.13's — D-18: I.13.7, a named `SKIP` row); (8) the executor's global `cycle` counter per domain or renamed
(I.1.6).

**I.12.7 Carried NOT-YET list.**
- The structural wiring of domains — a parent receiving several clocks, each child on its
  own, the crossing instance a synchronizer — and the dual-clock memory (a synchronizer-kind
  primitive with no form, I.12.4(b)) → I.13. (D-18: I.13.4(e), I.13.8 — the wiring closed; the dual-clock memory carried.)
- X on a crossing wire (a `RegOut` holding `Bits::x()` read by a synchronizer; the X algebra of
  the foreign read) → I.14. (D-19: I.14.2(g) — the bits cross unchanged; closed.)
- The interpreter: a synchronizer runs as a one-clock module (deferred today only by the
  plain-`Out` class, until the tick-last rewrite); a two-domain design is an environment of
  III.2 (D-20: III.2.6) — a stated interleaving of `resume`/`edge` calls per domain — and needs I.12.1's
  "resumed by its own domain only" and nothing else → Phase C, III.2 (D-20: III.2.6).
- An independent reference for a two-domain design under a stated interleaving beyond the 2:1
  hierarchy anchor, and one for the never-ticked startup of I.12.2 → III.2 (D-20: III.2.11 item 7) (the user writes
  references).
- The Phase C obligations of I.12.6 (1)–(8).

### I.13 Structural modules, wiring and the settle

Decided: D-18 (2026-11-05). Anchor: `two_domain_top` vs `examples/cdc/sv/two_domain_hierarchy.sv`
(`tests/two_domain_hierarchy_cdc.rs`: the simulator's hand-wired design, the transpiled hierarchy
under Verilator and an independent hand-written SystemVerilog reference agree at 2:1, measured
PASS, D-18 (v)); the Mealy composition of I.11.5 (a `RegOut`-writing core beside a `combinational`
reader vs an independent hand-written machine, D-14 (v)) for the settle's reading of a `RegOut`
as a source. Sim ≡ SV only: the received-memory bus (`tests/received_memory_abi.rs`), the
agreement of the two schedulers (D-18 (i)–(ii)). Decided, not anchored: the structural module's
meaning as the design it denotes, the structural-only instantiation form, the kind-agnostic net.

In coroutine terms (D-7, D-8): a design is a finite set of coroutines (I.0.1) and functions
(I.11.1) that share wires; a wire has one driver and any number of readers; at each instant the
functions are evaluated over the wires until every wire is its driver's value, and every
coroutine resumed at that instant reads those values; a `structural` module is no coroutine — it
names a design.

**I.13.1 A design as a set of coroutines over wires.** A **design** is a finite set of module
**instances** — each the coroutine of a `sequential` or `synchronizer` module (I.0.1, I.12.3) or
the function of a `combinational` module (I.11.1) — and a finite set of **wires**. A wire has
**exactly one driver** and **any number of readers** (I.6.8 W6; II.1.2 R22): the driver is one of
(i) a `RegOut` port of a coroutine — the wire holds the committed value and changes only at that
coroutine's edge (I.6.1, I.1.3), (ii) an `Out` port of a function — the wire holds the function's
value at every point in time (I.11.1), or (iii) the environment, which drives an input wire
between an observation and the next tick (I.1.5). A reader is an `In` port of an instance, or
the environment's observation. **Wiring** is a connection of one driver to its readers and is
**type-checked**: the payload type and the domain of the driver's port equal those of every
reader's port (`In<T, D>` to `Out<T, D>` or `RegOut<T, D>`; I.12.4(a) — the one crossing is a
`synchronizer`'s foreign `In`, whose domain is the driver's); the one-driver rule is the type
system's — `Out` and `RegOut` are not `Clone`, `In` is (`copper-core/src/port.rs:64-77`,
`:108-118`, `:190-218`). Nothing here is new: a design is what I.0, I.6 and I.11 say of its parts,
with the wires shared. Realizations: the simulator realizes a wire as a shared cell whose identity
is its pointer (`WireId`, `port.rs:14`; `wire(init)` `:136-150` makes an `Out`/`In` pair over one
cell, `registered_wire(clock, init)` `:220-235` a `RegOut`/`In` pair over the committed cell), and
a design as the tasks `spawn_wired` is given with their output dirty handles and the `WireId`s
they read (`copper-sim/src/executor.rs:401-416`) — the testbench's hand wiring is the design's
wiring written out; the transpiler realizes a wire as a net and a design as a module that
instantiates its parts and connects their ports by name (`emit.rs:436-455`: `.port (net)`). The
environment's input wire is created at the zero of its payload (I.11.2(e), I.5.6; D-20: III.2.3; a III.2
precondition). Measured (D-18 (iii), (vi)): a second driver of a net is E0382 (`s4`, `s10`), a
second reader with `.clone()` compiles (`s11`); the corpus has 1 `structural` module, 225
`spawn_wired` sites in `examples/` and `tests/` (40 and 185; 41 files wire two or more modules,
six of them examples), 7 `spawn_untracked` and 2 `spawn_child!` sites. Prior work: Chisel's `:=`
is wiring, "exactly one connection" per port [chisel §2.4]; Filament's connection `pd = ps` over
instances `x := new C`, "continuously active wires", composition parallel [filament §3.5, §6.1];
Arch's single-driver rule [arch §7.3.4]; CλaSH's function application as instantiation, every
argument a port [clash §II-A]; Spade's `inst` [spade Listing 3]; PyMTL3's `connect` [pymtl3 Fig. 2];
RHDL's struct-of-signals makes a signal's writer its owner [rhdl §3]; Prost has no instantiation
— a sub-task is called and inlined [prost §4] — and Bluespec composes modules by method calls,
with no wire [bluespec slide 14] — both not covered as wiring.

**I.13.2 The settle.** At each instant the **combinational network** — the functions of the
design's `combinational` instances composed over the wires — is evaluated from its **sources**,
the committed `RegOut` wires and the environment's inputs, until every wire holds its driver's
value: the **settled** network. Every read and every observation at that instant sees settled
values (I.1.3, I.4.1, I.11.4). Three statements, all consequences:
(a) **The settle is a function of the sources.** Each function is a function of its reads
(I.11.1), and the combinational cycle is OUTSIDE (I.11.4, D-14 A4: two or more functions whose
reads reach each other with no `RegOut`, memory or synchronizer between them have no meaning), so
the network is a directed acyclic graph from the sources through the functions to the wires; its
fixpoint exists, is unique, and is reached by evaluating each function once in a topological
order. "Simulation must be independent of poll order" (`SYNCHRONOUS_SEMANTICS.md`, SC1) is
therefore a **theorem** of this section, not a requirement on the design: under any evaluation
order and any number of evaluations the settled wires are the same (I.13.3).
(b) **When the settle runs, relative to the resumptions.** Per cycle N of a domain `D` (I.0.2,
I.1.2): the environment drives `x_{N+1}`; the network settles on `(q_N, x_{N+1})`; each coroutine
in `D` parked at a `tick` is resumed and runs to its next `tick`, reading the settled wires
(I.4.1) and setting `RegOut` pending values (I.6.1) — a resumption changes **no wire** (a
sequential module has no plain `Out`, D-8; RO6, M15: a `RegOut` is a source of the settle, never
a combinational dependency), so the resumptions of one instant cannot invalidate the settle they
read, in any order (I.12.1: only `D`'s coroutines); the edge N+1 commits every pending value and
advances every memory in `D`; the network settles on `(q_{N+1}, x_{N+1})`; `obs_{N+1}` is read
(I.1.4). The network is thus evaluated at two points of every cycle — after the drive, for the
resumptions, and after the edge, for the observation — and both are the one function of (a)
applied to the sources of that point. What D-8 retires is the second **resumption** of the
two-job body and the barrier that placed it (I.0.7); the post-edge **evaluation of the network**
is the observation's and stays. A settle resumes no coroutine: a coroutine parked at a `tick` is
resumed by its domain's edge (I.0.2, I.12.1), and a realization that polls a parked coroutine
during a settle runs no statement of it (the audit's pending executor fact,
`review/DECISION_AUDIT.md` row "a fixpoint re-poll runs no statement", is this clause's
consequence). The first settle of a domain is the one at `t_1⁻` (I.9.2): the functions of a
design that no coroutine has yet driven evaluate over `init(q)` and `x_1`.
(c) **Order independence as a proposition of III.1.** (D-20: P5.) The simulator's levelized settle (one
topological pass over the strongly-connected components of the combinational dependency graph)
and its fixpoint oracle (every task re-polled until no wire changes) compute the same settled
wires on every acyclic design — the correctness proposition of `SchedulerMode::Levelized`
(`LEVELIZED_SCHEDULING_SCOPE.md`), which (a) proves and the following measure: `tests/
levelized_differential.rs` (every wire after every settle of every cycle, six topologies, 6/6),
`tests/poll_order_fuzz.rs` (the oracle under eight visit orders, 2/2), `tests/golden_traces.rs`
(six frozen traces, byte-identical under both schedulers, 6/6 + 6/6), `two_domain_hierarchy_cdc`
3/3 under `COPPER_SCHEDULER=fixpoint`, and a 65-case sample of the corpus sweep under the oracle
(65 passed, 0 failed, the 34 ignored rows the same `SKIP`/`REF_SKIP` rows as under the default) —
every verdict predicted first (D-18 (i)). Prior work: Esterel's causality — a program is rejected
when "a signal … can still be written" when read, by a static acyclic signal-dependency graph,
even when the solution is unique (P6) [esterel §7]; Arch — a static DAG across comb blocks and
instance port connections, a bounded settle of one or two passes "not a fixpoint iteration", and
"DAG level barriers" as the parallel-execution discipline [arch §3.8, §7.3.1, §7.3.4]; PyMTL3 —
the `StaticSchedulingPass` evaluates each block once in topological order, the `EventDrivenPass`
re-evaluates on change and throws on "actual combinational loops" [pymtl3 §2–§3]; Bluespec — a
cycle's parallel firing is correct iff linearizable to a one-rule-at-a-time sequence [bluespec
slide 15]; Chisel — one value per wire per cycle from a DAG of operators, combinational loops
"not discussed anywhere" [chisel §2.5]; FIRRTL's combinational-loop check — not covered by the
notes; SystemC — the evaluate/update delta cycle is IEEE 1666's; the synthesizable subset fixes the
thread's shape and states nothing of the settle [systemc §4.2.2]; Verilog — the stratified event queue, blocking evaluation then
NBA update [cummings §4.0, §6.0]; RustHDL — fixpoint iteration with an error cap, RHDL —
"feedback loops must be broken by registers" [rhdl §3, §4]; CλaSH — no settling model [clash
§II]; Filament — composition is parallel, no ordering within a cycle [filament §6.1].

**I.13.3 The evaluation count and the dirty flag.** A function evaluated **zero or more times**
during a settle leaves the same value on its wires once its reads are settled: the value is `f`
of the reads (I.11.1), an evaluation has no state (I.7.2(e)) and no effect but its writes
(I.11.2), and a body with a host side effect is OUTSIDE (I.2.5). The number of evaluations a
realization performs is therefore **unobservable** (I.11.1), as is the order among functions
none of which reads another's wire. The simulator's two schedulers differ in exactly this: the
levelized settle polls **every** task exactly once per settle, in a topological order of the
condensed graph, and discards a singleton task's dirty flag (`poll_task_once`,
`copper-sim/src/executor.rs:561-572`); the fixpoint oracle re-polls every task until a full pass
sets no dirty flag (`poll_tasks_fixpoint`, `:753-803`); a multi-task component — a cycle, OUTSIDE
since D-14 — is iterated by the levelized settle until clean (`iterate_scc`, `:588-632`). The
**dirty flag** (`Out::write` sets it on a change of value, `port.rs:86-92`; a `RegOut` sets it at
the edge, `:173-183`) and the oscillation counter `consecutive_dirty` (`:254`) are the oracle's
and the iteration's re-evaluation rule — a scheduler artifact with no clause of meaning (I.6.8).
Measured (D-18 (ii)): a `RegOut` counter feeding a `combinational` module that counts its own
evaluations, feeding a second one, ten ticks — the levelized settle evaluates it exactly 20 times
(one per settle, two settles per tick), the oracle 40 (two passes per settle: the first changes a
wire, the second confirms — the prediction said 30, having placed the `RegOut`'s edge-set dirty
flag in one settle rather than two), and the two traces are byte-identical. Prior work: MyHDL's
`always_comb` infers its sensitivity from its reads and runs once at time 0 [myhdl §6.2.1];
Cummings' `always_comb` re-evaluates on any read's change [cummings §5.3]; PyMTL3's two passes
differ in exactly this count [pymtl3 §2]; Arch's settle depth of one or two is a count fixed at
compile time [arch §7.3.1].

**I.13.4 The structural module.** (a) **Meaning — decided (D-18 A1).** A
`#[hardware(structural)]` module **is not a coroutine and not a function**: it is a **name for a
design** of I.13.1 — the instances its body instantiates, the wires its nets and ports name, and
the connections its calls make. Its meaning is exactly that design's: the traces of its
instances under I.0–I.12 over the wires it connects. It executes no statement, holds no state,
has no instant of its own; its `Clock<D>` parameters are the clocks it threads to its children,
each child on its own clock (I.12.1, I.12.4(d)). The simulator's **hand wiring** of the same
instances (`spawn_wired` with the same cells) and the transpiler's **hierarchy** (`--hierarchy`,
I.13.6) are two realizations of the one design and must denote it — a III.2 obligation (D-20: III.2.7, P19), measured
today by the one anchor (`two_domain_top`: hand-wired simulation ≡ transpiled hierarchy ≡
independent reference, D-18 (v)). The **sim-as-unit** form — the macro lowering the body into the
spawns and the per-domain tick driver so that the parent itself runs in the simulator (item 4a,
H1) — is a **realization form deferred with a date** (2026-11-05; carried in I.13.8), not a
meaning: nothing in a trace distinguishes a design spawned by hand from one spawned by its
parent. (b) **The body.** The body is a **wiring language**, not the kernel of I.2.3 (I.2.7):
in source order, **net declarations** `let n = net::<T, D>()` — Phase C's spelling, D-18 A3; today
`wire::<T, D>(init)` and `registered_wire::<T, D>(&clk, init)` (c) — and **instantiations**
`child(arg, …)`, one positional argument per parameter of a `#[hardware]` module of any mode,
each argument a parent port, a net's driver end `n.0` or reader end `n.1`, or a clock (cloned
where a clock reaches several children), and nothing else (D-22: or a `.clone()` of a reader end or
of a parent `In` — a net with several readers needs one in Rust, where an `In` moves; the rule
admits it, Phase C step 10, §P.4). (c) **The net — decided (D-18 A3).** A
net is a wire of I.13.1 and **carries no value of its own**: its value is its driver's — a
`RegOut` child port's committed value, `init(q)` before the first edge (I.6.6); a `combinational`
child port's evaluation (I.11.2(e)); a parent `In` the environment drives. The cell **kind**
follows the driver, which the type system fixes (a `RegOut<T, D>` port connects only to a
`registered_wire` pair, an `Out<T, D>` only to a `wire` pair — `s6a`, E0308), so a kind-agnostic
spelling with no initial value is one spelling per meaning: Phase C introduces `net::<T, D>()`
and retires the two. Until then the `init` argument is a III.2 precondition (D-20: III.2.3) — the zero of `T`,
what every wire is created at — and `registered_wire::<T, D>(&clk, init)` is the sanctioned
spelling for a `RegOut`-driven net, which under D-8 is every net a sequential child drives; the
transpiler admits it today by an accident of `parse_wire_net`'s substring match (`s6b`, emitted
correctly as `logic n;`, D-18 (iii)) and ignores `init` (`chir_lower.rs:2187-2213`). (d) **The
shape rules — S0 rows of II.1.1 for the structural mode**, each measured through both front-ends
on a probe (D-18 (iii); M = macro, T = transpiler):

| rule | principle | today: M | today: T | Phase C owes |
|---|---|---|---|---|
| the body is nets and instantiations only — no other statement, no port read | S0 (I.13.4(b)) | admits (`s1`, `s2`: the fn is emitted unchanged) | refuses (`parse_wire_net`) | M |
| every child port connected | S0 | refuses (E0061, `s3`) | refuses (arg count, `:2268-2278`) | — |
| one driver per net (R22) | S5 / I.13.1 | refuses (E0382, `s4`, `s10`) | **admits** (`s4`: two instances `.o(n)`, Verilator MULTIDRIVEN) | T |
| every net driven | I.13.1 (exactly one driver) | admits (`s9`, a warning) | admits (`logic n;` undriven, Verilator UNDRIVEN) | both |
| same payload and domain on both ends; a crossing only into a `synchronizer` (I.12.4(a), (d)) | S0 | refuses (E0308, `s5`; D-17 (v)) | refuses (call-site check, `:2286-2310`) | — |
| the port kind on both ends (a `RegOut` port to a `registered_wire` net) | S0 (I.13.4(c)) | refuses (E0308, `s6a`) | **admits** (`s6a`: no port-kind check) | T (vacuous once `net::<T, D>()` lands) |
| at least one `Clock` parameter, every clock threaded | S0 (I.2.2 for the mode) | refuses (`lib.rs:1496-1503`, `s8`, `s8b`) | **admits** (`s8b`) | T |
| one owner per memory port (I.10.5): a `Memory` parameter reaches one child | S0 (D-16 A7) | refuses (E0382, `Memory` is not `Clone`, `s7`) | refuses (a `Memory` parameter is no signal of the lowering, `s7`) — the received-memory bus is the ABI of a child, not of a parent (I.13.6) | the port set and mode in the parameter type (I.10.8) |
| an instantiation names a `#[hardware]` module (no plain `fn` call) | S0 | admits (`s12`: a plain fn call compiles) | refuses ("call is not a `#[hardware]` submodule", `:2170-2175`) | M |
| no instance of the parent's own name (recursion) | S0 (I.2.5(c): finite) | admits (`s13`: the dropped future is no recursion to rustc) | admits (`s13`: a self-instantiating module, Verilator UNSUPPORTED) | both |

Every verdict in the M column was predicted; two of the sixteen in the T column were not (`s6b`
admitted by accident, `s7` refused by the memory parameter's absence from the signal map). A row with one
refusing front-end is a parity break of II.1.3 and is owed to the other, with one message naming
this clause. (e) **Domains and memories.** A structural parent receives several clocks and places
each child in the domain of the clock it passes; every crossing inside it is a `synchronizer`
child (I.12.4(d), enforced at the call site in the transpiler and by E0308 in rustc); a `Memory`
parameter of the parent is owned by the one child it reaches, and the port set and mode a child
uses are owed to the parameter type (I.10.5, I.10.8 — Phase C, with the kind-agnostic net). The
dual-clock memory — one port per domain — is a synchronizer-kind primitive with no form, carried
(I.13.8). Prior work: CλaSH, Chisel, Spade, PyMTL3, Filament and Arch each have an instantiation
form and a connection form (I.13.1); none gives the hierarchy a meaning other than the flattened
design, and Chisel's and PyMTL3's elaboration flatten it [chisel §2.2; pymtl3 §1]; Filament
types the composition (Lemma C.18: a well-typed composition is safe) [filament §6.1, Lemma C.18]; Prost
flattens by inlining [prost §4]; SystemC's module binding — not covered by the subset note.

**I.13.5 Hierarchy inside a behavioral module — decided (D-18 A2).** **Instantiation and net
declaration are forms of the structural module only** (S0): a `sequential`, `synchronizer` or
`combinational` body contains no instantiation and no net — I.2.3's grammar has neither, and a
body that needs a child is written as a `structural` parent over two modules (the Mealy pair of
I.11.5 is the pattern: a core and a reader, wired). I.11.7's "hierarchy inside a combinational
body" closes here: a port a child drives is a wire of a structural parent, and the never-written
port rule of I.11.3 has no exemption. Measured (D-18 (iv)): the corpus has **0** behavioral bodies
calling a `#[hardware]` fn (the two name hits, `ripple_carry_adder` → `full_adder` and
`stage_add_one` → `add_one`, call plain helper `fn`s of the same name — inlined, I.2.4); on
probes, passing an `In` port into a child is refused by the macro's R19 (`b1`); a `wire()` net in a
body compiles under the macro and is refused by the transpiler before any instantiation ("cannot
infer bit width", `b1b`); a child that takes a parent `Out` port (`one(o)`, `b2`) is refused by
the macro in a `combinational` body only by accident (the `loop { body; delta_yield }` wrapper
moves the port every iteration, E0382) and **admitted** in a `sequential` body, where the child's
`HardwareModule` is dropped and **never runs**; the transpiler **admits** both through its legacy
expression form (`lower_hardware_call`, `chir_lower.rs:4961-5008`, reached from any call
expression whose callee is a `#[hardware]` fn, `:3785`): the combinational parent emits
`one one_0 (.o (o), .out (one_0_out))` — a port the child does not have, Verilator PINNOTFOUND —
and the sequential parent **drops** the pre-loop call from its text (`o` undriven), the linear
route's pre-loop-expression drop of I.9.3. A shape both admit and neither realizes. Phase C: both
front-ends refuse a call of a `#[hardware]` fn and a `wire`/`registered_wire`/`net` call inside a
behavioral body with one message naming this clause (ui case from `b2`); the transpiler's
expression form (`lower_hardware_call`, `CHIRSubmoduleInst::inputs`/`output_wire`,
`SHIRCombBody::submodules`, `SHIRSeqBody::submodules`, the legacy branch of `emit::submodules`)
and the executor's `spawn_child` (`executor.rs:427-465`; the `spawn_child!` macro,
`copper-sim/src/lib.rs:129-137`; `module_composition_hybrid.rs`'s one use rewritten to
`spawn_wired`) are retired. Prior work: Prost — a sub-task is a *call*, inlined into the caller's
state space, "coroutine call" replacing instantiation [prost §1, §4] — the form this clause
declines; Spade's `inst` inside a pipeline body [spade Listing 3] and Chisel's instances inside a
module body, nodes of the elaborated graph [chisel §2.2] admit it, with the body being elaboration, not execution; CλaSH's
application inside a function [clash §II-A] likewise; Arch's instances inside a module beside
`comb` blocks [arch §7.3.1]; Filament's `new`/`invoke` are composition, not statements of a body
[filament §3, A.2].
(D-23, 2026-11-08, Phase C step 11b, `review/MEASUREMENTS.md` §S: **realized as §R.6 (1) option (a) —
rustc decides.** Every generated `sequential`, `synchronizer` and `combinational` fn carries
`#[must_use = <this clause's message>]` and `#[deny(unused_must_use)]`, so `child(a, b);` in a
behavioural body is a compile error whose note is the message (`ui/fail/behavioural_instantiation`,
from `b2`'s sequential half; `b2`'s combinational half — a parent passing its own `Out` — is refused
first by I.11.3's never-written rule). The transpiler refuses every call in a non-structural body that
its registry resolves to a `#[hardware]` fn, with the same message
(`front_end::check_no_behavioural_instance`, `BEHAVIOURAL_INSTANCE_MESSAGE`). Deleted, nothing legal
using them: `lower_hardware_call`; the `submodules` fields of the combinational and sequential bodies
at CHIR, SHIR and VLIR; the instance's expression-form fields (`inputs`, `output_wire`, `output_ty`,
`output_port`) and `emit::submodules`' legacy branch; `HardwareExecutor::spawn_child`, `spawn_child!`,
`ModuleInfo`, `module_info`, `module_infos` (the one use, `module_composition_hybrid.rs`, wires its
children with `spawn_wired`, a testbench's form). Corpus 0; the emitted SV of all 217 modules
byte-identical. **Not closed by (a)**, measured by probe: a *bound* instance — `let _ = child(..)`,
which rustc's own help line for the refusal suggests, or `let m = child(..)` — compiles, and its child
never runs, while the transpiler refuses it; and a `wire`/`registered_wire` call in a behavioural body
compiles while the transpiler refuses it ("cannot infer bit width", D-18 `b1b`). Each is an OPEN line
of III.3.4 — `II.3 found let-instance` (a decision), `II.3 found body-wire` (the front-ends).)
(D-24 (1), 2026-11-08, Phase C step 11c, `review/MEASUREMENTS.md` §T: the bound instance is **II.1.3's
second named exception** — a `let` binding is a use, so the denied `#[must_use]` does not fire, and the
macro cannot tell that a `let`'s callee is a module, while the transpiler refuses it with this
clause's message. Its silent non-execution is gone: a `HardwareModule` dropped without being
consumed by a spawn or by `copper_sim::__instance` panics with `UNSTARTED_MODULE_MESSAGE` (the drop
guard, silent while another panic unwinds), so `let _ = child(..)` fails at the parent's first
resumption and a `let m = child(..)` live across the module loop fails when the parent's coroutine is
dropped. Every legitimate path consumes the value: corpus 0.)

**I.13.6 The emitted hierarchy and its ABIs — realization, dated.** `copper-transpile
--hierarchy` emits the target and every `#[hardware]` module it transitively instantiates,
deepest first, each once (`transpile_source_hierarchy`, `copper-codegen/src/lib.rs:173-196`;
`hierarchy_emit_order`, `:304-318`); a structural parent is emitted as its nets (`logic n;`) and
its instances with named connections, clocks first (`emit.rs:436-455`, `:480-490`;
`vlir_lower::lower_structural`, `:275`); a `synchronizer` child is a `sequential` module to it
(I.12.6). **`ARRAY_PORT_ABI.md`** (DECIDED 2026-08-24, option B: a packed `[ELS-1:0][W-1:0]` port
with element select; H4) is the array-port realization, and **`RECEIVED_MEMORY_ABI.md`**
(DECIDED 2026-08-27, option (a): the owner's array behind per-port nets on the child, not a
hierarchy edge; I.10.5) is the received-memory realization — the port set a child exposes for a
`Memory` parameter. Both are III's, recorded with their dates and not meanings: nothing in
I.13.1 changes if a port or a memory is realized another way. The one obligation is **III.2's** (D-20: III.2.7):
the emitted hierarchy's trace equals the design's. Measured (D-18 (v)):
`tests/two_domain_hierarchy_cdc.rs` 3/3 (the structural parent against the hand-wired simulation
and the independent reference); `tests/received_memory_abi.rs` 1/1 (the child under a
hand-written owner against the simulator's `Memory`, sim ≡ SV only); `copper-codegen/tests/
structural_hierarchy.rs` 5/5 (the emitted text, the crossing refused and admitted, the leaf
equal to the single emission).

**I.13.7 Implementation map.** Wires: `copper-core/src/port.rs` — `WireId` (`:14`), `WireKind`
(`:28-34`: `Comb` induces a graph edge, `Registered` none), `DirtyHandle` (`:36-61`), `Out`
(`:64-105`, not `Clone`), `In` (`:108-134`, `Clone`), `wire` (`:136-150`), `RegOutShared` and its
listener (`:168-184`), `RegOut` (`:190-218`), `registered_wire` (`:220-235`). Simulator:
`copper-sim/src/executor.rs` — `spawn_untracked` (`:364-375`), `spawn_wired` (`:401-416`),
`spawn_child` (`:427-465`, retired by I.13.5), `comb_cycles` (`:490-496`, detection; D-14 makes
it the refusal), `poll_tasks` (`:520-529`), `poll_tasks_levelized` (`:546-555`),
`poll_task_once` (`:561-572`), `iterate_scc` (`:588-632`), `ensure_graph` (`:639-645`),
`comb_dependency_graph` (`:657-687`: producer → consumer for `Comb` wires in a task's `reads`;
a later spawn with a `Comb` handle on the same `WireId` silently replaces the producer, CODE
(10) 3 — the one-driver rule is the type system's, not the graph's), `compute_scc_plan`
(`:700-751`, Tarjan `:152` and Kahn with the minimum-index tie-break), `poll_tasks_fixpoint`
(`:753-803`), `tick_clock` = `pre_edge_settle` + `post_edge_settle` (`:813-854`),
`SchedulerMode` (`:112-141`, `COPPER_SCHEDULER`), `PollOrder` and `visit_order` (`:226-335`, the
oracle only); `tests/levelized_differential.rs`, `tests/poll_order_fuzz.rs`,
`tests/golden_traces.rs` (both schedulers), `executor::tests::levelized_*`, `comb_cycles_*`,
`poll_order_does_not_change_cross_task_settle`. Macro: the structural arm emits the function
unchanged, not a `HardwareModule` (`copper-macros/src/lib.rs:1385-1398`); `validate_hardware_fn`'s
clock rule (`:1496-1503`) and output rule (`:1512-1517`); `find_unprotectable_in_uses` (R19)
refuses an `In` passed on. Transpiler: `FrontendClassification::StructuralFn`
(`chir_lower.rs:34-35`), `lower_structural_body` (`:2129-2183`), `parse_wire_net` (`:2187-2213`),
`domain_of_type_text` (`:2218-2236`), `lower_structural_inst` (`:2243-2333`),
`structural_signal_name` (`:2338-2350`), the name validation (`:5234-5252`); the legacy
expression form `lower_hardware_call` (`:4961-5008`, from `:3785`; retired); `emit::submodules`
and `structural_body` (`emit.rs:436-490`); `transpile_source_hierarchy` (`lib.rs:173-196`);
`copper-codegen/tests/structural_hierarchy.rs`. Sweep: `two_domain_top` a named `SKIP` row
(`build.rs:118-121`: a parent is the design its children already cover; the two-clock
interleaving is the harness's); every other hierarchy is swept module by module and anchored by
its hand-wired test. Interpreter: `copper-ref` runs one coroutine; a design is an environment
of III.2 (D-20: III.2.7) — a set of interpreter instances over shared values, the functions evaluated per
I.13.2 — and needs I.13.1–I.13.2 and nothing of I.13.4 (a structural module is the design).
**Phase C obligations (D-18):** (1) the structural shape rules of I.13.4(d) enforced in both
front-ends with one message each — T: one driver per net, every net driven, the port kind, the
clock rule; M: the body-form rule, the non-`#[hardware]` call; ui cases from `s1`, `s4`, `s6a`,
`s8b`, `s9`, `s13`; (2) the kind-agnostic net `net::<T, D>()` in `copper-core`, both front-ends and the
one `structural` module, `wire`/`registered_wire` retired from the structural grammar and
`parse_wire_net`'s substring match deleted; (3) instantiation refused in behavioral bodies in
both front-ends (ui case from `b2`), `lower_hardware_call` and the `submodules` fields of the
combinational and sequential bodies, `spawn_child` and `spawn_child!` deleted,
`module_composition_hybrid.rs` rewritten; (4) the combinational-cycle refusal of D-14 over the
design the hierarchy emits — at spawn in the executor from the same graph, and over the
structural parent in the transpiler; (5) the executor's own-domain resumption (D-17 (1)) stated
as I.13.2(b)'s order: `tick_clock::<D>` settles the network, resumes `D`'s coroutines, advances
`D`, settles the network; the barrier and the post-edge resumption gone (D-8); (6) the port set
and mode of a received memory in the parameter type (I.10.8), checked at the structural call
site; (7) the sim-as-unit form, if taken, as a realization with its own differential test
against the hand wiring — dated here, not owed; (8) `LEVELIZED_SCHEDULING_SCOPE.md` noted:
"detection is not rejection" reversed by D-14, order independence a theorem of I.13.2(a);
`SYNCHRONOUS_SEMANTICS.md`'s poll-order section pointed here (the coordinator's edits). (D-22: (1), (2)
and (4) discharged in Phase C step 10 — the shape rules and their ui cases (`s6a` vacuous with `net`),
`net::<T, D>()` and the one structural module rewritten to it, the cycle refusal at spawn and over the
parent, §P.3–§P.4; (3) not done, §P.4; (8) the coordinator's.)
(Phase C step 11, 2026-11-07, read off `924793e`; the map's line numbers above are the 2026-11-05
snapshot, kept as history.) (5) is **realized** since steps 2–3 (`tick_clock::<D>`,
`copper-sim/src/executor.rs:883`: the pre-edge settle resumes `D`'s coroutines once, the post-edge
settle resumes none; `PreEdgeBarrier` gone, §F–§G). (8)'s `LEVELIZED_SCHEDULING_SCOPE.md` note is
**applied** (step 11: "Reversed (D-14 …)" under its phase 5). `iterate_scc` (`:588-632` above) is
**deleted** (§P.3); the refusal is `refuse_comb_cycle` (`executor.rs:568`) over the same SCC plan
(`compute_scc_plan` :761), and the transpiler's is `chir_lower::refuse_structural_comb_cycle`
(:2417). The macro's structural arm (now `copper-macros/src/lib.rs:340`) rewrites `net::<T, D>()` to
`net::<T, D, _>()` and wraps each instance in `copper_sim::__instance` (§P.4); the header checks are
`front_end::check_header` (:688). **(3) is open** — `spawn_child` (`executor.rs:508`), `spawn_child!`
(`copper-sim/src/lib.rs:69-75`) and `lower_hardware_call` (`chir_lower.rs:5850`) stand, neither
front-end refuses a `#[hardware]` call in a behavioural body, and `tests/module_composition_hybrid.rs`
still spawns children; its III.3 line is OPEN with a decision packet (§R). (6) is open (I.10.5's line).
(D-23, 2026-11-08: **(3) is discharged** in Phase C step 11b — option (a) of §R.6 (1), I.13.5's note:
the refusal is rustc's in the macro and the registry's in the transpiler, one message; the deleted
forms are listed there. The bound-instance and body-wire residues are III.3.4 lines.)

**I.13.8 Carried NOT-YET list.**
- The **dual-clock memory** — a memory with one port in each of two domains — is a
  synchronizer-kind primitive with no form (I.12.4(b), D-17): a NOT-YET row with the direction
  "a `structural` primitive of the synchronizer kind whose two ports lie in two domains, with the
  read-during-write across domains OUTSIDE"; no corpus instance → Phase C or a packet of its own.
- The **sim-as-unit** structural module (the parent spawned, 4a; H1) — a realization form,
  deferred 2026-11-05 (I.13.4(a)); if built, a III realization with a differential test against
  the hand wiring, never a meaning → III / Phase C.
- The static combinational-cycle refusal over a hierarchy (D-14, I.11.4) → Phase C (I.13.7 (4)).
- The executor's two resumptions → one, the barrier retired (D-8; I.0.7), and the own-domain
  resumption (D-17) in the order of I.13.2(b) → Phase C (I.13.7 (5)).
- An **independent reference for a hierarchical design** beyond the 2:1 CDC anchor — a
  single-domain parent of a `RegOut` core and a `combinational` reader, and a two-domain parent
  under a second interleaving → III.2 (D-20: III.2.11 item 8) (the user writes references).
- The received memory's port set and mode in the parameter type (I.10.8) → Phase C (I.13.7 (6)).
- X propagation through the settle — a function evaluated over an X input, a `RegOut` holding
  `Bits::x()` read by a function, X on a net → I.14. (D-19: I.14.2(g) — closed.)
- `poll_tasks` outside a tick and a mid-cycle drive (I.1.5, I.1.6) → III.2 (preconditions; D-20: III.2.3).
- The audit's executor-fact row ("a fixpoint re-poll runs no statement", `review/DECISION_AUDIT.md`
  rows 54 and 98) — resolved by I.13.2(b) as a consequence; the coordinator's edit.

### I.14 Data: `Logic`, `Bits`, X, widths

Decided: D-19 (2026-11-05). Anchor: IEEE 1800 4-state operator semantics as Icarus Verilog 12.0
executes them — Copper's X algebra (`copper-core/src/types.rs`) against it on 400 X-sprinkled operand
rows per operator, bit for bit: equal on every row of every operator except `eq_logic`/`ne_logic`
(209/400 rows more X than IEEE, none less; decided IEEE-exact, I.14.2(h)) and the derived `==`, which
is IEEE's `===` (equal on 400/400 against `===`) (D-19 (i-b); `cummings.md` and the other notes are
silent on X propagation). Sim ≡ reference interpreter: every operator class on 20 % X inputs (D-19 (i));
`RefBits` ≡ `Bits<N>` at seven widths (`copper-ref/tests/bits_property.rs`). Sim ≡ SV only: modular
arithmetic, shifts by ≥ N, `!` as `~`, the out-of-range address under zero-fill. Decided, not anchored:
the type set and the signed types, `usize`'s width (a consequence of the simulator being rustc), the
constant grammar, the width-constraint rule, the out-of-range address and the port index.

In coroutine terms (D-7, D-8): a value is what the Rust coroutine computes, typed by rustc; the
hardware's values are the same bits; X is a third bit value the coroutine can hold and the 2-state
realization cannot, so it is a value of the kernel and a precondition of the agreement relation, never a
meaning of the emitted text.

**I.14.1 The type set.** The payload `T` of a port, a register, a wire, a constant, a memory word and
every expression is one of:

| Type | Values | Width w(T) | Zero | SV type | Where |
|---|---|---|---|---|---|
| `Logic` | 0, 1, X | 1 | `Logic::Zero` | `logic` | payload, local |
| `Bits<N>`, 1 ≤ N ≤ 128 | N bits, each 0, 1 or X, index 0 the LSB | N | `Bits::zero()` | `logic [N-1:0]` (`logic` at N = 1) | payload, local |
| `bool` | false, true — no X | 1 | `false` | `logic` | payload, local |
| `u8 … u128`, `i8 … i128` | Rust's (two's complement for `iN`) | N | `0` | `logic [N-1:0]`, `$signed` where signedness is observable | payload, local |
| `usize`, `isize` | Rust's on the host | 64 | `0` | `logic [63:0]` | payload, local |
| `[T; K]` | K values of T | K · w(T) | K zeros | packed 2-D (`ARRAY_PORT_ABI.md`) | payload, local |
| a body-local `enum` | its unit variants (a value set) | the encoding's | none (initialized) | the transpiler's encoding | local only |
| a struct, a tuple | one local per field or element (I.2.4) | — | — | — | local only |

(a) **Signed types — decided (D-19 1A):** admitted wherever an unsigned integer is, ports included
(`check_admissible` admits them today, `copper-analysis/src/lib.rs:773-799`), with Rust's meaning;
overflow is I.14.3(c)'s run-time failure. Corpus: no signed payload; 6 signed sites in 3 bodies (5
`as i32`, 1 annotated `let`). (b) **`usize`/`isize` are 64 bits** — the simulator is rustc's lowering
on the host (`SPEC_OVERVIEW.md` §1), so no other width is the coroutine's; the transpiler resolves
them as 32 (`chir_lower.rs:135-139`), and `let v: usize = a.read().as_usize(); (v << 40) >> 40` is `a`
on the simulator and the interpreter and 0 in the SV (measured 64/64, D-19 (v)) — a Phase C defect
(I.14.7 (3)), masked in the corpus (no swept module carries a `usize` value past bit 31). (c) `Bits<N>`
with N > 128 is OUTSIDE: its arithmetic does not monomorphize (`as_u128`'s assert, `types.rs:281-284`)
and `RefBits` stops at 128; `Bits<0>` likewise (no value to observe). The harness compares ports of at
most 64 bits (`verification.rs:330-340`, a `u64` literal) — a III.2 precondition (D-20: III.2.3), corpus-masked
(the widest `Bits` anywhere is 64). (d) `Logic` and `Bits<1>` are distinct Rust types of width 1 with
`From` both ways (`types.rs:804-810`), realized as the same SV scalar (I.14.5(c)). (e) The zero of a
type is the column above: it is what every port and wire is created at (I.6.6, I.11.2(e)) and what
`Memory::new` fills (I.10.3). Measured (D-19 (vii)), 374 `#[hardware]` fns: port payloads `Bits<lit>`
651, `Logic` 237, `Bits<param>` 40, `u8` 5, arrays 2, `bool` 0. Prior work: Arch's `Bit`, `UInt<N>`,
`SInt<N>`, `Bool` [arch §3.1 Table 1]; Clash's Bit, Bool, Signed/Unsigned, Vector, Index [clash §II-C];
MyHDL's `bool`/`intbv`/`int`/lists [myhdl §10]; Chisel's `UInt`/`SInt` and Spade's integer types —
not covered by the notes. R16 (`check_admissible`) approximates this set from below (II.2.2); the
transpiler's `resolve_type` (`chir_lower.rs:117-146`) and the interpreter's `parse_ty`
(`copper-ref/src/types.rs`) are its other two readers — the interpreter refuses integer ports today
(measured, `int_wrapping`), a Phase C item.

**I.14.2 Values and X.** (a) **The value.** A `Bits<N>` value is N bits, each 0, 1 or X; X is **per
bit** (`types.rs:145-147`: `[Logic; N]`); there is no Z (`types.rs:14-19`) — a wire has one driver
(I.13.1), so nothing is ever undriven. A `bool` and a Rust integer have no X.
(b) **The X rule — decided (D-19 2-i): the simulator's, normative,** with `eq_logic`/`ne_logic` per
(h). Measured equal to IEEE 1800 4-state on every row (D-19 (i-b)):

| Operator | X rule | IEEE 1800 |
|---|---|---|
| `!` (bitwise on `Bits`, logical on `Logic`), `&`, `\|`, `^` | per bit: `!X = X`; 0 dominates `&`, 1 dominates `\|`; `^` X if either is X (`types.rs:57-115`, `:626-672`) | `~ & \| ^`, equal |
| `+ - * %`, unary `-` | all bits X if any operand bit is X; `%` by zero all X (`:676-746`) | equal |
| `/` | the same; by zero all X (`:721-728`) | equal |
| `<< n`, `>> n` (`n: usize`) | the bits move, X with them; vacated bits 0; n ≥ N gives 0 (`:332-348`) | equal |
| `arithmetic_shift_right(n)` | vacated bits take bit N−1, X included; n ≥ N gives N copies of it (`:388-395`) | `$signed(a) >>> n`, equal |
| `and/or/xor_reduce` and their negations | the fold of the per-bit rule (`:367-383`) | `& \| ^ ~& ~\| ~^`, equal |
| `Bits::mux(sel, a, b)` | `sel` X: the bits where `a` and `b` agree, X elsewhere (`:494-506`) | `sel ? a : b`, equal |
| `lt/le/gt/ge_logic` | X if any operand bit is X, else the unsigned comparison (`:408-426`) | `< <= > >=`, equal |
| `eq_logic`, `ne_logic` | (h): IEEE's | `==`, `!=` |
| derived `==`, `!=` | structural, a `bool`: X == X is true (`:144`) | `===`, `!==`, equal |
| `sign_extend`, `zero_extend`, `truncate`, `part_select`, `concat`, `replicate`, `get`, `set`, `with_lsb`/`with_msb` | the bits move, X with them (`:431-488`, `:304-314`, `:351-362`) | the corresponding select/concatenation, equal |

(c) **X where no value can stand is a run-time failure** (OUTSIDE, as `data()` on an undelivered port,
D-16; no value exists for the resumption to continue with, I.2.5(a)): `Logic::as_bool` of X (and every
`if`/`match` that needs a `bool` from a `Logic` through it), `as_u8 … as_u128`/`as_usize` of a value
with an X bit (`types.rs:28-35`, `:288-301`) — hence an X shift amount, an X index into an array or a
memory, an X `from_usize` argument. Measured (D-19 (i)): the simulator panics ("Cannot convert X to
bool|integer") and the interpreter errs at the same cycle, on `xp_cond`, `xp_shift`, `c_narrow`,
`c_signed`, `usize_width`. A condition through the derived `==` (`if s == Logic::One`) is defined: X is
not `One`, the `else` arm runs — IEEE's `if (s)` takes the same arm (400/400). (d) **Sources of X:**
a register initialized with `Bits::x()`/`Logic::X` (I.7.4(b): X until its first assignment, a constant
X of I.7.2(b) if never assigned); an operator over X or `/`, `%` by zero; a word of a memory whose
initial array holds X (I.14.6(d)); an X input the environment drives (I.1.5: a value of `T` like any
other — the realization's testbench cannot drive one, I.14.2(f)); a write of X (e). (e) **A write of
`Bits::x()` or `Logic::X` to a `RegOut` or a wire — decided (D-19 2b-A): admitted;** the port shows X
from the observation the write reaches (I.6.4; on a wire, the instant's, I.11.2), outside the agreement
relation by (f). Measured (`xw_port`): the macro admits it, simulator and interpreter show X on every
written cycle; the transpiler refuses it ("non-hardware function calls cannot appear in hardware
expressions") — a lowering limitation II.3 owes a row for (I.14.8), fix: emit the type's X literal.
Corpus 0. (f) **The realization and the X precondition** (handed to III.2; D-20: III.2.2). Verilator is 2-state: an
unstated register powers on at 0, or at a seeded random value under `COPPER_VERILATOR_X_INITIAL=1`
(`verification.rs:497-560`), and an operator that is X in 4-state yields a 2-state value (`% 0` is 0:
`xp_rem` FAILS exactly the 9 of 64 cycles with a zero divisor, both power-on modes). The harness
compares an expected X bit **as 1** (`logic_vec_to_int`, `verification.rs:330-340`): it is neither
skipped nor tolerated, and fails unless Verilator happens to show 1 (`xp_reg`: a `Bits::x()` register
read before its first load fails 9 checks under zero-fill and 9 under random power-on). The
precondition this document states: **an observation bit the coroutine shows as X is outside the
agreement relation**, and a module with an X register is compared only from the first assignment of
every such register on every path (the `RESET` rows, I.7.4(b), I.9.5); the form — masking X bits, or
requiring that no recorded observation holds one — and the inputs' X-freedom are III.2's (D-20: III.2.2 — refused against the SV, not masked; III.2.3). (g) **X
through a design** (I.12, I.13): a `RegOut` holding X is a source of the settle like any other
(I.13.2(a)); a `combinational` function evaluates (b)'s table over X inputs; a net carries the per-bit
value its driver holds; a synchronizer's foreign read of an X `RegOut` reads X, the bits crossing
unchanged (I.12.3 — the two-flop rule is about time, not value; metastability is not modelled). No
case is new: each is (b) at the reading module. (h) **`eq_logic`/`ne_logic` — decided (D-19 2a-B):
IEEE-exact.** `a.eq_logic(&b)` is 0 when some bit pair is known and differs, else X if any bit is X,
else 1; `ne_logic` its negation. Today the simulator and `RefBits` return X on any X operand bit
(`types.rs:400-405`; 209 of 400 Icarus rows) — Phase C (I.14.7 (1)); corpus 0 uses, and the transpiler
refuses both (I.14.3(d)). Prior work: Arch is 2-state, "no per-bit X/Z state", and guarantees "no X
from uninitialized state" [arch §7.2, §6.1, Table 6]; Chisel's `Reg` without `resetVal` is X at
power-on in the Verilog with the C++ simulator's start value unstated [chisel §2.6, §10.2]; MyHDL's
registers are X in the HDL until reset while its simulator holds the initial value [myhdl §6.3.1,
§11.2]; Verismith initialises at power-on so that no X arises [verismith §3.1 p.279]; Clash's
`undefined`, Chisel's `DontCare`, Spade's undefined enum payload [spade Fig. 2] — not covered as X
propagation.

**I.14.3 Operators and methods.** (a) **On `Bits<N>`:** `+ - * %` and unary `-` are modular at N
(wrapping, `types.rs:676-746`); `& | ^ !` bitwise; `<<`/`>>` by a `usize`, logical, n ≥ N gives 0;
comparisons are the derived `==`/`!=` (a `bool`, I.14.2(b)) and the `*_logic` family (unsigned, a
`Logic`); `Bits` has no `PartialOrd`, so `<` is not available on it; a width changes only through the
methods of (d). Measured (D-19 (i-c)): `xp_arith`, `xp_bitwise`, `xp_shift` (amounts 0..15 on 8 bits),
`xp_not` (`!` lowers to `~`), `xp_eq` agree sim ≡ SV under both power-on modes. (b) **`/` — decided
(D-19 3A):** truncating unsigned division, all X on an X operand or a zero divisor (`types.rs:721-728`;
equal to IEEE on 400/400 rows). It is refused in both front-ends until the transpiler lowers it —
the transpiler lacks the form (`chir_lower.rs:4822`, the `_` arm of `lower_binop`), so R15 is a dated
lowering limitation of II.3, not a shape rule (I.14.8: II.3 owes the row, II.1 restates R15); fix
direction: lower `/` as `%` is lowered. The macro's refusal is R15's parity obligation (D-12 A 2b).
Corpus 0. (c) **Rust integers and `bool`** keep Rust's meaning: `+ - *` overflow and a shift by ≥ the
width are run-time failures (OUTSIDE; the simulator panics in the debug profile the sweep runs,
measured `int_overflow`: "attempt to add with overflow"); `wrapping_add/sub/mul` are modular
(`int_wrapping` agrees sim ≡ SV); `saturating_add/sub` clamp at the type's bounds — IN, a method the
transpiler refuses by name (`chir_lower.rs:4919`), row (e); `checked_add/sub` return an `Option` —
OUTSIDE (I.2.5(a), II.2 D3); `as` between integers truncates or extends by Rust's rule (zero for
unsigned, sign for signed sources); `&& || !` on `bool` are Rust's; comparisons on integers are Rust's.
(d) **Every `Bits`/`Logic` method of `types.rs`, with its meaning and status** (realized = the
transpiler lowers it; (e) = II.3 row (e), meaning stated here):

| Method | Meaning | Status |
|---|---|---|
| `zero()`, `all_ones()`, `x()`, `from_lit::<V>()`, `from_u8 … from_u128(v)`, `from_usize(v)` | the constants of I.14.4; `from_usize` of a value that does not fit is a run-time failure (`types.rs:222-226`) | realized: `zero`, `from_lit` (decimal), `from_uN`, `from_usize`; `all_ones` in every spelling, a non-decimal `from_lit` and `x()` in an expression: refused (I.14.4, I.14.2(e)) |
| `from_array(a)`, `from_slice(s)` | the bits of `a` (LSB first); a slice of the wrong length a run-time failure | (e) |
| `as_u8 … as_u128`, `as_usize`, `as_bits`, `as_bool` | the value as a Rust integer (zero-extended to the integer's width) / `bool`; X a run-time failure (I.14.2(c)) | realized as passthroughs, with the width defect of I.14.5(b) |
| `get(i)`, `set(i, l)` | bit `i`; `i ≥ N` a run-time failure (`types.rs:304-314`) | (e) |
| `as_array`, `as_array_mut`, indexing `x[i]` (`Deref`) | the bit array, LSB first; `x[i]` is bit i | indexing realized (width-1 defect, I.14.5(c)); `as_array`, `as_array_mut` (e) |
| `shift_left(n)`, `shift_right(n)` | `<<`, `>>` | (e) |
| `arithmetic_shift_right(n)` | I.14.2(b) | realized (`$signed(a) >>> n`, `chir_lower.rs:4846-4854`) |
| `with_lsb(l)`, `with_msb(l)` | `self` with bit 0 / bit N−1 replaced | (e) |
| `and/or/xor/nand/nor/xnor_reduce` | the reductions (`Logic`) | (e) — CHIR already has `ReductionAnd/Or/Xor` |
| `eq_logic`, `ne_logic`, `lt/le/gt/ge_logic` | I.14.2(b), (h) | (e) — `eq_logic` and `ne_logic` are missing from row (e)'s list today (I.14.8) |
| `zero_extend::<M>()`, `sign_extend::<M>()`, `truncate::<M>()` | width casts; a wrong-direction M a run-time assertion in the simulator, a fact at the instance | (e) |
| `part_select::<L>(lo)`, `concat::<M, OUT>(o)`, `replicate::<OUT>()` | `x[lo +: L]`, `{self, o}`, `{OUT/N{x}}`; out of bounds / a wrong OUT a run-time failure | (e) |
| `Bits::mux(sel, a, b)` | `sel ? a : b`, I.14.2(b) | (e) |
| `is_valid()` (`Bits`, `Logic`) | whether no bit is X — a question about the simulator's third value, which no realization holds | OUTSIDE (a shape refusal owed to II.1, I.14.8) |
| `==` / `!=` with an X literal operand (`v == Logic::X`, `Bits::x() != v`) | the derived structural equality against X — `is_valid()` spelled with an operator | OUTSIDE (D-23 (3), 2026-11-08): refused in both front-ends by `check_outside_methods` with one message citing (d); `eq_logic`/`ne_logic` with an X operand are IEEE values (X), unaffected |
| a pattern matching the X literal (`Logic::X` anywhere in a pattern — `match`, `if let`, `let`, `matches!` —, or an identifier pattern naming a body `const` whose initializer is an X literal) | the structural match against X — `is_valid()` spelled as a pattern | OUTSIDE (D-24 (2), 2026-11-08): refused in both front-ends by `check_outside_methods` with one message (`X_PATTERN_MESSAGE`) citing (d); a file-level `const` is outside the `ItemFn` both rules hold — a pattern naming one is line `II.3 found const-pattern` |
| `Logic::from_bool`, `From<Logic> for Bits<1>`, `From<Bits<1>> for Logic` | the identity on the bit | `From` refused (measured `w1_from`) — Phase C, I.14.5(c) |
| `wrapping_add/sub/mul`, `saturating_*`, `checked_*` (integers) | (c) | `wrapping_*` realized; `saturating_*` (e); `checked_*` OUTSIDE |

(e) **Patterns** (I.2.3's `p`) match by Rust's rule: literal, enum-variant, tuple, range and wildcard
patterns over integers, `bool`, `Logic` and enums; a `Logic` scrutinee holding X matches `_` only — a
`Logic::X` pattern is OUTSIDE, the `is_valid` question of (d) (D-24 (2)). (D-23, 2026-11-08: measured by
probe, a `Logic::X` *pattern* was admitted by the macro and refused by the transpiler ("unsupported
pattern"); D-24 (2), the same day, Phase C step 11c: OUTSIDE, refused in both by
`check_outside_methods` with one message, `review/MEASUREMENTS.md` §T.) (f) **The port-write cast** (e5d103b,
`chir_lower.rs` `resize_to_target` at the write, `:4450-4460`) is a realization: rustc has already typed
the written value at the port's type, so the meaning has no cast; the emitted width cast reconciles the
transpiler's width of the value with the port's, and it cannot repair a value whose own width the
transpiler got wrong (I.14.5(b)). Prior work: Clash's Signed/Unsigned wrap around [clash §II-C]; Spade
widens and makes truncation explicit [spade §III-B]; Verismith guards division by zero at run time and
reports signedness as the richest source of synthesis bugs [verismith §3.3 p.280, Fig. 1 p.277]; Arch,
MyHDL — not covered.

**I.14.4 Constants — decided (D-19 4A): Rust's constants.** A constant `c` (I.2.3's `s_pre`
initializers, a counted `for`'s bound, a memory's initial array, a const-generic argument) is an
expression rustc can evaluate at compile time over:

```
c ::= integer literal (any base, underscores, a type suffix) | true | false
    | Logic::Zero | Logic::One | Logic::X | E::V                      an enum unit variant
    | Bits[::<N>]::zero() | ::all_ones() | ::x() | ::from_lit::<c>() | ::from_uN(c) | ::from_usize(c)
    | K                                                                  a `const` item whose body is a c
    | P                                                                  a const generic of the instance
    | T::K                                                               an associated const
    | f(c, …)                                                            a `const fn` of the file
    | ( c ) | c as T | op c | c op c | [c, …] | [c; c]
```

with Rust's meaning, the width of a `Bits` constructor taken from Rust's inference when the turbofish
is absent. `Bits::x()` (and `Logic::X`) as an initializer is the unstated value of I.7.4(b). A constant
whose evaluation fails (overflow, `from_lit` too wide — `types.rs:247-251`'s post-mono assert) is a
compile error. **The three evaluators against it:** (1) R11 is rustc since D-21 A3 — the macro evaluates a
register initializer in a `const { }` block, every `Bits`/`Logic` constructor a `const fn` — exact
by construction; until D-21 it was `is_constant_expr` (`copper-analysis/src/cfg.rs`), syntactic —
over on calls (any multi-segment path with constant arguments passed, including a non-`const`
associated function), under on a single-segment `const fn` call, a const generic or a `const`
spelled in lower case (refused); (2) the transpiler's fold (`chir_lower.rs:1158-1300`,
`classify_value_ctor` `:654-706` with `trailing_const_param`'s decimal `parse::<u128>` `:698-707`) is
under: no `all_ones` in any spelling, no hex or underscore literal in a `from_lit` turbofish, no
non-literal `const` body, no associated const, no const generic as a value, no `const fn` (II.3 rows
(d), (g)); `eval_const_usize` (`:4688-4699`) folds integer literals only; (3) the interpreter
(`copper-ref/src/interp.rs:193-200`, `:1240-1300`) folds literals in any base, `const` items and const
generics, and refuses `all_ones()` without a width turbofish (a parity item). Measured (D-19 (iv)): the
macro admits `from_lit::<0xA5>` (body, initializer), `from_lit::<1_000>`, `all_ones` with and without a
width (body, initializer); simulator ≡ interpreter on each (the interpreter refusing the unsized
`all_ones`); the transpiler refuses all six ("non-hardware function calls cannot appear in hardware
expressions" in a body, "cannot infer bit width" as an initializer). Corpus: 264 `from_lit`, none
non-decimal; `all_ones` 0; `Bits::x()` 5 initializers. **The direction (D-21 B2):** one grammar, the
interpreter's evaluator independent of the front-ends': rustc judges R11 (A3);
`copper_analysis::consts` serves the transpiler's fold, counted-`for` bounds and constant indices;
the interpreter keeps its own, so the reference leg shares no constant code with the transpiler
(III.2's attribution); the two are pinned against each other on every corpus constant
(`tests/const_evaluators_agree.rs`, G-J) and both against rustc by the sweep. Rows (d) and (g) are
closed (I.14.7 (2)).

**I.14.5 Widths.** (a) **Expression widths are Rust's types:** `Bits<N> op Bits<N>` is `Bits<N>` for
every operator of I.14.3(a) — no growth, the carry discarded (Clash's reading, not Spade's widening
[spade §III-B] or Chisel's growing inference [chisel §10.4]); a comparison is 1 bit (`bool` or
`Logic`); a reduction 1; `x[i]` on a `Bits` is 1 bit and on an array its element's width (e5d103b,
`indexed_array_elem`, `chir_lower.rs:4417-4430`); the methods of I.14.3(d) state their own. A port write
has the port's type (rustc). (b) **The passthrough width defect (measured, Phase C).** The transpiler
lowers `as_u32()`/`as_usize()` to the receiver itself (`chir_lower.rs:4885-4890`) and types an
unannotated `let` from it, so the value keeps the `Bits` width instead of the integer's:
`let v = a.read().as_usize(); (v << 30) >> 30` is `a` on the simulator and the interpreter and 0 in the
SV (`usize_width`, 64/64 cycles: `v` declared `logic [7:0]`); `let v = a.read().as_u32()` of a 16-bit
`a` is declared 8 or 16 bits and fails `-Wall` (`c_narrow` UNUSEDSIGNAL, `c_narrow2` WIDTHTRUNC). With
(b) of I.14.1 (`usize` 32), this is the integer side's realization gap: the fix lowers each `as_*` view
as a zero-extension to its integer's width and `usize`/`isize` as 64 bits (I.14.7 (3)); masked in the
corpus (the sweep passes every swept module that uses `as_usize`, 51 call sites). (c) **Width 1.**
`Logic` and `Bits<1>` are both emitted as the scalar `logic` (`emit.rs:753-760`, `range_str`), and a
constant index is a one-bit select (`chir_lower.rs:4677-4680`), so `v[0]` on a `Bits<1>` is emitted as
`v[0]` on a scalar, which Verilator refuses ("Illegal bit or array select; type does not have a bit
range"): measured on the sequential `w1_index` (`q <= v[0]`) and on I.11.7's combinational draft
`w1_comb` (`assign q = a[0]`), both admitted by both front-ends; `Logic::from(v)` / `Bits::from(l)` are
refused by the transpiler (`w1_from`). Fix direction: a select of bit 0 on a width-1 operand lowers to
the operand, and `From` to the identity (I.14.7 (4)). (d) **Generic widths.** A generic module means
the family of its instances (I.2.2); the transpiler emits one parametric module (`#(parameter int N)`)
and the sweep Verilates it at `build.rs`'s `PARAMS` widths (7 rows). **A width constraint — decided
(D-19 5A) — is a shape rule at instantiation:** `const { assert!(N_LOG == safe_clog2(N)) }` (ERASED,
I.2.4; 7 files) makes an instance that violates it not exist — rustc refuses it at monomorphization,
which is the rule's enforcement on the Rust side; the emitted parametric module states nothing today
(`rotate_right`'s SV admits any `N_LOG`), and owes an elaboration-time check of each constraint (a
`generate if (!(…)) $error(…)`, I.14.7 (8)); until it lands, III.2 (D-20: III.2.3) holds the precondition that the SV is
instantiated only at parameters the Rust instance compiles at.

**I.14.6 Memory data and addresses** (I.10). (a) **Address width.** A memory of `depth` words has
⌈log₂ depth⌉-bit address nets (depth 10 → `logic [3:0]` nets, `logic [7:0] mem [0:9]`, measured
`mem_oob`), the staged address cast to it (cause Q, `vlir_lower.rs:782-830`); the address operand
is a `usize` (I.14.1(b)). (b) **An out-of-range address — decided (D-19 6a-A): OUTSIDE, a run-time
failure,** as `data()` on an undelivered port (D-16): the trace ends at the call. The simulator panics
there (`copper-core/src/memory.rs:116-138`, measured at the first address ≥ 10); the SV reads 0 and drops
the write under zero-fill (PASS against that hand model over 19 out-of-range cycles) and reads a random
word under `COPPER_VERILATOR_X_INITIAL=1` (19/19 FAIL); IEEE reads X. None of these is a meaning.
(c) **A port index `k ≥ R` (`≥ W`) — decided (D-19 6b-A): a shape rule.** `k` is a const generic,
known at the instance; the program is refused at compile time in both front-ends (a post-mono assert in
`read_port::<K>`/`write_port::<K>`, the transpiler's check) — today a run-time panic
(`memory.rs:195-201`, CODE (7).2), a Phase C rule (I.14.7 (9)) owed to II.1 as a row. (d) **X in a
memory.** A word holds the per-bit value of I.14.2(a); a memory whose initial array holds X words
(`from_contents` of `Bits::x()` elements) delivers them as any word; an **X-initialized memory** in
the sense of an unstated array is no form of this document — `Memory::new` is the all-zero array
(I.10.3) and there is no constructor that leaves the array unstated; an X address is I.14.2(c)'s
run-time failure. Prior work: Verismith guards out-of-range selects as undefined [verismith §3.3
p.280]; Chisel's `Mem` out of range — not covered.

**I.14.7 Implementation map.** Values and operators: `copper-core/src/types.rs` — `Logic` (`:14-140`),
`Bits<N>` (`:145-830`: constructors `:155-259`, views `:261-301`, bit access `:304-327`, shifts
`:332-395`, the `*_logic` comparisons `:400-426`, width conversions `:431-488`, `mux` `:494-506`,
operators `:626-758`, `From` `:804-822`); `RefBits` (`copper-ref/src/bits.rs`), pinned by
`copper-ref/tests/bits_property.rs`; the interpreter's value and type grammar (`copper-ref/src/
value.rs`, `types.rs`) and constant evaluation (`interp.rs:193-200`, `:1240-1300`). Admissibility:
`check_admissible` (`copper-analysis/src/lib.rs:773-870`, R16), `is_constant_expr`
(`cfg.rs:3519-3550`, R11). Transpiler: `resolve_type` (`chir_lower.rs:117-146`), `classify_value_ctor`
and its turbofish parsers (`:640-720`), `lower_register_init`/`fold_const_init` (`:1158-1300`),
`lower_index` (`:4640-4690`), `eval_const_usize` (`:4688-4699`), the width walks `expr_width`/
`width_of_chir_expr`/`resize_to_target` (`:4390-4560`), `lower_binop`/`lower_unop` (`:4802-4840`),
`lower_method_call` (`:4842-4937`); `emit.rs:753-760` (`range_str`, width 1 scalar); `vlir_lower.rs:
782-830` (cause Q). Memory: `copper-core/src/memory.rs:116-138`, `:195-201`. Harness:
`copper-sim/src/verification.rs:330-340` (X compared as 1; a `u64` literal), `:497-560` (power-on).
**Phase C obligations** (each with its probe from D-19 as a fixture): (1) `eq_logic`/`ne_logic`
IEEE-exact in `types.rs` and `RefBits`, the property pin re-run, and an IEEE table pin beside it;
(2) I.14.4's grammar with two evaluators pinned against each other (D-21 B2: `copper_analysis::consts`
for the transpiler's fold and `eval_const_usize`, the interpreter's own, `tests/const_evaluators_agree.rs`
between them) and R11 judged by rustc (D-21 A3) — DONE (Phase C step 8, D-21) — closes II.3 (d), (g), `all_ones` in every spelling,
underscores (`k_hex_body`, `k_hex_init`, `k_underscore`, `k_ones_*`); (3) the `as_*` passthroughs
lowered as zero-extensions to the integer's width and `usize`/`isize` as 64 bits (`usize_width`,
`usize_annot`, `c_narrow`, `c_narrow2`); (4) the width-1 select and `From` (`w1_index`, `w1_comb`,
`w1_from`); (5) `/` lowered as `%` is, and the macro's R15 refusal until then (`xp_div`); (6) `Bits::x()`
and `Logic::X` in an expression emitted as the type's X literal (`xw_port`); (7) the row-(e) lowerings,
`eq_logic`/`ne_logic`/`as_array_mut`/`from_array`/`from_slice` among them, `saturating_*` lowered,
`checked_*` and `is_valid` refused as OUTSIDE in both front-ends; (8) each width constraint emitted as an
elaboration-time check (`rotate_right` at a violating parameter as the fixture); (9) the memory
port-index rule in both front-ends (a ui case); (10) the interpreter learns integer and `bool` ports,
`From`, `/`, and the unsized `all_ones`.

**I.14.8 Carried NOT-YET list.**
- III.1 (D-20: P7): the property pin's uncovered operators — the constructors (`zero`, `all_ones`, `x`,
  `from_lit`, `from_uN`, `from_usize`, `from_array`, `from_slice`), `as_u8 … as_u64`/`as_usize`,
  shift amounts above N+1, `Logic`'s `!` and `as_bool`, `From`, `as_array`, widths outside
  {1, 4, 8, 16, 32, 64, 128}, and the Rust integer types, `bool` and arrays, none of which `RefBits`
  models; the IEEE comparison of D-19 (i-b) as a standing check (Icarus is optional, so a recorded table).
- III.2 (D-20: III.2.2–III.2.4; the form decided — refused against the SV, not masked): the X precondition of I.14.2(f) and its form (mask an X bit, or refuse to record one), X-free
  inputs, the ≤ 64-bit harness port, the generic-width precondition until I.14.7 (8) lands; an
  out-of-range address, an X in a condition or a view, and integer overflow as run-time failures
  outside the relation.
- II.3 owes rows (the next II.3 edit): `/` (R15, from II.1); `Bits::x()`/`Logic::X` in an expression;
  the passthrough and `usize` widths (ADMITTED-AND-DIVERGENT: `usize_width`, `usize_annot`; lint-refused
  as emitted: `c_narrow`, `c_narrow2`); the width-1 select (admitted by both front-ends, refused by
  Verilator as emitted: `w1_index`, `w1_comb`); row (e) widened by `eq_logic`, `ne_logic`,
  `as_array_mut`, `from_array`, `from_slice`, the `From` conversions and `all_ones`; row (g) widened to
  underscore literals.
- II.1 owes rows: the memory port index (I.14.6(c)); `checked_*` and `is_valid` (OUTSIDE); `Bits<N>`
  with N > 128 or N = 0 as R16's bound; R15 restated as II.3's.
- The inclusive counted `for` (`a..=b`): I.2.4 admits exclusive ranges only and the interpreter
  refuses the inclusive one; corpus 0 → Phase C, with II.1's parity list.
- `SPEC_OVERVIEW.md` §6's count of II.3 rows — the coordinator's edit when II.3 gains the rows above.

## Part II — Admissibility

### II.1 Shape rules

Decided: D-12 (2026-11-05). Anchor: none of its own — the rules are definitions over the
kernel (I.2.2–I.2.3) and the coroutine (I.0.1); the one hazard a shape rule names that was
ever observed, the zero-time cycle, was measured on the simulator before the rule existed
(`copper-analysis/src/cfg.rs:3078-3090`, 99.5 % CPU and no progress). Every refusal set below
is measured over the corpus (364 `#[hardware]` modules in `examples/`, `src/`, `tests/`: 319
`sequential`, 5 `synchronizer`, 39 `combinational`, 1 `structural`; 162 in the sweep
directories, 135 of them clocked — D-8's population), predicted first, and every parity
verdict was measured on a probe in both front-ends (D-12 §0). Decided: the basis of a shape
rule (the five clauses and the grammar), the enumeration, the placement of
`check_admissible`, the refusal of three NOT-YET shapes, the parity rule and its open breaks.

In coroutine terms (D-7, D-8): a shape rule refuses a function that is not a hardware
coroutine — one that could suspend other than at a `tick`, run forever inside one
resumption, hold unbounded state, return, or drive a wire from a clocked body. It says
nothing about what a program means; a program it refuses has no trace.

**II.1.1 What a shape rule is.** A shape rule is a refusal that cites a clause of the shape
principle (I.0.6; `SPEC_OVERVIEW.md` §5). The principle's sentence has **five** predicates;
each is a clause below, with its basis in the kernel grammar (I.2.2, I.2.3), and the grammar
itself is the sixth, **S0**, because the grammar *is* the coroutine's shape written down
(D-12 A, sub-clause 1): a construct outside I.2.2–I.2.3 to which no clause of Part I gives a
meaning is refused citing the production it fails, never as a lowering limitation — II.3 is
for forms the transpiler lacks, and a labelled `break` is a form the *kernel* lacks.
- **S0 — the grammar.** The header is I.2.2's (`#[hardware(mode)]`, one `Clock<D>` on a
  `sequential` module, ports `In`/`RegOut`, `Out` on a `combinational` module only, `Memory`,
  no return type, at least one output; the body `s_pre ; loop { s }` with the `loop` last and
  only); the body is a production of I.2.3 — `break` and `continue` unlabelled and of the
  innermost loop, `e` a pure expression over I.14's operators (D-19: I.14.3), an input read only as the
  hoisted `let t = read x`, no macro outside `const { }` (`matches!` is DESUGARED, I.2.4),
  the name classes `v`, `t`, `x`, `o`, `q`, `m` disjoint (a `let` may not shadow a port). The
  flag `allow_pretick_alignment` is outside the header (I.2.2, D-8).
- **S1 — one suspension kind.** The only `await` is `clk.tick().await` on a `Clock`
  parameter (I.2.3's `tick`; I.2.5(b)): a nested `async`, a `yield`, any other awaited value
  is a second suspension the protocol of I.0.2 does not resume.
- **S2 — every loop path reaches a `tick`.** Every path from the module loop's head back to
  it, every path around a nested loop, and every exit of a nested loop that the enclosing
  path relies on for its boundary, reaches a `tick` of a clock the environment drives
  (I.2.5(b)-(c), I.3.6, I.8.2(a), I.8.4(a)); a tick-free body is the degenerate case
  (I.2.2). A `Clock` the body constructs is driven by nothing, so a `tick` on it never
  resolves and the resumption never ends: `Clock::new` is an S2 refusal (I.2.5(b)).
- **S3 — the state is finite.** No unbounded work inside one resumption (I.2.5(c)): a
  tick-free nested `loop` or `while`, recursion through a helper, a tick-free `for` whose
  bound is not a constant of I.14 (D-19: I.14.4; the constant-bound form is unrolled by `D`, I.2.4); and
  the entry is a *state* — `s_pre ::= let v = c`, every pre-loop initializer a constant of
  I.14 (D-19: I.14.4) or `Bits::x()` (I.7.4(c), I.9.2: H(M)'s initial state is a value, not a computation
  over inputs).
- **S4 — it never returns.** `break` of the module loop and `return` end the coroutine,
  which has no meaning after I.1.1's infinite edge sequence (I.2.5(d), I.8.1).
- **S5 — its outputs are registers.** Every output of a `sequential` or `synchronizer`
  module is a `RegOut` (I.2.2, I.6, D-8); a plain `Out` is a `combinational` module's wire
  (I.11; D-14: I.11.1 — a function of its reads; the combinational-cycle refusal of I.11.4 is
  the shape row owed here). A `RegOut` has one driver — the handle is not `Clone` (I.6.8, W6) — so the
  coroutine that received the port is the only writer.
I.2.5(a)'s family — `Option`, `?`, the panic family, `unwrap` — is **not** a shape rule: the
kernel can spell what they spell (a maybe-value is a `{ valid, payload }` pair of wires, an
error is a port), so the refusal is one spelling per meaning and belongs to II.2 (II.2.1,
D-12 A). **Message obligation** (`SPEC_OVERVIEW.md` §3): the message names the clause S0–S5
and, for S0, the production; today's messages say what is refused and why, and none names
its clause — an obligation on every message in Phase C, no rule changes. Prior work: Prost
refuses a loop without a `wait` [prost §5] — S2 — and a loop that may run zero iterations
[prost §5, §8.1], which D-10 A3-i rejected at a measured cost of 20 modules; combinational
loops are "not expressible" there by construction [prost §5]. Esterel rejects an
instantaneous loop body at compile time [esterel §3.4, §6.5.7] — S2 — and rejects cyclic
signal dependencies by a static graph that "sometimes rejects correct programs"
[esterel §7], the precedent for stating a predicate's direction (II.2.2). Spade's `reg` is
explicit, a use before its register is ready is a type error and no loop carries a `reg`
[spade §4, §9]: a shape stricter than S2, with no loop forms to rule on. SystemC's
synthesizable subset refuses `break` of the thread loop and admits `continue` and `wait()`
in inner loops [systemc §4.2.2, §10.5] — S4 and the admitted side of S2. CλaSH declares its
state in the type and has no sequencing, so no shape rule of this kind [clash §9]: not
covered. No surveyed language states the grammar as a refusal principle of its own
(S0); Esterel's and Prost's refusals are each one of S1–S4.

**II.1.2 The shape rules, enumerated.** One row per rule: the clause it cites; the
front-end that enforces it today (**M** = `copper-macros/src/lib.rs`, **T** =
`copper-codegen`, lines as of commit `ea54d26`); whether parity holds (II.1.3). Every rule
refuses **0** corpus modules (D-12 §0 (ii)) except where a count is given, and every verdict
in the parity column was measured on a probe (D-12 §0 (v)). Decided (D-12 A, sub-clauses
2a–2b): `check_admissible` is R16, a shape rule with a parity obligation; R5, R10 and R15 are
refused now, in both front-ends, at a measured cost of 0 each.
(Phase C step 11, 2026-11-07: the M and T cells are the `ea54d26` snapshot, kept as history; the
macro file is 359 lines since step 10 moved its rules to `copper_analysis::front_end`. Where the
rows' functions live at `924793e` — R1 `check_awaits` `copper-analysis/src/shape.rs:297` (M
`copper-macros/src/lib.rs:261`, T `copper-codegen/src/lib.rs:433`); R2 `Cfg::check_reachability`
`cfg.rs:752` via `copper-analysis/src/lib.rs:194` (M :242, T :426), the header's every-clock-ticked
clause `front_end::check_header` (:688); R3 `nested_loop_cfg` `cfg.rs:1358`, `may_exit_without_tick`
:2192 and its all-paths walk `untick_seq` :2235 (`ticks_before_any_break` is gone), called as R2;
R4 `check_nested_loop_shape` `shape.rs:37` (M :258, T :432); R11 `check_register_initializers`
`init.rs:157` via `copper-analysis/src/lib.rs:131`, the call half `check_register_initializer_calls_in`
:141 (T only, :458), `check_held_preloop_values` `init.rs:424` (M :287, T :453), and in the macro
rustc's `const { }` (M :281); the shared list `front_end::check_front_end_rules` (:1600; M :221, T
:419). Each row's state today is its ledger line in III.3.)

| # | Rule | Clause | M | T | Parity |
|---|---|---|---|---|---|
| R1 | an `await` that is not `<clock>.tick().await` | S1 | `check_awaits` (:980; `copper-analysis/src/shape.rs`) | `lib.rs:365` | holds — one function; ui `foreign_await` |
| R2 | a module-loop path that reaches no `tick`, including a tick-free body | S2 | `check_reachability` (:963, `cfg.rs:2405-2464`); `validate_hardware_fn` :1549 | `lib.rs:358` | holds — one function; ui `tickless_loop`, `partial_tick_path` |
| R3 | a nested loop with a zero-time cycle, or an enclosing loop whose only boundary is a nested loop that can be left before it ticks | S2 (I.8.2(a), I.8.4(a)) | `check_reachability` over `nested_loop_cfg` (`cfg.rs:2896-2920`), the fold (`:3062-3119`), `may_exit_without_tick` / `ticks_before_any_break` (`:3594-3640`) | `lib.rs:358` | holds; **over-refuses** and **under-admits** (II.2.2); ui `for_break_before_tick` |
| R4 | a tick-free nested `loop` or `while` | S3 (I.2.5(c)) | `check_nested_loop_shape` clauses 1–2 (:977; `shape.rs`) | `lib.rs:364` | holds — one function |
| R5 | a tick-free `for` whose bound is not a constant of I.14 (D-19: I.14.4) | S3 (I.2.5(c), I.8.5) | **none** | **none** — emits `always_comb … for (…; _ < n; …)`, a loop with a run-time bound | both admit; **both owe** (D-12 A 2b) (D-22: **holds** — `front_end::check_tick_free_for_bounds`, a shared syntactic half, rustc's `const { }` in the macro, `ConstEnv` in the transpiler; ui `runtime_for_bound`; corpus 0; Phase C step 10, `review/MEASUREMENTS.md` §P.1) |
| R6 | recursion through a helper `fn` | S3 (I.2.5(c)) | none (rustc admits non-`async` recursion) | the inliner (I.2.4), unmeasured | **unmeasured** → II-b (D-22: measured, M admits / T refuses — **the one named exception to II.1.3**, option (a): the macro holds the module's `ItemFn` only and cannot see a helper's body; the transpiler refuses when it inlines, `copper_analysis::front_end::RECURSIVE_HELPER_MESSAGE`, so no text is emitted for it, and a terminating recursion simulates as Rust runs it. Pins: `tests/recursive_helper_r6.rs` (simulated over all 256 inputs, refused with R6's message), ui/pass `recursive_helper_r6` with `front_end_parity_ui.rs`'s `NAMED_PARITY_EXCEPTIONS`, and `front_end_parity.rs::the_named_parity_exceptions_are_r6_and_the_let_instance` (renamed by D-24) — the tree's helper call graph, the witness the only module reaching a recursive helper; corpus 0) |
| R7 | `break` of the module loop; `return` | S4 (I.2.5(d), I.8.1) | **none** — admitted; the simulator panics at run time | `control_extract.rs:1786`, `chir_lower.rs:3603`; "return is not supported" (`:3892`) | **T-only** → M owes (D-22: **holds** — `front_end::check_module_loop_exit`, one message; ui `module_loop_break`, `module_return`; Phase C step 10, `review/MEASUREMENTS.md` §P.1) |
| R8 | a plain `Out` parameter on a `sequential` or `synchronizer` module | S5 (D-8) | none | none | both admit; D-8's Phase C rule; cost 88 (sweep) / 273 (all) — D-8's rewrites (Phase C step 11: **holds** since step 3 — `copper_analysis::check_no_plain_out` (`copper-analysis/src/lib.rs:94`), called by the macro and, through `check_clocked_outputs`, by the transpiler, one message naming R8; ui `plain_out_on_sequential`, `plain_out_on_synchronizer`; `review/MEASUREMENTS.md` §G.4) |
| R9 | the flag `allow_pretick_alignment` | S0 (I.2.2) | none — parsed and honoured | none | both admit; Phase C; 3 (sweep) / 78 (all), the battery's (Phase C step 11: **holds** since step 3 — the flag is refused by `front_end::parse_hardware_args` in both front-ends, one message; ui `retired_pretick_opt_out`; `front_end_parity.rs::no_corpus_module_carries_the_retired_opt_out`) |
| R10 | a second `Clock` parameter on a `sequential` module | S0 (I.2.2: exactly one) / S2 (a second resumption protocol with no meaning until I.12; D-17: I.12.5 confirms the refusal) | none — the CDC check is skipped (:947) | none — measured accepted | both admit; **both owe** (D-12 A 2b); cost 0 (D-22: **holds** — `front_end::check_one_clock`; ui `two_clocks_sequential`; corpus 0; Phase C step 10, `review/MEASUREMENTS.md` §P.1) |
| R11 | a pre-loop register initializer that is not a constant (registers only, D-21 C2); and (D-21 H1) a held pre-loop value — a pre-loop `let` the loop never reassigns, read in the loop, whose initializer reads an input | S3 (I.7.4(c), I.9.2; `s_pre ::= let v = c` for a register) | rustc: the initializer in a `const { }` block (D-21 A3); before it the shared `check_register_initializers` (`init.rs`): a run-time read, R11's message | the same function, then `check_register_initializer_calls_in` (a non-`const` callee) and the fold | holds — ui `register_init_reads_input` (R11's message), `register_init_not_constant`, `register_init_non_const_ctor` (E0015), `register_init_rust_constants_ok`; `copper-codegen/tests/register_init_parity.rs` transpiles those cases (D-21); the held value: `check_held_preloop_values` in both front-ends, ui `held_preloop_read`, `front_end_parity::no_corpus_module_holds_a_preloop_read` (corpus 0) |
| R12 | a labelled or valued `break` | S0 (I.2.3; D-10 A5, I.8.3) | **none** | `control_extract.rs:1792`, `:1797` | **T-only** → M owes (D-22: **holds** — `front_end::check_break_forms`; ui `labelled_break`, `valued_break`; Phase C step 10, `review/MEASUREMENTS.md` §P.1) |
| R13 | a macro in statement or expression position outside `const { }` (not `matches!`) | S0 (I.2.3: `e` pure, reads hoisted) | **none** | `parser.rs:275` → "macro invocation is not supported" | **T-only** → M owes; corpus 0 (one `vec!` inside a preload is DESUGARED, I.2.4) (D-22: **holds** — `front_end::check_no_body_macros` (`matches!`, `const { }` and a preload's `vec![…]` admitted); ui `body_macro`; Phase C step 10, `review/MEASUREMENTS.md` §P.1) |
| R14 | an input read inside macro tokens | S0 (I.2.3: `let t = read x` the only form; I.2.8, I.4.8) | **none** — the visitor does not enter tokens; `cfg.rs:2945` counts them as uses | refused with R13 | **T-only** → M owes (with R13) (D-22: **holds** with R13; ui `input_read_in_macro`; Phase C step 10, `review/MEASUREMENTS.md` §P.1) |
| R15 | `/` | S0 via I.14 (`e` over I.14's operators; `/` has no meaning on `Bits` until I.14 gives one; D-19: I.14.3(b) — given, so R15 is a dated II.3 limitation, not a shape) | **none** | `chir_lower.rs:4822` | **T-only** → M owes (D-12 A 2b); corpus 0 (D-22: **closed** — `/` has I.14.3(b)'s meaning and its emission (Phase C step 8); both admit; no rule) |
| R16 | a port payload or an annotated `let` whose type is outside I.14's set (D-19: I.14.1) | S0 (I.2.6) | `check_admissible` (:932; `copper-analysis/src/lib.rs:824-880`) | by lowering, another message ("cannot resolve type") | verdict parity on the probe; **rule parity owed**: T calls it, same message, a ui/fail case (D-12 A 2a) (D-22: **holds** — `check_admissible` run by both from `check_front_end_rules`, one message; Phase C step 10, `review/MEASUREMENTS.md` §P.1) |
| R17 | `Clock::new` / `Clock::default` in a body | S2 (I.2.5(b)) | `check_no_clock_construction` (:911, :295) | by accident ("cannot infer bit width") | verdict parity by accident → T owes the rule (D-22: **holds** — `front_end::check_no_clock_construction`, one message; ui `clock_construction`; Phase C step 10, `review/MEASUREMENTS.md` §P.1) |
| R18 | a `let` that shadows a port parameter | S0 (I.2.3's name classes) | `check_no_param_shadowing` (:937, :383) | **accepts** (measured) | **M-only** → T owes (D-22: **holds** — `front_end::check_no_param_shadowing`; ui `param_shadowing`; Phase C step 10, `review/MEASUREMENTS.md` §P.1) |
| R19 | an `In` handle used other than as the receiver of `.read()` (cloned, rebound, passed on) | S0 (I.2.3) | `find_unprotectable_in_uses` (:1365, via `inject_synced_reads`, which D-8 removes) | unmeasured | M; kept as a check of its own in Phase C; → II-b to measure T (D-22: **holds** — `front_end::check_in_param_uses`, its message no longer citing the retired injection; ui `sequential_unprotectable_read`; Phase C step 10, `review/MEASUREMENTS.md` §P.1) |
| R20 | the header: `async`, no return type, named parameters, port types only, at least one `Clock`, at least one output, every `Clock` ticked, one top-level `loop`, last and only | S0 (I.2.2) | `validate_hardware_fn` (:1403-1576) | `has_hardware_signature` (`lib.rs:596-610`), `chir_lower.rs:1978` (the loop) | partial; T owes the same messages (D-22: **holds** — `front_end::check_header` with R9's flag parsed by one `parse_hardware_args`, and the port-domain clause `check_port_domains` (I.12.6 (4)); every `function_typed_*` case refused by both with the macro's message; Phase C step 10, `review/MEASUREMENTS.md` §P.1) |
| R21 | a `tick` before the loop | S0 (I.2.2: `s_pre` tick-free; I.9.3) | none explicit (R2 sees it as a pre-loop statement) | unmeasured | corpus 0 → II-b to measure (Phase C step 11: **holds** since step 5 — `copper_analysis::check_no_preloop_tick` (`shape.rs:212`), both front-ends, one message; ui `preloop_tick`; `unsupported_constructs.rs::a_tick_before_the_module_loop_is_refused_by_r21_on_both_routes`; II.3 (f6) closed with it; `review/MEASUREMENTS.md` §J.12) |
| R22 | a second driver of a `RegOut` | S5 (I.6.8) | the type system (`RegOut` is not `Clone`) | the type system | holds by construction |

Rows that are **not** shape rules, placed for II-b: the hex literal in a `from_lit` turbofish
(`Bits::from_lit::<0xFF>()`, I.8.7) is a constant of I.14 (D-19: I.14.4) the transpiler's constant parser does
not read (`chir_lower.rs:3812`) — a transpiler defect, not a rule; a nested `continue` is IN
(D-10 A2-i) and its refusal (`control_extract.rs:1777`) is a II.3 row; a nested loop whose
ticks lie only in arms is a II.3 row (D-10, I.8.2(b); `check_nested_loop_shape` clause 4); "a
nested loop ends at its tick" (`check_nested_loop_shape` clause 3) is II.2's D1. The memory
rules — `check_memory_staging` (:999; `lib.rs:531`) and `memory_result_drives_plain_out`
(`lib.rs:537`) — are I.10's and are **deferred** with that section, not derived here (D-16: I.10.4 — (c) and (d) derived there, (a) and (b) retired); the CDC
rule (`check_cdc`, :947) and the unit-domain `In<T>` are I.12's (D-17: I.12.4(a)–(c) — `check_cdc` holds and is widened to the `combinational` mode; the unit domain OUTSIDE, I.12.1; the two-flop rule of I.12.3 is a new realizability row). (D-13: the hex literal is row (g), the nested `continue` row (a), the arms-only loop row (b); `memory_result_drives_plain_out` is retired as vacuous, II.3.2.)

**II.1.3 The parity obligation as a rule.** A module compiles under `#[hardware]` **iff** it
transpiles (`SPEC_OVERVIEW.md` §3; I.2.1: both front-ends hold the same `syn::ItemFn`), **with two
named exceptions**. (1) R6 (D-22): recursion through a helper `fn` compiles and does not transpile —
the macro cannot see a helper's body, the transpiler refuses the shape when it inlines, so the
break is a refusal-only asymmetry and never a divergence (no text is emitted; a terminating
recursion simulates as Rust runs it). (2) The bound instance (D-24 (1)): `let _ = child(..)` /
`let m = child(..)` in a behavioural body compiles — a `let` is a use, so I.13.5's denied
`#[must_use]` does not fire, and the macro cannot tell that the callee is a module — and the
transpiler refuses it with I.13.5's message; no text is emitted, and the simulator refuses it at run
time (the `HardwareModule` drop guard, I.13.5). Each exception is pinned so it cannot widen
(`front_end_parity.rs::the_named_parity_exceptions_are_r6_and_the_let_instance`, which scans the
tree for each and requires exactly its witnesses, and one ui/pass row each in `front_end_parity_ui.rs`);
any other shape one front-end admits and the other refuses is a break of this rule.
Every rule of II.1 and II.2 is **one function** of `copper-analysis`, called by both
front-ends, with **one message**; a shape refused by both is a `copper-macros/tests/ui/fail`
case and never a swept module. The audit is `copper-analysis/tests/front_end_parity.rs`
(measured PASS, 2 tests, 2026-11-05): test 1 runs `check_nested_loop_shape`, `check_awaits`,
`check_register_initializers`, `check_memory_staging` and `memory_result_drives_plain_out`
over every clocked corpus module and expects no refusal; test 2 asserts
`multi_write_collapse`'s corpus set is empty and that the seven alignment rules fire only on
opted-out modules. It does **not** run R2, R16, `check_definite_assignment`, the header
rules, or any syntactic shape of I.2.4, and it has no transpiler-admits direction. **Open
breaks** (every one measured on a probe, D-12 §0 (v)): *transpiler-only refusals* — R7, R12,
R13/R14, R15, the nested `continue` (II.3) and the hex literal (defect); *macro-only
refusals* — R18, and R16, R17 and the `Option` family (II.2 D3) by the accident of another
message; *both admit, rule owed* — R5, R8, R9, R10. The Phase C fix per row: the macro gains
R5, R7, R8, R9, R10, R12, R13/R14, R15; the transpiler gains R16, R17, R18 and the idiom
rule as the shared functions, and parses hex const generics; the nested `continue` gets its
lowering (D-10); `front_end_parity.rs` gains R2, R16, `check_definite_assignment` and the
header rules, and a battery of I.2.4's OUTSIDE rows in the transpiler-admits direction
(the probe set of D-12 §0 (v) is its seed). D-10's and D-11's parity notes (I.2.8, I.8.3,
I.8.7) are closed into this list.
(D-22: **the state after Phase C step 10** (`review/MEASUREMENTS.md` §P.1, §Q). The syntactic rules
are one function, `copper_analysis::front_end::check_front_end_rules(f, mode)`, which both front-ends
run first, in one order: R20 (with R9's flag and the port-domain clause of I.12.6 (4)), R10, R17,
I.14.3's OUTSIDE methods, the memory port index, D3, R16, R18 — then, for a `structural` body, the
rules of I.13.4(d) and nothing else; for any other, R7, R12, R5, R13/R14, the mode's CDC rule (the
port rule, the two-flop rule, the combinational one-domain rule) and, on a clocked body, R19. The open breaks above are closed — R5, R7, R10, R12,
R13/R14 hold in both; R16, R17, R18, D3 are shared; R15 is closed (I.14.3(b)); the `cdc_*` rule,
macro-only and a break the list never named (the transpiler admitted `cdc_crossing` and
`cdc_memory_parameter`), is shared — except R6, the named exception. **The audit** is two tests.
`copper-analysis/tests/front_end_parity.rs` runs every shared rule, R2/R3 (`check_reachability`) on a
clocked body and D2 on a combinational one over every `#[hardware]` fn of the tree in every mode
(272 fns), plus the step-8 data rules, R11's halves, the held pre-loop value and R6's call-graph
scan; `copper-codegen/tests/front_end_parity_ui.rs` is the transpiler-admits direction: every
`ui/fail` case refused by the transpiler with the macro's first line (three rustc-judged rows
named — E0015 ×2, E0308), every `ui/pass` case admitted (`register_init_constants_ok` a named
lowering-limit row; `recursive_helper_r6` the R6 row, refused with R6's message). P10's blind
spots are closed.)
(D-23, 2026-11-08, Phase C step 11b, `review/MEASUREMENTS.md` §S: the three REFUSED-one shapes of
§R.6 are closed — a `#[hardware]` call in a behavioural body (I.13.5; rustc's `unused_must_use`,
denied, its note the message — `front_end_parity_ui.rs`'s one lint-judged row, whose transpiler
message must equal the note), a `continue` of a tick-bearing `for` (II.3.3 (h)) and `==`/`!=` with an
X literal (I.14.3(d)), the last two shared rules of `check_front_end_rules`. **Re-verified by probe:**
R6 is still the only *named* exception, and no shape is admitted by the transpiler and refused by
the macro; but "any other shape is held by both" is not yet true — besides the REFUSED-one lines
III.3.4 already carried (array-for, const-call, `saturating_*` on a signed or wide operand under row
(e)), four macro-admits / transpiler-refuses shapes were found: a bound instance (`let _ = child(..)`),
a `wire`/`registered_wire` call in a behavioural body, a `Logic::X` pattern and a `continue` of a
tick-free `for` (the transpiler's message blames the module loop). In none is text emitted; each is
an OPEN line of III.3.4 (`II.3 found let-instance`, `body-wire`, `x-pattern`, `tickfree-continue`).)
(D-24, 2026-11-08, Phase C step 11c, `review/MEASUREMENTS.md` §T: `let-instance` is closed as the second
named exception, with the simulator's drop guard; `x-pattern` is closed as OUTSIDE (I.14.3(d)–(e)),
a shared rule of `check_outside_methods`. **Re-verified by probe:** one further macro-admits /
transpiler-refuses shape — a pattern naming a non-integer `const` (`const ONE: Logic = Logic::One;
… match v { ONE => … }`, file-level or in the body), which the transpiler reads as an enum variant
and refuses with a tuple-pattern message; no text emitted; an OPEN line, `II.3 found const-pattern`.)

**II.1.4 Implementation map.** (D-22: the line numbers below predate Phase C step 10, which moved the
macro's syntactic rules into `copper-analysis/src/front_end.rs`; II.1.3's D-22 note names the functions.) Rule home: `copper-analysis/src/lib.rs` (`check_awaits`,
`check_nested_loop_shape` from `shape.rs`; `check_reachability` :219, `check_register_initializers`
:51, `check_admissible` :824, `check_definite_assignment` :274, `multi_write_collapse` :109
— retired, II.2.1; the alignment rules :70-205 — retired, D-8). Macro
(`copper-macros/src/lib.rs::hardware`, :897-1400): `validate_hardware_fn` :906 → R20/R2's
tick clause; `check_no_clock_construction` :911 → R17; `check_no_unsupported_idioms` :917 →
II.2 D3; `check_admissible` :932 → R16; `check_no_param_shadowing` :937 → R18; `check_cdc`
:947 → I.12 (D-17: I.12.6); one `Cfg` per module :963; `check_reachability` :963 → R2/R3;
`check_nested_loop_shape` :977 → R4, II.2 D1, II.3; `check_awaits` :980 → R1;
`check_register_initializers_with` :990 → R11; `check_memory_staging` :999 → I.10 (D-16: I.10.4); the
alignment rules :1065-1322 (D-8, retired); `inject_synced_reads` :1365 → R19 (and the
injection D-8 removes); `check_definite_assignment` :1378 → II.2 D2 (combinational arm).
Transpiler (`copper-codegen/src/lib.rs::vlir_for_target` :352-560): `check_reachability` :358,
`check_nested_loop_shape` :364, `check_awaits` :365, `check_definite_assignment` :371,
`check_register_initializers` :377, the alignment rules :385-460 and `multi_write_collapse`
:462 (retired), `check_memory_staging` :531, `memory_result_drives_plain_out` :537; the
lowering refusals that stand in for shape rules today: `control_extract::unflattenable_in_expr`
(`control_extract.rs:1768-1820`: nested `continue` :1777, module-loop `break` :1786,
labelled :1792, valued :1797, a nested loop not ending at a boundary), `chir_lower.rs:3603`
(`break`/`continue` on the linear route), `:3812` (a call the registry does not know),
`:3892` (`return` and other unsupported expression kinds), `:4822` (`/`), `parser.rs:275`
(a statement macro), `has_hardware_signature` (`lib.rs:596-610`). Pins:
`copper-macros/tests/ui/fail` — `foreign_await`, `tickless_loop`, `partial_tick_path`,
`for_break_before_tick`, `tick_first_wait` (II.2 D1), `register_init_not_constant`,
`clock_construction`, `param_shadowing`, `multiple_top_level_loops`, `unsupported_idiom_panic`,
`sequential_unprotectable_read`, `function_typed_*`, `cdc_*` (I.12; D-17: I.12.6 — kept, joined by the one-flop and mixed-domain cases in Phase C), `memory_result_drives_plain_out`
(I.10; D-16: goes with rule 7 in Phase C, I.10.7), the `combinational_latch*` trio (II.2 D2), `multi_write_collapse` and the
alignment family (retired with D-8); `ui/pass` — `register_init_constants_ok`,
`single_loop_local_ok`, `multi_write_regout_ok`, `combinational_mux_valid`;
`copper-codegen/tests/unsupported_constructs.rs` (the transpiler's own refusals: division,
the wait shapes, `continue`, the counted `for`); `admissible_calibration.rs` (R16's one
direction); `front_end_parity.rs` (II.1.3); the `cfg.rs` unit tests named in I.8.6. Obligations
carried to Phase C (D-12): the rules the table owes to each front-end (II.1.3), each as a
shared function with one message and a ui/fail case; every message naming its clause; R11
widened to every pre-loop `let` (D-9); R3's two directions fixed — `ticks_before_any_break`
walking every arm of an `if`/`match` (all-paths) and a literal range evaluated for emptiness
(I.8.7) — with `livelock_a`/`livelock_b` (D-12 §0 (iii)) as `ui/pass` cases and the
literal-empty `for` as a `ui/fail` case; `front_end_parity.rs` extended as II.1.3 says;
`check_admissible` run by the transpiler with its ui/fail case.

**II.1.5 Carried list.**
- II-b: the sixteen rules of `review/INVENTORY_RULES.md` §(1) sorted into II.1 / II.2 / II.3
  / retired, with this section's placements as given: rules 2, 3 (clauses 1–2), 4, 5 → II.1
  (R2–R4, R11); rule 1 → II.1 (R16); rule 3 clause 3 → II.2 D1, clause 4 → II.3; rule 16 →
  II.2 D2; rule 8 → retired (II.2.1); rules 9–15 → retired (D-8) or re-sorted by II-b where a
  residue survives for the combinational module; rules 6–7 → I.10 (D-16: I.10.4, I.10.6). II.3 written there, with
  the nested `continue`, the ticks-only-in-arms loop, the run-time-empty counted `for`'s
  Phase C defect (I.8.4(b)) and whatever Phase C finds. (D-13: done.)
- R6 (recursion), R19's transpiler side and R21 — unmeasured, to II-b's probe battery. (D-13: measured, II.3.4.)
- The memory rules → I.10 (D-16: I.10.4); the CDC rule and the unit-domain port → I.12 (D-17: done — I.12.3, I.12.4, I.12.1); `/`'s meaning, the
  constant set behind R11 and the type set behind R16 → I.14 (D-19: I.14.3(b), I.14.4, I.14.1 — closed); the combinational module's
  definite assignment and dead write → I.11. (D-14: I.11.3 — D2 stays in II.2, the never-written
  port refused; I.11.2(c) — the dead write admitted.)
- The over-refusal of R3 and its literal-empty under-admission → Phase C (II.1.4); until then
  a nested loop with a tick-carrying `if` before its bare tick as the only boundary of its
  enclosing loop is refused although well-formed.
- The parity fixes of II.1.3 → Phase C; the hex-literal parser defect → Phase C.
- `front_end_parity.rs`'s missing directions → Phase C (II.1.3).

### II.2 Rules derived from Part I

Decided: D-12 (2026-11-05). Anchor: `bsg_dff_en` for the hold a `RegOut` has (D-6; the
spelling the dead-write question would have refused keeps its anchor); `dbl_write` and
`straddle` (D-12 §0 (iv): a `RegOut` written twice in one resumption, and rule 8's shape on a
`RegOut`, sim = hand model = SV on 24 cycles each) are sim ≡ SV only. Decided: what a
derived rule is, which survive D-8, the retirement of `multi_write_collapse`, the routing of
the dead write, the direction of every static predicate.

In coroutine terms (D-7, D-8): a derived rule refuses a coroutine that *is* a hardware
coroutine, for a reason Part I states — an effect no trace can show, or a second spelling of
a meaning Part I already gives one spelling. It never cites an emitted form; a refusal that
does is a lowering limitation of II.3, dated, with a fix direction.

**II.2.1 What a derived rule is.** A derived rule is a refusal that cites the design
principle (I.0.6: every effect is observable in some trace; one spelling per meaning) or a
clause of Part I directly, as opposed to a lowering limitation (II.3). Under D-8 the former
derived rules about a plain `Out` — I.5.5's dead write, I.4.5's passthrough, and the eight
alignment rules that refused instances of them for a lowering reason — are **vacuous for
sequential modules**: a sequential module has no wire to write (I.0.1), every `RegOut`
write's effect is its pending value at the edge, and that value is always shown (I.6.1,
I.6.4). The rules that survive:
- **D1 — a nested loop ends at its tick** (`check_nested_loop_shape` clause 3,
  `shape.rs::ends_at_clock_boundary`): one spelling per meaning — `loop { tick; C }` and
  `tick; loop { C; tick }` are the same coroutine by rotation, and the tick-last form is the
  spelling (D-10 A1-i, I.8.2(b)). Both front-ends; ui `tick_first_wait`; corpus 0.
- **D2 — combinational definite assignment** (`check_definite_assignment`,
  `cfg.rs:2514`; `combinational` modules only, `lib.rs:274` returns `Ok` on a clocked body):
  every path writes every output (D-22: the mode is read from the attribute when one is present —
a `structural` body has no loop and is not combinational, Phase C step 10, §P.5). The principle, stated now and owed to I.11 as its clause (D-14: I.11.3, written; D2 stays here): a
  combinational module is evaluated afresh at every instant and has no cycle to hold across
  (I.2.7, I.5.7), so an output unwritten on some path would be *state*, and state's spelling
  is the `RegOut` of a sequential module (one spelling per meaning). Both front-ends since
  2026-11-02 (`copper-codegen/tests/comb_definite_assignment_parity.rs`); ui
  `combinational_latch*`; corpus 0. The default-then-override shape is admitted (the
  transpiler's `check_no_latches` stays as defence in depth).
- **D3 — `Option`, `?`, the panic family, `unwrap`** (`check_no_unsupported_idioms`, macro
  :917, :506): one spelling per meaning — a maybe-value is a `{ valid: Logic, payload }`
  pair of wires and an error is a port, which is the message the rule prints; I.2.5(a)'s
  "no cycle to fail in" is the reason there is nothing else they could mean. Macro only; the
  transpiler refuses the same probes by other arms (`cannot resolve type`, the macro
  refusal) — rule parity owed (II.1.3); corpus 0. (D-22: shared since Phase C step 10 —
  `front_end::check_no_unsupported_idioms`, both front-ends, one message.)
- **D4** — retired. `multi_write_collapse` (rule 8; `cfg.rs:439-480`): its predicate names a
  bare tick with the same **plain `Out`** written on both sides after a leading read, over
  `comb_outputs` only (`RegOut` excluded by construction), so under D-8 it is identically
  empty; D-8 lists it eighth among the retired rules and this clause confirms it **retired
  as vacuous** (D-12 A, sub-clause 4). Measured: its corpus set is empty (the parity test
  asserts it); the collapse shape spelled on a `RegOut` (`straddle`) and a `RegOut` written
  twice in one resumption (`dbl_write`) agree with the hand model and with Verilator, the
  last write before the edge winning (D-6 A1, I.6.2). A `RegOut` write that a later write of
  the same port overrides before the edge is **not** refused: the effect of a `RegOut` write
  is the pending value the edge commits, and that value is observed (I.6.4); the alternative,
  a design rule refusing a write dominated by a later unconditional one, was costed at 0
  corpus modules (syntactic, D-12 §0 (iv)) and not taken — it would refuse a program whose
  meaning is defined and realized, a lint rather than a semantic refusal. The **dead write of
  I.5.5** (D-4/D-5) is vacuous for sequential modules (D-8) and its combinational case — a
  wire written twice in one evaluation, the first write in no observation — is **I.11's** to
  decide with the wire's last-write-wins (→ I.11; D-14: admitted, I.11.2(c)). `ui/fail/multi_write_collapse.rs` and
  the parity test's assertion go with the rule in Phase C; `ui/pass/multi_write_regout_ok.rs`
  stays as D-6 A1's pin.
Prior work: Prost's `.next` last-write-wins is implicit in sequential execution into a
next-value variable [prost §5]; Esterel combines several emissions of one signal in an
instant by its declared `comb` and refuses a signal without one that collides, while a
variable "x := 1; x := x+1" simply ends at 2 [esterel §3.4, §5] — the variable, not the
signal, is the `RegOut`'s analogue; Spade refuses a `&mut` wire set twice and makes a
conditional set a latch [spade §9] — the type-rule version of D2 and of the refusal not
taken in D4. No surveyed language refuses a dominated write of a register; Esterel's
compile-time refusal of an instantaneous loop and Spade's use-before-ready are the
precedents for D1's kind (a spelling rule on the source).

**II.2.2 The direction of every static predicate.** Each static analysis a surviving rule or
the realization consumes approximates a dynamic property of the coroutine; its direction is
stated here, with its witnesses, so that a refusal can be read for what it is. *Over* = may
refuse a well-formed coroutine; *under* = may admit an ill-formed one; *exact* = neither.

| Predicate | Approximates | Direction | Witnesses |
|---|---|---|---|
| `check_awaits` (`shape.rs`) | S1 | exact (syntactic) | — |
| `check_reachability` on the module CFG (`cfg.rs:2405-2464`) | S2: every execution's loop path reaches a `tick` | exact on a body without nested loops (a `Comb` cycle from the head is a tickless path and conversely) | ui `tickless_loop`, `partial_tick_path` |
| `may_exit_without_tick` / `ticks_before_any_break` (`cfg.rs:3594-3640`), the fold's zero-time edge (`:2432`) | "the nested loop can be left with no `tick` behind it" (I.8.4(a)) | **both ways**. *Over*: the walk returns "exitable" at the first statement that contains a tick without being one (`:3634-3636`), whatever its arms do — `livelock_a` (an `if` with a tick, no `break`, the bare tick last) and `livelock_b` (an `if`/`else` whose both arms tick, then the `break`, then the tick) are refused by both front-ends though every path ticks (measured 2026-11-05; a `continue`-carrying wait of the same shape too). *Under*: a `for` over a literal-empty range is counted as a boundary (`:3594-3598`; RULES open 5, I.8.4(b)), so an enclosing loop whose only tick it holds passes and livelocks the simulator. | D-12 §0 (iii); Phase C: an all-paths walk of each arm, literal ranges evaluated (D-22: **narrowed both ways** in Phase C step 10 — an all-paths walk (`cfg.rs::untick_seq`): a `loop` is left untick'd only through a `break` reached before any tick, a `for` also through an untick'd fall-through or a literal-empty range. Still **over** by path-insensitivity (complementary `if`s) and **under** at a range empty only through a constant (`0..EMPTY`). Witnesses: ui/pass `livelock_a`, `livelock_b`; ui/fail `literal_empty_for_boundary`; the swept fixture `all_paths_boundary_dut.rs`; `static_predicate_directions.rs`; §P.5) |
| `check_nested_loop_shape` clauses 1–2 (`shape.rs`) | S3: a tick-free nested `loop`/`while` | exact (syntactic) | — |
| `ends_at_clock_boundary` (clause 3) | D-10's spelling (I.8.2(b)) | exact — it is the definition of the spelling | — |
| `infer_registers` (`cfg.rs:394-415`) | I.7.1: live at a `tick` | **over** by the head clause (8 wires in 7 modules, I.7.5) and by the fold's added local; **under** by the fold's missed local (a local assigned before a ticking `for` and read inside it after its tick); the poison run that decides I.0.5(a) → III.1 (D-20: P2, P3) (Phase C step 11: since step 6 the head clause is deleted and the nested loop's CFG descended, so both fold defects are gone — **over** by path-insensitivity only, never under; `register_reconciliation.rs`, the poison run, `review/MEASUREMENTS.md` §K.1, §K.3, §P.5) | I.7.5, I.7.7 |
| R11: rustc's `const { }` in the macro; `check_register_initializers` + `check_register_initializer_calls_in` and the fold in the transpiler (`init.rs`, D-21 A3) | I.14's constants (R11; D-19: I.14.4) | **exact** in the macro (rustc); in the transpiler **over** only where its fold's grammar is narrower than rustc's — a `const` method call (`K.wrapping_add(1)`), a tuple-struct ctor, a `static` — each reported as a lowering error, never a dropped value; corpus 0 each | `init.rs` header; `register_init_parity.rs`; `register_initializer_corpus.rs` |
| `check_admissible` (`lib.rs:824-880`) | I.14's type set (R16; D-19: I.14.1) | **under** by design: unannotated `let`s and array lengths are not checked; calibrated never to over-reject (`admissible_calibration.rs`: admissible Err ⟹ transpile Err) | `tuple_local` (D-12 §0 (v)) |
| `check_definite_assignment` (`cfg.rs:2514`) | D2: every path writes every output | **under**: a folded loop is taken to write every port; a port never written is not flagged (`TODO:1366`) | `comb_definite_assignment_parity.rs` (D-22: **narrowed** in Phase C step 10 — the combinational CFG expands its loops, and the never-written port is refused (D-14); still **over** by path-insensitivity. Witnesses: ui/fail `combinational_latch_in_for`, `combinational_never_written`; `static_predicate_directions.rs`; the cfg tests `comb_output_never_written_is_refused`, `comb_write_under_an_if_in_a_for_is_partial`; §P.5) |
| `check_no_param_shadowing`, `check_no_clock_construction`, the idiom visitor, `find_unprotectable_in_uses` | S0 productions, R17, D3, R19 | exact (syntactic) on what they name; R19's visitor does not enter macro tokens (R13/R14 close that) | — |
| `has_top_level_loop` / `check_single_top_level_loop` / `validate_hardware_fn` | S0 (I.2.2), R20 | exact (syntactic) | ui `multiple_top_level_loops`, `function_typed_*` |
| `front_end::check_two_flop` (D-22: new, Phase C step 10) | I.12.3's two-flop rule | **over** by path-insensitivity (arms joined by union) | ui `sync_one_flop`, `sync_one_flop_guard`; `static_predicate_directions.rs` |
| `HardwareExecutor::refuse_comb_cycle`, `chir_lower::refuse_structural_comb_cycle` (D-22: new, step 10) | I.11.4 / I.13.2: a combinational cycle | **exact** on the graph the testbench declares (`reads`, `dirties`) and on a parent's nets; **under** for a cycle through a read the testbench does not declare (the fixpoint oracle's oscillation guard stays for it) | `executor.rs::fixpoint_oscillating_loop_trips_threshold`; `structural_hierarchy.rs::combinational_cycle_through_a_structural_parent_is_refused` |
| `front_end::check_tick_free_for_bounds` (R5; D-22: new, step 10) | I.14.4's constants as a tick-free `for` bound | **exact** in the macro (rustc's `const { }`); in the transpiler **over** where `ConstEnv` is narrower than rustc (as R11) | corpus 24/24 bounds fold; ui `runtime_for_bound` |
| `front_end::check_structural_body` (D-22: new, step 10) | I.13.4(d)'s S0 rows | **exact** (syntactic) | the seven `structural_*` ui/fail cases, ui/pass `structural_net_ok` |

Retired with D-8 and named once, not derived: `classify_reads` (the barrier placement),
`leading_read_reaches`, `deferred_read_reaches`, `seen_in_read`, `passthrough_read`,
`pre_tick_region`, `read_free_path_reaches`, `written_on_all_paths` as a rule input and
`comb_phases` as a rule input — each approximated the park, and there is none (I.0.6,
I.4.6). `clock_phase_count` and `derivation_facts` refuse nothing and stay as reporting.

**II.2.3 Implementation map.** D1: `copper-analysis/src/shape.rs::check_nested_loop_shape`
(macro :977, transpiler `lib.rs:364`; `control_extract::body_ends_at_a_clock_boundary` and
`chir_lower::nested_loop_error` as defence in depth). D2: `Cfg::check_definite_assignment`
(`cfg.rs:2514`; `Cfg::build_combinational`), macro :1378 (combinational arm), transpiler
`lib.rs:371` (a no-op on a clocked body) plus `vlir_lower::check_no_latches`. D3:
`copper-macros/src/lib.rs::check_no_unsupported_idioms` (:506, called :917). D4 retired:
`Cfg::multi_write_collapse` (`cfg.rs:439-480`), `lib.rs:109`, macro :1065-1322 (the
collapse arm), transpiler `lib.rs:462`, ui `multi_write_collapse`, the parity test's
`collapsing` assertion. Obligations carried to Phase C (D-12): D3 as a shared function run by
both front-ends with one message (II.1.3); D4's code, ui case and assertion removed with
D-8's eight; D2's clause written into I.11 with the dead-write decision for the wire (D-14: done, I.11.2(c), I.11.3); every
direction in II.2.2 that is *under* or *over* either narrowed in Phase C (R3's two
witnesses) or pinned as a direction by a test (R16's calibration already is; D2's folded
loop and unwritten port are not).

**II.2.4 Carried list.**
- II-b: II.3 written, with the sort of the sixteen rules (II.1.5); the D-8 retirement of
  rules 8–15 recorded there with their replacement forms or their vacuity. (D-13: II.3.2.)
- I.11: D2 as a clause of the combinational module; the wire's dead write (I.5.5's
  combinational case) and last-write-wins; whether a `combinational` module may write a
  port twice in one evaluation. (D-14: I.11.3 and I.11.2(b)–(c); it may, the last write wins.)
- I.10: whether a memory staging dominated by a later staging of the same port in one
  resumption is a design refusal (today `check_memory_staging` (b) refuses two stagings
  that reach each other tick-free) — I.10 decides with its own principle. (D-16: I.10.4 —
  admitted, the last staging wins, as I.6.2 for a `RegOut`; (a) and (b) retired in Phase C.)
- I.14: the constant set R11 approximates; `/`. (D-19: I.14.4 — Rust's constants, one shared evaluator; I.14.3(b) — `/` given a meaning.)
- III.1 (D-20: P2, P3): the poison run that decides `infer_registers`' direction (I.0.5(a)); the
  two fold defects.
- Phase C: the obligations of II.1.4 and II.2.3.

### II.3 Lowering limitations carried with a dated obligation

Decided: D-13 (2026-11-05). Anchor: none of its own — a row records what the emitted text
does not realize, and every verdict below is sim ≡ SV (the probes of D-13 §0, each
predicted before it was measured) or a refusal read off the transpiler; no row is anchored
to independent hardware, because a row names no behaviour of its own. Decided: what a row
is and what it carries, the sort of the sixteen rules of `review/INVENTORY_RULES.md` §(1)
into II.1 / II.2 / II.3 / retired, the retirement of `memory_result_drives_plain_out` as
vacuous, the rows known today with their status, the placement of the three rows II.1.2
left unmeasured (R6, R19, R21), and that the inventory keeps its 2026-11-03 column with a
pointer here.

In coroutine terms (D-7, D-8): a lowering limitation is a shape the coroutine gives a
meaning — Part I says what its resumption does — and the emitted machine does not realize
today. The coroutine is not at fault and nothing about it changes; the row is a debt of the
realization, dated, with the form that would pay it.

**II.3.1 What a lowering limitation is.** A row of this section is a refusal, or a recorded
divergence, that cites the realizability principle (I.0.6; `SPEC_OVERVIEW.md` §5): the
emitted text cannot realize the resumption's effects for this shape today — its reads of
`x_{N+1}`, its register updates and its pending values, shown from `obs_{N+1}`. Every row
carries **the shape**, **the Part I clause** that gives it its meaning, **the date** it was
recorded, **the transpiler site** (file:line, as of commit `ea54d26`), **the fix direction**
(the form that would realize it) and **its status**, one of three: **REFUSED-both** — a
dated limitation in `SPEC_OVERVIEW.md` §6's sense, refused by both front-ends with one
message; **REFUSED-one** — refused by one front-end only, a parity break of II.1.3, the
other front-end owing the refusal until the form lands; **ADMITTED-AND-DIVERGENT** — a
defect: admitted by both, meaning defined, emitted text wrong, so the module is outside the
agreement relation (III.2; D-20: III.2.8) until the fix lands, and a sweep fixture of the shape is owed
with the row (III.3). Two rules of the section (D-13 A1): **a row without a date or without
a fix direction is deleted** — it is then either a refusal citing no principle (forbidden
by I.0.6: a rule that cannot name its principle is deleted) or a meaning in disguise; and
**a row's existence never changes a meaning** (`SPEC_OVERVIEW.md` §6: the limitation list
is not the language). A row is closed when its form lands and its fixture passes, and the
closing is recorded in III.3's ledger, not by deleting history here. D-10's and D-11's
"II.3 — nothing: both defects are Phase C fixes, not dated limitations" is read as "no
refusal", which stays true: a defect row refuses nothing. `SPEC_OVERVIEW.md` §6's "none
today" predates this section and names the alignment rules only; its restatement — six
dated refusal rows and six defect rows, every one a transpiler form, none a meaning — is the
coordinator's edit of the generating page (D-13 obligations).

**II.3.2 The sort of the sixteen rules.** One row per rule of `review/INVENTORY_RULES.md`
§(1), from II.1.5's placements (D-12); every placement that II.1.5 left to this packet is
decided here (D-13 A2).

| # | Rule | Placement | Note |
|---|---|---|---|
| 1 | `check_admissible` | II.1 R16 | the transpiler owes the call (II.1.3) |
| 2 | `check_reachability` | II.1 R2 / R3 | R3 over-refuses (`livelock_a`/`livelock_b`) and under-admits the literal-empty `for` — Phase C (II.1.4, II.2.2) |
| 3 | `check_nested_loop_shape` | clauses 1–2 → II.1 R4; clause 3 → II.2 D1; **clause 4 → II.3 row (b)** | clause 4 measured in D-13 §0 (ii) |
| 4 | `check_awaits` | II.1 R1 | |
| 5 | `check_register_initializers` | II.1 R11 | row (d) closed (D-21: rustc judges in the macro; the transpiler's fold covers I.14.4) |
| 6 | `check_memory_staging` | I.10 (deferred) | its principle is I.10's to state (D-16: I.10.4 — (c) design, (d) one spelling, both II.2; (a) and (b) retired as lints) |
| 7 | `memory_result_drives_plain_out` | **retired — vacuous** (D-13 A2) | its predicate requires ≥ 2 comb phases **and** a plain-`Out` write that observes a memory result (`cfg.rs`, `copper-analysis/src/lib.rs:537` in the transpiler): a sequential module has no plain `Out` (D-8, S5), and a `combinational` module has no phase and cannot observe a memory — I.2.7 excludes staging from the tick-free fragment and `check_memory_staging` (d) refuses an observe every staging reaches tick-free. Nothing survives for any mode; I.10 may re-derive a rule of its own if a memory ever reaches a `combinational` module (D-16: I.10.6 — none does; OUTSIDE). Its ui/fail case, its `cfg.rs` tests and the `vlir_lower::reject_memory_driven_comb_outputs` RULE entry of `phase_sensitive_checks.rs` go with it in Phase C |
| 8 | `multi_write_collapse` | retired (II.2.1 D4) | |
| 9 | `unprotected_pretick_out_write` | retired (D-8) | refused a pre-tick plain-`Out` port with mixed anchors or a forwarded guard; the shape no longer exists — no plain `Out` in a clocked body, one resumption, no anchor to mix; no residue (the predicate ranges over `comb_outputs`) |
| 10 | `pretick_out_write_before_update` | retired (D-8) | refused a plain-`Out` write before the update of a register it read, in three editions; every write is a `RegOut` pending value shown from `obs_{N+1}` (I.6.4); no residue; `e18j`'s over-refusal closes with it |
| 11 | `trailing_regout_reads_input` | retired (D-8) | refused a trailing `RegOut` write whose value reaches an *immediate* input read; under D-8 the trailing region is the next resumption's head and the read sees `x_{N+1}`, which is what `q <= x` samples — **vacuous by its predicate** (D-13 §0 (iii)): it fires only at a read site with no tick comb-reachable from it (`cfg.rs:1229`), the two-job classification, and no site is such under one job; the §10.13 guard clause on `claude/trailing-regout-guard-67cf37` (`e19a`–`e19j`) keys on the same site and is vacuous with it; I.6.7's opening-instant case closes |
| 12 | `trailing_out_reads_input` | retired (D-8) | the plain-`Out` sibling (`e8`); no plain `Out`; no residue |
| 13 | `linear_out_write_anchor_mismatch` | retired (D-8) | refused linear-route plain-`Out` drives by operand kind and shape (§10.7–§10.9); the five forms F1–F5 go with the port; no residue |
| 14 | `multi_phase_out_write` | retired (D-8) | refused a plain `Out` driven in two phases whose anchors differ between the sides; no second instant, no anchor; no residue |
| 15 | `out_first_driven_late` | retired (D-8) | refused a constant plain `Out` first written after the second edge (a startup transient of the assign); a sequential module has no wire and a `combinational` module no edge; its class is I.11's (I.9.4; D-14: vacuous for the wire, I.11.2(e)); no residue |
| 16 | `check_definite_assignment` | II.2 D2 (→ I.11) | D-14: I.11.3 is the clause it cites; the never-written port joins it |

**No combinational residue** survives from rules 9–15 (D-13 A2, read off every predicate):
each is keyed on a tick, a phase, the trailing region or the register set
(`crosses_more_than_one_tick`, `comb_phases`, `tick_free_reaches(n, head)`, `registers()`),
and a `combinational` module has none of these (I.2.7); the one combinational rule is D2.
**Retired with the rules — the Phase C deletion, sized** (D-13 §0 (v)): `copper-macros/tests/ui/fail`
**18 of 39** cases (rule 8 `multi_write_collapse`; rule 9 `pretick_alignment`,
`pretick_forwarded_guard`, `pretick_mixed_anchors`, `pretick_path_dependent_drive`; rule 10
`write_before_update`, `write_before_update_via_temp`, `write_before_update_via_bool_temp`,
`pretick_guard_update`, `single_tick_trailing_update`; rule 11 `trailing_regout_reads_input`;
rule 12 `trailing_out_reads_input`; rule 13 `linear_anchor_mismatch`,
`linear_regfree_passthrough`, `linear_trailing_conditional`; rule 14 `multi_phase_out_write`,
`path_dependent_single_phase`; rule 15 `out_first_driven_late`) and `ui/pass/pretick_alignment_ok`
(`multi_write_regout_ok` stays, D-6 A1); rule 7's `memory_result_drives_plain_out` makes 19;
the exact-set pins `copper-analysis/tests/pretick_alignment_corpus.rs` (seven arrays, **83
entries**: FLAGGED 10, MULTI_PHASE 8, LATE_CONSTANT 4, TRAILING_REGOUT 0, TRAILING_OUT 11,
LINEAR_ANCHOR 36, WRITE_BEFORE_UPDATE 14 — the whole file) and
`copper-codegen/tests/hold_form_audit.rs` (**20 entries** — the whole file);
`front_end_parity.rs` test 2 (the `collapsing` and opt-out assertions); the battery
`tests/sequential_forwarding_divergence.rs` (167 tests, 148 opt-out modules; its **62**
refused-divergent modules become unexpressible, `SPEC_OVERVIEW.md` §2, and its `d*`/`e*`
verdict tables go, the agreeing `RegOut` rows kept where they compile — D-8); the `cfg.rs`
unit tests of the retired rules (**about 109 of 165**, by name prefix: `hazard_` 11, `d*`
16, `v8*` 18, `linear_` 24, `mphase_` 20, `late_` 6, `trailing_` 13, the collapse 1) and the
unit pin `post_tick_def_read_pre_tick_is_a_register` (D-9); `build.rs` `SKIP` rows **4 of
7** (`probe` — stale already: its source carries no opt-out and it measures agreeing,
`review/MEASUREMENTS.md` §A, deletable now; `out_from_reg_before_commit`, `vr_sink_mealy`,
`vr_skid` — rule 10's, deletable when their fixtures are rewritten or removed; the other
three — `rv32i_cpu_pipelined`, `two_domain_top`, `mod8` — cite no rule and stay; **no**
`SKIP` row cites a row of this section); `phase_sensitive_checks.rs` re-scoped to what is
left. **Rows D-8 closed before this section existed**, recorded as closed and not
re-derived: the mux + hold form on a wire-valued port, on the linear route, and the
`for`-loop exclusion (I.5.10 gaps 9, 13, unstated 31; `vlir_lower.rs:1590`); the carrier
flop for an input read reaching a `RegOut` drive, the single-tick carrier `let` emitted as a
wire (`e19e`, `e19l`) and the forwarded trailing guard (`e19k`) (I.6.9, I.6.10) — each a
realization of the second instant, and there is none.

**II.3.3 The rows.** Known today, every status measured (D-13 §0, `scratchpad/predictions.md`
of session spec-packet-II-b; the probes are not in the tree). A row's shape is IN by I.2.4
unless the row says otherwise.

| Row | Shape | Meaning | Date | Transpiler site | Fix direction | Status |
|---|---|---|---|---|---|---|
| (a) | a nested `continue` | I.8.3 (D-10 A2-i): a zero-time return to the nested loop's head, in the same resumption | 2026-11-05 | `control_extract.rs:1777-1783` (the rotated head holds `C ; W`, so re-entering it would run `C`) | lower as the loop's entry copy `W ; pc = head` — the statements `lower_into` already emits for entering the loop (`:1278-1281`) | REFUSED-one (the transpiler; the macro admits, D-10 probe) |
| (b) | a nested loop whose ticks lie only inside `if`/`match` arms | I.8.2: its tick sites are suspension sites like any other; the simulator runs it (measured: `b1`, `b2`) | 2026-11-05 | `copper-analysis/src/shape.rs:154-162` (`check_nested_loop_shape` clause 4, both front-ends, one message) gating off the lowering arm `control_extract.rs:1290-1300` at `unflattenable_in_expr` `:1804` | **measured on the gated arm** (D-13 §0 (ii), the clause and the gate turned off in a scratchpad copy of the toolchain): the arm **agrees** on the wait shape `loop { if c { tick } else { break } }` (`b1`, 48/48) and **diverges** on `loop { if c { n += 1; tick } else { n += 2; tick } if n[3] { break } }` (`b2`, 41 of 48 cycles) — each arm's tick allocates a state whose body is the post-tick continuation, and that continuation's fall-through to the nested head is a state transition costing a cycle, where the coroutine runs the continuation, the head test and the next arm's work in one resumption; `b1` agrees only because its continuation is empty. The fix: at every `AfterTick` fall-through of an arms-only loop, inline the head's lowered body (the clone the entry already uses) instead of `pc = head`, one clone per tick site; `b1` and `b2` as sweep fixtures when it lands. Until then the refusal stands with this measurement as its reason (D-13 A3) | REFUSED-both |
| (c) | two element writes on an array register in one resumption (`arr[0] = a; arr[1] = b; tick`) | I.2.4's `arr[i] = e` row: two statements of one resumption, each an element assignment, the last write to an element winning (I.7.3) | 2026-11-05 | `chir_lower.rs:3467-3481`: a `[T; N]` local with an inferable element width > 1 that the inference calls a register lowers as an internal memory with **one** write port (`:1721-1745`), and a second element write in the same cycle is refused ("array register `arr` is written twice in one cycle; it has ONE write port"); one write agrees (`c_arr1`) | a second write port, or a priority mux of the cycle's element writes onto one port (last write wins); note the classification keys on the element's *inferable* width — `[Bits::zero(); 4]` with no turbofish is a packed vector and is refused as a latch by `check_no_latches`, a message that names the wrong thing | REFUSED-one (the macro admits: rustc runs both writes) |
| (d) | a pre-loop constant initializer the evaluator cannot fold: a `const` whose body is not a literal (`const START: u8 = 1 + 2`), an associated const, a const generic used as a value, a constant free function | I.7.4(a), I.9.2: read once as a constant of I.14; `check_register_initializers` admits it (`is_constant_expr`: a SCREAMING_CASE identifier, a multi-segment path) | 2026-11-05 | `chir_lower.rs:1158-1198` (`lower_register_init`, `fold_const_init`: "register `n` has no power-on value the emitted SystemVerilog can state: its initializer names `const START`, whose value `1 + 2` is not an integer or boolean literal") | a constant evaluator over I.14's constant grammar — `ItemConst` bodies, associated consts, const generics, const fns — once I.14 states that grammar (D-19: I.14.4 — stated: Rust's constants, one evaluator for both front-ends and the interpreter; II.2.2 records the two over-approximations) | **CLOSED** (D-21, 2026-11-07): the fold runs `ConstEnv` (Phase C step 5) — a non-literal `const` body, an associated const, a `const fn`, a const generic (stated as the parameter) — and since D-21 A3 the single-segment `f(c)`, refused by both until then, is admitted by both (rustc in the macro); swept: `tests/fixtures/const_eval_dut.rs`, `tests/fixtures/const_init_dut.rs` |
| (e) | a `Bits` method outside the transpiler's passthrough set — `and_reduce`, `or_reduce`, `xor_reduce`, `nand_reduce`, `nor_reduce`, `xnor_reduce`, `concat`, `part_select`, `replicate`, `sign_extend`, `zero_extend`, `truncate`, `shift_left`, `shift_right`, `get`, `set`, `mux`, `with_lsb`, `with_msb`, `is_valid`, `lt_logic`, `gt_logic`, `le_logic`, `ge_logic`, `as_array` (the simulator's `copper-core/src/types.rs:149-508`); and, refused by name, `saturating_add`, `saturating_sub`, `checked_add`, `checked_sub` | I.2.4's `Bits` methods row: a pure expression of I.14, each method's meaning pending I.14 (D-19: I.14.3(d) — every meaning stated; `checked_*` OUTSIDE, `saturating_*` IN; the row owes `eq_logic`, `ne_logic`, `as_array_mut`, `from_array`, `from_slice`, `From`, `all_ones`, I.14.8) | 2026-11-05 | `chir_lower.rs:4842-4937` (`lower_method_call`: passthroughs `read`, `as_bool`, `as_u8`…`as_u128`, `as_usize`, `as_bits`, `lock`, `unwrap`, `clone`; lowered `arithmetic_shift_right`, `wrapping_add`/`sub`/`mul`, the memory `data`/`is_ready`; `:4922` the named four; `:4934` "method `…` is not supported in hardware expressions" for the rest — measured on `or_reduce`, `concat`) | one lowering per method as I.14 gives it a meaning (a reduction → `|x`/`&x`/`^x`, `concat` → `{a, b}`, `part_select` → a slice, the extensions → width casts, …); the checked/saturating four are I.14's to admit with a meaning or to refuse as a shape (a maybe-value, I.2.5(a)) | REFUSED-one, one ledger line per method (III.3) |
| (f1) | a tick-bearing counted `for` over a range empty only at run time | I.8.4(b) (D-10 A4-i): zero iterations, zero time | 2026-11-05 | `control_extract.rs:411-449` (the hoisted tick is taken after the first exit test breaks: one extra cycle per zero-count pass) | guard the hoisted tick on the counter having advanced (`c != a`, extending the author's-`break` guard `:435-449`), or lower I.8.5's kernel form through the rotation of I.8.2; D-10's probe (ii) as the fixture | ADMITTED-AND-DIVERGENT |
| (f2) | pre-loop expression statements, extraction route | I.9.3 (D-11 A): statements of the first resumption, executed once | 2026-11-05 | `control_extract.rs:532-549` (moved into state 0 and re-run at every return to it) | the entry of I.9.2 — an entry state or a started carrier, the form Phase C measures; D-11's probe (i) as the fixture | ADMITTED-AND-DIVERGENT |
| (f3) | pre-loop expression statements, linear route | I.9.3 | 2026-11-05 | `chir_lower.rs:1958-1972` (`_ => {}`: dropped silently) | the entry of I.9.2; D-11's probe (i-b) as the fixture | ADMITTED-AND-DIVERGENT |
| (f4) | the trailing region folded before the head at edge 1, linear route | I.9.2: the entry is H(M)'s initial state, the first resumption runs `s_pre` and the head only | 2026-11-05 | the phase-0 fold of `shir_lower.rs` (I.3.8) with `trailing_reg_outs` `:205-215`: the `always_ff` runs a trailing region the coroutine has not executed, showing the trailing drive's power-on operand one observation early | the entry of I.9.2; D-11's probes (iv), (iv-c) as fixtures; corpus-masked (64 linear-route modules, none with a non-zero power-on operand in its trailing region) | ADMITTED-AND-DIVERGENT |
| (f5) | a register read through an array-element assignment in the resumption that assigns it (`bits[12 + k] = w3[k]` beside `w3 = data_i`) | I.3.4, I.7.3: the resumption reads the value it assigned | 2026-11-05 | `shir_lower.rs:478-483` — `IndexAssign` is lowered with its committed `value` only, with no forwarded `edge_value` form (a port drive has both, `:363-378`), emitted at `vlir_lower.rs:2029`: the element read is the flop's `Q` | forward the register's new value into the element read as every other read of the resumption is forwarded; `q1_c_sipo_block` (`review/Q1_TRADEOFFS.md` §4 (ii), the same finding as §4 (c''')) as the fixture; reachable today, independent of the port kind (D-8 obligations) | ADMITTED-AND-DIVERGENT |
| (f6) | a `tick` before the module loop, linear route (R21) | none — OUTSIDE (I.2.2: `s_pre` is tick-free; I.9.3) | 2026-11-05 | `chir_lower.rs:1958-1972` (the pre-loop tick is dropped: the SV is `always_ff q <= a + 1` with no state); measured divergent at `obs_1` only (the simulator's first resumption ends at the pre-loop tick with no write; the SV's head runs from edge 1), agreeing from `obs_2` | the R21 refusal in both front-ends (II.1.2, Phase C); the row closes when it lands and is not a limitation — it is recorded so the shape is not silently wrong until then | ADMITTED-AND-DIVERGENT until R21 lands |
| (g) | a hex literal in a `from_lit` turbofish (`Bits::from_lit::<0xFF>()`) | I.14's constant (I.8.7; D-19: I.14.4 — any base and underscores; closed by the shared evaluator) | 2026-11-05 | `chir_lower.rs:3812` region (the constant parser reads decimal only) | parse hex const generics (II.1.3's parity list) | REFUSED-one (the transpiler; the macro admits) |
| (h) | a `continue` that targets a tick-bearing `for` (`for i in 0..n { if c { tick; continue; } …; tick }`) | I.8.5 (D-10 A2-i): the next iteration — `c = c + 1`, then the test with the next `c` | 2026-11-08 (D-23 (2); measured REFUSED-one 2026-11-06/07, §J.10 item 4, §R.3) | `control_extract.rs` `unflattenable_in_stmts`' `ForLoop` arm (the desugar declines the `for`: its hoisted tick would no longer stand for the last iteration's), now a backstop | lower the `continue` as the desugaring's increment-then-test — the entry copy row (a) uses for a nested `loop` (§J.4); the probe of §R.3 as the swept fixture | REFUSED-both (`front_end::check_ticking_for_continue`, one message; `ui/fail/ticking_for_continue`); corpus 0 |

(Phase C step 11, 2026-11-07: the Status column above is each row as recorded on its date; every
row but (e)'s `saturating_*` residue is **closed**, with its fixture, in III.3.4's ledger, and the
rows Phase C found that this table does not carry are III.3.4's `II.3 found …` lines.)

Cross-references, not rows: `/` is R15, refused in both front-ends by D-12 (II.1.2), its
meaning I.14's (D-19: I.14.3(b) — given; R15 becomes a II.3 row); a nested `continue` appears in II.1.3's parity list and is row (a). **Not
rows:** the two nested-loop fold defects of I.7.5 — the local assigned before a ticking
`for` and read inside it is missed by the inference but hoisted by the transpiler, so
sim ≡ SV holds, and the local assigned inside and read only after is a dead flop — are
inference defects with no divergence, III.1's (D-9; D-20: P3); the hoisted-initializer forwarding claim
(I.7.7) is unmeasured and III.1's to measure (D-20: P14) before it can be a row; `det_110101`'s golden
overlap (`review/Q1_TRADEOFFS.md` §4 (i)) is the example's. R3's over-refusal and
under-admission are II.1.4's Phase C items, not rows: they are a predicate's direction
(II.2.2), not a form the transpiler lacks.

**II.3.4 The three rows II.1.2 left unmeasured.** Measured on probes in both front-ends
(D-13 §0 (i), every verdict predicted first; three of fourteen predictions missed and are
named there). **R6, recursion through a helper `fn`:** the macro admits (rustc admits
non-`async` recursion and no macro rule looks), the transpiler refuses ("cannot inline
recursive function `dec` into hardware", `chir_lower.rs:389`, the `inlining` set `:2391`) —
II.1 S3 holds in the transpiler, the macro owes it: a REFUSED-one parity break of II.1.3, not
a row here (the kernel lacks recursion; I.2.5(c)). **R19, an `In` handle used other than as
the receiver of `.read()`:** the macro refuses all three spellings probed (`let h = x;`,
`x.clone().read()`, a helper taking `&In`), the transpiler **admits all three** and realizes
each as the alias it is (an `always_comb` wire of the port) — a parity break the other way
(M-only); the rule's recorded reason (the read injection, which D-8 removes) is gone and its
S0 reason stands as D-12 placed it (`let t = read x` the only read form; the name classes of
I.2.3), so the transpiler owes the shared function; not a row here. **R21, a `tick` before
the loop:** the macro admits (`validate_hardware_fn` counts any tick of the clock; R2 ranges
over loop paths), the linear route admits and drops the tick (row (f6)), the extraction route
refuses by accident ("clk.tick().await inside a conditional branch is not supported": the
statement is moved into state 0's body by `control_extract.rs:532-546`) — II.1 S0, both
front-ends owe the refusal with one message and a ui/fail case; (f6) closes with it.

**II.3.5 The inventory's SR/LL column, re-cut.** The column of `review/INVENTORY_RULES.md`
§(1) reads, after this section (D-13 A5): 1 → II.1 R16; 2 → II.1 R2/R3; 3 → II.1 R4 (clauses
1–2), II.2 D1 (clause 3), II.3 (b) (clause 4); 4 → II.1 R1; 5 → II.1 R11 (+ II.3 (d));
6 → I.10 (D-16: I.10.4); 7 → retired, vacuous; 8 → retired (II.2.1); 9–15 → retired (D-8); 16 → II.2 D2
(I.11; D-14: I.11.3). **Decided (D-13 A5):** the inventory stays the 2026-11-03 snapshot every decision
cites and gains one pointer line to this clause; the column is not rewritten there. This
section is the authority.

**II.3.6 Implementation map.** Rows: (a) `control_extract.rs:1777-1783`, `:1278-1281`; (b)
`copper-analysis/src/shape.rs:107-168` (clause 4 `:154-162`; both front-ends: macro
`copper-macros/src/lib.rs:977`, transpiler `copper-codegen/src/lib.rs:364`),
`control_extract.rs:1290-1300` (the arm), `:1804` (the gate), `chir_lower::nested_loop_error`
and `shir_lower::check_no_tick_in_branch` as defence in depth; (c) `chir_lower.rs:1708-1801`
(`array_regs`), `:3467-3481`; (d) `chir_lower.rs:1158-1198`, `fold_const_init`,
`copper-analysis/src/init.rs`, `cfg.rs:3519-3550`; (e) `chir_lower.rs:4842-4937`; (f1)
`control_extract.rs:339-470`; (f2) `control_extract.rs:520-549`; (f3) `chir_lower.rs:1958-1972`;
(f4) `shir_lower.rs:205-215`, `:363-367`, the phase-0 fold; (f5) `shir_lower.rs:478-483`,
`vlir_lower.rs:2029`, `:910`; (f6) `chir_lower.rs:1958-1972`, `control_extract.rs:532-546`;
(g) `chir_lower.rs:3812` region. Retired rules: `copper-analysis/src/lib.rs:70-205`
(rules 9–15), `cfg.rs:1218-1234` (rule 11; the guard edition on
`claude/trailing-regout-guard-67cf37`, unmerged), macro `:1065-1322`, transpiler
`lib.rs:385-460`; rule 7 at macro `:999`/transpiler `lib.rs:537` with
`vlir_lower::reject_memory_driven_comb_outputs`. Pins that stay: `unsupported_constructs.rs`
(`continue_inside_a_nested_loop_is_unsupported` → (a);
`ticks_only_inside_branches_has_its_own_diagnostic` → (b); `division_operator_is_unsupported`
→ R15; the memory tests → I.10 (D-16: I.10.7); the counted-`for` tests → I.8.5), `register_initializer_corpus.rs`
(R11), `comb_definite_assignment_parity.rs` (D2), `tests/det_for_probe.rs` (I.8.5),
`front_end_parity.rs` test 1 less rule 7. **The Phase C fix list, in order** (each row's
fixture lands with its fix; III.3): (1) the entry of I.9.2 — closes (f2), (f3), (f4); (2) the
hoisted-tick guard — (f1); (3) the array-element forwarding — (f5); (4) the nested-`continue`
entry copy — (a); (5) the arms-only fall-through clone, measured again on `b1`/`b2` — (b);
(6) the array write-port form — (c); (7) the constant evaluator, with I.14 (D-19: I.14.4, I.14.7 (2)) — (d); (8) hex const
generics — (g); (9) the method lowerings as I.14 lands (D-19: landed, I.14.3(d), I.14.7 (7)) — (e); (10) R21 in both front-ends —
closes (f6); then the deletions sized in II.3.2 (the 19 ui cases, the two pin files,
parity test 2, the battery's refused rows and verdict tables, the ~109 `cfg.rs` tests, the 4
`SKIP` rows, rules 7 and 9–15 with their macro and transpiler call sites), and the parity
fixes of II.1.3 (R6's macro side, R19's transpiler side, R21 in both).

**II.3.7 Carried list.**
- III.3: the conformance ledger's shape — one line per row of II.3.3 (one per method for
  (e)) and one per retired rule (7–15), each naming its fixture and closed when the fixture
  passes under the sweep (five stimulus sequences); the ledger is where a closed row is
  recorded, this section keeping the row with its date.
- III.2 (D-20: III.2.8): every ADMITTED-AND-DIVERGENT row puts its shape outside the agreement relation until
  closed; `b1`/`b2`, `r21_linear`, D-10's probe (ii), D-11's (i), (i-b), (iv), (iv-c) and
  `q1_c_sipo_block` as sweep fixtures once their fixes land (III.3).
- I.10: rule 6 (`check_memory_staging`) with its principle and the dominated staging
  (II.2.4); rule 7 retired here, re-derivable there only if a memory reaches a
  `combinational` module. (D-16: done — I.10.4, I.10.6; two rows for the received memory
  owed to II.3.3, D-16 obligations.)
- I.11: D2 (II.2.4); nothing of this section. (D-14: I.11.3 done; I.11.2(c) owes this section one
  row — a combinational port written twice at the top level, emitted as two `assign`s and refused by
  Verilator as MULTIDRIVEN, admitted by both front-ends, fix: the §10.12 routing for any port written
  more than once; the never-written port is D2's, not a row.)
- I.14: the constant grammar behind (d) and R11; a meaning for every method of (e) and for
  the checked/saturating four; the hex literal of (g); `/` (R15). (D-19: closed — I.14.3(b)–(d), I.14.4.)
- Phase C: the fix list of II.3.6 in order; the parity fixes of II.3.4; the deletions of II.3.2.
- `SPEC_OVERVIEW.md` §6: "none today" restated as this section's count — the coordinator's
  edit of the generating page.
- `review/INVENTORY_RULES.md` §(1): one pointer line to II.3.2 (D-13 A5), the column unchanged.

## Part III — Implementations and their obligations

### III.1 Propositions and their evidence

Decided: D-20 (2026-11-05). Anchor: none of its own — a section that sorts claims by the
evidence each rests on; the anchors it cites are the clauses'. Decided: the evidence kinds,
the statement of the projection as a bisimulation, the retirement of the two-iterations
proposition, the mechanization proposal.

In coroutine terms (D-7, D-8): Part I defines one object, the coroutine, and its trace; every
row below is a claim that some implementation — rustc's lowering run by the executor, the
macro, the analysis, the transpiler, the interpreter — computes that object or a stated
function of it.

**III.1.1 Evidence kinds.** A row names one kind, the strongest its evidence reaches.
**THEOREM** — follows by argument from Part I clauses named in the row; no test is needed for
its truth, and a test, if any, measures the implementation against it. **MEASURED** — a test
in the tree (file:line) or a recorded probe (its entry) exercised the claim; between two
realizations of one source this is **consistency**, never correctness (`SPEC_OVERVIEW.md` §7).
**ANCHORED** — measured against an independent reference (BaseJump, a hand-written golden, a
`REFERENCE` row, IEEE 1800 as executed by Icarus); this is the only kind that decides what the
hardware does. **ASSERTED** — stated in the code or the docs with no evidence. A row that is
measured **false today** says so and names the Phase C item that makes it true. A
proposition that no surviving rule or realization consumes is **retired**, not deleted.

**III.1.2 The propositions.**

| # | Proposition | Serves | About | Evidence | Open obligation |
|---|---|---|---|---|---|
| P1 | **The projection.** The emitted state machine is bisimilar to H(M) (I.0.4) under a relation `R` between `pc` values and **continuations** (resumption start points): `(p, k) ∈ R` iff the code `pc = p` resumes runs `k`. `R` is many-to-many — sites whose continuation is the loop head share `pc = 0`, and one site's continuation is cloned per arm and per hoisted counted-`for` tick — so P1 is no function from states to sites; trace equality follows step by step from equal initial states | I.0.4, I.0.8, I.8.7, I.9.7 | transpiler | MEASURED (consistency): the sweep (G-D) and the reference leg (G-F); `tests/reference_leg_measurements.rs:279` counts states against sites on 34 extracted modules (ratio 0.67–4.00, `review/MEASUREMENTS.md` §D.3) but does not build `R` | a static test that builds `R` per module and checks each transition; the initial state is P15's; mechanization proposed (III.1.4) |
| P2 | **The register proposition** (I.0.5(a)): overwriting every non-register at every `tick` suspension leaves the trace unchanged | I.0.5, I.7.1 | coroutine (via the interpreter) | MEASURED: the poison run, `tests/reference_leg_measurements.rs:381` — 47 modules clean; the tick-last `sync_2ff`'s `ff2` poisonable (first negative), `ff1` not (first positive) (§D.4) | re-run over the corpus once D-8's rewrite puts the 83 plain-`Out` modules in the leg |
| P3 | **`infer_registers` computes I.7.1**: over by the head clause (8 wires in 7 modules) and by the fold's added local, under by the fold's missed local | I.7.5, II.2.2 | analysis | MEASURED: the poison run flags nothing on the corpus and flags `v` on the fold probe (`tests/ref_probes/sync_2ff_tick_last.rs::fold_defect`); `copper-codegen/tests/register_reconciliation.rs` (fixtures only) | Phase C: delete the head clause, descend into the nested loop's CFG, two reconciliation fixtures (I.7.6) |
| P4 | **rustc's `Future` layout ⊇ I.7.1**, tight up to the counted-`for` iterators | I.0.5(c), I.7.7 | simulator | MEASURED: seven fixtures, `-Zprint-type-sizes` under `RUSTC_BOOTSTRAP=1`, `tests/reference_leg_measurements.rs:466` (§D.5) | none; not a standing pin (the flag is unstable) |
| P5 | **Order independence**: the levelized settle and the fixpoint oracle compute the same settled wires on every acyclic design | I.13.2(a), (c) | simulator | THEOREM (I.13.2(a): a DAG has one fixpoint) and MEASURED: `tests/levelized_differential.rs` 6/6, `tests/poll_order_fuzz.rs` 2/2, `tests/golden_traces.rs` 6+6, a 65-case corpus sample under the oracle (D-18 (i)) | the static cycle refusal at spawn (I.11.6, I.13.7 (4)) (D-22: discharged — the refusal at spawn under both schedulers and over a structural parent, Phase C step 10, §P.3) |
| P6 | **The simulator's X algebra equals IEEE 1800 4-state** on every operator but `eq_logic`/`ne_logic` | I.14.2 | simulator, interpreter | ANCHORED: Icarus Verilog 12.0, 400 rows per operator (D-19 (i-b)), a recorded table not in the tree | Phase C: `eq_logic`/`ne_logic` IEEE-exact; the table as a standing pin (I.14.7 (1)) |
| P7 | **`RefBits` ≡ `Bits<N>`** | I.14.7 | interpreter | MEASURED: `copper-ref/tests/bits_property.rs` (widths {1, 4, 8, 16, 32, 64, 128}, X-sprinkled operands) | the uncovered operators of I.14.8 |
| P8 | **Realizability**: for every shape both front-ends admit, the emitted text realizes the resumption's effects in every cycle — its reads of `x_{N+1}`, its register updates, its pending values, shown from `obs_{N+1}` — except the ADMITTED-AND-DIVERGENT rows of II.3.3 | I.0.6, II.3.1 | transpiler | MEASURED (consistency): the sweep over 166 modules (G-D); ANCHORED for the 5 `REFERENCE` modules and the goldens of III.2.11; the six defect rows measured divergent (D-13 §0, D-19) | the II.3.6 fix list; mechanization proposed (III.1.4) |
| P9 | **Two iterations suffice** (FORMAL_SEMANTICS P6/G6, [anvil Lemma C.19]) | I.3.6 | analysis | **retired** (D-20): its consumers were the trailing alignment rules, retired by D-8; the surviving analyses (`check_reachability`, `infer_registers`) are fixpoints over the CFG and unroll nothing. I.3.6's lemma (a segment crosses the back edge at most once) stays a fact Part I uses | none |
| P10 | **Front-end parity**: a module compiles under `#[hardware]` iff it transpiles | II.1.3 | macro, transpiler | MEASURED: `copper-analysis/tests/front_end_parity.rs:121`, `:158`; blind spots — R2, R16, `check_definite_assignment`, the header rules, I.2.4's syntactic shapes, the transpiler-admits direction; the open breaks of II.1.3 measured on probes (D-12 §0 (v), D-13 §0 (i)) | the II.1.3 fixes and the audit's extension |
| P11 | **Per-domain determinism**: a module none of whose inputs another domain drives has one trace under every interleaving | I.12.1 | simulator | THEOREM (I.12.1, I.0.2) and MEASURED (`two_domain_counter` under two interleavings, D-17 (vi); `tests/two_domain_counter_behavioral.rs:124` at three ratios); **false today** at startup: the first settle of any domain resumes every coroutine (D-17 (i), probe not in the tree) | Phase C I.12.6 (1) with probe (i) as a two-clock pin (III.2.8) (D-22: no longer false — `tests/two_clock_pins.rs::never_ticked_startup_one_resumption_agrees_on_all_three_legs`, sim ≡ SV ≡ ref, Phase C steps 2–3; checked in step 10, §P.2) |
| P12 | **The simulator is the coroutine**: rustc's lowering of the unchanged body, resumed once per tick at `t_{N+1}⁻` | I.0.1, I.0.7 | simulator, macro | MEASURED: sim ≡ ref on 46 modules (G-F); **false today** on a read after the last tick (`regout_trailing_single_tick`, sim = SV ≠ ref, pinned at `tests/reference_leg_measurements.rs:136`) and wherever the macro injects the barrier (`copper-macros/src/lib.rs:712-725`) | Phase C: one resumption per tick, the injection removed; the pin flips |
| P13 | **Hoisted and promoted locals ⊆ registers** (∪ carriers; none known) | I.7.7, I.0.8 | transpiler | MEASURED on fixtures only (`register_reconciliation.rs`); unmeasured on the corpus | a corpus measurement |
| P14 | **The hoisted initializer's forwarding is unobservable** (`control_extract.rs`'s comment that "a registered output can forward the flop before" its assignment) | I.7.7, II.3.3 | transpiler | ASSERTED | a probe; a II.3 row if it is observable |
| P15 | **The emitted initial state encodes the entry** (I.9.2) | I.9.2, I.9.7 | transpiler | MEASURED **false**: `tests/reference_leg_measurements.rs:80`, `:101` (sim = ref ≠ SV on rows (f2), (f3), (f4)) | Phase C: the entry of I.9.2 (II.3.6 item 1) |
| P16 | **`obs_0 = init(q)` on both sides** for a sequential module | I.9.4, I.6.6 | simulator, transpiler | MEASURED by probe in both power-on builds (D-11), never by the harness | III.2.1 |
| P17 | **A re-poll of a parked coroutine runs no statement** | I.13.2(b) | simulator | THEOREM (`ClockTick`'s target is fixed at `tick()`, I.0.7) | none |
| P18 | **The order of the clock's listeners is unobservable** for a `RegOut` | I.6.8 | simulator | THEOREM (the pending value is a copy) | none |
| P19 | **Hand wiring ≡ the emitted hierarchy** | I.13.4, I.13.6 | simulator, transpiler | ANCHORED at 2:1 (`tests/two_domain_hierarchy_cdc.rs`, 3/3 vs `examples/cdc/sv/two_domain_hierarchy.sv`); MEASURED for the received memory (`tests/received_memory_abi.rs`, 1/1) | a single-domain hierarchy reference (III.2.11) |
| P20 | **A wiring across domains outside a synchronizer is a type error** | I.12.4 | types | MEASURED: `copper-core/tests/ui/fail/` (E0308, E0599; trybuild) | the two-flop rule (I.12.6 (2)) (D-22: discharged — `front_end::check_two_flop`, both front-ends, Phase C step 10; and `()` is no domain, I.12.6 (4), step 11) |
| P21 | **`check_admissible` never over-rejects** (admissible Err ⟹ transpile Err) | II.2.2 | analysis | MEASURED: `copper-codegen/tests/admissible_calibration.rs` | none |
| P22 | **Every direction of II.2.2 is as stated** | II.2.2 | analysis | MEASURED by the witnesses in the table (`livelock_a`/`livelock_b`, the fold probe, `tuple_local`, `comb_definite_assignment_parity.rs`); D2's folded loop and unwritten port unpinned | the pins of II.2.3 (D-22: D2's two directions pinned — `static_predicate_directions.rs`, Phase C step 10, §P.5) |
| P23 | **The memory**: `ReadFirst`, latency 1, dual port | I.10 | simulator, transpiler | ANCHORED (`examples/memory/sv/dual_port_ram.sv`; `ram_read_first` `REFERENCE`); latency above 1, port priority and preloads sim ≡ SV only; no interpreter leg | the interpreter's memory model (D-16); the references of III.2.11 |
| P24 | **A combinational module** is a function evaluated per settle; the Mealy composition | I.11 | simulator, transpiler | ANCHORED: the five `examples/combinational/sv` goldens, `bit_not_bits`, the Mealy pair (D-14 (v)) | no interpreter leg (25 modules skipped by class) |

Counts (2026-11-05): 24 rows — THEOREM 4 (P5, P11, P17, P18), MEASURED 14, ANCHORED 4 (P6,
P19, P23, P24), ASSERTED 1 (P14), retired 1 (P9); measured false today 3 (P11, P12, P15).
Every MEASURED row between the simulator, the interpreter and the SV is consistency.

**III.1.3 The correction to I.0.4.** The interpreter measured `pc` states against tick sites
(`review/MEASUREMENTS.md` §D.3): 34 extracted modules, the ratio from 0.67 (`det_010_awaits`,
6 sites, 4 states) to 4.00 (`counted_repetition`, 1 site, 4 states). `pc` is allocated per
continuation, not per tick occurrence; sites whose continuation is the head merge into state 0
(`control_extract.rs:1213-1215`), and a ticking branch's continuation is duplicated per arm
(`:1310`, `:1315`, `:1333`). I.0.4's "many-to-one onto tick sites" is read as many-to-many (D-20
note there); P1 is stated over that relation.

**III.1.4 Mechanization — proposed, not started (D-20, dated 2026-11-05).** P1 and P8 are
proposed for mechanization in a proof assistant, after the interpreter and Phase C (the
2026-11-04 direction: the Rust interpreter first, mechanization later). P1 is the analogue of
Kôika's compiler theorem `∀R, s, r. δs(R)[r] = δ⟨s⟩r(R)`, a per-cycle equality of next-state
functions [koika §6], restated as a bisimulation because Copper's two sides do not share their
control state; Kami's refinement as trace inclusion over labeled transition systems [kami
§3.3] is the lifting from steps to traces. P8 goes further than either: both trust the last
step to HDL text (Kôika's "thin unverified pretty-printing layer" [koika §6]; Kami's
commercial Bluespec compiler [kami §4]), and Copper's emitted SystemVerilog *is* the
realization. No other row is proposed: the THEOREM rows are short arguments, and the MEASURED
and ANCHORED rows are claims about code a proof would have to model first.

**III.1.5 Implementation map.** The tests named in the table; the sweep's three legs
(`build.rs` → `tests/corpus_generated.rs`, `tests/common/mod.rs`); `tools/regression.sh`
G-D, G-E, G-F.

**III.1.6 Carried list.** Every "Open obligation" cell is a Phase C item or a probe owed
(the Phase C list of D-20, the input of III.3). The rows measured false today (P11, P12, P15) become MEASURED when their
items land.

### III.2 The agreement relation (sim ≡ SV ≡ reference)

Decided: D-20 (2026-11-05). Anchor: none of its own — the relation is what the three legs
are compared by; III.2.11 lists the anchors, which alone decide correctness. Decided: the
relation as three pairwise trace equalities over stated preconditions, X inside the
simulator–interpreter pair and outside the SV pair, the reading of a run-time failure, the
multi-domain coverage owed, the reading of each disagreement pattern.

In coroutine terms (D-7, D-8): the coroutine has one trace per input stream (I.0.3, I.12.1).
The simulator, the interpreter and the emitted SystemVerilog each produce a trace from the same
stream; the relation says when the three are the coroutine's, which is consistency. Whether the
coroutine's trace is what the hardware should do is the anchors' question.

**III.2.0 The relation.** For one **module instance** (a module at the parameters of its Rust
instance, I.2.2) under one **environment** (an input stream `x_1, x_2, …` per domain and, for
several domains, a stated interleaving of their ticks, I.1.5, I.12.1), each leg `L ∈ {sim, ref,
sv}` yields a trace `obs^L_0, obs^L_1, …` of every port, per domain (I.0.3, I.1.4). Two legs
**agree** iff their traces are equal at every observation the preconditions (a)–(g) below admit,
port by port and bit by bit, under the bit equality each pair states. **sim ≡ SV ≡ ref** is the
three pairwise relations: **sim ≡ ref** compares the kernel's values, X included (an X bit
equals only an X bit; `tests/common/mod.rs:1245` compares `Logic` vectors exactly); **sim ≡ SV**
and **ref ≡ SV** compare 2-state values and admit no X observation (III.2.2). Agreement is a
property of one instance under one environment; the sweep's verdict is the conjunction over
the stimulus sequences of III.2.9 and is **coverage, not proof**.

**III.2.1 (a) Observation 0 is compared.** `obs_0` is in the trace (I.1.4, D-1 A1); for a
sequential module it is `init(q)` (I.9.4). **Today it is not compared:** the testbench drives,
toggles the clock and then compares (`copper-sim/src/verification.rs:255-299`), and
`record_differential` (`tests/common/mod.rs:853`) is called after `tick_clock`, so every leg is
compared from `obs_1`; `obs_0 = init(q)` is asserted on the interpreter's side only
(`review/MEASUREMENTS.md` §D, the `obs_0` gap) and measured on both sides by D-11's probes, not by
the harness. Phase C: observe `obs_0` on all three legs.

**III.2.2 (b) X.** An X is a value of the kernel (I.14.2(a)) and is inside **sim ≡ ref**. It is
outside every pair with the SV (I.14.2(f)): Verilator is 2-state. **Decided (D-20):** a case
whose simulator trace holds an X bit at an observation the relation compares against the SV has
**left the relation** at that observation, and the harness reports it so — loudly, as its own
verdict, neither compared nor masked. **Today** the harness writes an expected X into the
testbench as 1 (`logic_vec_to_int`, `verification.rs:330-340`, used at `:259` and `:283`), so
such a case fails unless Verilator shows 1, with a message ("expected 255 got 0") that reads as
a divergence; the form decided is a Phase C harness change. Masking X bits was considered and
not taken (`mod8`'s `SKIP` row records it: a silent pass for every X). **"From reset"**: a
`RESET` row `(module, port, active_low)` (`build.rs:206-230`) declares that the relation covers
the module from the observation after the reset takes effect: the stimulus asserts the port on
cycle 0 of every sequence (`tests/common/mod.rs:577-600`; deasserted on the first live cycle of a
first-cycle sequence), so the resumption that ends at edge 1 reads it asserted, and comparison
from `obs_1` covers exactly the observations after that edge. The row's sentence asserts what
makes this sound — the reset assigns every X register (I.7.4(b)) on every path of that
resumption, or defines the state the module's X depends on (`rv32i_cpu_transpilable`: its boot
port owns the memory's write bus) — and nothing checks the assertion statically; a recorded X
after reset leaves the relation by the rule above. Once `obs_0` is compared (III.2.1), `obs_0` of
a `RESET` module is outside the relation, and the relation covers `obs_k` for `k ≥ 1`.

**III.2.3 (c) Inputs.** The environment drives X-free inputs (I.14.2(f): the testbench cannot
drive an X) only between an observation and the next tick (I.1.5); a mid-cycle drive and a
settle requested outside a tick are outside the language (I.1.5, I.13.8). Every port compared is
at most 64 bits wide (`verification.rs:330-340`, a `u64`; corpus-masked, the widest `Bits`
anywhere is 64, I.14.1(c)). A generic module is instantiated in the SV only at parameters its
Rust instance compiles at (I.14.5(d), until the elaboration-time check of I.14.7 (8)); the
sweep's widths are `build.rs`'s `PARAMS` (7 rows). The environment creates every wire at the
zero of its payload (`wire(zero)`, `registered_wire(clock, zero)`; I.6.6, I.11.2(e), I.13.4(c)),
which is `init(q)`.

**III.2.4 (d) Run-time failures.** An undelivered `data()` (I.10), an out-of-range address
(I.14.6(b)), an X fed to a condition or an `as_*` view (I.14.2(c)) and Rust integer overflow
(I.14.3) end the coroutine's trace at the failing resumption (I.14.6(b)). **Decided (D-20):** the
relation compares observations up to the last one before the failing resumption; **sim ≡ ref**
requires both to fail at the same resumption, and one failing where the other does not is a
disagreement; the SV, which shows a value where the coroutine fails (zero-fill reads 0, a random
power-on a random word, D-19 (vi)), is outside the relation from that resumption on, and the
case is reported as **truncated** at it — never as passing beyond it. The stimulus policy does
not steer around failures (`CORPUS_DIFFERENTIAL_SWEEP.md` §3.4). Today a simulator panic fails the
whole case and the interpreter's stop is a reference-leg failure (`tests/common/mod.rs:1233`);
the truncated verdict is a Phase C harness change.

**III.2.5 (e) Power-on.** The emitted text states every constant initializer as the register's
`initial` value and every port's power-on value as `init(q)` (I.7.4(a), I.9.7; `emit.rs:418-432`)
— a realization obligation, not an environment precondition. A register with a `Bits::x()`
initializer states nothing (I.7.4(b)) and its module is a `RESET` row or outside the relation
until assigned (III.2.2). `COPPER_VERILATOR_X_INITIAL=1` measures what is left unstated
(`verification.rs:497-560`).

**III.2.6 (f) Several domains.** The environment of a design with domains `D_1 … D_k` is a
stated interleaving of their ticks (I.12.1); a module is resumed by its own domain's edges only
(I.0.2) and its trace is per domain; the relation compares per-domain traces at each domain's
observations. The sweep drives **one** clock: a module with a second `Clock` is a `SKIP`
(`build.rs:702-708`), `two_domain_top` a named `SKIP` row, and the interpreter defers more than
one clock (`copper-ref/src/module.rs:211-215`). **Decided (D-20):** the relation's multi-domain
coverage is two dedicated pins, owed to Phase C — D-17's probe (i) (the never-ticked startup,
expected `q = 2` on all legs once I.12.6 (1) lands) and probe (vi) (`two_domain_counter` under the
interleavings P = [2;10] and Q = [1,3,1,3,2,2,3,1,2,2], sim against SV) — beside the existing
`tests/two_domain_hierarchy_cdc.rs` (2:1, anchored) and `tests/two_domain_counter_behavioral.rs`;
the sweep stays one-clock and its second-clock `SKIP` is this clause's precondition. Until the
pins land the never-ticked startup is a known disagreement outside the sweep (P11).

**III.2.7 (g) Hierarchy.** A design's hand wiring in the simulator and its emitted hierarchy are
two realizations of one design (I.13.4) and must agree as a design: per-domain traces of every
port the environment observes. Measured by one anchor (`tests/two_domain_hierarchy_cdc.rs`, P19)
and by the received-memory bus (`tests/received_memory_abi.rs`, sim ≡ SV only); every other
hierarchy is swept module by module. The interpreter's design leg (several instances over shared
values, I.13.2) is Phase C's.

**III.2.8 (h) Which legs, and how a disagreement is read.** All three pairs must agree where the
preconditions admit the observation. A disagreement is read by its pattern (`copper-ref/README.md`):

| Pattern | Reading |
|---|---|
| sim ≠ ref, SV = ref | a simulator or macro bug, or a Phase C obligation not yet landed (P12) |
| sim = ref ≠ SV | a transpiler bug, or an ADMITTED-AND-DIVERGENT row of II.3.3 (P8, P15) |
| sim = SV ≠ ref | both implementations share a reading Part I does not give: the two-job residue (`regout_trailing_single_tick`, its `REF_SKIP` row, pinned at `tests/reference_leg_measurements.rs:136`) |
| all three differ | a spec gap or an interpreter bug; read the interpreter's first disagreement first |
| a case left the relation (X vs SV, III.2.2) or truncated (III.2.4) | no reading of agreement; the precondition's row names why |

A module outside a leg's coverage is excused only by a row with a sentence (`SKIP`, `REF_SKIP`,
`RESET`, or the signature classes of `ref_skip_reason`), never silently (III.2.10). Every
ADMITTED-AND-DIVERGENT row of II.3.3 puts its shape outside the relation until its fix lands;
its fixture joins the sweep then (II.3.7).

(Phase C step 11, 2026-11-07 — the table's examples are the 2026-11-05 state, kept as the reading
of each pattern.) **No instance of any pattern is live.** P12 holds (the one-resumption simulator,
steps 2–3; `tests/reference_leg_measurements.rs::regout_trailing_single_tick_agrees_on_three_legs`
asserts sim = SV = ref, so the `sim = SV ≠ ref` example is history, its `REF_SKIP` row deleted and
`REF_SKIP` / `REF_SKIP_PATTERN` empty); P15 holds (step 5, the entry); every ADMITTED-AND-DIVERGENT
row of II.3.3 is closed with its fixture swept (III.3); the excused class is empty (G-G's EXCUSED
count is 0) and the pinned-disagreement count is 0. A disagreement found today is read by the table
and fails G-G unless a `REF_SKIP` row with a sentence names its pattern and a pin measures it.

**III.2.9 (i) Coverage: the stimulus policy.** Per swept clocked module, four seeded sequences
of 200 cycles, plus a 32-cycle sequence from every initial input vector when the module has at
most 4 input bits (the reset port excluded) or a fixed first-cycle pattern list otherwise; per
combinational module, the whole truth table up to 8 input bits, else 4 × 64 random vectors and
the pattern list — all in one testbench run of one Verilated model restarted at each boundary
(`CORPUS_DIFFERENTIAL_SWEEP.md` §3.2; `build.rs` `SEEDS` … `COMB_VECTORS`; pinned by
`tests/corpus_generated.rs::the_stimulus_policy_is_what_the_design_doc_says`, G-D). The reference
leg replays the same sequences (G-F). This is **coverage**: a pass says the legs agree on these
sequences. Measured blind spot: a slow protocol's sites are not all reached in 200 cycles (the
UARTs: `uart_rx` 1 of 5 sites, `uart_tx` 2 of 4; §D.3). **Reach on 2026-11-05** (`build.rs`'s
ledger at 0138a87): 166 modules swept (5 anchored, 161 sim ≡ SV only); the reference leg on 46,
113 excused (4 `Memory`, 25 combinational, 1 measured disagreement, 83 plain `Out` awaiting
D-8's rewrite), the 7 `SKIP` modules not asked. Measured by the full regression on 2026-11-06
(`tools/regression.sh`, `REGRESSION OK`, same tree): G-C 115 test files, G-D 159 differential
cases plus 7 ignored with a reason, G-E 5 anchored and 161 transpiler-only, G-F 46 run and 120
skipped with a reason (the 113 above plus the 7 `SKIP` modules) — the ledger's counts confirmed.

**III.2.10 The excusing rows, by precondition** (2026-11-05; lists and counts re-derived from
the code at Phase C step 9, 2026-11-07). `SKIP` (`build.rs:52-99`, 3): `mod8` — (b), an X
observation (`% 0`); `rv32i_cpu_pipelined` — (g), a `Memory` parameter the harness cannot own
(its own lane, `tests/rv32i_pipelined_verilator.rs`, sim ≡ SV under a hand-written owner);
`two_domain_top` — (f)/(g), a structural two-clock parent (the reference leg runs it as the
design it names outside the sweep, `tests/two_domain_hierarchy_cdc.rs::reference_interpreter_design_matches_sim`,
against the hand-wired simulation at 2:1, 3:1 and 1:1). `RESET` (`build.rs:169-191`, 3):
`shift_register`, `simple_counter` — (b)/(e), a `Bits::x()` register; `rv32i_cpu_transpilable`
— (b), a memory the reset defines. `REF_SKIP` (`build.rs:202-203`): **0** named rows, and
`REF_SKIP_PATTERN` is empty. The signature classes of `ref_skip_reason` (`build.rs:725`) — a
`Memory` parameter (I.10.5: the owner's storage), a second clock in one kernel (I.12), a
`structural` parent (I.13.4(a): a design, not a module) — excuse **0** swept modules: each is
reached only by a module `SKIP` already names. The plain-`Out` class and the combinational
class are deleted (R8 left the first empty; the interpreter evaluates the second, I.11). The
reference leg runs **211** of the sweep's 214 modules: 159 clocked (18 of them binding a
`Memory`, I.10) and 52 combinational (I.11).

**III.2.11 (j) Anchors.** An independent reference — one nobody derived from the simulator, the
transpiler or the interpreter — is the only evidence of correctness (`SPEC_OVERVIEW.md` §7,
`ANCHORING_A_MODULE.md`): a misreading shared by the legs is invisible to the relation and
visible to it. In the sweep: 3 `REFERENCE` rows (`build.rs:194-205`: `ram_read_first`,
`bit_not_bits`, `handshake`), anchoring **5** modules (`handshake` has three copies —
`examples/sequential/handshake.rs`, `tests/fixtures/handshake_dut.rs`,
`tests/fixtures/wait_loop_dut.rs`); G-E reports 5 anchored, 161 not. Outside the sweep, in their
own tests: the BaseJump set (`examples/basejump/sv/`, 7), `examples/sequential/sv/` (4),
`examples/combinational/sv/` (5), `examples/memory/sv/dual_port_ram.sv`, `examples/cdc/sv/` (2),
`tests/fixtures/timing_probe_sv/mac_fsm.sv` (register set only), the `det_010` goldens, the
pipelined CPU's 13 programs, the Mealy pair (D-14 (v)) and IEEE 1800 under Icarus for the X
algebra (D-19 (i-b)) — `review/MEASUREMENTS.md` §C. **Owed by the user** (the user writes
references; none is written here): (1) a body whose trailing region follows more than one tick,
and the trailing-before-head order through a port (I.3.9; D-3); (2) one trailing `RegOut` write,
and one port written twice with distinct values before one edge (I.6.10; D-6); (3) a module with
a loop-born register (I.7.7; D-9); (4) a nested tick-bearing loop other than the `while` wait
(I.8.7; D-10); (5) a pre-loop expression statement, and a trailing drive over a non-zero
power-on operand (I.9.7; D-11); (6) a RAM with a write (`bsg_mem_1r1w_sync`, not vendored) and a
two-cycle read (I.10.8; D-16); (7) a two-domain design under an interleaving beyond 2:1, and the
never-ticked startup (I.12.7; D-17); (8) a single-domain hierarchy of a `RegOut` core and a
`combinational` reader, and a two-domain parent under a second interleaving (I.13.8; D-18).
D-4's dual-consumer `Out` leg of `sipo_block` and the parking rule's definition are vacuous under
D-8 (no plain `Out`, no park).

**III.2.12 Implementation map.** Harness: `tests/common/mod.rs` — `StimulusPlan` and `stim`
(`:570-610`, the reset on cycle 0), `EquivalenceTest::record_differential` (`:853`), `record`
(`:865`), `finish` (`:886`), `ReferenceLeg` (`:1083`, the exact comparison `:1245`);
`copper-sim/src/verification.rs` (the testbench `:250-300`, `logic_vec_to_int` `:330-340`,
power-on `:497-560`); `build.rs` (`SKIP` `:52`, `PARAMS` `:152`, `REFERENCE` `:194`, `RESET`
`:206`, `REF_SKIP` `:239`, the stimulus constants, the ledgers); `tools/regression.sh` (G-D, G-E,
G-F). **Phase C obligations (D-20):** (1) observe `obs_0` on all three legs (III.2.1); (2) the X
verdict — a recorded X against the SV reported as "left the relation", not compared as 1
(III.2.2); (3) the truncated verdict at a run-time failure, sim and ref required to fail at the
same resumption (III.2.4); (4) the two two-clock pins (III.2.6); (5) the interpreter's legs for
memory, combinational modules and designs; (6) the stale `REF_SKIP` sentences restated.

**III.2.13 Carried list.** `poll_tasks` outside a tick and a mid-cycle drive stay outside the
language (III.2.3); whether to refuse them in the executor is Phase C's (I.1.6). The references of
III.2.11 (the user's). The test-phase counts of G-C/G-D/G-F, to be read on the next full run.

### III.3 Conformance ledger

Decided: D-20 (2026-11-05), step (11) of its Phase C list, and the shape in II.3.7's first bullet
(D-13). Written 2026-11-07 (Phase C step 11, `review/MEASUREMENTS.md` §R). Anchor: none of its own —
a line records whether the code conforms to a clause, and the evidence it names is consistency
(III.1.1) unless the line names an anchor. Decided by the shape: one line per normative clause of
Parts I and II, per rule row of II.1.2, per derived rule of II.2.1, per row of II.3.3 (one per
method for row (e)) and per retired rule (II.3.2 rules 7–15); each line names its fixture and the
guard that runs it; a line is **CLOSED** only when its fixture passes under the sweep's full
stimulus policy (III.2.9), otherwise **OPEN** with an owner and a reason. II.3.3 keeps every row
with its date and status as recorded; this ledger is where a row is recorded as closed, and no
history is deleted. Where a line's status needs a design decision rather than a measurement, the
line is OPEN and its owner is "decision" (the packets are §R.6).

In coroutine terms (D-7, D-8): every Part I clause says something about the coroutine or its trace;
a CLOSED line says the three ways of computing that trace — rustc's lowering run by the executor,
the emitted machine, the interpreter — agree on a module that exercises the clause, or that both
front-ends refuse, with one message, a shape the clause puts outside.

**III.3.1 The form of a line, and the check.** A line is a row `| id | clause | fixture | guard |
status | owner / reason |`. The fixture cell holds backticked tokens: `sweep:<file>::<fn>` — a
`#[hardware]` module of `tests/fixtures/` or `examples/` the sweep runs (its `_differential` and
`_reference_leg` cases, every stimulus sequence of III.2.9); `ui:fail/<case>`, `ui:pass/<case>` — a
`copper-macros` trybuild case, which `front_end_parity_ui.rs` also runs through the transpiler
(the parity audits, "PA" below); `coreui:fail/<case>` — a `copper-core` trybuild case (rustc's
refusal); `test:<file>::<fn>` — a named `#[test]`; `example:<name>` — a registered example
binary; `gone:<fn>` — a retired rule's function, defined nowhere in the crates (never the only
token of a CLOSED line). Guard abbreviations: **G-D/F/G** — the sweep's differential case (G-D),
its reference leg (G-F) and the relation's verdict ledger (G-G); **G-C** — a test binary that
ran; **PA** — `copper-analysis/tests/front_end_parity.rs` and
`copper-codegen/tests/front_end_parity_ui.rs`; **G-I**, **G-J** as in CLAUDE.md; **G-B** — the
example ran. **The check is G-K**: `tests/conformance_ledger.rs` reads this section and fails on a
clause of Parts I–II, a II.1.2 rule, a II.2.1 rule, a II.3.3 row or method or a retired rule
without exactly one line; on a line naming nothing of the SPEC; on a CLOSED line without a running
fixture or a guard, or an OPEN one without an owner; and on any CLOSED token that does not resolve
(a missing file or `fn`, a module the sweep emitted no case for or `SKIP`s, a trybuild case
without its `.stderr`, a `#[test]` that is `#[ignore]`d, an unregistered example, a `gone:`
function still defined). It writes one `REQUIRE` line per fixture to `COPPER_LEDGER_REPORT`, and
`tools/regression.sh` finds each one's `... ok` in the same run's log (both cases of a swept
module; an example's PASS), so a CLOSED line whose fixture did not run and pass in that run fails
the regression. I.5 has no numbered clause (a forwarding stub, D-15) and so no line.

**III.3.2 Part I.**

| Line | Clause | Fixture | Guard | Status | Owner / reason |
|---|---|---|---|---|---|
| I.0.1 | the coroutine | `sweep:examples/sequential/pattern_detector_2.rs::det_010_awaits`, `test:copper-ref/tests/interp_smoke.rs::sync_2ff_one_job_is_two_flops`, `ui:fail/foreign_await` | G-D/F/G, G-C, PA | CLOSED | the body unchanged, one suspension kind (§G, §H.3) |
| I.0.2 | the resumption protocol | `test:tests/two_clock_pins.rs::never_ticked_startup_one_resumption_agrees_on_all_three_legs`, `test:tests/cdc_synchronizer_anchor.rs::sync_2ff_observable_latency_is_one_destination_cycle` | G-C | CLOSED | one resumption per tick since step 3; the startup divergence closed (§F.2, §P.2) |
| I.0.3 | the trace | `test:tests/sweep_sequence_restart.rs::every_sequence_starts_from_the_initial_state`, `sweep:tests/fixtures/preloop_entry_dut.rs::preloop_probe` | G-C, G-D/F/G | CLOSED | `obs_0` compared on all three legs since step 1 (§E.1) |
| I.0.4 | the projection H(M) | `test:tests/projection_relation.rs::the_projection_relation_holds_on_the_corpus`, `test:tests/reference_leg_measurements.rs::pc_states_vs_tick_sites`, `sweep:tests/fixtures/control_extraction_dut.rs::branch_merge` | G-I, G-C, G-D/F/G | CLOSED | P1's relation built and checked per module (§L), many-to-many (III.1.3) |
| I.0.5 | the register proposition | `test:tests/reference_leg_measurements.rs::poison_run_for_the_register_proposition`, `test:copper-codegen/tests/register_reconciliation.rs::corpus_realized_registers_are_inferred`, `test:tests/reference_leg_measurements.rs::future_size_bounds_the_register_storage` | G-C | CLOSED | P2 re-run clean, the head clause deleted (§K.1, §K.12) |
| I.0.6 | the three admissibility principles | `test:copper-analysis/tests/front_end_parity.rs::every_corpus_module_passes_every_shared_rule_in_its_mode`, `test:copper-codegen/tests/front_end_parity_ui.rs::the_transpiler_refuses_every_shape_the_macro_refuses_with_the_same_message` | G-C, PA | CLOSED | every message names its clause (§P.1); the alignment rules retired (§G.4) |
| I.1.1 | domains and edges | `test:tests/two_clock_pins.rs::interleavings_give_one_trace_per_domain`, `test:tests/cdc_synchronizer_anchor.rs::source_domain_ticks_do_not_advance_the_synchronizer` | G-C | CLOSED | `cycle::<D>()` per domain (§F) |
| I.1.2 | instants | `test:tests/cdc_synchronizer_anchor.rs::sync_2ff_observable_latency_is_one_destination_cycle`, `test:copper-ref/tests/interp_smoke.rs::registered_passthrough_shows_this_cycles_input_next_observation` | G-C | CLOSED | the post-edge settle resumes no coroutine (§F) |
| I.1.3 | state and wires | `test:tests/simulator_corner_cases.rs::acyclic_combinational_chain_converges_in_one_settle`, `test:copper-ref/tests/interp_design.rs::one_settle_evaluates_in_topological_order` | G-C | CLOSED | — |
| I.1.4 | observation | `sweep:tests/fixtures/preloop_entry_dut.rs::preloop_linear`, `test:tests/sweep_sequence_restart.rs::every_sequence_starts_from_the_initial_state` | G-D/F/G, G-C | CLOSED | `obs_0` in the trace and compared (§E.1) |
| I.1.5 | the environment | `test:tests/agreement_relation.rs::the_relation_testbench_never_compares_an_x`, `test:tests/sweep_sequence_restart.rs::every_sequence_starts_from_the_initial_state` | G-C | CLOSED | drive / tick / observe only; a mid-cycle drive and a settle outside a tick are outside the language, and refusing them in the executor is III.2.13's carried item, not this clause's |
| I.2.1 | the object | `test:copper-codegen/tests/front_end_parity_ui.rs::the_transpiler_admits_every_shape_the_macro_admits`, `test:copper-analysis/tests/front_end_parity.rs::every_corpus_module_passes_every_shared_rule_in_its_mode` | G-C, PA | CLOSED | both front-ends hold one `ItemFn` and run one rule list (§P.1) |
| I.2.2 | the module header | `ui:fail/plain_out_on_sequential`, `ui:fail/two_clocks_sequential`, `ui:fail/preloop_tick`, `ui:fail/missing_domain_port` | G-C, PA | CLOSED | R8, R10, R20, R21 and the port-domain clause in both front-ends (§G.4, §J.12, §P.1, §Q.1) |
| I.2.3 | the kernel grammar | `ui:fail/held_preloop_read`, `sweep:tests/fixtures/preloop_wire_dut.rs::preloop_wire_read`, `test:copper-analysis/tests/front_end_parity.rs::no_corpus_module_holds_a_preloop_read` | G-C, PA, G-D/F/G | CLOSED | D-21 C2 and H1 (§N.2, §N.4); a pre-loop `const { }` block is not named in `s_pre`'s grammar (§L.5 item 6) — wording owed to I.2.3, no behaviour |
| I.2.4 | the desugaring and the classification | `sweep:tests/fixtures/nested_continue_dut.rs::nested_continue`, `ui:fail/input_read_in_macro` | G-D/F/G, PA | OPEN | the front-ends (parity): an element write to an array register inside a combinational `for` (line II.3 found array-for) and a `continue` of a tick-free `for` (line II.3 found tickfree-continue). The `continue` of a tick-bearing `for` is REFUSED-both since 2026-11-08 (D-23, line II.3.3 (h)). The table's NOT YET / "today" cells predate steps 5–10 |
| I.2.5 | outside the kernel, and why | `ui:fail/clock_construction`, `ui:fail/module_loop_break`, `ui:pass/recursive_helper_r6` | G-C, PA | CLOSED | R6 the named exception (D-22, §Q.2); the other reasons refused in both (§P.1) |
| I.2.6 | types | `test:copper-codegen/tests/admissible_calibration.rs::grammar_never_rejects_what_the_transpiler_lowers`, `sweep:tests/fixtures/signedness_dut.rs::signed_lt_via_cast` | G-C, G-D/F/G | CLOSED | `check_admissible` shared (§P.1); R16's bound is its own line (II.1.2 R16) |
| I.2.7 | the four modes | `sweep:tests/fixtures/sync_2ff_dut.rs::sync_2ff_concrete`, `sweep:tests/fixtures/one_bit_comparator_dut.rs::one_bit_comparator`, `ui:pass/structural_net_ok` | G-D/F/G, PA | CLOSED | — |
| I.3.1 | executions | `sweep:tests/fixtures/counted_repetition_dut.rs::counted_repetition`, `sweep:tests/fixtures/wait_loop_dut.rs::waiter` | G-D/F/G | CLOSED | — |
| I.3.2 | segments | `sweep:tests/fixtures/continue_skip_dut.rs::skip_on_halt`, `sweep:tests/fixtures/for_empty_dut.rs::for_empty`, `test:tests/continue_equivalence.rs::a_continue_costs_no_cycle_and_matches_transpiled_verilog` | G-D/F/G, G-C | CLOSED | a zero-trip `for` adds no boundary ((f1), §J.2) |
| I.3.3 | the residue | `test:tests/projection_relation.rs::the_projection_relation_holds_on_the_corpus`, `sweep:tests/fixtures/control_extraction_dut.rs::branch_merge` | G-I, G-D/F/G | CLOSED | consistency only; the anchor of I.3.9 is the user's |
| I.3.4 | instants: program order | `sweep:tests/fixtures/regout_forwarding_dut.rs::assign_then_write`, `sweep:tests/fixtures/array_forwarding_dut.rs::f5_guard` | G-D/F/G | CLOSED | every read forwarded ((f5), §J.3) |
| I.3.5 | the trailing region | `sweep:tests/fixtures/trailing_statements_dut.rs::trailing_local_into_head`, `sweep:tests/fixtures/trailing_statements_dut.rs::trailing_out`, `test:tests/cdc_synchronizer_anchor.rs::independent_verilog_matches_sim` | G-D/F/G, G-C | CLOSED | the head of the next resumption, the fold removed (§H.2) |
| I.3.6 | the back edge | `ui:fail/partial_tick_path`, `sweep:tests/fixtures/trailing_statements_dut.rs::trailing_local_into_head` | PA, G-D/F/G | CLOSED | — |
| I.3.7 | cycle 0 | `sweep:tests/fixtures/preloop_entry_dut.rs::preloop_linear`, `test:tests/reference_leg_measurements.rs::preloop_statement_agrees_on_both_routes`, `test:copper-ref/tests/interp_smoke.rs::preloop_statement_runs_once` | G-D/F/G, G-C | CLOSED | (f2), (f3), P15 (§J.1) |
| I.4.1 | the value of a read | `sweep:examples/basejump/sipo_block.rs::sipo_block`, `sweep:examples/sequential/lfsr.rs::lfsr`, `example:sipo_block` | G-D/F/G, G-B | CLOSED | `sipo_block` anchored (BaseJump) |
| I.4.2 | one class of read | `test:tests/det_010_independent_golden.rs::det_010_awaits_matches_independent_verilog`, `sweep:tests/fixtures/read_timing_dut.rs::sample_hold_2` | G-C, G-D/F/G | CLOSED | `classify_reads` deleted (§H.3); anchored by the golden |
| I.4.3 | trailing reads | `test:copper-ref/tests/interp_smoke.rs::trailing_read_belongs_to_the_next_resumption`, `test:tests/reference_leg_measurements.rs::regout_trailing_single_tick_agrees_on_three_legs`, `sweep:examples/basejump/bsg_counter_up_down.rs::bsg_counter_up_down` | G-C, G-D/F/G | CLOSED | P12 holds (§H.5 item 4) |
| I.4.4 | cycle-0 reads | `sweep:tests/fixtures/preloop_wire_dut.rs::preloop_wire_first_head` | G-D/F/G | CLOSED | — |
| I.4.5 | the passthrough | `ui:fail/plain_out_on_sequential`, `test:copper-ref/tests/interp_design.rs::the_mealy_pair_reads_the_committed_register_and_the_current_input` | PA, G-C | CLOSED | vacuous for a sequential module (D-8); the combinational spelling |
| I.4.6 | no static approximation | `sweep:tests/fixtures/read_timing_dut.rs::accum_2`, `test:tests/read_timing_equivalence.rs::accum_2_sim_matches_verilog` | G-D/F/G, G-C | CLOSED | the read-site predicates deleted (§H.3, §H.4) |
| I.6.1 | the write | `sweep:tests/fixtures/out_phase_dut.rs::regout_trailing_single_tick`, `test:tests/regout_forwarding_equivalence.rs::assign_then_write_sim_matches_transpiled_verilog` | G-D/F/G, G-C | CLOSED | one port form (§H.1) |
| I.6.2 | several writes | `sweep:examples/sequential/pattern_detector_2.rs::det_010_awaits`, `ui:pass/multi_write_regout_ok` | G-D/F/G, PA | CLOSED | last write wins; an anchor with two distinct values is the user's (III.2.11 (2)) |
| I.6.3 | hold | `sweep:examples/basejump/sipo_block.rs::sipo_block`, `sweep:examples/basejump/bsg_dff_en.rs::bsg_dff_en` | G-D/F/G | CLOSED | anchored (BaseJump) |
| I.6.4 | observation | `sweep:tests/fixtures/handshake_dut.rs::handshake` | G-D/F/G | CLOSED | `handshake` anchored (`REFERENCE`) |
| I.6.5 | cycle 0 | `sweep:tests/fixtures/preloop_entry_dut.rs::preloop_probe`, `sweep:tests/fixtures/preloop_entry_dut.rs::preloop_linear` | G-D/F/G | CLOSED | (f2)/(f3) closed (§J.1) |
| I.6.6 | the initial value | `test:tests/reference_leg_measurements.rs::preloop_statement_agrees_on_both_routes`, `sweep:examples/basejump/sipo_block.rs::sipo_block` | G-C, G-D/F/G | CLOSED | `obs_0 = init(q)` compared (§E.1) |
| I.6.7 | an input read after the tick | `sweep:examples/basejump/bsg_counter_up_down.rs::bsg_counter_up_down`, `sweep:tests/fixtures/sync_2ff_dut.rs::sync_2ff_concrete` | G-D/F/G | CLOSED | no carrier (§H) |
| I.6.8 | composition and one driver | `coreui:fail/regout_is_not_clone`, `test:tests/two_clock_pins.rs::never_ticked_startup_one_resumption_agrees_on_all_three_legs` | G-C | CLOSED | — |
| I.7.1 | the register: definition | `test:tests/reference_leg_measurements.rs::poison_run_for_the_register_proposition`, `test:copper-codegen/tests/register_reconciliation.rs::corpus_realized_registers_are_inferred` | G-C | CLOSED | P2, P13 (§K.5, §K.7) |
| I.7.2 | what is a register | `sweep:tests/fixtures/struct_pipeline_dut.rs::tuple_destructure`, `sweep:tests/fixtures/mac_pipeline_dut.rs::mac_pipeline`, `sweep:tests/fixtures/array_forwarding_dut.rs::f5_sipo` | G-D/F/G | CLOSED | (h)'s forwarding defect closed (§J.3) |
| I.7.3 | the value of a register | `sweep:tests/fixtures/sync_2ff_dut.rs::sync_2ff_concrete`, `test:tests/cdc_synchronizer_anchor.rs::register_inference_matches_the_independent_reference`, `sweep:tests/fixtures/array_two_writes_dut.rs::arr_same_element` | G-D/F/G, G-C | CLOSED | — |
| I.7.4 | power-on | `test:tests/lfsr_equivalence.rs::before_reset_the_sv_holds_the_rust_initial_value_like_the_simulator`, `ui:fail/register_init_not_constant`, `test:copper-codegen/tests/register_init_parity.rs::the_transpiler_refuses_every_initializer_rustc_refuses` | G-C, PA | CLOSED | D-21 (§N); the transpiler's fold narrower than rustc is a REFUSED-one break, line II.3 found const-call |
| I.7.5 | the back-edge clause, re-derived | `sweep:tests/fixtures/nested_loop_registers_dut.rs::nested_loop_missed`, `sweep:tests/fixtures/nested_loop_registers_dut.rs::nested_loop_added`, `sweep:tests/fixtures/trailing_statements_dut.rs::trailing_local_into_head` | G-D/F/G | CLOSED | head clause deleted, the nested CFG descended (§K.1, §K.3) |
| I.8.1 | the module loop | `ui:fail/module_loop_break`, `ui:fail/module_return`, `sweep:tests/fixtures/continue_skip_dut.rs::skip_on_halt` | PA, G-D/F/G | CLOSED | R7 in both (§P.1) |
| I.8.2 | nested tick-bearing loops | `ui:fail/for_break_before_tick`, `ui:fail/tick_first_wait`, `sweep:tests/fixtures/arms_only_loop_dut.rs::arms_only_match` | PA, G-D/F/G | CLOSED | (b) closed (§J.5) |
| I.8.3 | `break` and `continue` of a nested loop | `sweep:tests/fixtures/nested_continue_dut.rs::nested_continue_match`, `ui:fail/labelled_break`, `ui:fail/valued_break` | G-D/F/G, PA | CLOSED | (a) closed (§J.4); R12 in both (§P.1); the counted-`for` case is I.8.5's line |
| I.8.4 | the zero-iteration loop | `sweep:tests/fixtures/for_empty_dut.rs::for_empty`, `ui:fail/literal_empty_for_boundary`, `ui:pass/livelock_a` | G-D/F/G, PA | OPEN | copper-analysis (R3): a nested `for` over a range empty only through a constant (`0..EMPTY`) is still counted as a boundary — under-admission recorded in II.2.2 (§P.5); fix: evaluate the range with `ConstEnv` as R5 does. (f1) itself closed (§J.2) |
| I.8.5 | the counted `for` as a desugaring | `test:tests/det_for_probe.rs::det_for_rtl_matches_the_simulator_cycle_for_cycle`, `sweep:tests/fixtures/counted_repetition_dut.rs::counted_repetition`, `ui:fail/runtime_for_bound`, `ui:fail/ticking_for_continue` | G-C, G-D/F/G, PA | CLOSED | D-23 (2), step 11b (§S): the `continue` of a tick-bearing `for` is refused in both front-ends with one message (line II.3.3 (h), whose fix direction is the lowering); the desugaring's other executions realized and anchored |
| I.9.1 | the first resumption | `sweep:tests/fixtures/preloop_entry_dut.rs::preloop_probe`, `test:tests/two_clock_pins.rs::never_ticked_startup_one_resumption_agrees_on_all_three_legs` | G-D/F/G, G-C | CLOSED | — |
| I.9.2 | `s_pre`, the initializers and the entry | `test:tests/projection_relation.rs::the_projection_relation_holds_on_the_corpus`, `sweep:tests/fixtures/preloop_entry_dut.rs::preloop_linear`, `test:tests/reference_leg_measurements.rs::trailing_fold_at_edge_1_agrees_on_three_legs` | G-I, G-D/F/G, G-C | CLOSED | the started carrier; P15 on every clocked module (§J.1, §L) |
| I.9.3 | pre-loop statements that are not a `let` | `test:tests/reference_leg_measurements.rs::preloop_statement_agrees_on_both_routes`, `ui:fail/preloop_tick`, `test:copper-codegen/tests/unsupported_constructs.rs::a_tick_before_the_module_loop_is_refused_by_r21_on_both_routes` | G-C, PA | CLOSED | (f2), (f3), (f6), R21 (§J.1, §J.12) |
| I.9.4 | observation 0 | `sweep:tests/fixtures/preloop_entry_dut.rs::preloop_probe`, `test:tests/reference_leg_measurements.rs::preloop_statement_agrees_on_both_routes` | G-D/F/G, G-C | CLOSED | §E.1 |
| I.9.5 | state the module did not declare | `sweep:tests/fixtures/shift_register_dut.rs::shift_register`, `sweep:tests/fixtures/counted_delay_dut.rs::counted_delay` | G-D/F/G | CLOSED | `RESET` rows with sentences (III.2.10) |
| I.10.1 | the memory as a variable of H(M) | `sweep:tests/fixtures/pipelined_ram_dut.rs::ram_r2w2`, `test:copper-ref/tests/interp_memory.rs::write_latency_two_commits_at_edge_n_plus_m`, `test:copper-ref/tests/interp_memory.rs::unready_data_is_a_runtime_failure` | G-D/F/G, G-C | OPEN | Phase C follow-up (simulator, front-ends): (a) `ReadMode` not deleted and `L ≥ 1` not checked by the simulator's type (`memory.rs:13`, `:215`); (b) last-staging-wins is realized by the interpreter only — both front-ends still refuse a second staging (rule 6 (a)/(b), I.10.4's line). (c)'s wording corrected this step; (d) agrees on 18 memory modules (§O.1) |
| I.10.2 | read-during-write | `test:tests/write_first_memory_equivalence.rs::read_first_and_write_first_actually_differ`, `sweep:tests/fixtures/write_first_ram_dut.rs::ram_write_first`, `sweep:tests/fixtures/pipelined_ram_dut.rs::ram_r1w2_wf` | G-C, G-D/F/G | CLOSED | the mode the author's; the stale inference comment (`memory.rs:323`) corrected this step |
| I.10.3 | initial contents and the first resumption | `test:tests/preloaded_memory_equivalence.rs::from_fn_preload_sim_matches_transpiled_verilog`, `sweep:tests/fixtures/preloaded_rom_dut.rs::rom_from_contents`, `sweep:examples/cpu/rv32i_cpu_transpilable.rs::rv32i_cpu_transpilable` | G-C, G-D/F/G | CLOSED | the entry (§J.1); probes (iv) as fixtures are I.10.7 (6), not this clause's meaning |
| I.10.4 | the memory rules and their principles | `test:copper-codegen/tests/unsupported_constructs.rs::memory_read_result_before_the_tick_is_rejected` | G-C | OPEN | Phase C follow-up (copper-analysis): D-16 retired rule 6's clauses (a) and (b), but `check_memory_staging` still refuses two stagings of one port in a cycle (`cfg.rs:491`, pins `two_accesses_*`), and (c) does not count `s_pre`'s stagings (I.10.7 (2)) |
| I.10.5 | a received memory, sharing and the bus | `ui:pass/memory_parameter_ok`, `ui:fail/cdc_memory_parameter`, `test:tests/received_memory_abi.rs::received_memory_bus_matches_the_owned_memory_semantics` | PA, G-C | OPEN | Phase C follow-up (types, front-ends): the port set and mode are not in the parameter type and one owner per port is not enforced ("nothing enforces it today"); `WRITE_LAT ≠ 1` refused by the transpiler only (I.10.7 (7), I.13.7 (6)) |
| I.10.6 | memory and the combinational module | `test:copper-ref/tests/interp_memory.rs::memory_binding_outside_s_pre_and_memory_parameter_are_refused` | G-C | OPEN | Phase C follow-up (front-ends): the header rule refusing a `Memory` in a `combinational` module is in neither front-end and has no ui case (I.10.7 (3)); corpus 0 |
| I.11.1 | the combinational module as a function | `sweep:examples/basejump/bsg_mux_one_hot.rs::bsg_mux_one_hot`, `test:copper-ref/tests/interp_comb.rs::no_local_survives_an_evaluation`, `example:bsg_mux_one_hot` | G-D/F/G, G-C, G-B | CLOSED | 52 combinational modules on three legs (§O.2); anchored (BaseJump) |
| I.11.2 | the wire rules | `sweep:tests/fixtures/comb_default_override_dut.rs::cdo_override_then_default`, `test:copper-ref/tests/interp_comb.rs::last_write_wins_and_the_dead_write_is_admitted` | G-D/F/G, G-C | OPEN | copper-codegen: (c) a port written twice at the top level is emitted as two `assign`s (measured 2026-11-07: `assign o = a; assign o = b;`, Verilator MULTIDRIVEN), admitted by both; fix the §10.12 routing into `always_comb` (line II.3 found comb-double-write) |
| I.11.3 | definite assignment | `ui:fail/combinational_never_written`, `ui:fail/combinational_latch`, `test:copper-codegen/tests/comb_definite_assignment_parity.rs::both_front_ends_admit_and_refuse_the_same_rows` | PA, G-C | CLOSED | the never-written port in both (§P.5) |
| I.11.4 | reads, the settle, the cycle refused | `test:tests/simulator_corner_cases.rs::non_convergent_combinational_loop_is_refused_at_spawn`, `test:tests/levelized_differential.rs::convergent_comb_loop_is_refused_at_spawn_by_both_schedulers`, `test:copper-codegen/tests/structural_hierarchy.rs::combinational_cycle_through_a_structural_parent_is_refused` | G-C | CLOSED | §P.3 |
| I.11.5 | composition: the Mealy output | `test:copper-ref/tests/interp_design.rs::the_mealy_pair_reads_the_committed_register_and_the_current_input`, `sweep:tests/fixtures/q1_b_rewrites_dut.rs::q1_b_vr_sink` | G-C, G-D/F/G | CLOSED | D-14 (v)'s anchor is a probe not in the tree |
| I.12.1 | the clock domain as the unit of time | `test:tests/two_clock_pins.rs::interleavings_give_one_trace_per_domain`, `ui:fail/missing_domain_port`, `coreui:fail/unit_domain_refused` | G-C, PA | CLOSED | `()` no domain (§Q.1) |
| I.12.2 | the never-ticked domain | `test:tests/two_clock_pins.rs::never_ticked_startup_one_resumption_agrees_on_all_three_legs` | G-C | CLOSED | `q = 2` on three legs (§P.2); its anchor is the user's (III.2.11 (7)) |
| I.12.3 | the synchronizer module | `ui:fail/sync_one_flop`, `test:tests/cdc_synchronizer_anchor.rs::a_one_flop_synchronizer_is_refused_by_both_front_ends`, `sweep:tests/fixtures/sync_2ff_dut.rs::sync_2ff_concrete` | PA, G-C, G-D/F/G | CLOSED | the two-flop rule (§P.2); `sync_2ff` anchored |
| I.12.4 | what may cross | `coreui:fail/in_port_foreign_domain`, `ui:fail/cdc_crossing`, `ui:fail/combinational_two_domains`, `ui:fail/cdc_memory_parameter` | G-C, PA | OPEN | a fixture (anyone's, not an anchor): (e) — a multi-bit value through a synchronizer is per-bit — has no witness in the tree (corpus 0 multi-bit crossings); (a)–(d) hold in both front-ends (§P.1–§P.2) |
| I.12.5 | the two-clock sequential module | `ui:fail/two_clocks_sequential` | PA | CLOSED | R10 (§P.1) |
| I.13.1 | a design as coroutines over wires | `coreui:fail/out_is_not_clone`, `coreui:fail/regout_is_not_clone`, `test:copper-core/tests/port_and_type_edge_cases.rs::cloned_inputs_observe_the_same_wire` | G-C | CLOSED | — |
| I.13.2 | the settle | `test:tests/levelized_differential.rs::diff_comb_chain`, `test:tests/poll_order_fuzz.rs::combinational_chain_is_poll_order_independent`, `test:copper-ref/tests/interp_design.rs::one_settle_evaluates_in_topological_order` | G-C | CLOSED | (b)'s order realized since steps 2–3 |
| I.13.3 | the evaluation count and the dirty flag | `test:tests/golden_traces.rs::counter_trace_is_frozen`, `test:copper-core/tests/port_and_type_edge_cases.rs::dirty_flag_tracks_changing_writes` | G-C | CLOSED | — |
| I.13.4 | the structural module | `test:tests/two_domain_hierarchy_cdc.rs::independent_reference_matches_sim_under_verilator`, `test:tests/two_domain_hierarchy_cdc.rs::reference_interpreter_design_matches_sim`, `ui:fail/structural_two_drivers`, `ui:fail/structural_undriven_net`, `ui:fail/structural_self_instance` | G-C, G-F, PA | OPEN | Phase C follow-up: (d)'s "every child port connected" row has no fixture (both refuse by an arity error, unpinned); (d)'s/(e)'s one owner per memory needs the port set in the parameter type (I.10.5's line). The rest of (a)–(d) closed (§P.4); the 2:1 design anchored |
| I.13.5 | hierarchy inside a behavioural module | `ui:fail/behavioural_instantiation`, `test:tests/module_composition_hybrid.rs::sequential_modules_compose_in_a_testbench`, `test:copper-codegen/tests/front_end_parity_ui.rs::the_transpiler_refuses_every_shape_the_macro_refuses_with_the_same_message` | PA, G-C | CLOSED | D-23 (1), step 11b (§S): rustc refuses `child(a, b);` (the denied `#[must_use]`, the note I.13.5's message), the transpiler refuses from its registry with the same message; `lower_hardware_call`, the behavioural `submodules` fields, `spawn_child`, `spawn_child!` deleted; corpus 0, SV byte-identical. The bound-instance and body-wire residues are lines II.3 found let-instance (CLOSED, D-24 (1): the second named exception, with the simulator's drop guard, `test:tests/let_instance_d24.rs`) and body-wire |
| I.13.6 | the emitted hierarchy and its ABIs | `test:copper-codegen/tests/structural_hierarchy.rs::hierarchy_co_emits_children_deepest_first`, `test:tests/received_memory_abi.rs::received_memory_bus_matches_the_owned_memory_semantics`, `example:two_domain_hierarchy` | G-C, G-B | CLOSED | — |
| I.14.1 | the type set | `sweep:tests/fixtures/signedness_dut.rs::signed_lt_via_cast`, `test:copper-ref/tests/interp_data.rs::int_wrapping_on_u8_ports`, `sweep:tests/fixtures/data_i14_dut.rs::usize_width` | G-D/F/G, G-C | CLOSED | `usize` 64 bits (§M.3); `Bits<N>`'s N bound is R16's line |
| I.14.2 | values and X | `test:tests/ieee1800_table.rs::copper_matches_the_recorded_ieee_table`, `test:tests/x_propagation_and_reset.rs::x_on_a_control_input_panics`, `sweep:tests/fixtures/data_i14_dut.rs::xw_port` | G-C, G-D/F/G | CLOSED | P6 a standing pin (§M.1); `xw_port`'s X observations are OUTSIDE, counted by G-G |
| I.14.3 | operators and methods | `sweep:tests/fixtures/data_i14_dut.rs::xp_div_nz`, `sweep:tests/fixtures/data_i14_dut.rs::data_methods_seq`, `test:copper-codegen/tests/unsupported_constructs.rs::checked_arithmetic_and_is_valid_are_outside_in_both_front_ends`, `ui:fail/outside_x_comparison`, `test:copper-codegen/tests/unsupported_constructs.rs::the_x_literal_is_emitted_as_a_value_and_refused_in_a_comparison`, `ui:fail/outside_x_pattern` | G-D/F/G, G-C, PA | CLOSED | D-23 (3), step 11b (§S): `==`/`!=` with an X literal OUTSIDE, refused in both by `check_outside_methods` with one message; D-24 (2), step 11c (§T): so is an X pattern. `saturating_*` on a signed or wide operand is lines II.3.3 (e) saturating_add / saturating_sub; a `Logic::X` pattern (I.14.3(e)) is line II.3 found x-pattern (CLOSED, D-24 (2)) |
| I.14.4 | constants | `test:tests/const_evaluators_agree.rs::the_two_constant_evaluators_agree_on_every_corpus_constant`, `sweep:tests/fixtures/data_i14_dut.rs::k_hex_body`, `ui:pass/register_init_rust_constants_ok` | G-J, G-D/F/G, PA | CLOSED | D-21; the narrower transpiler fold is line II.3 found const-call |
| I.14.5 | widths | `sweep:tests/fixtures/data_i14_dut.rs::usize_width`, `sweep:tests/fixtures/data_i14_dut.rs::w1_index`, `test:tests/width_constraint_check.rs::rotate_right_at_a_violating_parameter_is_refused_at_elaboration` | G-D/F/G, G-C | CLOSED | (b)–(d) realized (§M.2, §M.3, §M.5) |
| I.14.6 | memory data and addresses | `ui:fail/memory_port_index`, `sweep:tests/fixtures/mem_address_width_dut.rs::wide_index_into_narrow_addr` | PA, G-D/F/G | CLOSED | the port index a shared rule (§M.5) |

**III.3.3 Part II — II.1 and II.2.**

| Line | Clause | Fixture | Guard | Status | Owner / reason |
|---|---|---|---|---|---|
| II.1.1 | what a shape rule is | `test:copper-codegen/tests/front_end_parity_ui.rs::the_transpiler_refuses_every_shape_the_macro_refuses_with_the_same_message`, `ui:fail/foreign_await` | G-C, PA | CLOSED | every message names its clause (§P.1) |
| II.1.2 | the shape rules, enumerated | `test:copper-analysis/tests/front_end_parity.rs::every_corpus_module_passes_every_shared_rule_in_its_mode` | G-C, PA | CLOSED | the table, row by row below |
| II.1.2 R1 | a non-tick `await` | `ui:fail/foreign_await` | PA | CLOSED | — |
| II.1.2 R2 | a loop path reaching no tick | `ui:fail/tickless_loop`, `ui:fail/partial_tick_path` | PA | CLOSED | — |
| II.1.2 R3 | a zero-time nested cycle | `ui:fail/for_break_before_tick`, `ui:fail/literal_empty_for_boundary`, `ui:pass/livelock_a`, `ui:pass/livelock_b` | PA | CLOSED | narrowed both ways (§P.5); its residual directions are II.2.2's, the constant-empty range I.8.4's line |
| II.1.2 R4 | a tick-free nested `loop`/`while` | `test:copper-codegen/tests/unsupported_constructs.rs::nested_loop_without_a_tick_is_unsupported`, `test:copper-codegen/tests/unsupported_constructs.rs::a_tickless_nested_loop_with_a_continue_is_refused_by_the_shape_rule` | G-C | CLOSED | one function (`shape.rs`) |
| II.1.2 R5 | a run-time tick-free `for` bound | `ui:fail/runtime_for_bound` | PA | CLOSED | §P.1 |
| II.1.2 R6 | recursion through a helper | `ui:pass/recursive_helper_r6`, `test:tests/recursive_helper_r6.rs::the_transpiler_refuses_it_with_r6s_message`, `test:copper-analysis/tests/front_end_parity.rs::the_named_parity_exceptions_are_r6_and_the_let_instance` | PA, G-C | CLOSED | the first named exception (D-22); the second is line II.3 found let-instance (D-24 (1)) |
| II.1.2 R7 | `break` of the module loop; `return` | `ui:fail/module_loop_break`, `ui:fail/module_return` | PA | CLOSED | — |
| II.1.2 R8 | a plain `Out` on a clocked module | `ui:fail/plain_out_on_sequential`, `ui:fail/plain_out_on_synchronizer`, `test:copper-codegen/tests/unsupported_constructs.rs::a_plain_out_on_a_clocked_module_is_refused_by_r8` | PA, G-C | CLOSED | step 3 (§G.4); the row's "both admit" annotated this step |
| II.1.2 R9 | the flag `allow_pretick_alignment` | `ui:fail/retired_pretick_opt_out`, `test:copper-analysis/tests/front_end_parity.rs::no_corpus_module_carries_the_retired_opt_out` | PA, G-C | CLOSED | step 3; annotated this step |
| II.1.2 R10 | a second `Clock` on a sequential module | `ui:fail/two_clocks_sequential` | PA | CLOSED | — |
| II.1.2 R11 | a non-constant register initializer; a held value | `ui:fail/register_init_reads_input`, `ui:fail/held_preloop_read`, `test:copper-codegen/tests/register_init_parity.rs::the_transpiler_refuses_every_initializer_rustc_refuses` | PA, G-C | CLOSED | D-21 |
| II.1.2 R12 | a labelled or valued `break` | `ui:fail/labelled_break`, `ui:fail/valued_break` | PA | CLOSED | a labelled `continue` too (measured 2026-11-07) |
| II.1.2 R13 | a macro in a body | `ui:fail/body_macro` | PA | CLOSED | `matches!`, `const { }` and `vec![…]` as a preload admitted (§P.1) |
| II.1.2 R14 | an input read inside macro tokens | `ui:fail/input_read_in_macro` | PA | CLOSED | — |
| II.1.2 R15 | `/` | `sweep:tests/fixtures/data_i14_dut.rs::xp_div_nz`, `test:copper-codegen/tests/unsupported_constructs.rs::division_operator_is_supported` | G-D/F/G, G-C | CLOSED | no rule: `/` has I.14.3(b)'s meaning and is emitted (§M.2) |
| II.1.2 R16 | a type outside I.14's set | `test:copper-codegen/tests/admissible_calibration.rs::grammar_never_rejects_what_the_transpiler_lowers` | G-C | OPEN | the front-ends: `Bits<N>` with N = 0 or N > 128 is refused by neither (§M.9 item 3), and the rule has no ui/fail case (II.1.4) |
| II.1.2 R17 | `Clock::new` in a body | `ui:fail/clock_construction` | PA | CLOSED | — |
| II.1.2 R18 | a `let` shadowing a port | `ui:fail/param_shadowing` | PA | CLOSED | — |
| II.1.2 R19 | an `In` handle used other than by `.read()` | `ui:fail/sequential_unprotectable_read` | PA | CLOSED | — |
| II.1.2 R20 | the header | `ui:fail/function_typed_unit_return`, `ui:fail/multiple_top_level_loops`, `ui:fail/missing_domain_port` | PA | CLOSED | the messages spell `In<T, D>` / `Out<T, D>` / `RegOut<T, D>` since this step (2 `.stderr` files) |
| II.1.2 R21 | a `tick` before the loop | `ui:fail/preloop_tick`, `test:copper-codegen/tests/unsupported_constructs.rs::a_tick_before_the_module_loop_is_refused_by_r21_on_both_routes` | PA, G-C | CLOSED | step 5 (§J.12); annotated this step |
| II.1.2 R22 | a second driver of a `RegOut` | `coreui:fail/regout_is_not_clone`, `ui:fail/structural_two_drivers` | G-C, PA | CLOSED | by construction |
| II.1.3 | the parity obligation | `test:copper-codegen/tests/front_end_parity_ui.rs::the_transpiler_admits_every_shape_the_macro_admits`, `test:copper-analysis/tests/front_end_parity.rs::every_corpus_module_passes_every_shared_rule_in_its_mode`, `test:copper-analysis/tests/front_end_parity.rs::the_named_parity_exceptions_are_r6_and_the_let_instance` | G-C, PA | OPEN | the front-ends: "any other shape one front-end admits and the other refuses is a break" — re-verified 2026-11-08 (D-24, §T): two named exceptions, R6 and the bound instance; six macro-admits / transpiler-refuses shapes outside the ui battery remain, each a line (II.3 found array-for, const-call, body-wire, tickfree-continue, const-pattern; `saturating_*` under row (e)), plus the toolchain refusals II.3 found u8-const and comb-double-write |
| II.2.1 | what a derived rule is | `ui:fail/tick_first_wait` | PA | CLOSED | — |
| II.2.1 D1 | a nested loop ends at its tick | `ui:fail/tick_first_wait` | PA | CLOSED | — |
| II.2.1 D2 | combinational definite assignment | `ui:fail/combinational_latch`, `ui:fail/combinational_never_written`, `test:copper-codegen/tests/comb_definite_assignment_parity.rs::both_front_ends_admit_and_refuse_the_same_rows` | PA, G-C | CLOSED | §P.5 |
| II.2.1 D3 | `Option`, `?`, panics, `unwrap` | `ui:fail/unsupported_idiom_panic` | PA | CLOSED | shared (D-22) |
| II.2.1 D4 | retired: `multi_write_collapse` | `ui:pass/multi_write_regout_ok`, `gone:multi_write_collapse` | PA | CLOSED | retired as vacuous; D-6 A1's pin stays |
| II.2.2 | the direction of every static predicate | `test:copper-analysis/tests/static_predicate_directions.rs::r3_stays_over_by_path_insensitivity`, `test:copper-analysis/tests/static_predicate_directions.rs::d2_stays_over_by_path_insensitivity`, `test:copper-analysis/tests/static_predicate_directions.rs::two_flop_is_over_by_path_insensitivity`, `test:copper-codegen/tests/admissible_calibration.rs::grammar_never_rejects_what_the_transpiler_lowers` | G-C | CLOSED | every row's direction stated with a witness (P22); the `infer_registers` row's text annotated this step (the head clause is gone) |

**III.3.4 Part II — II.3.**

| Line | Clause | Fixture | Guard | Status | Owner / reason |
|---|---|---|---|---|---|
| II.3.1 | what a lowering limitation is | `test:copper-codegen/tests/unsupported_constructs.rs::a_runtime_empty_counted_for_ending_an_enclosing_for_has_its_own_diagnostic`, `ui:fail/nested_runtime_empty_for` | G-C, PA | CLOSED | the one dated REFUSED-both row today (§J.15) |
| II.3.2 | the sort of the sixteen rules | `ui:fail/plain_out_on_sequential`, `test:copper-codegen/tests/phase_sensitive_checks.rs::no_new_phase_sensitive_check_in_the_transpiler` | PA, G-C | CLOSED | the deletions sized there done (§G.4); rules 7–15 below |
| II.3.3 | the rows | `test:tests/projection_relation.rs::the_projection_relation_holds_on_the_corpus` | G-I | CLOSED | row by row below; no row's shape is outside the relation today |
| II.3.4 | the three rows II.1.2 left unmeasured | `ui:pass/recursive_helper_r6`, `ui:fail/sequential_unprotectable_read`, `ui:fail/preloop_tick` | PA | CLOSED | R6 (D-22), R19, R21 measured and settled |
| II.3.5 | the inventory's column, re-cut | `test:copper-analysis/tests/front_end_parity.rs::every_corpus_module_passes_every_shared_rule_in_its_mode` | G-C | CLOSED | a documentary clause: the pointer line is in `review/INVENTORY_RULES.md`; each placement it points to is the shared rule list |
| II.3.3 (a) | a nested `continue` | `sweep:tests/fixtures/nested_continue_dut.rs::nested_continue`, `sweep:tests/fixtures/nested_continue_dut.rs::nested_continue_match` | G-D/F/G | CLOSED | Phase C step 5 (§J.4), 2026-11-06 |
| II.3.3 (b) | a nested loop with ticks only in arms | `sweep:tests/fixtures/arms_only_loop_dut.rs::arms_only_b1`, `sweep:tests/fixtures/arms_only_loop_dut.rs::arms_only_b2`, `sweep:tests/fixtures/arms_only_loop_dut.rs::arms_only_match` | G-D/F/G | CLOSED | step 5 (§J.5); a `while` of the shape keeps clause 4's message in both |
| II.3.3 (c) | two element writes on an array register | `sweep:tests/fixtures/array_two_writes_dut.rs::arr_two_writes`, `sweep:tests/fixtures/array_two_writes_dut.rs::arr_same_element`, `sweep:tests/fixtures/array_two_writes_dut.rs::arr_comb_read` | G-D/F/G | CLOSED | step 5 (§J.6); the write inside a combinational `for` is its own line |
| II.3.3 (d) | a pre-loop constant the evaluator cannot fold | `sweep:tests/fixtures/const_eval_dut.rs::const_assoc_fn`, `sweep:tests/fixtures/const_eval_dut.rs::const_generic_init`, `sweep:tests/fixtures/const_init_dut.rs::init_const_fn` | G-D/F/G, G-J | CLOSED | D-21 (§J.13, §N) |
| II.3.3 (f1) | a run-time-empty counted `for` | `sweep:tests/fixtures/for_empty_dut.rs::for_empty`, `sweep:tests/fixtures/for_empty_dut.rs::for_empty_start` | G-D/F/G | CLOSED | step 5 (§J.2); D-10 probe (ii) |
| II.3.3 (f2) | pre-loop statements, extraction route | `sweep:tests/fixtures/preloop_entry_dut.rs::preloop_probe` | G-D/F/G | CLOSED | step 5 (§J.1); D-11 probe (i) |
| II.3.3 (f3) | pre-loop statements, linear route | `sweep:tests/fixtures/preloop_entry_dut.rs::preloop_linear` | G-D/F/G | CLOSED | step 5 (§J.1); D-11 probe (i-b) |
| II.3.3 (f4) | the trailing fold at edge 1 | `test:tests/reference_leg_measurements.rs::trailing_fold_at_edge_1_agrees_on_three_legs`, `sweep:tests/fixtures/trailing_statements_dut.rs::trailing_local_into_head` | G-C, G-D/F/G | CLOSED | step 4 (§H.2); D-11's (iv) is a reference-leg test, (iv-c) has no fixture — II.3.7's list, not a divergence |
| II.3.3 (f5) | array-element forwarding | `sweep:tests/fixtures/array_forwarding_dut.rs::f5_sipo`, `sweep:tests/fixtures/array_forwarding_dut.rs::f5_guard`, `sweep:tests/fixtures/array_forwarding_dut.rs::f5_wire`, `sweep:tests/fixtures/array_forwarding_dut.rs::f5_branch_wire` | G-D/F/G | CLOSED | step 5 (§J.3); `f5_sipo` is `q1_c_sipo_block` |
| II.3.3 (f6) | a pre-loop `tick` (R21) | `ui:fail/preloop_tick`, `test:copper-codegen/tests/unsupported_constructs.rs::a_tick_before_the_module_loop_is_refused_by_r21_on_both_routes` | PA, G-C | CLOSED | step 5 (§J.12) — not a limitation: OUTSIDE, refused by both |
| II.3.3 (g) | a hex `from_lit` turbofish | `sweep:tests/fixtures/data_i14_dut.rs::k_hex_body`, `sweep:tests/fixtures/data_i14_dut.rs::k_hex_init`, `sweep:tests/fixtures/data_i14_dut.rs::k_underscore` | G-D/F/G | CLOSED | steps 5 and 8 (§J.7, §M) |
| II.3.3 (e) and_reduce | `Bits::and_reduce` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_comb` | G-D/F/G | CLOSED | step 5 (§J.8) |
| II.3.3 (e) or_reduce | `Bits::or_reduce` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_comb`, `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_seq` | G-D/F/G | CLOSED | §J.8 |
| II.3.3 (e) xor_reduce | `Bits::xor_reduce` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_comb` | G-D/F/G | CLOSED | §J.8 |
| II.3.3 (e) nand_reduce | `Bits::nand_reduce` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_comb`, `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_seq` | G-D/F/G | CLOSED | §J.8 |
| II.3.3 (e) nor_reduce | `Bits::nor_reduce` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_comb` | G-D/F/G | CLOSED | §J.8 |
| II.3.3 (e) xnor_reduce | `Bits::xnor_reduce` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_comb`, `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_seq` | G-D/F/G | CLOSED | §J.8 |
| II.3.3 (e) concat | `Bits::concat` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_comb`, `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_seq` | G-D/F/G | CLOSED | §J.14 |
| II.3.3 (e) part_select | `Bits::part_select` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_comb`, `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_seq` | G-D/F/G | CLOSED | §J.14 |
| II.3.3 (e) replicate | `Bits::replicate` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_comb` | G-D/F/G | CLOSED | §J.14 |
| II.3.3 (e) sign_extend | `Bits::sign_extend` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_comb` | G-D/F/G | CLOSED | §J.8 |
| II.3.3 (e) zero_extend | `Bits::zero_extend` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_comb` | G-D/F/G | CLOSED | §J.8 |
| II.3.3 (e) truncate | `Bits::truncate` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_comb` | G-D/F/G | CLOSED | §J.8 |
| II.3.3 (e) shift_left | `Bits::shift_left` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_comb` | G-D/F/G | CLOSED | §J.8 |
| II.3.3 (e) shift_right | `Bits::shift_right` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_comb`, `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_seq` | G-D/F/G | CLOSED | §J.8 |
| II.3.3 (e) get | `Bits::get` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_comb`, `sweep:tests/fixtures/data_i14_dut.rs::const_index` | G-D/F/G | CLOSED | §J.8 |
| II.3.3 (e) set | `Bits::set` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_seq` | G-D/F/G | CLOSED | §J.14 |
| II.3.3 (e) mux | `Bits::mux` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_comb` | G-D/F/G | CLOSED | §J.14 |
| II.3.3 (e) with_lsb | `Bits::with_lsb` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_comb` | G-D/F/G | CLOSED | §J.14 |
| II.3.3 (e) with_msb | `Bits::with_msb` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_comb` | G-D/F/G | CLOSED | §J.14 |
| II.3.3 (e) is_valid | `is_valid` — OUTSIDE (I.14.3(d)) | `ui:fail/outside_is_valid`, `test:copper-codegen/tests/unsupported_constructs.rs::checked_arithmetic_and_is_valid_are_outside_in_both_front_ends` | PA, G-C | CLOSED | refused by both with one message (§M.4); step 5's `1` lowering retired |
| II.3.3 (e) lt_logic | `lt_logic` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_comb` | G-D/F/G | CLOSED | §J.14 |
| II.3.3 (e) gt_logic | `gt_logic` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_comb` | G-D/F/G | CLOSED | §J.14 |
| II.3.3 (e) le_logic | `le_logic` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_comb` | G-D/F/G | CLOSED | §J.14 |
| II.3.3 (e) ge_logic | `ge_logic` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_comb` | G-D/F/G | CLOSED | §J.14 |
| II.3.3 (e) as_array | `as_array` | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_comb`, `sweep:examples/basejump/bsg_adder_one_hot.rs::bsg_adder_one_hot` | G-D/F/G | CLOSED | §J.14 |
| II.3.3 (e) saturating_add | `saturating_add` — IN | `sweep:tests/fixtures/data_i14_dut.rs::data_methods_seq` | G-D/F/G | OPEN | copper-codegen: unsigned ≤ 64 bits lowered and swept (§M.4); a signed or wider operand is refused by the transpiler only (§M.9 4(b)); fix: a signed clamp form |
| II.3.3 (e) saturating_sub | `saturating_sub` — IN | `sweep:tests/fixtures/data_i14_dut.rs::data_methods_seq` | G-D/F/G | OPEN | as `saturating_add` |
| II.3.3 (e) checked_add | `checked_add` — OUTSIDE | `ui:fail/outside_checked_add`, `test:copper-codegen/tests/unsupported_constructs.rs::checked_arithmetic_and_is_valid_are_outside_in_both_front_ends` | PA, G-C | CLOSED | refused by both (§M.4) |
| II.3.3 (e) checked_sub | `checked_sub` — OUTSIDE | `test:copper-analysis/tests/front_end_parity.rs::the_step_8_data_rules_refuse_their_witnesses_and_admit_their_twins` | G-C | CLOSED | one rule over every `checked_*` name (`copper-analysis/src/data.rs`); a `checked_sub` witness added to the test this step |
| II.3.3 (e) eq_logic | `eq_logic` (owed by the row) | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_comb`, `test:tests/ieee1800_table.rs::copper_matches_the_recorded_ieee_table` | G-D/F/G, G-C | CLOSED | IEEE-exact (§M.1) |
| II.3.3 (e) ne_logic | `ne_logic` (owed by the row) | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_rest_comb`, `test:tests/ieee1800_table.rs::copper_matches_the_recorded_ieee_table` | G-D/F/G, G-C | CLOSED | §M.1 |
| II.3.3 (e) as_array_mut | `as_array_mut` (owed by the row) | `sweep:tests/fixtures/data_i14_dut.rs::data_methods_seq` | G-D/F/G | CLOSED | §M.4 |
| II.3.3 (e) from_array | `from_array` (owed by the row) | `sweep:examples/combinational/ripple_carry_adder.rs::ripple_carry_adder` | G-D/F/G | CLOSED | identity since step 5 |
| II.3.3 (e) from_slice | `from_slice` (owed by the row) | `sweep:tests/fixtures/bits_methods_dut.rs::bits_methods_comb`, `sweep:tests/fixtures/data_i14_dut.rs::const_index` | G-D/F/G | CLOSED | identity since step 5 |
| II.3.3 (e) from | the `From` conversions (owed by the row) | `sweep:tests/fixtures/data_i14_dut.rs::data_methods_seq`, `sweep:tests/fixtures/data_i14_dut.rs::w1_from` | G-D/F/G | CLOSED | `Bits::from(Logic)`, `Logic::from_bool` (§M.2, §M.4) |
| II.3.3 (e) all_ones | `all_ones` (owed by the row) | `sweep:tests/fixtures/data_i14_dut.rs::k_ones_nowidth`, `sweep:tests/fixtures/data_i14_dut.rs::k_ones_width`, `sweep:tests/fixtures/data_i14_dut.rs::k_ones_init` | G-D/F/G | CLOSED | every spelling (§M.9 item 2) |
| II.3 found array-for | an element write to an array register inside a combinational `for` (2026-11-06, §J.10 item 6 (i)) | `test:copper-codegen/tests/unsupported_constructs.rs::an_array_register_element_write_inside_a_combinational_for_is_refused` | G-C | OPEN | copper-codegen (or both front-ends): REFUSED-one — the transpiler refuses, the macro admits; fix direction: unroll the `for`'s writes into the write-port form of (c), or refuse in both with one message |
| II.3.3 (h) | a `continue` of a tick-bearing `for` (found 2026-11-06, §J.10 item 4; a row since D-23, 2026-11-08) | `ui:fail/ticking_for_continue`, `test:copper-codegen/tests/unsupported_constructs.rs::a_continue_of_a_tick_bearing_for_is_refused_with_the_shared_message` | PA, G-C | CLOSED | REFUSED-both, a dated limitation (D-23 (2), step 11b, §S): one message (`front_end::check_ticking_for_continue`); fix direction the lowering (increment, then test); corpus 0 |
| II.3 found const-call | a constant the transpiler's fold is narrower than rustc on (2026-11-07, §N.5 item 4): a `const` method call (`K.wrapping_add(1)`), a tuple-struct constructor, a `static` read | `test:copper-codegen/tests/register_init_parity.rs::the_transpiler_refuses_every_initializer_rustc_refuses` | G-C | OPEN | copper-analysis (`ConstEnv`): REFUSED-one, corpus 0 — rustc admits each in `const { }`, the transpiler refuses with a lowering error (never a dropped value); fix: widen the fold, II.2.2's R11 row |
| II.3 found u8-const | a `u8` file const in a body expression (2026-11-07, §N.5 item 1) | `sweep:tests/fixtures/const_init_dut.rs::init_file_const` | G-D/F/G | OPEN | copper-codegen: emitted as a 32-bit `localparam int`, which Verilator `-Wall` refuses (WIDTHEXPAND) — REFUSED-by-the-toolchain, admitted by both front-ends; the fixture names the working spelling; fix: a sized `localparam` |
| II.3 found comb-double-write | a combinational port written twice at the top level (2026-11-05, I.11.2(c), D-14; re-measured 2026-11-07) | `sweep:tests/fixtures/comb_default_override_dut.rs::cdo_default_then_override` | G-D/F/G | OPEN | copper-codegen: two `assign`s, Verilator MULTIDRIVEN; the fixture names the `if`-guarded override that is routed; fix: the §10.12 routing into `always_comb` for any port written more than once |
| II.3 found preload-ok | `ui/pass/register_init_constants_ok`, admitted by the macro, refused by the transpiler (2026-11-07, §N.5 item 2; re-measured 2026-11-07): a `match` as an expression with no wildcard arm, and element writes on a `[Logic; 4]` register refused as a latch (row (c)'s packed-vector note) | `test:copper-codegen/tests/front_end_parity_ui.rs::the_transpiler_admits_every_shape_the_macro_admits` | G-C, PA | OPEN | copper-codegen: REFUSED-one, the one named lowering-limit row of `front_end_parity_ui.rs`; fix: lower an exhaustive `match` expression, and an element write on a vector of one-bit elements |
| II.3 found nested-runtime-empty | a run-time-bounded counted `for` ending an enclosing tick-bearing `for` (2026-11-07, §J.15) | `ui:fail/nested_runtime_empty_for`, `test:copper-codegen/tests/unsupported_constructs.rs::a_runtime_empty_counted_for_ending_an_enclosing_for_has_its_own_diagnostic` | PA, G-C | CLOSED | REFUSED-both, a dated limitation (fix direction: unroll the enclosing loop's zero-time iterations) — closed as a refusal |
| II.3 found usize-width | `x.as_usize()` arithmetic at the receiver's width (2026-11-06, §J.10 item 8) | `sweep:tests/fixtures/data_i14_dut.rs::usize_width`, `sweep:tests/fixtures/data_i14_dut.rs::usize_annot` | G-D/F/G | CLOSED | step 8 (§M.3): `usize` 64 bits |
| II.3 found let-instance | a `#[hardware]` call bound by a `let` in a behavioural body — `let _ = child(..)`, `let m = child(..)` (2026-11-08, §S) | `ui:pass/let_instance_d24`, `test:copper-codegen/tests/front_end_parity_ui.rs::the_transpiler_admits_every_shape_the_macro_admits`, `test:copper-analysis/tests/front_end_parity.rs::the_named_parity_exceptions_are_r6_and_the_let_instance`, `test:tests/let_instance_d24.rs::the_transpiler_refuses_both_with_i_13_5s_message`, `test:tests/let_instance_d24.rs::let_wildcard_instance_panics_at_the_first_resumption`, `test:tests/let_instance_d24.rs::let_named_instance_panics_when_the_parent_is_dropped`, `test:tests/let_instance_d24.rs::the_guard_is_silent_while_another_panic_unwinds` | PA, G-C | CLOSED | II.1.3's second named exception (D-24 (1), step 11c, §T): rustc's `unused_must_use` sees an unused *statement* only, so the macro admits the binding; the transpiler refuses with I.13.5's message; the simulator's `HardwareModule` drop guard panics (`UNSTARTED_MODULE_MESSAGE`) when the child is dropped unstarted — no silent non-execution; corpus 0 |
| II.3 found body-wire | a `wire`/`registered_wire` call in a behavioural body (D-18 `b1b`, re-measured 2026-11-08, §S) | — (probe `wirebody` of §S, scratchpad) | G-C | OPEN | the front-ends: I.13.5 puts a net outside a behavioural body; the macro admits the call (a plain fn), the transpiler refuses with a width message ("cannot infer bit width"), not I.13.5's; fix: a shared rule over the two constructors' paths, one message |
| II.3 found x-pattern | a `Logic::X` pattern in a `match` (2026-11-08, §S) | `ui:fail/outside_x_pattern`, `test:copper-codegen/tests/unsupported_constructs.rs::a_pattern_matching_the_x_literal_is_refused_with_the_shared_message`, `test:copper-analysis/tests/front_end_parity.rs::the_step_8_data_rules_refuse_their_witnesses_and_admit_their_twins` | PA, G-C | CLOSED | OUTSIDE (D-24 (2), step 11c, §T): `check_outside_methods` refuses a `Logic::X` pattern in any position and a body `const` whose value is X, in both front-ends, with `X_PATTERN_MESSAGE` citing I.14.3(d); I.14.3(e): an X scrutinee matches `_` only; corpus 0 |
| II.3 found tickfree-continue | a `continue` of a tick-free counted `for` (2026-11-08, §S) | — (probe `cfor_tickfree_ok` of §S, scratchpad) | G-C | OPEN | copper-codegen: REFUSED-one — IN (I.2.4, the unrolled `for`; Rust's `continue`), the macro admits, the transpiler refuses with a message that blames the module loop ("`continue` is not supported in hardware; the module's top-level `loop` runs forever"); fix: lower it in the unroller (the rest of the iteration under the negated condition) |
| II.3 found const-pattern | a pattern naming a non-integer `const` — `const ONE: Logic = Logic::One;` (file-level or in the body), `const UB: Bits<2> = …` — `match v { ONE => … }` (2026-11-08, §T) | — (probes `x7`–`x9`, `x11` of §T, scratchpad) | G-C | OPEN | copper-codegen: REFUSED-one — the macro admits (a const pattern, rustc's), the transpiler reads the uppercase identifier as an enum variant and refuses with "tuple-pattern match lowering is not yet implemented (M2)"; an integer `const` (`K => …`) transpiles; no text emitted; fix: lower a const pattern by its value; a file-level `const` whose value is X stays outside the `ItemFn` the shared rule holds, so how it is refused is a decision for the change that lands the lowering |

**III.3.5 The retired rules (II.3.2 rules 7–15).** Closed when the function is gone from every crate
(`gone:`) and the shape it reasoned about is unexpressible (R8) or vacuous.

| Line | Clause | Fixture | Guard | Status | Owner / reason |
|---|---|---|---|---|---|
| retired 7 | `memory_result_drives_plain_out` — vacuous | `gone:memory_result_drives_plain_out`, `gone:reject_memory_driven_comb_outputs`, `ui:fail/plain_out_on_sequential`, `test:copper-codegen/tests/phase_sensitive_checks.rs::no_new_phase_sensitive_check_in_the_transpiler` | PA, G-C | CLOSED | step 3 (§G.4); I.10.6's header rule is I.10.6's line |
| retired 8 | `multi_write_collapse` | `gone:multi_write_collapse`, `ui:pass/multi_write_regout_ok` | PA | CLOSED | II.2.1 D4 |
| retired 9 | `unprotected_pretick_out_write` | `gone:unprotected_pretick_out_write`, `ui:fail/plain_out_on_sequential` | PA | CLOSED | D-8; step 3 |
| retired 10 | `pretick_out_write_before_update` | `gone:pretick_out_write_before_update`, `ui:fail/plain_out_on_sequential` | PA | CLOSED | D-8; step 3 |
| retired 11 | `trailing_regout_reads_input` | `gone:trailing_regout_reads_input`, `test:tests/reference_leg_measurements.rs::regout_trailing_single_tick_agrees_on_three_legs` | G-C | CLOSED | vacuous under one job; the trailing read sees `x_{N+1}` on three legs |
| retired 12 | `trailing_out_reads_input` | `gone:trailing_out_reads_input`, `ui:fail/plain_out_on_sequential` | PA | CLOSED | D-8 |
| retired 13 | `linear_out_write_anchor_mismatch` | `gone:linear_out_write_anchor_mismatch`, `ui:fail/plain_out_on_sequential` | PA | CLOSED | D-8; forms F1–F5 deleted (§H.1) |
| retired 14 | `multi_phase_out_write` | `gone:multi_phase_out_write`, `ui:fail/plain_out_on_sequential` | PA | CLOSED | D-8 |
| retired 15 | `out_first_driven_late` | `gone:out_first_driven_late`, `ui:fail/plain_out_on_sequential` | PA | CLOSED | D-8; I.11's class vacuous (D-14) |

**III.3.6 Counts.** Measured by G-K on the bare regression of `review/MEASUREMENTS.md` §T.4
(2026-11-08, on `7589204`, Phase C step 11c, REGRESSION OK): **187 lines — 166 CLOSED, 21 OPEN**; 168
required by the SPEC (91 numbered clauses, 22 II.1.2 rules, 4 II.2.1 rules, 12 II.3.3 rows, 28 row-(e)
methods, 9 retired rules) and 19 beyond it (the 7 methods row (e) owes, 12 II.3 rows found in Phase C).
By part: Part I 82 lines, 73 CLOSED, 9 OPEN; II.1–II.2 31 lines, 29 CLOSED, 2 OPEN (R16, II.1.3); II.3
65 lines, 55 CLOSED, 10 OPEN (`saturating_add`, `saturating_sub`, and eight found rows); the 9 retired
rules CLOSED. Fixture tokens on CLOSED lines: 137 `sweep`, 116 `test`, 89 `ui`, 5 `coreui`, 3
`example`, 11 `gone`; G-K's run check found all 156 run checks satisfied and the 3 examples PASS. No
OPEN line waits on a decision; each names the component that owes it; none is a divergence the sweep
reaches (G-G 0 DISAGREE). (Step 11b's counts, on `272159e`: 186 lines — 164 CLOSED, 22 OPEN; D-24
closed two lines, `II.3 found let-instance` and `x-pattern`, and one found line opened,
`II.3 found const-pattern`, §T.3.)

**III.3.7 Implementation map.** `tests/conformance_ledger.rs` (the parser, the completeness and
token checks, `COPPER_LEDGER_REPORT`); `tools/regression.sh` G-K (the `REQUIRE` lines against the
run's log and the examples' status); `build.rs`'s `corpus_manifest.rs` (the swept names) and `SKIP`.

**III.3.8 Carried list.** Every OPEN line above, with its owner — the Phase C follow-up list; the
decisions of §R.6 are the user's. A line that closes moves to CLOSED with its fixture, its OPEN
reason kept in the commit that closes it; a new clause, row or rule of Parts I–II gains a line in
the same change (G-K fails otherwise). The II.3 found lines are owed to II.3.3 as dated rows (the
coordinator's edit).

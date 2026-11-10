# How the prior-art HDLs specify their semantics — survey, and what it means for Copper

> **Historical (2026-11-02), pre-D-8.** The survey of prior formal work still holds. The paragraphs
> stating Copper's position describe the language before D-8 and Phase C (a plain `Out` on clocked
> bodies, the read barrier); SPEC.md, normative since 2026-11-09 (D-31), states Copper's semantics
> now. `review/DOCS_AUDIT.md` lists the stale claims with line numbers.

**Status: survey complete, recommendation pending the user's decisions (2026-11-02).**
Companion: `FORMAL_SEMANTICS.md` (the DRAFT kernel, denotation and conformance ledger
this survey recommends). Per-paper reading notes, one file per document with
section/figure locators and transcribed rules, are in `design_docs/prior_art/`.

## 0. Purpose and method

The eighteen documents in `paper/references/` were read for one question: **how does
each specify the meaning of its language, and how does it relate that meaning to the
hardware it generates?** For every document the same nine questions were answered
(object given a semantics; style; time model; what makes something state; intra-cycle
ordering; properties stated and how established; relation to generated hardware; exact
locations of rules; what transfers to Copper and what does not). The inference rules
of the three papers that have any (Esterel, Filament, Anvil) were checked against the
typeset PDF pages, not the OCR text.

Copper's own position, for reference: `SYNCHRONOUS_SEMANTICS.md` declares a
**denotation normative** (one value per signal per cycle, the write-window invariant),
names the **simulator and the transpiler as two implementations with obligations**, and
states every rule in **prose plus measured evidence**. There is no grammar, no
judgment, no inference rule, and no stated proposition whose proof obligation is named.
That is the gap this survey is for.

## 1. The comparison in one table

| Document | Object formalized | Style | Time model | State rule | Intra-cycle order | Properties, and how established | Sim ↔ synth relation |
|---|---|---|---|---|---|---|---|
| **Esterel** (SCP 1992) | 14-statement kernel; plain Esterel by macro-expansion | Behavioral big-step per reaction (labelled SOS, residue = derivative, fixpoint for local signals) **and** execution microsteps with a 4-status signal memory; automata by derivatives | Instant = input event; one status + one value per signal per instant (sharing law); persistence law for absent signals | Program term is the state: `var X=[v]` boxed into the residue | Variables sequential; signals order-independent, combined by `comb`; cyclic reads **rejected** (not iterated) | Thm 1 strong confluence, Cor 2 global confluence, **Thm 3 execution ≡ behavioral and deterministic**, Thm 4 finite derivatives; proofs in a thesis, none mechanized | Automata correct by construction; circuits not in this paper |
| **Filament** (PLDI 2023) | 4-command core calculus (one event per component) | Denotational **log transformer** `⟦c⟧ : (T → R × W) → (T → R × W)`, no values; separation-logic type system | Events `t, t+n` = cycles; half-open availability intervals; delay = initiation interval | None in the calculus: `Register`/`Delay`/`Prev` are externs with latency signatures | None: `c₁ • c₂` unions logs; duplicate write = conflict; `R ⊆ W` | Def 6.1 well-formed, Def 6.2 safe pipelining (never proven); **Thm 6.3 well-typed ⇒ well-formed**, half-page paper induction, invocation rule only sketched | Low Filament → Calyx → Verilog, no correctness statement; cocotb harness, differential fuzz of one adder |
| **Anvil** (ASPLOS 2026) | Core calculus (Fig. 7) + CSP-style program layer (Fig. 10) | Big-step relational **execution logs** `Γ;I,M ⊢ t ⇝ ⟨α₀…α_k⟩ ◁ v`, one unordered action set per cycle, valueless, nondeterministic; event-graph type system | Explicit cycle index; `t₁ ⇒ t₂` overlaps the boundary cycle; `≤_G` = "for all timestamp functions" | Registers declared; `r := t` logs `RegMut` now, value next cycle (E/T-RegAssign) | Actions in a cycle are a set; no forwarding; values immutable; `set` nonblocking by fiat | Def C.15 timing safety; Lemma C.19 **two unrolled iterations suffice**; Thm C.20 well-typed ⇒ safe; paper proofs, sketched; checker uses unstated "sound approximations" | SV lowering has no correctness statement; unit tests vs SV baselines |
| **Arch** (arXiv 2026) | None (grammar asserted LL(1), no rules, no IR) | Prose per construct; "one construct always produces the same SV structure"; a second C++ sim model | Clocked RTL, bounded settle depth 1–2, comb loops rejected | Keywords `reg/comb/let`, explicit reset; `thread` lowers waits to FSM states | Not asked: Listing 9 is an un-analysed instance of Copper's opening/closing question | **None stated**; uses "structural theorem", "deadlock-free by construction", "corollary of the lowering equivalence proof" with no theorem or proof | Meaning = two implementations agree: `arch sim` vs Verilator, 441/443 |
| **CλaSH** (DSD 2010) | Haskell functions `f : (S, I) → (S, O)` | Denotational, inherited: `run f s (i:is) = o : run f s' is` (eq. 11) | One list element per cycle; implicit clock | `State` wrapper in the type | None: no sequencing inside a cycle | None proven (§VII: normalization "ad-hoc", termination unproven) | Emits a testbench to compare VHDL vs Haskell; no result reported |
| **Bluespec** (MEMOCODE 2004 slides) | Guarded atomic rules | **Term rewriting**: "pick any one enabled rule, perform its action", clockless, nondeterministic | None in the semantics; implementation fires rules per cycle | Registers explicit; methods | No intra-rule sequencing; read-after-write in one action forbidden | Refinement criterion (slide 15): a cycle's parallel firing must equal some legal one-at-a-time sequence; bisimulation proofs "not automated" | Compiler generates the scheduler; correctness criterion stated, not proved |
| **Chisel** (DAC 2012) | None; node graph | By translation, by example | Implicit clock; C++ sim "clock-low / clock-high" | `Reg` = one-cycle delay; `when` + **last-connect**; Fig. 1 mux chain seeded with old value (hold) | Static driver priority, no forwarding | None; FMA vs SoftFloat, OS boot on VCS and C++ (de facto differential, never claimed) | Two backends run one testbench |
| **Spade** (FPL 2022) | None | By translation + type system (linear `&mut` wires: set exactly once per cycle) | `reg;` inserts a pipeline stage; stage-indexed names `stage(-2).x` | `reg(clk) x reset(rst: v) = e`; all variables immutable | None; definite single assignment by typing | Stated, not proven; no evaluation | **The generated SV is normative**; no own simulator |
| **PyMTL3** (IEEE Micro 2020) | None; pluggable | Operational **by scheduler**, "each pass implements different modeling semantics" | Cycle-stepped; `@update_ff` vs `@update`; static DAG pass vs event-driven pass | Block kind decides flop vs comb | Between-block ordering inferred from reads/writes; every read is of the settled value | None formal; PyH2 property tests; translated SV co-simulated under the same TB (Fig. 1a) | Differential co-simulation as a workflow step, no numbers |
| **Golden Age** (SNAPL 2019 survey) | — | Classifies HDLs by state-encoding mechanism; names the variable ≠ register gap and the blocking/nonblocking problem | — | Recommends qualifiers and "explicitly model the synchronized temporal update semantics of a register variable in the evaluation semantics" | — | None | Recommends a formally specified core structural DSL as common target |
| **MyHDL** (manual 0.11) | None; Python kernel | Prose + analogy ("`.next` is the Verilog nonblocking assignment"); conversion = **subset restriction** | `yield` = VHDL `wait`; one suspension at the loop head in synthesizable code | All state is a `Signal`; no local crosses a yield | Last-write-wins, hold when unwritten | None; `verify()` diffs print logs; documents a time-0 sim/HDL divergence and tolerates it | "Equivalent" Verilog, undefined |
| **Prost!** (LATTE 2026) | None | Prose: "advance the coroutine until its next suspension point at each posedge"; `advance : State × In → State + Ret` | `wait` = next cycle; everything closing-anchored | **Every local is a register**; all outputs `.next` (= `RegOut`) | None: segment = comb next-state function | None; loop rule a heuristic ("at least one wait, runs at least once") | No simulator, no check |
| **RHDL / RustHDL** (LATTE 2024/25) | None | Two-sentence mental model; typed SSA IRs | One `update` per cycle; fixpoint sim with iteration cap | Explicit `DFF<T>`; `.next`/`.val` | Last write to `.next` wins; feedback must cross a register | None; "deployed"; CDC by **marker types** (ancestor of `Clock<D>`) | Verilator *rejected* RustHDL's Verilog (§4) |
| **Cummings** (SNUG 2000) | Verilog | IEEE 1364 **stratified event queue** restated; 26 examples; 8 coding guidelines | Time step with active / inactive / NBA regions, unordered active events | `<=` evaluate-at-start, update-at-end | Race = same step, different result under a permitted reorder (§2.0) | "90–100% of races eliminated", case analysis | Synthesis reads structure, simulation reads order; style rules restore agreement |
| **SystemC synth subset** 1.4.7 | A **subset** of C++/IEEE 1666 | Normative **shall/should/may**; every construct Supported / Ignored / Not Supported / with Restrictions | `wait()` = the one clock edge; `wait(N)` static; reset = everything before the first statically-placed `wait()` | Reset values constant; registers "values used one or more cycles after generation" (informative) | Single writer; variables private to a process | Portability only; **explicitly does not promise cycle exactness** (§3, A.2) | "may have different sequential timed behavior" |
| **Verismith** (FPGA 2020) | Deterministic Verilog fragment, by construction | Testing: random generation + formal equivalence + reduction | `always @(posedge clk)`; miter asserts per edge; internals concatenated into the output | Declared with initializers (FPGA honours them) | No multiple drivers, dependency-ordered assignments | Correct = netlist ≡ design over all inputs; sound because deterministic; 11 bugs / 18 000 CPU-h | Equivalence checking; simulation-based differential testing as fallback |
| **BaseJump STL** (DAC 2018) | None; library | Principles, handshake taxonomy (`rv->&`, `r->v`, `v->r`), SRAM `write_then_read_same_addr_p` | Single clock, explicit reset | — | — | Silicon (four chips) and unit tests | Reference implementation whose Verilog *is* the spec |

## 2. Six ways the field specifies a hardware language

Reading across the table, the documents fall into six patterns. Copper today sits in the
last two and aspires to the first.

1. **Operational rules plus theorems relating two semantics** — Esterel alone. A
   big-step relation per reaction defines meaning; an effective microstep machine
   implements it; Theorem 3 says the machine computes the relation, deterministically.
   This is the only document in the set whose main result has the *shape* Copper needs:
   "the effective thing computes the declared thing".
2. **Valueless timing/resource logs with a type-system soundness theorem** — Filament
   and Anvil. The denotation records *which* ports or registers are touched in *which*
   cycle, never what value they carry. The theorem is "well-typed ⇒ no conflict /
   timing-safe". Functional meaning is delegated, unproven, to the compiler. Neither
   paper could state Copper's correctness bar, because neither has values.
3. **Denotation inherited from the host language** — CλaSH (`run` over streams), and
   in a weaker sense MyHDL and PyMTL3 ("the kernel is the semantics"). Honest and cheap,
   but it makes the host's evaluation order part of the meaning, which is the
   blocking/nonblocking problem the Golden Age survey names.
4. **Semantics by translation: the generated Verilog is normative** — Spade (explicitly),
   Chisel, Arch, RHDL, Prost. Nothing can then be *wrong* about the hardware, and a
   second implementation (a simulator) can only be checked against it, never the other
   way round. Arch is the cautionary case: a paper that defines meaning as "two
   implementations agree" and then uses the words "theorem" and "proof" for that.
5. **Normative subset standards and coding rules** — the SystemC synthesizable subset,
   Cummings's guidelines, MyHDL's conversion chapter. These are the best models for
   *how to write admissibility rules*: shall-language, a four-way construct
   classification, a named act ("synthesis refinement") for closing an
   implementation-defined hole. SystemC is also the clearest admission that a coroutine
   HDL can be synthesizable without being cycle-exact.
6. **Extensional, testing-based correctness** — Verismith (random programs + formal
   equivalence), PyMTL3's co-simulation, Arch's 441/443. Verismith is the one that
   states its *precondition*: the fragment is deterministic, single-driver, loop-free,
   and has known power-on values, so any disagreement is a bug by definition. That
   precondition is a sentence Copper's definitions should contain.

## 3. Per-document findings that matter for Copper

Each item names the one device worth taking, the one thing to avoid, and where to look.

- **Esterel.** Take the *form*: labelled big-step per cycle on a kernel, with a
  **termination flag** (`b ∈ {tt, ff}`: did the statement finish inside the instant) and
  a **residue** (the derivative, which carries the state in the term — `var X=[v]`).
  The sequence rule's two cases are exactly "the segment continues past `s₁`" and "the
  segment ends inside `s₁`"; the compile-time rule "a loop body cannot terminate
  instantly" is `check_reachability`; the persistence law `S⁻(v) ∈ Ê_{n+1} ⇒ Ê_n(S) = v`
  is "an unwritten `Out` holds" in one line; the **potential** π̂ (signals a residue can
  still emit) is the device behind every pre-tick rule ("a port a suffix can still
  write"). Avoid importing the signal/`||`/trap machinery: Copper locals are Esterel
  *variables*, sequential, not broadcast signals, so Copper's forwarding is the sequential
  half of the sharing law, not the law. **Esterel has one observation point per instant;
  Copper has two** (pre-edge and post-edge settle), and its whole alignment family lives
  in that difference, which Esterel's apparatus cannot express. Locations: §6.3 transition
  form, §6.5.6 sequence, §6.5.14–15 var/signal, §8.1 statuses, §8.5 Theorem 3.
- **Anvil.** Take the **carrier**: a log `⟨α₀, α₁, …⟩` indexed by cycle, one *unordered*
  set of actions per cycle — the exact statement that program order inside a cycle is not
  part of the meaning. Take E/T-RegAssign (`RegMut` now, value at `e_c ▷ #1`) as the
  `RegOut` rule in inference-rule form; take **loop = unrolling** (C-Extend) with Lemma
  C.19 "two iterations suffice" as the template for proving a back-edge rule once; take
  the **"for all timestamp functions"** quantifier (Def. C.11) as the name for what
  Copper's phase-sensitive rules implicitly assert over every schedule of the `Cfg`.
  Avoid its register story: values are immutable and `set` is nonblocking by fiat, so a
  sequential body writing a plain `Out` cannot be written — Anvil avoids the alignment
  family by lacking the construct, and porting its rule would make every `Out` a `RegOut`
  (the option Copper rejected 2026-10-21). Locations: App. C.2 rules, C.3 event graph,
  C.4 Def. C.15, Lemma C.19.
- **Filament.** Take the interval vocabulary: a segment between tick `i` and tick `i+1`
  is "available in `[G+i, G+i+1)`"; `Register : in@[G,G+1) → out@[G+1,L)` *is* "a
  `RegOut` write takes effect at the first edge after it executes"; `Prev` (same-cycle
  read of the previous value) is Copper's opening-anchored read of committed state; **safe
  pipelining as the union of two shifted copies of the log** is a clean way to define the
  loop back edge without infinite unrolling. Avoid `R ⊆ W` as a definedness rule: in
  Filament an unwritten port is *invalid*, in Copper it *holds*. Locations: Fig. 7/9,
  App. A.2 worked logs, Def. 6.1–6.2, Thm 6.3.
- **Arch.** A mirror, not a source. It defines meaning as "`arch sim` agrees with
  Verilator" — Copper's current bar — and reads as a paper without a written reference.
  Its Listing 9 (`aw_valid = 1; … wait until aw_ready; aw_valid = 0`) is a worked
  instance of the opening/closing question that it never asks. Borrow only the words
  "register commit barrier" and "DAG level barrier" and the idea of a *stated bound* on
  settle passes.
- **CλaSH.** `run f s (i:is) = o : run f s' is` is the smallest possible top-level
  denotation and is the shape of Copper's module-level statement (body between ticks =
  the Mealy step). Copper must add what CλaSH declares in the type: that the
  liveness-inferred register set *is* `S`.
- **Bluespec.** Copy the three-part structure — abstract semantics, lowering, a stated
  correctness criterion relating them (slide 15) — noting Copper's criterion is stronger
  (trace equality, no existential over schedules). BSV forbids read-after-write inside one
  atomic action where Copper admits it; that is exactly the D1 / write-before-update class.
- **Chisel.** Fig. 1 (mux chain seeded with the old value, OR-ed enable) is the
  canonical realization of "`Out` holds when unwritten". Last-connect is *static priority
  with no forwarding* — a read inside `when` is always the pre-edge value — so it
  coincides with Copper's sequential segment only when no write is read back.
- **Spade.** `reg;` is the cleanest published statement of what a tick does to locals
  (everything visible above it is registered; references below refer to the delayed
  version). Spade *refuses* use-before-ready where Copper *inserts* the register. Spade's
  `&mut` hazards (unset → undefined, conditional set → latch, double set) map one to one
  onto Copper's plain-`Out` hazards, which Copper defines (hold) or refuses by form
  (§10.8's latch finding). Spade says the generated SV is normative; Copper says the
  denotation is — the definitions should say so in as many words.
- **PyMTL3.** The static-DAG / event-driven simulator pair is Copper's levelized /
  fixpoint pair, including the false-comb-loop policy; Copper is stronger (SCC
  condensation accepts the false loops, and the two schedulers are pinned to agree).
  `VerilogTBGenPass` (trace compiled into a Verilog testbench) is the ancestor of
  `verify_with_verilator`. "Semantics = whichever pass you applied" is the position to
  cite as the contrast.
- **Golden Age.** Recommendation (1) — model the register's synchronized update in the
  language's own evaluation semantics so the designer's model matches the hardware — is
  Copper's thesis verbatim; the Silica paragraph's open problem ("restricting the
  coroutine semantics to describe only FSMs") is what `check_reachability`,
  `check_nested_loop_shape`, `check_awaits` and `control_extract` solve. The survey asks
  for one semantics; Copper needs two plus an agreement relation, or "sim ≡ SV" is
  circular.
- **MyHDL.** Fenced the problem off: synthesizable generators have exactly one `yield`,
  at the loop head, so no local crosses a suspension and the suspension means only
  "wait for the event". `always_comb` running at time 0 in Python but not in the emitted
  Verilog is a documented, *tolerated* divergence (§10.14); Copper's `out_first_driven_late`
  and the D2 fix are the refusal and the fix of the same class.
- **Prost!** The closest coroutine prior art: `wait` = tick, `advance : State × In →
  State + Ret`, **every local a register, every output registered**, reset segment at
  the first edge. Everything is closing-anchored; there is no plain `Out`, so the
  alignment family is absent by construction at the price of no passthroughs and a dead
  cycle after reset. Its multi-cycle *call with a return value* is expressiveness Copper
  lacks. Its loop rule is a heuristic Copper's CFG check makes exact.
- **RHDL / RustHDL.** Cite for marker-type clock domains (the ancestor of `Clock<D>`)
  and for naming the two-front-end agreement hazard that `copper-analysis` removes.
  Nothing on ticks or liveness registers.
- **Cummings.** §2.0's race definition (same time step, different result under a
  permitted reordering) is the cleanest statement of poll-order independence; the NBA
  two-phase is `RegOut` commit at the edge; `pipeb1`'s collapse is *correct* Copper
  semantics; `ao4`'s one-delta lag is the fixed D2. Foil: Cummings treats synthesis as
  normative and the designer's operator choice as the fix.
- **SystemC subset.** The model for writing admissibility rules: shall-language, the
  four-way classification (Supported / Ignored / Not Supported / with Restrictions,
  violating instances Not Supported by default), "synthesis refinement", and a positional
  definition of the reset segment (everything before the first statically placed
  `wait()`). It supplies Copper's `check_awaits`, one-clock, reachability, constant-reset
  and single-writer rules in normative form — and then explicitly declines cycle
  exactness ("may have different sequential timed behavior", §3), which is the claim
  Copper makes.
- **Verismith.** State Copper's correctness extensionally the way §4 does, *with the
  precondition* (deterministic, single driver, no comb loops, known power-on). Two
  strengthenings worth adopting: observe internals as well as ports (§3.3), and treat an
  oracle-checks-oracle failure (the Icarus bug) as the reason for a third leg — Copper's
  G-E anchoring. Cite the equivalence-checking scaling data for why Copper simulates.
- **BaseJump.** An empirical oracle, not a normative one: "a reference implementation
  whose Verilog is the specification". `rv->&` is Copper's registered valid/ready;
  `r->v` / `v->r` are the Mealy shapes Copper refuses, with a FIFO as the structural
  remedy; `write_then_read_same_addr_p` is the model for `Memory`'s collision modes.

## 4. What none of them has, and where Copper sits

No document in the set gives a **valued** per-cycle meaning to a **sequential** body
with reads and writes interleaved before a clock boundary, realizes it **twice**, and
states the **agreement relation** between the two realizations. The formal papers
(Esterel, Filament, Anvil) have one realization or none and no values in the hardware
sense; the engineering papers have two realizations and no written reference; the subset
standards have a reference (the host LRM) and disclaim exactness.

Copper has the two realizations and the measurement (the sweep), and a prose denotation
that was validated by predicting divergences before they were measured. What it lacks
is the form the first group has: a kernel syntax, a judgment, rules, and propositions
with named evidence. Writing those is not an exercise in rigor for its own sake. Three
concrete things follow from having them:

- The **anchor** of a drive — opening or closing — is today computed twice, by
  `copper-codegen/src/shir_lower.rs` (`seen_in_read`, per arm) for emission and by
  `copper-analysis/src/cfg.rs` (`leading_read_reaches`) for refusal, with no audit that
  the two agree. A written definition of `anchor` names the single authority and turns
  the audit into a test (the c2 discipline, applied to one more fact).
- The refusal rules are **sufficient conditions** for a proposition that is nowhere
  stated: *for an accepted module, the simulator's observation trace equals the
  SystemVerilog's for every input stream*. Stating it makes the sweep a test of that
  proposition instead of a test of each side against the other, and gives each future
  divergence a place to be filed (which clause failed, on which side).
- The simulator's instant model (two instants per cycle, writes as events, a signal's
  value = its last write at or before the instant) is the write-window invariant made
  operational. Written as rules it is **executable** — a reference interpreter over the
  kernel is a third implementation the sweep can run, which is the one thing that would
  make "the simulator implements the denotation" a measured statement rather than a
  scoping finding (`PAIRED_IMPLEMENTATION_SCOPE.md` §0).

## 5. Devices to take, devices to leave

**Take, in this order of importance:**

1. Esterel's judgment shape: `⟨s, σ⟩ ⇓ ⟨σ′, k⟩` per cycle with a completion kind `k`
   (finished / paused with residue / broke / continued) — one deterministic coroutine,
   so no interleaving, no traps, no `||`.
2. Anvil's and Filament's cycle-indexed carrier, **with values added**: the observation
   function `obs_N : Port → Value`, derived from the execution, not primary.
3. The two-instant refinement nobody has: each cycle has a pre-edge and a post-edge
   settle instant; a statement executes at one of them; an `In` read's value and an
   `Out` write's visibility are decided by the instant. This is Copper's content.
4. Anvil's loop-by-unrolling plus "two iterations suffice" for the back-edge rules;
   Filament's "union of two shifted copies" as the alternative phrasing.
5. Esterel's persistence law for `Out` hold; Anvil's E-RegAssign for `RegOut`.
6. SystemC's normative register and four-way classification for the admissibility
   rules; Verismith's extensional correctness statement with its precondition.
7. Esterel's Theorem 3 as the *template* for the proposition, with the honest status
   the field itself uses: stated, argued, measured — not mechanized.

**Leave:** Esterel's signals/`||`/traps/causality statuses (Copper has no intra-module
parallelism and iterates convergent comb SCCs instead of rejecting them); Filament's
`R ⊆ W` definedness; Anvil's immutable values and declared registers; Bluespec's
nondeterministic scheduling; Chisel's last-connect as a model of forwarding; Spade's
"SV is normative"; MyHDL's and SystemC's tolerance of sim/synth timing differences; Arch's
proof vocabulary.

## 6. Recommendation, and the decisions that are the user's

The recommendation is written out as a draft in `FORMAL_SEMANTICS.md`: a kernel
("basic Copper", surface Rust desugared), a per-cycle big-step relation with a phase
flag that realizes the two instants, the observation function, the admissibility rules
in normative form, the propositions with their evidence, and a conformance ledger that
maps every clause to where the simulator and the transpiler implement it and which test
pins it — with the gaps marked. Six choices in that draft are forks the user should make,
not the assistant; they are listed in its §7. The two that shape everything else:

- **Object:** the kernel over the *surface* (what both front-ends see, `syn::ItemFn`),
  with register inference as a *defined function* on it — recommended, because "the
  simulator runs the user's Rust" is the first contribution and must stay visible in the
  definition; the alternative is to define meaning on the post-inference IR and prove
  inference meaning-preserving (Filament's move with Low Filament).
- **The anchor as definition vs. as implementation:** define a statement's anchor
  *dynamically* (the phase flag flips at the first deferred read executed) — this is what
  the simulator does and what every measurement confirmed — and present codegen's static
  per-drive form as the realization and the rules as the conditions under which the two
  coincide; the alternative, a static anchor as the definition, would make the simulator
  wrong on every path-dependent shape it currently handles.

# Bluespec SystemVerilog — R. S. Nikhil, "Efficient, Correct RTL from High-Level Specs", MEMOCODE 2004 tutorial slides

Source: paper/references/bluespec_memocode2004_tutorial.pdf (37 slides; text:
scratchpad/papers/bluespec_memocode2004_tutorial.txt; slide numbers below are the slide
footers). These are *tutorial slides*, not a semantics paper: the formal content is three
slides (14, 15, 22). The real formal semantics are in the cited MIT TRS work
(Arvind/Shen/Hoe/Stoy, slide 3) and later Bluespec papers; the slides summarize it.

## 1. What is given a semantics; grammar; what defines meaning in practice

- **Object:** BSV surface language — SystemVerilog-style modules, interfaces, types,
  expressions (slide 7: "SystemVerilog basis (design constructs)") plus **rules** and
  **interface methods**. Slide 10 names the pieces: `module name #(parameters)
  (interface formal)`, interface/module instantiation, `rule name (explicit condition)
  ... actions ... endrule` with the note "method calls may have implicit conditions".
- **Grammar:** none shown. Syntax is "SystemVerilog as a basis (but with all the expressive
  power of Bluespec Classic)" (slide 3); Bluespec Classic was Haskell-inspired
  (Augustsson).
- **What defines meaning:** slide 7: "'Rules' — Behavior specs, Term Rewriting Systems
  (TRS) semantics". Slide 14 is the semantic definition (quoted in §2). The compiler
  (slide 25) produces both a "Bluespec C sim" and "Verilog 95" → "Verilog sim"; the
  slides do not say which is the reference. The TRS is the stated reference; both
  backends are implementations of it.
- **Methods are rule fragments** (slide 12): "Methods, like rules, have conditions (e.g.,
  'enq()' only when not full) and actions. The conditions and actions become part of the
  (transitively) invoking rule. Thus, interface methods are a mechanism to 'modularize'
  rules." Slide 11 shows the FIFO interface and its hardware ABI: each method has
  `rdy` (out) and `enab` (in) wires plus data; `enq` ready = not full, `deq`/`first` ready
  = not empty.

## 2. Style

- **Term rewriting / nondeterministic operational.** Slide 14, "Rule semantics", in full:
  > Simple semantics (standard Term Rewriting System): while some rule is enabled (its
  > condition is true) — pick any one enabled rule — perform its (composite) action.
  > A rule can be interpreted as a declarative assertion: an allowable next-state
  > transition. Note: Not a mandatory state transition; non-deterministic choice of one
  > enabled rule. No mention of clocks. One at a time! Can reason about invariants: Each
  > rule in isolation; Legal sequential compositions of rules. We also say: rules are
  > atomic.
- **Plus a refinement/implementation relation** (slide 15, "Rules: Concurrent
  implementation"):
  > Each rule executes within a single clock (implementation choice). Correctness of
  > implementation: When a set of rules execute in parallel in a single clock, there is a
  > legal TRS sequence of those rules, Ri,...,Rj, that has exactly the same effect. The
  > Bluespec compiler generates a hardware scheduler for this — a rule-scheduling decision
  > on every clock (the compiler does "meta scheduling").
  This is the only formal-esque sentence in the deck and it is the key one: the clocked
  implementation is correct iff each cycle's parallel firing is *linearizable* to some
  one-at-a-time TRS sequence.
- Slide 16 (figure): current-state registers → each rule's condition π_i and action δ_i;
  conditions feed a **Scheduler** producing enables φ_i; actions and enables feed
  **Control logic** producing the next-state register inputs. So the generated hardware
  is: all rules evaluated combinationally on the current state; a scheduler picks a
  conflict-free subset; muxes build next state.
- Slide 33 states the design philosophy: "By making it part of the language semantics,
  designer can directly express intent; design intent is formally stated; compilers can
  exploit this in optimization and synthesis; simulators can exploit this."

## 3. Time model

- **The semantics has no clock.** Slide 14: "No mention of clocks." A rule firing is an
  abstract step. The TRS is an untimed transition system over the state (terms = contents
  of registers/FIFOs).
- **Clock cycles are introduced only by the implementation** (slide 15: "Each rule
  executes within a single clock (implementation choice)"), where several rules may fire
  per cycle. The mapping to cycles is therefore not part of the language's meaning; it is
  part of the correctness criterion for the compiler.
- **Within a cycle (implementation):** every firing rule reads the *pre-edge* state and
  writes the next state (slide 16 figure); the "appears sequential" order among rules in a
  cycle is logical. So in the hardware there is one value per register per cycle, and all
  reads in a cycle see the same value, unless a later Bluespec feature (RWire, bypass) is
  used — none appear in these slides.
- No delta cycles in the semantics. The generated Verilog (slide 9/13, hand-written
  comparison side) uses `always @(posedge clk)` + `always @*`, so the Verilog simulator's
  delta cycles exist but carry no meaning for BSV.

## 4. State

- Explicit state-element modules: FIFOs via `mkSizedFIFO#(fifo_depth)` (slides 9, 17,
  21), registers via `mkReg` (not in these slides; shown in the Golden Age survey Fig. 4
  as `Reg#(Bit#(8)) value <- mkReg(0)`). State is never inferred from code shape; a local
  `DataT in_data = inbound1.first;` inside a rule is a wire.
- Initial/reset values: not discussed in the slides (`mkReg(0)` carries one elsewhere).
- The TRS's terms are the state; "terms → synchronous storage elements, rewrite rules →
  combinational logic" is how the Golden Age survey (§3.1.2) paraphrases it.

## 5. Intra-cycle ordering, atomicity, conflicts

- **Within one rule:** the action is a *composite*, parallel action. In `rule enq1`
  (slide 9): `out_queue.enq(in_data); inbound1.deq;` — both happen atomically; `in_data =
  inbound1.first` reads pre-state. There is no sequential read-after-write inside a rule
  (two writes to the same register in one rule is a compile error in BSV, not shown).
- **Across rules in one cycle:** the linearizability criterion of slide 15. Slide 18 ("2
  in, 2 out example"): "Each output FIFO is a shared resource for the rules. Data mux
  logic and control logic are generated by the Bluespec compiler. Correctness: rule
  atomicity ensures that there is no race condition." Two `enq` rules targeting the same
  outbound FIFO cannot both fire in one cycle (one-port FIFO); the scheduler arbitrates.
- **Implicit conditions compose**: slide 10/12 — `enq` has the implicit condition "not
  full", `first`/`deq` "not empty"; the rule's effective guard is the conjunction. This is
  what lets the BSV switch be *one* rule while the hand-written Verilog on slide 9/13 is a
  three-state FSM (`waiting / waiting1 / waiting2`) that serializes dequeue and enqueue —
  the two are not behaviourally identical (the BSV version moves one item per cycle when
  both conditions hold), which the slides use to make the productivity point.
- **Slide 20** (CPU example): atomicity across modules — ROB insert, functional-unit
  writeback, retire, and branch-kill are each a "process" (rule) and "Rule atomicity
  eliminates the nightmare of avoiding race conditions ('inconsistent state') under such
  complex concurrency."
- **Combinational loops:** not discussed. By construction a rule reads current state and
  writes next state, so loops cannot arise within rules; method-to-method combinational
  paths across modules (the `rdy` signals) are where they could, and the scheduler must
  handle it — outside these slides.

## 6. Properties stated and how established

- **Slide 22, "Proofs of correctness":** "Based on rule semantics: Rules in isolation
  (invariants); Sequential compositions of rules. Refinement proofs: Given a spec and a
  design, both written in Bluespec, does the design correctly implement the spec?
  Bisimulation (see Arvind/Shen/Stoy et al.). (Note: not currently automated, but still
  very useful)". So: proof *methodology* is named (invariants per rule, bisimulation for
  refinement), not carried out here.
- **Slide 23:** "Static checking (types etc.) and rule semantics eliminate most functional
  errors, and all race conditions." — a claim from the model, not a measured result.
- **Empirical (slides 29–31):** three LPM (IP lookup) designs, BSV vs hand Verilog: area
  within 1–8%, speed within −2%…+7%, "three Bluespec designs in 3 days"; 25 small designs
  (Gray code … random number generator) with gate counts and timings nearly identical;
  a 1.5M-gate 0.18µ router ASIC: 13× source reduction (66K → 4.7K lines), "66% reduction
  in verification bugs", matched clock/area. All quality-of-results and productivity;
  none is a semantic-agreement measurement.
- **No differential testing claim.** Slide 25 shows the C simulator and the Verilog path
  as two outputs of one compiler; nothing says they were compared.

## 7. Relation between the semantics and the generated Verilog

- The contract is slide 15: the clocked implementation must, each cycle, equal some legal
  sequential composition of the rules that fired. The compiler is trusted to generate a
  scheduler that respects this ("meta scheduling", slide 15; figure slide 16; slide 34
  "Rule meta scheduling" vs CDFG scheduling in C-based HLS). No proof of the compiler is
  offered. Rule atomicity is thus a *semantic* guarantee the compiler discharges, not
  something the designer checks.
- Slide 9/13 side-by-side (BSV vs Verilog 95) is the only generated-vs-hand comparison, and
  the Verilog there is *hand-written*, not compiler output.

## 8. (Survey question — n/a.)

## 9. Transfer to Copper — what fits, what does not

**Fits:**

- **The two-level structure of the specification.** Bluespec gives (i) an abstract
  semantics (TRS, one rule at a time), (ii) an implementation mapping (rules → cycles,
  scheduler), and (iii) a *stated correctness criterion* relating them ("there is a legal
  TRS sequence ... that has exactly the same effect"). Copper should adopt exactly this
  three-part form: (i) the coroutine/segment semantics (what the simulator computes, as a
  denotation, not as "whatever the executor does"), (ii) the lowering to SV
  (`transpile_fir`: control_extract → CHIR → SHIR → VLIR), (iii) the criterion "the SV's
  per-cycle observable trace equals the simulator's". Copper's criterion is simpler and
  stronger than Bluespec's — trace *equality*, no existential over schedules — because
  Copper has no nondeterministic choice. State it as a sentence the way slide 15 does;
  then `tools/regression.sh` and the corpus sweep are the evidence for it, module by
  module, the way bisimulation would be the proof.
- **"Reason about each rule in isolation; then about legal sequential compositions."**
  Copper's analogue: reason about each tick-free segment as a function, then about the
  sequence of segments selected by control flow. The pre-tick alignment family is
  precisely "a property of one segment" (what the segment reads/writes before the edge),
  which is why the rules are checkable on `syn::ItemFn` without simulating.
- **"Appears sequential" is Copper's forwarding.** In BSV, "R_i then R_j" means R_j sees
  R_i's writes; the hardware realizes that only when R_j does not read what R_i wrote
  (conflict-free) or via an explicit bypass. In a Copper segment, a statement after a
  register assignment *does* see the new value (forwarding), and the SV must realize it
  combinationally — the "forwarded guard" clause of D1 (`d4a`: an `if` testing a register
  assigned earlier in the segment) and `pretick_out_write_before_update` are the hazards
  that arise when the two sides disagree about whether a read sees an earlier write. BSV
  avoids the whole class by forbidding read-after-write in one atomic action; Copper
  admits it and must specify it. That is a useful sentence for the design doc: "a Copper
  segment is a *sequential* composition of micro-actions, whereas a Bluespec rule is a
  *parallel* one; the SV realizes the sequential reading by forwarding, and the rule
  family refuses the shapes where forwarding and the simulator's point-of-read differ."
- **The hardware ABI of methods (slide 11)** — per-method `rdy` out, `enab` in — is a clean
  model of a valid/ready port pair and is Mealy (ready is combinational from state).
  Copper decided option 1 (handshakes registered/Moore, `RegOut`); Bluespec is the
  standing counterexample that the Mealy form is the industry norm, worth citing when
  that decision is defended.

**Does not fit, and why:**

- **No intra-rule sequencing** — a rule's action is a parallel composite; all reads see the
  pre-state. Copper's segment is strictly more expressive (read-after-write, multiple
  writes to one port with hold semantics, an `In` read at a point). Nothing in the TRS
  model addresses opening vs closing anchors, `Out` hold, or when an `In` is sampled; the
  problems Copper's `d*`/`e*` batteries measure do not exist in BSV because the language
  cannot express them. Citing BSV as precedent for a segment semantics would be wrong.
- **Nondeterminism and scheduling are the whole intellectual content of BSV and are
  irrelevant to Copper.** One body per module, deterministic; Copper's "scheduler" is the
  *combinational settling* order (levelized / fixpoint), which must be semantically
  invisible — the opposite role from Bluespec's scheduler, which *chooses behaviour*.
  Do not borrow the word "scheduler" without qualifying it.
- **A clockless abstract semantics would be wrong for Copper.** `tick` is a first-class
  edge; outputs are observed per cycle; the pretick family is *about* phases. Copper sits
  at Bluespec's *implementation* level (one rule = one cycle, always) and should say so:
  "a Copper module is the degenerate Bluespec program with exactly one rule per cycle,
  always enabled, and the rule body is a sequential program." That makes the
  linearizability criterion trivially satisfied and moves all the content into the body.
- **Guards/implicit conditions:** Copper has no guard concept; `while !ready.read() {
  tick }` is the manual encoding, and it costs a cycle per check (Moore). BSV's implicit
  conditions compose combinationally across modules in the same cycle. Not transferable
  without a Mealy port form.
- **The slides are not a formal source.** For the actual semantics cite Hoe & Arvind
  (TRAC, "Hardware synthesis from term rewriting systems", Golden Age ref. [32]) and the
  BSV Reference Guide's rule-semantics chapter; use these slides only for the three
  quoted sentences (slides 14, 15, 22).

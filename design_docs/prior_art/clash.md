# CλaSH — Baaij, Kooijman, Kuper, Boeijink, Gerards, "CλaSH: Structural Descriptions of Synchronous Hardware using Haskell", DSD 2010

Source: paper/references/clash_dsd2010.pdf (text: scratchpad/papers/clash_dsd2010.txt).
Note the date: this is the 2010 prototype, **before** the `Signal` type. State is passed
explicitly (`State` wrapper); there is no stream type in the language. The Golden Age
survey's Fig. 3 (`acc1` with `Stream`/`delay`) describes the *later* Clash.

## 1. What is given a semantics; grammar; what defines meaning in practice

- **Object:** a *subset of Haskell itself*, not an embedded DSL. §I: "This research uses (a
  subset of) the Haskell language itself for the purpose of describing hardware." The
  abstract: CλaSH "borrows both its syntax and semantics from the functional programming
  language Haskell."
- **Grammar:** none in the paper. The grammar is Haskell 98's ([11]) by inheritance; the
  supported subset is described by enumeration in §II (function application §II-A, choice
  §II-B, types §II-C — Bit, Bool, Signed/Unsigned with wrap-around overflow, Vector, Index,
  single-constructor and enumeration ADTs; multi-constructor-with-fields ADTs unsupported;
  polymorphism §II-D; higher-order functions §II-E; state §II-F).
- **What defines meaning in practice:** two things, and the paper is explicit about both.
  1. *Host-language evaluation.* Abstract: "As the circuit descriptions, simulation code,
     and test input are also valid Haskell, complete simulations can be done by a Haskell
     compiler or interpreter." §II-F: "The complete simulation can be compiled to an
     executable binary by a Haskell compiler, or executed in a Haskell interpreter."
     Simulation = running `run f s inputs` (eq. 11) under GHC.
  2. *The compiler pipeline* (§III, Fig. 5): GHC front-end (parser, type checker) → **Core**
     ("System F with type equality coercions", [13]) → **normalization** ("exhaustively
     applies a set of meaning-preserving transformations on the Core description until this
     description is in a normal form", including β-reduction, η-expansion, specialization
     of higher-order functions and polymorphic types) → **VHDL netlist** ("a straightforward
     process due to the resemblance of a normalized description and a set of concurrent
     signal assignments"; "the fact that it is VHDL is only an implementation detail; e.g.,
     the output could have been Verilog or even EDIF").
- So the semantic contract is: Haskell's meaning of the function *is* the meaning; the
  netlist is claimed to be "equivalently behaving" (§I). Core is the IR the compiler
  actually reasons over, but no semantics for Core is given here (it is GHC's).

## 2. Style

- **Denotational, inherited from the host.** A combinational circuit is a pure function
  (§II-F: "Pure functions are a perfect match for combinational circuits"). A stateful
  circuit is "the combinational parts of a Mealy machine": `macS (State c) (a, b) =
  (State c', c') where c' = mac a b c` (eq. 10, Fig. 4).
- **The `run` function (eq. 11) is the whole temporal semantics**, and it is three lines:
  ```
  run f s (i : inps) = o : (run f s' inps)
    where (s', o) = f s i
  ```
  "The run function applies the function the developer wants to simulate, f, to the
  current state, s, and the first input value, i. The result is the first output value, o,
  and the updated state s'."  Equation (12): `run macS (State 0) inputpairs`.
- **By-translation for structure** (§II-A, the four rules): every function → component,
  every argument → input port, result → output port, application → instantiation; §II-B
  case/if/pattern-match/guards → multiplexer (Fig. 3); §II-F `State` → register (Fig. 4).
  These are the only translation "rules" stated; they are stated in prose.
- There is no operational semantics, no type-system-as-semantics beyond Haskell's own, and
  no delta-cycle model.

## 3. Time model

- **Implicit clock, one function application per cycle.** §I: "the clock is implicit for the
  descriptions ... A circuit designer describes the behavior of the hardware between clock
  cycles, as a transition from the current state to the next." §II-F: "In the context of
  this paper, it is assumed that there is one input per clock cycle. However, this input
  can be a variable with multiple fields."
- **One value per signal per cycle, by construction:** the input stream is a Haskell list,
  one element per cycle; `run` yields one output per element. There is no finer
  granularity — no delta cycles, no events, no notion of "when within the cycle" a value
  is read. Combinational settling is simply Haskell evaluation of a pure expression.
- **Cycle-stepped, not event-stepped**: `run` is a fold over the input list.
- **Single clock, synchronous only.** §I: "This abstraction of state and time limits the
  descriptions to synchronous hardware. However, work is in progress to add an abstraction
  mechanism that allows the modeling of asynchronous and multi-clock systems."
- Contrast with stream-based functional HDLs stated explicitly (§I): "Many functional HDLs
  model signals as a stream of values over time; state is then modeled as a delay on this
  stream of values. Descriptions presented in this research make the current state an
  additional input and the updated state a part of their output."

## 4. State

- **What is a register:** exactly what the designer wraps in `State` in the type signature.
  §II-F: "The State wrapper indicates which arguments are part of the current state, and
  what part of the output is part of the updated state. ... which variables are part of
  the state is completely determined by the type signature." The CPU example (§IV-B):
  `type CpuState = State [Signed 16 | 4]` — "Each function unit outputs its result into a
  register, i.e., the state of the CPU."
- **Initial values:** supplied as an argument to `run` at simulation time (`run macS
  (State 0) inputpairs`, eq. 12). The initial value is therefore a *simulation parameter*,
  not part of the circuit description. The paper says nothing about how the generated VHDL
  initializes or resets registers (no reset port appears anywhere). This is a real gap
  between the Haskell semantics (always starts from the given `s`) and the VHDL.
- **Cost acknowledged** (§VII): "distributing and collecting substate can become tedious and
  even error-prone. Automating the required distribution and collection, or finding an
  abstraction mechanism that suppresses state would make CλaSH easier to use. Currently,
  one of the examined approaches to suppress state in the specification is by using
  Haskell's arrow-abstraction." (The later `Signal` type is the resolution.)

## 5. Intra-cycle ordering

- **There is none to specify.** The body is a pure function; `where` bindings are equations;
  evaluation order is irrelevant by purity; each name is bound once. No last-write-wins, no
  blocking/nonblocking, no forwarding question. The state read (`c` in `macS`) is the
  current (pre-edge) state and the output `c'` is computed from the *same cycle's* input —
  i.e. every output is Mealy-combinational from the cycle's input unless the designer
  routes it through `State`.
- **Combinational loops:** not discussed. A recursive `where` binding would be a lazy
  value-level fixpoint in Haskell and the paper is silent on whether the compiler accepts
  it.
- The FIR example (eq. 16) and `shiftInto` (eq. 17) show the shift-register idiom: the new
  state is a pure function of (old state, input); the output `(x . xs) • hs` uses the
  *current* input `x` and the *old* samples `xs` in the same cycle — exactly the
  Mealy transition.

## 6. Properties stated and how established

- **No proofs.** §VII is unusually candid: "The transformations in the normalization phase
  of the prototype compiler are developed in an ad-hoc manner, which makes the existence of
  many desirable properties unclear. Such properties include whether the complete set of
  transformation will always bring the description into a form that can be translated to
  hardware or whether the normalization process always terminates. Although extensive use
  of the compiler suggests that these properties hold, they have not been formally proven."
- **Claims:** §I "converts the Haskell code to equivalently behaving synthesizable VHDL";
  §III "meaning-preserving transformations". Both asserted, not argued.
- **Evidence offered:** Table I (§VI) — a streaming floating-point reduction circuit in
  CλaSH vs hand-optimized VHDL: 4076 vs 4734 CLB slices/LUTs, 2467 vs 2810 DFFs, 159 vs
  171 MHz; "they only give a first impression". That is a synthesis-quality number, not a
  behavioural-agreement number. No count of designs on which Haskell simulation and VHDL
  simulation were compared.

## 7. Relation between the semantics and the generated VHDL

- The generated VHDL is *claimed* to behave equivalently; the mechanism offered is a
  **generated testbench**, §III: "For verification purposes of the generated VHDL, the
  compiler also creates a test bench and corresponding input, allowing a developer to
  compare the external behavior of the VHDL netlist against the original CλaSH design."
  So differential testing of host simulation vs VHDL simulation exists as tooling, is
  user-driven, and no result of using it is reported.
- No translation-correctness statement; the normalization is the unverified part (§VII).

## 8. (Survey question — n/a for this paper.)

## 9. Transfer to Copper — what fits, what does not

**Fits:**

- **The `run` equation is the denotation Copper should write down first.** A Copper
  sequential module is a Mealy step function `f : (S, I) → (S, O)` where `S` is the set of
  registers (locals live across a tick, plus `RegOut` flops and memories), `I` the `In`
  values for the cycle, `O` the `Out`/`RegOut` values observed for the cycle; the trace is
  `run f s0 inputs`. Copper's "one value per signal per cycle" *is* "one list element per
  cycle". The thing Copper adds is that `f` is not written by the designer; it is the code
  between two `tick`s, and `S` is *inferred* (`infer_registers`: defined-in-loop ∧
  live-across-a-tick) rather than declared in the type. A semantics for Copper should
  therefore state (a) what `S` is, (b) that the inferred `S` is sufficient (every value
  that flows across a tick is in it), and (c) that the per-segment body is a function of
  `(S, I)` only. CλaSH gets (b) for free because the designer declares `S`; Copper must
  prove or test it (the G2 structural reg-match helpers and the register-reconciliation
  tests are that evidence).
- **Initial state as the argument of `run`** matches Copper's register power-on rule
  (2026-10-29): a pre-loop register initializer must be a constant that reaches the SV as
  `initial`. Copper is stricter than CλaSH here: CλaSH's initial value is a simulation
  argument with no VHDL counterpart in the paper. Worth saying in Copper's doc that the
  initial state is part of the description, not of the testbench.
- **Purity as the explanation of "simulation is independent of poll order".** CλaSH's
  combinational part is a pure function, so the question of evaluation order cannot
  arise. Copper's levelized/fixpoint settling should be *stated* as "the settled values
  are the unique solution of the combinational equations" (a denotation), with the
  scheduler as an implementation of that — not as "whatever the scheduler computes".

**Does not fit, and why:**

- **CλaSH has no intra-cycle sequencing, so it says nothing about Copper's hard problems.**
  A CλaSH output is defined once per cycle by an equation. Copper's segment is an
  imperative program: a plain `Out` can be written several times, an `In` can be read at a
  *point* in the segment before or after a register update, a later statement sees an
  earlier `let`. Every rule in the pre-tick alignment family (`unprotected_pretick_out_write`,
  `multi_phase_out_write`, `pretick_out_write_before_update`, `trailing_*`,
  `linear_out_write_anchor_mismatch`) exists because of that gap; the Mealy-function
  denotation cannot even express the distinction between an opening and a closing anchor.
  To use the `run` denotation, Copper must first define how a segment *denotes* a function
  — that is the content of CYCLE_DATAFLOW_SEMANTICS.md and it has no CλaSH analogue.
- **No multi-tick bodies.** A CλaSH `f` is one cycle; a Copper body with ticks in branches
  or loops is an FSM that `control_extract` flattens into `match pc`. In CλaSH the program
  counter would be an explicit field of `State`. The denotation for Copper must either be
  stated on the extracted single-tick form (then `pc` is a register in `S` and the
  CλaSH view applies directly) or on the coroutine (then it is a different object: a
  sequence of segment-functions selected by `pc`). Choosing the extracted form is the
  cheap way to inherit the CλaSH shape — but then the semantics of the *source* is "the
  semantics of its extraction", which is what the phase-sensitive checks in
  `copper-analysis` exist to see through. Decide this explicitly.
- **Input reads are implicitly "this cycle's value".** In CλaSH `f s i` gets the whole
  cycle's input `i`; there is no way to read an input "early" or "late". Copper's
  `Immediate` vs deferred reads (D2 fix) and the anchor rules are the price of having a
  *point* of reading. CλaSH cannot be cited as support for either anchor; it is the
  degenerate case where every read is at the (unique) observation point.
- **Mealy by default.** `c'` in `macS` is combinational from `a`, `b` in the same cycle.
  Copper's default plain `Out` *holds when unwritten* in a sequential module and `RegOut`
  is a flop; neither is the CλaSH default. If Copper borrows the Mealy-function picture it
  must add the hold register (Chisel Fig. 1 / Copper phase C `<port>_hold`) to the
  denotation of a plain `Out`, since in CλaSH an output that "is not written this cycle"
  is a type error, not a hold.
- **No settling model at all.** CλaSH has neither a levelized schedule nor a fixpoint
  loop; combinational loops are outside the paper. Copper's SCC-condensed levelized
  scheduler with convergent-loop iteration is beyond what CλaSH would accept.

**Bottom line for the owner:** copy the *shape* — "the body between ticks is the
combinational part of a Mealy machine; `run` folds it over the input stream; registers
are the state argument" — as the top-level denotation; it is three lines and matches
"one value per signal per cycle". Everything Copper's rule family is about lives *inside*
`f` and has to be defined separately (what function a segment denotes), which CλaSH
never had to do because its `f` was already a function.

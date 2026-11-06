# Herklotz & Wickerson, "Finding and Understanding Bugs in FPGA Synthesis Tools" (Verismith; FPGA 2020, 11 pp.)

Source: `paper/references/verismith_fpga2020.pdf`, text in `scratchpad/papers/verismith_fpga2020.txt`.
Read in full. Citations are the paper's section numbers and ACM page numbers (277–287).

## 0. One-paragraph verdict

Verismith is a *methodology* paper: it does not specify Verilog's semantics; it takes the
synthesisable subset of Verilog 2005 (IEEE 1364.1) as given and defines "correct synthesis"
*extensionally* as logical equivalence between the input design and the output netlist,
checked formally. Its semantic contribution is the observation that the equivalence
question only has a yes/no answer if the generated Verilog is **deterministic** — "the
values of output wires should be uniquely determined by values of the input wires" (§2
p.278) — and the engineering of a generator that stays inside that deterministic fragment
by construction (no undefined values, no multiple drivers, no comb loops, no out-of-range
selects, no division by zero, registers initialised). With that, "false negatives and false
positives are not possible" (§2 p.278) and any inequivalence is a tool bug. It is the direct
model for Copper's corpus differential sweep (seeded random stimulus, sim vs SV under
Verilator) and for the "a divergence is a bug, by definition" stance; the difference is that
Verismith's oracle is a formal equivalence check over *all* inputs while Copper's is
simulation over a finite stimulus set — and Verismith's own §4 explains the trade-off
(equivalence checking scales exponentially, times out at 900 s).

## 1. What is given a meaning, and by what

- **Target language**: "The synthesisable subset of Verilog 2005 [10] was chosen as the
  target HDL as it is widely supported" (§3.1 p.278). Meaning of Verilog = IEEE 1364.1-2005;
  the paper adds nothing and cites the standard for what synthesis ignores ("constructs that
  do not have a direct hardware equivalent and include delays, specify blocks or system task
  calls", §3.1 p.279) and for the initial-value exception ("initial blocks and variable
  initialisation are supposed to be ignored by the synthesis tool, except when modelling
  ROMs [10, section 7.7.9.1]", §3.1 p.279).
- **Correctness of a synthesis tool** (the thing the paper defines): "checking that the
  synthesised netlist is always equivalent to the original design" (Abstract p.277);
  operationally (§4 p.281): "the output wires of the randomly generated Verilog design and
  the synthesised netlist should always be equal at the clock edge given the same inputs",
  expressed as a Verilog miter `always @(posedge clk) assert(y1 == y2);` passed through Yosys
  to SMT-LIBv2 or ABC. "As only deterministic Verilog is being tested, which does not
  contain any undefined values, the equivalence check proves that the design is equivalent
  to the netlist over all possible inputs."
- **Semantic validity** (§3.2 p.279): "We define semantically valid programs as ones that
  should be accepted by simulators and synthesis tools without producing any errors or
  critical warnings." — a tool-acceptance definition, not a semantic one. Rules: "module
  inputs have to be nets; no continuous assignments to variables or blocking/non-blocking
  assignments to nets occur; every variable that is used has to be declared."
- **Determinism** (§3.2 p.279): "the generated Verilog should be deterministic. This
  simplifies the equivalence checking stage, as there cannot be any false positives or false
  negatives. Therefore the equivalence check indicates if there is a bug or not." Argument
  for why that is the right fragment: "bugs found using purely deterministic Verilog are more
  severe than bugs that include undefined values, as most Verilog in production is
  deterministic and it is generally bad practice to write Verilog that depends on undefined
  values. Furthermore, undefined values might mask bugs in the synthesis tool by allowing it
  to optimise away large sections of the design."
- Non-deterministic constructs avoided (§3.2 p.279): "driving a wire from two different
  sources, dividing by zero, passing fewer bits to a module than it expects, selecting bits
  that are out of range, and using a net that has not been declared previously."

## 2. Style

Testing-based (random differential testing with a formal oracle) plus subset-restriction by
construction (a generator that can only emit the deterministic fragment). No semantics, no
rules beyond the generation invariants. Empirical evaluation (18 000 CPU hours, Table 2).

## 3. Time model

- Single clock `clk` input added to every module; sequential blocks are `always @(posedge
  clk)` (§3.3 p.280, Fig. 4). Equivalence is asserted "at the clock edge" (§4 p.281), i.e.
  per-cycle on the output concatenation; internal state is exposed by concatenating "all the
  internally declared variables and nets ... and assigned to the output, so that any
  discrepancies in the internals of the module are detected" (§3.3 p.280). This is the
  "observe everything every cycle" discipline — stronger than Copper's port-only
  comparison, and the reason Copper's sweep found `branch_merge`'s lag only after
  multi-seed stimulus (observing a port whose divergence depended on the first cycle's
  input). Verismith's design makes every internal register an observed output, so a wrong
  internal value cannot hide.
- No delta cycles, no event regions discussed: formal equivalence over the unrolled
  sequential circuit abstracts them away. The synthesis tools and Yosys agree on the
  synchronous abstraction; the paper never needs the event queue because its fragment has
  no races (one driver, generated-in-dependency-order combinational logic).
- `for` loops inside `always` blocks are generated (Fig. 4 line 36) with static bounds —
  unrolled by synthesis.

## 4. State

- Registers: `reg` declared with an initialiser `reg [2:0] reg20 = (1'h0);` (Fig. 4
  p.280). Justification (§3.1 p.279): "Although initial blocks and variable initialisation
  are supposed to be ignored by the synthesis tool, except when modelling ROMs, both of
  these features are supported in all the synthesis tools that were tested for FPGAs, as
  these can be powered up into a known state. These are therefore used by Verismith to set
  the design in a known state at the start." — i.e. Verismith relies on FPGA-style
  power-on initialisation to make the sequential equivalence problem well-posed (no X at
  reset), exactly Copper's 2026-10-29 decision (constant initializers → SV `initial`;
  `COPPER_VERILATOR_X_INITIAL=1` randomises them to measure the residue). The C-Reduce
  comparison's validity heuristics also include "requiring all registers to be initialised"
  (§6.5 p.285).
- Variables vs nets: nets (`wire`) driven only by `assign`; variables (`reg`) only by
  procedural assignment; inputs are nets (§3.2 p.279).
- Both blocking and nonblocking assignments are generated inside `always @(posedge clk)`
  (§3.1 p.279; Fig. 4 line 49 `reg19 = ...` blocking next to `reg20 <= ...`), but the shipped
  configuration sets `statement.blocking = 0` (Fig. 3 p.279) — blocking disabled in the
  reported runs. Within one always block with one writer per reg, blocking in a clocked block
  is deterministic (Cummings' "if confined to one always block" case), so this would still be
  inside the deterministic fragment; the authors chose not to exercise it.

## 5. Intra-cycle ordering, drivers, loops

- **Combinational loops**: excluded by the generation order. "the syntax tree is built
  sequentially, line by line, using a context to keep track of important facts about
  previously generated code. This inherently prohibits combinational loops, as all the
  values being assigned to the current wire will already have been assigned beforehand"
  (§3.3 p.279). I.e. every `assign` reads only earlier-assigned wires — a topological order
  by construction. This is the same acyclicity Copper's levelized scheduler *checks*
  (`comb_cycles()` static detection; non-convergent SCC → panic).
- **Multiple drivers**: excluded ("driving a wire from two different sources", §3.2 p.279)
  — nets are assigned once, regs by one block (the context tracks "a list of nets and
  variables that are declared and assigned in the current context").
- **Undefined values at run time**: division by zero and out-of-range selects are guarded
  "statically at generation or dynamically at runtime ... runtime checks need to be added to
  operations like division to check against division by zero. This is similar to the safe
  math wrappers that are used in an existing C fuzzer called Csmith [26] to avoid undefined
  behaviour like signed overflow" (§3.3 p.280). Division/modulo were mostly removed for
  cost reasons (§3.3 p.280).
- **Signedness** is the richest bug source found: `$signed`/`$unsigned`, `-1'b1` expansion
  (Fig. 1 p.277: "Vivado incorrectly expands -1'b1 to -2'b11 instead of -2'b01"), bit
  selection truncation (§6.1.3 p.283), peephole on multiply-shift (§6.1.1 p.283). These are
  all width/sign-extension rules of IEEE 1364 — the part of Verilog's semantics that is
  precisely specified and still mis-implemented. Relevant to Copper's `Bits<N>` width/sign
  lowering in `vlir_lower` ("concrete widths") and to the `yosys` profile.
- **Sim/synth mismatch**: here the mismatch is *tool bugs*, not language ambiguity, because
  the fragment is deterministic. The paper's framing of the trust problem (§1 p.277): Silver's
  proof "assumes that the 'toolchain taking Verilog to FPGA bitstreams is bug-free'", Kami's
  guarantees hold "if we trust [the] compiler to preserve the semantics". Verismith tests
  that trust.

## 6. Properties claimed and how established

- "Every synthesis tool except Quartus Prime was found to introduce discrepancies between
  the netlist and the design" (Abstract p.277). 11 bugs reported (7 mis-synthesis: 4 Yosys,
  3 Vivado; 3 crashes; 1 Icarus simulator bug), 6 fixed at submission. Table 2 (p.282):
  failure rates per tool/version (Yosys 0.8 27.1%, Yosys 0.9 2.31%, Vivado 18.2 2.36%,
  Quartus Prime 0% over 80 300 cases, Quartus Prime Lite 39.5% all traced to `$signed`).
- Method soundness: "If the netlist is shown not to be equivalent to the design, it must mean
  that there is a synthesis bug, as false negatives and false positives are not possible.
  However, there is the possibility that the SMT solver does not give an answer in time"
  (§2 p.278). Caveat when testing Yosys with Yosys as the equivalence front end: "there might
  be bugs that are not found by the equivalence check, as the same bug will be present when
  design is passed to the SMT solver or to ABC" (§4 p.281); mitigations: compare two tools'
  netlists against each other, or "a test bench can also be created ... by passing random
  test vectors to both top-level modules and checking that the output remains the same" (§4
  p.281) — i.e. simulation-based differential testing is presented as the fallback oracle,
  which is Copper's primary one.
- Counterexamples are confirmed by simulation: "the checker returns a counterexample which
  can be added to a testbench and hence simulated, to make sure that it does indeed expose a
  difference between the design and the netlist" (§4 p.281) — and this found a bug in Icarus
  Verilog (§6.1 p.282). An oracle checking an oracle.
- Size/efficiency findings (§6.2 p.283–284): "shorter programs are more effective at
  triggering mis-synthesis bugs, and longer programs are more effective at triggering
  crash bugs"; ~700 lines recommended; equivalence checking "scales exponentially"; XOR-
  reducing outputs to one bit makes checking far worse (§6.3 p.284) — "the output should not
  be combined unless it is absolutely necessary."
- Stability across Vivado versions (§6.4 p.284–285, Fig. 7): bugs fixed and introduced per
  release; one group of 15 cases fails in every version.

## 7. Relation between simulation and synthesized hardware

- The design's meaning for the check is the *synchronous unrolling* that Yosys/ABC/Z3 build
  from the RTL — not a simulator. Simulation (Icarus) is used only to validate
  counterexamples. So the paper's "correct" is: RTL (as interpreted by the equivalence
  front end) ≡ netlist (as interpreted by the same front end) for all input sequences.
- Reduction (§5 p.281–282): hierarchical, greedy, binary-choice delta debugging over the
  AST (modules → module items → statements/branches → expressions), with the generator's
  concatenated-output structure used as an optimisation ("binary search can then be
  performed only on the concatenation"). Compared with C-Reduce (§6.5 p.285): Verismith
  faster (119 s vs 2640 s mean) and smaller for mis-synthesis (median 11 vs 61 lines);
  C-Reduce reduced 6 of 30 cases "to contain only undefined behaviour ... and discarded the
  original cause" — the semantic-validity-preserving reducer matters.

## 8. Transfer to Copper (concrete and critical)

### What fits
- **The definition of correctness by differential equivalence over a deterministic
  fragment** is Copper's definition: simulator ≡ transpiled SV, cycle for cycle, and "a
  divergence is a bug, by definition" because the admitted language has no undefined
  values, one driver per port, no comb loops that fail to converge, and constant power-on
  values. Verismith states the precondition that makes this a theorem-shaped claim
  ("deterministic ... false negatives and false positives are not possible"); Copper's
  formal-esque definitions should state the same precondition explicitly — which
  constructs are refused *so that* the equivalence question is well-posed (X, multiple
  drivers, non-convergent SCCs, non-constant initializers, UB in the data surface).
- **Observe internals**: Verismith concatenates every internal reg/wire into the output so
  "any discrepancies in the internals of the module are detected" (§3.3). Copper compares
  ports only; its 2026-10-28 multi-sequence policy (four seeds + every initial input vector
  for ≤4 input bits + truth tables for comb ≤8 bits) is the stimulus-side answer to the same
  concern. A corpus option to also compare inferred registers (the `V<module>` has them)
  would be the Verismith-style strengthening.
- **Power-on initialisation to make sequential equivalence well-posed** (§3.1): identical
  to Copper's `initial` emission decision and to `COPPER_VERILATOR_X_INITIAL=1` as the
  measurement of what the decision hides.
- **Acyclicity by construction vs by check**: Verismith's generator orders assignments so
  loops cannot be written; Copper admits convergent comb loops (iterated) and refuses
  non-convergent ones structurally. Verismith is evidence that the loop-free fragment is the
  one tools agree on; Copper's "detect ≠ reject" stance on convergent loops is a deliberate
  widening that the sweep must keep honest.
- **Test-case reduction that preserves validity** (§5, §6.5): Copper has no reducer; a
  failing sweep case is a whole module. The lesson that a reducer which can introduce UB
  "discards the original cause" is relevant if Copper ever automates shrinking a diverging
  `#[hardware]` fn (the reduction must stay inside the admitted subset or the divergence can
  turn into a rule violation).
- **Oracle self-check**: Verismith validates equivalence counterexamples by simulation and
  found an Icarus bug. Copper's three-way anchoring (sim vs transpiled SV vs independent
  BaseJump Verilog, G-E) is the same discipline; Verismith shows why one oracle (Yosys as
  both synthesiser and checker) is not enough.
- **Scaling data** (§6.2–6.3): formal equivalence checking is exponential in design size
  and times out at modest sizes; simulation-based differential testing (Copper's method)
  trades completeness for scale. Cite for why Copper uses simulation rather than
  equivalence checking, and for why the RV32I pipeline is swept by stimulus, not proved.

### What does not fit
- Verismith's correctness is over *all* inputs (formal); Copper's is over a finite,
  seeded, partly exhaustive stimulus set. Copper's definitions should not borrow "proves ...
  over all possible inputs" (§4) — the honest claim is Verismith's fallback: "passing random
  test vectors to both top-level modules and checking that the output remains the same."
- Verismith tests a *fixed* language against tools; Copper tests a language *and* its two
  implementations (simulator, transpiler) against each other, with no third normative
  artefact except where BaseJump anchors a module. The analogue of "the netlist is wrong" is
  ambiguous in Copper (simulator wrong? codegen wrong? rule missing?), which is why Copper's
  design docs record which side is normative (`CYCLE_DATAFLOW_SEMANTICS.md`: denotation
  normative) — Verismith never needs that.
- Verismith's fragment has no coroutine, no multi-cycle control flow, no notion of
  opening/closing anchors; it is RTL-level Verilog. Nothing in it speaks to the pre-tick
  alignment family.

### Critical take
Verismith is the right citation for (i) the extensional definition of synthesis
correctness, (ii) the precondition (determinism, no UB) under which differential checking
is sound, (iii) the engineering of a generator confined to that fragment, (iv) the
observed fact that signedness/width rules are where tools are wrong, and (v) the cost
curve that justifies simulation over equivalence checking. It is not a source of semantics;
it inherits IEEE 1364.1 and the equivalence checker's unrolling.

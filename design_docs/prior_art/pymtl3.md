# PyMTL3 — Jiang, Pan, Ou, Batten, "PyMTL3: A Python Framework for Open-Source Hardware Modeling, Generation, Simulation, and Verification", IEEE Micro 2020

Source: paper/references/pymtl3_ieeemicro2020.pdf (text:
scratchpad/papers/pymtl3_ieeemicro2020.txt; Fig. 2's code was read from PDF page 5 —
the text extraction dropped it). Sections are unnumbered: Workflow, Framework, Embedded
DSL (Unified Multilevel Scheduling; Static Elaboration; Polymorphic Interface
Connections; User-Defined Data Types; PyH2), Passes (Linting, Statistics,
Presynthesis, Simulation, Tracing, Translation, Import, Prototype Proxy, Ad-Hoc
Transform). It is a *framework* paper; semantics is discussed only as "what the
simulation passes do".

## 1. What is given a semantics; grammar; what defines meaning in practice

- **Object:** Python classes extending `Component` with `InPort`/`OutPort`/`Wire`, update
  blocks, and connections. Fig. 2 (page 5, lines 1–28):
  ```
  class RegIncr( Component ):
    def construct( s, Type, inc=1 ):
      s.in_ = InPort( Type ); s.out = OutPort( Type ); s.tmp = Wire( Type )
      @update_ff
      def seq_reg():  s.tmp <<= s.in_
      @update
      def comb_out(): s.out @= s.tmp + inc
  class RegIncrNstage( Component ):
    def construct( s, Type=Bits32, N=1 ):
      ... s.rs = [ RegIncr( Type ) for _ in range(N) ]
      connect( s.rs[0].in_, s.in_ ); connect( s.rs[-1].out, s.out )
      for i in range(N-1): s.rs[i].out //= s.rs[i+1].in_
  dut = RegIncrNstage( Bits16, 3 ); dut.set_param(...); dut.elaborate()
  dut.apply( SimulationPass() )
  ```
  `@update_ff` + `<<=` = sequential (flop) block; `@update` + `@=` = combinational block;
  `connect` / `//=` = structural wiring. Three modelling levels: FL, CL, RTL (only RTL
  translates).
- **Grammar:** none; it is Python. Elaboration (`elaborate()`) builds the **IMIR**
  (in-memory IR, "a model-level view of the whole design hierarchy for not only the RTL
  circuits, but also CL/FL methods and update blocks which can sometimes include
  arbitrary Python code"); translation lowers RTL to **RTLIR** then to SystemVerilog.
- **What defines meaning in practice — stated explicitly as pluggable:** Simulation Passes:
  "Simulation passes are instrumentation passes that add a tick function to the top-level
  component for the user to simulate the whole design cycle-by-cycle. **Each simulation
  pass implements different modeling semantics and/or creates a different simulator** for
  different simulation performance." PyMTL3 therefore declines to have a single
  normative semantics; the meaning is whichever pass you applied. (The Mamba paper, ref.
  6, is where the scheduling techniques are defined.)

## 2. Style

- **Operational-by-scheduler, informal.** The semantic content is the UMOC paragraph
  (Unified Multilevel Scheduling): "The intracycle ordering of RTL update blocks is
  implicitly inferred from the signals that each block reads or writes. The intra-cycle
  ordering of CL/FL update blocks is deduced from local explicit ordering constraints
  between method and/or update blocks, and the information of the methods each update
  block calls. The user can simply set explicit ordering constraints in each component.
  The simulation passes will handle all the ordering constraints globally."
- Two simulators with different acceptance:
  - **EventDrivenPass**: "can schedule pure-RTL models with cyclic dependencies between
    update blocks and throw exceptions for actual combinational loops. The pass queries
    the read/write information of all update blocks and constructs sensitivity
    information to decide the dependent blocks of each update block. The added tick
    function maintains an event queue to trigger update blocks."
  - **StaticSchedulingPass**: "can only schedule models without cyclic dependencies even
    though they may not be actual combinational loops. However, removing the event queue
    leads to higher simulation performance when the toggle rate is high. The pass
    constructs a direct acyclic graph and applies topological sort to compute a linear
    execution schedule for every cycle. The added tick function simply iterates over the
    static schedule."
- No denotation, no type system (Python is dynamically typed; Bits types are runtime
  objects), no translation rules stated.

## 3. Time model

- **Cycle-stepped `tick`.** VcdGenerationPass "adds a callback function before the
  simulated rising clock edge". `@update_ff` blocks model flops with `<<=` (nonblocking);
  `@update` blocks are combinational with `@=` (blocking). The paper does not spell out the
  phase order inside `tick` (flops then comb, or comb then flops); Fig. 2's `seq_reg`
  reads `s.in_` and writes `s.tmp`, `comb_out` reads `s.tmp` — the schedule must run the
  ff block's commit before `comb_out` sees the new `tmp`, which is the standard
  "ff-commit at the edge, then comb settle" order.
- **One value per signal per cycle** at the observation point; within the cycle the static
  schedule evaluates each comb block exactly once in topological order (levelized). The
  event-driven pass re-evaluates on change — a fixpoint-style settle with an exception on
  true loops.
- Sensitivity granularity: "The simulation passes can determine the sensitivity of
  subfields to correctly schedule the simulation" (User-Defined Data Types).
- No delta cycles as a user concept; the event queue is an implementation of one.

## 4. State

- **A signal written in an `@update_ff` block is a flop — inferred from the block kind.**
  CheckClockGatingPass "reports all signals that are inferred to flip-flops, but
  nonblocking assignments are not included in an if statement block" (so a `<<=` outside
  an `if` is a flop with no enable). CheckInferredLatchPass "reports potential inferred
  latches by querying the AST of combinational update blocks to check if each signal
  written in the block has valid assignments in all conditional branches" — i.e. the
  Verilog latch rule, enforced as a lint, which tells us the comb semantics is "last
  blocking write in the block; unwritten = latch (bad)".
- **Initial/reset values:** not discussed in the paper (Fig. 2 has no reset). Don't claim
  anything.

## 5. Intra-cycle ordering

- **Between blocks:** dependency DAG from read/write sets → topological sort → linear
  schedule (static) or event queue (event-driven). "DumpUDGPass leverages graphviz to
  visualize the directed graph of all update blocks as vertices and all dependencies as
  edges."
- **CL/FL:** explicit user constraints between method and update blocks (the paper does not
  show syntax; in PyMTL3 practice `s.add_constraints( U(blk) < M(method) )`).
- **Within a block:** Python semantics (sequential, last write wins) — not discussed, just
  inherited.
- **Combinational loops:** static scheduler refuses *any* cyclic block dependency (even a
  false loop, e.g. two blocks that read and write disjoint fields of one struct); the
  event-driven scheduler accepts false loops and raises on real ones at runtime.

## 6. Properties stated and how established

- **No formal properties.** Verification devices:
  - pytest-based testing; **PyH2** property-based random testing with hypothesis
    (shrinking both the transaction sequence and the design parameters: "shrinking a
    50-transaction case for an eight-node network to a 10-transaction case for a
    four-node network").
  - **VerilogTBGenPass** "captures the cycle-by-cycle value change of the interface
    signals of a marked component, and generates a Verilog TB with assertions for use in
    pure-Verilog four-state RTL or gate-level simulation." → the Python simulation is the
    golden trace and the translated Verilog is checked against it. One-directional,
    per-design, user-invoked; no aggregate result reported.
  - **Import passes**: VerilogImportPass "leverages Verilator to generate a C++ simulator
    for all specified SystemVerilog files ... produce a C++ shared library" and the
    workflow (Fig. 1a) is "invokes translation backends to generate SystemVerilog code and
    import it back to PyMTL3 for co-simulation with the same TB". So sim-vs-translated-SV
    differential testing is a *workflow step*, not a claim.
  - Prototype proxy passes run the same TB against the FPGA/ASIC.
  - PyMTL2 pedigree: "three chip tape-outs in IBM 130 nm, TSMC 28 nm, and TSMC 16 nm".
- No numbers of the form "sim agrees with Verilator on N designs".

## 7. Relation between the semantics and the generated Verilog

- RTLIRGenPass → SystemVerilog backend (plus a Yosys-compatible variant that "inherits most
  code generation functions ... and only adds several hundred lines"). No correctness
  statement. The modelling/simulation gap is their *motivation* (framework papers that
  "still generate and simulate low-level HDL code ... creates a modeling/simulation
  language gap"), and their fix is co-simulation of the translated SV with the Python TB,
  i.e. differential testing as a habit.

## 8. (Survey question — n/a.)

## 9. Transfer to Copper — what fits, what does not

**Fits:**

- **The static-vs-event-driven simulator pair is Copper's Levelized-vs-fixpoint pair,
  including the loop policy.** PyMTL3 static: DAG + topological sort, refuses cyclic
  dependency graphs even when not real loops; event-driven: tolerates false loops, raises
  on real ones. Copper: SCC-condensed topological single pass, convergent comb loops
  iterated, non-convergent → panic; `COPPER_SCHEDULER=fixpoint` as the permanent
  differential oracle. Copper's treatment is *stronger* in two ways worth stating: (a)
  SCC condensation lets the levelized scheduler accept the false-loop case PyMTL3's
  static pass refuses; (b) Copper *pins* that the two schedulers agree (the poll-order
  fuzzer, the corpus run under both), which PyMTL3 never claims. The semantic sentence
  to write: "the combinational values of a cycle are the unique fixpoint of the
  update equations; any topological evaluation of the dependency DAG computes it; an
  SCC is iterated to convergence; the schedule is semantically invisible."
- **`@update_ff`/`<<=` vs `@update`/`@=` is Copper's `RegOut`/`Out` and the phase model.**
  Writes in ff blocks take effect at the edge; comb blocks settle after. PyMTL3's latch
  lint ("each signal written in the block has valid assignments in all conditional
  branches") is the *combinational* definite-assignment rule that Copper applies only to
  combinational modules — same split.
- **VerilogTBGenPass is the ancestor of `verify_with_verilator`.** Sim trace compiled into
  a testbench with assertions, stimulus and expected values baked in. Copper's
  content-keyed model cache (key over SV + testbench + params + Verilator version) is a
  refinement of exactly this architecture; citeable as prior art for "the testbench is
  generated from the simulator's trace".
- **Co-simulate the translated SV with the same TB** (Fig. 1a) is the workflow Copper's
  example `main()`s implement, generalized to every module by the sweep.

**Does not fit, and why:**

- **No suspension, no program counter.** A PyMTL3 block is re-run from the top every cycle;
  all state is in signals written by ff blocks. Copper's module body is a coroutine whose
  state includes *where it is* (`pc`) and every local live across a tick. Nothing in
  PyMTL3 addresses tick-free segments, control extraction, or registers by liveness. The
  closest PyMTL3 gets is "a signal written in an `@update_ff` block is a flop" — a
  block-kind rule, not a liveness rule.
- **Ordering is between blocks by sensitivity; Copper's problem is within a segment.**
  PyMTL3's answer to "when is an input read?" is "re-run the block until inputs are
  stable" — every read is of the settled value (the fixpoint anchor; what Copper's
  combinational mode's `delta_yield` loop does). Copper's `Immediate` read at a point in a
  sequential segment, followed by a register update and an `Out` write, has no PyMTL3
  analogue; the `d*`/`e*` batteries and the anchor rules exist because Copper permits it.
- **"Each simulation pass implements different modeling semantics"** is the position Copper
  must *not* take. PyMTL3 is the clearest published example of a framework whose semantics
  is "whichever simulator you applied" — cite it as the contrast: Copper has one normative
  semantics with two implementations (simulator, SV) that are required to agree, and a
  scheduler choice that is required to be invisible.
- **CL/FL method-based modelling and explicit ordering constraints** are irrelevant to
  Copper (no multi-level modelling; no user-visible scheduling constraints — Copper's
  stance is that a well-formed design is poll-order-independent, so there is nothing for
  the user to constrain).
- **Last-write-wins inside a block is just Python.** PyMTL3 inherits it silently; Copper's
  sequential `Out` written twice in a segment is also last-write-wins, but Copper must say
  what the *SV* does with the intermediate write (nothing, if both are opening drives; a
  hold-form mux if mixed) — PyMTL3 translates a block to an `always_comb`/`always_ff` with
  the same blocking order, so the question never arises for it. Copper's extra question
  comes from reads of `In` interleaved with the writes, which PyMTL3's block granularity
  cannot express.

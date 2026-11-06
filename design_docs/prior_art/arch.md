# Arch (Zhao, arXiv 2604.05983v3, April 2026) — how the paper specifies semantics

Source: `paper/references/arch_2026.pdf` (28 pp.). Section numbers below are the paper's.

## Headline

**The paper contains no formal semantics of any kind.** There is no grammar
beyond two illustrative listings, no inference rule, no typing rule, no
definition, no lemma or theorem with a statement and proof, and no IR. The
language's meaning is fixed by (a) prose descriptions of each construct,
(b) the SystemVerilog the compiler emits ("one Arch construct always produces
the same SystemVerilog structure", §2.1), and (c) an independently written
C++ simulator (`arch sim`) that is differentially tested against Verilator
running the emitted SV (§6.1, §7.1). Two places *claim* formal results
("structural theorem", "lowering equivalence proof") without stating or
proving them (§4.6, §7.3.4). The paper is a language/toolchain description
plus benchmark evaluation; for Copper's purpose (how to write a semantics) it
is a negative example and a mirror of Copper's own current bar.

## 1. Object given a semantics; grammar

- Object: the **surface language** only. §6.2: "The current compiler is a
  multi-phase pipeline: parse → elaborate → resolve → type-check → codegen,
  transforming the AST directly to SystemVerilog text without a dedicated
  intermediate representation." A dedicated IR ("AIR") and a CIRCT backend are
  future work (§6.2, §13). No kernel calculus; the catalogue of constructs is
  the language (module, pipeline/stage, fsm/state, fifo, arbiter, bus/handshake,
  thread, regfile, ram, synchronizer, counter, linklist, clkgate, latch,
  template, hook, function, package, generate_for/generate_if).
- Grammar: the paper asserts the grammar is "strictly LL(1)" (§5.1) but gives
  **no BNF**. The only grammar-like material:

  Listing 1 (§2.2), "Universal block grammar":
  ```
  keyword Name
    // body
  end keyword Name
  ```
  Listing 2 (§2.3), "Universal schema" — every first-class construct has four
  sections in order:
  ```
  keyword Name
    param NAME: const = value;   // 1: params
    param NAME: type  = SomeType;
    port name: in Type;          // 2: ports
    port name: out Type;
    // construct-specific body    // 3: body
    assert name: expr;           // 4: verify
    cover  name: expr;
  end keyword Name
  ```
  Table 1 (§3.1) lists primitive types: `Bit`, `UInt<N>`, `SInt<N>`, `Bool`,
  `Clock<D>`, `Reset<S,P,D?>`, `Vec<T,N>`, `struct`, `enum`, `Token` (0 bits).
  Body-level statements seen only in listings: `reg x: T reset rst=>0;`,
  `reg x: T guard VALID;`, `seq on clk rising ... <= ... end seq`,
  `comb ... = ... end comb`, `comb result = expr;`, `let x: T = e;`,
  `inst p: Mod  a <- e; ... end inst p`, `stall when e;`, `flush Stage when e;`,
  `-> State when e;` (fsm transition), `wait until e;`, `wait N cycle;`,
  `fork ... and ... join`, `for i in a..b`, `lock r ... end lock r`,
  `latch on ENABLE ... end latch`, `todo!`.

## 2. Style of semantics

- **Informal prose per construct** (§3–§4). E.g. §4.2: "The default block
  assigns busy = false and done = false at the top of the generated
  always_comb; individual states override only the signals that differ" — the
  meaning of an FSM is explained by what SV it becomes.
- **Semantics-by-translation to SystemVerilog**, explicitly adopted as a
  design principle ("Predictable RTL. One Arch construct always produces the
  same SystemVerilog structure. Designers can audit every output line", §2.1).
  The SV, under IEEE 1800, is the de facto semantics.
- **Type-system-only properties**, described in prose, no rules: width safety
  (§3.2), clock-domain tagging (§3.4), single-driver (§3.6), no implicit
  latches (§3.7), no combinational loops (§3.8).
- **A second implementation as oracle**: `arch sim` "independently generates
  compiled C++ models" (§7.1); "every code generation change is validated by
  comparing Arch-emitted SystemVerilog behavior against Verilator's independent
  interpretation of the same RTL" (§7.1). Note the asymmetry: what is compared
  is the C++ model vs. Verilator-on-SV; neither is a written definition.
- **A transition-relation encoding** (§7.4, `arch formal`): "lowers the AST
  directly to SMT-LIB2 ... Registers become per-cycle bit-vector variables,
  next-state logic is encoded as an ite chain, and each assert or cover
  becomes a per-cycle disjunction checked by a single check-sat call." This is
  the nearest thing to a mathematical model (a bit-vector transition system
  over per-cycle register copies), but it is a tool path, not a written
  semantics, and is limited to "flat modules with scalar signals and a single
  clock (no sub-inst, no Vec/struct/enum)".
- No operational, denotational, or axiomatic semantics; no relation between
  the prose, the SV, the C++ model, and the SMT encoding is stated other than
  testing. The paper's own prior-work citation RTL++ [22] is credited with
  "pipelined register variables with formally defined execution semantics
  (structural operational semantics)" (§12), which only highlights that Arch
  itself gives none.

## 3. Time model

- Synchronous clocked RTL: a `seq on clk rising` block is the edge; `comb`
  blocks and `let` bindings are combinational; `Clock<D>` carries a phantom
  domain `D`. Unit of observation: one 2-state value per signal per clock
  cycle ("signal values are integer-like and have no per-bit X/Z state", §7.2).
- **Micro-steps within a cycle are a bounded settle, not a fixpoint** (§7.3.1):
  "The compile-time DAG analysis statically determines a settle depth of 1 or 2
  for each module: depth 1 when sub-instance inputs are driven directly, depth
  2 when intermediate comb or let bindings in the parent feed instance inputs
  and a second pass is needed to propagate them. The generated code is
  therefore a fixed bounded loop, not a fixpoint iteration—unlike
  SystemVerilog's delta cycles." Table 4 row 1: "No combinational loops—static
  DAG → Bounded settle (1–2 passes), no fixpoint iteration."
- The pybind/cocotb path (§7.2) states the simulator's cycle discipline as a
  list of *deviations* from cocotb: "edges are detected by per-tick sampling
  rather than event callbacks, writes take effect immediately rather than in
  an NBA region, and there are no delta cycles—and are documented as known
  deltas from real cocotb." Writes from the testbench "are visible to
  model.eval() on the very next tick."
- Parallel-simulation determinism argument (§7.3.4) is the only place the
  phase structure of a cycle is spelled out: "(1) no write conflicts
  (single-driver rule), (2) register commit barriers (all next-state values
  computed before any update), and (3) DAG level barriers (level N+1 never
  starts before level N completes on all threads)." So a cycle = evaluate comb
  in topological levels → commit all registers.
- Multi-clock: domain-tagged, "separate clock domains run on dedicated
  threads, synchronizing only at CDC crossings" (§7.3.4); `--cdc-random`
  randomizes synchronizer latency (§7.1). No formal model of multi-domain time.
- Assertions: "An assert or cover body is a combinational expression evaluated
  at each clock edge under the construct's posedge clk with disable iff (rst)"
  (§7.4); `a implies b` lowers to `(!a || b)` per cycle; multi-cycle
  properties via explicit shadow registers (`reg req_d1 ... req_d1 <= req`).

## 4. State

- **By declaration keyword, not inference.** `reg` is a register; `comb`,
  `let`, `port` are wires; `seq on clk rising` uses `<=`; `comb` uses `=`.
  Intentional level-sensitive state needs `latch on ENABLE` (§3.7, §11.1.5,
  emits `always_latch`).
- Compiler-synthesized state: pipeline inter-stage registers and `valid_r`
  per stage (§4.1); `pipe_reg x_d2: x stages 2` (§12); thread lowering
  produces "an integer state register (_t{i}_state), a per-state
  combinational enable block, and a single merged always_ff block; counters
  for wait N cycle and for-loops" (§4.6 "Lowering"); fork/join arms get
  "done-bit registers per arm"; FSMs get a state register with one-hot or
  binary encoding chosen by param/flag (§4.2).
- Initial values: every register "declare[s] an explicit reset policy
  (including opt-out via reset none)" (§6.1); `reset rst=>0`; `--check-uninit`
  traps the first read of an unwritten `reset none` register at runtime; the
  `guard VALID_SIG` clause (§7.3.3) declares a reset-free data register
  meaningful only when a valid flag is high, suppressing the consumer-side
  warning and adding a producer-side check ("if the guard asserts ... but the
  data register has never been written, –check-uninit emits a runtime
  warning"). Table 6 enumerates every X source and its handling.
- The thread construct's classification (§4.6): "The compiler classifies every
  signal referenced inside any thread of a module as combinational-driven,
  sequential-driven, or read-only" — the criterion is not given.

## 5. Intra-cycle ordering, causality, forwarding

- Ordering across blocks: single-driver rule (§3.6) — "Every signal in Arch
  has exactly one driver. Multiple assignments to the same signal from
  different blocks are compile-time errors." Within an `always_comb` the FSM
  default block is "emitted before the state case statement, so states only
  override what differs" (§4.2) — i.e. last-assignment-wins, inherited from SV
  blocking-assignment semantics; not stated as a rule of Arch.
- Causality: no combinational loops (§3.8) — "The compiler builds a
  combinational dependency graph across all comb blocks, let bindings, and
  instance port connections, then performs a topological sort. Any cycle in
  this graph—a signal whose value transitively depends on itself within a
  single clock cycle—is rejected as a compile-time error with a path trace."
  Cycles are rejected outright; there is no constructive or fixpoint
  treatment, and no notion of a convergent loop.
- Completeness: no implicit latches (§3.7) — a `comb`-assigned signal must be
  assigned on all control paths.
- Forwarding: never articulated. Reads of a `reg` in `comb`/`seq` are the
  current (pre-edge) value, `<=` is next-state — ordinary RTL, left implicit.
  The one place where the question is live is `thread` (Listing 9):
  ```
  aw_valid = 1;  aw_addr = addr_r;  wait until aw_ready;  aw_valid = 0;
  ```
  The paper says `wait until cond` is a "state boundary that advances when
  cond is high" and nothing about in which cycle `aw_valid = 1` is visible on
  the port, whether `aw_ready` is sampled in the same cycle `aw_valid` first
  appears, or whether `aw_valid = 0` after the wait takes effect in the cycle
  the wait completes or the next. These are exactly the questions Copper's
  pre-tick alignment family answers; Arch leaves them to whatever the lowering
  does. (The auto-emitted thread SVA, `state==si && cond |=> state==next`, is
  a next-cycle implication, which suggests the wait's condition is sampled in
  state si and the state advances at the edge — but that is my inference, not
  the paper's statement.)
- Multiple writes per cycle to one signal from one block: not discussed.

## 6. Properties stated

- No theorem is stated with hypotheses, let alone proved. Three claims use
  proof vocabulary:
  1. §7.3.4 "Determinism Guarantee ... This is a structural theorem, not a
     best-effort property. A simulation passing on 1 core produces identical
     assertion results on 64 cores." Justified by the three barrier properties
     listed in §3 above; no statement of the theorem, no proof.
  2. §4.6 "Single-resource lock is therefore deadlock-free by construction: a
     thread is granted iff it requests, no thread holds a resource it did not
     request, and the arbiter's strict ordering rules out request cycles."
     Three-line argument for a fixed-priority arbiter.
  3. §4.6 "Each property is a corollary of the lowering equivalence proof for
     its construct: an assertion firing therefore evidences a compiler bug or
     a hand-edit of the lowered RTL rather than a user-program bug." No
     lowering equivalence proof appears anywhere in the paper.
- Validation is **testing**: Verilator as oracle for codegen (§6.1, §7.1);
  441/443 benchmark problems (VerilogEval v2 + CVDP, §11) pass reference
  testbenches, with a footnote that "These pass rates were measured while the
  compiler was actively under development ... the numbers are indicative
  rather than definitive"; EBMC bounded model checking of auto-emitted SVA
  (§7.3.3, §7.4); ten `arch formal` integration tests where "The three solvers
  agree."
- Nothing mechanized.

## 7. Relation between semantics and generated code

- The generated SV *is* the reference meaning, by design ("Predictable RTL").
  Correctness of translation is asserted as an output guarantee list (§6.1:
  no latches, no multiply-driven nets, no high-Z, no X from uninitialized
  state, no implicit CDC) and checked by "systematically verified against
  Verilator ... to confirm behavioral equivalence between Arch source
  semantics and the emitted RTL" (§6.1). Since "Arch source semantics" has no
  written form, this is a differential test between two implementations
  (`arch sim` C++ vs. Verilator-on-SV) — structurally the same bar as Copper's
  sim-vs-SV regression, with no third, written reference.
- `arch formal` is a *third* implementation of the meaning (AST → SMT
  transition relation), also related to the others only by the ten
  integration tests.

## 8. Exact locations

- Listing 1 (§2.2, p.3) block grammar; Listing 2 (§2.3, p.3) universal schema;
  Table 1 (§3.1, p.3) primitive types.
- §3.6 single-driver, §3.7 no implicit latches, §3.8 no comb loops (p.5).
- Listing 5 (§4.1) pipeline; Listing 6 (§4.2) FSM; Listing 9 (§4.6) thread;
  §4.6 "Lowering" paragraph (p.8) thread lowering.
- Table 3 (§6) compiler phases; §6.1 output guarantees; §6.2 no IR.
- §7.2 (p.13–14) simulator timing deltas vs cocotb; §7.3.1 settle depth;
  Table 4 (p.15) language invariants vs SV runtime; §7.3.4 (p.16–17)
  determinism argument; §7.4 (p.17) property scope and SMT encoding;
  Table 6 (p.16) X sources.
- No figure or section contains a rule, a definition, or a theorem to
  transcribe. The closest "definitions", verbatim:
  - Settle depth (§7.3.1): "depth 1 when sub-instance inputs are driven
    directly, depth 2 when intermediate comb or let bindings in the parent feed
    instance inputs and a second pass is needed to propagate them."
  - Cycle phases (§7.3.4): "(2) register commit barriers (all next-state
    values computed before any update), and (3) DAG level barriers (level N+1
    never starts before level N completes on all threads)."
  - Property scope (§7.4): "a combinational expression evaluated at each
    clock edge under the construct's posedge clk with disable iff (rst) ...
    a implies b is exactly SVA's a |-> b and lowers to (!a || b) at each cycle."
  - Formal encoding (§7.4): "Registers become per-cycle bit-vector variables,
    next-state logic is encoded as an ite chain."

## 9. Transfer to Copper

What is worth taking:

- **The `thread` construct is Arch's analogue of Copper's coroutine body**
  ("the body reads as straight-line code with wait statements at points where
  the design must pause, and the compiler lowers each wait to a state boundary
  in a synthesizable FSM", §4.6) — `wait until` ≈ a wait loop of
  `clk.tick().await`, `wait N cycle` ≈ a counted tick loop, `_t{i}_state` ≈
  Copper's `pc` from `control_extract`, "for i in 0..N (loop counter with one
  body state per iteration)" ≈ a tick-bearing `for`. Arch does not define its
  semantics; Copper already has more (reachability, register-by-liveness,
  alignment rules). The usable lesson is the *unasked question*: Listing 9 is a
  worked example of the opening/closing anchor ambiguity, and Arch's silence on
  it is a good motivating example for why Copper's rules exist.
- **Cycle phase vocabulary**: "register commit barrier" and "DAG level
  barrier" are two clean phrases for Copper's "pre-edge settle → edge" and
  levelized order. Copper's `SchedulerMode::Levelized` is the same device as
  Arch's static DAG, except Copper tolerates convergent comb SCCs (iterated)
  and Arch rejects all cycles; Arch's "settle depth 1 or 2 ... a fixed bounded
  loop, not a fixpoint iteration" is a *statement of a bound* that Copper could
  prove for its own levelized pass (no SCC → one pass).
- **The two-implementation differential bar is where Arch stops.** Arch
  defines meaning as "what the SV does, checked by a second simulator". Copper
  is at the same place operationally (sim ≡ Verilator) but has a design doc
  denotation (`CYCLE_DATAFLOW_SEMANTICS.md`) the paper-of-record could make
  normative. Arch shows what the paper reads like without that: every claim
  about timing is a claim about the compiler's output.
- **Claims to avoid**: "structural theorem" and "corollary of the lowering
  equivalence proof" with no theorem written down. If Copper states
  determinism / poll-order independence / sim≡SV, state the proposition and
  the evidence (sweep, fuzzer) honestly, as Esterel's paper does when it says
  proofs are elsewhere.

What does not transfer:

- Nothing formal to borrow: no rules, no grammar, no denotation.
- Arch's state is by keyword (`reg`); Copper's is by liveness inference. Arch
  has no concept corresponding to "a value live across a tick becomes a
  register", so its text gives no help on defining that.
- Arch has no forwarding story at all (reads are pre-edge register values;
  `comb` is plain SV blocking assignment). Copper's "a read sees a local write
  earlier in the same segment" and the opening/closing anchoring of an `Out`
  have no counterpart.
- Arch's `assert` model (combinational expression at each posedge, next-cycle
  via shadow regs) is the standard SVA fragment; it presupposes the one-value-
  per-cycle view and says nothing about intra-cycle phases.

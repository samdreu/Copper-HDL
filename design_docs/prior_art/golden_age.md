# A Golden Age of Hardware Description Languages — Truong & Hanrahan, SNAPL 2019

Source: paper/references/golden_age_hdl_snapl2019.pdf (text:
scratchpad/papers/golden_age_hdl_snapl2019.txt). Sections: 1 Intro; 2 Background (2.1
Digital Design, 2.2 Verilog, 2.3 Functional HDLs, 2.4 TRS, 2.5 HLS); 3 Intellectual
Challenges (3.1 Language Design: 3.1.1 Circuit, 3.1.2 Register-Transfer, 3.1.3 Virtual
Machine; 3.2 Compiler Infrastructure; 3.3 Formal Methods: 3.3.1 Execution Semantics,
3.3.2 Type Systems); 4 Vision (4.1 meta-programmed host, 4.2 core structural DSL, 4.3
sea of DSLs, 4.4 verification); 5 Conclusion. Figures: 1 (comb + state block diagram),
2 (Verilog edge detector), 3 (Clash `acc1` stream vs `acc2` state-passing), 4 (Bluespec
counter), 5 (SystemC counter). It is a position/survey paper with no formal content of
its own; its value is the taxonomy and the problem statements.

## 1–2. What it treats as "the semantics" of each language family, and in what style

The survey characterizes each family by **how it encodes state and time**, which it takes
to be the whole semantic question for synchronous HDLs (§2.3: "The fundamental problem
these languages face is integrating the concepts of time and state in order to enable
the description of sequential logic"; §3.1.2: "The fundamental issue in RTL language
design is the precise choice of semantics for abstracting the concept of a register").

1. **Verilog — simulation semantics.** §2.2: "The language was originally developed as a
   commercial verification and simulation product and was later adopted as a basis for
   logic synthesis. As a result, the semantics of the language are defined in terms of a
   hardware simulation being executed as a software program [57]." Modules "simulated
   using a dataflow execution model." Fig. 2 caption spells out the two `always` blocks:
   the `@(posedge clk)` one "enforces the state updates to be synchronous"; the `@(*)`
   one is "sensitive to changes to any input signal ... Because the second always block
   is sensitive to any input change, the semantics are defined asynchronously."
2. **Functional HDLs — two denotational encodings** (§2.3, Fig. 3, §3.1.2):
   - *State in the interface* (µFP's µ operator; Clash `acc2 in s = (s', out)`): "A
     function of this form describes the transition function of a finite-state
     transducer (FST), providing a basis for a simple hardware synthesis algorithm where
     the current state is stored in registers. This technique necessarily implies a
     synchronized state update because the next values of the state are produced at the
     end of the evaluation of a function. There is no means to specify a state update at
     any other time."
   - *Stream + delay* (Daisy, Hydra, Clash `acc1 in = out where out = (delay out 0) + in`):
     "Haskell's lazy evaluation can be used to construct infinite streams"; "The simplest
     compilation algorithm for this approach will insert registers to implement the
     behavior specified by the delay operator. This approach provides a convenient
     abstraction for working with the past values of the input, but prevents the user
     from explicitly managing state" (the compiler must discover that a partial
     computation, not the raw delayed inputs, is the efficient state).
3. **Term rewriting systems (Bluespec)** (§2.4, Fig. 4, §3.1.2): "circuits are described as
   a set of rewrite rules which are applied to the inputs and state values to produce the
   outputs and next state values. An important quality of TRS is that they model the
   non-determinism and concurrency that are intrinsically present in hardware."
   "abstract registers by mapping terms to synchronous storage elements and rewrite
   rules to combinational logic. This approach shares many similarities to the
   functional HDLs that encode state in the interface of a function. However, TRS faces
   a unique challenge because the technique introduces the possibility of conflicts
   during state updates." Fig. 4 caption: "Notice that the compiler must handle the data
   race between load and increment on value."
4. **HLS / "virtual machine abstraction"** (§2.5, §3.1.3): the input language "provide[s]
   some notion of unbounded resources such as an infinite register space"; the compiler
   does "a variant of register allocation"; SystemC Fig. 5 is a C program with
   `sensitive << clock.pos()`.
5. **Hardware construction languages (Chisel, Magma)** (§3.1.1): "embed the circuit
   abstraction into a general purpose programming language which provides a mechanism
   for meta-programming circuit definitions ... moving features related to
   parametrization and code generation to the host language simplifies the precise
   specification of the HDL."
6. **Coroutine FSMs (Silica)** (§3.1.2): "augment the semantics of the Verilog always block
   to describe a coroutine. The designer may suspend the procedure in arbitrary locations
   to incorporate more structure in the code. For example, the sequencing of two states
   can be achieved by separating the logic with a yield statement, and the looping of a
   state can be described using a while loop containing a yield." Challenge: "restricting
   the coroutine semantics to be only able to describe FSMs while still enabling the use
   of coroutine composition."

**Three abstraction levels** (§3.1): circuit (graph of circuits/ports/wires; structural;
"the fundamental primitive upon which all other HDL abstractions can be constructed";
"expressively complete" because agnostic to sync/async), register-transfer (registers
as language primitives "that behave similarly to variables in a standard software
programming language"), virtual machine (unbounded resources).

## 3–5. Time, state, intra-cycle ordering — the survey's view (§3.1.2 is the core)

- **The sim-vs-synthesis gap, stated three ways:**
  1. *Variable ≠ register.* "The Verilog specification is explicit in stating that a
     variable does not imply a hardware register [57], instead it is left to the
     synthesis tool to determine how the simulation behavior of an always block can be
     mapped into an implementation using concrete hardware registers. ... However, it
     also removes the ability for the user to explicitly specify the registers used in
     the synthesized design. In practice, this choice can result in a mismatch between
     the results of register synthesis and the designer's intent. To remedy this,
     Verilog design teams enforce style guidelines that restrict the usage of always
     blocks such that the synthesis results are transparent. An alternative design could
     use qualifiers to explicitly declare variables that should be hardware registers."
  2. *Sequential evaluation vs simultaneous update.* "A related issue is reconciling the
     synchronous update semantics of registers with the asynchronous update semantics of
     standard variables. For example, given standard imperative evaluation semantics,
     writing to two different variables in a procedure would happen at different steps
     in the evaluation. However, if both variables are mapped to hardware registers, they
     would be updated at the same time in the synthesized hardware. **One design choice
     would be to explicitly model the synchronized temporal update semantics of a
     register variable in the evaluation semantics of the language, ensuring that the
     user's model of the computation exactly matches the behavior of the synthesized
     hardware.**"
  3. *Blocking/nonblocking.* "Verilog's non-blocking assignments provide the capability to
     explicitly model synchronous storage. ... a non-blocking assignment does not block
     procedural flow and is performed near the end of a time-step. ... One might think
     that the semantics of non-blocking assignment could be simply to delay the
     evaluation until all blocking assignments have been completed. However, the
     evaluation of a non-blocking assignment will trigger an event on the variable being
     assigned, which in turn may trigger an event involving a blocking assignment. To
     handle this, Verilog's semantics include a loop for each time step that moves
     between blocking and non-blocking assignments until there are no events left to
     process. The complexity of these evaluation semantics can make it difficult to
     reason about a Verilog design involving both forms of assignment. In practice,
     Verilog design teams will follow style guides..."
- **Mealy vs Moore** (§2.1, Fig. 1): "a Moore machine can be viewed as a purely synchronous
  entity where changes to the state and output values are triggered by a clock, while a
  Mealey machine can exhibit asynchronous behavior where output values immediately
  respond to changes to input values. An expressively complete HDL will be able to
  describe and compose both Moore and Mealey machines."
- **FSM structure** (§3.1.2): the flat `case` pattern "exhibits a serious lack of structure"
  (25-state SDRAM controller), likened to `goto`; coroutines as the remedy; sequential
  composition of FSMs (Lava's Pace, Blarney Recipes, Bluespec STMTFSM, Silica coroutine
  composition).
- **TRS scheduling**: "The compiler could schedule the rules for each module separately, or
  it could lift the rules into a single top-level module which is then scheduled as a
  single unit"; "Minimizing the overhead of the compiler generated logic for scheduling
  rewrite rules is the key challenge."
- **Compiler passes** (§3.2): "hardware computations are always predicated ... Conditional
  logic is implemented using multiplexers on the flow of data. ... programs can be
  rewritten into single-static assignment form. ... this simplifies the synthesis stage
  of the compiler by enabling a one-to-one mapping from phi nodes to multiplexers."

## 6–7. Properties and sim/synthesis relation

- The survey reports no properties of any language; it reports that **Verilog's only
  definition is its simulator** (§2.2) and that teams cope with the synthesis gap by
  **style guides** (§3.1.2, twice). No differential-testing discussion.
- §3.3 (Formal Methods): "Formalizing the execution semantics of an HDL is essential
  requirement for the integration with formal tools such as model checkers. The major
  challenge is capturing the intrinsic concurrency and parallelism in hardware." Process
  calculi with discrete time; CSP for async circuits; "whether a core calculus can be
  constructed that captures the execution semantics of the circuit abstraction in the
  same way that the lambda calculus captures the semantics of function abstraction ...
  two key issues ... circuits can hold state and must have finite size. ... A type system
  can be used to enforce the finiteness of computation [49], which leaves the issue of
  managing state." The two functional encodings "restrict the language to describing
  synchronous circuits."
- Types (§3.3.2): product types are zero-overhead; sum types cost a discriminant;
  session/behavioural types for protocols; dependent types for "magic bit patterns".

## 8. Classification, the gap, and the recommendations (direct answer)

- **Classification of how HDLs define semantics:** (a) by a simulator (Verilog: "defined in
  terms of a hardware simulation being executed as a software program"); (b) by a host
  language's evaluation of a pure function — state-passing FST or stream/delay (functional
  HDLs); (c) by a rewriting system with nondeterministic rule choice (TRS/Bluespec); (d) by
  a virtual machine plus a compiler's resource mapping (HLS); (e) by elaboration to a
  structural graph (Chisel/Magma — "simplifies the precise specification of the HDL"
  because generation is offloaded to the host). The survey does not use the words
  operational/denotational; it classifies by *state-encoding mechanism and abstraction
  level*.
- **On the sim/synthesis gap:** the three quotes above. The diagnosis is that Verilog's
  simulation semantics (events, blocking/nonblocking, variables) is richer than the
  synthesizable subset and the mapping is left to the synthesis tool and to style guides.
- **Recommendations:** (1) "explicitly model the synchronized temporal update semantics of
  a register variable in the evaluation semantics of the language, ensuring that the
  user's model of the computation exactly matches the behavior of the synthesized
  hardware" (§3.1.2); (2) or use register qualifiers; (3) coroutines for FSMs with a
  restricted semantics that compiles exactly to a circuit (§3.1.2); (4) design new
  languages "with formal specification in mind" (WebAssembly as the model, §3.3); (5) a
  core calculus for the circuit abstraction handling state and finiteness (§3.3.1); (6) a
  **formally specified core structural DSL** as the common target: "The definition of this
  core DSL should be simple and precise ... An important requirement is that this core
  language be formally specified, which then provides a consistent basis for the formal
  specification of other DSLs in the system" (§4.2); (7) SSA/phi→mux lowering (§3.2); (8)
  verification in the host language (fault, §4.4).

## 9. Transfer to Copper — what fits, what does not

**Fits (the survey almost describes Copper's thesis):**

- **Recommendation (1) is Copper's design goal verbatim.** "Explicitly model the
  synchronized temporal update semantics of a register variable in the evaluation
  semantics of the language, ensuring that the user's model of the computation exactly
  matches the behavior of the synthesized hardware." Copper's evaluation semantics (the
  async executor with phased ticks: pre-edge settle → edge → post-edge settle →
  observation) *is* such a model, and the cycle-by-cycle sim ≡ SV bar is the "exactly
  matches" clause made testable. The paper can cite §3.1.2 as the stated goal and the
  sweep as the evidence that it is met for N modules.
- **The coroutine-FSM paragraph is the closest prior art, and its stated open challenge is
  what Copper's shape rules solve.** "Restricting the coroutine semantics to be only able
  to describe FSMs while still enabling the use of coroutine composition" = Copper's
  `check_reachability` (every loop path reaches a tick), `check_nested_loop_shape` (a
  nested loop ends at a clock boundary), `check_awaits` (only `clk.tick().await`), and
  `control_extract` (branch-nested ticks → `match pc`). "The sequencing of two states can
  be achieved by separating the logic with a yield statement, and the looping of a state
  can be described using a while loop containing a yield" is Copper's `tick` exactly.
  (Composition — "coroutine composition" — is the part Copper has *not* done: a
  structural parent is transpile-only and the simulator wires by hand; be honest about
  that when citing.)
- **The "variable ≠ register" gap class is the pretick alignment family.** Copper's answer
  is neither "leave it to synthesis" nor a qualifier: registers are *inferred* (liveness)
  and the shapes where the simulator's variable and the SV's register would disagree are
  *refused* with a spanned error in both front-ends (front-end parity). That is a third
  option the survey did not list and is worth presenting as such: "inference plus
  refusal of the divergent residue". `RegOut` is the qualifier option for outputs.
- **Blocking/nonblocking as "a loop per time step until no events"** is the model Copper's
  fixpoint scheduler implements and the levelized scheduler replaces by a canonical
  order; the survey's complaint ("difficult to reason about") is the reason Copper's
  semantics should be stated as a fixpoint *denotation* with the schedule invisible, not
  as the loop.
- **A formally specified core structural language as the target** (§4.2) matches the
  pipeline's VLIR ("1:1 with legal SystemVerilog — legalized names, concrete widths").
  The survey would have Copper give the SV side of its semantics as "the meaning of VLIR"
  and the lowering `FIR → control_extract → CHIR → SHIR → VLIR` as translations. The
  SSA/phi→mux remark (§3.2) is what `shir_lower`/`vlir_lower` do with branch merges
  (the "wire branch-merge" bug class in the transpiler-coverage notes).
- **Mealy/Moore must both be expressible and composable** (§2.1). Copper's plain `Out`
  (combinational / held) vs `RegOut` (registered) is that split; the valid/ready decision
  (option 1, Moore handshakes) should be stated against this sentence, since it means
  Copper's handshake *ports* are Moore-only by policy even though the language can
  express Mealy outputs.

**Does not fit / cautions:**

- **The survey asks for one semantics; Copper needs two and a relation.** "Circuit
  abstraction is the fundamental primitive" would make Copper's meaning "the netlist it
  lowers to" — but then "sim ≡ SV" is circular. Copper's correctness bar requires an
  *independent* semantics for the coroutine side (what a segment denotes, with anchors)
  plus the translation, plus the agreement claim. CλaSH/Bluespec give the shape
  (abstract semantics + implementation + criterion); the survey does not.
- **The FST remark cuts both ways.** "There is no means to specify a state update at any
  other time" is what the functional encodings give up; Copper *does* permit writes at
  points within a segment (a plain `Out` written mid-segment, an `In` read before or after
  a register update) and pays for it with the opening/closing anchor machinery and the
  hold-form emission. The survey's sentence is a good way to *introduce* why Copper's
  semantics has anchors at all: it is an FST whose transition function is a sequential
  program, and the program's reads and writes have to be assigned to the opening or the
  closing edge for the FST reading to be recoverable.
- **No formal device to borrow.** The survey proposes process calculi, a core calculus, and
  session types; none is worked out. Cite it for taxonomy and problem statements only.
- **Its vision is embedded-structural-DSL-in-Python with a dynamically typed host**
  (§4.1); Copper's choice (Rust, static types, phantom clock domains in the type system,
  `compile_fail` doctests as the executable CDC spec) is the opposite side of the
  survey's own trade-off ("A statically typed host language would provide increased
  safety ... a dynamically typed language would promote rapid design space
  exploration"). Worth one sentence of positioning, not more.

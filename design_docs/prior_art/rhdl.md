# RustHDL (Basu, LATTE 2024) and RHDL (Basu, LATTE 2025) — Rust-embedded HDLs

Sources: `paper/references/rusthdl_latte2024.pdf` and `rhdl_latte2025.pdf`; text in
`scratchpad/papers/rusthdl_latte2024.txt` (4 pp.) and `rhdl_latte2025.txt` (3 pp.).
Both read in full. Section numbers are the papers' own.

## 0. One-paragraph verdict

Neither paper gives a semantics. Both are *one-cycle update-function* HDLs: a module is a
struct of signals and sub-modules plus an `update` function that computes next values from
current values; there is no coroutine, no multi-cycle control flow, no suspension point.
RustHDL (2024) states a "mental model" in two sentences (§3) — `.val()` reads the current
value, the last write to `.next` wins and becomes `val` "in the next clock cycle" — and
relies on an event-based fixpoint simulator plus Yosys lints for latch detection. RHDL
(2025) replaces that with "the update function becoming pure, with no side effects ...
Feedback loops must be broken by registers" (RustHDL §3, previewing RHDL) and a
co-compiler (RHIF typed SSA → RTL untyped SSA → flow graph), with marker-type clock
domains (`Signal<b4, Red>`) that are the direct ancestor of Copper's `Clock<D>`/`In<T,D>`
phantom domains. Neither paper makes a simulation-vs-synthesis claim, reports a
differential test, or defines what a register, a cycle, or a combinational path means
beyond the mental-model sentences. The one empirical sim/synth remark is negative:
Verilator rejected RustHDL's generated Verilog (RustHDL §4).

## 1. What is given a meaning, and by what

### RustHDL (LATTE 2024)
- Principle (§2, boxed): "RustHDL designs are valid Rust programs that can be compiled
  and run on a host computer using the included event-based simulator." Meaning = the
  Rust program's behaviour under the bundled simulator.
- Hardware meaning = AST transformation to Verilog: "unlike MyHDL, RustHDL does not use a
  generator pattern and infer the required hardware. Instead, the AST itself is
  transformed into the circuit description ... The MUX is inferred from the imperative
  code" (§2). "RustHDL is a transpiler, which generates Verilog syntax that is matched to
  the allowed subset of Rust that RustHDL supports, stripping type information out of the
  AST, and providing small shims to convert, e.g., match statements into Verilog case
  statements" (RHDL §3).
- Mental model (§3): `Signal` has "a read only endpoint x.val() for signal x, and a write
  endpoint x.next". Boxed rule: "The current value of the signal is accessible via .val(),
  and the value that signal will take on the next clock cycle will be decided by the last
  assignment to .next. I.e., in the next clock cycle next -> val." Then: "Note that in the
  case of asynchronous combinatorial logic, the value of a signal is defined when next and
  val are equal. Local variables can be both written to and read, functioning as
  scratchpads, as long as a write precedes any subsequent reads or writes."
- Registers are explicit components: `state: DFF<SPIState>` "is equivalent to a module
  instantiation. The DFF is a flip-flop" (§2); the update function writes `self.counter.d.next`
  and reads `self.counter.q.val()`. Latch prevention is a coding idiom: the first line
  `self.counter.d.next = self.counter.q.val(); // latch prevention` (§3 listing).
- Rust's checks are the type system: "An entire class of potential bugs are eliminated,
  such as type mismatches, use-before-initialization, unassigned outputs, etc." (§2) and
  "Rust lacks write-only semantics, so the framework checks for read-before-write on the
  x.next endpoint" (§3). Latch inference is checked downstream: "analysis passes that look
  (with the aid of Yosys in RustHDL) for latch inferences due to missing assignments" (§3).

### RHDL (LATTE 2025)
- A co-compiler "runs alongside rustc to analyze the Rust source code and generate a
  series of HDL-compatible representations that are successively lowered" (§1); "Both
  compilers work together to ensure that the language invariants are met at all stages,
  and that undefined behavior is prevented" (§1). Type inference "must agree with the types
  inferred by rustc to avoid miscompilation" (§2.2) — the two-front-end agreement problem
  Copper solves with the shared `copper-analysis` crate, stated here as a risk.
- Lowering (§3): "RHDL Hardware Intermediate Form (RHIF), which is a strongly typed, static
  single assignment (SSA), register-based virtual machine instruction set" → "RTL, which is
  an untyped SSA register-based virtual machine instruction set" → "Flow graph, which is a
  netlist representation (which may not be directed if the design has loops)". Verilog is
  treated "like a machine-code target". `if`/`match`/blocks-as-values/early returns lowered
  to mux SSA with renaming (§2.3, worked example `r3 <- r1 ? r2 : r0`).
- Clock domains (§2.2): "a Signal type indicates the clock domain it belongs to by use of a
  marker type parameter (arbitrarily chosen to be a color). So, for example, Signal<b4,
  Red> indicates a nibble that changes on edges in the Red clock domain ... so that rustc
  will prevent inadvertent substitution." Adding a Red and a Blue value "fails to compile".
  "Clock domain crossings require special constructs provided in the RHDL core library."
- ADTs with compiler-controlled layout (§2.1); timing estimation (§2.4).
- Mental model (stated in RustHDL §3 as the RHDL plan): "RHDL simplifies the mental model
  by using the natural Rust data flow that arises from functions operating on value types.
  Data inputs are fed into functions, and data outputs are returned, with the update
  function becoming pure, with no side effects. Feedback loops must be broken by
  registers."

## 2. Style

Prose mental model + code listings + IR snippets. No rules, no event queue description, no
subset enumeration (RustHDL says only that its subset is "too small to write 'natural' Rust
code", §6; RHDL that "only a very small set of Rust code can be directly mapped to Verilog",
§2). Testing: Rust unit tests on the simulator (RustHDL §4); no differential testing.

## 3. Time model

- **RustHDL**: event-based simulator; a clock is a `Signal<In, Clock>` distributed by the
  `clock!` macro (§2 listing). A cycle is implicit in "in the next clock cycle next -> val"
  (§3). Combinational propagation: "because RustHDL supports combinatorial connections
  across modules that the simulator iterates until it reaches a fixed point. The
  iterations will terminate with an error if some upper limit is reached" (§4). That is
  the delta-cycle/fixpoint model with a hard cap — the same as Copper's `fixpoint`
  oracle mode, and the same as MyHDL's 1000-delta cap. No phases, no NBA region described.
- **RHDL**: no time model stated; the update function is pure, registers break feedback.
  Effectively `(state, inputs) → (outputs, next_state)` evaluated once per edge — the
  Mealy-machine kernel. Nothing about delta cycles, since there is no intra-cycle
  sequencing to resolve once the function is pure and acyclic ("Flow graph ... may not be
  directed if the design has loops" §3 is the only hint that comb loops can appear).
- **No suspension point exists in either language.** All control flow is within one
  cycle's update.

## 4. State

- Register = explicit `DFF<T>` component (RustHDL §2); in RHDL, presumably a typed
  register element in the flow graph (not described). No inference from control flow.
- Signals vs variables (RustHDL §3): signals have `.val()`/`.next`; locals are
  "scratchpads, as long as a write precedes any subsequent reads or writes" — i.e. locals
  must be definitely assigned before use within the update (checked how? not stated; Rust
  would reject use-before-init of a `let` but not a stale struct field).
- `.next` semantics: "A signal's .next endpoint can be written to as many times as desired
  inside of an update function. Only the last value it takes will matter when the function
  completes. In essence, the last successful write to a signal 'wins', where success may be
  conditional" (§3). Equals Verilog last-NBA-wins / MyHDL `.next`.
- Initial/reset values: not discussed in either paper.
- "Unassigned outputs" are said to be caught by Rust (§2) — unclear how, since `.next` is
  a field write; more plausibly by the Yosys latch lint (§3).

## 5. Intra-cycle ordering

- Locals forward (blocking); signals do not (`.next`, visible next cycle) — RustHDL §3.
- Combinational signals: "the value of a signal is defined when next and val are equal"
  (§3) — a fixpoint characterisation, enforced by iteration to convergence in the simulator.
- Multiple drivers: not discussed; the struct-of-signals design makes a signal's writer the
  owning module's update, so parallel drivers are unlikely by construction.
- Combinational loops: RustHDL iterates to a fixpoint with an error cap (§4); RHDL requires
  "Feedback loops must be broken by registers" (RustHDL §3). RHDL's flow graph "may not be
  directed if the design has loops" (§3) — so loops are representable and presumably
  rejected or lint-reported; not stated.
- **Sim/synth mismatch**: the only remark is that Verilator rejected RustHDL's output
  "(possibly due to the presence of inter-module asynchronous logic in the design)" (§4).
  Mismatch avoidance is by the latch-prevention idiom and Yosys lints (§3). The pure
  update-function model of RHDL removes the mismatch class structurally: a pure function of
  current state and inputs, with registers only at explicit `DFF`s, is exactly what
  synthesis infers, and simulating it once per edge is exactly what the hardware does — as
  long as the simulator schedules comb dependencies correctly, which is what the fixpoint
  loop is for.

## 6. Properties claimed and how established

- RustHDL: "used for non-trivial commercial firmware development and is deployed" (§6).
  Rust's compiler eliminates "an entire class of potential bugs" (§2). No property of the
  transpiler. "RustHDL is a reasonably fast simulator" (§4), no numbers; direct comparison
  with Verilator "proved difficult".
- RHDL: "meant to be a zero-cost abstraction" (§1); type pass "must agree" with rustc
  (§2.2); CDC prevented by marker types (§2.2 and Fig. 2). No evaluation, no metrics.
- Neither paper has a soundness or equivalence claim, a test corpus, or a formal statement.

## 7. Relation between simulation and synthesized hardware

Nothing beyond RustHDL §4's failed Verilator attempt and the Yosys-assisted latch lint.
No co-simulation, no equivalence checking, no differential sweep.

## 8. Transfer to Copper (concrete and critical)

### What fits
- **Phantom clock domains**: RHDL's `Signal<T, Red>` marker type is the same device as
  Copper's `Clock<D>`, `In<T,D>`, `Out<T,D>` with `D: ClockDomain`, including "crossings
  require special constructs" (Copper: `#[hardware(synchronizer)]`). Copper's
  `copper-core/src/cdc.rs` compile_fail doctests are the executable version of RHDL's Fig. 2
  error message. Cite RHDL as prior art for the nominal-type CDC check; Copper's addition is
  that a module cannot *construct* a clock and the synchronizer exemption is a module mode.
- **Rust-as-host, two front-ends**: RHDL names the agreement hazard ("must agree with the
  types inferred by rustc to avoid miscompilation") that Copper's single shared
  `copper-analysis` over `syn::ItemFn` is designed to remove. Copper's "front-end parity"
  audit (`copper-analysis/tests/front_end_parity.rs`) is the discipline RHDL does not
  describe having.
- **Last-write-wins on `.next` / `Out` holds when unwritten**: RustHDL §3 states the rule
  Copper's sequential `Out` follows (and the enabled-register idiom `if enable { d.next =
  q + 1 }` after a default assignment is RustHDL's canonical listing).
- **Fixpoint comb scheduling with an iteration cap**: RustHDL §4 = Copper's
  `COPPER_SCHEDULER=fixpoint` oracle. Copper's production `Levelized` scheduler (SCC-condensed
  topological pass, convergent loops iterated, non-convergent → structural panic) is the
  step RustHDL did not take; RHDL instead forbids loops ("must be broken by registers").
- **Latch prevention**: RustHDL's "assign default first" idiom plus a Yosys lint is the
  mitigation for the incomplete-assignment class. Copper's combinational mode has no latch
  problem because an unwritten sequential `Out` *holds* by definition (a register), and
  combinational modules are the `loop { ...; delta_yield }` shape; the hazard Copper does
  have is the pretick/forwarding family, which RustHDL/RHDL cannot express.

### What does not fit
- **No coroutine, no suspension point, no multi-cycle body.** Both languages are
  one-cycle update functions with explicit `DFF`s. They have nothing to say about what a
  `tick` means, about registers inferred from liveness, about `In` reads anchored to
  opening vs closing edges, or about plain `Out` written in several phases. Copper's
  `#[hardware(sequential)]` body with several `clk.tick().await`s is strictly outside
  their design space, as is `control_extract`. Where Copper and RHDL overlap (a
  single-tick body, `loop { ...; clk.tick().await }`) they coincide with RHDL's pure
  update function, and Copper's combinational mode coincides with a RustHDL update with no
  DFFs.
- **Registers explicit vs inferred**: RustHDL/RHDL require a `DFF<T>` field; Copper infers
  registers (`infer_registers`) and synthesises them. This is the central difference and
  is why Copper needs `check_register_initializers` (constant power-on values) where
  RustHDL/RHDL say nothing about initial values at all.
- **Verification posture**: neither paper has a differential oracle; Copper's
  `verify_with_verilator` + corpus sweep + anchoring to BaseJump is the thing the RHDL line
  lacks entirely. RustHDL's note that Verilator rejected its Verilog is a useful contrast:
  Copper's sweep requires every module to Verilate (and lint) or be `SKIP`ped with a reason.

### Critical take
RustHDL/RHDL are the strongest prior art for *Rust as the host language* and for
*type-level clock domains*, and the weakest for *semantics*: the mental model is two
sentences, the time model is "once per clock", registers are explicit components, and
correctness is delegated to rustc, a fixpoint simulator, and Yosys lints with no
sim-vs-synth check. Copper should cite them for the host-language and CDC devices and
position its coroutine/tick model, liveness registers, and swept equivalence as what they
do not attempt.

# Prost! Coroutine-based Hardware Description (Riedl, Scheipel, Baunach; LATTE 2026, 3 pp.)

Source: `paper/references/prost_latte2026.pdf`, text in `scratchpad/papers/prost_latte2026.txt`.
Read in full. Section numbers are the paper's (§1–§6); it has no page numbers beyond 1–3.

## 0. One-paragraph verdict

Prost is the closest prior art to Copper's core idea — a coroutine *is* a single-clocked
synchronous circuit — and it states that idea as an equivalence in one sentence (§2) and
then immediately admits it "is not perfect (see Section 5)". No semantics is written down;
the meaning of a Prost task is given (a) by an informal operational reading ("at each
positive clock edge, we advance the coroutine until its next suspension point", §2) and (b)
by the Verilog the compiler emits (Listing 2). The design choice that makes Prost simple is
that **everything is anchored to the clock edge**: locals are registers, all outputs are
`.next`-written registers that become visible "after the next wait", and the per-state
code is compiled to a combinational `advance` function. Prost therefore has no plain
combinational output driven from inside a clocked task, no opening/closing distinction,
and no pre-first-edge execution — the three things Copper's pretick/forwarding rules exist
for. There is no simulator-vs-synthesis check, no evaluation, no property, and the
rejection criterion ("each loop contains at least one wait and runs at least once") is
acknowledged as a heuristic to be formalised later.

## 1. What is given a meaning, and by what

- **The coroutine model**: "We consider stack-less, first-class, asymmetric coroutines
  [de Moura & Ierusalimschy 2009]: By calling a coroutine, we transfer control to it. At
  any point, the coroutine may suspend itself, returning control back to its caller while
  maintaining its state. The caller is then responsible for resuming or aborting the
  suspended coroutine" (§2). Meaning of the host construct is by citation to a PL paper.
- **The hardware reading**: "Coroutines are equivalent to single-clocked synchronous
  circuits: Local variables correspond to registers, control flow maps to next-state
  logic, and variable updates describe the data path. We begin execution at the entry
  point on reset, and at each positive clock edge, we advance the coroutine until its
  next suspension point" (§2). This single sentence is the semantics. It is followed by:
  "While this equivalence of coroutines and synchronous circuits is not perfect (see
  Section 5), it motivates the idea..."
- **The language**: "Prost ... is semantically similar to, e.g., SystemC, but avoids
  supporting non-synthesizable language constructs. Syntactically, Prost is inspired by
  Rust's async/await syntax" (§3). Semantics by reference to SystemC's SC_THREAD/wait()
  (§1: "This abstraction is equivalent to the cooperative multitasking found in SystemC
  using SC_THREAD and wait()").
- **The compiled form**: "Calling a task returns a value representing its current
  (initial) state. This state can be passed to an advance function associated with the
  task. This function then either returns the task's return value if it ran to
  completion, or a new state that can be further advanced on suspension. This
  interpretation of coroutines greatly simplifies synthesis: The values representing a
  task state can be mapped directly to registers, and the advance function can be
  compiled to combinational logic" (§3). So the denotation of a task is a Mealy/Moore
  step function `advance : State × Inputs → State + Return`, and hardware = (state
  register, advance as comb logic). Listing 2 is the emitted next-state logic.

## 2. Style

Informal operational prose plus a worked compilation example (Listing 1 → Listing 2).
No rules, no event queue, no subset enumeration beyond "avoids ... non-synthesizable
constructs" and the loop heuristic. Testing-based: none. The paper is a position/vision
paper.

## 3. Time model

- **Cycle = one advance of the coroutine to its next suspension point, executed at the
  positive clock edge** (§2). The `wait` statement "suspends the task's execution until
  the caller resumes it" (§3); the caller is the clock: "Contrary to the wait statement,
  where the caller waits until the next cycle, the .wait expression waits for a called
  task to complete" (§3).
- **Two kinds of suspension**: `wait;` (cycle boundary) and `task(args).wait`
  (await a sub-task's completion — which may itself take many cycles, each of its internal
  `wait`s being a boundary). Calls are inlined into the caller's state space: "The other
  two states correspond to the single wait statement in the wait_n task, which is
  reachable via two separate calls in the mac task" (§4). So a callee's `wait` becomes a
  distinct caller state per call site — states are (call-site path, program point), i.e.
  full inlining, no shared sub-FSM. Compare Copper's `structural` hierarchy, which keeps
  submodules separate; Prost replaces "module instantiation" with "coroutine call" (§1)
  and flattens.
- **Reset**: "The RESET state corresponds to the initial state at the task entry ... The
  RESET task will be entered on an external reset. On the next positive clock edge, it
  updates all registers according to the code between the task entry point and the first
  wait statement, before transitioning to the corresponding new state" (§4). So the
  opening segment (entry → first wait) is *not* executed at time 0 or during reset; it is
  executed at the first clock edge after reset like every other segment. There is no
  pre-first-edge phase at all.
- **No delta cycles, no event regions**: the model is a pure synchronous step. Nothing
  happens "between" edges except that the combinational advance function is a function of
  current inputs and state.
- **Multi-clock**: not supported; "Prost may support to explicitly wait on different clock
  signals" in future (§5). SystemC's domain splitting is cited as the existing solution.
- **Combinational circuits**: "modeling pure combinational circuits via regular functions
  is a goal, combinational cycles are currently not expressible. If this capability is
  desired, e.g., for modeling latches, it needs to be reconsidered" (§5).

## 4. State

- **Registers**: "Local variables correspond to registers" (§2) — *all* locals, no liveness
  analysis mentioned. The task state value = program counter + locals (§3). Listing 2 shows
  `acc`, `byte`, `i`, `cycles` and `state_reg` as the registers; `byte` and `i`, which are
  dead once the inner `for` finishes, are still registers and are explicitly re-zeroed
  (`byte = 0; i = 0;`) in the transition back — the compiler re-initialises them in the
  caller's state rather than treating them as dead. Copper's `infer_registers` (register
  iff defined-in-loop ∧ live-across-a-tick) is strictly finer.
- **Signals**: ".in and .out suffixes are used to represent I/O signals; their current
  value can be accessed using the .val projection. Output signals can be modified using
  the .next projection, which will affect all .val accesses after the next wait" (§3). So
  an output is a register written with NBA-like semantics; reading `acc.val` after
  `acc.next = x` in the same segment still yields the old value (`acc.next = acc.val *
  rx_byte(rx).wait` relies on `.val` being the registered value across the multi-cycle
  `.wait`). **There is no plain combinational output.** Every `.out` is Copper's `RegOut`.
- **Inputs**: `rx.val` is read where it executes; since the segment's code is the
  combinational `advance` evaluated at the edge, the input value is the one present at
  the edge that *ends* the segment (closing-anchored). Listing 2 line 16 `byte[i] = rx;`
  samples `rx` inside the clocked next-state logic.
- **Initial/reset values**: given by the code before the first `wait` (`acc.next = 0`),
  executed at the first edge after reset (§4). Registers have no stated power-on value
  otherwise; Listing 2's RESET arm assigns every register (`acc = 0; byte = 0; i = 0;
  cycles = 250; cycles = cycles - 1;`), so reset is a full assignment of state — the
  SystemC "reset behaviour = code before the first wait" rule (systemc.md §4.2.2) adopted
  verbatim.

## 5. Intra-cycle ordering

- **Within a segment, locals forward (blocking semantics)**: Listing 2 compiles
  `cycles = 250; cycles = cycles - 1;` and `byte[i] = rx; i = i + 1; if i < 8 ...` as
  sequential blocking statements; the `if` tests the *updated* `i`. The whole case
  statement is next-state logic (registers get the final value at the edge), so this is
  the standard "blocking inside a combinational next-state block, then register" form —
  exactly Cummings-safe because all of it is one block and the registers are updated once.
- **Outputs do not forward**: `.next` writes "affect all .val accesses after the next
  wait" (§3). Last write wins (implicit, by sequential execution of blocking assignments
  into a next-value variable).
- **Multiple drivers**: cannot arise — an output belongs to one task and tasks compose by
  call, not by parallel instantiation.
- **Combinational loops**: "not expressible" (§5), by construction (no comb outputs).
- **Control flow**: "all control flow has been unrolled into nested if-else-blocks in
  Verilog: Just as software can support all control flow with conditional jumps, this
  means we can support any control flow construct" (§4). The FSM state set = reachable
  suspension points along call paths; the state transition function enumerates, per state,
  every path to the next wait.
- **Sim/synth mismatch mechanism and avoidance**: the paper does not discuss a mismatch;
  the architecture avoids it by making the simulator and the synthesised circuit both be
  "evaluate advance at each edge". A software simulation is "compiling it into a software
  program" (§2) — presumably executing the same advance function. If the simulator is
  literally the coroutine runtime (resume the coroutine at each edge), then the only way it
  could disagree with Listing 2 is if input reads or `.next` visibility were timed
  differently — and the paper gives no such phase structure, so there is nothing to
  disagree about. The uniformity (everything closing-anchored, everything registered) is
  what buys agreement.

## 6. Properties claimed and how established

- Claimed: coroutines "are equivalent to single-clocked synchronous circuits" (§2) —
  immediately qualified as "not perfect".
- Claimed: "we can support any control flow construct" (§4) — by the unrolling argument.
- Claimed benefit: "A cycle-accurate simulation of a design implemented using coroutines
  can be performed by compiling it into a software program" with "source-level debugging"
  (§2) — no measurement, no comparison against the emitted Verilog.
- Future work (§5): rejecting unsynthesisable code is undecidable in general ("Determining
  statically whether a program reaches a certain state is undecidable"), so "we envision
  the compiler using a heuristic ... which must be well-defined and computationally
  efficient while rejecting as few valid programs as possible. Currently, the compiler
  requires each loop to contain at least one wait statement and to run for at least one
  iteration. Future work shall develop more precise heuristics and also formally verify
  their correctness." No property is established.

## 7. Relation between simulation and synthesized hardware

None stated beyond §2's "cycle-accurate simulation" benefit. No differential testing, no
testbench, no equivalence check, no example of the simulator. Listing 2 is presented as the
compiler output, not as something checked against a simulation.

## 8. Transfer to Copper (concrete and critical)

### 8.1 How Prost decided what a suspension point means

- `wait` = "the caller waits until the next cycle" (§3) = Copper's `clk.tick().await`.
  The segment between two waits is compiled to combinational next-state logic evaluated at
  the closing edge; all effects (register updates, `.next` output updates) land at that
  edge; the next segment starts from the new state. This is precisely Copper's
  `control_extract` → `match pc` FSM, with one difference: Prost **inlines callee tasks
  into the caller's state space** (a `.wait` on a sub-task adds that task's wait points,
  per call site, as caller states — §4), whereas Copper's `check_awaits` allows only
  `clk.tick().await` and composes modules structurally.
- **What Prost refuses**: anything not reaching a `wait` inside a loop ("each loop to
  contain at least one wait statement and to run for at least one iteration", §5) — the
  same rule as Copper's `check_reachability` ("every loop path must reach a
  `clk.tick().await`") and `check_nested_loop_shape`, but with the extra "runs at least one
  iteration" clause (so a loop's first `wait` is on every path from the loop entry, i.e. a
  `while cond { ... wait }` whose body may be skipped is *rejected*; Copper's CFG-based rule
  admits a loop that is skipped as long as the enclosing loop path still reaches a tick).
  Combinational cycles and multi-clock are refused by being inexpressible.

### 8.2 Devices that fit

- *tick = edge; tick-free segments as FSM states*: identical.
- *registers from locals*: Prost takes all locals; Copper infers by liveness. Prost's
  Listing 2 re-zeroing of dead `byte`/`i` is the visible cost of not inferring.
- *`RegOut`*: Prost's `.out` with `.next` is exactly Copper's `RegOut` (write takes effect
  at the first edge after it executes; `.val` reads the register). The 2026-10-23 "RegOut
  rule DECIDED: takes effect at first edge after the write" matches Prost's "affect all
  .val accesses after the next wait".
- *closing-anchored input reads*: Prost reads inputs in the advance function at the edge.
  This is the simplest consistent anchoring and is what Copper's `trailing_regout_reads_input`
  / `trailing_out_reads_input` rules push towards when they refuse a trailing drive from an
  input read directly.
- *reset as the opening segment*: Prost executes entry→first-wait at the first edge after
  reset (§4). Copper executes the opening segment at time 0 before any edge (pre-tick), and
  constant initializers reach the SV as `initial` (2026-10-29). Prost's choice makes the
  opening segment just another state and eliminates D1 by construction, at the cost of
  one dead cycle after reset and of having no "combinational-before-the-first-edge"
  behaviour at all.

### 8.3 Devices that do not fit / where Copper is richer

*(Overtaken 2026-11-04: D-8 adopted Prost's one-job model, and R8 removed the plain `Out` of a clocked
body, so the contrast below no longer holds. See SPEC_DECISIONS.md D-8 and `review/Q1_TRADEOFFS.md`.)*

- Prost has **no plain `Out`**. Copper's plain `Out` inside a sequential body (a
  combinational output driven from within the clocked process, visible immediately in the
  simulator) is the source of the entire D1/multi_phase/linear-route rule family; Prost
  cannot express it and so has nothing to say about it. A reader comparing the two should
  note that Copper's complexity here is the price of the feature, and that Prost's
  simplification (all outputs registered) is one of the "rejected fixes" in spirit — it
  is the design decision Copper memory records as "option 1 DECIDED 2026-10-21: handshakes
  stay registered/Moore (RegOut); Mealy ready refused" applied globally.
- Prost has no simulator phase structure (pre-edge settle / edge / post-edge settle /
  observation); Copper does. Prost's model cannot express a combinational passthrough from
  input to output within a cycle.
- Prost flattens hierarchy into one FSM; Copper keeps `structural` parents and per-module
  SV.
- Prost's `.wait` on a sub-task returning a value (`rx_byte(rx).wait`) is a multi-cycle
  *call* with a return value — a feature Copper lacks (Copper has no cross-module call;
  everything is ports). This is a real expressiveness advantage for sequential algorithms
  and is where Prost's "coroutines compose cleanly and reduce duplication in the FSM
  description" (§6) comes from.
- No differential testing, no sweep, no anchoring — the paper offers no evidence that
  Listing 2 matches the coroutine's software execution; Copper's regression is exactly the
  evidence Prost lacks.

### 8.4 Critical take

Prost is a clean statement of the "coroutine = synchronous FSM" thesis with the simplest
possible anchoring (everything at the closing edge, everything registered), and it is
candid that the equivalence is imperfect and that rejection is heuristic. It is the right
citation for "the idea is in the air" and for "the all-registered simplification"; it is
not a source of semantics, and it contains no sim-vs-synth methodology. Copper's defensible
delta is (i) liveness-based register inference, (ii) the plain-`Out` / forwarding model and
the rules that keep it sound, (iii) the measured, swept equivalence.

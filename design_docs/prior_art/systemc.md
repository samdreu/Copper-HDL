# Accellera SystemC Synthesizable Subset, Version 1.4.7 (March 2016, 60 pp.)

Source: `paper/references/systemc_synthesis_subset_1_4_7.pdf`, text in
`scratchpad/papers/systemc_synthesis_subset_1_4_7.txt`. Read in full. Citations are the
standard's own section numbers and printed page numbers (p.N). Annex A is informative.

## 0. One-paragraph verdict

This is a *pure subset standard*: it gives no semantics of its own and says so — it "is
defined within the existing ISO C++ and IEEE 1666 SystemC specifications" (Intro p.iv;
§1.1 p.1), and every construct is classified **Supported / Ignored / Not Supported /
Supported with Restrictions** (§1.3.3 p.2–3) with the meaning of a Supported construct
being "as specified in the SystemC LRM" (§4.2 p.11). Its contribution to the sim-vs-synth
question is therefore entirely *negative*: it removes from the language everything whose
C++ or SystemC meaning is implementation-defined, undefined, unspecified, or event-driven
in a way synthesis cannot reproduce, and it names the mismatch risk each time ("to avoid
accidental latching, which would lead to synthesis and simulation mismatches", §4.1.1 p.10;
"The presence of undefined behavior may lead to differences between simulation and
synthesis", §1.5.2 p.3). For clocked threads it fixes the *shape* — one clock edge, at
least one reset, only `wait()`/`wait(const int)`, reset code = everything before the first
`wait()`, an infinite loop with no `break` out of it and no `return` — and leaves what a
`wait()` *means* to IEEE 1666. It then disclaims cycle-exactness twice: "the result of the
synthesis should have the same functionality as the input model, but may have different
sequential timed behavior" (§3 p.6) and, in the informative annex, "there is no guarantee
the pre-synthesis simulation of a design will match the post-synthesis simulation of the
resulting RTL" (A.2 p.52). For Copper this is the most instructive document on *how to
write a subset normatively*, and the clearest example of a standard that gives up on
cycle agreement rather than specify it.

## 1. What is given a meaning, and by what

- **Base standards** (§1.3.1 p.1): ISO/IEC 14882:2003 (C++03), 14882:2011 for `long long`
  and truncation-toward-zero division, IEEE Std 1666-2011 (SystemC LRM). "Hence,
  familiarity with these standards is presumed" (§1.1 p.1).
- **Normative vocabulary** (§1.3.2 p.1–2): *shall* (mandatory), *should* (recommended),
  *may* (permitted). *Accept* vs *interpret*: "A synthesis tool is said to accept a SystemC
  construct if it allows that construct to be a legal input; it is said to interpret the
  construct ... if it accepts that construct and produces a corresponding synthesis
  result" (p.2). *Synthesis refinement*: "imposes a restriction or alteration upon some
  other standard ... in order to subset the otherwise supported methods to describe
  behavior to a set that can be implemented in hardware" (p.2).
- **Construct categories** (§1.3.3 p.2–3): **Supported** — "A synthesis tool shall interpret
  the construct." **Ignored** — "shall accept the construct, but may choose not to interpret
  the construct" (e.g. `printf`, `cout`, `sc_trace`, `sc_assert`, §15 p.48–49). **Not
  Supported** — "may choose not to accept the construct. The behavior of the synthesis tool
  upon encountering such a construct is not defined in this standard ... a design that uses
  such constructs runs the risk of losing portability." **Supported with Restrictions** —
  within restrictions Supported; violating instances "are either Ignored or Not Supported.
  Unless explicitly categorized as Ignored, the violating instances are Not Supported."
- **What the standard does not define** (its own list): the top-level module selection
  (§3.1.1 p.7); how translation units are given to the tool (§2.1 p.5); tool directives /
  pragmas (Intro p.iv; §2.2 p.5); how cycle-accurate regions are marked (A.2 p.52);
  verification methodology (A.2 p.52).
- **Semantics of Supported constructs** = the LRM's. The thread section is explicit: "They
  are Supported, as specified in the SystemC LRM, when the following restrictions regarding
  the process sensitivity and process body are met" (§4.2 p.11). Nowhere does the document
  state what `wait()` does, what `sc_signal::write` does, or when a read sees a write — all
  of that is IEEE 1666 (evaluate/update phases, delta cycles, signal update at the end of
  the evaluation phase). A reader needs the LRM to know that a `write()` is visible to
  reads only after the next update phase, that a CTHREAD resumes at its clock edge, etc.

## 2. Style

Subset-restriction, clause-by-clause over the C++ and SystemC standards' tables of
contents, with normative *shall* language, informative NOTE boxes, and examples labelled
"for the sole purpose of demonstrating the syntax and semantics ... It is not the intent of
this standard to demonstrate, recommend, or emphasize coding styles" (§1.4 p.3). No
operational rules, no event queue, no properties. Each restriction, where motivated, is
motivated by *portability among conforming HLS tools* and by *avoiding sim/synth
differences*.

## 3. Time model

### 3.1 Processes (§4 p.10–14)
- `SC_METHOD`, `SC_THREAD`, `SC_CTHREAD` Supported with Restrictions; `sc_spawn`,
  `sc_process_handle` and all dynamic process control (`suspend`, `resume`, `kill`, ...)
  Not Supported (§4 p.10). `sc_event` Not Supported (§5.3 p.16); `next_trigger` Not
  Supported (§15.2.7 p.49); `sc_clock` channel Not Supported but clock *ports* are
  (§5.1.3 p.15, `sc_in_clk`, `sc_in<bool>`).
- **SC_METHOD** (§4.1 p.10–11): "all paths through an SC_METHOD shall either be
  combinational or sequential. An SC_METHOD with a mix of combinational and sequential
  paths is Not Supported." *Combinational*: "the sensitivity list here shall be static and
  include all signals that are read in the body of the method. This is done to avoid
  accidental latching, which would lead to synthesis and simulation mismatches" (§4.1.1
  p.10). *Sequential*: "shall consists of two mutually exclusive branches based on the
  clock and reset conditions" (§4.1.2 p.10); sync reset → sensitive only to one clock edge;
  async reset → that edge plus one reset edge (§4.1.2.1–2 p.10–11). `reset_signal_is` Not
  Supported for methods.
- **SC_THREAD / SC_CTHREAD** (§4.2 p.11–14):
  - **Clock and reset** (§4.2.1 p.11): "A process shall be statically sensitive to exactly
    one clock edge and shall have at least one reset specification. A process may have at
    most one synchronous reset specification and at most one asynchronous reset
    specification. Different SC_(C)THREADs may be sensitive to different clocks and resets,
    i.e., an SC_MODULE may contain multiple clocks." `reset_signal_is` /
    `async_reset_signal_is` Supported.
  - **Suspension** (§4.2.2 p.12): "a design shall only use a call to wait(), where the wait
    condition is the clock edge to which the thread process is sensitive ... the only
    Supported waits are the following. • wait() • wait(int): where the integer argument is
    statically determinable. Other forms of suspending a thread process are Not Supported."
  - **Body shape** (§4.2.2 p.12): `[<optional reset behavior>] [<optional operational
    behavior>] <infinite loop>`. "The reset and operational behavior shall be separated by
    a call to the wait() function. Any behavior encountered in the thread process prior to
    encountering the first call to the wait() function shall be considered by the synthesis
    tool as reset behavior. Any behavior encountered ... after ... the first call to the
    wait() function shall be considered ... operational behavior." If the first `wait()` is
    inside the loop, "the behavior within the loop body prior to the first call to the
    wait() function shall be considered by the synthesis tool to be both reset behavior and
    operational behavior." "the first call to the wait() function shall be statically
    determinable ... may be within conditional constructs only when the condition is
    statically determinable ... Subsequent calls to the wait() function (in the operational
    behavior) may occur within conditional constructs." "Reset values shall be statically
    determinable constants." Allowed loop forms enumerated: `while(1)`, `while(true)`,
    `do{}while(1)`, `do{}while(true)`, `for(;;)` (p.13).
  - **Loop exits** (§10.5 p.37): "a break that exits the infinite loop of an SC_CTHREAD or
    SC_THREAD process is Not Supported"; "a return statement that occurs within the
    function defining an SC_CTHREAD or SC_THREAD process is Not Supported"; `goto` Not
    Supported; exceptions Not Supported (§10.7 p.38). `continue` Supported.
- **What happens at the boundary**: not stated here. By the LRM, a CTHREAD suspended at
  `wait()` resumes in the evaluation phase following its clock edge; signal writes it made
  before suspending were committed at the update phase of the delta in which it wrote;
  reads return the current (committed) value. The subset only guarantees the *shape* that
  makes this correspond to "one state per wait". Between waits, the thread runs C++ with
  immediate variable semantics and deferred signal semantics (LRM).
- **Cycle accuracy is explicitly not promised**: §3 p.6 "the result of the synthesis should
  have the same functionality as the input model, but may have different sequential timed
  behavior." Annex A.2 p.52: "SystemC-based HLS typically needs to accommodate two
  different semantics of timing. Portions of a design are described in a time-independent
  manner — allowing the synthesis tool to schedule operations for specific clock cycles.
  Portions of a design are described in a timing accurate matter ... The SystemC language
  has no constructs to distinguish where cycle-accuracy is intended and where a synthesis
  tool has degrees of freedom to re-order operations. The practical implication of this is
  there is no guarantee the pre-synthesis simulation of a design will match the
  post-synthesis simulation of the resulting RTL. ... existing synthesis tools have adopted
  tool-specific methodologies for expressing when a segment of code is intended to be
  treated as cycle-accurate. Standardizing how these blocks of code should be specified is
  beyond the current scope of this standard."
- A.4.4.3 p.59 (informative, behavioral level): "The specification of behavior is in the
  form of an implicit-state machine ... there is no explicit state variable ... the behavior
  consists of a process that is sensitive to the clock and possibly a reset signal ... and
  wait statements that specify cycle timing among sets of output assignments. ... The
  verification methodology of the generated specifications against the behavioral (source)
  specification is more complex (than the RTL level vs. Gate-Level specification) since the
  cycle-by-cycle behavior may be changed by synthesis."
- A.4.3 p.57 defines **Cycle Accurate (CA)**: "possible to predict the state of the model
  in any given cycle at the external boundary of the model and, thus, establish a
  one-to-one correspondence between the states of the model and the externally observable
  states of a corresponding RTL model in each cycle" — and notes "The CA modeling style is
  not addressed in detail in the SystemC LRM."

## 4. State

- **Registers**: never defined normatively. Informatively, A.4.4.3 p.59: "Registers are not
  explicitly defined, but instead are determined by synthesis. Storage requirements are
  dependent on how operations are scheduled: registers are used to store values that are
  used one or more cycles after the cycle in which they are generated." — an informal
  statement of liveness-based register inference, the same criterion as Copper's
  `infer_registers` (live across a tick), with the caveat that in HLS the schedule itself is
  tool-chosen.
- **Signals vs variables** (§3.1.3.2 p.7): "Within a module, processes shall only
  communicate with each other through member variables which are of type sc_signal.
  Non-const variables which are not of sc_signal type shall be read and written by only one
  process." Modules communicate only via `sc_in`/`sc_out` bound to `sc_signal` (§3.1.3.3
  p.8); `sc_inout` Not Supported (§5.2.1 p.15). `sc_signal` only with `SC_ONE_WRITER`
  (`SC_MANY_WRITERS` Not Supported) and only `write`/`read`/ctor/`operator=` (§5.1.1 p.15).
  Resolved/4-valued channels Not Supported; `sc_logic` X and Z constants Not Supported
  (§5.1.2 p.15; §6.3.4.2 p.22). `sc_buffer`, `sc_fifo`, `sc_mutex`, `sc_semaphore`,
  `sc_event_queue` Not Supported (§5.1.3 p.15).
- **Initial / reset values**: "Reset values shall be statically determinable constants"
  (§4.2.2 p.13). Constructor writes to data members Not Supported: "writing to data
  members other than initialization of SC_MODULE pointers, signals, or ports of a module
  from inside the SC_MODULE constructor are Not Supported" (§3.1.3.5 p.9) with NOTE
  "Initialization of an SC_MODULE data member should be performed within the reset portion
  of an SC_THREAD, SC_CTHREAD, or sequential SC_METHOD in order to have the initialization
  be performed at reset time." And the sim/synth warning (§4.2.2 NOTE p.13–14): "Default
  constructors of data members of the SC_MODULE that initialize the data members (e.g., the
  default constructor for sc_int initializes the value to 0), are not part of the reset
  behavior. If the behavior of the design depends on those non-reset initial values, then
  synthesis results might differ from simulation results." The example (p.14) shows signals
  of a class type whose default ctor zeroes a field: "Warning! Synthesis will Not invoke
  default ctor for X." — the C++ power-on value is a simulation artefact; hardware has none
  unless the reset segment writes it.
- **Internal representation freedom** (§6 p.17): "Synthesis may choose alternative data
  representations for internal objects (not part of the interface of the design) provided
  the I/O behavior of the design is unchanged." I/O behaviour is the only observable.

## 5. Intra-cycle ordering, drivers, loops, mismatch

- **Forwarding vs deferred**: inherited from the LRM — C++ variables immediate, `sc_signal`
  writes deferred to the update phase. The subset enforces that every cross-process value
  is a signal (§3.1.3.2), so the only race-able thing (a shared variable) is forbidden by
  the one-process rule.
- **Multiple drivers**: forbidden by `SC_ONE_WRITER` (§5.1.1 p.15) — a single-writer
  check that the simulator itself enforces at run time (LRM), so sim and synth agree by
  construction.
- **Last write wins**: LRM semantics for `sc_signal` (the value at the update phase is the
  last written in the evaluation phase); not restated.
- **Combinational loops**: not discussed. The comb SC_METHOD rule (static, complete
  sensitivity) prevents the *latch* mismatch, not loops.
- **Undefined / unspecified behaviour as a mismatch source** (§1.5.2–1.5.3 p.3–4; §9 p.34):
  "The presence of undefined behavior may lead to differences between simulation and
  synthesis and differences in results from different synthesis tools. Unless this standard
  provides a synthesis refinement that provides a definition, such undefined behavior is Not
  Supported. In some cases, undefined behavior is assumed to be not present in the design.
  One specific example is division by zero (0). Synthesis tools may assume that such a
  condition will not be present ... and treat such a condition as a don't care." Unspecified
  behaviour (argument evaluation order; `i = x[i++]`, §9 p.34): "Expressions that are legal
  in ISOC++, but whose behavior is unspecified due to order of evaluation or order of side
  effects, are supported for synthesis. Synthesis tools are permitted to interpret such
  expressions in any way that is compliant with the ISOC++ standard" + NOTE "User code
  should not use such expressions to avoid differences in the results between simulation
  using a particular compiler and synthesis using a particular synthesis tool." Errors
  (out-of-bounds bit/part select, §6.3.5.1–2 p.22–23; §1.6.3 p.4): "Synthesis tools assume
  that such conditions will not be present and treat such condition as a don't care, unless
  the synthesis tools are able to prove that the error is present."
- **Synthesis refinements that pin down an implementation-defined meaning**: signed right
  shift shifts in the sign bit (§6.1.3 p.19–20 — "a synthesis refinement on ISOC++ and
  ISOC++11, since these standards leave the behavior implementation-defined when E1 is
  negative"); ASCII assumed for char literals (§6.1.1.1 p.18); two's complement only
  (§6.1.1.2 p.18); tool "shall warn in case the choice of bit-width or the representation is
  not consistent with the definition for the computer platform" (Table 1, p.18); floats Not
  Supported because "bit accurate behavior of floating point arithmetic is dependent on
  implementations and compiler options" (§6.1.4 p.20); fixed-point division Not Supported
  because of implementation-dependent precision limits (§6.3.5.5 p.25); `sc_int`
  representation >64 bits Not Supported (§6.3.1 p.21); shift amounts must be in range,
  statically determinable return widths (§6.3.5.10 p.26–27); bit-reversal part selects Not
  Supported (§6.3.5.2 p.23); `reversed` Not Supported (§6.3.5.17 p.29). Each of these is a
  case where C++/SystemC left a hole and the subset either closes it with a refinement or
  excludes it — the methodology Copper's "restrict-don't-grow" direction follows.
- **Static determinability as the recurring criterion**: `wait(int)` argument, first
  `wait()` position, reset values, array bounds, pointer targets (§8.3.1 p.32: "Statically
  determinable implies the synthesis tool is able to determine the actual object whose
  address is contained by the pointer"), `new` array sizes (§9.4.4 p.35), virtual call
  receivers (§12.10.3 p.41), part-select lengths, shift amounts for width-changing ops.
  Recursion Not Supported (§9.1 p.34).

## 6. Properties claimed and how established

- None proven. The standard's claims are about *portability* ("portable among all
  conforming HLS tools", Intro p.iv) and *what a conforming tool shall do* with each
  construct. The only functional statement is §3 p.6: synthesis "should have the same
  functionality as the input model, but may have different sequential timed behavior" —
  note *should*, not *shall*, and the explicit timing disclaimer.
- Equivalence/verification is out of scope: "The large body of existing techniques in
  testbench design and verification methodology which experienced designers use to validate
  functional correctness is beyond the scope of this document" (A.2 p.52). A.4.4.2 p.59
  notes that for RTL "The verification methodology of the output from RTL synthesis against
  the reference RTL specification is well defined for both combinational and sequential
  hardware" (citing IEEE 1076-2004) — i.e. equivalence checking exists for the RTL subset,
  not for behavioral threads.
- Conformance language: §1.4(f) p.3 explicitly denies that the examples "represent a
  compliance test suite".

## 7. Relation between simulation and synthesized hardware

- Simulation semantics = C++ + IEEE 1666 under a particular compiler/platform (§1.5.1 p.3:
  "Simulation of the design is based on C++ compilers on specific computer platforms ...
  synthesis tools are required to either adhere to those behaviors or provide warnings").
- Synthesis semantics = the tool's, constrained to "same functionality" at the I/O
  boundary, with internal representation freedom (§6 p.17) and sequential timing freedom
  (§3 p.6, A.2 p.52, A.4.4.3 p.59). The RTL-style subset (SC_METHOD comb/seq; CTHREAD with
  fixed waits) is implicitly cycle-accurate; the behavioral style is not, and the document
  admits the language cannot tell them apart.
- No differential testing is mentioned; the whole burden is on "testbench design and
  verification methodology".

## 8. Transfer to Copper (concrete and critical)

### 8.1 How SystemC decided what a coroutine suspension means, and what it refuses
- **Meaning**: `wait()` = "wait for the one clock edge this process is sensitive to"
  (§4.2.2); the content of that meaning (resume after the edge in the next evaluation
  phase; signal writes visible after the update phase) is the LRM's, not the subset's.
  `wait(N)` with static N = N edges. Nothing else may suspend: no events, no `wait(time)`,
  no `wait(event)`, no `next_trigger`, no dynamic sensitivity. This is exactly Copper's
  `check_awaits` ("only `clk.tick().await` may be awaited") and the one-`Clock<D>`-parameter
  rule, stated in the normative *shall* register. `wait(int)` with a static argument is
  Copper's `for _ in 0..N { clk.tick().await }`.
- **Reset = the opening segment**: everything before the first `wait()` is reset behaviour,
  with constant reset values, and the first `wait()` must be statically placed. Copper's
  analogue is the pre-loop prefix with `check_register_initializers` (constant initializer
  → SV `initial`, else refused). The difference is *when it runs*: SystemC executes the reset
  segment while reset is asserted and then waits for an edge; Copper runs the opening
  segment at time 0 before any edge and lets it drive plain `Out`s combinationally (the D1
  problem). SystemC's "first wait statically determinable" rule is a *shape* rule that
  guarantees a unique reset state; Copper's `control_extract` tolerates branch-nested ticks
  and resolves them into a `match pc` FSM, which is strictly more permissive and is why
  Copper needs the alignment rules SystemC does not.
- **Loop shape**: infinite loop of five syntactic forms, no `break` out of it, no `return`
  in the thread, no `goto`, no exceptions. Copper's `check_reachability` ("every loop path
  must reach a `clk.tick().await`") and `check_nested_loop_shape` ("a nested loop must end
  at a clock boundary") are the semantic versions of these syntactic restrictions; Copper
  allows `break` inside a tick-bearing `for` (fixed 2026-09-02) where SystemC's subset only
  forbids `break` out of the *outer* infinite loop and allows it in inner loops — so the
  two are close, with Copper's rule phrased over the CFG rather than over syntax.
- **Single writer, signal-only communication**: `SC_ONE_WRITER`, variables private to one
  process, modules talk only through ports — Copper's `In`/`Out`/`RegOut` port ownership
  and `multi_write_collapse`.
- **Complete static sensitivity for comb methods** "to avoid accidental latching" — Copper's
  combinational mode has no sensitivity list to get wrong (the executor derives
  dependencies), so this mismatch class is removed by construction rather than by rule.
- **Power-on values are not reset values** (§4.2.2 NOTE): the C++ default-constructed
  value exists only in simulation. Copper took the opposite decision on 2026-10-29: a
  constant initializer *does* reach the SV as `initial`, so the simulator's power-on value is
  the hardware's (for FPGA-style init; `COPPER_VERILATOR_X_INITIAL=1` measures what is left
  unstated). SystemC's rule is the ASIC-safe one (BaseJump makes the same point: ASICs need
  explicit reset, basejump.md §3); Copper's is the FPGA-style one. Worth naming the choice
  and its consequence explicitly in the formal-esque definitions.
- **Static determinability** as the universal admissibility criterion (wait counts, reset
  values, bounds, pointer targets, widths) is the same principle as Copper's generic
  modules transpiling to parametric SV with widths fixed only at Verilate time, and
  `PARAMS` in `build.rs`.

### 8.2 What SystemC refuses that Copper admits (and must therefore specify)
- A plain combinational output driven from inside a clocked thread, visible before the
  next edge. In SystemC a CTHREAD's `sc_out.write()` is a signal write that commits at the
  next update phase of the *current* delta — i.e. visible to other processes in the same
  time step but before the next clock edge — which is in fact the same hazard shape as
  Copper's plain `Out` in a sequential body. The subset says nothing about it; HLS tools
  register all thread outputs in practice (the §3 "different sequential timed behavior"
  escape hatch). Copper chose to keep the immediate-visibility semantics and to make codegen
  match (phase C hold form, drive-level mux) or refuse (`unprotected_pretick_out_write`,
  `multi_phase_out_write`, `linear_out_write_anchor_mismatch`). This is the single largest
  place where Copper specifies what SystemC declines to.
- Cycle exactness itself. SystemC's subset is explicit that it does not promise it (§3,
  A.2). Copper's correctness bar *is* cycle exactness (sim ≡ Verilator cycle for cycle). The
  formal-esque definitions should say so in the normative register SystemC uses ("shall"),
  because SystemC shows what the alternative looks like: a subset that is portable but whose
  pre- and post-synthesis simulations are allowed to differ in timing.

### 8.3 Devices to borrow for the formal-esque definitions
- The four-way construct classification (Supported / Ignored / Not Supported / Supported
  with Restrictions) with the default "violating instances are Not Supported" — a clean
  frame for listing what `#[hardware]` accepts, what it silently drops (nothing, in Copper —
  a point worth stating), and what it refuses.
- "Synthesis refinement" as the named act of closing an implementation-defined hole in the
  host language (e.g. Copper fixing wrapping/truncation semantics of `Bits<N>` arithmetic
  where Rust would panic or where Verilog would widen).
- The explicit list of *where* UB/unspecified/implementation-defined behaviour of the host
  language leaks into sim/synth disagreement (§1.5–1.6, §9) — Copper's analogue is the
  subset-restriction direction memo ("restrict data surface, keep control surface").
- The reset-segment definition by position of the first `wait()` — a precise, checkable
  definition of "opening segment" that Copper could adopt as vocabulary (Copper's
  `CYCLE_DATAFLOW_SEMANTICS.md` two-anchor segment model is the richer version).
- The informative A.4.4.3 sentence on registers ("values that are used one or more cycles
  after the cycle in which they are generated") as a citable informal statement of the
  liveness criterion that `infer_registers` makes exact.

### 8.4 Critical take
The SystemC subset is the best model for *how to write the admissibility rules
normatively* and the clearest admission that a coroutine HDL can be made synthesizable
without being made cycle-exact. It supplies the shape of Copper's `check_awaits`,
reachability, single-clock, single-writer and constant-reset rules in *shall* form, but it
supplies no semantics (all deferred to IEEE 1666), no notion of opening/closing anchors,
no treatment of combinational outputs from threads, and no verification story — and it
explicitly declines to guarantee that simulation matches the RTL. Copper's claim is the
one the SystemC working group said was "beyond the current scope".

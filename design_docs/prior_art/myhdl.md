# MyHDL 0.11 manual (Jan Decaluwe, May 2019) — how it specifies (and fails to specify) semantics

Source: `paper/references/myhdl_manual.pdf`, text extracted to `scratchpad/papers/myhdl_manual.txt`.
Citations below give the manual's own section numbers and its printed page numbers (p.N).
Read in full (112 pp.). Chapters 10–12 (conversion, examples, reference) and 9.4–9.5
(co-simulation race/delta discussion) carry nearly all the semantic content.

## 0. One-paragraph verdict

MyHDL never writes down a semantics. The meaning of a MyHDL description is defined
(a) by the Python simulator (an event-driven kernel whose rules are only described by
analogy: "similar to always blocks in Verilog and processes in VHDL", ch.1 p.1), and
(b) for the convertible subset, by the Verilog/VHDL text the converter emits, whose
meaning is then whatever the target HDL simulator / synthesis tool gives it. The only
correctness statement is a single adjective — the converter produces "an equivalent model
in Verilog or VHDL" (§10.2 p.61) — with "equivalent" never defined, and the only check
is differential: run the same test bench on both sides and diff the print logs
(§10.11 p.70, §12.5 p.103). The manual is honest about one time-0 divergence (§10.14
p.72) and about the limits of its co-simulation delta-cycle handling (§9.4.2 p.57).
Crucially for Copper, the *synthesizable* subset of MyHDL has exactly one suspension
point per process, at the loop head — so MyHDL never had to decide what a mid-body
coroutine suspension means in hardware; it simply inherited the Verilog `always @(…)`
model.

## 1. What is given a meaning, and by what

- **The language is Python plus a library**: "MyHDL is not a language in itself. The
  underlying language is Python, and MyHDL is implemented as a Python package" (§3.5
  p.12). There is no grammar, no typing rules, no evaluation rules of its own.
- **Simulation meaning** = the behaviour of the `Simulation` class / `run_sim` (§3.1
  p.7–8; §12.1 p.91). The kernel is described operationally and by analogy only:
  generators are "resumable functions" and "In hardware simulations, there is also a
  higher order agent, the Simulator, that interacts with such resumable functions; they
  are called processes in VHDL and always blocks in Verilog" (§2.2 p.4). `Signal` is
  "roughly modeled after VHDL signals" (§3.2 p.8). `.next` assignment "is the MyHDL
  equivalent of the VHDL signal assignment and the Verilog non-blocking assignment"
  (§3.2 p.9). That analogy *is* the specification of the update phase; the manual never
  states when `.next` commits relative to other generators resuming.
- **Hardware meaning** = the emitted Verilog/VHDL. "The converter analyzes the code of
  each generator and maps it to equivalent constructs in the target HDL. For Verilog, it
  will map generators to always blocks, continuous assignments or initial blocks. For
  VHDL, it will map them to process statements or concurrent signal assignments"
  (§10.3 p.62). The conversion works "from an instantiated design that has been
  elaborated by the Python interpreter. The converter uses the Python profiler to track
  the interpreter's operation and to infer the design structure and name spaces. It
  then selectively compiles pieces of source code for additional analysis and for
  conversion" (§10.2 p.61) — i.e. meaning is assigned per generator source text, after
  Python elaboration, to a target-HDL construct. Synthesis meaning is then the
  third-party synthesis tool's (§10.2 p.61).
- **The convertible subset** (§10.4 p.63–66) is given as *lists of supported types,
  statements and built-ins*, each with a one-line restriction, not as rules. It is
  "much broader than the RTL synthesis subset which is an industry standard. In other
  words, MyHDL code written according to the RTL synthesis rules, should always be
  convertible. However, it is also possible to write convertible code for
  non-synthesizable models or test benches" (§10.4.1 p.63). So "convertible" is a
  strictly larger set than "synthesizable" and the manual leaves "synthesizable" to the
  RTL templates of ch.6 and to the downstream tool.

## 2. Style

Prose + analogy + examples + generated-code listings. Operational in spirit (the
simulator kernel), event-driven (sensitivity lists), subset-restriction for conversion
(lists of allowed constructs), and testing-based for correctness (`verify()` = diff two
simulation logs). No formal rules anywhere. The nearest thing to a rule is the
`always` decorator desugaring in §12.2.4 p.97:

```
def _gen_func():
    while True:
        yield event1, event2, ...
        _func()
```

which is a precise rewrite and is the actual definition of the synthesizable process
shape (see §8 below).

## 3. Time model

### 3.1 Events, sensitivity, `yield`

- "in MyHDL, the yielded values are used to specify the conditions on which the
  generator should wait before resuming. In other words, yield statements work as
  general sensitivity lists" (§2.2 p.4). "[yield] has a similar meaning as the wait
  statement in VHDL: the statement suspends execution of a generator, and its clauses
  specify the conditions on which the generator should wait before resuming" (§3.3
  p.10).
- Reference definition (§12.2.3 p.95–96): "When a generator executes a yield statement,
  its execution is suspended at that point. At the same time, each clause is a trigger
  object which defines the condition upon which the generator should be resumed.
  However, per invocation of a yield statement, the generator resumes exactly once,
  regardless of the number of clauses. This happens on the first trigger that occurs."
  Trigger objects: a `Signal` ("Whenever a signal changes value, the generator
  resumes"), `posedge`/`negedge` ("An edge occurs when there is a change from false to
  true (positive) or vice versa"), `delay(t)`, `join(...)` (all clauses), a generator
  (fork; resume when it returns, §7.2 p.34), and `None` (do-nothing trigger: resume
  immediately).
- Time is integer simulation time (`now()`, `run_sim(duration)`), with `delay()`
  advancing it. Simulation stops on `StopSimulation` or "No more events" (§6.2 p.25).

### 3.2 Delta cycles

The word "delta" appears only in the co-simulation chapter, and there it is the only
place the manual explains why race-free intra-timestep ordering matters: "In a typical
RTL code, some events cause other events to occur in the same time step. For example,
when a clock signal triggers some signals may change in the same time step. For
race-free operation, an HDL must differentiate between such events within a time step.
This is done by the concept of 'delta' cycles" (§9.4.2 p.57). The MyHDL kernel's own
delta ordering (evaluate all resumed generators, then commit all `.next`, repeat until
quiescent) is never spelled out; it is implied by the VHDL-signal analogy and by the
co-simulation implementation note that maps "MyHDL delta cycles onto real Verilog time
steps" with a factor of 1000: "For each MyHDL time step, 1000 Verilog time steps are
available for MyHDL delta cycles. In practice, only a few delta cycles per time step
should be needed. Exceeding this limit almost certainly indicates a design error; the
limit is checked at run-time" (§9.5.1 p.58). That is the closest the manual gets to a
statement about combinational convergence: a hard iteration cap, treated as a design
error.

### 3.3 The clock cycle

There is no clock in the language. A clock is an ordinary `Signal(bool(0))` toggled by
a generator (`@always(delay(10)) def clkgen(): clk.next = not clk`, §6.3.2 p.26), and a
"cycle" is whatever a generator sensitive to `clk.posedge` does between two resumptions.
Sequential RTL = "sensitive to a clock edge" (§6.3.1 p.25). The `always_seq(clock.posedge,
reset=reset)` template (§6.3.1 p.25) is the only construct that knows it is clocked, and
it knows it only to infer reset.

### 3.4 What happens at the boundary and in between

- In the synthesizable templates the boundary is the single `yield` at the loop head
  inserted by `always`/`always_seq`/`always_comb`; between boundaries the body runs as an
  ordinary Python function with no suspension (the decorated function "should be a classic
  function", §12.2.4 p.97). Reads of signals see the committed value; writes go to `.next`
  and commit after the body returns (by the VHDL analogy). Variables are immediate.
- In general (`@instance`) generators, a `yield` can sit anywhere, with any mix of
  clauses; the manual uses this for bus-functional procedures (§7.2 p.33–37), test benches,
  and the two-flop monitor idiom `yield clock.posedge; yield delay(1); print(...)`
  (§6.3.2 p.27 — note the manual itself sidesteps the post-edge race by waiting one time
  unit before reading).
- Nothing is said about the order in which several generators sensitive to the same edge
  run, beyond the VHDL analogy. The co-simulation chapter admits the general case is not
  race-free when the clock is driven from the HDL side and recommends the "chip
  interface" discipline: "when data is triggered at positive clock edges, it can safely
  be sampled at negative clock edges. Alternatively, the MyHDL data signals can be
  declared with a delay value, so that they are guaranteed to change after the clock
  edge" (§9.4.2 p.57). `Signal(val, delay=0)` (§12.2.2 p.93) is that mechanism.

## 4. State

### 4.1 Signals vs variables

- `Signal`: "Hardware languages use special objects to support deterministic
  communication between concurrent code. In particular, MyHDL has a Signal object which
  is roughly modeled after VHDL signals" (§3.2 p.8). Attributes `next` (read-write, "the
  next value of the signal"), `val` (read-only, "the current value"); all numeric/bitwise
  operators delegate to the current value; augmented assignment deliberately *not*
  implemented "as they would break the rule that the current value should be a read-only
  attribute"; and "when a Signal object is assigned to the next attribute of another
  Signal object, its current value is assigned instead" (§12.2.2 p.94). Only signals can
  be waited on.
- Variables: plain Python objects (`intbv`, `int`, `bool`, lists, dicts), mutated in place,
  immediately visible. `intbv` is mutable precisely so slice/index assignment works
  (§12.2.5 p.98). In generator code a variable is a scratch value; the converter maps it to
  a Verilog `reg`/VHDL variable assigned with blocking semantics (implied by §10.7; the
  manual never shows a converted variable explicitly).
- The manual's own hazard note on Python naming: assignment "merely creates a new binding
  of a name to a certain object"; the converter supports multiple assignments to one name
  only when "a consistent type and bit width is being used"; otherwise "a single assignment
  should be used when an object is created. Subsequent value changes are then achieved by
  modification of an existing object. This technique should be used for Signal and intbv
  objects" (§10.7.1 p.67).

### 4.2 What is a register

Never defined. In practice a register is a `Signal` written (via `.next`) inside a
generator sensitive to a clock edge, and the converter emits it as a `reg` assigned with
`<=` in an `always @(posedge clock, negedge reset)` block (§11.2 p.75: `count <= (count + 1)`).
There is **no register inference from generator-local state**: in the synthesizable
templates the body is a classic function, so no local survives across the `yield`; all
persistent state must be a `Signal`. (A general `@instance` generator *does* keep locals
across yields — `rs232_tx`'s loop index, §7.2 p.34 — but such code is test-bench code and
is converted, if at all, to a Verilog `initial`/`always` block with embedded `@(...)`
and `#delay` statements, whose hardware meaning is undefined.)

### 4.3 Initial and reset values

- Every `Signal(val)` has an initial value and the simulator starts from it.
- `always_seq` "automatically infers the reset functionality. It detects which signals
  need to be reset, and uses their initial values as the reset values" (§6.3.1 p.25).
  `ResetSignal(val, active, isasync)` (§6.3.1 p.25; §12.2.2 p.94). The converted Verilog
  has no `initial`; reset values appear only in the reset branch (§11.2 p.75). Hence in
  the HDL the registers are X until the first reset, while the MyHDL simulator already
  holds the initial value — a pre-reset divergence the manual does not mention.
- The explicit template `@always(clock.posedge, reset.negedge)` requires reset values
  "to be specified explicitly" (§6.3.3 p.27).
- Combinational outputs: `always_comb` (§6.2 p.23) — see the time-0 issue in §7 below.

## 5. Intra-cycle ordering, drivers, loops, and the mismatch mechanism

- **Forwarding vs nonblocking**: variables forward (immediate), signals do not (`.next`
  commits later). Multiple `.next` writes to one signal in one body: last wins — the FSM
  example relies on it (`index.next = (index + 1) % FRAME_SIZE` then, in the SEARCH arm,
  `index.next = 1`, §6.4 p.29) and the converted Verilog preserves source order of `<=`
  (§11.5 p.83), so last-NBA-wins in Verilog gives the same answer. The manual never
  states the rule; it is inherited from Verilog/VHDL by the analogy.
- **Multiple drivers**: for simulation, nothing is said for ordinary `Signal` (the
  `TristateSignal` resolves drivers and warns on contention, §12.2.2 p.95). For
  conversion the converter "detects undriven and multiple driven signals ... it assumes
  that signals are not driven by default. It then processes the code to find out which
  signals are driven from where" (§10.9 p.68). So multiple drivers are a conversion-time
  error only; the simulator will happily run them.
- **Combinational loops**: not discussed. Only indirectly: the delta-cycle cap of 1000 in
  co-simulation "almost certainly indicates a design error" (§9.5.1 p.58).
- **Sensitivity inference**: `always_comb` "infers the input signals automatically. It
  returns a generator that is sensitive to all inputs" (§6.2.1 p.23). This is the
  mechanism that eliminates the classic Verilog incomplete-sensitivity-list mismatch.
- **The admitted mismatch** (§10.14 p.72, "Known issues"): "The semantics of always_comb,
  both in Verilog and MyHDL, is to have an implicit sensitivity list at the end of the
  code. However, this may not be synthesizable. Therefore, the inferred sensitivity list
  is put at the top of the corresponding always block or process. This may cause
  inconsistent behavior at the start of the simulation. The workaround is to create
  events at time 0." I.e. MyHDL's `always_comb` executes once at time 0 (sensitivity at
  the end = body runs first), the emitted Verilog does not (sensitivity at the top = waits
  for a change), so the two disagree until the first input event. This is exactly a
  time-0 / pre-first-edge divergence of the kind Copper's D1 family is about, and MyHDL
  documents it as a known issue with a user-side workaround rather than fixing or
  refusing it.
- **Other mismatch sources acknowledged**: Python ints map to 32-bit HDL integers (§10.14
  p.71); synthesis pragmas emitted as comments because Icarus does not support attribute
  lists on case (§10.14 p.72); VHDL `inout` port needed when an output is also read
  "to have a valid VHDL design that matches the behavior of the MyHDL design" (§11.2
  p.76).
- **Converted-code observations** (what the generator shapes actually become):
  - `always_comb` with a single assignment → continuous `assign` (§11.3 p.77: `assign G =
    ((B >>> 1) ^ B);`; §11.6 p.86: `assign dout = mem[addr];`).
  - `always_comb` with a ROM lookup → `always @(addr) ... case ... dout <= 17;` — a
    *nonblocking* assignment inside a combinational always block (§11.7 p.88), precisely
    the style Cummings' Guideline #3 warns against; harmless here only because there is one
    assignment and no intra-block reuse.
  - `always_seq` → `always @(posedge clock, negedge reset)` with `<=` (§11.2 p.75).
  - FSM `if/elif` on an enum signal → `casez` with `// synthesis parallel_case full_case`,
    one-hot encoding chosen at conversion time, `raise` → `$finish` in the default arm
    (§11.5 p.83–84).
  - Lists of signals → Verilog memory `reg [7:0] mem [0:128-1]` (§11.6 p.86).

## 6. Properties claimed and how established

- Claim: a convertible design converts to "an equivalent model" (§10.2 p.61). Established
  by: nothing formal. Two mechanisms: (i) the converter "also generates a test bench that
  makes it possible to simulate the Verilog design using the Verilog co-simulation
  interface. This permits one to verify the Verilog code with the same test bench used for
  the MyHDL code" (§10.11 p.70); (ii) `myhdl.conversion.verify(func, ...)` "converts MyHDL
  code, simulates both the MyHDL code and the HDL code and reports any differences"
  (§12.5.1 p.103), implemented by "comparing the HDL simulator output with the MyHDL
  simulator output" (§12.5.2 p.104) — i.e. diffing `print` logs. The manual says this
  "is used extensively in the package itself to verify the conversion functionality"
  (§12.5 p.103). This is a differential-testing oracle of exactly Copper's kind, but
  per-design and on user-written stimulus, not random, and comparing printed strings, not
  per-cycle port values.
- Methodology caveat, stated plainly: "there is no guarantee that it does a meaningful
  job unless the simulation runs fine" (§10.13.1 p.71). "Simulate first."
- Determinism of inter-generator communication is asserted ("deterministic communication",
  §3.2 p.8) but not argued; it rests on the VHDL signal model.
- No claim that the simulator is independent of generator scheduling order; no claim of
  cycle-accuracy of the converted code; no claim about synthesis.

## 7. Relation between simulation and synthesized hardware

- Three layers: Python simulation → converted HDL (checked by log-diff co-simulation) →
  synthesis (not checked; "a third-party synthesis tool is used", §10.2 p.61).
- The converter is a *source-to-source* mapping of generator bodies; it adds nothing that
  would make the HDL semantics match the Python semantics when they differ (the time-0
  `always_comb` case is the documented example). Where Verilog is weaker than MyHDL
  (signed arithmetic, enum encodings, case statements) the converter inserts the glue
  (§10.3 p.62–63).
- Co-simulation is only "race free" when MyHDL owns the clock (§9.4.2 p.57). Delta cycles
  are preserved MyHDL→HDL but not HDL→MyHDL: "The signals changes are only returned to the
  MyHDL simulator after all delta cycles have been performed in the HDL co-simulator"
  (§9.4.2 p.57). So the co-simulation oracle itself compares at time-step granularity,
  not delta granularity.

## 8. Transfer to Copper (concrete and critical)

### 8.1 How MyHDL decided what a suspension point means — and what it refused

- **In the synthesizable subset there is exactly one suspension point per process and it
  is at the loop head.** `always(...)` desugars to `while True: yield events; body()`
  (§12.2.4 p.97) with `body` a classic function. `always_comb` and `always_seq` are
  specialisations of the same shape. So the suspension point means "wait for the
  sensitivity event", nothing more — it is Verilog's `always @(...)`, and the hardware
  meaning (flop vs. combinational) comes from the *kind of event* (edge vs. level) and the
  downstream tool, exactly as in Verilog.
- **Consequence**: the register/wire question never arises from control flow. Everything
  that persists is a `Signal`; a local never crosses the boundary. There is no notion of
  "value live across a yield becomes a register". MyHDL therefore never confronted the
  problem Copper's `infer_registers`, `control_extract` and the whole pre-tick alignment
  family solve: a body with several `clk.tick().await`s in branches and loops, where a
  local is live across some ticks and not others and an `Out` is written in several
  phases. MyHDL's convertible-but-not-synthesizable multi-`yield` generators (test benches)
  are handed to the Verilog simulator as procedural blocks with embedded `@(posedge clk)`,
  which is legal simulation Verilog and not synthesizable; the manual makes no hardware
  claim for them.
- **What it refuses (the convertible subset, §10.4)**: unsized types; anything but
  `bool`/`intbv`/`enum`/`int`/lists thereof; `for` only over `range`/`downrange`; no
  `else` on loops; `yield` clauses only signals/edges/tuples/`delay`; generators must be
  created via the four decorators. These are *type-and-syntax* restrictions, not timing
  restrictions. The only timing restriction is structural: the synthesizable templates
  have the single loop-head yield.
- **Devices that fit Copper**:
  - *tick = edge*: MyHDL's `clk.posedge` trigger is the same device; but in MyHDL the clock
    is a plain signal and any edge of any signal is a trigger — Copper's `Clock<D>` with
    exactly one clock parameter per behavioural module is a stronger discipline MyHDL does
    not have (and SystemC's subset does: see systemc.md §4.2.1).
  - *`.next` / one value per signal per cycle*: MyHDL `Signal.next` is Copper's `RegOut`
    write (takes effect at the next commit); MyHDL `always_comb` + `.next` is Copper's
    combinational `Out`. MyHDL has no analogue of Copper's plain `Out` written inside a
    sequential body (a combinational output driven from within the clocked process) —
    in MyHDL that would just be another register. So the whole D1/multi_phase/forwarding
    problem class does not exist in MyHDL because the language offers no way to express
    it.
  - *reads anchored to opening vs closing edge*: MyHDL reads of signals inside a posedge
    body see the value committed before the edge (closing-anchored, the Verilog NBA
    model). The monitor idiom `yield clock.posedge; yield delay(1)` (§6.3.2 p.27) is the
    manual's way to read *post*-edge values — a time-based hack, with no language-level
    "post-edge observation" phase like Copper's.
  - *last-write-wins on `.next`*: same as Copper's sequential `Out` hold/overwrite
    semantics (and identical to Verilog NBA ordering, Cummings §15.3).
  - *`Out` holds when unwritten*: MyHDL signals hold when `.next` is not assigned; the
    enabled-register idiom (`if enable: count.next = count + 1`, §6.3.2 p.26) is the same
    as Copper's.
  - *sensitivity inference*: `always_comb` infers inputs; Copper's combinational mode
    wraps the body in `loop { ...; delta_yield().await }` and the levelized scheduler
    computes the dependency DAG — same intent, Copper's is global rather than per-block.
  - *differential verification*: `verify()` and the auto-generated co-simulation test
    bench are the direct ancestors of Copper's `verify_with_verilator`; the differences
    are that Copper compares every output every cycle against a compiled-in expectation,
    uses seeded random + exhaustive first-cycle stimulus, and runs it as a corpus-wide
    gate (G-D), where MyHDL compares `print` logs on hand-written stimulus per design.
  - *initial values*: MyHDL takes reset values from `Signal` initial values and emits them
    only in the reset branch; Copper (2026-10-29) emits constant initializers as SV
    `initial` and refuses non-constant ones. MyHDL's choice leaves a pre-reset X window in
    the HDL that the Python simulator never shows; the manual does not mention it.
- **Devices that do not fit**:
  - MyHDL's general `yield` (level sensitivity on any signal, `delay`, fork/join,
    multiple clauses) is a simulation-only feature set; Copper's `check_awaits` ("only
    `clk.tick().await` may be awaited") is exactly the refusal MyHDL *did not* make at the
    language level — it made it at the conversion level by what the templates look like.
  - MyHDL's variables-are-immediate/signals-are-deferred split is Verilog's blocking/NBA
    split with the race removed by construction (a signal has one writer generator in
    convertible code). Copper has no two-tier assignment; every local forwards, and the
    `In`/`Out`/`RegOut` port kinds carry the deferred/immediate distinction instead.
  - The time-0 `always_comb` divergence (§10.14) is a case where MyHDL *tolerates* a known
    sim/HDL mismatch and documents a workaround. Copper's stated bar is the opposite
    (refuse or fix; `out_first_driven_late`, D2 fix). Worth citing as the contrast.
- **Critical take**: MyHDL demonstrates the *cheap* way to keep simulation = synthesis for
  a coroutine-embedded HDL: make the synthesizable shape coincide with the host HDL's
  process shape (one wait at the loop head), move all state into explicitly declared
  signals, and let the target HDL's semantics be the spec. The price is that none of the
  coroutine's expressive power (multi-cycle algorithms as straight-line code) reaches
  hardware — multi-yield generators are test-bench-only. Copper's whole contribution lives
  in the region MyHDL fenced off, and the pre-tick alignment family is the cost of
  entering it.

## 9. Gaps in the manual worth citing

- No statement of the kernel's evaluate/commit ordering (delta cycle) for the MyHDL
  simulator itself; only for co-simulation.
- No statement of generator scheduling order within a time step or whether results are
  independent of it.
- No multiple-driver rule for simulation.
- No combinational-loop rule.
- No definition of "equivalent" in §10.2; no definition of "synthesizable" (deferred to
  "RTL synthesis rules", §10.4.1 p.63, and the templates of ch.6).
- The pre-reset X window in converted HDL vs initialised Python signals is not mentioned.

# Cummings, "Nonblocking Assignments in Verilog Synthesis, Coding Styles That Kill!" (SNUG San Jose 2000, Rev 1.2, 23 pp.)

Source: `paper/references/cummings_snug2000_nba.pdf`, text in `scratchpad/papers/cummings_snug2000_nba.txt`.
Read in full. Citations are the paper's section numbers (§N.0) and printed page numbers.

## 0. One-paragraph verdict

This is the canonical *informal* account of how Verilog's simulation semantics (the IEEE
1364-1995 "stratified event queue") and its synthesis semantics (structural inference by
the tool, here Synopsys and the IEEE P1364.1 draft) can disagree, and of the coding rules
that keep them aligned. Meaning is given by two external authorities — the IEEE 1364
standard's event-queue model for simulation and the synthesis tool's inference for
hardware — and the paper's contribution is to explain the gap between them with
eight-to-twelve minimal examples and to close it with eight guidelines. The mismatch
mechanism is precisely stated: a Verilog race is "two or more statements that are
scheduled to execute in the same simulation time-step, [that] would give different results
when the order of statement execution is changed, as permitted by the IEEE Verilog
Standard" (§2.0 p.2); synthesis does not see order, so it infers the same hardware for
every ordering while simulation gives order-dependent results. The repair is the
nonblocking assignment (`<=`), whose two-phase evaluate/update scheduling makes
same-edge register updates order-independent. The paper's correctness claim is a hedged
empirical one: "Following the above guidelines will accurately model synthesizable hardware
while eliminating 90-100% of the most common Verilog simulation race conditions"
(Conclusion, p.22).

## 1. What is given a meaning, and by what

- **Simulation semantics** = IEEE Std 1364-1995 [2], specifically §5.3 (stratified event
  queue), §5.4.1 ("Determinism"), §5.4.2 ("Nondeterminism"), §5.5 ("Race conditions")
  (§2.0 p.2; §6.0 p.5–6). The paper restates the model: "The 'stratified event queue' as
  described in the IEEE Verilog Standard is a conceptual model. Exactly how each vendor
  implements the event queues is proprietary" (§6.0 p.5). Four queues for the current time
  (active, inactive, nonblocking-update, monitor) "and additional queues for future
  simulation times" (§6.0 p.5). "Events are added to any of the event queues (within
  restrictions imposed by the IEEE Standard) but are only removed from the active events
  queue. Events that are scheduled on the other event queues will eventually become
  'activated,' or promoted into the active events queue. Section 5.4 of the IEEE 1364-1995
  Verilog Standard lists an algorithm that describes when the other event queues are
  'activated'" (§6.0 p.6). The one normative quote reproduced is §5.4.1 on NBA ordering
  (§15.3 p.21, see below).
- **Synthesis semantics** = what the synthesis tool infers; authority is IEEE P1364.1
  Draft Standard for Verilog RTL Synthesis [1] and, operationally, Synopsys behaviour
  (e.g. "Synopsys tools will report a syntax error because the blocking assignment is
  assigned to the same variable as one of the nonblocking assignments", §13.0 p.17;
  "Warning: In design 'badcode1', there is 1 multiple-driver net with unknown wired-logic
  type", §14.0 p.18). There is no statement of a synthesis semantics as such; synthesis is
  the oracle whose answer is reported per example ("will actually synthesize to a single
  register!", §8.0 p.8, Fig. 3).
- The paper itself gives meaning to nothing; it *relates* the two authorities on 26
  examples.

## 2. Style

Event-queue/operational (for simulation) and example-driven (for synthesis), culminating in
coding guidelines (a disciplined subset). Properties are demonstrated by exhaustive small
case analysis ("1 out of 4 blocking assignment coding styles was guaranteed to simulate
correctly; 3 out of 4 ... to synthesize correctly; 4 out of 4 nonblocking ... to simulate
correctly; 4 out of 4 ... to synthesize correctly", §8.0 p.11) and by quoting simulator
output (§15.1–15.2 p.19–20).

## 3. Time model

- **Time step and regions**: a simulation time step contains (Fig. 1, §6.0 p.6):
  - *Active events*: blocking assignments; evaluation of the RHS of nonblocking
    assignments; continuous assignments; `$display`; primitive input evaluation/output
    change — "These events may be scheduled in any order".
  - *Inactive events*: `#0` blocking assignments.
  - *Nonblocking assign update events*: "Update LHS of nonblocking assignments".
  - *Monitor events*: `$monitor`, `$strobe`, some PLI.
  - Future-time queues.
- **Blocking (`=`)**: "a one-step process: Evaluate the RHS ... and update the LHS ... without
  interruption from any other Verilog statement" (§3.0 p.3). It "blocks" trailing
  statements in the same block only.
- **Nonblocking (`<=`)**: "evaluates the RHS expression of a nonblocking statement at the
  beginning of a time step and schedules the LHS update to take place at the end of the time
  step. Between evaluation of the RHS expression and update of the LHS expression, other
  Verilog statements can be evaluated and updated and the RHS expression of other Verilog
  nonblocking assignments can also be evaluated" (§4.0 p.4). "a two-step process: 1.
  Evaluate the RHS of nonblocking statements at the beginning of the time step. 2. Update
  the LHS of nonblocking statements at the end of the time step." Only to register types,
  only in procedural blocks (§4.0 p.4).
- **Clock cycle**: not a first-class notion; it is "the same simulation time step, such as
  on the same clock edge" (§3.0 p.3) at which several `always @(posedge clk)` blocks are
  triggered in unspecified order. The *edge* is the moment all edge-sensitive blocks become
  active; the NBA update region at the end of that time step is the moment all registers
  take their new values "in parallel" from the user's view: "No matter which always block
  starts first after a reset, both nonblocking RHS expressions will be evaluated at the
  beginning of the time step and then both nonblocking LHS variables will be updated at the
  end of the same time step. From a users perspective, the execution of these two
  nonblocking statements happen in parallel" (§4.0 p.4).
- **Self-triggering / delta iteration**: an NBA update can re-trigger a block sensitive to
  the updated variable within the same time step (§7.0 p.7, `osc2`), which is how
  nonblocking-coded combinational logic needs "multiple passes through the always block"
  (§11.0 p.15, `ao5`). This is the paper's only mention of what Copper would call
  delta-cycle iteration, and it treats it as a performance smell, not a semantic device.

## 4. State

- **Register vs combinational is a synthesis-tool inference from the sensitivity list and
  assignment pattern**, not a language construct. The paper's framing: "Use blocking
  assignments in always blocks that are written to generate combinational logic";
  "nonblocking ... sequential logic" (§1.0 p.2) — the intent lives in the designer's head
  and the tool's inference, and the guideline is about making simulation agree with it.
- **Last NBA wins** (§15.3 p.21), quoting IEEE 1364-1995 §5.4.1: "Nonblocking assignments
  shall be performed in the order the statements were executed ... at the end of time-step
  1, the variable a will be assigned 0 and then 1." Paper's gloss: "Translation: 'Last
  nonblocking assignment wins!'"
- **Initial/reset**: only via the `if (rst) q <= 0; else q <= d;` pattern (§9.0 p.11–12)
  and async reset `always @(posedge clk or posedge rst)`. No discussion of power-on values.
- **Variables inside blocks**: a block-local `reg tmp` with blocking assignment feeding an
  NBA is legal and "will both simulate and synthesize correctly because the blocking
  assignment is not made to the same variable as the nonblocking assignments. Although this
  will work, the author discourages this coding style" (§13.0 p.17, `ba_nba2`) — i.e.
  intra-block forwarding temporaries are correct but disfavoured.

## 5. Intra-cycle ordering and the mismatch mechanism

### 5.1 The race
"A Verilog race condition occurs when two or more statements that are scheduled to execute
in the same simulation time-step, would give different results when the order of
statement execution is changed, as permitted by the IEEE Verilog Standard" (§2.0 p.2).

### 5.2 Mechanism, by example class
1. **Blocking across blocks on the same edge** (`fbosc1`, §3.0 p.3; `pipeb3`/`pipeb4`,
   §8.0 p.9): each `always @(posedge clk) qN = qM;` block is a flop to synthesis regardless
   of block order; in simulation the result depends on which block runs first. "This means
   that there might be a mismatch between the pre-synthesis and post-synthesis simulations"
   (§8.0 p.9). Mechanism: synthesis reads *structure* (one assignment per edge-triggered
   block = one register, data path = RHS), simulation reads *order*.
2. **Blocking within one block, wrong order** (`pipeb1`, §8.0 p.8): `q1 = d; q2 = q1; q3 =
   q2;` forwards `d` straight to `q3` in both simulation and synthesis ("will actually
   synthesize to a single register!", Fig. 3) — here sim and synth *agree* but both differ
   from the intent. The forwarding is the semantics, not a bug.
3. **Blocking with correct reverse order** (`pipeb2`, §8.0 p.9): simulates and synthesizes
   as a pipeline — correct only because the single block sequences the reads before the
   writes. Fragile.
4. **Feedback** (`lfsrb1`, §10.0 p.12): "There is no way to order the assignments in
   Example 15 to model the feedback loop unless a temporary variable is used." Blocking
   assignment cannot express simultaneous update of mutually dependent registers without
   a temp or a concatenation trick (`lfsrb2`, §10.0 p.13).
5. **Nonblocking in combinational blocks** (`ao4`, §11.0 p.14–15): `tmp1 <= a&b; tmp2 <=
   c&d; y <= tmp1 | tmp2;` — "the values of tmp1 and tmp2 were the original values ... not
   the values that will be updated at the end of the simulation time step. The y-output
   will reflect the old values". Fix needs `tmp1, tmp2` in the sensitivity list and two
   passes (`ao5`), or blocking (`ao2`). Synthesis would infer the intended and-or either
   way; simulation lags by a delta.
6. **Multiple drivers** (`badcode1`, §14.0 p.18): two blocks `q <= d1` and `q <= d2` —
   "the simulation output is a race condition"; Synopsys warns "multiple-driver net with
   unknown wired-logic type" and infers "two flipflops with outputs feeding a 2-input and
   gate ... The pre-synthesis simulation does not even closely match the post-synthesis
   simulation" (§14.0 p.18–19).
7. **Mixed `=` and `<=` to one variable** (`ba_nba6`, §13.0 p.17–18): "will most likely
   simulate correctly most of the time, but Synopsys tools will report a syntax error".

### 5.3 The repair (the eight guidelines, §5.0 p.5 and summary p.22)
1. Sequential logic → nonblocking. 2. Latches → nonblocking. 3. Combinational logic in an
always block → blocking. 4. Mixed sequential+combinational in one block → nonblocking.
5. Do not mix blocking and nonblocking in one always block. 6. Do not assign the same
variable from more than one always block. 7. Use `$strobe` to display NBA results.
8. No `#0` delays.

Why they work: NBA's evaluate-all-then-update-all makes same-edge register updates
order-independent (§4.0 p.4), matching synthesis's structural view of "one flop per
assigned variable, D = RHS evaluated from pre-edge values". Blocking inside a single
combinational block gives single-pass forwarding that matches the and-or structure.
Guideline 6 removes multiple drivers, whose hardware meaning (wired logic) synthesis cannot
infer. Guideline 5 avoids the ambiguous case where a variable has both a forwarded and a
deferred value in one block.

### 5.4 Combinational loops
Not treated except as the self-triggering oscillator (`osc2`) and the multi-pass `ao5`.

## 6. Properties claimed and how established

- "Adherence to these guidelines will also remove 90-100% of the Verilog race conditions
  encountered by most Verilog designers" (§5.0 p.5); "Following the above guidelines will
  accurately model synthesizable hardware while eliminating 90-100% of the most common
  Verilog simulation race conditions" (p.22). Established by: the 26 worked examples, each
  classified as simulates-correctly / synthesizes-correctly, with simulator transcripts for
  the `$display`/`$strobe`/`#0` ordering myths (§15 p.19–21) and one normative IEEE quote
  (§15.3 p.21).
- "Ignoring the above guidelines can still infer the correct synthesized logic, but the
  pre-synthesis simulation might not match the behavior of the synthesized circuit" (§1.0
  p.2) — the paper's thesis sentence: the gap is one-sided (synthesis is usually right,
  simulation is what races).
- No completeness claim; "90-100%" is deliberately hedged.

## 7. Relation between simulation and synthesized hardware

- Two independent semantic authorities, related by coding discipline rather than by any
  theorem. Synthesis is treated as the fixed point (what the hardware *is*); simulation is
  the thing that can be wrong. Post-synthesis simulation (gate-level) is the implied oracle
  ("pre-synthesis simulation does not even closely match the post-synthesis simulation",
  §14.0 p.19) — i.e. differential simulation of RTL vs netlist is how one would detect the
  mismatch, but the paper never runs it; it reasons about it.
- The paper is explicit that the standard *permits* the nondeterminism (§2.0 p.2; §3.0 p.3
  "the order execution is unknown"), so the mismatch is a property of legal Verilog, not of
  broken tools.

## 8. Transfer to Copper (concrete and critical)

### What fits
- **The definition of a race** (§2.0) is the cleanest statement of the property Copper
  demands of its simulator: "Simulation must be independent of Rust async poll order — a
  well-formed design simulates identically under any `poll_tasks` order" (CLAUDE.md). Copper
  elevates "no race" from a coding guideline to a checked invariant (poll-order fuzzer on
  the fixpoint oracle; canonical order under levelized). Cite §2.0 as the definition and
  §3.0/§8.0 as the failure mode.
- **NBA two-phase scheduling = `RegOut` / register commit at the edge.** Copper's tick phases
  (pre-edge settle → edge → post-edge settle → observation) are a refined stratified event
  queue: the pre-edge settle is the active region before the edge, the edge is the NBA
  update region (all registers and `RegOut`s commit together), post-edge settle is the
  re-triggered combinational propagation. The "evaluate RHS from pre-edge values, update at
  end" rule (§4.0) is exactly Copper's "a `RegOut` write takes effect at the first clock
  edge after it executes" and "every value live across an await becomes a register".
- **Last NBA wins** (§15.3) = Copper's sequential `Out`/`RegOut` overwrite semantics within
  a segment and the hold-when-unwritten idiom; both inherit IEEE 1364 §5.4.1 ordering. Copper
  should state it explicitly as a rule rather than inheriting it silently, as MyHDL does.
- **Blocking forwarding inside one block = Copper locals.** Copper's locals forward within a
  tick-free segment exactly like `=` in one `always` block (`pipeb1`'s `d → q3` collapse is
  what `let q1 = d; let q2 = q1; let q3 = q2;` would mean in a Copper segment, correctly,
  and `infer_registers` would make none of them a register because none is live across a
  tick). The feedback problem (`lfsrb1`, §10.0) that blocking cannot express without a temp
  is a non-problem in Copper: reads of a register in a segment see the pre-edge value by
  construction (the register is the loop-carried local), so `let (q3, q2, q1) = (q2, q1^q3,
  q3)` is natural. This is a good illustration that Copper's "locals forward, registers are
  the loop-carried values" model gives NBA semantics for state and blocking semantics for
  wires without the designer choosing an operator — the choice Cummings' Guidelines 1–5
  exist to police.
- **Guideline 6 (one driver per variable)** = Copper's `multi_write_collapse` and the
  single-owner `Out` port model; Copper's analogue of the "wired-logic" ambiguity cannot
  arise because a port has exactly one driving module.
- **Guideline 3's failure (`ao4`, NBA in comb logic lags a delta)** is the exact shape of
  Copper's fixed D2 ("a read feeding a combinational `Out` in a register-free segment is
  now `Immediate`, so a passthrough tracks its producer instead of lagging it"): a
  combinational output computed from a deferred value is one delta late. Cummings fixes it
  by operator choice; Copper fixed it in the simulator's read-timing classification.
- **The mismatch is one-sided**: Cummings' framing (synthesis right, simulation races) is
  also Copper's working assumption when a divergence is found — the SV (Verilator) side is
  the reference and the rules (`unprotected_pretick_out_write`, `multi_phase_out_write`,
  `linear_out_write_anchor_mismatch`) refuse shapes where the simulator's immediate
  execution cannot be matched by an emission form. But Copper also fixed emission forms
  (phase C hold-form, drive-level mux) — i.e. it treats the *simulator* as the normative
  denotation (`CYCLE_DATAFLOW_SEMANTICS.md`: "denotation NORMATIVE") and changes codegen to
  match, which is the opposite of Cummings' stance. Worth stating which side is normative
  and why; Cummings never has to, because for him Verilog-the-standard is the semantics of
  both.
- **Hedged empirical property statements** ("90-100%") are the register in which an
  informal guideline paper can honestly speak. Copper's rules are pinned by exact-set corpus
  tests and measured probe batteries ("fourteen probes, every verdict predicted first"),
  which is a stronger (but still empirical) footing; the formal-esque definitions should
  make the quantifier explicit the way Cummings does not ("most common", "most Verilog
  designers").

### What does not fit
- Verilog has no cycle, no process-local persistent state across edges other than regs,
  and no coroutine; Cummings' world is one `always` block = one edge's worth of behaviour.
  Copper's multi-tick bodies, `control_extract`, and the pre-first-tick segment have no
  counterpart; nothing in the paper speaks to opening/closing anchors or phases within a
  body.
- Cummings treats the `#0`/inactive region and `$strobe` monitor region — simulator
  artefacts Copper does not have (Copper's observation phase is post-edge observation, a
  cleaner single region).
- The paper's correctness evidence is reasoning about examples, not execution against an
  independent reference; Copper's bar (sim ≡ Verilator on every module, anchored to BaseJump
  where available) is what Cummings' Fig. 3 ("Actual synthesized result!") gestures at
  without automating.

### Critical take
Cummings is the best available plain-language statement of (i) what a race is, (ii) the
two-phase register-update model, and (iii) why synthesis and simulation can legally
disagree. It is *not* a semantics — it defers to IEEE 1364 for simulation and to the tool
for synthesis — and its guarantee is a hedged percentage. For Copper it is the right
citation for the stratified event queue, for last-NBA-wins, for the race definition, and
for the one-delta-lag pathology; and a useful foil: Copper replaces "pick the right
operator" with "the language decides register vs wire from liveness and phase, and refuses
the shapes it cannot make agree".

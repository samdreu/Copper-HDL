# Chisel — Bachrach, Vo, Richards, Lee, Waterman, Avižienis, Wawrzynek, Asanović, "Chisel: Constructing Hardware in a Scala Embedded Language", DAC 2012

Source: paper/references/chisel_dac2012.pdf (text: scratchpad/papers/chisel_dac2012.txt).
The paper has a main body (§1–§9) and a "Supplemental" (§10) with the generated Verilog
(Fig. 4), the FIFO (Fig. 3), width inference (§10.4), and the FSM `switch` (§10.2, §10.6).
This is pre-FIRRTL (FIRRTL is 2017); the IR is an in-memory node graph.

## 1. What is given a semantics; grammar; what defines meaning in practice

- **Object:** the elaborated *graph of nodes*. §2.2: "A circuit is represented as a graph of
  nodes in Chisel. Each node is a hardware operator that has zero or more inputs and that
  drives one output. A literal is a degenerate node that has no inputs and drives a
  constant value on its output." Scala evaluation *constructs* this graph; it is never
  "run" as the circuit. §2: "Chisel comprises a set of Scala libraries that define new
  hardware datatypes and a set of routines to convert a hardware data structure into
  either a fast C++ simulator or low-level Verilog."
- **Grammar:** none. The surface is Scala; Chisel is a library (§10.3 discusses what Scala
  could not be made to express: new literal tokens, overloading `if` — hence `when`).
- **What defines meaning in practice:** the two backends. (a) The generated **C++
  cycle-accurate simulator** (§5: "The Chisel compiler produces a C++ class for each Chisel
  design, with a C++ interface including clock-low and clock-high methods"). (b) The
  generated **Verilog** (§10.9, Fig. 4). The paper never names one as the reference; the
  node graph is the common source and the translation of each node kind is "obvious".
- The only places the paper *says what a construct means* are §2.5 (Reg = one-cycle
  delay), §2.6 (`when` and last-connect, with a truth table and Fig. 1), §4.3 (Mem with
  `readLatency`), and §10.4 (width inference rules).

## 2. Style

- **Informal, by example, by translation to a structural graph.** The closest thing to a
  semantic rule is the `when` truth table (§2.6) and Fig. 1 ("Equivalent hardware
  constructed for conditional updates"): each `when (c) { r := e }` "can be viewed as
  inserting a mux before the input of a register to select either the update expression
  or the previous input according to the when predicate. In addition, the predicate is
  OR-ed into a firing signal that drives the load enable of the register. The compiler
  places initialization values at the beginning of the chain so that if no conditional
  updates fire in a clock cycle, the load enable of the register will be deasserted and
  the register value will not change."
- `:=` is wiring, not assignment: §2.4 "The := assignment operator ... is a special
  operator in Chisel that wires the input of left-hand side to the output of the
  right-hand side."
- There is no operational semantics, no type-system semantics (Scala's types are used for
  generators, §3.1; widths are a separate inference pass, §10.3–10.4).

## 3. Time model

- **Implicit single global clock and reset.** §2.5: "In the current version of Chisel, clock
  and reset are global signals that are implicitly included where needed."
- **Cycle-stepped.** The C++ simulator exposes "clock-low and clock-high methods" (§5): a
  two-phase step — evaluate combinational nodes, then latch registers. No events, no delta
  cycles. §2.6: register "r is updated at the end of the current clock cycle only if c is
  zero."
- **One value per wire per cycle** follows from the graph being a DAG of operators with
  registers as the only delay (`Reg(in)`: "an output that is a copy of the input signal in
  delayed by one clock cycle", §2.5).
- Verilog (Fig. 4): one `assign` per node (`T0..T11`), one `always @(posedge clk)` with a
  synchronous `if(reset) ... else if(1'h1) ...` per register — the `else if (1'h1)` is the
  OR-ed enable of Fig. 1 after constant folding (every register in the FIFO is written
  unconditionally via a `Mux`).

## 4. State

- **Registers are explicit:** `Reg(in)`, `Reg(){ UFix(width=16) }` (§2.6), `Reg(resetVal =
  ...)` (§10.2, Fig. 3). `delay(x, n)` builds n of them recursively (§3.1). Never inferred
  from code shape.
- **Initial/reset:** `resetVal` → the synchronous reset branch in Fig. 4 (`deq_ptr <=
  5'h0`). A `Reg` without `resetVal` gets no reset branch; the Verilog is then X at
  power-on and the C++ simulator's start value is not stated. Width of a register "must be
  specified by the user either explicitly or from the bitwidth of the reset value" (§10.4).
- **Memories** (§4.3): `Mem(depth, target, readLatency)`; `readLatency = 0` is a
  combinational read, `1` a registered one; `hexInitFile` for initial contents.

## 5. Intra-cycle ordering

- **Last-connect semantics** (§2.6): "In a sequence of conditional updates, the last
  conditional update whose condition is true takes priority." Truth table: `c1=1,c2=1 → r
  = 2`, "c2 takes precedence over c1"; `c1=0,c2=0 → r unchanged`.
- **Crucial reading:** this is a *static priority over drivers*, not dynamic last-write-wins
  over values. The Scala program runs once, at elaboration; inside `when (c) { r := r +
  UFix(1) }` the `r` on the right is the register's output node — the current (pre-edge)
  value — regardless of any `when` textually earlier. There is **no forwarding** and no
  read-after-write within a cycle. The FIFO (Fig. 3) is written in that style:
  `is_full_next` is computed from current `is_full`, `enq_ptr`, `deq_ptr`, then
  `is_full := is_full_next`.
- **Unwritten register holds** — by Fig. 1's seeded mux chain and deasserted enable. (This
  is the "enabled-register idiom".)
- **Wires driven by `:=` at most once**: §2.4 "Chisel warns users if ports have other than
  exactly one connection" (for bulk `<>`). Plain combinational outputs are functions
  (`io.enq_rdy := !is_full`), not held.
- **Combinational loops:** not discussed anywhere. The graph may contain them; neither
  backend's behaviour on a loop is stated.
- **Width inference fixpoint** (§10.4): "The bit-width inference process continues until no
  bit width changes ... output bit widths either grow or stay the same ... we can show
  that the bit-width inference process will converge to a fixpoint." The only "fixpoint"
  in the paper, and it is about widths, not values.

## 6. Properties stated and how established

- **No semantic property is stated or proven.**
- **Evidence given (§6, §10.7):** (a) A 64-bit FMA "has been mapped to both Verilog and C++
  emulation code, and both results have been simulated in testbenches using SoftFloat and
  TestFloat to verify IEEE-754-2008 compliance" — the *design* is checked against a golden
  model through *both* backends; this is de facto differential evidence that the two
  backends agree on that design, but it is not claimed as such. (b) A 5-stage 64-bit RISC
  booting an OS (88,291,350 cycles) run on both VCS (generated Verilog) and the C++
  simulator (Table 2: 5390 s vs 694 s) — again both backends ran the same workload, de
  facto cross-checked, reported only as speed. (c) 3× LOC reduction vs hand Verilog; area
  parity on 65 nm; a 1.76 mm² 45 nm vector processor GDSII.
- Width-inference convergence is argued informally (§10.4).

## 7. Relation between the semantics and the generated Verilog

- Translation *is* the definition. Fig. 4 is the generated Verilog for the FIFO of Fig. 3:
  flat, one `wire`/`assign` per node with machine names, a single `always @(posedge clk)`
  with per-register `if(reset) ... else if(1'h1)`. §10.9: "the low-level structural nature
  of the Verilog code generated by Chisel" (1.65× slower in VCS than hand behavioural
  Verilog on 92 test programs — another de facto cross-run, reported as a speed number).
- No translation-correctness statement; no systematic differential test is described,
  although two independent backends from one graph exist and were run on the same
  testbenches.

## 8. (Survey question — n/a.)

## 9. Transfer to Copper — what fits, what does not

**Fits:**

- **Fig. 1 is the canonical realization of Copper's "Out holds when unwritten" and
  `RegOut` written in several `if`s.** A `RegOut` (or a register-backed plain `Out`)
  assigned under several conditions before a tick denotes exactly Chisel's seeded mux
  chain with an OR-ed enable; the "unwritten → unchanged" row of the truth table is
  Copper's hold semantics (memory: verified against `bsg_dff_en`). Copper can define the
  *meaning* of multiple conditional writes to a port as "the last write on the taken path,
  else hold", and cite Chisel §2.6/Fig. 1 as the standard hardware form. Copper's phase-C
  `<port>_hold` register is the same device applied to a plain `Out`.
- **"Updated at the end of the current clock cycle"** (§2.6) is Copper's `RegOut` rule
  ("takes effect at the first clock edge after the write", decided 2026-10-23).
- **Two backends from one source, cross-run on the same testbench** is Copper's regression
  made explicit. Chisel did it implicitly (FMA vs SoftFloat through both backends; OS boot
  on both); Copper asserts cycle-by-cycle equality and sweeps every module. Worth noting
  that Chisel never *claimed* backend agreement — Copper's claim is the stronger one and
  should be stated as a property with the sweep as evidence.
- **By-translation to a structural core.** Chisel's node graph is a "core structural DSL" in
  the Golden Age sense; Copper's VLIR ("1:1 with legal SystemVerilog") plays that role.
  Giving the SV side of Copper's semantics as "the meaning of VLIR" and the lowering as a
  translation is the Chisel shape.

**Does not fit, and why:**

- **Last-connect is static priority on drivers; Copper's segment is dynamic sequential
  execution.** In Chisel a read of `r` anywhere is the pre-edge value; in a Copper segment a
  read after an assignment sees the assigned value (forwarding). The two coincide only when
  no write is read back within the segment. Copper's `pretick_out_write_before_update`
  (a port written from a register before that register's update in the same segment
  publishes the previous generation) and the D1 "forwarded guard" clause (`d4a`) are
  exactly the cases where Chisel-style static reading and Copper's sequential reading
  diverge. So: Copper may cite Chisel for the *mux-chain form* of conditional writes, but
  not for *which value* a read returns.
- **No point-of-read; no anchors.** Every Chisel read is of the cycle's settled value
  because elaboration has no "time within the cycle". Copper's `In` read at a point in a
  segment, `Immediate` vs deferred reads (D2), opening vs closing anchors — none have a
  Chisel analogue. Chisel is the degenerate case: all reads at the (unique) observation
  point.
- **Plain combinational outputs do not hold.** `io.enq_rdy := !is_full` is a Mealy function;
  an undriven wire is an error. Copper's sequential-module plain `Out` *holds when
  unwritten*; Copper's combinational module is the Chisel case. The two defaults differ and
  Copper's doc should say which one applies where (it does: sequential vs combinational
  mode), citing Chisel as the "wire" default and `bsg_dff_en` as the "hold" default.
- **No coroutine, no FSM-by-control-flow.** Chisel's FSM is an explicit `state` register with
  `switch`/`is` (§10.2, §10.6 vending machine), which is Copper's `match pc` *after*
  `control_extract`. Chisel therefore has an analogue of Copper's *extracted* form, not of
  its source form; the `switch` → chain of `when` desugaring (§10.2) is a small precedent
  for "the meaning of the sugar is the meaning of its expansion", which is also how
  Copper's control extraction is positioned — and the reason Copper keeps the
  phase-sensitive checks upstream of it.
- **Registers must be declared; widths must be given for registers.** Copper infers
  registers by liveness and widths from `Bits<N>`. Chisel offers no help on proving the
  inferred register set correct.
- **No combinational-loop story.** Copper's levelized scheduler with SCC iteration is beyond
  anything Chisel 2012 specifies.

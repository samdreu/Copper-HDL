# Copper's semantics on one page

**Status (2026-11-04): the generating document.** Every clause of `SPEC.md` must follow from
this page or from a design choice listed on it. A clause that neither follows nor is listed
is suspect and goes back to its packet. `review/DECISION_AUDIT.md` sorts D-1…D-8 by that
test. The one question this page had open, Q1, is **decided** (D-8, 2026-11-04): one job.

## 1. What a module is

A Copper module is a Rust coroutine. The clock resumes it. A resumption runs plain Rust
from one `tick` to the next: it reads input wires, writes output registers, and whatever
it keeps across the suspension is its state. The hardware is the same coroutine's transition
function laid out as flip-flops and logic. The simulator is rustc's own lowering of the
coroutine, resumed by an executor that is the protocol below and nothing more. The
transpiler emits a state machine with one state per `tick` site and the live-across-tick
locals as registers, and is correct iff that machine's trace equals the coroutine's.

## 2. The one choice, decided

A sequential body does **one job** per cycle (Prost's model; D-8). The resumption that ends
at edge N+1 reads the inputs present before that edge, computes the next state, and writes
the output registers that take effect at it. Every read sees that edge's input; every
output of a sequential module is a `RegOut`; there is one instant per cycle; the trailing
region is the head of the next resumption.

What it gives up: a Mealy output — a combinational function of the current input — inside
a clocked body. That shape lives in a separate `combinational` module wired to the body's
`RegOut`s (measured at no cell cost, `review/Q1_TRADEOFFS.md` §4 (d')). A Moore output
written after the tick becomes a `RegOut` of the next state written before it, trace-
identical, at one flop per output bit unless exposed through a `combinational` module.

What it removes: the barrier before a deferred read, the second instant, the five
plain-`Out` emission forms, the eight alignment rules and their opt-out, and the one hazard
the two-job body had that no rule covered (a trailing combinational drive sampled once,
wrong in composition). The measured cost: 19 shipped and 69 probe modules rewritten by port
kind plus a tick at the loop's end; every shipped rewrite trace-identical to its independent
anchor; the paper's clocked cell-count matches unchanged. The 62 refused divergence probes
become unexpressible. The user settled the Prost question with the decision: the model is
shared and is cited; Copper's daylight is the realization (rustc's lowering as the FSM, the
simulator running plain Rust), liveness-based register inference, the written semantics,
and the measured, anchored sweep.

## 3. The contract with the programmer

Write the body as Rust; if it compiles under `#[hardware]` it transpiles; the hardware
behaves exactly as the simulation, observation for observation, from the first instant.
A program is refused for one of two reasons only, and the message says which: it is not a
hardware coroutine (§5, shape and design), or the transpiler has no form for it yet (§6,
a dated limitation with the form that would fix it).

## 4. The design choices

These are choices, not consequences; each is recorded in `SPEC_DECISIONS.md`.

1. One instant per cycle, the drive–tick–observe environment, observation 0 in the
   trace (D-1 as re-derived by D-8).
2. One job: the sequential body resumes once per cycle, at the edge; no plain `Out` in a
   clocked body (D-8). This replaces the barrier placement rule and the plain-`Out` hold.
3. A `RegOut` holds when unwritten (D-6). Anchored by `bsg_dff_en` in its `RegOut`
   spelling.
4. A write no observation can show is refused, and a shape with two spellings keeps one
   (D-4, D-5; under one job the first is vacuous for sequential modules and the second
   routes the Mealy output to a `combinational` module).
5. `RegOut` is a port kind whose write takes effect at the next edge (D-6) — the output
   rule.
6. Every port and register powers on at the zero of its type (D-6; wires in I.11;
   registers in I.7).
7. A register is a local live at a `tick` suspension; anything else a realization must
   store is a carrier, named with the form that provides it (D-7). Under one job no
   carrier is known (I.0.5).

## 5. The three principles behind every rule

**Shape.** A hardware coroutine has one suspension kind, every loop path reaches a
`tick`, its state is finite, it never returns, and its outputs are registers.
**Design.** Every effect is observable in some trace; one spelling per meaning.
**Realizability.** The emitted text must realize the resumption's effects in every cycle.
Where it cannot, the shape is refused as a dated limitation with a fix direction, never as
a meaning. A rule that cannot name its principle is deleted.

## 6. What is refused, and why

Two kinds. A **semantic refusal** cites a clause of §5's first two principles: a non-tick
`await`, a tickless loop path, a return, a plain `Out` on a sequential module. A
**lowering limitation** cites the form the transpiler lacks and the date it was recorded:
twelve rows today (`SPEC.md` §II.3, D-13, 2026-11-05) — six dated refusals (a nested
`continue`; a nested loop whose ticks are only inside arms; two element writes to an array
register in one cycle; a constant initializer the evaluator cannot fold; the `Bits` methods
the transpiler lacks; a hex literal) and six shapes the transpiler admits and realizes
wrongly (the run-time-empty counted `for`, the pre-loop re-run and drop, the trailing fold
at edge 1, array-element forwarding, a pre-loop tick), each with a fixture owed to Phase C.
The eight alignment rules went with the second instant (D-8). The first list is the language.

## 7. How it is checked

Three legs and an anchor. The **simulator** runs the coroutine. The **reference
interpreter** runs the kernel under the protocol of §1 without rustc's lowering, and
disagreement with the simulator is a simulator or macro bug. The **transpiled SystemVerilog**
runs under Verilator, and disagreement with the other two is a transpiler bug or a §6
limitation. Agreement among the three is consistency, not correctness: only the
**independent anchors** (BaseJump, the hand-written goldens) decide what the hardware does.
Every claim says which of the four it rests on.

## 8. What done looks like

This page, unchanged in substance. `SPEC.md` as its elaboration, every clause traceable
here. The interpreter as a sweep leg with its guard. Every rule in §6's two lists with its
principle or its date. The decision log as the record of §4 and of nothing else.

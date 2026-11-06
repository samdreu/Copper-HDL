# Kôika (Bourgeat, Pit-Claudel, Chlipala, Arvind — PLDI 2020, "The Essence of Bluespec: A Core Language for Rule-Based Hardware Design")

A short note, written 2026-11-05 for `SPEC.md` §III.1 (D-20): Kôika is the mechanization
precedent for a **compiler-correctness theorem stated against a cycle semantics**, the shape
Copper's P1 and P8 would take if mechanized. Section numbers are the paper's. Source: the
PDF at `people.csail.mit.edu/bthom/pldi20.pdf` (DOI 10.1145/3385412.3385965), text
extracted with `pdftotext` in session spec-round-5's scratchpad; not vendored. The paper's
two-column layout scrambled some subsection headings in the extraction, so a claim below
names a section only where the heading was read unambiguously.

## What it formalizes

- **Object** (§3.1): a program is a set of rules and a scheduler. A rule is an action over
  registers, read and written through ports 0 and 1 (`rd0`/`rd1`/`wr0`/`wr1`, §2; a value
  written at port 0 is visible to a port-1 read in the same cycle).
- **Semantics** (§3.2): a rule is characterized by the **log** of reads and writes it
  performs; a cycle runs the scheduler's rules in order over an accumulated log `L`, a rule
  whose log would conflict with `L` is dropped, and at the end of the cycle the registers are
  updated from `L`: the state-transition function is `δs(R) = update(R, L)`. Per cycle, not
  per trace; no statement runs between cycles.
- **Theorem 1, one-rule-at-a-time** (§4): if `δs(R) = R1` then some sequence of the
  scheduler's rules, run one at a time, reaches `R1` (`OneRuleAtATime.v`). Bluespec's
  serializability, proved of the cycle semantics.

## The compiler theorem

§6 compiles a program to circuits ("mini-RTL"), one circuit per register. The top-level
theorem (§6, "Proving compiler correctness") is `∀R, s, r. δs(R)[r] = δ⟨s⟩r(R)`: for every
register state, schedule and register, the compiled circuit's next value equals the cycle
semantics'. It is a **per-cycle** equality of next-state functions; trace equality follows by
induction from equal initial states. Mechanized in Coq, with the semantics, the compiler and
its proof. **Trusted:** "a thin unverified pretty-printing layer" from mini-RTL to a small
subset of Verilog (§6), then standard synthesis. Its C++ simulator path is separate code.

## Transfer to Copper

- P1 (SPEC III.1) is the analogue of `δs(R)[r] = δ⟨s⟩r(R)` with Copper's H(M) in the role of
  `δs` and the emitted `match pc` machine in the role of `⟨s⟩`. Kôika's two sides share the
  state (the registers), so the theorem is a function equality; Copper's sides do not share
  the control state — `pc` encodes continuations many-to-many (I.0.4, D-20) — so the
  statement is a bisimulation under a relation `R`, not an equality of functions.
- Kôika's trusted pretty-printer is where Copper's P8 sits: Copper's emitted text *is* the
  realization, so a mechanized P8 would have to reach SystemVerilog text (or a stated subset
  of it), which Kôika does not.
- Kôika proves serializability of a multi-rule cycle; Copper's one-job body (D-8) has one
  resumption per module per cycle and its settle is a function of the sources (I.13.2(a)), so
  there is no ORAAT analogue to prove.

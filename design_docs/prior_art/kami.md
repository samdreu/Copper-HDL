# Kami (Choi, Vijayaraghavan, Sherman, Chlipala, Arvind — ICFP 2017, PACMPL 1, Article 24, "Kami: A Platform for High-Level Parametric Hardware Specification and Its Modular Verification")

A short note, written 2026-11-05 for `SPEC.md` §III.1 (D-20): Kami is the mechanization
precedent for **refinement as trace inclusion between labeled transition systems**, checked
modularly, in Coq. Section, figure and page numbers are the paper's (pp. 24:1–24:30). Source:
the PDF at `adam.chlipala.net/papers/KamiICFP17/KamiICFP17.pdf` (DOI 10.1145/3110268), pages
24:1–24:4 and 24:9–24:12 read as typeset in session spec-round-5; not vendored.

## What it formalizes

- **Object** (§2, Fig. 1): a module is registers, rules and methods in the style of Bluespec,
  embedded in Coq (Gallina functions can parameterize modules). A rule is a sequence of
  actions run **atomically**: a register read sees the state before the rule, writes are
  collected and applied together (p. 24:4).
- **Semantics** (§3, Fig. 6): a labeled transition system. Actions (Fig. 6c) build register
  updates and a label; a **substep** is at most one rule and any number of methods with
  disjoint updates (Fig. 6d); a **step** is a substep with internal calls matched and hidden
  (Fig. 6e); a **behavior** `m ⇓ ⟨n, α⟩` is a sequence of steps from `initRegs(m)` producing
  the label sequence `α` (Fig. 6f) — "the one-rule-at-a-time semantics" (p. 24:12).
- **Correctness** (§3.3): `m ⊑ m'` iff every trace `α` that `m` produces `m'` can also
  produce — refinement as **trace inclusion**; `≅` is refinement both ways. §5 gives the
  modular lemmas (composition, inlining, simulation arguments); §6 verifies an infinite
  family of multicore systems with coherent caches and pipelined RV32I cores against a
  sequentially consistent specification.

## The path to hardware

A verified Kami design is extracted to Bluespec (§4, §7) and compiled by the **commercial,
unverified** Bluespec compiler, which schedules several rules per clock cycle while
preserving the one-rule-at-a-time meaning; the paper trusts that compiler and names
verifying a compiler for core Bluespec as future work (footnote, p. 24:3).

## Transfer to Copper

- Kami's `⊑` is the form of Copper's III.2 relation in one direction: Copper requires trace
  **equality** of three realizations of one deterministic coroutine, not inclusion of an
  implementation's traces in a nondeterministic specification's, because a Copper module has
  one trace per input stream (I.0.3, I.12.1).
- Kami's step-to-trace lifting is the structure a mechanized P1 would use: a bisimulation of
  steps (one resumption against one `pc` transition) gives trace equality by induction.
- Like Kôika, Kami stops short of the emitted HDL: the Bluespec compiler is trusted, where
  Copper's P8 is about the emitted SystemVerilog text itself.

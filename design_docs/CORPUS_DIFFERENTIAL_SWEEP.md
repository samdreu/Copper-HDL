# Corpus differential sweep — scope and as-built design

**Status:** phases 1–4 BUILT and green (2026-08-25). `build.rs` generates one
differential case for every `#[hardware]` module in `tests/fixtures/` and
`examples/` — those two directories only; `src/sync.rs::sync_2ff` and modules
declared inline in test files are outside it (a `sync_2ff_concrete` twin is swept)
— and `tools/regression.sh`'s **G-D** asserts the sweep covered the corpus and ran.

The counts move as the corpus does, so trust the tool over any number written here:
`build.rs` prints `corpus sweep: N generated, M ignored-with-reason` on every build,
and G-D prints `corpus sweep ran N differential cases (M ignored-with-reason)` on
every full run. The current counts are in the build and regression printouts above (and in
`paper/stats/SUMMARY.md`); on 2026-09-01 it was 133 running, 10 ignored-with-reason, out of
143 modules, and on 2026-10-28 146 running, 9 ignored-with-reason, out of 155. Every
ignored one is a row in `build.rs`'s `SKIP` table — a recorded
transpiler refusal, a structural module with no simulatable body, a deliberate
divergence witness, a measured divergence (`branch_merge`, §0), a stimulus
precondition the sweep will not keep (`mod8`, §0), or the pipelined CPU, whose
`Memory` parameter the harness cannot supply. **None is "the generator cannot do
this."**

## 0. What the sweep found — the 2026-08-25 log, compressed

Phases 1–4 were built and run in one day; this is what each found, kept because
the findings are the argument for the sweep.

* **Phase 1 (hand-written cases for the unchecked backlog): two defects** in modules
  that had been in the tree for weeks. A measured sim ≠ synth divergence —
  `branch_merge_explicit` and its async twin transpile to byte-identical
  SystemVerilog, the twin agrees with it for 200 random cycles and it leads by one
  (read then as "D1's constant-write exemption is unsound for a
  conditionally-written `Out`", `PRETICK_ALIGNMENT_GUARDRAIL.md` §5.5; re-derived
  2026-10-28 as a mixed-anchor port, §5.8 — pinned in
  `tests/sequential_forwarding_divergence.rs`). And invalid emitted SystemVerilog —
  `event_source` has a port named `event`, a keyword missing from the legalizer's
  reserved list. Neither was reachable from the existing tests: that example is
  checked against hand-written Verilog, and nothing had ever Verilated what it
  transpiles to. Lesson carried into the generator: the testbench addresses the
  Verilated model by the **emitted** port name (`event` → `event_sig`), so the
  generated case calls `copper_codegen::legalized_port_name` at run time rather than
  reimplementing the rule.
* **Phase 2 (`build.rs` over `tests/fixtures/`): nothing** — every fixture agreed
  with its emitted SystemVerilog on the first run. The load-bearing part is the
  guard, `every_corpus_module_has_a_generated_case` in `tests/corpus_generated.rs`,
  which re-scans the corpus at test time and asserts the generated manifest
  (`COVERED`) matches in both directions; verified by negative control (dropping a
  module from the generator fails naming it). The runtime fit inside `cargo test`'s
  existing parallelism, which settled the "does it belong in the default path"
  question.
* **Phase 3 (extend to `examples/`): nothing new.** What it needed: per-file import
  and clock-domain rules keyed on file *content* (a fixture declares no `use` and no
  `ClockDomain`, an example brings both), `__`-aliased imports in the generated body,
  and the public `legalized_port_name`. One skip was too strict and was relaxed —
  a width written as a file-scope `const` is concrete, so the BaseJump modules sweep.
* **Phase 4 (generics, array ports, reset): a design's undefined region.**
  `shift_register` initialises its register to `Bits::x()` — deliberately, an unreset
  flip-flop is X — and the simulator carries X where Verilator's 2-state model reads
  0. Comparing undefined against undefined is a test of nothing, so the `RESET` table
  drives the reset port asserted on cycle 0 and randomly after. Generic modules
  sweep at the widths in `PARAMS` (the earlier "cannot emit it" reason was simply
  wrong — a generic module emits a *parametric* SystemVerilog module and needs only
  `-G`). Array ports sweep via `RandStim for [T; N]` (`tests/common/mod.rs`) whose bit
  layout is the one the hand-written array tests already record. Type rewriting in
  the generator is token-wise (`alias_ty`): `Bits<N_LOG>` must not be rewritten by a
  rule for `N`.
* **2026-10-28 (the multi-sequence stimulus policy, §3.2): one defect, the one
  it was built for.** The sweep's verdict had rested on one stimulus per module.
  `branch_merge` — the live, unflagged async twin of `branch_merge_explicit`,
  byte-identical SystemVerilog — leads its SV by one cycle on `tail_o` whenever the
  first cycle takes `sel == 1`, and the per-module seed drives `sel == 0` first, so
  the module had passed by its seed alone (recorded as a blind spot in
  `PRETICK_ALIGNMENT_GUARDRAIL.md` §5.5 on 2026-10-26). Measured first, as its own
  probe (`tests/sequential_forwarding_divergence.rs::branch_merge_tail_leads_its_sv_when_the_first_cycle_takes_sel_one`:
  sim `tail_o = 1` at cycle 0, SV `0`, agreement from cycle 1 on); then the policy
  landed and the sweep's `sel == 1` first-cycle sequence failed the case on the first
  run (`FAIL: Cycle 832 tail_o expected 1 got 0 (sequence 5 "first sel=1", cycle 0)`;
  two of the four seeds open with `sel == 1` too and fail at their cycle 0), which
  is the `SKIP` row it carries now. **Three other modules changed verdict on the
  same run, none a divergence of the lowering, each a precondition the single seed
  had satisfied by luck:** `simple_counter` starts from `Bits::<8>::x()` and `lfsr`
  (both copies) from a Rust initial value of 1 that the emitted SV does not carry
  (its flop has no initializer, so Verilator starts it at 0 and an LFSR at 0 stays
  there) — every sequence whose first cycle had `reset_i == 0` failed at cycle 0,
  and the old seed had asserted `reset_i` first for all three. They got `RESET`
  rows, the table that exists for state-undefined-until-reset, with the measured
  pre-reset difference in the row's sentence and `lfsr`'s pinned in
  `tests/lfsr_equivalence.rs` (whether a Rust initializer should reach the SV is a
  transpiler question recorded there, not settled). `mod8`, a combinational `a % b`
  whose header says the divisor is kept non-zero *by the stimulus*, drew `b == 0`
  from the all-zeros pattern and from two seeds — the simulator's all-X against
  Verilator's 0 — and got a `SKIP` row (the sweep does no stimulus masking, §3.4);
  its operator stays covered by hand-written vectors. Every other case passed its
  four seeds and its first-cycle sequences.
* **2026-10-29 (register power-on values): the `lfsr` finding fixed at its source.**
  The transpiler carried a register's initializer into the SV only when it was an
  integer literal or an enum variant; every constructor call — `Bits::zero()`,
  `Bits::from_lit::<0>()`, `Bits::from_u32(1)`, `[Logic::Zero; 16]` — was dropped,
  and the register's flop had no power-on value. Measured first, with the sweep's
  new randomized-power-on switch (§3.3): 63 of 163 cases failed, every one a
  register the SV left unstated. Then: every constant initializer folds to the
  register's `initial` value (`chir_lower::lower_register_init`; a repeated array
  as a fill), a non-constant one is refused by both front-ends
  (`copper_analysis::check_register_initializers`, pinned by
  `register_initializer_corpus.rs` at zero corpus hits), `Bits::x()` stays unstated.
  The emitted SV of 121 corpus modules gained an `initial` block and nothing else
  (`sv-baseline diff`), `lfsr`'s `RESET` row is gone and its pin in
  `tests/lfsr_equivalence.rs` asserts the pre-reset agreement; `simple_counter`
  keeps its row — it starts from `Bits::<8>::x()`, which is what the table is for.
  The randomized sweep's remaining 36 failures were all codegen-synthesized state
  with no Rust initializer (Appendix A); the user decided the same day that it is
  stated too — port flops, memory stage registers and hoisted input-read locals at
  zero, `Memory::new` arrays as a zero fill — after which the switch passes every
  transpiled leg (162 of 163 cases, `RESET`-row modules included); the one failure
  left is `ram_read_first`'s hand-written reference, whose array is deliberately
  uninitialized (§3.3).
* **After the log:** the `SKIP` table gained `<wrapper>::<module>` keys because the
  *example* copy of `ripple_carry_adder` refused (cause J-b) while the fixture copy
  swept; J-b closed 2026-08-27 and both copies sweep, but the key form stays. On
  2026-09-01 an audit found two `SKIP` rows whose reasons had silently stopped being
  true (`bit_not_bits`, `lit_width_in_ternary`); both were deleted, both sweep, and
  `build.rs` now validates every `SKIP` key against the corpus at build time so a
  dead row fails the build. A stale *reason* still cannot be detected mechanically —
  see §3.4.

---

## 1. Why this and not the individual bugs

The repo `TODO` records "acceptance is not correctness" repeatedly, and the reason
it keeps having to is structural, not a lapse of discipline:

* `tools/transpile_coverage.sh` measures **acceptance**. It says so in its own
  header. A module that transpiles into wrong SystemVerilog counts as covered.
* **Correctness** was measured only where somebody hand-wrote a harness *and* a
  reference model. That is real work per module, so it lags — and the lag is
  invisible, because the coverage number does not move when it grows.
* Stimulus was hand-written vectors, typically 8–12 cycles. Before the sweep only
  **three** DUTs got randomized streams
  (`tests/randomized_sequential_equivalence.rs`), because each needed an independent
  Rust model to compare against.

The load-bearing observation is that **the reference model is not required**.
Simulator vs Verilated-emitted-SV is already a differential oracle: two independent
implementations of the same source. A model is a valuable third opinion — it catches
the case where both sides are wrong the same way — but it is not what makes the
comparison meaningful. Dropping the model requirement is what makes the sweep
mechanical, and mechanical is what makes it complete.

### What this would have caught, as evidence rather than claim

| Defect | Caught? | How |
|---|---|---|
| Memory result → plain `Out` in an extracted module, one cycle late (2026-08-25) | yes | sim vs SV, any stimulus |
| `CASEINCOMPLETE` on an extracted comb `case` (2026-08-25) | yes | Verilator `-Wall` build |
| `WIDTHTRUNC` on a literal memory address (2026-08-25; FIXED — addresses carry an explicit width cast, `4'(64'd0)`) | yes | Verilator `-Wall` build |
| `uart/rx` wrong twice after it started transpiling | yes | sim vs SV |
| D1 pre-tick alignment instances | yes | sim vs SV |
| `RegOut` sequential forwarding | yes | the shape exists and sweeps: `tests/fixtures/regout_forwarding_dut.rs` (write_then_assign / assign_then_write) |

### What it would NOT catch, and why the existing checks stay

* A divergence that needs a **specific long input sequence** (a deep FSM state
  reached only by an exact pattern). Random stimulus is weak here; hand vectors and
  the golden traces stay. Since 2026-10-28 the *first* cycle is no longer left to
  chance (§3.2 — every initial vector for a narrow module, a pattern list for a wide
  one), and four seeds rather than one walk the rest; a rare *later* pattern is
  still the random stimulus's weak spot.
* A **shared misunderstanding** — both sides wrong the same way. Only an independent
  reference catches that, which is what `examples/basejump/` and the `REFERENCE`
  table are for (`ANCHORING_A_MODULE.md`). The anchoring is not replaced by this and
  must not be traded against it.

---

## 2. The corpus, measured

143 `#[hardware]` modules across `examples/` and `tests/fixtures/`, counted
2026-09-01 from the generated manifest (`COVERED` in `$OUT_DIR/corpus_generated.rs`)
cross-checked against a scan of the attributes. The mode split is by `#[hardware(…)]`
attribute; the rest by signature.

| | count |
|---|---|
| sequential | 121 |
| combinational | 19 |
| synchronizer | 2 |
| structural | 1 (`two_domain_top`, the only module with more than one clock port) |
| generic (const-generic params) | 10 instances of 6 names — all swept, at the widths in `build.rs`'s `PARAMS` |
| uses `RegOut` | 40 |
| array-typed ports | 2 (`mux`, `bsg_mux_one_hot`) — both swept |
| receives a `Memory<…>` parameter | 1 (`rv32i_cpu_pipelined`) — `SKIP`, anchored outside the sweep |

Port payload types are near-uniform, which is what makes generation tractable:
`Bits<N>` and `Logic` account for all but the two array ports, and there are no
`Vec` ports left (the CPUs' `Vec<Bits<32>>` ports went with the subset restriction
on 2026-08-26; both CPUs transpile — `rv32i_cpu_transpilable` sweeps under a `RESET`
row, `rv32i_cpu_pipelined` is gated by `tests/rv32i_pipelined_verilator.rs`). Port
*heads* are only `In` / `Out` / `RegOut` / `Clock`, plus a `Memory` parameter, which
is not a port (`build.rs`'s `Kind::Memory`). A width written as a file-scope `const`
or a const-generic parameter is still `Bits<N>`; the generator handles both.

### The backlog it closed (historical — phase 1's starting point, 2026-08-25)

Concrete example modules with **no** sim ≡ transpiled-SV check at the time: 10 —
`det_010_awaits`, `event_sink`, `event_source`, `fast_counter`, `flag_sync`,
`slow_consumer`, `two_domain_top`, `rv32i_cpu_pipelined`, `uart_tx`, `uart_rx`.
Of those, the last three did not transpile then and `two_domain_top` is structural,
so **six** were transpilable-but-unchecked. Fixture modules unchecked: **five**
(`branch_merge`, `branch_merge_explicit`, `match_tick`, `match_tick_explicit`,
`sync_2ff_concrete`).

Note the examples were *not* unverified — they are checked sim ≡ **hand-written**
Verilog, which is the stronger anchor. What was missing is sim ≡ **emitted** SV,
which is the direction transpiler bugs live in.

Eleven modules is a small backlog; it is not the point. The point is that the
backlog **regrows with every module added**, silently, and this removes that.

---

## 3. Design, as built

### 3.1 Generation, not invocation

`build.rs` (with `syn` and `quote` as build-dependencies) walks `examples/` and
`tests/fixtures/` (`collect_rs`, which prunes `old/` directories), parses every
`#[hardware]` signature (`Module::from_item`, `classify`), and emits one `#[test]`
per module into `$OUT_DIR/corpus_generated.rs`, which `tests/corpus_generated.rs`
`include!`s. `cargo:rerun-if-changed` on both directories and on `build.rs` itself
keeps it honest.

Each generated case wraps the source file in its own module and `include!`s it, so
per-file `struct MainClk` / imports do not collide. The wrapper name is
path-derived (`wrapper_name`: `examples/cdc/flag_crossing.rs` →
`ex_cdc_flag_crossing`), not stem-derived, because two different modules can share
a name (`fast_counter` exists in both `two_domain_counter.rs` and
`two_domain_hierarchy.rs`):

```rust
#[allow(unused_imports, dead_code)]
mod ex_basejump_lfsr {
    include!("…/examples/basejump/lfsr.rs");   // its own `use`s come with it
    #[test] fn lfsr_differential() { … }
}
```

The example files need *no* change for their `fn main` demos: the module-level
`#[allow(unused_imports, dead_code)]` is enough. What a file must not have is a
`//!` header — `include!` cannot carry inner doc comments — so `emit_file` turns
such a file's modules into `#[ignore]`d cases whose reason says so, rather than
emitting a case that will not compile. Every fixture and example uses `//` headers.

**Why build.rs and not a proc macro:** a proc macro reading files off disk has no
rebuild tracking, so a changed module would silently keep its stale generated test.
That is this repo's recurring bug class, not a hypothetical.

### 3.2 Stimulus — the multi-sequence policy (2026-10-28)

**Until 2026-10-28 each module was driven by ONE sequence** — 200 cycles from one
per-module seed (64 vectors for a combinational module) — so a module's verdict
rested on one stimulus, and a divergence that depends on the initial input pattern
was invisible. The instance that made this a bug rather than a caveat is
`branch_merge` (§0, 2026-10-28): its seed happened to drive `sel == 0` first, under
which its `tail_o` lag cannot be seen. The policy below is what replaced it; the
constants are `build.rs`'s `SEEDS`, `SEED_CYCLES`, `FIRST_CYCLES`,
`EXHAUSTIVE_BITS`, `COMB_EXHAUSTIVE_BITS`, `COMB_VECTORS`, and the plan itself is
built at run time by `StimulusPlan` in `tests/common/mod.rs` (the generator cannot
know a width written as a file-scope `const`), so the policy is one function with
one pin, `tests/corpus_generated.rs::the_stimulus_policy_is_what_the_design_doc_says`,
which G-D requires to have run.

Each swept module is driven by **several independent stimulus sequences, back to
back, in one testbench run of one Verilated model.** In order:

1. **Seed sequences.** `SEEDS` (= 4) independent random sequences of `SEED_CYCLES`
   (= 200) cycles. Sequence 0 uses the historical per-module seed — the FNV-1a hash
   of the module name (`seed_of`) — so the pre-policy stimulus is sequence 0
   *verbatim* and every verdict the sweep ever gave is a subset of the new one.
   Sequence *k* uses `seed_of("<name>#k")`. A seeded `Rng` (SplitMix64) produces a
   fresh value for every `In` port on every cycle through `RandStim`, as before.
2. **First-cycle sequences.** Let *W* be the module's total `In` width, excluding
   its `RESET` port if it has one, and let the *first live cycle* be cycle 0 — or
   cycle 1 for a `RESET` module, whose cycle 0 is the asserted reset as before; the
   pattern cycle deasserts the reset, so the pattern is the first thing the design
   sees out of reset.
   * ***W* ≤ `EXHAUSTIVE_BITS` (= 4): one sequence per initial input vector** —
     every one of the 2^*W* ≤ 16 vectors on the first live cycle, random stimulus
     for the remaining `FIRST_CYCLES` − 1 (= 31) cycles, under a seed derived from
     the vector. A one-input-bit module such as `branch_merge` therefore starts
     once from `sel == 0` and once from `sel == 1`; nothing about the first cycle
     is left to the seed.
   * ***W* > 4: a fixed first-cycle pattern list** — all bits zero; all bits one;
     and for each `In` port, that port all ones with every other port zero, and
     that port zero with every other port all ones (2 + 2·*P* patterns, *P* the
     number of `In` ports; every input bit is tried at 0 and at 1 against both
     backgrounds). Duplicates are dropped, and so is **any pattern whose vector a
     seed sequence already produced on its first live cycle** — checked at run time
     against the vectors actually driven, not predicted from the seed, and said so
     in the sequence's legend line.

   A **combinational** module (no clock; every vector independent) with *W* ≤
   `COMB_EXHAUSTIVE_BITS` (= 8) is driven by its **whole truth table** — one
   sequence, every vector once — instead of random vectors, which is strictly
   stronger; a wider one gets `SEEDS` × `COMB_VECTORS` (= 64) random vectors and the
   pattern list as further vectors.

**Isolation.** Every sequence starts from the design's initial state on both
sides: a fresh executor, wires and spawn in the simulator, and a fresh Verilated
instance in the testbench (`delete top; top = new V<module>()` at each sequence
boundary — `SimulationTrace::sequences`, `HardwareTest::begin_sequence`). The
sequences share one compiled testbench, so the cost is **one Verilator build per
module, exactly as before**; the model cache keys the whole testbench, so the
policy change itself is one cold miss per module and a repeat is free.

**Reporting.** The case prints a legend — one line per sequence with its index,
kind, seed or vector, and length — before it runs. A mismatch is reported by the
testbench as `FAIL: Cycle <global> <port> expected <e> got <g> (sequence <i>
"<label>", cycle <local>)`, and the harness's failure report lists which sequences
failed with their labels. `COPPER_SWEEP_SEQUENCE=<i>` re-runs the case with that
one sequence, so a failure is reproducible from the module name and an index.

**Budget.** Per clocked module: 4 × 200 cycles of seeds plus at most 16 × 32
first-cycle cycles — **≤ 1,312 cycles per module against 200 before**, about
5× the testbench text corpus-wide (134,000 cycles against 27,000, estimated from
the signatures on 2026-10-28), in the same number of Verilator builds. What that
costs in wall time is a measured number, not an estimate: none worth seeing (§5).

### 3.3 Comparison

`EquivalenceTest::differential_only` + `record_differential` (`tests/common/mod.rs`)
record the simulator's outputs as both actual and expected, so the trace comparison
is trivially satisfied and the Verilator leg carries the weight. The mode is
**explicit** and mutually exclusive with `record` by assertion: passing the sim's own
values as the "reference" is right here and a silent disaster anywhere else, so it
cannot be reached by accident. A module with a `REFERENCE` row additionally gets
`with_hand_written_reference`, which replays the same stimulus against the
independent Verilog (`ANCHORING_A_MODULE.md`).

**Power-on values — the randomized switch (2026-10-29).** Verilator is two-state and
zero-fills every register the SV does not initialize, so a register whose emitted
SV states no power-on value *agrees by accident* whenever the simulator's value is
zero — which is why ~120 `Bits::zero()` registers passed the sweep for two months
while their initializers were silently dropped by the transpiler, and why `lfsr`'s
`Bits::from_u32(1)` did not (§0, 2026-10-28). `COPPER_VERILATOR_X_INITIAL=1` closes
the accident: `verify_with_verilator` builds the model with `--x-initial unique`
(and `--x-assign unique`) and runs it with `+verilator+rand+reset+2` and a fixed
seed (`=<n>` picks it), so every register the SV leaves unstated starts at a random
value, and the only ways to agree are an `initial` assignment the transpiler
emitted or a reset the `RESET` row asserts before the first observation. The flags
are Verilator arguments, so they are in the model-cache key and a randomized build
never aliases a zero-fill one (`copper_sim::VerilatorPowerOn`). What it measures is
exactly "which observable state has no stated power-on value": before the register
initializer fix it failed 63 of 163 cases; after it, 36 — and none of those is a
Rust initializer. Every register they leave unstated is one codegen synthesizes: a
`RegOut` port flop (`skip_on_halt`'s `o`, `vr_src_regout`'s `valid`/`data`), the
edge-sampled register of a conditionally driven plain `Out` (`bsg_dff_en`'s
`data_o`, `conditional_plain_out`'s `o`), a memory's read- and write-stage registers
(`mem_rd0_q0`, `mem_wr0_s1_*`), a cross-state local hoisted from an input read
(`mac_pipeline`'s `product_r`) — or an unpreloaded `Memory` array (`ram_*`,
`dual_port_ram`, the `regs` register file). Their simulator-side power-on value is
the harness wire's zero or `Memory::new`'s `T::default()`, a convention of the
harness rather than anything the module states; `vlir_lower` already gave a
`<port>_hold` register `initial = 0` on exactly that convention. **Decided the same
day: state them all.** Every such register now gets `= 0` in the module's `initial`
block (`VLIRSeqBody::port_inits` for the port flops, `init: Some(0)` for the stage
registers and hoisted locals) and every `Memory::new` array a zero fill (the
simulator starts a memory at `T::default()`); Appendix A keeps the measured set.
After that the randomized sweep passes 162 of its 163 cases, the three `RESET`-row
modules included (their reset is asserted on cycle 0 and the comparison starts
there). The one failure is not a transpiled leg at all: `ram_read_first`'s
**independent reference** (`tests/fixtures/reference_sv/ram_read_first.sv`, a
`REFERENCE` row) leaves its array deliberately uninitialized, so under randomized
power-on the hand-written RAM reads random words where the simulator and the
transpiled SV — which agree with each other — read zero; the hand-written file is
the author's and is left as written. The hand-rolled callers
of `tests/common::verilator_command` are outside the switch, as they are outside
the model cache. It is a switch, not the default: the default sweep keeps the
zero-fill comparison the `RESET` table was written against.

### 3.4 The tables

There is no separate constraint file and no stimulus masking; a module whose
random inputs would hit a precondition is either fine under the harness's own rules
(a guarded 32-bit address that is simply never in range) or has a `SKIP` row saying
why. What cannot be inferred from a signature lives in four reviewed tables at the
top of `build.rs`, each row a sentence with an author:

| table | carries | verified at build time |
|---|---|---|
| `SKIP` | modules the sweep must not run, with the reason; keys are a bare name or `<wrapper>::<module>` for one copy of a duplicated name | every key names a module in the corpus (a dead row fails the build, since 2026-09-01) |
| `PARAMS` | the monomorphization for a generic module — the same widths its hand-written test uses | a generic module with no complete row is ignored with a reason naming the table |
| `RESET` | `(module, port, active_low)` for a design whose state is genuinely X until reset — a `Bits::x()` register, or a memory the reset must define. Not for a constant initializer: that reaches the SV as the register's `initial` value (2026-10-29), so the sweep observes such a module from its first cycle | — |
| `REFERENCE` | an independent Verilog reference, by module | module exists, file exists, declares `module <name>`, is not `// @generated` |

The generator adds its own ignore reasons where no table applies (`skip_reason`): a
`Memory` parameter (the sweep would have to invent its size and contents), more than
one clock port (the ratio is a design decision), a payload `RandStim` cannot produce,
or a `//!` header.

One limit is worth knowing. `emit_test(…, body = false)` replaces an ignored case's
body with a `panic!`, so `cargo test -- --ignored` **cannot** tell you a `SKIP` reason
has gone stale — it panics either way. Two rows sat in the table after their
lowerings were fixed (found 2026-09-01). Re-transpile a module before trusting a
reason that names emitted text.

### 3.5 The guards (this is the part that matters)

Two layers. In the test binary, `every_corpus_module_has_a_generated_case`
re-scans both directories with the same pruning as `collect_rs` and asserts the
`COVERED` manifest matches in **both** directions — a module with no case, or a case
for a module that no longer exists, fails naming it.

In `tools/regression.sh`, **G-D** requires that guard's `ok` line in the test log,
counts the `_differential ... ok` and `... ignored` lines, fails if none ran, and —
because a case without Verilator compares the simulator to itself — prints
`G-D NOT SATISFIED` and forces `PARTIAL` when `verilator` is not on the path. **G-E**
prints the anchoring ledger `build.rs` writes to the build log (`anchoring: N
module(s) checked against an independent reference, M cross-checked against the
transpiler only`); it does not fail.

This repo already had G-A, G-B and G-C because "the check silently did not run" is
its most-repeated bug class. A sweep without G-D would be one more instance of it.

---

## 4. Phasing (all done 2026-08-25)

| Phase | Work | Outcome |
|---|---|---|
| **1** ✔ | `differential_only` + a hand-written `tests/corpus_equivalence.rs` covering the 11-module backlog with random stimulus | found two defects on the first run (§0) |
| **2** ✔ | `build.rs` generator over `tests/fixtures/` | all generated cases green; the hand-written fixture cases deleted (§0) |
| **3** ✔ | extend to `examples/`, add `SKIP` and G-D | G-D in the driver; phase 1's hand-written file deleted, `tests/corpus_generated.rs` is the includer |
| **4** ✔ | generic modules via `with_params` monomorphization (`PARAMS`); array ports; reset sequencing (`RESET`) | the last generator-shaped skip gone (§0) |

`REFERENCE` and G-E were added on 2026-08-26 (`ANCHORING_A_MODULE.md`).

## 5. Costs and risks

* **Runtime.** One Verilator build per module, a few seconds each, absorbed by
  `cargo test`'s thread pool. It belongs in the full driver run, not `--quick`;
  `tools/regression.sh` prints the elapsed time of every run. The multi-sequence
  policy (§3.2) kept the build count and grew the testbench about 5×; measured on
  the `recycle` machine (96 cores, no ccache, `tools/regression-remote.sh --sync`
  into a clone of its own, 2026-10-28), the full regression's wall time was
  **699 s cold / 323 s warm before** and **704 s cold / 307 s warm after** (cold:
  every Verilator model built; warm: every model a cache hit) — within the
  run-to-run noise of a shared box, because the Verilator build dominates and its
  count did not change.
* **Noise from undefined inputs.** A module that needs more than the harness's
  rules is usually a finding about the module's contract rather than about the
  sweep; `RESET` and `SKIP` are the two places that finding is recorded.
* **False confidence.** A green sweep says the two implementations agree, not that
  either is right. §1 states what it cannot see; the independent anchoring stays.
* **Generated-test staleness.** Handled by `rerun-if-changed`, the two-directional
  guard, and the build-time validation of `SKIP` and `REFERENCE` keys. A stale
  *reason* is the residual (§3.4).

## 6. Relationship to the bigger refactor

This was deliberately sequenced **before** unifying the timing model (one
authoritative register inference in `copper-analysis`, consumed by both front-ends
— `SYNCHRONOUS_SEMANTICS.md`). That refactor touches the lowering's spine, and the
standing rule for migrations here is to build against a differential oracle and
keep the old path rather than debug it afterwards. This *is* that oracle, and it was.

---

## Appendix A — the power-on residual, measured 2026-10-29 (since stated)

The 36 of 163 sweep cases that still failed under `COPPER_VERILATOR_X_INITIAL=1` once
every Rust register initializer reached the SV (§3.3), as measured before the
decision to state them: each row lists the registers the module's emitted
SystemVerilog then left without an `initial` value, read off the emission itself;
none is a register the Rust source initializes. **Every class below is now stated
as zero** (same day, the user's decision; `SYNCHRONOUS_SEMANTICS.md`, *Register
power-on values*), so the table is the measurement history, kept so the decision
can be revisited without re-measuring. The classes:

* **B1** — a `RegOut` port's flop. The simulator shows the harness wire's initial
  value (zero) until the first write; the SV flop states nothing.
* **B2** — the edge-sampled register of a conditionally driven plain `Out` (the
  enabled-register idiom, `vlir_lower::conditional_output_ports`). Same convention,
  same gap. (The `<port>_hold` register of the opening-drive form already carries
  `initial = 0` and is not here.)
* **B3** — a memory's read- and write-stage registers (`<mem>_rd<k>_q<j>`,
  `<mem>_rd<k>_v<j>`, `<mem>_wr<k>_s<j>_*`), synthesized by the memory lowering;
  the simulator's pipeline starts empty.
* **B4** — a cross-state local control extraction hoisted out of its state, whose
  initializer is an input read (`control_extract::hoist_cross_state_locals`): its
  value is assigned before any read in the simulator, but the Verilated flop is
  read through the port's registered path before that in the first cycle.
* **C** — an unpreloaded `Memory` array (`Memory::new`): the simulator fills it
  with `T::default()` (zero), the SV array states nothing.

Not a `RESET` row's business (the table is for state that is X by design). Stating
these as `initial = 0` — the convention the `_hold` register already encoded —
closed every row below; the switch re-measures it without this list being
re-derived.

| module | unstated register(s) | class |
|---|---|---|
| `examples/basejump/bsg_dff_en.rs::bsg_dff_en` | `data_o` (B2) | B2 |
| `examples/basejump/sipo_block.rs::sipo_block` | `data_o` (B1), `w0_dbg` (B2), `w0_r` (B4), `w1_dbg` (B2), `w1_r` (B4), `w2_dbg` (B2), `w2_r` (B4), `w3_dbg` (B2) | B1 B2 B4 |
| `examples/memory/dual_port_ram.rs::dual_port_ram` | `memory[]` (C) | C |
| `examples/sequential/pipeline_mac.rs::mac_fsm` | `out` (B1) | B1 |
| `examples/sequential/pipeline_mac.rs::mac_pipeline` | `c_s_r` (B4), `out` (B1), `product_r` (B4), `sum_r` (B4) | B1 B4 |
| `examples/uart/rx.rs::rx` | `rx_byte` (B1), `rx_dv` (B1) | B1 |
| `examples/uart/system.rs::uart_rx` | `rx_byte` (B1), `rx_dv` (B1) | B1 |
| `tests/fixtures/branch_on_updated_reg_dut.rs::branch_on_updated_reg` | `o` (B1) | B1 |
| `tests/fixtures/continue_skip_dut.rs::skip_on_halt` | `o` (B1) | B1 |
| `tests/fixtures/cross_state_local_dut.rs::capture_after_wait` | `captured` (B4), `o` (B1) | B1 B4 |
| `tests/fixtures/extracted_dut.rs::ex_reg_then_wire` | `o` (B1) | B1 |
| `tests/fixtures/extracted_dut.rs::ex_regfile` | `o` (B1), `o3` (B1), `regs_wr0_s1_addr` (B3), `regs_wr0_s1_data` (B3), `regs_wr0_s1_v` (B3) | B1 B3 |
| `tests/fixtures/extracted_rom_dut.rs::ram_arms` | `mem[]` (C), `mem_rd0_q0` (B3), `o` (B1) | B1 B3 C |
| `tests/fixtures/extracted_rom_dut.rs::rom_gated` | `data` (B1), `rom_rd0_q0` (B3) | B1 B3 |
| `tests/fixtures/extracted_rom_dut.rs::rom_paced` | `data` (B1), `rom_rd0_q0` (B3) | B1 B3 |
| `tests/fixtures/mac_fsm_dut.rs::mac_fsm` | `out` (B1) | B1 |
| `tests/fixtures/mac_pipeline_dut.rs::mac_pipeline` | `c_s_r` (B4), `product_r` (B4), `sum_r` (B4) | B4 |
| `tests/fixtures/mem_address_width_dut.rs::narrow_index_into_narrow_addr` | `mem[]` (C) | C |
| `tests/fixtures/mem_address_width_dut.rs::wide_index_into_narrow_addr` | `mem[]` (C) | C |
| `tests/fixtures/mem_address_width_dut.rs::wide_index_sole_consumer` | `mem[]` (C) | C |
| `tests/fixtures/multiphase_rom_dut.rs::mp_regout` | `data` (B1), `rom_rd0_q0` (B3) | B1 B3 |
| `tests/fixtures/pipelined_ram_dut.rs::ram_r1w2_wf` | `mem[]` (C), `mem_wr0_s1_addr` (B3), `mem_wr0_s1_data` (B3), `mem_wr0_s1_v` (B3) | B3 C |
| `tests/fixtures/pipelined_ram_dut.rs::ram_r2w2` | `mem[]` (C), `mem_rd0_q0` (B3), `mem_rd0_v0` (B3), `mem_wr0_s1_addr` (B3), `mem_wr0_s1_data` (B3), `mem_wr0_s1_v` (B3) | B3 C |
| `tests/fixtures/pipelined_ram_dut.rs::ram_r3w1` | `mem[]` (C), `mem_rd0_q0` (B3), `mem_rd0_q1` (B3), `mem_rd0_v0` (B3), `mem_rd0_v1` (B3) | B3 C |
| `tests/fixtures/regfile_dut.rs::regfile_read_first` | `regs_wr0_s1_addr` (B3), `regs_wr0_s1_data` (B3), `regs_wr0_s1_v` (B3) | B3 |
| `tests/fixtures/regfile_dut.rs::regfile_write_through` | `regs_wr0_s1_addr` (B3), `regs_wr0_s1_data` (B3), `regs_wr0_s1_v` (B3) | B3 |
| `tests/fixtures/regout_forwarding_dut.rs::conditional_plain_out` | `o` (B2) | B2 |
| `tests/fixtures/regout_multiphase_dut.rs::regout_multiphase` | `lo_r` (B4), `word_o` (B1) | B1 B4 |
| `tests/fixtures/uart_rx_dut.rs::uart_rx_dut` | `rx_byte` (B1), `rx_dv` (B1) | B1 |
| `tests/fixtures/valid_ready_dut.rs::vr_read_after_delay` | `sum` (B1) | B1 |
| `tests/fixtures/valid_ready_dut.rs::vr_sink_moore` | `ready` (B1), `sum` (B1) | B1 |
| `tests/fixtures/valid_ready_dut.rs::vr_src_regout` | `data` (B1), `valid` (B1) | B1 |
| `tests/fixtures/valid_ready_dut.rs::vr_src_same_cycle` | `count` (B1), `valid` (B1) | B1 |
| `tests/fixtures/write_first_ram_dut.rs::ram_read_first` | `mem[]` (C) | C |
| `tests/fixtures/write_first_ram_dut.rs::ram_wf_priority` | `mem[]` (C) | C |
| `tests/fixtures/write_first_ram_dut.rs::ram_write_first` | `mem[]` (C) | C |

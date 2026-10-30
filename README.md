# Copper

**A Rust-embedded hardware description language.** You write hardware as
`#[hardware]`-annotated async Rust functions. Copper both **simulates** them —
with a custom async executor that models clock edges and delta cycles — and
**transpiles** them to synthesizable SystemVerilog.

```rust
#[hardware(combinational)]
fn one_bit_comparator(i0: In<Logic, ()>, i1: In<Logic, ()>, eq: Out<Logic, ()>) {
    let p0 = !i0.read() & !i1.read();
    let p1 = i0.read() & i1.read();
    eq.write(p0 | p1);
}
```

```systemverilog
module one_bit_comparator (
    input  logic i0,
    input  logic i1,
    output logic eq
);
    logic p0;
    logic p1;

    always_comb begin
        p0 = ((!i0) & (!i1));
        p1 = (i0 & i1);
    end

    assign eq = (p0 | p1);
endmodule
```

The project's correctness bar is that **the two agree**. Most examples run the
Copper simulator, emit SystemVerilog plus a Verilator C++ testbench, compile it,
and assert the two match cycle-by-cycle — and the sharpest of them check Copper
against *independent, third-party* Verilog rather than against Copper's own
output.

> **Status: research prototype (v0.1.0).** The semantics are anchored and
> regression-tested, but the language surface is deliberately narrow. Every
> `#[hardware]` module under `examples/` transpiles (34 of 34 as of 2026-09-01;
> `tools/transpile_coverage.sh` re-measures it); see [Limitations](#limitations)
> for what is refused **by design** and what is still a gap — the list is
> measured rather than aspirational.

---

## Why async Rust?

The central idea is that `clk.tick().await` **is** a clock-cycle boundary:

- every suspension point becomes a state in the generated FSM;
- every value **live across a tick** becomes a **register** (inferred by
  backward liveness analysis, not by an annotation you write);
- everything between ticks is combinational logic.

So an ordinary Rust `loop` with an `enum` state variable is a state machine, and
you get to keep Rust's type system, generics, `match`, and tooling while writing
it. Here is a real example — `examples/sequential/pattern_detector.rs`, a
"110101" sequence detector — and the SystemVerilog Copper emits for it:

```rust
#[hardware(sequential)]
async fn det_110101(
    clk: Clock<MainClk>,
    rstn: In<Logic, MainClk>,
    in_i: In<Logic, MainClk>,
    out_o: Out<Logic, MainClk>,
) {
    let mut state = State::IDLE;
    loop {
        if rstn.read() == Logic::Zero {
            state = State::IDLE;
        } else {
            state = match (state, in_i.read()) {
                (State::IDLE,   Logic::One)  => State::S1,
                (State::S1,     Logic::One)  => State::S11,
                (State::S11,    Logic::Zero) => State::S110,
                (State::S110,   Logic::One)  => State::S1101,
                (State::S1101,  Logic::Zero) => State::S11010,
                (State::S11010, Logic::One)  => State::S110101,
                _ => State::IDLE,
            };
        }
        if state == State::S110101 {
            out_o.write(Logic::One);
        } else {
            out_o.write(Logic::Zero);
        }

        clk.tick().await;
    }
}
```

```systemverilog
module det_110101 (
    input  logic clk,
    input  logic rstn,
    input  logic in_i,
    output logic out_o
);

    logic [2:0] state;

    always_comb begin
        if ((state == 3'd6)) begin
            out_o = 1'b1;
        end else begin
            out_o = 1'b0;
        end
    end

    always_ff @(posedge clk) begin
        state <= ((rstn == 1'b0) ? 3'd0 : (({state, in_i} == 4'd1) ? 3'd1 : (({state, in_i} == 4'd3) ? 3'd2 : (({state, in_i} == 4'd4) ? 3'd3 : (({state, in_i} == 4'd7) ? 3'd4 : (({state, in_i} == 4'd8) ? 3'd5 : (({state, in_i} == 4'd11) ? 3'd6 : 3'd0)))))));
    end

endmodule
```

That block is the CLI's output verbatim (`cargo run -q -p copper-codegen --bin
copper-transpile -- examples/sequential/pattern_detector.rs`); the one long line
is the `match` on `(state, in_i)` lowered to a nested ternary over the
concatenation `{state, in_i}`. Nobody declared a register. `state` became one
because it is assigned in the loop and read after the tick.

---

## Requirements

| | |
|---|---|
| **Rust** | every crate is `edition = "2024"`, which needs rustc ≥ 1.85 (pinned as `rust-version = "1.85"` in every `Cargo.toml`; developed on 1.92) |
| **Verilator** | required for examples and equivalence tests — `brew install verilator` (developed on 5.044; the numbers in the paper were generated with 5.051) |
| **Icarus Verilog** | *optional* — `brew install icarus-verilog` (developed on 12.0). Used only by the simulator-throughput benchmark `tests/sim_throughput.rs` and by `tools/stats/simperf.py`; when `iverilog` is absent that test skips its Icarus leg (`iverilog_available`) and the stats script refuses to write a CSV |

Verilator is not optional for a full regression run. A genuinely *absent*
binary is skipped with an explicit marker (`VERILATOR_NOT_INSTALLED` in
`copper-sim/src/verification.rs`); an installed-but-broken one fails loudly, by
design — "the check silently didn't run" is a bug class this project takes
seriously. The same three-way split (absent / present / present-but-broken)
applies to Icarus.

## Quick start

```bash
cargo build --workspace
```

Run one example end-to-end (simulate → transpile → Verilate → compare):

```bash
cargo run --example pattern_detector
```

The largest design in the tree is a 5-stage pipelined RV32I CPU
(`examples/cpu/rv32i_cpu_pipelined.rs` — forwarding, load-use stalls, branch
flush, a word-indexed register file, and a unified instruction/data memory
received as a parameter). It runs 13 architectural programs on the simulator
**and** on its own transpiled SystemVerilog under Verilator, matching
cycle-for-cycle:

```bash
cargo test --test rv32i_pipelined_verilator
```

Run the full regression — build, CLI, `cargo test --workspace`, and **every**
registered example:

```bash
tools/regression.sh
```

A clean full run ends with `REGRESSION OK`. Any subset ends with `PARTIAL OK`,
so a partial run can never be mistaken for a full one:

```bash
tools/regression.sh --quick          # fast loop: build + CLI + a few examples
tools/regression.sh --no-examples    # build + CLI + tests, no Verilator
tools/regression.sh --example lfsr   # one named example
```

The driver also enforces five wiring guards, because "the check silently didn't
run" has been a recurring bug class here: every `examples/**.rs` is registered as
a `[[example]]` (**G-A**), every registered example actually ran (**G-B**), every
`tests/*.rs` (root and per-crate) produced a test binary that ran (**G-C**), the
corpus differential sweep covered every module in `examples/` and
`tests/fixtures/` and ran with Verilator present (**G-D**), and — printed rather
than enforced — how many of those swept modules are additionally anchored to an
independent hand-written Verilog reference (**G-E**). It prints every
`#[ignore]`d test on each run so a deliberately-skipped check stays visible.

### The corpus differential sweep

`build.rs` generates one equivalence case per `#[hardware]` module in `examples/`
and `tests/fixtures/` (not `src/`, and not modules defined inline in a
`tests/*.rs` file): seeded random stimulus into the simulator, and the
SystemVerilog that module transpiles to Verilated against the same trace. No
reference model is needed — the simulator and the emitted SV are two independent
implementations of one source, so comparing them is already an oracle — which is
what makes a case cheap enough to have for *every* module rather than for
whichever ones someone wrote a harness for.

Each case drives **several stimulus sequences**, not one (since 2026-10-28): four
independent seeds of 200 cycles, plus a 32-cycle sequence starting from *every*
initial input vector for a module with at most 4 input bits (a fixed first-cycle
pattern list for a wider one), and the whole truth table for a combinational
module of up to 8 input bits (`SEEDS`, `SEED_CYCLES`, `FIRST_CYCLES`,
`EXHAUSTIVE_BITS`, `COMB_EXHAUSTIVE_BITS`, `COMB_VECTORS` in `build.rs`). The
sequences run back to back in one testbench against one Verilated model,
restarted from its initial state at each boundary, so the cost stays one
Verilator build per module. A failure names the sequence it fell in, and
`COPPER_SWEEP_SEQUENCE=<index>` re-runs that sequence alone. Until then a
module's verdict rested on one seed — `branch_merge` passed for two months while
its `tail_o` led the SV by a cycle whenever the first cycle took `sel == 1`.

A module the sweep cannot run gets an `#[ignore]` with its reason, never an
omission, and G-D fails if the generator quietly stops covering something. Widths
for generic modules (`PARAMS`), resets for designs whose state starts undefined
(`RESET`), the reason for each skip (`SKIP`), and an optional independent
Verilog reference to check the module against as well (`REFERENCE`) live in four
tables in `build.rs`. See `design_docs/CORPUS_DIFFERENTIAL_SWEEP.md` and
`design_docs/ANCHORING_A_MODULE.md`.

### Transpiling from the command line

```bash
cargo run -q -p copper-codegen --bin copper-transpile -- examples/sequential/pattern_detector.rs
```

| Flag | Meaning |
|---|---|
| `-o <path>` | write to a file (default: stdout) |
| `--module <name>` | which module to emit (required when a file has more than one) |
| `--profile <p>` | `verilator` (default) \| `generic` \| `yosys` |
| `--hierarchy` | also emit every submodule the target instantiates, deepest-first |
| `--list` | list the hardware modules in the file, then exit |

A **generic** module (`const N: usize`, `Bits<N>`) transpiles to a *parametric*
SystemVerilog module — `copper-transpile examples/combinational/rotate_right.rs`
emits `module rotate_right #(parameter int N = 1, parameter int N_LOG = 1)`. The
CLI cannot choose the widths for you: parameters are often constrained
(`N_LOG == clog2(N)`, asserted inside the module), so concrete values are
supplied only when the module is *Verilated* — `HardwareTest::with_params` in
`copper-sim/src/testing.rs`, or the `PARAMS` table in `build.rs` for the sweep.

---

## Writing hardware

### Module kinds

| Attribute | Shape | Use for |
|---|---|---|
| `#[hardware(combinational)]` | plain `fn`, no ticks | pure logic; every output must be assigned on every path |
| `#[hardware(sequential)]` | `async fn` with one top-level `loop` containing `clk.tick().await` | registers, FSMs, datapaths |
| `#[hardware(synchronizer)]` | `async fn` | the **only** place a clock-domain crossing is legal |
| `#[hardware(structural)]` | no ticks, instantiates children | hierarchy, including multi-clock parents (transpile-only today) |

Every path through the loop body must reach a `clk.tick().await`. A loop that
can spin without ticking is a spanned **compile error**, in both the simulator
front end and the transpiler — a combinational loop is not a thing you can
accidentally write.

### `Out` vs `RegOut`

`Out<T, D>` is a plain wire. `RegOut<T, D>` is a **registered** output driven
from `always_ff`. Use `RegOut` for write-before-tick Moore outputs; plain `Out`
otherwise. A sequential `Out` **holds** its value when a path does not write it
(the enabled-register idiom — verified against BaseJump's `bsg_dff_en`), so
"assign on all paths" is a rule for combinational modules only.

### Clock domains are types

Signals and clocks carry a phantom domain type (`Clock<D>`, `In<T, D>`,
`Out<T, D>`, `D: ClockDomain`). Wiring one domain into another domain's port is
a nominal type mismatch — it fails at `cargo build`, not in a lint or a review.
The only legal crossing is inside a `#[hardware(synchronizer)]` module, such as
the standard library's `sync_2ff`. A module may receive or clone a clock, never
construct one.

`copper-core/src/cdc.rs` contains no runtime code: its `compile_fail` doctests
**are** the executable specification, so `cargo test` fails if a CDC guarantee
regresses.

### Memory

`Memory<T, R, W, D, READ_LAT, WRITE_LAT>` is a first-class construct — `R` read
ports and `W` write ports on domain `D`, with configurable latencies and
read-during-write mode (`ReadFirst` / `WriteFirst`), preloadable via `from_fn` /
`from_contents`. It simulates and transpiles — see
`examples/memory/dual_port_ram.rs`, checked against an independently written
RAM reference (`examples/memory/sv/dual_port_ram.sv`, a textbook simple-dual-port
block-RAM template that Copper had no hand in).

---

## Repository layout

Five crates, plus the root crate as the hardware standard library:

| Crate | Role |
|---|---|
| **`copper-core`** | The type system and every IR in the pipeline: `Logic`, `Bits<N>`, `Clock<D>`, ports, `Memory`, the phantom clock-domain machinery, and the `frontend_ir` / `chir` / `shir` / `vlir` structs. A leaf crate — which is what lets the proc-macro and the transpiler share analysis without a dependency cycle. |
| **`copper-analysis`** | The shared compile-time control/liveness analysis: a real CFG over `syn::ItemFn`, backward-liveness register inference, and the reachability check. Keyed off the representation *both* front ends already hold, so the simulator and the transpiler consume **one** authoritative analysis rather than two that must agree. |
| **`copper-macros`** | The `#[hardware(...)]` proc macro — signature validation, CDC enforcement, per-read freshness guards, and making a module a `HardwareModule` so a bare `async fn` won't simulate. |
| **`copper-codegen`** | The transpiler and the `copper-transpile` CLI. |
| **`copper-sim`** | The async simulation executor and the Verilator equivalence verifier. |
| **`copper`** (root `src/`) | The hardware standard library (e.g. `sync_2ff`), which must live downstream of `copper-sim` because the macro expands to `::copper_sim::…` paths. |

### The transpilation pipeline

```
Rust #[hardware] fn
  → FIR    source-shaped frontend IR
  → control_extract    flatten branch-nested ticks into an explicit `match pc` FSM
  → CHIR
  → SHIR
  → VLIR   1:1 with legal SystemVerilog — legalized names, concrete widths
  → SystemVerilog text
```

### The timing model

A tick has four phases: **pre-edge settle → clock edge → post-edge settle →
post-edge observation**. The production scheduler is `SchedulerMode::Levelized`
(`copper-sim/src/executor.rs`): a topologically-ordered combinational DAG with
Tarjan SCC condensation, evaluated in one canonical pass, so Rust's async poll
order cannot influence the result. The older iterate-to-fixpoint delta-cycle
scheduler is kept permanently as the differential oracle
(`COPPER_SCHEDULER=fixpoint`), and *that* is what the poll-order fuzzer
(`tests/poll_order_fuzz.rs`) exercises — a well-formed design must simulate
identically under any `PollOrder` there, or the levelized-vs-fixpoint comparison
loses its footing. A convergent combinational loop is iterated to a fixed point
(`iterate_scc`); one that is still changing after `OSCILLATION_THRESHOLD` passes
is a **panic** naming the offending SCC, not a hang and not a silent
approximation.

The full semantics live in
[`design_docs/SYNCHRONOUS_SEMANTICS.md`](design_docs/SYNCHRONOUS_SEMANTICS.md).

---

## Verification

Copper's central discipline is that **the transpiler is never treated as a
correctness oracle for the simulator's semantics**. Three kinds of check, in
descending order of authority:

1. **Sim vs independent third-party Verilog** under Verilator — the only kind
   that can adjudicate a timing question. `examples/basejump/` checks Copper
   modules against [BaseJump STL](https://github.com/bespoke-silicon-group/basejump_stl)
   Verilog that Copper had no hand in writing.
2. **Sim vs Copper-generated Verilog** under Verilator, plus a Rust reference
   model — good for datapath equivalence, circular for timing.
3. **Sim vs itself** — self-consistency only.

Register inference is additionally checked *structurally*: the registers the
analysis infers are matched reg-for-reg against the independent reference
Verilog, not merely against matching behavior.

Current state: the counts (tests, examples, sweep cases, ignored cases, anchored
modules) are not copied here, because a hand-copied number goes stale. A full
`tools/regression.sh` run prints them and ends in `REGRESSION OK`, and the
numbers used in the paper are generated into `paper/stats/SUMMARY.md` by
`tools/stats/collect.sh`. The guards G-A through G-E (above) report the example,
sweep and anchoring counts on every run. The pipelined RV32I CPU matches its
Verilated self cycle-for-cycle on all 13 architectural programs
(`tests/rv32i_pipelined_verilator.rs`).

Every ignored test prints its reason on each run. They are pinned one-cycle
divergence witnesses that keep `allow_pretick_alignment` on purpose, shapes
refused by design, a structural parent with no simulatable body, the pipelined
CPU (anchored in its own lane because the sweep cannot supply its `Memory`
parameter), diagnostic printouts with no assertions, and one documented startup
transient — not silent skips. `tools/regression.sh` prints the full list with
reasons at the end of every run (the sweep's come from the `SKIP` table in
`build.rs`; three more are hand-written under `tests/`).

---

## Limitations

Measured, not guessed — **34 of 34** `#[hardware]` modules in `examples/`
transpile as of 2026-09-01 (`tools/transpile_coverage.sh` prints the current
number). The cause ledger that used to sit here — `Vec` ports, tuple-returning
helpers, struct pipeline latches, const match patterns, the word-indexed
register file, the whole-file `spawn_uart` rejection — is empty as of
2026-08-27; its full history lives in [`TODO`](TODO), the engineering journal
and cause ledger (dated entries, not a curated open list).

Transpiling is not the same as being *checked*: coverage counts acceptance, and
the corpus sweep above is what says the emitted SystemVerilog agrees with the
simulator. The equivalence harness runs Verilator under `-Wall`, and warnings
are failures.

What remains unsupported falls into two kinds.

**Decisions, not gaps** — constructs that cannot be given the same meaning in
simulation and in silicon are refused with a spanned error and a recorded
counterexample rather than lowered to something plausible:

- **An `In` read placed after a delay is not refused.** A consumer that waits
  for `valid`, ticks twice, and only then reads `data`
  (`while valid.read() == Logic::Zero { clk.tick().await; } clk.tick().await;
  clk.tick().await; acc = acc + data.read();`) is accepted by the macro and by
  the transpiler, and the differential sweep finds the simulator and the
  emitted SystemVerilog agree on every cycle
  (`tests/fixtures/valid_ready_dut.rs::vr_read_after_delay`, measured
  2026-10-20). The read opens its own segment, so it samples at that cycle's
  opening on both sides. Earlier editions of this list said the shape was
  refused by design; it is not, and no rule targets it. The read-first,
  delay-last spelling in `examples/uart/rx.rs` is the one anchored against an
  independent reference, not the only one that works.
- **A register's power-on value must be stateable.** A pre-loop `let mut` the
  liveness inference calls a register must be initialized from a constant (a
  literal, `Bits::zero()`, `Bits::from_u32(k)`, `Bits::from_lit::<K>()`, an enum
  variant, an array of those) or from `Bits::x()`. The constant reaches the emitted
  SystemVerilog as the register's `initial` value, so the simulator and the hardware
  start together without a reset; `Bits::x()` states nothing and says the design
  needs a reset; anything else — an input read, a local, a free function — is
  refused by both front-ends (`check_register_initializers`, 2026-10-29), because
  the hardware has no power-on value for it. `initial` is honored by simulators and
  FPGA tools and ignored by most ASIC flows.
- **The pre-tick alignment family**: four compile-time rules refuse plain-`Out`
  drive shapes whose phase would silently differ between the simulator and the
  synthesized FSM: before the first tick, a port with *mixed anchors* (an input
  read reaches some write of it while some write has a read-free path from the
  loop head) or an opening write guarded by a test of a register the same segment
  just assigned (`unprotected_pretick_out_write`, re-derived 2026-10-28 on a
  sixteen-probe battery — a constant written in a state arm no read reaches, or
  behind its own read, is accepted); a port written in more than one clock phase
  where the two sides anchor a write differently (a port none of whose writes an
  input read reaches, or one whose every write follows an input read a tick
  follows, is accepted); and a write that precedes its register's update. The
  remedy each error names is usually `RegOut`. Five *rejected* fixes are recorded with measured
  evidence in
  [`design_docs/PRETICK_ALIGNMENT_GUARDRAIL.md`](design_docs/PRETICK_ALIGNMENT_GUARDRAIL.md)
  so they are not re-tried.
- **A `RegOut` written after the last tick from an input read directly**: a
  `RegOut` write takes effect at the first clock edge after it executes, so such
  a write commits at the next edge, where the flip-flop samples the input one
  drive later than the simulator's read. Read the input before the tick, or
  through a register (`trailing_regout_reads_input`).
- **A plain `Out` written after the last tick from an input read directly, in a
  body that crosses more than one clock edge**: the simulator reads the input the
  instant the write executes; the emitted drive samples it one observation later
  (edge-sampled on the extraction route, an ungated continuous assign on the
  linear multi-tick route). A single-tick body agrees and is not refused. Read
  the input before the tick, or through a register
  (`trailing_out_reads_input`; the `e8` probes in
  `tests/sequential_forwarding_divergence.rs` measure every variant).
- **A constant-only `Out` first written after the second clock edge** of a
  multi-tick body: the simulator shows the initial value at the first
  observation, the emitted `assign` drives the constant from time 0. Write it in
  the head segment too, or declare it `RegOut` (`out_first_driven_late`; the same
  constant one edge earlier agrees and is kept as the positive witness).
- A **wait loop that ticks before testing** its condition (a repeating wait must
  be `loop { <test>; clk.tick().await; }`) — refused by both front-ends
  (`copper_analysis::check_nested_loop_shape`), and `continue` inside a nested
  loop — refused by the transpiler with its own diagnostic
  (`copper-codegen/src/control_extract.rs`).
- **Zero-latency memories** (`READ_LAT` and `WRITE_LAT` must both be at least
  1), a memory port accessed more than once in one cycle
  (`check_memory_staging`), a plain `Out` driven from a memory read result
  across clock phases (`memory_result_drives_plain_out`), and memory preloads
  that are not a closure written at the `from_fn` call site — memory shapes
  that would lower to something subtly wrong.
- **Two writes to one array register in a cycle**: `let mut regs = [Bits<W>; N]`
  lowers to storage with one write port; a second write statement in the same
  cycle is refused with the muxed-address rewrite suggested.
- `while` loops (write `loop { …; clk.tick().await; }`), `..rest` struct
  update, and or-patterns that bind a name — each a spanned "not supported"
  error from `copper-codegen/src/chir_lower.rs`.

**Gaps** — expressible in the simulator, not yet in the transpiler:

- the division operator `/` (`%` works — see `lower_binop` in
  `copper-codegen/src/chir_lower.rs`);
- a behavioral module is **single-clock** (the macro requires exactly one
  `Clock<D>` parameter); multi-clock designs compose clock domains through
  `#[hardware(structural)]` parents, which transpile but have no simulatable
  body — the simulator wires the hierarchy by hand;
- a module that *receives* a `Memory` parameter emits a bus the corpus sweep
  cannot drive by itself; anchoring one behaviorally takes a Verilated parent
  that owns the array (`tests/rv32i_pipelined_verilator.rs` is the pattern).

**Pinned divergences** — shapes the simulator and the transpiler still disagree
on by one cycle, kept as `#[ignore]`d sweep cases with the reason attached
rather than papered over (`SKIP` in `build.rs`):

- `branch_merge_explicit`'s `tail_o`, a mixed-anchor port (written behind an
  input read and, in another state, with no read before it) that leads its SV by
  one cycle when the first cycle takes the `sel == 1` path — and the same port
  of its live, unflagged async twin `branch_merge`, which transpiles to the
  byte-identical SV and which no rule reaches. The single-seed sweep never
  produced that first cycle for either; the multi-sequence stimulus (above) does,
  and both rows record the measured trace. (`probe_fsm`, listed here until
  2026-10-26, agrees and sweeps.)

The phase-E probe battery (2026-10-23) found two single-phase shapes that
disagreed by one cycle and that no rule guarded; both are closed as of
2026-10-26. **e8** (a plain `Out` written after the last tick from an input read in
the same statement) is refused by `trailing_out_reads_input`, listed above. **e9**
(an opening write from a committed register, demoted to an edge-sampled drive by an
input read in *another* FSM state) was fixed by making the transpiler's
opening-prefix walk arm-granular, and the same change brought the D1 witnesses
`pc_arm_*` and `probe_fsm` into agreement (`design_docs/CYCLE_DATAFLOW_SEMANTICS.md`
§9.4 has both derivations). D1 itself was then re-derived against that emission
(2026-10-28, §10 there): the rule now refuses exactly the two head-segment hazards
the model derives, its constant-write clause is gone, and four measured
divergences it had missed are refused.

One correctness rule is worth knowing when reading the generated Verilog: an
output drive that is *sampled at the clock edge* is emitted from the values the
registers hold **before** that edge, while one emitted as a continuous `assign` is
read **after** it. Copper carries both forms and picks at the point where the
choice actually exists, because "is this drive edge-sampled?" is not answerable
from the port type — a plain `Out` written conditionally becomes a register too.
Getting this wrong is a silent one-cycle error, so it is pinned by
`tests/regout_forwarding_equivalence.rs` rather than left to inspection.

And X-propagation cannot be checked against Verilator even in principle
(Verilator is 2-state), so Copper's 4-state behavior is pinned by its own tests
instead.

[`TODO`](TODO) is the engineering journal and cause ledger — dated entries
recording what was measured, decided and closed, not a curated list of what is
still open.

---

## Documentation

| | |
|---|---|
| [`design_docs/SYNCHRONOUS_SEMANTICS.md`](design_docs/SYNCHRONOUS_SEMANTICS.md) | the timing model — start here |
| [`design_docs/PRETICK_ALIGNMENT_GUARDRAIL.md`](design_docs/PRETICK_ALIGNMENT_GUARDRAIL.md) | the pre-tick alignment family — its compile-time rules, and the rejected fixes with measured evidence |
| [`design_docs/CORPUS_DIFFERENTIAL_SWEEP.md`](design_docs/CORPUS_DIFFERENTIAL_SWEEP.md) | why every module gets a generated sim-vs-emitted-SV case, and how |
| [`design_docs/ANCHORING_A_MODULE.md`](design_docs/ANCHORING_A_MODULE.md) | how to add an independent Verilog reference to a swept module (the `REFERENCE` table) |
| [`design_docs/LEVELIZED_SCHEDULING_SCOPE.md`](design_docs/LEVELIZED_SCHEDULING_SCOPE.md) | the levelized scheduler |
| [`design_docs/ARRAY_PORT_ABI.md`](design_docs/ARRAY_PORT_ABI.md), [`design_docs/RECEIVED_MEMORY_ABI.md`](design_docs/RECEIVED_MEMORY_ABI.md) | the emitted port shapes for array-typed ports and for a `Memory` received as a parameter |
| [`design_docs/CYCLE_DATAFLOW_SEMANTICS.md`](design_docs/CYCLE_DATAFLOW_SEMANTICS.md), [`design_docs/DERIVATION_TABLE.md`](design_docs/DERIVATION_TABLE.md), [`design_docs/PAIRED_IMPLEMENTATION_SCOPE.md`](design_docs/PAIRED_IMPLEMENTATION_SCOPE.md) | the cycle-dataflow denotation behind the pre-tick rules, and its derivation across the corpus |
| [`design_docs/TIMING_MODEL_UNIFICATION.md`](design_docs/TIMING_MODEL_UNIFICATION.md) | how far the simulator's and the transpiler's timing derivations actually diverge — measured |
| [`design_docs/TIMING_COVERAGE_MATRIX.md`](design_docs/TIMING_COVERAGE_MATRIX.md) | which timing patterns have an independent hardware anchor |
| [`design_docs/SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md`](design_docs/SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md) | the historical implementation plan for the executor/analysis architecture — its status notes are dated; `SYNCHRONOUS_SEMANTICS.md` is normative where they disagree |
| [`CLAUDE.md`](CLAUDE.md) | orientation for contributors (and for Claude Code) |

`design_docs/OUTDATED/` is history, not authority.

---

## License

The Copper source is licensed under the MIT License (see `LICENSE`; the
copyright holder and year there are placeholders to be confirmed by the author).
The BaseJump STL-derived reference Verilog listed below is under the Solderpad
Hardware License v0.51 and keeps that license: see `LICENSE-Solderpad`.

The Verilog vendored as **independent references** under `examples/*/sv/` and
`tests/fixtures/reference_sv/` comes from three sources, and each file's header
is the record:

- **BaseJump STL, under the Solderpad Hardware License v0.51**
  (Apache-2.0-based), Copyright 2016 Michael B. Taylor / BaseJump STL
  contributors — the seven files in `examples/basejump/sv/`, plus
  `examples/combinational/sv/mux.sv`, `ripple_carry_adder.sv`,
  `rotate_right.sv` and `examples/sequential/sv/lfsr.sv`. Each carries a header
  naming the upstream module, the license, and the adaptations made
  (BaseJump macros stripped, widths pinned, `clk_i` renamed).
- **Hand-written for this repository**, and saying so in the header:
  `examples/cdc/sv/sync_2ff_ref.sv`, `examples/cdc/sv/two_domain_hierarchy.sv`,
  `examples/sequential/sv/pattern_detector_010.sv`,
  `tests/fixtures/reference_sv/ram_read_first.sv`; the other files under
  `tests/fixtures/reference_sv/` are test-specific reference models written for
  the tests that name them.
- **Adapted from third-party tutorial or template sources**:
  `examples/sequential/sv/shift_register.sv` (chipverify.com, cited in its
  header), `examples/combinational/sv/priority_encode.sv` (Greg Stitt, University
  of Florida, cited in its header), and three whose headers record no provenance
  — `examples/memory/sv/dual_port_ram.sv` (a textbook simple-dual-port block-RAM
  template), `examples/sequential/sv/pattern_detector.sv`, and
  `examples/combinational/sv/one_bit_comparator.sv`. No license is recorded for
  any of these five.

`examples/counter_enable.sv` is tracked but referenced by nothing in the tree.

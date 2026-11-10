# Copper

**A Rust-embedded hardware description language.** You write hardware as
`#[hardware]`-annotated async Rust functions. Copper both **simulates** them,
with a custom async executor that resumes each module once per clock edge, and
**transpiles** them to synthesizable SystemVerilog.

```rust
#[hardware(combinational)]
fn one_bit_comparator<D: ClockDomain>(i0: In<Logic, D>, i1: In<Logic, D>, eq: Out<Logic, D>) {
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

The project's correctness bar is that **the two agree**. Every module in the corpus is
run through the Copper simulator, through a reference interpreter of the specification,
and as its transpiled SystemVerilog under Verilator, and the three must agree
cycle by cycle. The sharpest checks compare Copper against *independent, third-party*
Verilog rather than against Copper's own output.

> **Status: research prototype (v0.1.0).** The semantics are specified in
> [`design_docs/SPEC.md`](design_docs/SPEC.md) (normative since 2026-11-09) and every
> clause has a line in its conformance ledger, which the regression checks. The language
> surface is deliberately narrow. Every `#[hardware]` module under `examples/` transpiles
> (40 of 40, measured 2026-11-09; `tools/transpile_coverage.sh` re-measures it). See
> [Limitations](#limitations) for what is refused and why.

---

## Why async Rust?

The central idea is that `clk.tick().await` **is** a clock-cycle boundary:

- every suspension point becomes a state in the generated FSM;
- every value **live across a tick** becomes a **register** (inferred by
  backward liveness analysis, not by an annotation you write);
- everything between ticks is combinational logic.

So an ordinary Rust `loop` with an `enum` state variable is a state machine, and
you get to keep Rust's type system, generics, `match`, and tooling while writing
it. Here is a real example, `examples/sequential/pattern_detector.rs`, a
"110101" sequence detector, and the SystemVerilog Copper emits for it:

```rust
#[hardware(sequential)]
async fn det_110101(
    clk: Clock<MainClk>,
    rstn: In<Logic, MainClk>,
    in_i: In<Logic, MainClk>,
    out_o: RegOut<Logic, MainClk>,
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

    initial begin
        state = 3'd0;
        out_o = 1'b0;
    end

    /* verilator lint_off MULTIDRIVEN */
    always_ff @(posedge clk) begin
        if ((((rstn == 1'b0) ? 3'd0 : (({state, in_i} == 4'd1) ? 3'd1 : (({state, in_i} == 4'd3) ? 3'd2 : (({state, in_i} == 4'd4) ? 3'd3 : (({state, in_i} == 4'd7) ? 3'd4 : (({state, in_i} == 4'd8) ? 3'd5 : (({state, in_i} == 4'd11) ? 3'd6 : 3'd0))))))) == 3'd6)) begin
            out_o <= 1'b1;
        end else begin
            out_o <= 1'b0;
        end
        state <= ((rstn == 1'b0) ? 3'd0 : (({state, in_i} == 4'd1) ? 3'd1 : (({state, in_i} == 4'd3) ? 3'd2 : (({state, in_i} == 4'd4) ? 3'd3 : (({state, in_i} == 4'd7) ? 3'd4 : (({state, in_i} == 4'd8) ? 3'd5 : (({state, in_i} == 4'd11) ? 3'd6 : 3'd0)))))));
    end
    /* verilator lint_on MULTIDRIVEN */

endmodule
```

That block is the CLI's output verbatim, measured 2026-11-09 with
`cargo run -q -p copper-codegen --bin copper-transpile -- examples/sequential/pattern_detector.rs`.
The long lines are the `match` on `(state, in_i)` lowered to a nested ternary over
the concatenation `{state, in_i}`. The `RegOut` is driven from the value `state` is
assigned in the same cycle, which is Rust's program order: the write reads the
`state` the statement before it computed. Nobody declared a register. `state` became
one because it is assigned in the loop and read after the tick. Its initializer
reaches the SV as its `initial` value.

---

## Requirements

| | |
|---|---|
| **Rust** | rustc **1.98.1**, pinned by `rust-toolchain.toml` (rustup installs it on the first cargo command). The trybuild `.stderr` files pin rustc's diagnostic text, so every machine builds with that exact version. The crates are `edition = "2024"` |
| **Verilator** | required for examples and equivalence tests: `brew install verilator` (developed on 5.044; the numbers in the paper were generated with 5.051) |
| **Icarus Verilog** | *optional*: `brew install icarus-verilog` (developed on 12.0). Used only by the simulator-throughput benchmark `tests/sim_throughput.rs` and by `tools/stats/simperf.py`. When `iverilog` is absent, that test skips its Icarus leg (`iverilog_available`) and the stats script refuses to write a CSV |

Verilator is not optional for a full regression run. A genuinely *absent*
binary is skipped with an explicit marker (`VERILATOR_NOT_INSTALLED` in
`copper-sim/src/verification.rs`); an installed-but-broken one fails loudly, by
design. "The check silently didn't run" is a bug class this project takes
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
(`examples/cpu/rv32i_cpu_pipelined.rs`, with forwarding, load-use stalls, branch
flush, a word-indexed register file, and a unified instruction/data memory
received as a parameter). It runs 13 architectural programs on the simulator
**and** on its own transpiled SystemVerilog under Verilator, matching
cycle for cycle:

```bash
cargo test --test rv32i_pipelined_verilator
```

Run the full regression: build, CLI, `cargo test --workspace`, and **every**
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

The driver also enforces ten wiring guards, because "the check silently didn't run"
has been a recurring bug class here. `tools/regression.sh`'s header describes each:

- every `examples/**.rs` is registered (**G-A**), and every registered example ran (**G-B**);
- every `tests/*.rs` produced a test binary that ran (**G-C**);
- the corpus sweep covered every module and ran with Verilator (**G-D**), and how much of
  it is anchored to independent Verilog is printed (**G-E**);
- the reference-interpreter leg ran (**G-F**);
- every swept case's verdict under the agreement relation was written, with no
  disagreement (**G-G**);
- the projection test (**G-I**), the constant-evaluator test (**G-J**) and the
  conformance ledger (**G-K**) ran and passed.

It prints every `#[ignore]`d test on each run, so a deliberately skipped check stays
visible.

### The corpus differential sweep

`build.rs` generates two cases per `#[hardware]` module in `examples/` and
`tests/fixtures/` (not `src/`, and not modules defined inline in a `tests/*.rs` file).
A `_differential` case drives seeded random stimulus into the simulator, the reference
interpreter (`copper-ref`) and the SystemVerilog the module transpiles to, under
Verilator. A `_reference_leg` case runs the interpreter alone. No hand-written reference
model is needed: the three are independent implementations of one source, so comparing
them is already an oracle, and that makes a case cheap enough to have for *every* module.
The verdict is the agreement relation of SPEC §III.2: per observation, observation 0
included, with X against the 2-state SV reported as OUTSIDE rather than compared.

Each case drives **several stimulus sequences**, not one (since 2026-10-28):
- four independent seeds of 200 cycles;
- a 32-cycle sequence starting from *every* initial input vector, for a module with at most
  4 input bits (a fixed first-cycle pattern list for a wider one);
- the whole truth table, for a combinational module of up to 8 input bits.

The constants are `SEEDS`, `SEED_CYCLES`, `FIRST_CYCLES`, `EXHAUSTIVE_BITS`,
`COMB_EXHAUSTIVE_BITS` and `COMB_VECTORS` in `build.rs`. The sequences run back to back in
one testbench against one Verilated model, restarted from its initial state at each
boundary, so the cost stays one Verilator build per module. A failure names the sequence
it fell in, and `COPPER_SWEEP_SEQUENCE=<index>` re-runs that sequence alone.

A module the sweep cannot run gets an `#[ignore]` with its reason, never an
omission, and G-D fails if the generator quietly stops covering something. What
cannot be inferred from a signature lives in reviewed tables in `build.rs`:
- `PARAMS`: widths for generic modules;
- `RESET`: resets for designs whose state starts undefined;
- `SKIP`: the reason for each skip;
- `REFERENCE`: an optional independent Verilog reference to check the module against as
  well;
- `REF_SKIP` and `REF_SKIP_PATTERN`: the interpreter leg's skips, both empty.

See `design_docs/CORPUS_DIFFERENTIAL_SWEEP.md` and `design_docs/ANCHORING_A_MODULE.md`.

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
SystemVerilog module: `copper-transpile examples/combinational/rotate_right.rs`
emits `module rotate_right #(parameter int N = 1, parameter int N_LOG = 1)`. The
CLI cannot choose the widths for you. Parameters are often constrained
(`N_LOG == clog2(N)`, asserted inside the module), so concrete values are
supplied only when the module is *Verilated*: `HardwareTest::with_params` in
`copper-sim/src/testing.rs`, or the `PARAMS` table in `build.rs` for the sweep.

---

## Writing hardware

### Module kinds

| Attribute | Shape | Use for |
|---|---|---|
| `#[hardware(combinational)]` | plain `fn`, no ticks | pure logic; every output must be assigned on every path |
| `#[hardware(sequential)]` | `async fn` with one `Clock<D>` and one top-level `loop` containing `clk.tick().await` | registers, FSMs, datapaths |
| `#[hardware(synchronizer)]` | `async fn` | the **only** place a clock-domain crossing is legal |
| `#[hardware(structural)]` | no ticks, instantiates children and wires them with `net::<T, D>()` | hierarchy, including multi-clock parents (transpile-only; the simulator wires the children by hand with `spawn_wired`) |

Instantiation is structural-only: calling a child module from a `sequential`,
`synchronizer` or `combinational` body is a compile error.

Every path through the loop body must reach a `clk.tick().await`. A loop that
can spin without ticking is a spanned **compile error**, in both the simulator
front end and the transpiler. A combinational loop is not a thing you can
accidentally write.

### `RegOut` and `Out`

Every output of a clocked module is a `RegOut<T, D>`, a **registered** output. A write in
the cycle that ends at edge N+1 is shown from that edge on, and a `RegOut` left unwritten
**holds** its value (the enabled-register idiom, verified against BaseJump's `bsg_dff_en`).
`Out<T, D>`, a plain wire, is for combinational modules only; a plain `Out` on a clocked
module is refused by both front-ends (SPEC II.1, R8). A value that must follow an input within
the cycle (a passthrough, a Mealy output) is a combinational module wired to the clocked
module's `RegOut`s.

### Clock domains are types

Signals and clocks carry a phantom domain type (`Clock<D>`, `In<T, D>`,
`Out<T, D>`, `D: ClockDomain`). Wiring one domain into another domain's port is
a nominal type mismatch: it fails at `cargo build`, not in a lint or a review.
The only legal crossing is inside a `#[hardware(synchronizer)]` module, such as
the standard library's `sync_2ff`. A module may receive or clone a clock, never
construct one. `()` is not a clock domain. A combinational module that serves any
domain is generic over it (`fn m<D: ClockDomain>(…)`, as `one_bit_comparator` above).

`copper-core/src/cdc.rs` contains no runtime code: its `compile_fail` doctests
**are** the executable specification, so `cargo test` fails if a CDC guarantee
regresses.

### Memory

`Memory<T, R, W, D, READ_LAT, WRITE_LAT>` is a first-class construct: `R` read
ports and `W` write ports on domain `D`, with configurable latencies (at least 1) and
read-during-write mode (`.read_first()` / `.write_first()`), preloadable via `from_fn` /
`from_contents`. It simulates and transpiles. See
`examples/memory/dual_port_ram.rs`, checked against an independently written
RAM reference (`examples/memory/sv/dual_port_ram.sv`, a textbook simple-dual-port
block-RAM template that Copper had no hand in).

---

## Repository layout

Six crates, plus the root crate as the hardware standard library:

| Crate | Role |
|---|---|
| **`copper-core`** | The type system and every IR in the pipeline: `Logic`, `Bits<N>`, `Clock<D>`, ports, `Memory`, the phantom clock-domain machinery, and the `frontend_ir` / `chir` / `shir` / `vlir` structs. A leaf crate, which is what lets the proc-macro and the transpiler share analysis without a dependency cycle. |
| **`copper-analysis`** | The shared compile-time analysis both front-ends run: a CFG over `syn::ItemFn`, backward-liveness register inference, the reachability check, the front-end rules (`check_front_end_rules`), the memory rules and the constant evaluator. Keyed off the representation *both* front ends already hold, so the macro and the transpiler consume **one** analysis rather than two that must agree. |
| **`copper-macros`** | The `#[hardware(...)]` proc macro: signature validation, CDC enforcement, the shared rules, and making a module a `HardwareModule` so a bare `async fn` won't simulate. |
| **`copper-codegen`** | The transpiler and the `copper-transpile` CLI. |
| **`copper-sim`** | The async simulation executor and the Verilator equivalence verifier. |
| **`copper-ref`** | The reference interpreter: the specification made executable, the sweep's third leg (`copper-ref/README.md`). |
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

Per tick of a clock domain `D`, `HardwareExecutor::tick_clock::<D>` runs four steps:
1. it settles the combinational network on the driven inputs;
2. it resumes each of `D`'s coroutines **once**, to its next `tick`;
3. it commits the edge;
4. it settles the network again, for the observation.

Only `D`'s ticks resume `D`'s coroutines (SPEC I.0.2).

The production scheduler is `SchedulerMode::Levelized` (`copper-sim/src/executor.rs`): a
topologically ordered combinational DAG, evaluated in one canonical pass, so Rust's async
poll order cannot influence the result. A combinational cycle is refused when the module is
spawned. The older iterate-to-fixpoint delta-cycle scheduler is kept permanently as the
differential oracle (`COPPER_SCHEDULER=fixpoint`), and *that* is what the poll-order fuzzer
(`tests/poll_order_fuzz.rs`) exercises. A well-formed design must simulate identically under
any `PollOrder` there.

The semantics are [`design_docs/SPEC.md`](design_docs/SPEC.md); start with its one-page
generator, [`design_docs/SPEC_OVERVIEW.md`](design_docs/SPEC_OVERVIEW.md).

---

## Verification

Copper's central discipline is that **the transpiler is never treated as a
correctness oracle for the simulator's semantics**. Four kinds of evidence, in
descending order of authority:

1. **Sim vs independent third-party Verilog** under Verilator, the only kind
   that can adjudicate a timing question. `examples/basejump/` checks Copper
   modules against [BaseJump STL](https://github.com/bespoke-silicon-group/basejump_stl)
   Verilog that Copper had no hand in writing; the sweep's `REFERENCE` rows add
   references the user wrote by hand (G-E counts them).
2. **The three legs of the sweep**: the simulator, the reference interpreter and the
   transpiled SV, judged under SPEC §III.2's agreement relation. Agreement of the three is
   consistency, not correctness. A disagreement says which side to suspect.
3. **Static propositions** of SPEC §III.1: register inference matched against the
   transpiler's flip-flops, and the projection relating the emitted FSM's states to
   the coroutine's tick sites (G-I).
4. **Sim vs itself**, self-consistency only.

The conformance ledger, SPEC §III.3, has one line per numbered clause, rule and lowering
row, each CLOSED with the fixtures that show it or OPEN with an owner. G-K checks that every
CLOSED line's fixtures ran and passed in the same run.

The counts (tests, examples, sweep cases, ignored cases, anchored modules, ledger lines)
are not copied here, because a hand-copied number goes stale. A full `tools/regression.sh`
run prints them and ends in `REGRESSION OK`, and the numbers used in the paper are
generated into `paper/stats/SUMMARY.md` by `tools/stats/collect.sh`. Every ignored test
prints its reason on each run, and the sweep's reasons come from the `SKIP` table in
`build.rs`.

X-propagation cannot be checked against Verilator even in principle, because Verilator is
2-state. A simulator X at an observation therefore takes that sequence out of the SV
comparison (OUTSIDE) rather than being compared as 0 or 1, and Copper's 4-state behavior
is pinned by its own tests and by the interpreter.

---

## Limitations

What the language admits is SPEC Part I and the rules of Part II. Each refusal is in one of
two lists:
- **II.1/II.2**, refused by design. These are shapes with no meaning or no single spelling:
  a non-`tick` `await`, a loop path that never ticks, a plain `Out` on a clocked module, a
  second clock on a behavioural module, a register whose power-on value is not a constant.
- **II.3**, a dated lowering limitation, with a fix direction.

The macro and the transpiler refuse the same shapes with the same message. **Four named
exceptions** are pinned so they cannot widen: recursion through a helper `fn`, a let-bound
instance, a pattern naming an X-valued file const, and an integer method the transpiler does
not lower (`copper_codegen::LOWERED_INT_METHODS` lists the ones it does). Each compiles under
rustc and is refused by the transpiler, which emits no text for it. `to_be`/`to_le` depend on
the host's byte order and are refused by both.

The conformance ledger (SPEC §III.3.4) has no OPEN line since 2026-11-10.

Other boundaries:
- A behavioural module is **single-clock**. Multi-clock designs compose clock domains through
  `#[hardware(structural)]` parents, which transpile but have no simulatable body: the
  simulator wires the hierarchy by hand.
- A module that *receives* a `Memory` parameter emits a bus the corpus sweep cannot drive by
  itself. Anchoring one behaviorally takes a Verilated parent that owns the array
  (`tests/rv32i_pipelined_verilator.rs` is the pattern).
- A register's power-on value is its constant initializer, emitted as an `initial` value;
  `Bits::x()` states none and needs a reset. `initial` is honored by simulators and FPGA
  tools and ignored by most ASIC flows.

Transpiling is not the same as being *checked*. Coverage counts acceptance, and the
sweep is what says the emitted SystemVerilog agrees. The equivalence harness runs Verilator
under `-Wall`, and warnings are failures.

[`TODO`](TODO) is the engineering journal and cause ledger: dated entries recording what was
measured, decided and closed, not a curated list of what is still open. The measurement log
of the specification work is `design_docs/review/MEASUREMENTS.md`.

---

## Documentation

| | |
|---|---|
| [`design_docs/SPEC.md`](design_docs/SPEC.md) | **the language specification — normative**; Part I the semantics, Part II the rules, Part III the implementations' obligations and the conformance ledger |
| [`design_docs/SPEC_OVERVIEW.md`](design_docs/SPEC_OVERVIEW.md) | the semantics on one page; start here |
| [`design_docs/SPEC_DECISIONS.md`](design_docs/SPEC_DECISIONS.md) | the decision log, D-1…D-31, each with its options, anchor and date |
| [`design_docs/review/MEASUREMENTS.md`](design_docs/review/MEASUREMENTS.md) | the dated measurement log behind the decisions and the implementation |
| [`copper-ref/README.md`](copper-ref/README.md) | the reference interpreter |
| [`design_docs/CORPUS_DIFFERENTIAL_SWEEP.md`](design_docs/CORPUS_DIFFERENTIAL_SWEEP.md) | why every module gets a generated three-leg case, and how |
| [`design_docs/ANCHORING_A_MODULE.md`](design_docs/ANCHORING_A_MODULE.md) | how to add an independent Verilog reference to a swept module (the `REFERENCE` table) |
| [`design_docs/ARRAY_PORT_ABI.md`](design_docs/ARRAY_PORT_ABI.md), [`design_docs/RECEIVED_MEMORY_ABI.md`](design_docs/RECEIVED_MEMORY_ABI.md) | the emitted port shapes for array-typed ports and for a `Memory` received as a parameter |
| [`design_docs/TIMING_COVERAGE_MATRIX.md`](design_docs/TIMING_COVERAGE_MATRIX.md) | which timing patterns have an independent hardware anchor |
| [`design_docs/PUBLICATION_CHECKLIST.md`](design_docs/PUBLICATION_CHECKLIST.md) | what blocks a public release: unlicensed third-party references, provenance, open items |
| [`CLAUDE.md`](CLAUDE.md) | orientation for contributors (and for Claude Code) |

The other documents in `design_docs/` are historical and say so in a dated header:
`SYNCHRONOUS_SEMANTICS.md` (the reference before SPEC), `CYCLE_DATAFLOW_SEMANTICS.md`,
`PRETICK_ALIGNMENT_GUARDRAIL.md`, `DERIVATION_TABLE.md`, `PAIRED_IMPLEMENTATION_SCOPE.md`,
`TIMING_MODEL_UNIFICATION.md`, `LEVELIZED_SCHEDULING_SCOPE.md`, `FORMAL_SEMANTICS*.md`,
`SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md`. `design_docs/OUTDATED/` is history, not authority.

---

## License

Copper is licensed under either of

- Apache License, Version 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE) or
  <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([`LICENSE-MIT`](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Copper by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

The exception is the BaseJump STL-derived reference Verilog listed below: it is
under the Solderpad Hardware License v0.51 and keeps that license (see
`LICENSE-Solderpad`), not the dual license above.

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
  the tests that name them, except `bit_not_bits.sv`, which was written in a
  session, is no longer a `REFERENCE` row and is read by nothing (2026-11-10).
- **Adapted from third-party tutorial or template sources**:
  `examples/sequential/sv/shift_register.sv` (chipverify.com, cited in its
  header), `examples/combinational/sv/priority_encode.sv` (Greg Stitt, University
  of Florida, cited in its header), and three whose headers record no provenance
  — `examples/memory/sv/dual_port_ram.sv` (a textbook simple-dual-port block-RAM
  template), `examples/sequential/sv/pattern_detector.sv`, and
  `examples/combinational/sv/one_bit_comparator.sv`. No license is recorded for
  any of these five. The first three block a public release until the user replaces
  them; [`design_docs/PUBLICATION_CHECKLIST.md`](design_docs/PUBLICATION_CHECKLIST.md)
  lists them with every other open item.

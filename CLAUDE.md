# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Copper is a Rust-embedded HDL. You write hardware as `#[hardware]`-annotated
async Rust functions; Copper both **simulates** them (a custom async executor)
and **transpiles** them to SystemVerilog. The project's correctness bar is that
the two agree: most examples run the Copper simulator, emit SystemVerilog + a
Verilator C++ testbench, compile it with Verilator, and assert the two match
cycle-by-cycle.

It is a **library/CLI workspace, not a GUI or server** — "does it work?" means
the example equivalence check passes, and there is nothing to screenshot.

## Commands

The driver script is the regression command. **Bare, it runs everything** —
build, CLI, `cargo test --workspace`, and *every* registered example — and exits
non-zero on the first failure (a clean full run ends with `REGRESSION OK`; any
subset ends with `PARTIAL OK`, so it cannot be mistaken for one):

```bash
tools/regression.sh
```

It lives in `tools/` **because it must be in version control**: `.gitignore` excludes
`.claude/`, so anything under a local skill directory is invisible to everyone else
and lost on a fresh clone. If a local `run-copper` skill wrapper exists, it should be
nothing more than an `exec` of this script — `tools/regression.sh` is the driver.

It also enforces seven wiring guards, because "the check silently didn't run" has
been a recurring bug class here: **G-A** every `examples/**.rs` is registered as a
`[[example]]`, **G-B** every registered example actually ran, **G-C** every
`tests/*.rs` (root and per-crate) produced a test binary that ran, **G-D** the
corpus differential sweep covered every `#[hardware]` module in `examples/` and
`tests/fixtures/` and ran with Verilator present, **G-E** (printed, not
enforced) how many swept modules are additionally anchored to an independent
hand-written Verilog reference, **G-F** the reference-interpreter leg ran, and
**G-G** every swept case wrote its per-sequence verdicts under the agreement
relation of `design_docs/SPEC.md` §III.2 (`tests/common/relation.rs`, a ledger in
`COPPER_VERDICT_LEDGER`) and each count is printed. Since 2026-11-06 observation 0
(after the first settle, before edge 1) is compared on all three legs. **OUTSIDE**: a
simulator X at an observation compared against the 2-state SV takes that sequence's
SV pair out of the relation from there — reported, never compared as 1 (the old
`logic_vec_to_int` behaviour) and never masked. **TRUNCATED**: a run-time failure (an
undelivered `data()`, an out-of-range address, an X fed to a condition or `as_*`,
integer overflow) ends the trace, the simulator and the interpreter must fail at the
same resumption, and the SV is not compared beyond it; any other panic fails the case.
**EXCUSED**: a disagreement a signature class excuses, counted and printed with its
pattern — none today (the observation-0 plain-`Out` class of 25 modules emptied with
the D-8 rewrites and was deleted, Phase C step 3). **DISAGREE**: every disagreement prints its pattern —
`sim≠ref,SV=ref` simulator/macro, `sim=ref≠SV` transpiler or II.3 row (none pinned since
Phase C step 5: the pre-loop pair `preloop_probe`/`preloop_linear`, II.3 (f2)/(f3), agrees
and is a swept fixture; the trailing fold's P12 and (f4) agree since step 4), `sim=SV≠ref` the reference, `all-differ` spec gap or interpreter — and G-G
fails on any swept disagreement and on a `REF_SKIP` measured-disagreement row whose
pattern (`REF_SKIP_PATTERN`) no pin measured this run. The sweep does not reach `src/` (the standard
library's `sync_2ff` is covered by its `tests/fixtures/` twin) or modules defined
inline in a `tests/*.rs` file. That sweep
(`build.rs` → `tests/corpus_generated.rs`, see
`design_docs/CORPUS_DIFFERENTIAL_SWEEP.md`) generates one case per module — seeded
random stimulus, simulator vs the SystemVerilog it transpiles to under Verilator —
so **a new module is covered the moment it exists**, with no harness to write. A
module it cannot run gets an `#[ignore]` with a reason, never an omission. **Each
case drives several stimulus sequences, not one** (since 2026-10-28, §3.2 of the
design doc): four independent seeds of 200 cycles, plus a 32-cycle sequence from
*every* initial input vector for a module with at most 4 input bits (a first-cycle
pattern list for a wider one; the whole truth table for a combinational module up
to 8 bits) — all in one testbench run of one Verilated model, restarted from its
initial state at each boundary, so the cost stays one Verilator build per module. A
failure names the sequence (`FAIL: … (sequence 5 "first sel=1", cycle 0)`) and
`COPPER_SWEEP_SEQUENCE=<index>` re-runs that one alone. Before this, a module's
verdict rested on one seed: `branch_merge` passed for two months while its `tail_o`
led the SV by a cycle whenever the first cycle took `sel == 1`. The policy is pinned
by `tests/corpus_generated.rs::the_stimulus_policy_is_what_the_design_doc_says`,
which G-D requires. Four
tables in `build.rs` carry what cannot be inferred — `PARAMS` (widths for a generic
module), `RESET` (a design whose state is X until reset), `SKIP` (why a module is not
swept), `REFERENCE` (an independent Verilog file to check the module against as
well) — so adding one is the way to teach the sweep about a new module, and each
entry is a reviewed sentence rather than a silent exclusion. It prints the
`#[ignore]`d tests on every run so a deliberately-skipped check stays visible.
**A `SKIP` reason that names emitted text cannot detect its own staleness** — the
skipped case's body is a `panic!`, so `--ignored` proves nothing; re-transpile the
module before trusting one (two rows outlived their fixes that way).

Subsets — all of these print `PARTIAL OK` and skip some guards, so they are **not**
a regression run:

```bash
tools/regression.sh --quick          # fast loop: build + CLI + a few examples
tools/regression.sh --no-examples    # build + CLI + tests, no Verilator
tools/regression.sh --no-test        # skip cargo test
tools/regression.sh --example lfsr   # one named example
```

**For a number you will quote, run it remotely.** On macOS every freshly built
executable is assessed by Gatekeeper at launch (an online check), a full run
launches a few hundred of them, and a laptop sleeps — measured 2026-10-21, the same
tree took 304 s or 2598 s for the test phase depending on nothing in the tree.
`tools/regression-remote.sh` runs the bare regression on the `recycle` machine (96
cores, Linux, the paper-numbers toolchain) against an exact pushed commit, with
`target/` on local scratch disk, refuses to start on a loaded box, and copies the
log back under `target/remote-regression/`:

```bash
tools/regression-remote.sh --setup          # once per clone: scratch target/, tool check
tools/regression-remote.sh                  # HEAD, which must already be on origin
tools/regression-remote.sh --sync           # this working tree instead (not for numbers)
```

`--sync` deliberately does **not** preserve file times (`rsync --checksum --no-times`):
cargo calls a source fresh when its mtime predates the last build's fingerprint, so a
file edited before the previous remote run and synced after it would never be
recompiled (measured 2026-10-28: a stale proc-macro `.so` failed a ui case that passed
locally). Two sessions must not `--sync` into the same remote clone — give each its own
`--dir`/`--scratch` (clone `~/Copper-HDL` beside itself and run `--setup` once).

Note an example's `main()` is a real self-check — several assert against
independent BaseJump Verilog and `exit(1)` on mismatch — and `cargo test` only
*builds* examples, never runs them. That is why examples are in the default path.

Underlying cargo commands:

```bash
cargo build --workspace
cargo test --workspace
cargo test -p copper-sim executor          # a single test by name filter
cargo run --example mux                     # run one example end-to-end (needs Verilator)
```

The `copper-transpile` CLI emits SystemVerilog for one module (a generic module
comes out parametric — see the gotcha below):

```bash
cargo run -q -p copper-codegen --bin copper-transpile -- examples/combinational/one_bit_comparator.rs
```

Flags: `-o <out.sv>`, `--module <name>` (required when a file has >1 module),
`--profile verilator|generic|yosys`, `--hierarchy` (also emit every submodule the
target transitively instantiates), `--list`. Example names are the `[[example]]`
entries in `Cargo.toml`.

### Environment gotchas (important)

- **Verilator is required** for examples and equivalence tests (`brew install
  verilator`). A stale `VERILATOR_ROOT` in the environment makes `verilator` refuse
  to run; every Verilator invocation in the harness clears the variable itself
  (`verilator_status` / `verilator_command` in `copper-sim/src/verification.rs`, and
  `tools/regression.sh` unsets it too), so `cargo test`, `cargo run --example …`, and
  the driver all work with it set. When invoking `verilator` **directly** in a shell
  that exports one, prefix `env -u VERILATOR_ROOT`.
- **ccache is optional but worth having** (`brew install ccache`): a full run builds a
  few hundred Verilator models and each recompiles the Verilator runtime. When
  `ccache` is on the PATH, `tools/regression.sh` and `verify_with_verilator` hand it to
  Verilator's makefiles (`OBJCACHE`), and the equivalence harness compiles its C++ at
  `-O0` (a few hundred cycles per model; compile time dominates). Neither changes what
  is checked. Set `OBJCACHE=` to opt out.
- **Icarus Verilog is optional** (`brew install icarus-verilog`): only
  `tests/sim_throughput.rs` (the simulator-throughput benchmark, three-way sim vs
  Verilator vs Icarus) and `tools/stats/simperf.py` use it. Absent, the test skips
  its Icarus leg via `iverilog_available` — the same absent / present-but-broken
  split as Verilator, so a broken install still fails.
- **A Verilator failure is never silently skipped.** Only a genuinely *absent*
  binary is skippable, signalled by the `VERILATOR_NOT_INSTALLED` marker
  (`copper-sim/src/verification.rs`). Installed-but-broken fails loudly, as does any
  build error. If you add a test that drives Verilator directly, go through
  `tests/common::verilator_available()` and `verilator_command()` rather than
  spawning `verilator` yourself — a hand-rolled `--version` probe that treats a
  non-zero exit as "not installed" reintroduces the silent skip.
- **A generic module transpiles to a PARAMETRIC SystemVerilog module.**
  `copper-transpile examples/combinational/rotate_right.rs` emits
  `module rotate_right #(parameter int N = 1, parameter int N_LOG = 1)`, so it needs
  concrete widths only when you *Verilate* it — `HardwareTest::with_params(&[("N", 8),
  …])`, which is how the equivalence tests and the corpus sweep run them (the sweep's
  widths live in `build.rs`'s `PARAMS`). What the CLI cannot do is pick those widths
  for you: parameters are often constrained (`N_LOG == clog2(N)`, asserted inside the
  module), so a guess is a compile error rather than a wrong number.
  Multi-module files need `--module`.

## Architecture

### The five crates

- **`copper-core`** — the type system and every IR in the pipeline. Hardware
  types (`Logic`, `Bits<N>`, `Clock<D>`, ports `In`/`Out`/`RegOut`, `Memory`),
  the phantom clock-domain machinery (`cdc.rs`), and the IR structs
  (`frontend_ir`, `chir`, `shir`, `vlir`). No proc-macro or codegen logic. A leaf
  crate (no workspace dependencies) — which is what lets both the proc-macro and
  the transpiler share analysis over it without a dependency cycle.
- **`copper-analysis`** — the shared compile-time control/liveness analysis (the
  c2 architecture; depends only on `copper-core` + `syn`). Keys off `syn::ItemFn`
  — the representation *both* front-ends already hold — so the sim macro and the
  transpiler consume **one** authoritative analysis rather than two that must
  agree. See `design_docs/SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md` (item 2). Provides
  `Cfg` (a real control-flow graph over `syn::ItemFn`, `E_comb`/`E_tick` edges),
  `infer_registers` (backward-liveness register inference — a local is a register
  iff it is defined-in-loop ∧ live-across-a-tick), `check_reachability` (every loop
  path must reach a `clk.tick().await`, enforced as a hard spanned compile error in
  **both** front-ends), the G2 structural reg-match helpers, and `consts` — the shared constant evaluator of SPEC
  I.14.4 (Phase C step 5: `ConstEnv` over const items, associated consts and `const fn`s;
  the transpiler's counted-`for` bounds, `from_lit` turbofish and register initializers use
  it; R11 and the interpreter do not — a decision owed, MEASUREMENTS §J.13). It is also where
  the checks that reason about **clock phases** live — `check_memory_staging` (and,
  until D-8 retired them, the plain-`Out` phase rules) — because a check
  downstream of `control_extract` cannot see the phases it is counting (that pass
  rewrites branch- or loop-nested ticks into a single-tick `match pc` FSM). Any new
  phase-sensitive rule belongs here, on the source, not in codegen. That policy is
  enforced, not just documented: `copper-codegen/tests/phase_sensitive_checks.rs`
  pins every transpiler function that reasons about phases and can fail, so a new one
  has to be justified as a lowering limitation or moved. Consumed by both
  `copper-macros` and `copper-codegen`.
- **`copper-macros`** — the
  `#[hardware(sequential|combinational|synchronizer|structural)]` proc macro.
  Validates the signature (exactly one `Clock<D>` parameter for a behavioral
  module), enforces CDC rules at compile time, refuses a plain `Out` on a clocked
  module (R8),
  wraps combinational bodies in the `loop { …; delta_yield().await; }` shape, and
  makes the module a `HardwareModule` (so a bare `async fn` won't simulate). A
  `structural` parent is transpile-only: it is not spawned in the simulator, which
  wires the hierarchy by hand.
- **`copper-codegen`** — the transpiler and the `copper-transpile` CLI
  (`bin/`, `main.rs`). Owns the lowering pipeline and Verilog emission.
- **`copper-sim`** — the async simulation executor and the Verilator
  equivalence verifier (`verification.rs`, `verify_with_verilator`). The production
  scheduler is `SchedulerMode::Levelized` (SCC-condensed topological single pass;
  `COPPER_SCHEDULER=fixpoint` selects the iterate-to-fixpoint delta-cycle loop,
  kept permanently as the differential oracle). `PollOrder` (`Insertion` /
  `Reversed` / `Seeded`) and its poll-order-independence fuzzer exercise the
  fixpoint oracle only — under levelized the order is canonical.

The **root `copper` crate** (`src/`) is the hardware standard library —
reusable `#[hardware]` modules like `sync_2ff` that must live *downstream* of
`copper-sim` because the macro expands to `::copper_sim::…` paths.

### Transpilation pipeline (codegen)

`transpile_fir` in `copper-codegen/src/lib.rs` is the spine:

```
Rust #[hardware] fn
  → FIR   (frontend_ir; source-shaped, pre-normalization; capture_frontend_ir)
  → control_extract  (FIR→FIR: flatten branch-nested ticks into an explicit `match pc` FSM)
  → CHIR  (chir_lower)
  → SHIR  (shir_lower)
  → VLIR  (vlir_lower; 1:1 with legal SystemVerilog — legalized names, concrete widths)
  → SystemVerilog text  (emit.rs)
```

A clocked body lowers one phase per tick (`shir_lower::lower_seq_body`; a `phase_r`
counter when there are several). Its outputs have **one emitted form**
(`vlir_lower::split_output_regs`, SPEC I.6.9): every drive is `out <= v` in
`always_ff` under its guards and the phase guard, `v` the forwarded value (registers
the resumption already assigned stand for what it assigned them), holding when
unwritten; a plain `Out` reaching `lower_seq` is an error, not a form. The **trailing
region** (statements after the last tick) is the head of the next resumption (SPEC
I.0.2): it lowers into phase 0 ahead of segment 0, one forwarding walk across both,
under `if (__copper_started)`, a synthesized one-bit register that is 0 until the
first edge. The **pre-loop expression statements** (a `RegOut` write, a memory staging,
an assignment — SPEC I.9.3) are the first resumption's: both routes lower them into
segment 0 ahead of the head under `if (!__copper_started)` (Phase C step 5; until then the
extraction route re-ran them at every return to state 0 and the linear route dropped
them). Every statement of a segment reads a register it assigned earlier as the
assigned value — the `always_comb` ones too (step 5, II.3 (f5)). A memory `data()` /
`is_ready()` always reads the registered capture, the word the last edge delivered.
Until Phase C step 4 (2026-11-06) a clocked plain `Out` had five more forms (a
continuous `assign`, the every-path mux, the Moore default hoist, the mux +
`<port>_hold` form, the phase-scoped flop) chosen by an opening/closing drive tag,
and the trailing region was FOLDED into the last phase's edge; all of that is gone
(`design_docs/review/MEASUREMENTS.md` §H). A combinational module still emits a
top-level drive as `assign` and a branch-nested one in `always_comb`.

### Execution / timing model (sim)

This is the semantic core; read `design_docs/SYNCHRONOUS_SEMANTICS.md`. The
rules the whole project is built around:

- Every `clk.tick().await` is a **clock-cycle boundary**; each suspension point
  becomes an FSM state; every value live across an await becomes a **register**.
- **One resumption per tick** (SPEC §I.0.2, D-8; the only executor since Phase C
  step 3, 2026-11-06): `HardwareExecutor::tick_clock::<D>` settles the network on the
  driven inputs, resumes each coroutine of `D` once to its next `tick` (every read
  sees `x_{N+1}`), commits the edge, and settles the network for the observation;
  the post-edge settle resumes no coroutine, and only `D`'s ticks resume `D`'s
  coroutines. The two-job executor, its `PreEdgeBarrier` and the macro's per-read
  barrier injection are gone.
- **Simulation must be independent of Rust async poll order** — a well-formed
  design simulates identically under any `poll_tasks` order. Do not introduce
  timing that depends on task ordering.
- **Every output of a clocked module is a `RegOut`** (SPEC II.1 R8, D-8; refused in
  both front-ends by `copper_analysis::check_no_plain_out` / `check_clocked_outputs`).
  A `RegOut` written in the resumption that ends at edge N+1 shows from `obs_{N+1}`
  and holds when unwritten (the enabled register). A value that must follow an input
  within the cycle (a passthrough, a Mealy output) is a `#[hardware(combinational)]`
  module wired to the clocked module's `RegOut`s; plain `Out` is combinational-only.
  A `Memory` read reaches a `RegOut` one edge after delivery (SPEC I.10.1 (d)); a
  latency-1 synchronous read whose register IS the port (`dob <= ram[a]`) is an array
  register written in program order (`dual_port_ram`, `ram_read_first`).

### Clock-domain crossing (CDC)

Signals and clocks carry a **phantom domain type** (`Clock<D>`, `In<T,D>`,
`Out<T,D>`, `D: ClockDomain`). Wiring one domain into another port is a nominal
type mismatch rejected at `cargo build`. A crossing is only legal inside a
`#[hardware(synchronizer)]` module (e.g. `sync_2ff`), which is exempt from the
per-domain check. `copper-core/src/cdc.rs` has no runtime code — its
`compile_fail` doctests **are** the executable specification, so `cargo test`
fails if a guarantee regresses. A module may not *construct* a clock, only
receive/clone one.

## Conventions

- **Verify empirically, and check "which behavior is correct" against an
  independent reference, not by argument.** `examples/basejump/` checks Copper
  modules against independent BaseJump STL Verilog; that Verilog is under the
  Solderpad license and needs attribution.
- Design docs live in `design_docs/`, by role:
  **normative semantics** — `SYNCHRONOUS_SEMANTICS.md` (start here; where another
  doc disagrees with it, it wins);
  **the sweep and anchoring** — `CORPUS_DIFFERENTIAL_SWEEP.md`,
  `ANCHORING_A_MODULE.md`;
  **the pre-tick alignment family** — `PRETICK_ALIGNMENT_GUARDRAIL.md`;
  **the scheduler** — `LEVELIZED_SCHEDULING_SCOPE.md`;
  **emitted ABIs** — `ARRAY_PORT_ABI.md`, `RECEIVED_MEMORY_ABI.md`;
  **the cycle-dataflow model and its derivation** — `CYCLE_DATAFLOW_SEMANTICS.md`,
  `DERIVATION_TABLE.md`, `PAIRED_IMPLEMENTATION_SCOPE.md`,
  `TIMING_MODEL_UNIFICATION.md`, `TIMING_COVERAGE_MATRIX.md`;
  **historical plan** — `SYNCHRONOUS_SEMANTICS_IMPL_PLAN.md` (its status notes are
  dated; do not take "not started" there at face value).
  `design_docs/OUTDATED/` is historical, not authoritative.
- **The pre-tick alignment rules are retired by D-8** (Phase C step 3, 2026-11-06).
  The family (`unprotected_pretick_out_write`, `pretick_out_write_before_update`,
  `trailing_regout_reads_input`, `trailing_out_reads_input`,
  `linear_out_write_anchor_mismatch`, `multi_phase_out_write`,
  `out_first_driven_late`), `multi_write_collapse` and
  `memory_result_drives_plain_out` all reasoned about a plain `Out` on a clocked body,
  which R8 now refuses; they were deleted with their exact-set pins, ui/fail cases,
  `cfg.rs` unit tests, the `allow_pretick_alignment` opt-out (now itself refused) and
  the plain-`Out` divergence battery. SPEC §II.3.2 is the authority on the sort;
  `PRETICK_ALIGNMENT_GUARDRAIL.md` and `CYCLE_DATAFLOW_SEMANTICS.md` §9–§10 are the
  historical record of what they measured.
- **Verilator work dirs are unique PER INVOCATION** (`obj_dir_<module><params>_<pid>_<n>`),
  not per module. Two tests in one binary can Verilate the *same* top module in
  parallel — `tests/det_010_independent_golden.rs` checks two codings against the same
  golden `det_010` — and a shared directory made them clobber each other's build.
  That is a false-PASS mechanism as well as a false-failure one: a test can end up
  run against another test's model. Never key that directory on anything less unique.
  The same rule covers the **source** directory a test writes its transpiled `.sv`
  into before Verilating it: `tests/sequential_forwarding_divergence.rs` keyed that
  on `(top, pid)` and two tests transpiling `trailing_update` raced on a 96-core host
  (2026-09-03), one deleting the file the other's Verilator was about to read. Every
  test temp dir now carries a `TMP_NONCE`.
- **Both front-ends enforce the same rules (front-end parity, 2026-10-23).** The
  macro's sequential arm and `transpile_target` run the same list from
  `copper-analysis`: the two source-shape rules (`check_nested_loop_shape` — a nested
  loop must end at a clock boundary; `check_awaits` — only `clk.tick().await` may be
  awaited), reachability, the register power-on rule (`check_register_initializers`,
  2026-10-29 — a pre-loop register's initializer must be a constant, which reaches the
  SV as its `initial` value, or `Bits::x()`; no opt-out; `COPPER_VERILATOR_X_INITIAL=1`
  randomizes Verilator's power-on values to measure what is left unstated), R8 (no
  plain `Out` on a clocked module, one shared message) and the memory rules. A module
  that compiles therefore
  transpiles, or is refused by both. The standing audit is
  `copper-analysis/tests/front_end_parity.rs`; a shape both refuse is a
  `copper-macros/tests/ui/fail` case, never a swept module (the former sim-only
  fixtures `tick_first_waiter` and `mp_rom` live there now). When adding a rule to one
  front-end, add it to the other and to the audit in the same change.
- **When adding a `#[hardware]` mode or flag, fix every attribute parser.**
  `parse_args::<syn::Ident>()` fails outright once a flag is present, which silently
  drops those modules from corpus scans (`copper-analysis/tests/pretick_alignment_corpus.rs`
  and `copper-codegen/tests/register_reconciliation.rs` both had this, as did a
  since-deleted scan). Read the first token of the attribute list instead.
- Generated Verilator artifacts (`tb_*.cpp`, `obj_dir_*`) land under
  `target/verilator/` (per-invocation names, reclaimed by `cargo clean`);
  `waveforms/*.vcd` is gitignored.
- **Built Verilator models are cached, content-keyed** (2026-10-20):
  `target/verilator/cache/<sha256>/` holds the `V<module>` executable, where the key
  is a SHA-256 over the full emitted SV (plus any `` `include ``d file), the full
  generated testbench (stimulus and expected outputs are compiled in), the module
  name, the sorted `-G` parameters, the tracing flag, every Verilator argument, and
  `verilator --version`. A repeat regression with unchanged SV runs Verilator zero
  times; a changed module cannot hit, and a hit also compares the stored key material
  byte-for-byte. A miss builds in the per-invocation directory exactly as before and
  then publishes by atomic `rename` of a private staging dir — an entry is never
  built in place, never overwritten, and never written into at run time (the VCD
  path is `argv[1]`, not baked into the testbench). The log says
  `Verilator model: cache hit <key>` / `cache miss <key>`. Off with
  `COPPER_VERILATOR_CACHE=0`; `cargo clean` reclaims it. Hand-rolled callers that
  spawn Verilator through `tests/common::verilator_command` are outside it.
  Pinned by `tests/verilator_model_cache.rs` (hit / one-character miss with a
  different verdict / four parallel invocations of one model / traced runs / off).
- Do research on prior HDLs and hardware DSLs to get knowledge on what correct semantics
  should be and to get advice on design decisions.

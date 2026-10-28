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

It also enforces five wiring guards, because "the check silently didn't run" has
been a recurring bug class here: **G-A** every `examples/**.rs` is registered as a
`[[example]]`, **G-B** every registered example actually ran, **G-C** every
`tests/*.rs` (root and per-crate) produced a test binary that ran, **G-D** the
corpus differential sweep covered every `#[hardware]` module in `examples/` and
`tests/fixtures/` and ran with Verilator present, and **G-E** (printed, not
enforced) how many swept modules are additionally anchored to an independent
hand-written Verilog reference. The sweep does not reach `src/` (the standard
library's `sync_2ff` is covered by its `tests/fixtures/` twin) or modules defined
inline in a `tests/*.rs` file. That sweep
(`build.rs` → `tests/corpus_generated.rs`, see
`design_docs/CORPUS_DIFFERENTIAL_SWEEP.md`) generates one case per module — seeded
random stimulus, simulator vs the SystemVerilog it transpiles to under Verilator —
so **a new module is covered the moment it exists**, with no harness to write. A
module it cannot run gets an `#[ignore]` with a reason, never an omission. Four
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
  **both** front-ends), and the G2 structural reg-match helpers. It is also where
  the checks that reason about **clock phases** live — `multi_phase_out_write`,
  `check_memory_staging`, `memory_result_drives_plain_out` — because a check
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
  module), enforces CDC rules at compile time, injects per-read freshness guards,
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

### Execution / timing model (sim)

This is the semantic core; read `design_docs/SYNCHRONOUS_SEMANTICS.md`. The
rules the whole project is built around:

- Every `clk.tick().await` is a **clock-cycle boundary**; each suspension point
  becomes an FSM state; every value live across an await becomes a **register**.
- A tick has phases: **pre-edge settle → clock edge → post-edge settle →
  post-edge observation**. `HardwareExecutor::tick_clock` drives these; the
  current phase gates phase-aware futures.
- **Simulation must be independent of Rust async poll order** — a well-formed
  design simulates identically under any `poll_tasks` order. Do not introduce
  timing that depends on task ordering.
- `Out` vs `RegOut`: `RegOut<T,D>` is a registered output driven from
  `always_ff` (use for write-before-tick Moore outputs); plain `Out` otherwise.

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
- **`PRETICK_ALIGNMENT_GUARDRAIL.md`** — the pre-tick alignment family: four rules
  (`Cfg::unprotected_pretick_out_write`, `multi_phase_out_write`,
  `pretick_out_write_before_update`; the first's constant-write clause was retired
  2026-10-28), joined 2026-10-23 by two more of the same discipline —
  `trailing_regout_reads_input` (a `RegOut` written after the last tick from an input
  read directly; a `RegOut` write takes effect at the first clock edge after it
  executes, so the flip-flop would sample the input one drive late) and
  `out_first_driven_late` (a constant-only plain `Out` first written after the second
  edge: a startup transient the `assign` cannot show), and 2026-10-26 by
  `trailing_out_reads_input` (the plain-`Out` sibling of the `RegOut` rule: a plain
  `Out` written after the last tick from an input read directly, in a body that
  crosses more than one edge — neither trailing drive form samples the input when the
  simulator's immediate read did; the single-tick body agrees and is exempt) — every
  one with an exact-set
  corpus pin in `copper-analysis/tests/pretick_alignment_corpus.rs` and enforced in
  both front-ends. A former fifth, `unprotected_trailing_out_write`, was retired
  2026-10-23 when phase C landed (see below). **D1** is **guarded** (rule
  2026-08-21, narrowed 2026-08-26, **re-derived 2026-10-28** —
  `CYCLE_DATAFLOW_SEMANTICS.md` §10, guardrail §5.8): before the first tick, a plain
  `Out` is a compile error when its drives have **mixed anchors** — an `In` read
  comb-reaches some write of the port while some write has a read-free path from the
  loop head (the simulator runs that write at the opening, the SV edge-samples the
  port or muxes it on committed state one observation late), unobservable only for a
  single every-path drive that forwards no register it reads (`d3a`) — or when a
  write no read reaches sits under an `if`/`match` that tests a register assigned
  earlier in the segment (a **forwarded guard**: the comb copy tests the committed
  value; `d4a`). Witnesses `w4_mixed_alignment`, `branch_merge_explicit`, and the
  seven refused `d*` probes; `pc_arm_*` and `probe_fsm` (a read in a sibling arm
  reaches nothing) are admitted and compile clean, as is a constant behind its own
  read. The former constant-write clause and its "unprotected register assignment"
  conjunct are gone. A register-reading write that no `In` read reaches is an
  opening-prefix drive and is **legal** — forwarded continuous-assign emission gives
  it the meaning the simulator always had. Fix a real hit with `RegOut`, make the
  anchor uniform (read the input before every write of the port or before none),
  test only committed state, or move the writes after the tick. A module that
  exists to *demonstrate* the hazard
  opts out with `#[hardware(sequential, allow_pretick_alignment)]` — this silences the
  error, not the detection, and must never be reached for in a real design.
  **D2 is FIXED** (2026-08-21) — a read feeding a combinational `Out` in a
  register-free segment is now `Immediate`, so a passthrough tracks its producer
  instead of lagging it.
  The other rules: **`multi_phase_out_write`** (a plain `Out` driven in more than one
  clock phase, **narrowed 2026-10-23 by cycle-dataflow phase E** to the measured
  divergent class — the linear route, a register-free body with reads, mixed anchors
  (an input read reaching some write of the port while another write is opening), or
  a wait loop's read before an opening write (unmeasured, refused by construction);
  an extracted body none of whose writes of the port an input read reaches (whatever
  it reads elsewhere) and one whose every write follows a deferred read are accepted
  and measured agreeing — the "input read anywhere" clause was retired 2026-10-26 —
  `CYCLE_DATAFLOW_SEMANTICS.md` §9, the `e*` battery in
  `tests/sequential_forwarding_divergence.rs`; enforced in both front-ends);
  **`pretick_out_write_before_update`** (a
  plain `Out` written from a register before that register's update in the same
  segment publishes the previous generation; 2026-08-26/27). **Phase C landed
  2026-10-23** (`vlir_lower::opening_hold_outputs`): a conditionally-driven plain
  `Out` whose drives are all *opening* drives (no `In` read precedes them in the
  arm) is emitted as a forwarded combinational mux with a `<port>_hold` register
  instead of an edge-sampled register, which is what made the control-extraction
  route's trailing drives one cycle late; `trailing_update`, `branch_trailing` and
  `pulse_plain` now agree, and `unprotected_trailing_out_write` is gone. **Reach of
  phase C:** since 2026-10-26 the opening predicate is **arm-granular** (an
  `if`/`match` seeds each arm's walk from its condition's reads alone), so the mux
  form reaches any extracted module whose drives have no read before them in their
  own arm — `e9` agrees, and so do the former D1 witnesses `pc_arm_*` and `probe_fsm`
  (sv-baseline delta: seven modules, no referenced one; D1 was re-derived on that
  evidence 2026-10-28, see above). Until then the predicate
  was statement-granular over the whole `match pc` and the form was emitted only for
  input-free modules. `e8`, the battery's other single-phase divergence, is refused
  by `trailing_out_reads_input` since the same day (`CYCLE_DATAFLOW_SEMANTICS.md`
  §9.4), so both gaps phase E found are closed. The doc also records **five** rejected fixes with
  measured evidence so they are not re-tried, and the prior-art survey in §10.
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
  awaited), reachability, the pre-tick alignment family (opt-out honoured in both),
  the memory rules and `multi_write_collapse` (no opt-out in either). A module that
  compiles therefore transpiles, or is refused by both. The standing audit is
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

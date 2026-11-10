# copper-ref — the reference interpreter

**What it is.** The third leg of the corpus sweep (`design_docs/SPEC_OVERVIEW.md` §7).
It takes a `#[hardware(sequential)]` or `synchronizer` module as the `syn::ItemFn`
the two front-ends already hold and runs it under the **one-resumption protocol** of
`SPEC.md` §I.0 as decided by D-8 — with no proc macro, no rustc lowering and no
executor, so it shares nothing with the simulator but the source text and
`copper-core`'s value semantics:

| protocol (SPEC I.0.2)            | API                                             |
|----------------------------------|-------------------------------------------------|
| the resumption, `t_{N+1}⁻`       | `Module::resume(inputs)` — entry on the first call (`s_pre`, then the head to the first `tick`, I.9.1), afterwards from the parked `tick` site to the next one it reaches; every `In::read()` yields the inputs given, `x_{N+1}` (I.4.1); every `RegOut::write` sets a pending value, last write wins (I.6.1–I.6.2); the locals are the registers and forward within the resumption (I.7.3) |
| the edge N+1                     | `Module::edge()` — pending values take effect; an unwritten `RegOut` holds (I.6.3) |
| the observation `obs_{N+1}`      | `Module::observe()` — every `RegOut`'s committed value; before the first edge the zero of its type, `init(q)` (I.6.6, I.9.4) |

A suspension site is a `TickSite`: the statement/arm/loop index path of the `tick`
(the residue of I.3.3). `Module::tick_sites()` is the static set — the states of
H(M), I.0.4 — and `reached_sites()` what some resumption parked at. `with_poison`
runs the register proposition's poison run (I.0.5(a)): every named local overwritten
at every suspension, a read before reassignment reported as `RefError::Poisoned`.

`RefBits` mirrors `Bits<N>` bit for bit with a run-time width; `tests/bits_property.rs`
pins every operator against `copper_core::Bits<N>` at widths {1, 4, 8, 16, 32, 64, 128}
on X-sprinkled random operands — the first executable check of I.14.

**Memories (SPEC §I.10, Phase C step 9).** A `let m = Memory::<T, R, W, D, L, M>::{new,
from_contents, from_fn}(clk, …)[.write_first() | .read_first()]` in `s_pre` is a
variable of H(M) beside the environment (`src/memory.rs`, every behaviour cited to its
clause, derived from the text and not from either implementation): `read_port::<k>().read(a)`
and `write_port::<j>().write(a, e)` are stagings of the resumption, last wins
(I.10.1(b)); `edge()` runs the read step and the write step in the instance's mode order
(I.10.1(c), I.10.2); `is_ready()` / `data()` read the output stage the last edge left
(I.10.1(d)), `data()` unready a `Runtime` failure; an out-of-range address likewise.
Pinned by hand-derived traces in `tests/interp_memory.rs`.

**Combinational modules (SPEC §I.11).** `Module::evaluate(inputs)` runs a
`combinational` body once from a fresh environment (every local a wire of that
evaluation): last write wins, a dead write admitted, a never-written port refused at
construction and an unwritten path at the evaluation (I.11.3), no `loop`/`while`/
`break`/`continue` (I.2.7). The reference leg evaluates once per vector — the settle.
Pinned in `tests/interp_comb.rs`.

**Designs (SPEC §I.13).** `Design::new(src, top)` builds the design a `structural`
module names: its nets (`net`/`wire`/`registered_wire`, whose `init` is no value of the
net) and its instantiations, each child a `Module` of the same file; one driver per wire,
every net driven, payload types checked, a combinational cycle refused (I.11.4).
`settle()` evaluates every function once in a topological order from the sources;
`pre_edge(clk)` / `post_edge(clk)` are I.13.2(b)'s cycle for the domain of parent clock
`clk` (settle, resume that domain's coroutines, edge, settle) — so a two-clock parent
runs by cycling each clock. Run against the hand-wired simulation of `two_domain_top`
in `tests/two_domain_hierarchy_cdc.rs`; pinned in `tests/interp_design.rs`.

**How it is wired.** `build.rs` emits a `<name>_reference_leg` case per corpus module
(the simulator's trace under the sweep's stimulus policy, replayed through the
interpreter: `resume` with cycle N's inputs, `edge`, `observe` against `obs_{N+1}`, and
`obs_0` before the first edge, compared on all three legs since 2026-11-06) and adds the leg to the
`_differential` case so one failure report carries all three readings. `tools/regression.sh`'s
**G-F** counts the cases that ran and the ones skipped with a reason; the leg needs no
Verilator, so it also runs under `--quick`.

**What it refuses, and how.** Every refusal names its SPEC row (`RefError`):

- `Deferred` — a class the interpreter does not cover yet, skipped by `build.rs`
  (`ref_skip_reason` for a whole class, a `REF_SKIP` row for one module) with the clause that owes it: a plain `Out` on a clocked module (R8 refuses
  it in both front-ends since Phase C step 3), a `Memory` **parameter** (I.10.5: its
  depth and contents are the owner's), a second clock in one kernel (I.12), and a
  `structural` module given to `Module::new` (it is a `Design`, I.13.4(a)).
- `Unsupported` — outside the kernel or in a dated row: a read inside macro tokens or a
  statement macro (II.1 R13/R14), a labelled or valued `break` (R12),
  `return` (I.2.5(d)), a non-`tick` `await` (R1), a `tick` before the loop (R21), recursion
  (R6), an or-pattern that binds, a tuple-variant enum, `let … else`, `if let`, closures.
- `Runtime` — what the Rust program would panic on: an X read as an integer, an index
  out of bounds, integer overflow, a literal that does not fit its width.

It has **no opinion about lowering**: a shape in II.3's defect rows (the run-time-empty
`for`, the pre-loop re-run and drop, the trailing fold at edge 1, array-element
forwarding) is interpreted by its Part I meaning — which is the point.

**Reading a disagreement.** The interpreter is the spec made executable, so:

| result                       | reading                                                                 |
|------------------------------|-------------------------------------------------------------------------|
| sim ≠ ref, SV = ref          | a **simulator or macro bug** (or a Phase C obligation not yet landed)   |
| sim = ref ≠ SV               | a **transpiler bug**, or a II.3 row                                     |
| sim = SV ≠ ref               | both implementations share a reading the spec does not give — or the interpreter is wrong (the one case found, `regout_trailing_single_tick`, a residue of the retired two-job executor, became sim = ref ≠ SV at Phase C step 3 and agrees on all three legs since step 4) |
| all three differ             | a **spec gap or an interpreter bug** — read the interpreter's first disagreement first |

Agreement of the three is consistency, not correctness: only an independent anchor
(BaseJump, a hand-written golden) says what the hardware does.

**The constant evaluator.** Widths, array lengths and `const` items (file scope, `impl`
blocks as `T::K`, fn bodies) are evaluated by the interpreter's own evaluator
(`ConstScope`, `Exec::eval_const`) against the file's consts and the module's const
generics. It shares SPEC I.14.4's grammar with the front-ends'
`copper_analysis::consts::ConstEnv` but no code (D-21 B2), which is what lets the
reference leg attribute a disagreement. `tests/const_evaluators_agree.rs` (G-J) pins the
two against each other on every corpus constant, pre-loop initializer, counted-`for` bound
and index, and against rustc on the widened grammar. An expression only one evaluator
folds is a pinned `ASYMMETRIC` row.

**The corpus.** The interpreter implements the one-resumption model only, as the
simulator has since Phase C step 3 (2026-11-06). Every clocked module's outputs are
`RegOut`s (R8), so `REF_SKIP` is empty. The leg's only skips are the signature classes
`build.rs`'s `ref_skip_reason` computes: a `structural` module, a `Memory` parameter, more
than one clock. The five
B-rewrites of `review/Q1_TRADEOFFS.md` §4 live in `tests/fixtures/q1_b_rewrites_dut.rs`
and run through every leg.

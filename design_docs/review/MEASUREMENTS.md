# Phase A measurements — 2026-11-03

The three things the inventories could not settle by reading, measured before the
first clause packet. Both sweep runs used temporary tree edits (a `build.rs` row
removed; a fixture file added) that were restored afterwards; nothing in the tree
changed.

## A. `tests/fixtures/probe_timing_dut.rs::probe` — the SKIP row is stale

The `build.rs` `SKIP` row says `probe` is "an `allow_pretick_alignment` witness … sim ≠
SV is its purpose". The source carries no opt-out and no rule flags it. With the row
removed, the generated case `fx_probe_timing_dut::probe_differential` ran against
Verilator on every stimulus sequence:

```
=== probe ===
  trace:            PASS
  verilator:        PASS
test fx_probe_timing_dut::probe_differential ... ok
```

**Verdict: agrees.** `probe` is `let x = inp.read(); tick; out.write(x); tick;` — the
deferred read commits `x` at edge A, the unconditional register-valued drive is a
continuous `assign out = x`, and both sides show `x` from observation A on. It is an
admitted, agreeing module. **Obligation (Phase C housekeeping):** delete the row so the
sweep covers it; the comment in the fixture header that calls its timing "under
dispute" is historical.

## B. Pre-loop expression statements under control extraction — a real divergence

`INVENTORY_CODE.md` item (12).14 / unstated-behavior 29: `control_extract` moves
pre-loop expression statements into state 0's body
(`copper-codegen/src/control_extract.rs:526-549`), and the module loop re-enters state 0
on every iteration. The probe (scratchpad `probes/preloop_probe.rs`, run as a temporary
`tests/fixtures/preloop_probe_dut.rs`):

```rust
#[hardware(sequential)]
async fn preloop_probe(clk: Clock<C>, go: In<Logic, C>, q: RegOut<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    q.write(Bits::from_u8(7));            // pre-loop: executes once in the simulator
    loop {
        cnt = cnt + Bits::from_u8(1);
        if go.read() == Logic::One {
            clk.tick().await;             // nested tick → control extraction
            q.write(cnt);
        }
        clk.tick().await;
    }
}
```

Emitted SystemVerilog (excerpt): the pre-loop write sits under `pc == 0`, which is
entered every iteration:

```systemverilog
always_ff @(posedge clk) begin
    case (pc)
        8'd0: begin q <= 8'd7;  end
        8'd1: begin q <= cnt;   end
        default: begin end
    endcase
    ...
    case (pc)
        8'd0: begin pc <= ((go == 1'b1) ? 8'd1 : 8'd0); end
        8'd1: begin pc <= 8'd0; end
```

Sweep result (first lines; 559 cycles failed over the five sequences):

```
FAIL: Cycle 59 q expected 37 got 7 (sequence 0 "seed 0x2a0435ee1908b5ad", cycle 59)
FAIL: Cycle 60 q expected 37 got 7 ...
FAIL: Cycle 63 q expected 40 got 7 ...
```

**Verdict: diverges; no rule refuses it.** The simulator executes the pre-loop write
once (so `q` is 7 only until the first `q.write(cnt)`); the SV re-executes it at every
return to state 0. `RegOut` was chosen for the probe so that no alignment rule could
interfere — the bug is in cycle accounting, not in the anchor family. The linear route
is also suspect: the comment at `control_extract.rs:530-531` says the CHIR scan "does not
lower" pre-loop expression statements at all. **This is a Round 1 clause (I.9, startup:
what pre-loop statements mean) and a Phase C fix; until then it is an admitted,
unrefused divergence.**

## C. Anchor debt — clauses whose only evidence is sim/codegen agreement

"Anchored" = measured against Verilog that was not derived from the transpiler or from
the cycle-dataflow model: third-party (BaseJump, Solderpad), or hand-written from a
transition table or textbook. The goldens in the tree: `examples/basejump/sv/*`
(7, BaseJump), `examples/sequential/sv/{lfsr,shift_register,pattern_detector,pattern_detector_010}.sv`,
`examples/combinational/sv/*` (5), `examples/memory/sv/dual_port_ram.sv`,
`examples/cdc/sv/{sync_2ff_ref,two_domain_hierarchy}.sv` (the latter also holds
`ref_fast_counter`), the `REFERENCE` rows `ram_read_first`, `bit_not_bits`, `handshake`,
and `tests/fixtures/timing_probe_sv/mac_fsm.sv` (register-set match only).

### Anchored

| clause area | anchored by |
|---|---|
| A read feeding a commit samples at the closing edge; the FSM samples at its own edge | `det_010`, `det_010_awaits` vs `pattern_detector_010.sv`; `det_110101`; `sipo_block` (BaseJump); `lfsr` (BaseJump `bsg_lfsr`); `shift_register`; `bsg_counter_up_down` |
| A read after a tick whose result crosses the next tick (mid-phase read) | `sipo_block` |
| `RegOut` written before a tick takes effect at that edge (one cycle later) | `sipo_block`, `bsg_counter_up_down`, `bsg_dff_en` |
| A plain `Out` left unwritten holds (enabled register, the flop form) | `bsg_dff_en` |
| A plain `Out` written from committed state (Moore comb-out) | `det_110101`, `det_010`, `lfsr` |
| A data-dependent wait loop (test before tick) | `det_010_awaits` vs `pattern_detector_010.sv` |
| Write-then-update spelling of the sticky flag (registered form) | `ref_fast_counter` |
| Memory `ReadFirst`, latency 1, dual-port | `dual_port_ram.sv`; `ram_read_first` REFERENCE |
| Two-flop synchronizer latency; a dual-clock hierarchy at 1:1, 2:1, 3:1 | `sync_2ff_ref.sv`, `two_domain_hierarchy.sv` |
| Combinational modules | the five `combinational/sv` goldens; `bit_not_bits`, `handshake` |
| Register inference (set equality) | `mac_fsm.sv`, `det_010`, `det_110101`, `lfsr` |
| The D2 passthrough tracks a clocked producer (`mid == out`) | hand-written Verilog in `d2_is_fixed_and_d1_still_demonstrates_the_hazard` (project-written, textbook shape) |

### Debt (sim ≡ SV only, or the model's own hand-lowered prediction)

1. **The forwarded opening drive** (`r = r + 1; o.write(r)` means `assign o = r + 1`): the
   V1 class agrees sim ≡ SV since phase B; the only independent voice, `ref_fast_counter`,
   anchors the *other* spelling. The meaning of the forwarded shape in hardware is a
   model decision, not a measurement.
2. **The mux + hold form** for a conditionally driven plain `Out` with opening drives
   (`pulse_plain`, `trailing_update`, `branch_trailing`, `branch_merge`'s `tail_o`, `d2a`,
   `e4`, `e6`, the `d5*` probes): anchored only by the m2 hand-lowered module, which was
   written *from the model* (`TIMING_COVERAGE_MATRIX.md` addendum). Circular as an anchor.
3. **A trailing `RegOut` commits at the next edge**: `regout_trailing_single_tick` sweeps
   (sim ≡ SV only). The register case of the trailing region is anchored (`sync_2ff_ref`),
   the `RegOut` and plain-`Out` cases are not.
4. **A multi-phase plain `Out` as a state mux** (`e2`, `e3`, `e9`, `e5d`, `e2x`): m2-style
   only.
5. **Every linear multi-tick form** of §10.7–§10.9 (the 36 `e14*`/`e15*`/`e16*` probes):
   sim ≡ SV only; the forms were "read off the transpiler".
6. **Power-on values**: a decision (2026-10-29), with a known *mismatch* left in place —
   a `RegOut`'s, a `_hold`'s and every port register's simulator initial is the wire's
   while the SV says 0 (`INVENTORY_CODE.md` unstated 8).
7. **Startup before the first edge** (`out_first_driven_late`, `late_constant_one_edge`):
   decided by refusal; the "one edge earlier agrees" measurement is sim ≡ SV only, and the
   harness never observes before the first edge.
8. **Memory beyond latency-1 `ReadFirst`**: `WriteFirst` (`ram_wf_*` fixtures, circular),
   latencies above 1 (none), write–write collisions (none), `data()` before ready
   (mismatched: sim panics, SV reads 0).
9. **X semantics**: nothing can anchor it under a two-state Verilator; a four-state
   simulator (Icarus is installed for the throughput benchmark) could.
10. **Multi-domain interleave independence**: anchored at three fixed ratios; the
    never-ticked-domain default has no anchor and no statement.
11. **Combinational default-then-override and the single-tick comb mux** (`e16j`,
    §10.10–10.12): sim ≡ SV only.
12. **The dynamic, path-dependent anchor itself** (the simulator's parking rule): every
    shape where it matters is refused, so no anchor exists for the *definition*; V8c,
    cited as its falsification test, measured sim against the transpiler's `assign`.

## D. Reference interpreter (`copper-ref`) — the third leg, 2026-11-05

Session copper-ref-interpreter, branch `claude/copper-ref-interpreter-c4bbc9` (cut from
`refs/heads/paper` at `ea54d26`; nothing pushed). The interpreter is a new workspace crate
(`copper-ref/`, README there) that runs a `#[hardware(sequential|synchronizer)]` module's
`syn::ItemFn` under SPEC §I.0.2's one-resumption protocol (D-8) with no proc macro, no
rustc lowering and no executor: `Module::new(src, module, params)`, `resume(inputs)` (one
resumption, every `In::read()` = `x_{N+1}`, `RegOut` writes pending, last write wins),
`edge()`, `observe()` (`init(q)` before edge 1). A `TickSite` is the statement/arm/loop
index path of the tick. `RefBits` ≡ `Bits<N>` is pinned by `copper-ref/tests/bits_property.rs`
at widths {1, 4, 8, 16, 32, 64, 128} on X-sprinkled random operands (every operator,
method, X-aware comparison, width conversion — the first executable check of I.14; green).

**Wiring.** `build.rs` emits `<name>_reference_leg` per corpus module (the sweep's stimulus
policy replayed: `resume` with cycle N's inputs, `edge`, `observe` vs the simulator's
`obs_{N+1}`; no Verilator) and adds the leg to `<name>_differential` so one failure report
gives the three-way reading. `REF_SKIP` (validated like `SKIP`) excuses a module by name; the
deferred classes are computed from the signature. `tools/regression.sh` G-F counts the cases
(fails on zero) and runs the leg under `--quick`. `tests/fixtures/q1_b_rewrites_dut.rs` holds
the five B rewrites of Q1 §4, so they sweep and run the leg.

**Coverage (build log, `cargo build`):** `reference leg: 46 module(s) run through copper-ref,
113 REF_SKIP'd (4 Memory (I.10), 25 combinational (I.11), 1 measured disagreement (REF_SKIP
row), 83 plain Out (D-8, awaits Phase C))`; the sweep's 7 `SKIP` modules are not asked.
The 46 = 41 corpus modules in the one-job spelling (every output `RegOut`, no memory
binding, one clock) + the 5 B rewrites.

**The obs_0 gap (III.2):** the harness records from `obs_1` (`record_differential` after
`tick_clock`), so the leg compares `obs_1` onward; `obs_0 = init(q)` is asserted on the
interpreter's side only (`Module::observe` before the first `edge`), never against the
simulator or the SV. Phase C's "observe obs_0" (I.9.4, D-1 A1) closes it on all three legs.

### D.1 The leg on the corpus — predicted, then measured

Prediction (scratchpad `predictions.md`, written before the first run): today's simulator
produces the one-job trace iff every `In` read site is *deferred* (some path from it reaches
a tick before the loop's back edge) — the barrier parks it to the pre-edge settle, which
reads `x_{N+1}`; a *trailing* read (no tick after it in the iteration) reads `x_N` at `t_N⁺`
today and `x_{N+1}` under D-8. Scanning the 47: one trailing read,
`regout_trailing_single_tick` (`tick; let d = step.read(); acc = acc + d; q.write(acc)`).
Predicted: 46 AGREE, 1 DISAGREE with sim = SV ≠ ref (the linear route's trailing fold +
committed-value drive, I.6.9, realizes the same `Σ x_1..x_N`).

Measured (`cargo test --test corpus_generated -- reference_leg`): **46 agree** (every
stimulus sequence, 864–1000 cycles each), **1 disagrees as predicted**:
`regout_trailing_single_tick`, 854 of 864 cycles, first at cycle 1 (`q` simulator 153,
interpreter 251); its `_differential` case: `verilator: PASS`, reference leg FAIL — **sim =
SV ≠ ref**. Reading: both implementations share the two-job reading (I.0.2: the trailing
region is the head of the next resumption); not a transpiler bug, not a simulator bug of its
own, but the residue Phase C removes with the second resumption. Recorded as a `REF_SKIP` row
stating the measurement and pinned by
`tests/reference_leg_measurements.rs::regout_trailing_single_tick_is_the_two_job_residue`
(asserts today's sim shows `Σ x_1..x_N`, SV passes, interpreter errs) — it fails when Phase C
lands, which is when the row is deleted. One interpreter bug found on the way (an empty
`_ => {}` arm tripped the resume-index check), fixed before the second run.

The five B rewrites (Q1 §4): all five agree with the simulator on every sequence (and sweep
sim ≡ SV), as Q1 measured them against their anchors — the first inputs the interpreter had
to agree on, and it does.

### D.2 (i) The startup probes: "sim vs transpiler" becomes "spec vs transpiler"

`tests/ref_probes/startup_probes.rs`, each run through all three legs by
`tests/reference_leg_measurements.rs` (Verilator required; a missing Verilator fails the
test, never skips it). Predicted for all three: **sim = ref ≠ SV**.

| probe | II.3 row | predicted | measured (transpiled / interpreter) |
|---|---|---|---|
| `preloop_probe` (§B, extraction route: a pre-loop `q.write(7)` re-run at every return to state 0) | (f2) | sim = ref ≠ SV | SV FAIL (37 of 64 cycles) / **64 cycles agreed** |
| `preloop_linear` (the same write, linear route: dropped) | (f3) | sim = ref ≠ SV | SV FAIL (4 cycles: the quiet-start cycles before the first `go`) / **64 agreed** — the first run was masked by a first-cycle `go = 1` (D-11's probe (ii) effect), fixed by holding `go = 0` for 4 cycles |
| `trailing_fold` (`let r = 5; loop { tick; q.write(r) }`, linear: SV `obs_1 = 5`) | (f4) | sim = ref ≠ SV at `obs_1` only | SV FAIL at recorded cycle 0 only (`q expected 0 got 5`) / **16 agreed**; the coroutine's `obs_1 = 0` asserted |

So each of the three defect rows is now a *spec* finding: the interpreter, which never saw
the simulator, agrees with it against the SV on all three. The rows stay
ADMITTED-AND-DIVERGENT until the entry of I.9.2 lands; these probes are the fixtures that
move with the fix (II.3.6 item 1). Commands:
`cargo test --test reference_leg_measurements preloop_statement_is_spec_vs_transpiler_on_both_routes`,
`… trailing_fold_at_edge_1_is_spec_vs_transpiler`.

### D.3 (ii) `pc` states vs tick sites — the I.0.4 encoding, measured

`tests/reference_leg_measurements.rs::pc_states_vs_tick_sites`: per corpus module, the
extraction route's `pc` states (the same pipeline `transpile_fir` runs —
`capture_frontend_ir`, `desugar_tick_waits`, `desugar_counted_loops_in`, `extract_control`;
arms of the `match pc` less the `_` default) against the interpreter's static tick sites
(`copper_ref::static_tick_sites`) and, where the leg runs, the sites reached under 4 seeds ×
200 cycles. Predicted: on every extracted module states ≥ sites + 1 (state 0 is the head;
a ticking branch's continuation is lowered once per arm), ratio ≥ 1 everywhere.

Measured: 140 modules with a tick, **34 extracted**. Histogram (states/sites: modules):
`2/1: 5, 2/2: 4, 2/3: 2, 3/2: 6, 3/3: 4, 4/1: 1, 4/4: 6, 4/6: 1, 5/4: 1, 9/4: 1, 11/5: 3`.
**The prediction was wrong in direction: the ratio runs both ways.** `det_010_awaits` has 6
sites and 4 states (0.67), `if_tick` and `match_tick` 3 sites and 2 states, while
`counted_repetition` has 1 site and 4 states (4.00), `uart_rx` 5 sites and 11 states (2.20).
The mechanism, read off `control_extract.rs`: a `pc` value encodes **the continuation
resumed at** — state 0 is "resume at the loop head", and every tick whose continuation is
the head (the trailing tick of each branch of `if_tick`, every arm's last tick in
`match_tick`, `det_010_awaits`'s `else { tick }` and its `while` body's tick) is emitted as
`pc = 0` (`:1213-1215`): several *sites* collapse into one *state*. In the other direction
the continuation after a ticking branch is duplicated per arm (`:1310`, `:1315`, `:1333`),
and a counted `for`'s hoisted tick gets states of its own: one *site* spreads over several
*states*. So `pc` is many-to-many against H(M)'s sites, not many-to-one, and the
proposition I.0.4 states ("`pc` is allocated per tick occurrence … many-to-one onto tick
sites") needs the first clause corrected: `pc` is allocated per *resumption start point*
(continuation), and its relation to tick sites is the composition of "sites with one
continuation merge" and "one site's continuation is cloned per arm". For the coordinator:
SPEC I.0.4 and I.0.7 (`:1217`, "allocates `pc` per tick occurrence") should say this.
Reached sites: every leg-eligible module reaches all its sites under the sweep stimulus
except the three UARTs (`rx`, `uart_rx` 1 of 5, `uart_tx` 2 of 4: a 434-cycle bit time does
not fit 200 cycles; the fixture `uart_rx_dut` with `CLKS_PER_BIT = 8` reaches 5 of 5) —
the stimulus policy's known blind spot for slow protocols, not the interpreter's.

### D.4 (iii) The poison run — I.0.5(a) on 47 modules

`tests/reference_leg_measurements.rs::poison_run_for_the_register_proposition`: for every
leg-eligible module, `targets = assigned_in_loop(kernel) − infer_registers(f)` (I.7.1 (i)
minus the inference's set; match-arm bindings and the hidden `for` counters excluded), then
4 seeds × 200 cycles with every target overwritten by a poison value at every suspension,
against the clean run. A read of a poisoned local before its reassignment is reported by
name; a trace difference without one is reported too.

Predicted (before the run): the I.7.5 fold defect flags serializer/`word`, uart_tx/`byte`,
det_for/`ok` (both copies); nothing else. **Measured: 47 clean, 0 flagged.** The prediction
mis-applied I.7.5: each of those locals is live out of a *tick node that precedes its `for`*
(serializer's start-bit tick, `uart_tx`'s start-bit `for`, `det_for`'s own fold node whose
out-edge is a tick and whose `uses` include `ok`), so backward liveness carries it into the
register set. The defect needs the read reachable *only* through the fold node's entry with
no tick between the definition and the loop — `loop { let v = x.read(); for i in 0..4 {
q.write(v + i); tick } }` — and no corpus module has that shape (I.7.5: "no fixture has the
shape"). That probe (`tests/ref_probes/sync_2ff_tick_last.rs::fold_defect`) is in the test:
`infer_registers` = {`i`} (no `v`), the poison run **flags `v`** — the first corpus-shaped
positive, proving the walk has teeth. The two first cases of I.7.7: the tick-last `sync_2ff`
(`ff2 = ff1; ff1 = d.read(); q.write(ff2); tick`) infers {`ff1`}; poisoning `ff2` leaves the
trace unchanged (**the first negative**); forcing `ff1` into the poison set fails at the
second resumption with `Poisoned("ff1")` (**the first positive**). So on today's corpus the
inference under-infers nothing the one-job reading can observe, and the one known
under-inference (I.7.5's fold) is reproducible on demand. Pinned: the flagged set is asserted
empty; the fold probe's flag asserted present.

### D.5 (iv) rustc's layout as a superset of I.7.1

`tests/reference_leg_measurements.rs::future_size_bounds_the_register_storage` prints
`size_of_val` of seven fixtures' futures beside their inferred registers; the per-variant
layout came from `RUSTC_BOOTSTRAP=1 cargo rustc --test reference_leg_measurements --
-Zprint-type-sizes` (stable `cargo 1.92.0`; no nightly is installed, the bootstrap variable
unlocks the flag on stable; log in this session's scratchpad `type_sizes.log`, 6869 lines).

| module | bytes | inferred registers | locals rustc keeps at the **tick** variant(s) |
|---|---|---|---|
| `mac_fsm` | 88 | c_latch, product, result, stage | Suspend3 (`ClockTick`): product, c_latch, result, **stage** — exactly the set; the three barrier variants (`PreEdgeBarrier`, 0 bytes) keep product/c_latch/result but not `stage` |
| `regout_trailing_single_tick` | 56 | acc | Suspend0: acc — exactly |
| `capture_after_wait` | 64 | captured | Suspend3 (the tick after `let captured`): captured; Suspend1/4 (the wait tick, the last tick): nothing — exactly, per site |
| `counted_repetition` | 80 | i | Suspend1 (the inner tick): the outer `Range` iterator (16 B) and the inner one (8 B); **not `i`** — `i` lives only in the barrier variant before `d.read()` |
| `uart_rx_dut` | 88 | byte_val | the five tick variants keep byte_val where live plus the `Range` iterator of each enclosing `for` (`_`-bound loops included) |
| `regout_multiphase` | 64 | lo | lo at the mid tick |
| `handshake` | 64 | n | n |

Reading: at every tick suspension rustc keeps **exactly the inference's registers that are
live at that site, plus the counted-`for` iterators** — the `Range` whose `start` field is
D-10's counter `c`. Where the loop binds `i` (`counted_repetition`), the inference names `i`
and rustc stores the iterator and drops `i` (dead at the tick, re-bound each iteration);
where it binds `_` (every `for _ in 0..K { tick }`), the inference names nothing and rustc
stores the iterator all the same. So rustc's layout ⊇ I.7.1's registers, with the surplus
being precisely the counted-loop counters under the iterator's name — the state D-10 says
the kernel has (I.7.2(g) as re-read by D-10) and the inference names by the binding or not
at all. No locals beyond those; no barrier-only locals survive at a tick. The bound holds
and is tight up to the counters.

### D.6 What the SPEC must say that it does not (for the coordinator; SPEC files untouched)

1. **I.0.4 / I.0.7:** `pc` is allocated per *continuation* (resumption start point), not per
   tick occurrence; its relation to tick sites is many-to-many (D.3): sites whose
   continuation is the head merge into state 0; one site's continuation is cloned per arm.
2. **I.7.2(g) / D-10:** the inference names the `for` binding (`i`) or nothing (`_`); the
   kernel's counter `c` is what rustc stores (the `Range` iterator) and what the transpiler
   realizes (`__copper_ctr`); the reconciliation of these three names belongs in I.7.6/III.
3. **III.2:** the leg compares from `obs_1`; `obs_0 = init(q)` is asserted on the interpreter
   alone until the harness observes it (I.9.4).
4. **The trailing-read class (D.1):** a module in the one-job spelling whose only read of an
   input follows its last tick (`regout_trailing_single_tick`) is where today's simulator and
   SV both differ from I.0.2; Phase C's one-resumption executor and the retirement of the
   trailing fold (I.6.9's "III's to keep or drop" — measured: it must go) close it together.
   Until then the row in `REF_SKIP` is the record.
5. **II.1 / the interpreter's refusals:** `Bits::all_ones()` with no width in its spelling has
   no value the kernel can state (the interpreter refuses it; the sim infers `N` from
   context); an inclusive range in a ticking `for` is refused by the interpreter (I.2.4
   "exclusive ranges only") though the macro admits it — parity items for II.1.3.
6. **I.14 / the constant evaluator:** the interpreter folds `const` items with expression
   bodies and const generics as values (II.3 row (d)'s first two shapes) and refuses
   associated consts and const fns; `is_constant_expr` accepts all four syntactically.

## E. Phase C step 1 — the harness implements the agreement relation (2026-11-06)

Branch `claude/phase-c-harness-5ee569` (from `paper` at `649ea17`). SPEC §III.2.0–III.2.13
implemented in `tests/common/mod.rs` (+ `tests/common/relation.rs`), the generated sweep
cases (`build.rs`), the relation testbench form (`copper-sim/src/verification.rs`,
`SimulationTrace::relation`) and `tools/regression.sh` (G-G). Every prediction below was
written into this file before its measurement ran.

### E.0 Predictions (written before the first run)

- **(i) `obs_0` on the corpus.** A `RegOut` port: sim = the `registered_wire` initial (0),
  ref = `init(q)` (0), SV = the port's `initial` (0) → agree on every leg, every sequence;
  so the 46 reference-leg modules agree at `obs_0` on all three legs. P15's probes (f2)–(f4)
  (`preloop_probe`, `preloop_linear`, `trailing_fold`, all `RegOut`): agree at `obs_0` on all
  three legs; their divergences stay where they are (obs_1 and later). A plain `Out` written
  before its first tick (an opening drive): realized by the forwarded comb/hold form, agrees.
  A plain `Out` first written AFTER a tick and realized as a continuous `assign` of a
  register/input expression: sim shows the wire's creation zero at `obs_0` (the write runs
  at `t_1⁺`), SV shows the expression at power-on with `x_1` → disagrees whenever that is
  non-zero. A static scan finds 62 such ports in ~45 modules; predicted to disagree for
  certain: `bit_not_bits`, `bit_not_via_xor`, `bit_not_bool` (`!` of a zero register is all
  ones); the rest depend on the emitted form and are not predicted individually; expected
  count 3–15 modules, every one pattern `sim≠SV,no-ref` (plain `Out` ⇒ no reference leg).
  The 3 `RESET` modules: `obs_0` declared out of the SV pair by the row on every sequence,
  not counted OUTSIDE. Combinational modules: no separate `obs_0` (I.11.2(e): it is the
  first evaluation, already the first compared observation).
- **(ii) OUTSIDE (X vs SV) on the corpus: 0 sequences.** The only `Bits::x()` registers are
  the 3 `RESET` modules' (`obs_0` declared out; the reset assigns them before `obs_1`), and
  the only other X source swept until now was `mod8`'s `% 0`, which is a `SKIP`. Today's
  green sweep is itself the evidence: an X written as 1 against Verilator's 0 would fail.
  Deliberate X probe (`a % b`, `b` driven 0 on cycle 3): OUTSIDE from that observation on,
  sim ≡ ref still compared exactly through the end.
- **(iii) TRUNCATED probes.** X into `as_usize` (no memory, reference leg runs): sim and ref
  fail at the same resumption → TRUNCATED at it, SV agreeing before. Out-of-range memory
  address and undelivered `data()` (both have a `Memory` binding, so no reference leg):
  TRUNCATED at the failing resumption with "reference leg not run", SV agreeing before.
- **(iv) Patterns of the pinned/excused disagreements.** `preloop_probe` (f2),
  `preloop_linear` (f3), `trailing_fold` (f4): `sim=ref≠SV`. `regout_trailing_single_tick`
  (P12, its `REF_SKIP` row): `sim=SV≠ref`. The `SKIP` demonstrations `out_from_reg_before_commit`,
  `vr_sink_mealy`, `vr_skid`: still divergent, `sim≠SV,no-ref` (plain `Out`); `probe`: agrees
  (§A found the row stale on 2026-11-03).
- **(v) Bare regression.** 159 differential cases, every sequence PASS except the `obs_0`
  disagreements of (i) (excused by a row each once measured); OUTSIDE 0, TRUNCATED 0,
  DISAGREE 0; reference leg 46 cases, every sequence PASS.

### E.1 (i) Observation 0 on the corpus — predicted, then measured

Measured (`cargo test --test corpus_generated`, ledger `COPPER_VERDICT_LEDGER`, first run
on the new harness): **25 modules / 131 sequences disagree, every one at observation 0,
every one on a plain `Out` port, every one pattern `sim≠SV,no-ref`**; nothing disagrees at
any later observation (the 1,148 other sequences pass as before), no `RegOut` port
disagrees anywhere, and the reference leg agrees at `obs_0` on all 46 modules (385
sequences). P15's probes: `obs_0` agrees on all three legs for (f2), (f3), (f4) — their
divergences stay at obs_1 (f3, f4) and obs_7 (f2), as before.

**Predicted vs measured.** RegOut ports agree on every leg — **as predicted** (0 of the 46
reference-leg modules, 0 RegOut ports anywhere). The plain-`Out` class — **predicted 3–15
modules, measured 25**, and the prediction named only one of the two mechanisms. The
three named for certain (`bit_not_bits`, `bit_not_via_xor`, `bit_not_bool`) do disagree.
By direction (sim value / SV value at `obs_0`):

- **(A) sim 0, SV ≠ 0 — the continuous-assign transient** (predicted mechanism), 18
  modules: `bit_not_bits`, `bit_not_bool`, `bit_not_via_xor`, `dynamic_shift`,
  `lit_width_in_ternary`, `lit_width_via_locals`, `manual_sra` (bits_ops);
  `sign_extend_via_cast`, `sign_extend_via_mask`, `signed_lt_after_sub`,
  `signed_lt_via_bias`, `signed_lt_via_cast` (signedness); `held_temp_trap`,
  `total_expr_temp` (aggregate_locals); `block_binding_shadow`, `sra_method`
  (struct_pipeline); `regfile_write_through`; `late_constant_one_edge` (port `dv`). The
  write follows the tick, so today's simulator runs it at `t_1⁺` and shows the wire's
  creation zero at `t_1⁻`; the emitted `assign o = …` shows the expression from time 0
  (`bit_not_bits`: `assign o = (~a)`, SV `obs_0 = ~x_1`).
- **(B) sim ≠ 0, SV 0 — the head's pre-edge write** (NOT predicted), 5 modules:
  `sipo_block` (`w0_dbg`, 19/20 sequences), `plain_out_multiphase` (`seen_o`, 19/20),
  `branch_merge` and `branch_merge_explicit` (`mid_o`), `conditional_plain_out` (`o`).
  The first resumption writes the port before its first tick with `x_1`, which today's
  simulator shows at `t_1⁻`; the SV's hold register / flop shows 0. No later observation
  can show this instant (each `obs_N`, N ≥ 1, is read at `t_N⁺`).
- **(C) both ≠ 0** (NOT predicted), `lfsr` (both copies, 2 sequences each — the ones whose
  first cycle has `yumi_i = 1`, `reset_i = 0`): the simulator shows the next state written
  pre-edge, the SV `o = state` shows the register's `initial` 1 — (B) with a non-zero power-on.

**The anchor's side.** `bit_not_bits` has an independent reference
(`tests/fixtures/reference_sv/bit_not_bits.sv`, `assign o = ~a`): it **agrees with the
transpiled SV** at `obs_0` (5 sequences, the same values). The only anchored reading of
the instant sides with the SV; the simulator's zero is the two-job artifact.

**Handled:** excused by a signature class, `Module::obs0_excuse` in `build.rs` (the
analogue of `ref_skip_reason`'s plain-`Out` class): on a clocked module, a disagreement
between the simulator and the SV (or the independent reference) **at observation 0, on a
plain `Out` port, only**, is the verdict **EXCUSED** with its pattern and the sentence
"under the one-job body there is none and obs_0 = init(q) (D-8, SPEC I.2.2, I.9.4) …
awaits the Phase C rewrite to RegOut". A mismatch on a `RegOut`, or at any later
observation, still fails. Not pinned per module: the class disappears with D-8's rewrite,
and G-G prints the count (131) on every run.

**`RESET` rows.** Observation 0 of a `RESET` module is declared outside the SV pair by its
row (III.2.2, "from reset"); 42 sequences across the 4 swept copies (`shift_register` ×2,
`simple_counter`, `rv32i_cpu_transpilable`), printed as such and not counted OUTSIDE. Its
`obs_0` is still compared sim ≡ ref (none of the four is in the reference leg today). From
`obs_1` on the rows are unchanged: the reset asserted on cycle 0 defines the state, and an
X recorded after it would be OUTSIDE (none was).

### E.2 (ii) OUTSIDE (X vs SV)

**Corpus: predicted 0, measured 0** sequences (of 1,279). **Deliberate probe**
(`tests/ref_probes/agreement_probes.rs::xp_rem`, `q <= a % b`, `b = 0` on cycle 3;
`tests/agreement_relation.rs::an_x_observation_takes_the_sv_pair_outside`): **predicted
OUTSIDE from recorded cycle 3, port `q`, the reference agreeing on all 12 cycles X
included; measured exactly that** (`OUTSIDE (X vs SV, III.2.2) from cycle 3 (recorded
cycle 3), port q`; `reference leg: 12 cycles agreed`; the SV compared and agreeing on
`obs_0`–`obs_3`). The relation testbench refuses an X expected value outright
(`the_relation_testbench_never_compares_an_x`): the legacy "compare X as 1" is unreachable
under the relation. `mod8`'s `SKIP` row (a `% 0` X) is the corpus instance of this probe
and could now sweep as OUTSIDE — listed in E.6, not changed.

### E.3 (iii) TRUNCATED — the three failure kinds

`tests/agreement_relation.rs`, each probe driven as a generated case drives a module
(`run_step` around every step, observation 0 between the settle halves):

| Probe | Stimulus | Predicted | Measured |
|---|---|---|---|
| `trunc_x_cond` (X into `as_bool`) | `b = 0` on cycle 5 | TRUNCATED at resumption 6, sim and ref fail together, SV agrees before | **as predicted**: "Cannot convert X to bool"; `reference leg: 5 cycles agreed … 1 sequence(s) truncated at an agreed run-time failure` |
| `trunc_oob` (address ≥ 10 into a 10-entry ROM) | address 12 on cycle 7 | TRUNCATED at resumption 8, reference leg not run (`Memory`), SV agrees before | **as predicted**: "address 12 is out of range — this memory has 10 entries" |
| `trunc_data` (undelivered `data()`) | `go = 0` on cycle 6 | TRUNCATED at resumption 7 by the simulator's count, reference leg not run | **as predicted**: "read pipeline output is not yet valid" |

One probe defect on the way, not a verdict: the first `trunc_x_cond` used only bit 0 of
`a % b`, and Verilator's `-Wall` refused the SV (`UNUSEDSIGNAL r[7:1]`) — the verdict was
already TRUNCATED at resumption 6 with the reference agreeing; the probe now writes `r`
in one arm. Under one job (D-8), `trunc_data`'s `data()` follows the tick, so it is the
head of resumption 8, not 7: today's simulator runs it at `t_7⁺` inside cycle 6's
`tick_clock`. The reference cannot say which (no memory model), so the resumption number
is the simulator's. Also pinned: a panic that is not a III.2.4 failure (a combinational
loop) fails the case instead of truncating it (`an_unrecognized_panic_is_a_failure_not_a_truncation`).

### E.4 (iv) The pattern of every pinned or excused disagreement

| Disagreement | Predicted | Measured | Handled |
|---|---|---|---|
| `preloop_probe` (II.3 (f2), P15) | `sim=ref≠SV` | `sim=ref≠SV` (first at cycle 6: sim 5 ref 5 SV 7) | pinned, `record_pin` |
| `preloop_linear` (f3) | `sim=ref≠SV` | `sim=ref≠SV` (cycle 0: sim 7 ref 7 SV 0) | pinned |
| `trailing_fold` (f4) | `sim=ref≠SV` | `sim=ref≠SV` (cycle 0: sim 0 ref 0 SV 5) | pinned |
| `regout_trailing_single_tick` (P12, `REF_SKIP`) | `sim=SV≠ref` | `sim=SV≠ref` (cycle 1: sim 215 ref 28 SV 215) | pinned; `REF_SKIP_PATTERN` names the pattern and G-G matches the two |
| the 25 `obs_0` modules (E.1) | — (new) | `sim≠SV,no-ref` (131 sequences) | EXCUSED by class |

### E.5 The `SKIP` rows re-checked (CLAUDE.md: a reason naming emitted text cannot detect its own staleness)

Re-transpiled with the CLI, then run through the new harness with the row removed
(temporary `build.rs` edit, restored; the tree is unchanged). Predicted before the run:
`probe` PASS on every sequence (§A); `out_from_reg_before_commit`, `vr_sink_mealy`,
`vr_skid` still DISAGREE at a later observation, pattern `sim≠SV,no-ref` (plain `Out`);
`mod8` (combinational, `o = a % b`) OUTSIDE on the sequences that draw `b == 0` — the row
names sequence 1 (cycle 59), sequence 2 (cycle 27) and the pattern list (sequence 4,
cycle 0) — and PASS on the rest, never DISAGREE.

Measured — **every row as predicted**:

| Row | Re-transpiled | Measured under the relation | Status of the reason |
|---|---|---|---|
| `probe` | `assign out = x_r` | 6 PASS of 6 | **stale** (as §A found 2026-11-03): delete the row (Phase C housekeeping, not step 1) |
| `out_from_reg_before_commit` | `assign q = acc` before `acc <= acc + d` | 6 DISAGREE of 6, `sim≠SV,no-ref`, first at obs_1 (sim 0, SV 52) | accurate; it cites `pretick_out_write_before_update`'s trailing clause, a rule D-8 retires in Phase C (still live in code) |
| `vr_sink_mealy` | `assign ready = can_take` | 11 DISAGREE of 12, `ready` sim 1 SV 0 | accurate; cites the same rule (D-8 retires it) |
| `vr_skid` | `assign in_ready = ir` | 12 DISAGREE of 12, `in_ready` sim 1 SV 0 | accurate; same |
| `mod8` | `assign o = (a % b)` | 2 PASS, **3 OUTSIDE** — sequence 1 from cycle 59, sequence 2 from cycle 27, sequence 4 (pattern list) from cycle 0, exactly the row's points; 0 DISAGREE | the row's premise ("an X-aware comparison … would turn every X into a silent pass") no longer holds: the OUTSIDE verdict is counted, not silent. The module could sweep; not changed in step 1 (E.6) |
| `rv32i_cpu_pipelined`, `two_domain_top` | — | not run (a `Memory` parameter; a structural two-clock parent) | accurate (III.2.6, III.2.7 preconditions) |

No disagreement seen here at observation 0: all three demonstrations already disagree
from obs_1 or later, so none of them depends on the E.1 excuse.

### E.6 What the SPEC must say that it does not (for the coordinator; SPEC files untouched, §III.3 left TBD)

1. **III.2.10 lacks the excusing class step 1 needed.** Observation 0 of a plain `Out` on
   a clocked module (25 modules / 131 sequences, E.1) is excused by a signature class
   (`Module::obs0_excuse`). III.2.10 lists SKIP, REF_SKIP, RESET and `ref_skip_reason`'s
   classes only. I.9.4 calls the plain-`Out` startup items vacuous under D-8, which is
   true of the language but not of today's corpus. The row belongs beside "83 plain
   `Out` (D-8's rewrite)", and it is deleted with that rewrite.
2. **III.2.4's "same resumption" before P12 lands.** A failure in a statement after the
   tick (`trunc_data`'s `data()`) runs in today's simulator at `t_{N+1}⁺`, inside
   recorded cycle N. Under one job it is the head of resumption N+2. The harness counts
   the simulator's position; the spec should say whose count the relation uses until the
   second job is removed, or that such a failure is a P12 disagreement.
3. **III.2.4's failure list is recognized by text.** The simulator has no error channel.
   The harness reads the panic message (`runtime_failure_kind`), adds Rust's
   `index out of bounds` (an array index, I.14.6(b)), and fails the case on any other
   panic. The interpreter's `RefError::Runtime` mixes the coroutine's failures with its
   own invariants, and the harness separates them by message. The spec should state
   "any other panic is an implementation failure". A distinct interpreter error variant
   is a Phase C item.
4. **III.2.2 should say the cut is per environment.** The OUTSIDE cut applies per
   stimulus sequence (III.2.0's unit), and sim ≡ ref keeps comparing past it. The
   harness does both.
5. **III.2.1 for a combinational module.** `obs_0` is the first evaluation (I.11.2(e)),
   already compared as the first cycle; the harness adds nothing. III.2.1 should say so.
6. **III.2.11 vs III.2.1: the anchors' `obs_0`.** A `REFERENCE` module's hand-written SV
   is now compared at `obs_0` too, under the same X and truncation cuts. At `obs_0`,
   `bit_not_bits`'s anchor sides with the SV against the simulator (E.1). The relation
   does not say whether the anchors are held to `obs_0`. The harness holds them to it
   and applies the plain-`Out` class excuse.
7. **III.2.2 "from reset" vs sim ≡ ref.** Observation 0 of a `RESET` module is outside
   the SV pair (42 sequences). The harness still compares it sim ≡ ref, X included. The
   spec implies this; it should state it.
8. **`mod8`'s `SKIP` (III.2.10 (b)) is now unnecessary.** Under the OUTSIDE verdict it
   sweeps as 2 PASS / 3 OUTSIDE, exactly at the row's points. Deleting the row is a
   one-line change, left to the coordinator; it changes G-D's counts by one.
9. **`probe`'s `SKIP` is stale** (§A, re-measured E.5: 6/6 PASS). Delete it with the
   Phase C housekeeping.
10. **III.1 P16 is now MEASURED by the harness**, not only by probe: `obs_0 = init(q)`
    on every `RegOut` of the corpus, on all three legs (46 modules on the reference leg,
    every swept `RegOut` on the SV leg). The row's "never by the harness" is stale.
11. **The ledger is a III.3 input.** Each run's per-sequence verdicts, patterns and pins
    are the raw material of the conformance ledger. §III.3 is left TBD as instructed.

### E.7 (v) The bare regression

`tools/regression.sh`, bare, on 12c556d (the tree of this commit minus this section), on the
laptop: **REGRESSION OK (395 s)** — most models were cache hits from the iteration runs; the
first relation run, every model a miss, took the sweep binary 153 s of test time. 1424 passed,
0 failed, 137 ignored across 142 binaries; G-C 116 test files (the 115 before plus
`tests/agreement_relation.rs`); G-D 159 differential cases + 7 ignored; G-F 46 run, 120 skipped
with a reason; G-E 5 anchored, 161 not; G-A/G-B 32/32 examples.

**G-G — predicted** (after the `--no-examples` run, before this one): identical counts; **measured**:

| Leg | PASS | OUTSIDE | TRUNCATED | EXCUSED | DISAGREE |
|---|---|---|---|---|---|
| differential (159 cases) | 1148 | 0 | 0 | 131 (`sim≠SV,no-ref`) | 0 |
| reference (46 cases) | 385 | 0 | 0 | 0 | 0 |

`obs_0` declared out by a `RESET` row: 42 sequences. Pinned disagreements: 4
(`preloop_probe`, `preloop_linear`, `trailing_fold` `sim=ref≠SV`; `regout_trailing_single_tick`
`sim=SV≠ref`, matched to its `REF_SKIP` row). As predicted, every count. E.0 (v)'s first
prediction (DISAGREE 0 before any excuse) was wrong by the 131 `obs_0` sequences of E.1.

## F. Phase C step 2 — the one-resumption simulator (2026-11-06)

Branch `claude/phase-c-simulator-29253d` (from `paper` at `a9349b0`). SPEC §I.0.2, §I.12.2,
§I.13.2(b), D-8, D-17, D-18, D-20 obligation (2). The executor gained
`ResumptionMode::{Two, One}` (`copper-sim/src/executor.rs`), selected by `COPPER_RESUMPTION`
(`one` | `two`) or `HardwareExecutor::with_resumption_mode`; **the compiled default stays
`Two`**. Under `One`, `tick_clock::<D>` settles the network (untagged tasks: the
combinational modules), resumes `D`'s coroutines once to their next `tick` (the network
re-settled over what they wrote), commits the edge, and settles the network again for the
observation; no other domain's coroutine is polled, and the post-edge settle polls no
coroutine. The macro tags every clocked module with the `D` of its one `Clock<D>`
(`HardwareModule::__new_in_domain`); the tag is read only under `One`. The barrier stays
injected and is Ready at once under `One` (a thread-local the executor sets per settle),
which is the body without the injection, so `POLL_PHASE` and its `PreEdge` default have no
meaning there. The global `cycle()` became `cycle::<D>()` (§I.1.6; no caller used it).
Every prediction below was written into the scratchpad before its measurement ran and is
copied here unchanged.

### F.0 Predictions (written before the first run under `One`)

Mechanism. Under `Two`, tick N's post-edge settle runs the code after a tick up to the first
deferred-read barrier: immediate reads there see x_N, and plain-`Out` writes there show at
obs_N. Under `One` the same code runs at t_{N+1}⁻: reads see x_{N+1}, plain-`Out` writes
show from obs_{N+1}. `RegOut` writes and register updates commit at edge N+1 in both. Code
after a deferred read runs pre-edge in both. So a module changes iff (a) it has an immediate
read whose value reaches an observable, or (b) it has a plain-`Out` write in the post-edge
(opening) part whose value is not constant over time.

- **(i) The changed set in the sweep** (Q1_TRADEOFFS §2's rows): row (3), plain-`Out` writes
  only in the opening (70 of 135 clocked), changes unless every opening write is a
  constant; immediate-read modules with no plain `Out` (40 − 30 ≈ 10) change; row (2), the
  18 closing-write modules, change only if they also have an opening write or an immediate
  read; RegOut-only deferred-read modules (≈ 37), every combinational module and the
  closing-only plain-`Out` modules are unchanged. **About 80** of the swept clocked modules
  go to DISAGREE, sim ≠ SV with the SV equal to the two-job sim, typically first at obs_1
  (obs_0 is the first resumption, identical in both modes).
- **(ii) The never-ticked pin**: `Two` q = 1 (D-17 (i)), `One` q = 2 = Verilator.
- **(iii) Two interleavings** (`two_domain_counter`, P = [2;10], Q = [1,3,1,3,2,2,3,1,2,2]):
  one per-domain trace under both, in both modes.
- **(iv)** Under `One`: the five Q1 B-rewrites PASS, the 46 reference-leg modules PASS,
  P15's (f2)/(f3)/(f4) stay sim = ref ≠ SV.
- **(v) P12** (`regout_trailing_single_tick`): under `One`, sim = ref ≠ SV — the pin flips
  from sim = SV ≠ ref.

### F.1 (i) The changed-verdict set — measured

`two` first, to show the switch changes nothing by default: `cargo test --test
corpus_generated` gives 1148 PASS / 131 EXCUSED differential sequences and 385 PASS
reference sequences, **identical to E.7**; `cargo test --workspace` 1428 passed, 0 failed
(E.7's 1424 plus the four pins of F.2–F.3).

`COPPER_RESUMPTION=one cargo test --test corpus_generated` (ledger per mode):

| Leg | PASS | OUTSIDE | TRUNCATED | EXCUSED | DISAGREE |
|---|---|---|---|---|---|
| differential (159 cases), `Two` | 1148 | 0 | 0 | 131 | 0 |
| differential (159 cases), `One` | 798 | 0 | 0 | 53 | **428** |
| reference (46 cases), both | 385 | 0 | 0 | 0 | 0 |

**66 of 159 cases change** (68 failed tests: the 66 differential cases plus two example
self-checks swept as tests, `rv32i_cpu_transpilable::selfcheck` and
`pattern_detector_2::det_010_variants_match_transition_table`). Every one of the 428 is
pattern **`sim≠SV,no-ref`** (no changed module runs the reference leg); the first
disagreement is at obs_1 in 143 sequences, obs_2 in 67, later in the rest. By signature:

| Kind (signature) | swept | changed | unchanged |
|---|---|---|---|
| combinational | 25 | 0 | 25 |
| `RegOut`-only (and one with no output) | 51 | **1** (`regout_trailing_single_tick`, P12) | 50 (incl. the 46 reference-leg modules and the 5 Q1 B-rewrites) |
| plain `Out` on a clocked body | 83 | **65** | 18 |

The 18 unchanged plain-`Out` modules are Q1 §2's closing-write list, as predicted
mechanism-wise: `sipo_block`, `det_110101` ×2, `lfsr` ×2, `shift_register` ×2,
`simple_counter`, `event_source`, `event_sink`, `slow_consumer` ×2, `rv32i_cpu_transpilable`
(sweep sequences only; its self-check fails), `plain_out_multiphase`, `conditional_plain_out`,
and three `match_selector` modules (`ifchain_on_const_expr`, `match_on_const_pattern`,
`match_on_literals`). Of the 25 modules E.1 excuses at obs_0, 20 change and 5 do not.

**Predicted about 80, measured 66.** The plain-`Out` half held (65 of the 70 + 18
predicted-at-risk; the 18 unchanged are exactly the closing-only ones). The other half was
wrong: predicted ≈ 10 `RegOut`-only modules with an immediate read to change, measured 1.
Q1's 40 immediate-read modules are almost all plain-`Out` modules in the swept set.

The 66 changed cases: `accum_2`, `accumulator_en`, `bit_not_bits`, `bit_not_bool`,
`bit_not_via_xor`, `block_binding_shadow`, `branch_local_counter`, `branch_merge`,
`branch_merge_explicit`, `bsg_counter_up_down`, `bsg_dff_en`, `counter`, `det_010` ×2,
`dual_port_ram`, `dynamic_shift`, `enabled_register`, `fast_counter` ×2, `flag_sync`,
`held_temp_trap`, `ifchain_on_usize`, `late_constant_one_edge`, `lit_width_in_ternary`,
`lit_width_via_locals`, `mac_pipeline` (the fixture), `manual_sra`, `match_on_usize`,
`mp_reg`, `narrow_index_into_narrow_addr`, `out_from_reg_after_commit`, `probe_fsm`,
`ram_r1w2_wf`, `ram_r2w2`, `ram_r3w1`, `ram_read_first`, `ram_wf_priority`,
`ram_write_first`, `regfile_read_first`, `regfile_write_through`,
`regout_trailing_single_tick`, `rom_direct`, `rom_from_contents`, `rom_from_fn`,
`sample_hold_2`, `seq6`, `sign_extend_via_cast`, `sign_extend_via_mask`,
`signed_lt_after_sub`, `signed_lt_via_bias`, `signed_lt_via_cast`, `slow_counter`,
`sra_method`, `struct_latch`, `struct_latch_cond`, `sum_hold_2`, `sync_2ff_concrete`,
`total_expr_temp`, `traffic_light` ×2, `trailing_local_into_head`, `trailing_out`,
`tuple_destructure`, `up_down_counter`, `wide_index_into_narrow_addr`,
`wide_index_sole_consumer`.

**The test suite under `One`** (`cargo test --workspace`): 1205 passed, **223 failed**, 137
ignored, in 35 binaries: `sequential_forwarding_divergence` 96 (the two-job divergence
probes, every one a plain `Out`), `corpus_generated` 68 (above), 10 unit tests (8 executor
tests whose hand-built clocked futures carry no domain tag, so `One` treats them as
network tasks resolved pre-edge, plus the two example self-checks), and 49 in 31 hand-written
equivalence, golden and anchor tests: among them `det_010_independent_golden`,
`cdc_synchronizer_anchor::sync_2ff_observable_latency_is_one_destination_cycle` and three
frozen `golden_traces` (`counter`, `det_010`, `mac_pipeline`). These are the modules D-8
rewrites, which is why the anchors fail. Under `One`, `det_010` and `sync_2ff` in their
present two-job spelling no longer match BaseJump / `sync_2ff_ref.sv`. Q1 §4 measured their
`RegOut` spellings identical to the anchors.

### F.2 (ii) The never-ticked pin — `tests/two_clock_pins.rs`

`b_reg` (`tests/ref_probes/two_clock_probes.rs`) in `ClkB`, a counter in `ClkA`; x = 1,
A ×3, x = 2, B, x = 3, B, x = 4, B, three legs through the relation harness. **Predicted
`Two` q = 1, `One` q = 2; measured the same.** `One`: resumption counts 0,0,0,0,1,2,3 (none
during A's ticks), trace (obs_0…obs_3) 0,2,3,4, sim ≡ SV ≡ ref. `Two`: counts 0,1,1,1,2,3,4
(the first resumption in A's first settle), trace 0,1,3,4, pattern **sim≠ref,SV=ref** at
obs_1 only, pinned (`record_pin`). `cycle::<ClkA>() = 3`, `cycle::<ClkB>() = 3` in both.

### F.3 (iii) Two interleavings — `tests/two_clock_pins.rs`

Per-domain traces (P11): under P and Q the fast trace per fast edge and the slow trace per
slow edge are each one sequence, in both modes, **as predicted**. Sim against the transpiled
`two_domain_top` under Verilator, per domain, P, Q and a third interleaving R =
[3,0,2,0,3,0,1,2,0,0,3,0,1,2] with empty slow periods (added after the result below):
- `Two`: sim ≡ SV on both domains under P, Q and R.
- `One`: `count_out` lags the SV by one fast observation under all three (the head write,
  as predicted for a plain-`Out` opening write). The slow chain was **predicted to differ
  and measured equal under P and Q**. That is agreement by cancellation: `flag_out` shows
  one fast cycle late, and `flag_sync`'s trailing `d.read()` moves from the slow t_N⁺ (before
  the period's fast ticks) to t_{N+1}⁻ (after them). Under R, whose empty periods break that
  cancellation, the synchronized flag rises **one slow observation late** (measured). All
  three are pinned. Step 3's rewrite removes both terms.

### F.4 (iv) The Q1 rewrites, the reference leg, P15 — under `One`

The five `q1_b_*` rewrites: PASS on every sequence (**as predicted**). The 46 reference-leg
modules: 385 of 385 sequences PASS, so sim ≡ ref under `One` wherever the reference runs, and
none of them changes against the SV (**as predicted**). P15: `preloop_statement_is_spec_vs_
transpiler_on_both_routes` and `trailing_fold_at_edge_1_is_spec_vs_transpiler` pass under
`One` unchanged, sim = ref ≠ SV (**as predicted**).

### F.5 (v) P12 flips

`tests/reference_leg_measurements.rs` now runs `regout_trailing_single_tick` under each mode
explicitly. `Two`: sim = SV ≠ ref, obs_{N+1} = Σ x_1..x_N, pinned as before. `One`:
**sim = ref ≠ SV** (**as predicted**). The trace was predicted as Σ x_1..x_{N+1} and measured
as **Σ x_2..x_{N+1}**: the first resumption runs only to the head `tick`, so x_1 is read by no
one, and obs_1 = 0. The interpreter agrees on 48 of 48 cycles. The `REF_SKIP` row's
pattern (`sim=SV≠ref`) is the `Two` pin's and stays until the default flips; under `One` the
row would name `sim=ref≠SV` (the trailing fold, D-20 step 4).

### F.6 (vi) The decision and the bare regression

**Decided (user, 2026-11-06): option A.** Step 2 lands with the default `Two`. The switch,
the per-domain counter, the two-clock pins and the per-mode P12 pins are in the tree, and
`tools/regression.sh` gained **G-H**: on every full test run the sweep runs again under
`COPPER_RESUMPTION=one`, and its verdict counts, its number of changed cases and their
patterns are printed. G-H fails on a reference-leg disagreement or a differential pattern
with sim ≠ ref; the sim ≠ SV of the two-job spellings is counted, not failed. The main run
unsets `COPPER_RESUMPTION`. The filter was checked on synthetic ledger lines:
`sim≠SV,no-ref` and `sim=ref≠SV` pass, while `sim≠ref,SV=ref`, `sim=SV≠ref` and a
reference-leg DISAGREE fail. The default flips with step 3 (B was step 3's rewrite without
its macro rule; C would excuse 428 sequences and two anchors by class).

**Predicted** (before the bare run): REGRESSION OK. G-G identical to E.7 (differential 1148
PASS / 131 EXCUSED, reference 385 PASS), with 5 pins (E.7's 4 plus `b_reg`'s never-ticked
startup, sim≠ref,SV=ref). G-H: differential 798 PASS / 53 EXCUSED / 428 DISAGREE,
reference 385 PASS, 66 cases changed, all `sim≠SV,no-ref`. G-C 117 test files (116 +
`two_clock_pins`). 1429 tests passed (1428 + the `One` arm of P12).

**Measured** (bare `tools/regression.sh` on `308beda` + this section's text, laptop):
**REGRESSION OK (442 s)**. 1429 passed, 0 failed, 137 ignored across 143 binaries; G-C 117
test files; G-D 159 differential cases + 7 ignored; G-F 46 run, 120 skipped; G-E 5 anchored,
161 not; examples 32/32. G-G: differential 1148 PASS / 131 EXCUSED / 0 DISAGREE, reference
385 PASS, 42 `obs_0` sequences declared by `RESET` rows, 5 pins (`b_reg` sim≠ref,SV=ref;
`preloop_probe`, `preloop_linear`, `trailing_fold` sim=ref≠SV; `regout_trailing_single_tick`
sim=SV≠ref, matched to its `REF_SKIP` row). G-H: differential 798 PASS / 53 EXCUSED / 428
DISAGREE, reference 385 PASS, 66 of 159 cases changed, all `sim≠SV,no-ref`. **Every count as
predicted.**

### F.7 What step 3 inherits

- Flip `ResumptionMode`'s default. Delete the `Two` arms: the P12 `Two` pin and its `REF_SKIP`
  row's `sim=SV≠ref` wording (under `One` that module is sim=ref≠SV, the trailing fold, which
  is D-20 item 4), the never-ticked `Two` pin, `PreEdgeBarrier`, `POLL_PHASE`,
  `set_one_resumption`, and the injection. G-H then folds into G-G.
- The 65 plain-`Out` sweep modules of F.1, the 96 `sequential_forwarding_divergence` probes,
  49 hand-written tests (two anchors among them: `det_010_independent_golden` and
  `cdc_synchronizer_anchor`'s latency test; three frozen `golden_traces`), and the two example
  self-checks.
- The 8 executor unit tests build clocked futures with `HardwareModule::__new` and no domain
  tag. Under `One` such a future is a network task (polled in every settle, ticks resolving
  pre-edge). Tag them (`__new_in_domain`) or rewrite them with the unit domain's removal (D-17 A1).
- `two_clock_pins`' (vi) `One` assertions (the fast lag, the slow cancellation under P/Q, the
  slow lag under R) become equalities with the SV once the CDC example is rewritten.

### F.8 What the SPEC must say that it does not (for the coordinator; SPEC files untouched)

1. **III.2.6's interleavings exercise no empty period.** P and Q give every slow period at
   least one fast tick, so a two-job spelling agrees with the SV there by cancellation. R,
   with empty periods, separates them. The relation's multi-domain pins should name one
   interleaving with a zero-tick period.
2. **A task with no domain.** I.13.2(b) and I.12.1 know two kinds, the network's
   combinational functions and a domain's coroutines. The executor also admits a hand-built
   clocked future with no domain (`__new`, test scaffolding only). Under `One` it is polled
   as part of the network. The spec should say such a task is outside the language (with the
   unit domain, D-17 A1), or the executor should refuse it.
3. **Cycle 0's input in a tick-first body.** P12's `One` trace is Σ x_2..x_{N+1}: the
   first resumption runs only to the head `tick`, so x_1 is read by no one. I.9 / I.4.4
   imply this; a sentence there would make it a stated consequence rather than a surprise
   (F.5's prediction missed it).
4. **The pre-edge network evaluation runs twice under `One` today,** before the resumptions
   and after them, because plain `Out`s still exist on clocked bodies. I.13.2(b)'s "a
   resumption changes no wire" makes the second evaluation vacuous after step 3. Until
   then, the order of two coroutines of one domain joined by a plain `Out` is
   visit-order-dependent under the fixpoint scheduler; the levelized scheduler orders them
   by the wire. This is outside the language-to-be, and nothing in the corpus measures it.

## G. Phase C step 3 — the macro, the D-8 rewrites, the flip (2026-11-06)

Branch `claude/phase-c-macro-0f3e3b` (from `paper` at `2aafbed`). SPEC II.1 R8, D-8, D-13
(II.3.2's deletion list), D-20 obligation (3). Every prediction below was written here before
the code it predicts was changed.

### G.0 Inventory and predictions (written before any code change)

Measured by a scan of every `#[hardware(sequential|synchronizer)]` signature in the tree
(`paper/` and `target/` excluded) for a plain `Out<`: **697 module definitions in 101 files**,
counting the Rust-source strings (`r#"…"#`, `parse_quote!`) that tests transpile or analyse.
By group:

**(A) Anchored to an independent reference — rewritten first.**

| module(s) | file(s) | reference | predicted trace (new spelling, `Two` and `One`) |
|---|---|---|---|
| `sync_2ff`, `sync_2ff_concrete` (and `structural_hierarchy.rs`'s copy) | `src/sync.rs`, `tests/fixtures/sync_2ff_dut.rs` | `examples/cdc/sv/sync_2ff_ref.sv` (`cdc_synchronizer_anchor.rs`) | identical, in the measured tick-last spelling (`tests/ref_probes/sync_2ff_tick_last.rs`). **G2 `NameExact` predicted to FAIL**: inference gives `{ff1}` (`ff2` is a wire of one resumption, D-8 packet note 1), the reference `{ff1, ff2}`; the second flop is the `RegOut` `q`, which G2's convention (output ports excluded) does not count |
| `sync_1ff` (the one-flop negative control) | `tests/cdc_synchronizer_anchor.rs` | the 2-FF trace | still one destination cycle faster than `sync_2ff` |
| `det_010` (example + fixture) | `examples/sequential/pattern_detector_2.rs`, `tests/fixtures/det_010_dut.rs` | `pattern_detector_010.sv` | identical (Q1 §4 (a)); G2 `StorageEquivalent` unchanged (`{state}`) |
| `det_110101` (example + fixture) | `pattern_detector.rs`, `pattern_detector_dut.rs` | `pattern_detector.sv` | identical on the example's stimulus; the LCG overlap at cycles 15/271 is the golden's (Q1 §4 (i)) and is not on any test's stream |
| `bsg_dff_en`, `bsg_counter_up_down`, `sipo_block` | `examples/basejump/` | BaseJump | identical (Q1 §4 (b), (c), (c'')) |
| `lfsr`, `shift_register` (example + fixture each) | `examples/sequential/`, fixtures | `lfsr.sv`, `shift_register.sv` | identical (closing writes; unchanged under `One` in F.1) |
| `dual_port_ram` | `examples/memory/` | `dual_port_ram.sv` | identical |
| `fast_counter`, `flag_sync`, `slow_consumer` | `examples/cdc/two_domain_hierarchy.rs` (+ `two_clock_pins.rs`, `two_domain_hierarchy_cdc.rs`) | `two_domain_hierarchy.sv` and the transpiled `two_domain_top` | identical on both domains under P, Q and R in both modes; `two_clock_pins` (vi)'s `One` lag assertions become equalities |
| `ram_read_first`, `bit_not_bits` | fixtures (`REFERENCE` rows) | `reference_sv/` | identical |
| `accumulator_en`, `held_temp_trap`/`total_expr_temp`/`enabled_register` | fixtures | `reference_sv/accumulator_en_dut.sv`, `aggregate_locals_dut.sv` | identical |
| `stage_add_one`, `stage_double`, `counter_by` | `tests/module_composition_hybrid.rs` | `reference_sv/hybrid_pipeline.sv` | identical |
| `dff`, `dff_after`, `enff`, `counter` | `tests/timing_probe_investigation.rs` | `timing_probe_sv/` | identical (`dff_after` becomes `dff`'s spelling) |
| `ram_straddle`, `ram_prewrite` | `tests/mem_latency_probe.rs` | `timing_probe_sv/ram1.sv` | identical |
| `rv32i_cpu_pipelined` | — | 13 programs (`rv32i_pipelined_verilator.rs`) | **not affected**: no plain `Out`; 13/13 unchanged |
| `rv32i_cpu_transpilable` | `examples/cpu/` | its `selfcheck` | identical (closing writes) |

**(B) The remaining swept modules** (`examples/`, `tests/fixtures/`): 87 plain-`Out` modules,
83 swept (F.1's 65 changed + 18 closing-write) and 4 `SKIP` rows. Predicted: every swept
rewrite PASSes under both modes; the `obs_0` EXCUSED class (25 modules, 131 sequences) shrinks
to 0 as its members become `RegOut`. The four `SKIP`-row modules: `probe`,
`out_from_reg_before_commit`, `vr_sink_mealy`, `vr_skid` — demonstrations of a two-job hazard,
**deleted** (unexpressible under D-8; `vr_sink_moore` / `q1_b_vr_sink` and
`out_from_reg_after_commit` are their agreeing spellings), with their `SKIP` rows (D-13: 4 of 7).

**(C) Hand-written tests** with inline plain-`Out` modules (excluding the battery): 33 modules in
13 root `tests/*.rs` files, 33 in 5 crate test files (`analysis_semantics.rs` 16,
`unsupported_constructs.rs` 8, `structural_hierarchy.rs` 5, `branch_local_defaults.rs` 2,
`literal_operand_width.rs` 1, `interp_smoke.rs` 1), plus every test that wires a rewritten
fixture (its `wire` becomes `registered_wire`). Predicted: each passes with the module
rewritten; the three frozen `golden_traces` (`counter`, `det_010`, `mac_pipeline`) unchanged.

**(D) The battery** `tests/sequential_forwarding_divergence.rs`: 167 tests, 318 module
definitions (each module twice, compiled and as a source string). Predicted: every test whose
modules include a plain `Out` is deleted (unexpressible); tests over `RegOut`-only modules stay.

**(E) Rule machinery** (D-13 sizing, II.3.2): ui/fail 18 of 39 for rules 8–15 (+1 for rule 7);
ui/pass `pretick_alignment_ok`; `pretick_alignment_corpus.rs` (83 entries) and
`hold_form_audit.rs` (20 entries) whole; `front_end_parity.rs` test 2; `cfg.rs` ≈109 of 165
unit tests by prefix; 4 of 7 `SKIP` rows. ui/fail cases for other rules that use a plain `Out`
incidentally (`cdc_crossing`, `clock_construction`, `foreign_await`, `function_typed_*`,
`multiple_top_level_loops`, `param_shadowing`, `partial_tick_path`,
`sequential_unprotectable_read`, `tickless_loop`) are re-spelled with `RegOut` so they still fail
for their own reason. Predicted: deleted counts within D-13's sizing.

**(F) After the flip** (predicted): G-G differential 0 DISAGREE, 0 EXCUSED (the class deleted),
reference leg unchanged; `regout_trailing_single_tick` pinned sim=ref≠SV (its `REF_SKIP` row
re-worded); the three sim=ref≠SV pins (f2)/(f3)/(f4) unchanged; `b_reg`'s never-ticked `Two`
pin deleted; G-H merged into G-G.

### G.1 (i) The anchored rewrites — predicted, then measured

Every row was run under `COPPER_RESUMPTION=two` (the default at that commit) and `=one`.

| module(s) | reference | predicted | measured |
|---|---|---|---|
| `sync_2ff`, `sync_2ff_concrete` (tick-last; `ff2` a wire) | `sync_2ff_ref.sv` (`independent_verilog_matches_sim`), transpiled twin | identical | **identical, both modes** (10/10 tests) |
| same, G2 `NameExact` | `{ff1, ff2}` | FAIL (`{ff1}`) | **FAIL as predicted**: `left: {"ff1"}, right: {"ff1", "ff2"}`. Replaced by a storage check: inferred `{ff1}` + the `RegOut` `q` = 2 = the reference's flop count |
| `sync_1ff` (negative control) | 2-FF trace | one cycle faster | **as predicted** |
| `fast_counter`, `flag_sync`, `slow_consumer` (combinational) | `two_domain_hierarchy.sv`; transpiled `two_domain_top` under P, Q, R | identical; F.3's `One` lags become equalities | **identical, both modes, all three interleavings**; `two_clock_pins` (vi) now asserts `One` = SV |
| `two_domain_counter`, `flag_crossing` (+ behavioural tests) | latency assertions | unchanged | **pass unchanged, both modes** |
| `det_010` | `pattern_detector_010.sv` | identical | **identical, both modes**; G2 `StorageEquivalent` unchanged; frozen golden trace unchanged |
| `bsg_dff_en`, `bsg_counter_up_down`, `sipo_block` | BaseJump | identical | **identical, both modes** (examples' self-checks and `*_equivalence`) |
| `lfsr`, `shift_register`, `det_110101` | `lfsr.sv`, `shift_register.sv`, `pattern_detector.sv` | identical | **identical, both modes** (the lfsr initial-value text check had to admit `o = 32'd0;` in the same `initial` block) |
| `counter`, `mac_pipeline` fixtures | frozen golden traces | unchanged | **unchanged, both modes**, after placing the write where the old post-edge write showed (counter: fold then publish; mac: register the sum at the end of stage 2) |
| `dff`, `dff_after`, `enff`, `counter` (timing probes) | `timing_probe_sv/` | identical | dff/dff_after/enff **identical**; `counter` matches `counter_comb.sv`, not `counter_reg.sv`, **as before the rewrite** (the probe's own verdict); `dff_after`'s Verilator leg cannot run (its harness names top `dff_after`, the file defines `dff` — pre-existing) |
| `stage_add_one`, `stage_double`, `counter_by` | sim-only assertions (`module_composition_hybrid`) | unchanged | **pass unchanged, both modes** |
| **`ram_straddle`, `ram_prewrite` (`Memory`)** | `timing_probe_sv/ram1.sv` (`dob <= ram[a]`) | **one cycle late** (I.10.1 (d)) | **one cycle late, both spellings, both modes**: sim `[0,105,106,107,108,109]` vs reference `[105,106,…]`. **STOPPED** — see G.2 |

Two transpiler/interpreter findings the rewrites exposed:
- **Write-driven width inference ignored `RegOut`** (`chir_lower::build_write_inferred_types`
  accepted only `Out<`): `shift_register`'s `Bits::x()` and `bsg_counter_up_down`'s
  `Bits::<PTR_W>::zero()` lost their widths ("cannot infer bit width") the moment the port
  kind changed. Fixed (one predicate); no plain-`Out` module's emission changes.
- **The reference leg now reaches every module that becomes `RegOut`-only** (G-F 46 → 61
  after part 1). Three interpreter gaps it found, each a `REF_SKIP` row with its sentence:
  `shift_register`'s `const { assert!(…) }` (statement macro), `struct_latch`/
  `struct_latch_cond`/`tuple_destructure`'s `Self { .. }` in an associated fn, and
  `rv32i_cpu_transpilable`'s `Memory`.

**Part 1 regression** (`d4483c0`, `--no-examples`, laptop under load): PARTIAL OK, 1444
passed. G-G differential 1151 PASS / 108 EXCUSED / 0 DISAGREE (E.7: 1148 / 131); reference
511 PASS (385). G-H 56 of 159 cases change under `One` (F.6: 66), 377 DISAGREE (428).

### G.2 Stopped: a `Memory` read cannot drive a port at the edge that delivers it

Under one job, a resumption that ends at edge N+1 observes what edge N delivered
(I.10.1 (d)), and a `RegOut` write shows from obs_{N+1}. So `dob <= ram[addr]` — a
latency-1 synchronous read whose output register *is* the port — has no `Memory`
spelling: the word delivered at edge N can be written to a `RegOut` only in the next
resumption, which shows it one observation later. Measured on `ram1.sv` above. This
reaches the two anchored memory modules (`dual_port_ram` vs `dual_port_ram.sv`,
`ram_read_first` vs its `REFERENCE` row) and every swept module whose plain `Out` shows a
`Memory` result (`pipelined_ram_dut` ×3, `preloaded_rom_dut` ×2, `multiphase_rom_dut` ×2,
`mem_address_width_dut` ×3, `write_first_ram_dut` ×3). They are left in their two-job
spelling pending a decision (question sent to the coordinator 2026-11-06): (a) stop; (b)
re-spell the two anchors with an array register (`dob.write(ram[addr])`, which is
`dob <= ram[a]`) and accept +1 on the unanchored ones; (c) accept +1 everywhere and
re-anchor. Until it is decided, R8 and the flip cannot land (each of these modules would be
refused).

**Part 2 regression** (`a6ccc98`, `--no-examples`): PARTIAL OK (446 s), 1471 passed, 0
failed, 89 ignored. G-D 160 differential cases (3 ignored-with-reason: the `SKIP` table is
down to `rv32i_cpu_pipelined`, `two_domain_top`, `mod8`). G-F 87 modules through the
interpreter (46 at E.7). G-G differential **1226 PASS / 0 EXCUSED / 0 DISAGREE** — the
`obs_0` plain-`Out` excuse class is already **empty** (predicted to empty at the flip; it
emptied with the rewrites, because every member was rewritten); reference 712 PASS. G-H:
**15 of 160** cases still change under `One` (F.6: 66) — exactly the 14 `Memory`-output
modules of G.2 and P12 (`regout_trailing_single_tick`), 186 sequences, all `sim≠SV,no-ref`.

### G.3 Decided: option (b) (user, 2026-11-06), and the Memory rewrites measured

The user chose (b) in this session. A coordinator message relayed "Spec packet first" (a
D-21 packet to give I.10 a no-extra-cycle output form); the user, asked to resolve the
conflict, answered "Keep option (b)". Part 4 (`de9e84e`):
- **Anchored, re-spelled as array registers, measured identical in both modes:**
  `dual_port_ram` (its self-check against `dual_port_ram.sv`), `ram_read_first` (the
  sweep's `REFERENCE` comparison). Predicted identical; measured identical.
- **Unanchored, Memory kept, +1 cycle:** 13 modules. Predicted: each hand model is the
  two-job model delayed one observation, and sim ≡ SV holds. Measured exactly that in every
  hand test (`pipelined_memory`, `preloaded_memory`, `write_first_memory`,
  `multiphase_memory`, `received_memory_abi`, the m3 derivation), both modes.
- New `ram_read_first_mem` keeps the `Memory` ReadFirst lowering swept and shape-pinned.
- Part 4 regression (`--no-examples`): G-G differential 1240 PASS / 0 DISAGREE, reference
  744 PASS; under `One` 1 of 161 cases (P12) differs.

### G.4 R8 and the rule retirement (`9e41b61`) — deletions against D-13's sizing

| item | D-13 sized | deleted |
|---|---|---|
| ui/fail cases of rules 7–15 | 19 of 39 | **19** (39 → 20), + `ui/pass/pretick_alignment_ok` |
| `pretick_alignment_corpus.rs` | whole file, 83 entries | whole file |
| `hold_form_audit.rs` | whole file, 20 entries | whole file |
| `front_end_parity.rs` test 2 | 1 | 1 (replaced by the opt-out-absence audit; test 1 runs R8) |
| `cfg.rs` unit tests | ≈109 of 165 | **114** (by the retired methods and their 8 test helpers) |
| battery `sequential_forwarding_divergence.rs` | 167 tests; 62 refused unexpressible, `RegOut` rows kept | **166 of 167**: 159 of its 160 modules had a plain `Out`; the one `RegOut`-only module's test stays |
| `SKIP` rows | 4 of 7 | 4 (in part 2) |
| `analysis_semantics.rs` | — | 8 (`multi_write_*`, rule 8) |
| unsupported_constructs / multiphase_memory | — | 1 each (rule 7's plain-`Out` refusals) |

R8 is one function in `copper-analysis` (`check_no_plain_out`, gated by the attribute in
`check_clocked_outputs`) with one message, called by both front-ends (the macro's input fn
no longer carries its own `#[hardware]` attribute, so the macro calls the ungated core — the
first spelling of the macro call never fired, caught by the ui case). Witnesses:
`ui/fail/plain_out_on_sequential.rs`, `plain_out_on_synchronizer.rs`,
`unsupported_constructs::a_plain_out_on_a_clocked_module_is_refused_by_r8`;
`ui/fail/retired_pretick_opt_out.rs` refuses the retired flag. Re-spelled ui cases kept their
error lines byte-identical (columns only). **Tree scan: plain-`Out` clocked module definitions
697 → 3**, the three being R8's two ui witnesses and copper-ref's refusal test.
**Not deleted:** the plain-`Out` emission forms F1–F5 in `vlir_lower` (now unreachable from a
clocked module; the combinational path shares code with them) — owed to a lowering clean-up.

### G.5 The flip (`3a8d616`) — predictions for the bare regression (written before it ran)

- REGRESSION OK.
- G-G differential: 0 DISAGREE, 0 EXCUSED (the class deleted); reference leg all PASS.
- Pins: four, all `sim=ref≠SV`: `preloop_probe`, `preloop_linear`, `trailing_fold` (P15,
  unchanged) and `regout_trailing_single_tick` (P12, now a `SKIP` row, II.3 (f4)); `b_reg`'s
  never-ticked `Two` pin gone.
- The pipelined CPU's 13 programs: unchanged (no plain `Out`; its body is unaffected by the
  barrier's removal — predicted, not argued: measured below).
- Examples: every self-check passes.

### G.6 (v) The bare regression — measured

Bare `tools/regression.sh` on `a5bd6bd` (the flip; laptop): **REGRESSION OK (233 s)**. The
first bare run, on the flip as first committed, failed to BUILD: `cargo fix` had removed
`HardwareTest` from `examples/basejump/sipo_block.rs`, used only by its `#[cfg(not(test))]`
`main` (restored, the commit amended). Counts:
- 1174 passed, 0 failed, 89 ignored, 140 test binaries; G-C 114 test files; examples 32/32.
- G-D 160 differential cases (4 ignored-with-reason: `SKIP` = `regout_trailing_single_tick`,
  `rv32i_cpu_pipelined`, `two_domain_top`, `mod8`).
- G-F 89 modules through the interpreter (46 at E.7), 75 skipped with a sentence.
- **G-G differential 1234 PASS / 0 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0 DISAGREE; reference
  744 PASS** — predicted 0 DISAGREE and 0 EXCUSED: **as predicted**.
- **Pins: 4, all `sim=ref≠SV`** (`preloop_linear`, `preloop_probe`, `trailing_fold`,
  `regout_trailing_single_tick`) — **as predicted**; `b_reg`'s `Two` pin gone.
- `rv32i_pipelined_verilator` (the 13 programs): PASS — **as predicted** (unaffected).
- G-E 5 anchored, 159 transpiler-only.

### G.7 What the SPEC must say that it does not (for the coordinator; SPEC files untouched)

1. **A `Memory` read cannot drive a port at the edge that delivers it** (G.2). Under the user's
   option (b) the two anchors are array registers; P23's anchor (`dual_port_ram`) now anchors
   an array register, not `Memory` — III.1's evidence column for P23 should say so, and the
   D-21 packet (running) may give `Memory` the form.
2. **G2's register-name convention** (output ports excluded on both sides) no longer fits
   D-8, where a clocked output IS a flop: `sync_2ff`'s second flop is the `RegOut` `q`. The
   anchor's G2 check is now a storage count (inferred registers + `RegOut` flops = the
   reference's flops).
3. **Interpreter front-end gaps** reached once modules became `RegOut`-only: `const { assert!
   }` blocks, `Self { .. }` struct literals in associated fns (REF_SKIP rows).
4. **Hand-built untagged futures** (F.8 item 2) remain in the executor's own unit tests for the
   combinational passthroughs (network tasks, correct) — the clocked one is now tagged.
5. **The truncation instant moves one resumption** under the flip (an undelivered `data()` after
   a tick is caught at resumption N+1, not N) — III.2.4's worked example should use the
   one-resumption count.
6. **`classify_reads`** (the deferred/immediate read classification) has no consumer left in
   either front-end; the inventory should mark it retired with the injection.

## H. Phase C step 4 — the transpiler's one port form, the trailing fold removed (2026-11-06)

Branch `claude/phase-c-transpiler-257dc5` (from `paper` at `663bbe2`). D-20 obligation (4);
SPEC I.0.7, I.6.9 (the transpiler's obligations), I.11 (what a combinational module keeps).
Every prediction in H.0 was written here before the emission it predicts was changed. The
instrument is `sv-baseline` (`copper-codegen/src/bin/sv-baseline.rs`), snapshot taken on
`663bbe2`: **224 top-level modules** in `examples/`, `tests/`, `src/` (131 clocked, 4 refused
sentinels; inline source strings in tests are not in it).

### H.0 Inventory and predictions (written before any code change)

**What exists.** After step 3 every clocked module's data outputs are `RegOut` (R8), so in
`vlir_lower::lower_seq` `hold_outs` = the registered ports and every drive reaches
`split_output_regs` (F3, the edge-sampled flop). Unreachable from a clocked module, and
deleted: F1 for a clocked body (a top-level drive to a non-hold port → module-level `assign`;
the seq body's `output_assigns` and its `MultiplyDrivenOutput` check), F2's sequential pieces
(the single-phase `nested_port_drives` comb routing, `every_path_outs`, and the Moore hoist
`hoist_moore_output_defaults`, which *is* reached — see below), F4 (`opening_hold_outputs`,
`DriveMix`/`port_drive_mix`, `strip_closing_drives`, `shadow_hold_ff`, `hold_register_name`,
the `<port>_hold` registers, `comb_defaults`, `VLIRHoldFormPort`/`hold_form_ports`), F5
(`phase_scoped_output_ports`); in `shir_lower` the opening/closing tagging (`seen_in_read`,
`forward_opening_drives`, `PortDrive::opening`) and `PortDrive`'s two value forms (one value,
the forwarded one: `at_edge` is the identity on the combinational path); the trailing fold
(`trailing_reg_outs`, the trailing segment's register updates folded into phase `n-1`).
Kept for the combinational path: `lower_flat_stmts` (its continuous-`assign` arm is F1 for a
combinational module), `nested_port_drives`, `hoist_branch_local_defaults`, `check_no_latches`.

**Probe, the trailing segment** (a temporary `eprintln!` in `lower_to_shir`, reverted):
20 of the 131 clocked modules have a non-empty segment after their last tick, all on the
linear route (no extracted module has one — extraction ends every path at a tick). 16 are
single-tick, 2 multi-tick (`trailing_out`, `trailing_local_into_head`), counting the probes
`trailing_fold`, `trunc_data`, `trunc_oob`.

**Probe, the same-edge memory binding** (`mem_binding`'s combinational arm, a register
update in the phase that stages the read): used by exactly 12 modules, all single-tick with
the `q = data()` update in the trailing segment (`mem_user`, `narrow_index_into_narrow_addr`,
`ram_r1w2_wf`, `ram_r2w2`, `ram_r3w1`, `ram_read_first_mem`, `ram_wf_priority`,
`ram_write_first`, `rom_from_contents`, `rom_from_fn`, `wide_index_into_narrow_addr`,
`wide_index_sole_consumer`). Its only consumer is the fold: a folded trailing update latches
one edge early, so it must read what the edge is about to deliver. Under D-8 every `data()` /
`is_ready()` reads what the last edge delivered (I.10.1(d)), the registered net — so the arm
is deleted with the fold.

**Probe, the Moore hoist on a clocked body:** it fires in exactly one module, `seq6`
(`b0`/`b1`/`b2` over a 3-bit state with 6 states).

**The new lowering of the trailing region** (SPEC I.0.2: the resumption that ends at edge
`N+1` runs the trailing region `T`, then the head `H`, to the head's tick): `T` moves into
phase 0 *ahead of* `H` with one forwarding map in that order (its register updates no
longer go to phase `n-1`), every drive takes the forwarded value (no committed-register
special case), and `T` is guarded by a one-bit register `started_r` (power-on 0, set at
edge 1), because the first resumption runs `pre; H` only. A module without a trailing segment
gets no `started_r`.

**Predicted emission diff (sv-baseline):**
- (P-a) The 20 trailing modules CHANGE: a `started_r` register (`initial` 0, `started_r <= 1'b1`
  every edge), `T`'s register updates and drives under `if (started_r)` in phase 0; in the 12
  memory modules `T`'s reads take the registered net (`<m>_rd0_q0`/`_v0`) instead of
  `<m>_rd0_data`/`_en`, and the port drive reads the forwarded update.
- (P-b) `seq6` CHANGES: the three top `bK <= 1'b0` lines before the state `case` go (the
  `case`'s empty default stays; its encodings 6 and 7 are unreachable).
- (P-c) Nothing else changes: the 4 `REFUSED` sentinels unchanged, every combinational and
  structural module **byte-identical** (checked separately below), every other clocked module
  byte-identical (no hold form is reachable; `opening`/`value` were read only by the deleted
  forms — except `merge_trailing_before_head`'s use sets, which see only trailing modules).

**Predicted traces:**
- (T-a) `regout_trailing_single_tick` (P12): sim = ref = SV, obs_{N+1} = Σ x_2..x_{N+1}; its
  `SKIP` row is deleted and it sweeps; `regout_trailing_single_tick_is_the_trailing_fold`'s
  pin flips to an agreement assertion.
- (T-b) `trailing_fold` (II.3 row (f4), D-11 (iv)): **flips to agree** — the pin's divergence
  at obs_1 was the fold executing `T` at edge 1, which `started_r` removes. (f4) is listed under
  D-20 step (5) as part of the entry of I.9.2, but its mechanism is this step's fold.
- (T-c) `preloop_probe` (f2) and `preloop_linear` (f3): **unchanged, still sim = ref ≠ SV** —
  the pre-loop statements (extraction re-runs them, the linear route drops them) are I.9.2's
  entry, step (5); this step adds no pre-loop lowering.
- (T-d) The other 18 trailing modules: port traces **identical** (the fold was an encoding of
  `T` shifted by one register state wherever `T` reads no input; the 12 memory modules shift
  their internal `q` by one edge and their port not at all; `T`'s edge-1 effects in the fold
  were writes of power-on zeros).
- (T-e) `seq6`: identical (the removed default covers unreachable encodings only).
- (T-f) Every anchor unchanged: BaseJump (`bsg_dff_en`, `bsg_counter_up_down`, `sipo_block`),
  `sync_2ff_ref.sv`, the goldens (`det_010`, `det_110101`, `lfsr`, `shift_register`, the three
  frozen traces), `dual_port_ram`, the `REFERENCE` rows, the two-clock pins, the pipelined CPU's
  13 programs — none has a trailing segment or reaches the Moore hoist, so their SV is
  predicted byte-identical, not merely trace-identical.
- (T-g) G-G 0 DISAGREE; one more swept case (161 differential, 3 ignored); the reference leg
  count grows by `regout_trailing_single_tick`'s sequences.

### H.1 (a) The plain-`Out` forms and the drive tagging deleted (`4b51ec6`) — measured

sv-baseline against `663bbe2`: **1 of 224 modules changed — `seq6`**, the three
`bK <= 1'b0` lines over the state `case` — **as predicted (P-b, P-c)**. Every one of the 68
combinational and structural modules in the corpus is byte-identical, as is every other
clocked module. `seq6`'s differential case and `fsm_six_state_equivalence` pass (T-e).

What the deletion found: `transpile_item_fn` (the unit tests' entry point) runs none of the
front-end rules, so 8 `vlir_lower` unit tests had been feeding `lower_seq` a plain-`Out`
clocked body. A `debug_assert` caught it; the lowering now returns
`VLIRLowerError::PlainOutOnClockedBody` (a release build would otherwise have dropped the
drive silently). The tests that test something else were re-spelled `RegOut`; the one that
tested F1's two-`assign` conflict was deleted.

### H.2 (b) The trailing fold removed — predicted, then measured

sv-baseline against `663bbe2` after (b): **21 of 224 changed — exactly the 20 trailing
modules and `seq6`; nothing else (P-a, P-c as predicted).** The emitted forms are the
predicted ones, with one design change made after the first measurement: the guard is
emitted as STRUCTURE, `if (__copper_started) begin … end` in `always_ff` (the trailing
region's register updates in `SHIRPhase::guarded_post_edge`), not as a `started ? v : r`
mux on every update. The mux spelling (measured first) was correct but hid a trailing
`match` next-state from `lower_reg_updates`' `case` lifting, so an FSM's next state came out
as nested ternaries (`enum_state_machine_golden_output`). The forwarded value segment 0
reads through the guard is still the mux (one walk over `if started { T }`); the emitted
updates come from a second walk over `T`, by the same function.

Two things the prediction did not name:
- **A head wire read after the tick is now promoted** (`find_promoted_wires`): it crosses an
  edge even in a single-tick body. The promotion test compared PHASES with the trailing
  segment mapped to the last phase; it now compares segments. Measured by diffing the `_r`
  declarations of the 224 modules: one gains a promoted register, `trailing_fold`, whose
  pre-loop `let r = 5` CHIR carries as a segment-0 wire — now `r_r`, captured at edge 1 and
  read by the guarded drive from edge 2 (correct; one register more than the constant needs —
  pre-loop statements are step 5's entry). Also `shir_lower::tests::test_promoted_wire_renamed_in_later_phase`
  (rewritten: `step1` is promoted and read as `step1_r` from phase 0).
- **The 12 memory modules' read registers**: a module that only used the combinational read
  net now declares `<m>_rd0_q0`/`_v0` capture registers (`ram_read_first_mem`:
  `data <= (mem_rd0_v0 ? mem_rd0_q0 : q)` under the guard, from `data <= q` and
  `q <= (mem_rd0_en ? mem_rd0_data : q)`).

Traces:
- `regout_trailing_single_tick` (T-a): swept, its `SKIP` row deleted — **sim = ref = SV**,
  864 cycles on 6 sequences, observation 0 agreed on 6 of 6.
  `regout_trailing_single_tick_agrees_on_three_legs` (was `…_is_the_trailing_fold`) asserts
  the Σ x_2..x_{N+1} trace and agreement. **As predicted.**
- `trailing_fold` (T-b): **agrees** (`trailing_fold_at_edge_1_agrees_on_three_legs`, was
  `…_is_spec_vs_transpiler`), obs_1 = 0 on all three legs. **As predicted** — (f4) is closed by
  this step, not step 5.
- `preloop_probe` (f2), `preloop_linear` (f3) (T-c): **still sim = ref ≠ SV**, pinned. **As
  predicted** — they are step 5's.
- The other 18 trailing modules and `seq6` (T-d, T-e): their differential cases pass on every
  sequence (`corpus_generated`, 21 cases run, 13 reference-leg cases `#[ignore]`d by their
  existing `REF_SKIP` rows — memory). **As predicted.**

### H.3 (c) `classify_reads` deleted (`21db6a5`)

No consumer in either front-end since step 3 (the macro's barrier injection was its only
user). The remaining callers were its own 10 tests and the `analysis-time` bin, which times
"the macro's checks in the macro's order" — it now matches the macro again, so
`tools/stats/analysis.py`'s per-module analysis time drops by `classify_reads`'s share the
next time it is regenerated (not re-measured here). `in_param_names` stays (`Cfg::build`).

### H.4 Deletions, against the step's brief

| item | where | deleted |
|---|---|---|
| F1 for a clocked body (continuous `assign`, `MultiplyDrivenOutput`) | `vlir_lower`, `VLIRSeqBody::output_assigns` | yes; the combinational arm of `lower_flat_stmts` kept |
| F2's clocked pieces (every-path routing, Moore hoist) | `vlir_lower` | yes (`nested_port_drives` kept: combinational) |
| F4 mux + `<port>_hold`, `comb_defaults`, `hold_form_ports` | `vlir_lower`, `copper-core::vlir`, `copper_codegen::hold_form_ports` | yes (`comb_defaults` was clocked-only) |
| F5 phase-scoped flop | `vlir_lower::phase_scoped_output_ports` | yes |
| opening/closing tagging, `seen_in_read` | `shir_lower::Forwarding`, `PortDrive::opening`, `PortAssign::opening` | yes |
| the second drive form (`edge_value`) | `SHIRStmt::PortDrive`, `VLIRStmt::PortAssign` | yes — one value |
| the trailing fold, `trailing_reg_outs` | `shir_lower` | yes; plus `merge_trailing_before_head`, `lower_pre_edge_stmts`, `phase_for_segment` |
| the same-edge memory binding | `vlir_lower::mem_binding`, `MemPortUse` | yes (found by H.0's probe; the fold's dependent) |
| `classify_reads` | `copper-analysis` | yes, with `ReadTiming` and 6 private helpers |
| `phase_sensitive_checks.rs` entries for the removed forms | — | none existed (step 3 had pruned them); `vlir_lower::lower_seq` stays listed |
| pins of the removed forms | `register_reconciliation.rs` `<port>_hold` allowance | yes |

Source lines in `copper-codegen/src`, `copper-core/src`, `copper-analysis/src`:
+413 / −2203.

### H.6 The bare regression — measured

Bare `tools/regression.sh` on `23e8283` (laptop): **REGRESSION OK (254 s)**.
- 1164 passed, 0 failed, 87 ignored, 140 test binaries; G-C 114 test files; examples 32/32
  (every BaseJump self-check, `dual_port_ram`, both CPUs, the two-domain and CDC examples).
- G-D **161** differential cases (160 at G.6), 3 ignored-with-reason (`SKIP` =
  `rv32i_cpu_pipelined`, `two_domain_top`, `mod8`; `regout_trailing_single_tick`'s row gone).
- G-F 90 modules through the interpreter (89 at G.6), 74 skipped with a sentence.
- **G-G differential 1240 PASS / 0 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0 DISAGREE; reference
  750 PASS** (1234 / 744 at G.6: +6 sequences each, P12's six) — predicted (T-g): **as predicted**.
- **Pinned disagreements: 2** (4 at G.6), `preloop_linear` and `preloop_probe`, both
  `sim=ref≠SV` — (f4) `trailing_fold` and P12 left the list, **as predicted** (T-b, T-a, T-c).
- `rv32i_pipelined_verilator` (the 13 programs) passes; `cdc_synchronizer_anchor`,
  `det_010_independent_golden`, `two_clock_pins`, the golden traces and the `REFERENCE` rows
  pass — every anchor's SV is byte-identical to `663bbe2` (none is among the 21 changed
  modules), so its trace cannot have moved (T-f).
- G-E 5 anchored, 159 transpiler-only (unchanged).
- New `SKIP` / `REF_SKIP` rows: **none**; one `SKIP` row deleted.

**Misses:** none in the emitted set or the traces. Not predicted: the guard's mux spelling
hiding `case` lifting (changed to structure before committing, H.2), the promotion of
`trailing_fold`'s pre-loop `let` (H.2), and the 8 unit tests feeding `lower_seq` plain-`Out`
IR through `transpile_item_fn` (H.1).

### H.5 For the coordinator — SPEC items (SPEC files untouched)

1. **I.6.9's implementation map is stale** (`SPEC.md:827-853`): `trailing_reg_outs`, the
   committed-value trailing drive and the "trailing region's register updates at edge N" are
   gone; the trailing region lowers into phase 0 under `__copper_started` with every drive's
   value the forwarded one. The line numbers it cites for `split_output_regs` moved.
2. **I.0.7's transpiler obligations** ("the transpiler's single port form … with the five
   plain-`Out` forms and the opening/closing tagging removed") are met; `classify_reads` is
   removed (its entry in I.0.7's "Today" paragraph and the `cfg.rs:3804` pointer are stale).
3. **II.3 row (f4)** (`SPEC.md:3424`, "the trailing region folded before the head at edge 1")
   is CLOSED by this step, not by the I.9.2 entry: its fix direction named the entry, but the
   mechanism was the fold. D-20's step (5) list should drop (f4); (f2), (f3) and P15 stay.
4. **III.1 P12** (`SPEC.md:3560`): "false today on a read after the last tick" is now true —
   `regout_trailing_single_tick` sim = ref = SV, swept. III.2's disagreement table
   (`SPEC.md:3715`, "sim = SV ≠ ref … `regout_trailing_single_tick`") has no instance left.
5. **I.10.1(d) in the emitted text**: every `data()`/`is_ready()` binds the registered
   capture; the combinational binding existed only for the fold. Worth a sentence in I.10's
   implementation map.
6. **The trailing region's guard is a realization of I.0.2's first resumption** ("the first
   resumption runs `s_pre` and the head only") for the trailing region only. Step 5's entry
   (pre-loop statements, (f2)/(f3)) can reuse `__copper_started` as its run-once bit — it is
   0 exactly during the first resumption.
7. **A pre-loop non-`mut` `let` is a segment-0 wire in CHIR** (`trailing_fold`'s `r`), so a
   trailing read of it is promoted to a register (`r_r`) rather than folded to a constant —
   correct, one register more. Belongs with step 5's pre-loop handling.

## J. Phase C step 5 — the II.3.6 fix list (2026-11-06)

Branch `claude/phase-c-fixlist-0c597d` (from `paper` at `80ae993`). D-20 obligation (5); SPEC
II.3.3 rows, II.3.4 (R21), II.3.6. Every prediction below was written here before the code it
predicts was changed. Instrument: `sv-baseline`, snapshot of the untouched tree at `80ae993`
(224 top-level modules), re-snapshotted after each row so each row's diff is its own.

### J.1 Row 1 — the entry of I.9.2: (f2), (f3), P15

**Inventory (a temporary `eprintln!` at both pre-loop sites, reverted).** Pre-loop expression
statements reach the transpiler in exactly 5 of 224 modules: extraction route —
`preloop_probe` (1 statement), `rv32i_cpu_pipelined` (4: three port writes, the first fetch
staging); linear route — `preloop_linear` (1), `rv32i_cpu_transpilable` (1: the first fetch
staging), `shift_register` (its `const { assert!(..) }`, a compile-time item that lowers to
nothing, in two copies: the example and the fixture).

**The form, decided on the evidence.** Two candidate entries (I.9.2): an entry value of
`pc`/`phase_r` whose arm is `s_pre` and a clone of the head, or a one-bit started carrier
(I.7.2(j)). The linear route has no `pc` and, single-tick, no `phase_r`, so the entry state
would need a state register the route does not have plus a clone of the head on both routes;
step 4's `__copper_started` is 0 during exactly the first resumption on both routes (each
reaches `shir_lower::lower_seq_body` as phases, the extraction route as one phase whose body
is `match pc`). **Chosen: the started carrier.** CHIR lowers the pre-loop expression
statements (both routes — `control_extract` no longer moves them into state 0) into
segment 0, after the pre-loop wires and ahead of the head, as `if (!__copper_started) { s }`;
`shir_lower` declares `__copper_started` whenever the trailing region or the entry guard
reads it. The first resumption runs `s_pre ; H`, every later one `T ; H` (I.0.2) — `T` and
`s` are exclusive by the one bit. A pre-loop `tick` (R21) is refused by this lowering until
row 9 puts the rule in both front-ends.

**Predicted emission diff (sv-baseline):** exactly 4 modules change —
- `preloop_probe`, `preloop_linear`: gain `__copper_started` (`initial` 0, set every phase-0
  edge) and `if (!__copper_started) q <= 8'd7;` ahead of the head's drive; `preloop_probe`'s
  state 0 no longer contains the write.
- `rv32i_cpu_pipelined`: gains `__copper_started`; the three port writes and the fetch staging
  leave state 0's arm and precede the `case (pc)` under `!__copper_started`.
- `rv32i_cpu_transpilable`: gains `__copper_started` and a guarded first-fetch staging
  `memory` read-port address/enable ahead of the head's.
- `shift_register` (both copies) and every other module **byte-identical**.

**Predicted traces:** `preloop_probe`, `preloop_linear` flip to **sim = ref = SV** (the pin in
`tests/reference_leg_measurements.rs` becomes an agreement assertion; P15 holds on both
routes); both CPUs trace-identical — the pipelined CPU's 13 programs (anchored) and the
transpilable CPU's examples pass, because the head writes the three ports on every path and
restages the fetch in the same resumption (masked, I.9.3). The probes become sweep fixtures
(`tests/fixtures/preloop_entry_dut.rs`), so G-D grows by 2 cases and G-G by their sequences.

**Measured.** sv-baseline against `80ae993`: **4 of 224 changed — `preloop_probe`,
`preloop_linear`, `rv32i_cpu_pipelined`, `rv32i_cpu_transpilable`; `shift_register` and every
other module byte-identical. As predicted.** The emitted forms are the predicted ones
(`logic __copper_started;`, `initial … = 1'b0`, `if ((!__copper_started)) begin q <= 8'd7; end`
ahead of the head, `__copper_started <= 1'b1;` at phase 0's edge). One detail the prediction
did not name: in `rv32i_cpu_pipelined` the old emission also carried the four pre-loop
statements **at the end of state 1** (the back edge's zero-time goto to state 0 spliced state
0's body, pre-loop statements included — the (f2) re-run, masked because the head had just
written the same forwarded values); they are gone with state 0's copy. A first run refused all
four modules (`validate` did not know the carrier name) — fixed before measuring.

Traces: `preloop_probe`, `preloop_linear` **sim = ref = SV** — as a sweep fixture
(`tests/fixtures/preloop_entry_dut.rs`, both legs, every sequence) and in
`tests/reference_leg_measurements.rs::preloop_statement_agrees_on_both_routes` (was
`…_is_spec_vs_transpiler_on_both_routes`, the pin removed). `rv32i_pipelined_verilator` (the 13
programs) passes; both CPU examples 13/13; both CPU sweep cases pass. **As predicted.**
`tools/regression.sh --no-examples`: PARTIAL OK (434 s); G-D **163** (+2); G-F 92 (+2); **G-G
differential 1252 PASS / 0 DISAGREE, reference 762 PASS; pinned disagreements 0** (2 before).
No new `SKIP`/`REF_SKIP` row. P15 holds on both routes (it is the entry the two probes test).

### J.2 Row 2 — (f1) the hoisted-tick guard

**Inventory (a temporary `eprintln!` in `expand_counted_for`, reverted).** 31 tick-bearing
counted `for`s in 13 modules reach the desugaring; every one starts at a literal (`0`). Those
with two literal bounds (`0..2`, `0..3`, `0..4`, `0..8`) are non-empty by construction and
keep today's unconditional hoisted tick. The rest have a bound the pass cannot fold — `det_for`
(`0..N`, with an author's `break`; two copies: the example `pattern_detector_for` and the
fixture), and the UART receivers/transmitter `rx`, `uart_rx`, `uart_rx_dut`, `uart_tx`
(`0..CLKS_PER_BIT`, `0..CLKS_PER_HALF_BIT`: named consts this pass has no environment for).

**The form.** The fix direction of (f1): the hoisted tick stands for the last iteration's
tick, so it is taken only if the counter advanced — `var != start` (no break), or `var >= end
&& var != start` (an author's `break`, extending the 2026-09-02 guard). Only for a range not
statically non-empty. `start` is re-evaluated at the guard, which is exact for a literal (the
whole corpus); a non-literal start is snapshotted at entry into a synthesized `let` (a
cross-state wire the extraction route hoists to a register), so a body that changes what
`start` reads cannot fool the guard.

**Predicted emission diff:** exactly 6 modules change — `det_for` ×2, `rx`, `uart_rx`,
`uart_rx_dut`, `uart_tx`: each post-loop transition through a hoisted tick becomes
conditional on the counter (`ctr != 0` / `i >= N && i != 0`) with the zero-time
continuation inlined on the other arm (more states or larger arms). Everything else
byte-identical. **Predicted traces:** all six unchanged (every bound is > 0 in every test, so
the guard is always true where the loop is left); no anchor among them. The new fixture
`tests/fixtures/for_empty_dut.rs` — D-10's probe (ii) `for_empty` (`for _ in 0..cnt`, `cnt`
an input, 0 on some cycles) and `for_empty_start` (a non-literal start, `for _ in s..e`) —
predicted **sim = ref = SV** on every sequence; before the fix `for_empty` would diverge on
every zero-count pass (D-10: cycles 0, 15, 16 of 24).

**Measured — a miss first.** The first measurement (guard on every range not two literals)
changed the predicted 6 modules, but **4 of the 6 came out REFUSED**, not changed: in `rx`,
`uart_rx`, `uart_rx_dut`, `uart_tx` an inner `for _ in 0..CLKS_PER_BIT { tick }` is the last
statement of an enclosing `for _ in 0..8`, and the enclosing expansion needs an unconditional
last tick — the guarded one is not. (My first check of the diff read the *old* snapshot for
`REFUSED`; the refusal surfaced in `uart_rx_equivalence`. Every later check reads the new
emission.) The prediction missed that a guarded hoisted tick composes badly with an enclosing
counted `for`. The cause is the bound, not the guard: `CLKS_PER_BIT` is a constant (434),
which the pass could not see. **Fix:** `copper_analysis::consts` — the start of the shared
evaluator of I.14.4 (literals in any base with `_` and a suffix, file consts by name in any
order, `+ - * / % << >> & | ^`, checked: overflow / division by zero is "unknown", never a
value) — and `control_extract::const_value` folds a bound over the file's consts. A cast is
deliberately not folded (`300 as u8` would need the truncation).

Remeasured, sv-baseline against `5aaa0ff` (226 = 224 + the two new fixture modules): **2
changed — `det_for` ×2 — plus the 2 new**; the four UART modules **byte-identical** (their
bounds now fold, statically non-empty). Predicted 6, measured 2: the miss is the 4 UART
modules, whose bounds were not "unknown" once constants evaluate. `det_for`'s guards read
`(i >= N) && (i != 32'd0)`. Traces: `det_for_probe` 2/2, `uart_rx_equivalence`,
`control_extraction_structural`, `cycle_dataflow_uart_derivation`, the sweep cases of
`det_for`, `rx`, `uart_rx`, `uart_tx`, `uart_rx_dut` pass. The fixture
`tests/fixtures/for_empty_dut.rs` (`for_empty`, `for_empty_start`) **agrees on both legs
and every sequence; with the fix reversed both differential cases FAIL** (checked by
reverse-applying the patch) — the fixture detects (f1).

**A second defect found by the regression:** `register_reconciliation` failed on the new
fixtures — codegen registered `cnt` (and `hi`), the bound's own local, which the shared
inference does not. The desugared loop re-read `end` at every exit test, where Rust evaluates
`start..end` once at entry; for an immutable `let` the extra register only holds the entry
value (no divergence), but a bound reading state the body changes would change the trip
count. **Fixed in the same row:** a bound that is not stable (a constant, a const generic,
arithmetic over those) is snapshotted at entry, `start` and `end` alike. Remeasured: the same 2
changed + 2 new; `register_reconciliation` passes; `timing_model_derivations` — which had
gained `for_empty` in its extraction-hides-phases set while `for_empty_start` (snapshotted
start) stayed out — is unchanged again (both out).

**Left, and recorded (not a stop of the row):** a counted `for` whose range may be empty at
*run time* as the last statement of an enclosing tick-bearing `for` (`for _ in 0..2 { …; for
_ in 0..cnt { …; tick } }`). With an empty inner range each enclosing iteration takes zero
time, which a state machine advancing one state per tick cannot run; before the guard it was
admitted and divergent exactly then, now the transpiler refuses it with its own message
("a counted `for` whose range may be empty at run time is the last statement of an enclosing
tick-bearing `for` …"). The macro admits: a **REFUSED-one** shape, no corpus module has it.
Pinned by `copper-codegen/tests/unsupported_constructs.rs::a_runtime_empty_counted_for_ending_an_enclosing_for_has_its_own_diagnostic`
(with the constant-bound twin transpiling). SPEC item (J.10).

`tools/regression.sh --no-examples` on the row's tree: **PARTIAL OK (196 s)**; G-F 94 (+2);
**G-G differential 1280 PASS / 0 DISAGREE, reference 790 PASS, pinned 0**; examples
`pattern_detector_for`, `uart_rx`, `uart_system` pass. No new `SKIP`/`REF_SKIP` row.

### J.3 Row 3 — (f5) array-element forwarding

**Reading the site first.** In `shir_lower::lower_stmt_list` a register assigned earlier in a
segment is forwarded (`Forwarding::at_edge`) into every **edge-sampled** consumer — a `RegOut`
drive, a memory staging, the `always_ff` copy of an `if`/`match` test — but **not** into the
statements the `always_comb` block computes: `IndexAssign` (the row), a `let` wire's value,
and the combinational copy of an `if` condition. A wire read by a drive is inlined by the
drive's forwarding, which is why the gap is invisible until a value reaches the drive through
something that cannot be inlined — an element-assigned array. So the row may be one instance
of a family. **Four probes, predicted before running** (scratchpad `f5_probes.rs`, sim vs SV):
- `f5_sipo` — `q1_c_sipo_block` rebuilt from Q1_TRADEOFFS §4 (ii) (`w3` carried across the
  back edge, assigned in the last segment, read as `bits[12 + k] = w3[k]`): **DIVERGES**
  (`data_o`'s top nibble is the previous block's `w3`).
- `f5_guard` — `r = x; if r[0] == 1 { bits[1] = 1 } bits[0] = r[3]`: **DIVERGES** (both the
  element value and the combinational `if` read the committed `r`).
- `f5_wire` — `r = x; let y = r + 1; bits[k] = y[k]`: **DIVERGES** (the wire `y` is computed
  in `always_comb` from the committed `r`).
- `f5_branch_wire` — `r = x; let mut t = 0; if r[0] == 1 { t = 5 } q.write(t)`: **DIVERGES**
  (the branch-assigned wire is combinational under the unforwarded test; the drive cannot
  inline a branch-assigned wire).

**Measured before the fix** (the probes as `tests/fixtures/array_forwarding_dut.rs`, two of them
widened to 8 bits — `Bits::<4>::from_u8` is a compile-time error): **all four differential
cases FAIL** and all four reference-leg cases pass — `sim = ref ≠ SV` on each, from cycle 0
(`f5_wire`: SV `q` is the previous cycle's value throughout). **As predicted: the row is a
family of four, not one shape.**

**The fix.** Every statement `lower_stmt_list` emits reads through `Forwarding::at_edge`: a
`let` wire's value, a wire reassignment, an element assignment's index and value, the
combinational copy of an `if` test and of a `match` scrutinee and its arm guards, a `for`'s
bounds. Under one resumption per cycle (D-8) there is no other instant for a read to see; the
two forms of a test (`condition` / `edge_condition`) become equal. **Predicted emission diff:**
the four fixture modules, plus every corpus module whose combinational block reads a register
the segment assigned earlier through one of those statements — not enumerable by reading;
measured below. **Predicted traces:** the four fixtures flip to agree; every other swept
module stays trace-identical (it agrees today, so any changed comb read in it was not
observable or was re-inlined by a drive); every anchor trace-identical.

**Measured.** sv-baseline against `0d5fe5e` (230 = 226 + the 4 fixture modules): **1 corpus
module changed — `rv32i_cpu_pipelined` — plus the 4 new.** Its diff (31 lines) is the
branch-assigned wires `fwd_rs1`, `fwd_rs2`, `flush`, `taken` inlined into their readers (the
combinational copy of a test now takes the forwarded form, which inlines a wire the segment
assigned) and then pruned as dead — the same expressions the `always_ff` copy already read.
No other module's combinational block reads a register its segment assigned earlier through
one of the forwarded statements — fewer than "not enumerable by reading" allowed for; the
prediction did not name the CPU. Traces: the four fixtures **agree on both legs, every
sequence**; `rv32i_pipelined_verilator` (13 programs) and the example 13/13 — the anchor is
trace-identical. `tools/regression.sh --no-examples`: **PARTIAL OK (191 s)**; G-F 98 (+4);
**G-G differential 1332 PASS / 0 DISAGREE, reference 842 PASS, pinned 0**. No new
`SKIP`/`REF_SKIP` row. (f5) closed with its family.

### J.4 Row 4 — (a) a nested `continue` as the loop's entry copy

**The form** (the row's fix direction): a `continue` of a nested loop `W ; tick ; C` runs the
loop's entry copy — `W` lowered in the loop's context, ending in `pc = head` — which is what
entering the loop runs (`lower_into` already emits it there), instead of a zero-time goto to
the head state, which holds the rotated `C ; W`. Lowered once and shared, like a `break`'s
continuation. Kept refused: a **labelled** `continue` (the CFG models a label as the innermost
loop's, D-10) and a `continue` of a tick-bearing counted `for` (it is the next *iteration* —
increment, test — and skips the iteration's tick, so the hoisted tick would no longer stand for
the last iteration's; now with its own message instead of the nested-loop one). A `continue`
inside `W` itself is a zero-time loop that reachability refuses before this pass; the pass
declines if it ever meets one.

**Predicted emission diff:** none in the corpus (230 modules; the three `continue`s in it —
`uart_rx_dut`, `uart/system.rs`, `uart/rx.rs` — and `continue_skip_dut` all target the module
loop). **Measured: 230 modules, no changes — as predicted.** **Predicted traces:** the new
fixture `tests/fixtures/nested_continue_dut.rs` — `nested_continue` (a non-empty `W`: a `let`
read and a `RegOut` write before the tick, `continue` in `C` under an input test, a second tick
and a `break`) and `nested_continue_empty_w` (an empty `W`, the `continue` right after the
loop's first tick) — transpiles and **agrees on both legs, every sequence**. The pin
`continue_inside_a_nested_loop_is_unsupported` (a tick-less nested loop with a `continue`)
predicted **still refused**, by the nested-loop shape rule (no clock boundary), not by the
retired `continue` message.

**Measured — the fixture had to change, and so did the form's reach.** The first fixture
(`W ; tick ; C` with the `continue` in `C`) does not compile: the macro's
`check_nested_loop_shape` admits a nested loop only when its one top-level tick is the LAST
statement ("a repeating wait must be written as …" — the tick-last rule, whose two-job
justification D-10 schedules for retirement, not this row). So in every admitted shape `C` is
empty, the rotation is trivial, the head state holds `W` as written, and the zero-time goto to
it **is** the entry copy — the refusal was over-broad, not the goto. The form landed as: entry
copy only for a non-trivial rotation (unreachable until the tick-last rule changes; kept so a
retirement cannot silently re-open the defect), the shared goto otherwise. The fixture is the
shape the macro admits — D-10's parity probe, a `continue` after a tick inside a branch of
`W`: `nested_continue` (an `if`) and `nested_continue_match` (a `match` arm, with a register
update and a write before the `continue`). Before the change both were refused ("`continue`
inside a NESTED loop …", checked by reverse-applying the patch); after it both **agree on both
legs, every sequence**. sv-baseline: no corpus change (as predicted). The pin was rewritten as
`a_tickless_nested_loop_with_a_continue_is_refused_by_the_shape_rule` (it is refused by "a nested
`loop` with no `clk.tick().await` would never terminate", as predicted).
`timing_model_derivations` gained both fixtures in its extraction-hides-phases set (the
`det_for` class: every tick in a loop or a branch), recorded there. `tools/regression.sh
--no-examples`: **PARTIAL OK (135 s)**; G-F 100; **G-G differential 1356 PASS / 0 DISAGREE,
reference 866, pinned 0**. (a) closes — the REFUSED-one break is gone; a labelled `continue`
and a `continue` of a tick-bearing counted `for` stay refused with their own messages
(transpiler only; the macro admits both — SPEC item J.10).

### J.5 Row 5 — (b) a nested loop whose ticks lie only inside arms

**The form.** The row's fix: at every `AfterTick` fall-through of an arms-only loop, inline the
head's lowered body instead of `pc = head`. In `lower_into` that is exactly a `ZeroTime`
fall-through — a zero-time goto to the head, spliced after every state exists (the head state of
an arms-only loop holds the body as written, unrotated) — so the change is the fall-through kind
of the arms-only branch, plus lifting clause 4 of `check_nested_loop_shape` (both front-ends,
one function) and the gate `body_ends_at_a_clock_boundary` in `control_extract`. The same branch
also serves a nested loop whose body ends in a nested loop (no top-level tick); its fall-through
becomes zero-time too, which is the coroutine's meaning (falling off the body returns to the
head in the same resumption).

**Predictions, two steps.** (A) clause 4 and the gate lifted, fall-through unchanged — D-13 §0
(ii)'s measurement reproduced: `arms_only_b1` **agrees**, `arms_only_b2` **diverges**,
`arms_only_match` (a tick-free continuation after two arms and a two-tick arm) **diverges**.
(B) the zero-time fall-through: all three **agree on both legs, every sequence**. Emission diff
over the corpus (232 modules): **none** at either step — no admitted module has an arms-only
nested loop, and none has a nested loop ending in a nested loop with no top-level tick (if one
does, it changes at (B), and its trace must not). The pin
`ticks_only_inside_branches_has_its_own_diagnostic` flips at (A) and is rewritten as the
admitted shape; no ui/fail case exists for clause 4 (none to delete).

**Measured.** (A) clause 4 and the gate lifted, fall-through unchanged: `arms_only_b1`
**agrees**; `arms_only_b2` and `arms_only_match` **diverge** (differential FAIL, reference leg
pass — `sim = ref ≠ SV`), D-13 §0 (ii) reproduced in the tree. (B) the zero-time fall-through:
**all three agree on both legs, every sequence. As predicted at both steps.** sv-baseline: **no
corpus module changes at either step** (235 = 232 + the 3 fixtures; the 3 are NEW) — as
predicted, including no module with a nested loop ending in a nested loop. Pins:
`ticks_only_inside_branches_has_its_own_diagnostic` → `ticks_only_inside_branches_of_a_nested_loop_transpile`;
the `shape.rs` unit test now admits the `loop` and keeps clause 4's message for a `while` of the
shape (not this row: the row names a nested `loop`; `while` stays refused in both front-ends, one
message). `timing_model_derivations` gained the 3 fixtures (the extracted class; a miss of the
prediction — I had not predicted this pin after row 4 showed it). `tools/regression.sh
--no-examples`: **PARTIAL OK (141 s)**; G-F 103; **G-G differential 1376 PASS / 0 DISAGREE,
reference 886, pinned 0**. (b) closes: REFUSED-both → admitted, the clause lifted in both
front-ends together (one function), no ui/fail case existed; `front_end_parity.rs` sweeps the
fixtures under the shared rule.

### J.6 Row 6 — (c) an array write port per element write

**The form** (the row's first fix direction, a second write port, generalized): write `k` of a
segment to an array register stages write port `k`, in program order; a read lowered after `n`
writes muxes ports `n-1 … 0` (latest outermost) over the committed array; the register's port
count is the most writes any one segment stages. The memory machinery already gives the highest
port priority — outermost in the commit-stage forwarding (`forward_ff_mem_index`, "highest index
outermost") and last among the nonblocking commits — so the last write wins (I.7.3). The priority
mux onto one port (the other direction) cannot carry two writes to *different* elements.

**A probe first** (scratchpad `arr_for.rs`, before the change): an element write inside a
combinational `for` (`for j in 0..4 { arr[j] = a + arr[j] }`) is **admitted today and emitted
wrong** — one write site, one port, its nets reassigned per iteration in `always_comb`, so only
the last iteration's write reaches the commit. Not in II.3.3. One port per site cannot carry it
either; **refused** now with its own message (a REFUSED-one shape, the macro admits; SPEC item),
not unrolled — unrolling is a form of its own.

**Predicted emission diff:** none in the corpus (the refusal of a second write proves no corpus
module writes an array register twice in a segment; a corpus module with an element write in a
combinational `for` would turn REFUSED — predicted none, since such a module would have
diverged in the sweep). **Predicted traces:** the fixture `tests/fixtures/array_two_writes_dut.rs`
— `arr_two_writes` (a constant- and a dynamic-index write, reads before and after) and
`arr_same_element` (two writes to one dynamic element, writes in exclusive arms, a read after
them) — **agrees on both legs, every sequence**; both were refused before. The register-file
anchors (both CPUs, `regfile_dut`) byte-identical.

**Measured — a pre-existing defect found by the fixture.** With the ports in place, sv-baseline
showed no corpus change and the probe `arr_for.rs` was refused with its new message, as
predicted; `arr_two_writes` **agreed**, but `arr_same_element` **diverged** (differential FAIL,
reference leg pass). The emitted ports were right (program order, exclusive arms on distinct
ports, the latest outermost); the wrong read was `arr[0] = arr[1] + arr[2]` — an array read in
**combinational** position (a write's data), which reads the array without the write the
previous cycle staged. An array register commits a stage late (write latency 2, a two-job-era
choice: "the write is staged in the post-tick segment"), so during the cycle after a write the
array does not hold it yet; `forward_ff_mem_index` forwards the committing stage into every
`always_ff` read, nothing did so for an `always_comb` read. **Confirmed pre-existing** with a
one-write-site probe (`arr[i] = arr[k] + a`, whose emission the port change does not touch):
it diverges on the committed tree (`395b23a`'s chir_lower, the row's patch reverse-applied) — an
ADMITTED-AND-DIVERGENT shape not in II.3.3, masked in the corpus because every array read there
reaches a drive. **Fixed:** `forward_comb_mem_index` applies the same committing-stage mux to
every `MemIndex` in the combinational phases, for a memory of write latency ≥ 2 only (at latency
1 the committing stage is this cycle's staging, which a read before the write must not see).
Remeasured: sv-baseline **no corpus change** (238 = 235 + 3 new); `arr_two_writes`,
`arr_same_element` and the probe as `arr_comb_read` **agree on both legs, every sequence**.
Not predicted: the defect itself. Predicted and held: no corpus emission change, the anchors
byte-identical (the CPUs' register-file reads all reach `always_ff`).

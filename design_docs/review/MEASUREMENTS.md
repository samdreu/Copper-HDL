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
byte-identical (the CPUs' register-file reads all reach `always_ff`). `tools/regression.sh
--no-examples`: **PARTIAL OK (226 s)**; G-F 106; **G-G differential 1414 PASS / 0 DISAGREE,
reference 924, pinned 0**. The comb-`for` refusal is pinned by
`unsupported_constructs.rs::an_array_register_element_write_inside_a_combinational_for_is_refused`.

### J.7 Row 7 — (d), (g): the shared constant evaluator (partial)

**Scope reached in this sitting.** I.14.4's direction is one evaluator in `copper-analysis` used by
R11, the transpiler's fold and the interpreter. Started in row 2 (`copper_analysis::consts`:
literals in any base with `_` and a suffix, consts by name, integer arithmetic, checked). This
row routes two of the transpiler's under-evaluations through it:
- **(g)** a `from_lit::<V>()` turbofish in any base or with underscores (`trailing_const_param`
  parsed decimal only) — in an initializer and in a body, since both go through
  `classify_value_ctor`;
- **(d), the non-literal `const` body** (`const START: u8 = 1 + 2`, and consts defined from other
  consts): `fold_const_text` evaluates the body over every file const (`eval_const_items`,
  extended to `true`/`false`).
**Not reached:** (d)'s associated const (`T::K` — the FIR's `FrontendImplIR` captures methods only,
so a parser change comes first), the const generic as a value (a parametric module's initializer
would be the parameter, not a folded literal: `CHIRRegDecl::init` is a literal today), the
`const fn` (its body must be evaluated); R11's `is_constant_expr` and the interpreter still use
their own tests (I.14.4's "the syntactic test becomes that evaluator's success").

**Predicted emission diff:** none in the corpus — 264 `from_lit` uses, all decimal (D-19), and
no corpus initializer names a non-literal `const` (it would be refused today, and the only 4
refused sentinels are other shapes: two width inferences, `with`, a non-hardware call).
**Predicted traces:** the fixture `tests/fixtures/const_eval_dut.rs` — `const_init_expr` (a
register initialized from `const START: u8 = 1 + 2` and a const defined from another, hex and
underscore `from_lit` in an initializer and in the body) — transpiles and **agrees on both legs,
every sequence**; before, refused by the transpiler at the initializer.

**Measured.** sv-baseline: **no corpus change** (239 = 238 + the fixture, NEW) — as predicted.
`const_init_expr` **agrees on both legs, every sequence**; with the `chir_lower` change
reverse-applied it is refused at `n` ("names `const CE_START`, whose value `CE_BASE * 4 + 0x1` is
not an integer or boolean literal", (d)) and, with `n`'s initializer replaced, at `m` ("a call to
`Bits::from_lit::<0xA5>`, which is not a `Bits` value constructor", (g)) — both REFUSED-one
breaks closed for these spellings. `tools/regression.sh --no-examples`: **PARTIAL OK (236 s)**;
G-F 107; **G-G differential 1420 PASS / 0 DISAGREE, reference 930, pinned 0**. Row status:
**(g) closed; (d) partly closed** (the non-literal `const` body); not reached as listed above.

### J.8 Row 8 — (e) the `Bits` method lowerings (partial)

**Scope reached:** 12 of the row's methods, each with its I.14.3(d) meaning and an SV form whose
2-state meaning is the same — `and_reduce` / `or_reduce` / `xor_reduce` → `&x` / `|x` / `^x`;
`nand_reduce` / `nor_reduce` / `xnor_reduce` → `~(&x)` / `~(|x)` / `~(^x)` (1 bit); `get(i)` → a
bit select (a constant index a slice, a dynamic one `x[i]`); `shift_left(n)` / `shift_right(n)` →
`x << n` / `x >> n` (logical; a shift of `N` or more is all zeros in both); `zero_extend::<M>()` /
`truncate::<M>()` → `M'(x)` (zero-fill / low bits); `sign_extend::<M>()` → `M'($signed(x))`.
**Not reached:** `concat`, `part_select`, `replicate`, `set`, `mux`, `with_lsb`, `with_msb`,
`is_valid`, `lt_logic`/`gt_logic`/`le_logic`/`ge_logic`, `as_array`, and the row's owed
`eq_logic`, `ne_logic`, `as_array_mut`, `from_array`, `from_slice`, `From`, `all_ones`; the
`checked_*` (OUTSIDE) / `saturating_*` (IN) decisions of I.14.3(d) are not acted on here.

**Predicted emission diff:** none in the corpus (each method is refused today — "method `…` is
not supported" — and the sweep transpiles every swept module, so none uses one). **Predicted
traces:** the fixture `tests/fixtures/bits_methods_dut.rs` — a combinational module using all 12
(`bits_methods_comb`) and a sequential one using the reductions on a register (`bits_methods_seq`)
— transpiles and **agrees** (sim ≡ SV on the combinational one, which the reference leg skips by
class; both legs on the sequential one).

**Measured.** sv-baseline: **no corpus change** (241 = 239 + the 2 fixture modules) — as
predicted. `bits_methods_seq` **agrees on both legs**; `bits_methods_comb` first **failed** on
`shr` — not a method lowering: the fixture's amount `s.read().as_usize() + 2` is a `usize` sum
in Rust (up to 9, so `shift_right` of 8 or more gives 0) and a 3-bit sum in the SV (`as_usize`
is a passthrough at the receiver's width; 7 + 2 wraps to 1). A width defect of I.14.5 / I.14.7
(3)'s class (masked in the corpus), recorded as a SPEC item, not fixed here; the fixture now
drives the amount from a 4-bit input (still reaching `n >= 8`) and **agrees** (sim ≡ SV; the
reference leg skips a combinational module by class).
`tools/regression.sh --no-examples`: **PARTIAL OK (245 s)**; G-F 108 (75 skipped: +1, `bits_methods_comb`, the combinational class — no new `REF_SKIP` row); **G-G differential 1431 PASS / 0 DISAGREE, reference 936, pinned 0**.

### J.9 Row 9 — R21 / (f6): not reached

Not started in this sitting. What row 1 changed for it: a `tick` before the module loop is now
**refused by the transpiler on both routes** — the pre-loop statements are lowered (row 1), and
a tick among them is refused (`chir_lower`, "`clk.tick().await` before the module's `loop`: the
statements before the loop are the first resumption's …"), so (f6)'s silent drop on the linear
route is gone. The macro still admits (`validate_hardware_fn` counts any tick), so R21 is a
REFUSED-one break until the shared rule lands in `copper-analysis` with a `copper-macros/tests/ui/fail`
case and a `front_end_parity.rs` entry — the row as written.

### J.11 The bare regression — measured

Bare `tools/regression.sh` on `b2eeea3` (laptop): **REGRESSION OK (248 s)**.
- 1205 passed, 0 failed, 88 ignored, 140 test binaries; G-C 114 test files; examples **32/32**
  (both CPUs, every BaseJump self-check, `dual_port_ram`, the UART, two-domain and CDC examples).
- G-D **180** differential cases (161 at §H.6; +19, every one a fixture of this step), 3
  ignored-with-reason (`SKIP`: `rv32i_cpu_pipelined`, `two_domain_top`, `mod8` — unchanged).
- G-F 108 modules through the interpreter (90 at §H.6), 75 skipped with a sentence (+1:
  `bits_methods_comb`, the combinational class — no new `REF_SKIP` row).
- **G-G differential 1431 PASS / 0 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0 DISAGREE; reference
  936 PASS** (1240 / 750 at §H.6).
- **Pinned disagreements: 0** (2 at §H.6: `preloop_probe`, `preloop_linear` — gone, row 1).
- G-E 5 anchored, 178 transpiler-only. New `SKIP` / `REF_SKIP` rows: **none**.
- Anchors: the pipelined CPU's 13 programs pass (its SV changed in rows 1 and 3, trace-identical);
  every other anchor's SV is byte-identical to `80ae993` (no anchored module is in any row's
  changed set), so its trace cannot have moved.

### J.10 For the coordinator — SPEC items (SPEC files untouched)

**Rows closed, with their fixtures (III.3 ledger lines):**
1. **(f2), (f3) — closed** (row 1): `tests/fixtures/preloop_entry_dut.rs` (`preloop_probe`,
   `preloop_linear`), swept; `tests/reference_leg_measurements.rs::preloop_statement_agrees_on_both_routes`.
   **P15 MEASURED true** on both routes. III.2's `sim = ref ≠ SV` instances: none left (0 pins).
   I.9.2's realization: the started carrier (`__copper_started`, I.7.2(j)), one bit, shared with
   the trailing region; the entry-state alternative was not taken (the linear route has no state
   register to give an entry value). (f4) was closed by step 4 (§H.5 item 3).
2. **(f1) — closed** (row 2): `tests/fixtures/for_empty_dut.rs` (`for_empty` = D-10 probe (ii),
   `for_empty_start`). Two further facts for I.8.4/I.8.5: the desugar now evaluates a
   counted-`for` range **once at entry** (a non-stable bound is snapshotted — it was re-read at
   every exit test); and a bound named by a file `const` is now statically known (the shared
   evaluator). **New REFUSED-one shape** (a II.3 row candidate): a counted `for` whose range may be
   empty at run time as the last statement of an enclosing tick-bearing `for` — with an empty
   inner range each enclosing iteration takes zero time; admitted-and-divergent before (only when
   empty), refused by the transpiler now with its own message; the macro admits. Pin:
   `unsupported_constructs.rs::a_runtime_empty_counted_for_ending_an_enclosing_for_has_its_own_diagnostic`.
3. **(f5) — closed, with a family of four** (row 3): `tests/fixtures/array_forwarding_dut.rs`
   (`f5_sipo` = `q1_c_sipo_block`, `f5_guard`, `f5_wire`, `f5_branch_wire`). II.3.3's site
   description is narrower than the defect: every `always_comb` statement (a `let` wire, a wire
   reassignment, an element assignment, the combinational copy of a test / scrutinee / guard, `for`
   bounds) read the committed register. I.6.9 / I.3's implementation map: `shir_lower::lower_stmt_list`
   now forwards every read; `SHIRStmt::If::condition == edge_condition` always.
4. **(a) — closed** (row 4): `tests/fixtures/nested_continue_dut.rs`. Under today's
   `check_nested_loop_shape` (tick last, one top-level tick) the rotation is always trivial, so the
   goto to the head IS the entry copy; the entry-copy lowering is in place for a non-trivial
   rotation (reachable once D-10's tick-last retirement lands). Still refused by the transpiler
   only (REFUSED-one, two parity-list lines for II.1.3): a **labelled** `continue`, and a
   `continue` of a tick-bearing counted `for` (now with its own message).
5. **(b) — closed** (row 5): `tests/fixtures/arms_only_loop_dut.rs` (`b1`, `b2`, a `match`);
   clause 4 lifted for a nested `loop` in both front-ends (one function); a `while` of the shape
   keeps the clause's message in both (not this row). II.3.2's sort: rule 3's clause 4 → retired.
   The same lowering change makes a nested loop with no top-level tick ending in a nested loop
   zero-time at its fall-through (no corpus module; `shape.rs`'s `body_ending_in_a_nested_wait_is_accepted`
   shape) — I.8.2's meaning.
6. **(c) — closed** (row 6): `tests/fixtures/array_two_writes_dut.rs` (`arr_two_writes`,
   `arr_same_element`, `arr_comb_read`). **Two new findings for II.3:** (i) an element write to an
   array register inside a combinational `for` was ADMITTED-AND-DIVERGENT (only the last
   iteration's write committed) — now **REFUSED-one** (pin
   `an_array_register_element_write_inside_a_combinational_for_is_refused`; the form that would
   realize it is unrolling); (ii) a combinational-position read of an array register missed the
   write still in its pipeline stage — **ADMITTED-AND-DIVERGENT, fixed in this row**
   (`vlir_lower::forward_comb_mem_index`), fixture `arr_comb_read`. Both deserve a II.3 line with
   its date for the ledger. I.10's implementation map: an array register's write-port count is the
   most element writes one segment stages.
7. **(g) — closed; (d) — partly** (row 7): `tests/fixtures/const_eval_dut.rs`. `copper_analysis::consts`
   exists (I.14.4's evaluator, started); the transpiler's `from_lit` turbofish and `const` bodies
   use it. Open for (d): associated consts (FIR has no impl consts), const generics as values
   (`CHIRRegDecl::init` is a literal), `const fn`s; R11's `is_constant_expr` and the interpreter
   still use their own evaluators (I.14.7 (2) open).
8. **(e) — 12 methods closed** (row 8): `tests/fixtures/bits_methods_dut.rs` — `and_/or_/xor_/
   nand_/nor_/xnor_reduce`, `get`, `shift_left`, `shift_right`, `zero_extend`, `sign_extend`,
   `truncate` (12 ledger lines). The others of the row stay REFUSED-one (listed in §J.8).
   **Width finding:** `x.as_usize()` lowers as a passthrough at `x`'s width, so `usize` arithmetic
   on it wraps at that width in the SV and not in Rust (`s.read().as_usize() + 2` on a 3-bit `s`) —
   an I.14.5 (a) realization defect, masked in the corpus; a II.3 line candidate.
9. **R21 / (f6) — not reached as a front-end rule**; (f6)'s silent drop is gone (row 1 refuses a
   pre-loop tick in the transpiler on both routes); still REFUSED-one (§J.9).
10. **II.3.6's map** has drifted further: (f1) `control_extract::expand_counted_for` (guard, bound
   snapshots), (f2)/(f3) `chir_lower` pre-loop lowering + `shir_lower` `has_started`, (f5)
   `shir_lower::lower_stmt_list`, (a) `control_extract::LoopCtx::entry`, (b)
   `control_extract::ticks_only_in_arms` and `copper-analysis/src/shape.rs::ticks_only_in_arms`,
   (c) `chir_lower` `array_written`/`array_write_ports`, (e) `chir_lower::lower_method_call`.

## J (continued). The open rows, second sitting (2026-11-07)

Same branch, from `2aca023`. Same method: prediction before code, fixture, sv-baseline against
the previous row's snapshot, anchors trace-identical.

### J.12 Row 9 — R21 / (f6): a `tick` before the module loop, refused in both front-ends

**The form** (the row's direction, II.1.2 R21): one shared rule, `copper_analysis::check_no_preloop_tick`
— any `<clock>.tick().await` in a statement before the module's top-level `loop` is refused with
one message, citing I.2.2 (`s_pre` is tick-free) and I.9.3 — called by the macro's behavioural
arms and by `transpile_target`, beside `check_awaits`. The `chir_lower` refusal row 1 added stays
as defence in depth, now carrying the same message. **Predictions:** sv-baseline **no change**
(every corpus module transpiles, and since row 1 the transpiler refuses a pre-loop tick, so none
has one); a new `copper-macros/tests/ui/fail/preloop_tick.rs` case (the macro admitted it before:
`validate_hardware_fn` counts any tick of the clock); `front_end_parity.rs` test 1 gains the rule
and stays green; `unsupported_constructs.rs` gains the transpiler's twin, asserting the same
message text.

**Measured.** sv-baseline: **no corpus change** (the 2 NEW were row 8's fixtures, not in the
previous snapshot) — as predicted. `copper-macros/tests/ui/fail/preloop_tick.rs` refuses with
the shared message (its `.stderr` the only file `TRYBUILD=overwrite` wrote — no other case moved);
`unsupported_constructs.rs::a_tick_before_the_module_loop_is_refused_by_r21_on_both_routes` asserts
the same text on a linear-route and an extraction-route module; `front_end_parity.rs` test 1 runs
the rule over the corpus, green. **(f6) closed, R21 in both front-ends with one message.**
`tools/regression.sh --no-examples`: **PARTIAL OK (282 s)**; G-G differential 1431 PASS / 0 DISAGREE, reference 936, pinned 0.

### J.13 Row 7, the rest of (d)

**Two parts of the request need a different form — stopped, recorded.**
- **R11 (`check_register_initializers`) on the shared evaluator.** In the macro the rule sees the
  annotated `ItemFn` only — a proc macro is not given the file — so it cannot *evaluate* `START`,
  `T::K` or `f(3)`; rustc evaluates them. What R11 can decide there is whether an initializer is
  constant-*shaped* (I.14.4's grammar with names unresolved), which is what `is_constant_expr` is.
  Moving it onto an evaluator needs either the file's items in the macro (not available to a proc
  macro) or a rustc-checked spelling (the macro emitting the initializer in a `const` context,
  which needs every `Bits` constructor to be a `const fn` — they are not today). A decision for
  the coordinator/user.
- **The interpreter on `copper_analysis::consts`.** `copper-ref` evaluates a constant with its own
  typed evaluator (`Exec::eval_const`, `lookup_const`: `Value`s with `Bits` widths, X, coercion to
  the declared type), which is *richer* than the `i128` evaluator in `copper-analysis`; routing it
  through the latter would lose the types. The direction "one evaluator" points the other way —
  the interpreter's typed evaluator as the shared one (a dependency question: `copper-codegen`
  would evaluate through `copper-ref`'s code, or that code moves into `copper-analysis`). A
  decision for the coordinator/user.

**Done here — the transpiler's fold.** `copper_analysis::consts::ConstEnv` over the file's
const items, impl consts (`T::K`) and `const fn`s (parameters bound to integer arguments; a body
of `let` bindings, `if`/`else` and a tail expression; recursion bounded), carried into the FIR as
source text (`FrontendModuleIR::file_const_source`, since `copper-core` has no `syn`). The fold
uses it for a path (`K`, `T::K`) and for a call to a `const fn`; row 2's counted-`for` bounds use
the same environment. **Predicted emission diff:** none in the corpus (no corpus initializer
names an associated const or calls a `const fn` — it would be refused today; a counted-`for` bound
naming a const defined by a `const fn` would become statically known, and none is in row 2's
inventory). **Predicted traces:** fixture `tests/fixtures/const_eval_dut.rs::const_assoc_fn`
agrees on both legs; refused before.

**Measured.** sv-baseline: **no corpus change** (243 = 241 + the 2 new fixture modules) — as
predicted, including no counted-`for` bound newly folded (the corpus's `const fn`s,
`safe_clog2` ×5, feed only combinational generics). Fixture `tests/fixtures/const_eval_dut.rs`
gained `const_assoc_fn` (`CeCfg::DEPTH`, `CE_W = ce_clog2(CeCfg::DEPTH as usize) as u8` — the
corpus's own `clog2` shape: `match`, mutable locals, `while` — and `CeCfg::half(201)`) and
`const_generic_init<START>` (`let mut n: usize = START`, the sweep instance `START = 5`, a new
`PARAMS` row with its sentence; emitted `initial n = 32'(START);`). Both **agree on both legs,
every sequence** — after two fixes the first run found:
- the **interpreter** refused `CeCfg::DEPTH` ("associated item … (II.3 row (d))"): it now
  collects an impl's consts as `T::K` and looks them up (`copper-ref` `module.rs`, `interp.rs`) —
  its own typed evaluator, not `copper_analysis::consts` (the stop above);
- the first `const_generic_init` spelling failed **Verilator** on `Bits::from_usize(n & 0xFF) ^ x`
  (a 32-bit `usize` reaching an 8-bit operand: step 8's I.14.7 (3) class, left alone as asked); the
  fixture observes `n` through comparisons instead. A `u8` const generic is not swept: the
  generated sweep passes every `PARAMS` value as `usize`.
Not re-measured: that each new shape was refused before (by reading: the fold's
"names `…`, which is not a constant this module can see" for `T::K` and `N`, "a call to `…`" for
`T::f(…)`). R11 is unchanged, so a single-segment `const fn` call as an initializer
(`let mut n = f(3);`) is still refused by **both** front-ends — no parity break, an
under-admission against I.14.4 that lands with R11's form decision.
`tools/regression.sh --no-examples`: **PARTIAL OK (325 s)**; G-F 110; G-G differential 1443 PASS / 0 DISAGREE, reference 948, pinned 0. Row status: **(d) closed in the transpiler** (associated consts, `const fn`, const generics as a power-on value); **stopped**: R11 and the interpreter on the shared evaluator (above).

### J.14 Row 8, the remaining `Bits` methods

**The forms** (each I.14.3(d)'s meaning in 2-state SV; X is outside every SV pair, III.2.2):
`a.concat(b)` → `{a, b}` (`a` high); `x.part_select::<LEN>(lo)` → `x[lo+LEN-1:lo]` (a constant
`lo`) or `LEN'(x >> lo)`; `x.replicate::<OUT>()` → `{k{x}}` as `k` copies (a receiver of known
concrete width); `x.set(i, l)` → the element assignment `x[i] = l` (a statement); `Bits::mux(s, a,
b)` → `s ? a : b`; `x.with_lsb(l)` / `x.with_msb(l)` → `{x[W-1:1], l}` / `{l, x[W-2:0]}` (concrete
width); `is_valid()` → `1` (a 2-state value is always valid; the simulator's X answer is outside
the SV pairs); `eq_logic`, `ne_logic`, `lt_logic`, `le_logic`, `gt_logic`, `ge_logic` → `==`,
`!=`, `<`, `<=`, `>`, `>=` (unsigned, one bit); `as_array()` → the receiver (the packed vector
is the array, as `from_array` already is); `Bits::all_ones()` → all ones at the turbofish width,
or `~0` retyped by context; in an initializer the fold states it. **Kept refused:**
`as_array_mut` (a `&mut` into the vector — element writes are `x[i] = l` / `set`), `From`
(`Logic::from(bool)`; not a `Bits` method of the row's spelling), and the `checked_*` four
(OUTSIDE by I.14.3(d)); `saturating_add`/`_sub` (IN by I.14.3(d)) not reached — they need a
clamp form, recorded. **Predicted:** no corpus change (each was refused); the fixture
`tests/fixtures/bits_methods_dut.rs` gains `bits_methods_rest_comb` (combinational, sim ≡ SV)
and `bits_methods_rest_seq` (`set`, `all_ones` initializer, a register through `concat` /
`part_select`; both legs) — agree.

**Measured.** sv-baseline: **no corpus change** (245 = 243 + the 2 new) — as predicted. The two
fixture modules **agree** (the combinational one sim ≡ SV; the sequential one on both legs) after
four fixture-side corrections the first runs found, none in the lowerings: (i) the interpreter
refuses an unsized `Bits::all_ones()` (I.14.4's recorded parity item) — the sequential module
uses `Bits::<8>::all_ones()`, the combinational one (no interpreter leg) keeps the unsized
spelling; (ii) Verilator `-Wall` refused my first sequential fixture for an unread register
half (`UNUSEDSIGNAL`) — the fixture now reads it; (iii) `Logic::from_bool(…)` is not lowered (the
row's `From` family, not reached) — `is_valid()` is used as a test; (iv) the unsized `all_ones()`
first emitted `~64'd0` into an 8-bit wire (`WIDTHTRUNC`): `expr_width` now gives `~<literal>`
its literal's width so the assignment resizes it — that one shape only, measured not to move
any corpus module. **Not reached / kept refused:** `as_array_mut`, `From` / `Logic::from_bool`,
`saturating_add` / `saturating_sub` (IN by I.14.3(d), need a clamp form), the `checked_*` four
(OUTSIDE). Each method that closed is one III.3 ledger line: `concat`, `part_select`,
`replicate`, `set`, `mux`, `with_lsb`, `with_msb`, `is_valid`, `eq_logic`, `ne_logic`,
`lt_logic`, `le_logic`, `gt_logic`, `ge_logic`, `as_array`, `all_ones` (16).

`tools/regression.sh --no-examples`: **PARTIAL OK (300 s)**; G-F 111 (76 skipped: +1, `bits_methods_rest_comb`, by class); G-G differential 1455 PASS / 0 DISAGREE, reference 955, pinned 0.

### J.15 The row-2 residue: a run-time-empty counted `for` ending an enclosing ticking `for`

**Decided on the evidence: refuse in both front-ends, not realize.** With the inner range empty,
each iteration of the enclosing `for` takes zero time, so a realization must run several enclosing
iterations' work inside one resumption — an unrolling of the enclosing loop. With a run-time
enclosing bound there is no finite unrolling; with a constant one it is a new lowering (copies of
the enclosing body chained in zero time, each guarded on the inner range being empty) whose state
count grows with the bound, for a shape no corpus module has (0 of 245; the transpiler has
refused it since row 2 and nothing broke). The refusal is a dated II.3 limitation (REFUSED-both),
its fix direction the unrolling.

**The rule** (`copper_analysis::check_counted_for_nesting`, both front-ends, one message, the
transpiler's gate keeping its copy): a tick-bearing `for` whose body's last statement is a
tick-bearing `for` with a bound that is not **static** — static meaning an integer literal or an
expression over constant-shaped names (I.14.4's syntactic test, `is_constant_expr`) none of which
is a const generic of the module or a local. The macro cannot evaluate a `const`, so a static
bound that evaluates to an empty range is still admitted — and is then dropped by the transpiler
as a statically empty range (zero work, zero time: the same meaning), never refused. A static
bound the transpiler cannot evaluate (a `const fn` outside the evaluator's grammar) stays a
transpiler-only refusal (recorded). **Predictions:** sv-baseline no change; a ui/fail case
`nested_runtime_empty_for.rs`; `front_end_parity.rs` gains the rule, green over the corpus; the
`unsupported_constructs.rs` pin asserts the shared message; its constant-bound twin still
transpiles; a const-generic inner bound (`0..N`) is refused by both.

**Measured — as predicted.** sv-baseline: **245 modules, no changes**. The macro refuses
`copper-macros/tests/ui/fail/nested_runtime_empty_for.rs` with the shared message (its `.stderr`
the only file `TRYBUILD=overwrite` wrote); `unsupported_constructs.rs::a_runtime_empty_counted_for_ending_an_enclosing_for_has_its_own_diagnostic`
asserts the same message for `0..cnt` and for a const-generic `0..N`, and its `const CNT`
twin still transpiles; `front_end_parity.rs` test 1 runs the rule over the corpus, green (the
UART's `0..CLKS_PER_BIT` inner loops are static by the rule). The REFUSED-one break of row 2
becomes **REFUSED-both** (a dated II.3 row; fix direction: unroll the enclosing loop).

### J.16 The bare regression after the second sitting — measured

Bare `tools/regression.sh` on `c25f182` (laptop): **REGRESSION OK (345 s)**. 1214 passed, 0
failed, 89 ignored, 140 test binaries; examples **32/32**; G-D **184** differential cases (180 at
§J.11), 3 ignored-with-reason (unchanged `SKIP` rows); G-F 111 modules, 76 skipped (+1 by class);
**G-G differential 1455 PASS / 0 DISAGREE, reference 955 PASS, pinned 0**; G-E 5 anchored, 182
transpiler-only. New `SKIP` / `REF_SKIP` rows: none; new `PARAMS` row: `const_generic_init`
(`START = 5`). Anchors: no anchored module is in any changed set of this sitting (every row's
sv-baseline diff was NEW fixtures only), so their SV is byte-identical to `2aca023`.

### J.17 For the coordinator — SPEC items of the second sitting (amending J.10)

1. **R21 / (f6) — closed** (J.12): `copper_analysis::check_no_preloop_tick`, both front-ends,
   one message; ui/fail `preloop_tick`; parity-audit row. II.1.2 R21 realized; J.10 item 9 is
   superseded.
2. **(d) — closed in the transpiler** (J.13): associated consts, `const fn`s, a const generic as a
   power-on value (`initial n = W'(N)`, `CHIRRegDecl::init_param`). Fixtures
   `const_eval_dut.rs::const_assoc_fn`, `::const_generic_init`. **Two decisions owed (stopped):**
   R11's macro side cannot evaluate names (a proc macro is not given the file) — keep it
   syntactic, or have the macro emit a rustc-checked `const` context (needs `const fn` `Bits`
   constructors); and "one evaluator" — the interpreter's typed evaluator is the richer one, so
   either it becomes the shared one (moves to `copper-analysis` or becomes a dependency of
   `copper-codegen`) or the `i128` evaluator grows types. A single-segment `const fn` call as an
   initializer stays refused by R11 in both front-ends until then (an I.14.4 under-admission,
   no parity break). The interpreter learned impl consts (`T::K`) in its own evaluator.
3. **(e) — 28 methods closed in all** (J.8: 12; J.14: 16), one III.3 line each. Open: `as_array_mut`,
   `From` / `Logic::from_bool`, `saturating_add` / `saturating_sub` (IN, need a clamp form); the
   `checked_*` four stay refused (OUTSIDE). The interpreter's refusal of an unsized `all_ones()`
   is the I.14.4 parity item, still open.
4. **The row-2 residue — REFUSED-both** (J.15): `copper_analysis::check_counted_for_nesting`;
   a dated II.3 row: shape "a counted `for` with a non-static bound as the last statement of an
   enclosing tick-bearing `for`", meaning I.8.4(b) (zero iterations, zero time), fix direction
   "unroll the enclosing loop's zero-time iterations". ui/fail `nested_runtime_empty_for`.
5. **Left alone, as asked:** the `as_usize` / `usize` width class (step 8, I.14.7 (3)); met again
   in J.13 (`Bits::from_usize(n & 0xFF)` at 32 bits).

## K. Phase C step 6 — register inference: the head clause, the nested loop, P3 / P13 / P14 (2026-11-07)

Branch `claude/phase-c-registers-51a627` (from `paper` at `cd14f6e`). D-20 obligation (6);
SPEC I.7.1 (the definition), I.7.5 (the clause and the two fold defects), I.7.6–I.7.7; III.1
rows P2, P3, P13, P14. Every prediction below was written here before the measurement it
predicts was run. Instruments: `register-baseline` (new, `copper-codegen/src/bin/register-baseline.rs`:
`infer_registers` for every clocked module of `examples/`, `tests/`, `src/` — **175** at `cd14f6e`,
inline test strings excluded), snapshot on the untouched tree; `sv-baseline` (**245** modules),
snapshot on the untouched tree.

### K.0 Predictions — (1) the head clause deleted

I.7.5 measured the clause's surplus on 2026-11-04 as 8 locals in 7 modules. Step 3 re-spelled
`sync_2ff`, `flag_sync` (all three copies) and `sync_2ff_concrete` tick-last, and D-8's rewrites
put `struct_latch`, `struct_latch_cond`, `tuple_destructure` in the write-before-tick form, where
`latch` and `target` are assigned before the tick and read after it — live at the tick. Read off
today's sources, the one remaining instance of the shape (assigned after the iteration's last
tick, read in the next head before its first tick) is `trailing_local_into_head`'s `w` (its own
doc comment already calls it a wire).

- Inferred-set change: **1 local in 1 module** — `tests/fixtures/trailing_statements_dut.rs::trailing_local_into_head`
  loses `w`. No other module changes.
- Emitted SV: `chir_lower` consults `fir.registers` for a pre-loop `let mut`, so the module's SV
  **changes** (not "none" as the brief expected): `w` stops being a flop and becomes the pre-loop
  wire (`Bits::zero()`) the trailing assignment overrides, `o <= ((__copper_started ? (r + 8'd1) : 8'd0) + r)`.
  No other module's SV changes.
- Verdicts: none change; the module agrees on all three legs before and after.

### K.1 (1) The head clause deleted — measured

`compute_registers` no longer adds `live_out[head]`.

- **Inferred-set diff: 1 of 175 — `trailing_local_into_head` loses `w`, as predicted.** The
  clause's 2026-11-04 surplus of 8 locals in 7 modules is down to this one: the other six
  modules were re-spelled by step 3 / D-8 so that the local is live at the tick.
- **sv-baseline: 1 of 245 — the same module, as predicted, but the predicted emission was
  wrong in one respect (a miss).** `w` stopped being a flop and became a pre-loop wire, and the
  wire's initializer was emitted as `w = 64'd0`: Verilator refused the model (`%Warning-WIDTHTRUNC`
  … `%Error: Exiting due to 1 warning(s)`), so the sweep case FAILED. Cause: the pre-loop wire
  path of `chir_lower` (`pre_loop_wires`, shared by a pre-loop `let` and by a pre-loop `let mut`
  the register authority calls a wire) lowered the initializer with no target width, where the
  in-loop `let` path (`lower_local`) applies `retype_default_literals_in_values` and
  `resize_to_target`. No corpus module had reached it: a pre-loop non-`mut` `let` with a
  context-width initializer does not occur, and until now no pre-loop `let mut` was demoted.
  **Fixed** (same two calls on that path); sv-baseline after the fix: still 1 of 245, the same
  module, now `w = 8'd0`. The trailing assignment `w = r + 1` is no longer emitted to `w` at all:
  its one reader, the head's `o.write(w + r)`, reads the forwarded value (`__copper_started ?
  (r + 8'd1) : w`), which is what I.3.4 says of a wire of a resumption.
- **Verdicts:** `trailing_local_into_head` 6 sequences PASS, reference leg agrees (864 cycles,
  observation 0 on 6 of 6) — before and after. No other verdict can change (no other SV changed).
- **Unit pin retired:** `cfg::tests::post_tick_def_read_pre_tick_is_a_register` (asserted
  `["ff1", "ff2"]`, I.7.6's obligation) → `post_tick_def_read_pre_tick_is_a_wire`, asserting
  `["ff1"]` for the head-read spelling and for the anchored tick-last one.

### K.2 Predictions — (2) the nested loop's CFG descended

The register graph (`Cfg::reg_nodes`, built by the same `Builder` with `expand_loops`) builds
each nested loop as control flow: `loop` = header → body → header, left by `break`; `while c` =
header[uses c] → body → header, header → exit; `for p in e` = entry[uses e, defs p] → header
| body, header[uses p] → body → header, header → exit. The `for`'s binding stands for the
counter of I.8.5's desugaring (I.7.2(g), D-10: "the counter `c` is a register of the kernel
whatever the pattern"), so a named counter is read at the header and is live at every interior
tick; a `_` counter has no name and stays outside the set, as before (`__copper_ctr`). The
zero-iteration exit is omitted only for a range whose bounds are integer constant expressions of
the source alone with `start < end` (`..=`: `<=`); a bound naming a const item or generic keeps
the exit — over, never under (I.7.5's direction). Labelled `break`/`continue` go to the loop they
name. Every other rule keeps the folded graph.

Read off the 40 modules of `examples/`, `tests/` with a tick-bearing nested loop (a source scan),
against today's sets:

- **`tests/ref_probes/sync_2ff_tick_last.rs::fold_defect`: `{i}` → `{i, v}`** — the missed local
  of I.7.5 (assigned before the `for`, read inside it after an interior tick).
- **`tests/fixtures/extracted_dut.rs::ex_regfile`: `{ra, regs, wa, wd, we}` → `{regs}`** and
  **`::ex_reg_then_wire`: `{r, xv}` → `{r}`** — the added local of I.7.5: `let we = wen.read()`
  etc. are assigned after the head tick and read after `if h { loop { tick } }`; the halt loop
  has no exit, so no path carries them across its tick, and they are wires of the head's
  resumption. Under the fold, the halt loop was a node whose out-edge is a tick and they were
  live out of it.
- Every other module unchanged, including `det_for` (`ok` is assigned before the `for` and read
  after it on the path through an interior tick and the exit, so it stays), `serializer`,
  `uart_tx`, the UART receivers, the counted fixtures, the arms-only and nested-`continue`
  fixtures, `for_empty*` (the run-time bounds are read at the entry only), and the pipelined CPU
  (its halt `loop { tick }` has no exit and nothing is live into it). No module with only
  tick-free nested loops changes (a fold had no defs, so expanding can only kill more).
- Emitted SV: **no change** (sv-baseline 0 of 245). `v` in `fold_defect` and the `ex_*` locals
  are realized by the transpiler's own mechanisms (control extraction hoists cross-state locals
  and appends them to `fir.registers` itself; a same-state in-loop `let` is a wire whatever
  `fir.registers` says).
- Verdicts: none change. The poison-run pin on `fold_defect` (`tests/reference_leg_measurements.rs`,
  "the fold probe's flag asserted present") **fails** and is inverted: `v` is no longer poisoned.
  The poison run newly poisons `we`, `wa`, `wd`, `ra`, `xv` and stays clean.

### K.3 (2) The nested loop's CFG descended — measured

- **Inferred-set diff (register-baseline against the post-(1) snapshot): 4 of 175.** Three as
  predicted — `fold_defect` `+v`; `ex_regfile` `−{ra, wa, wd, we}`; `ex_reg_then_wire` `−xv` —
  and **one miss: `rv32i_cpu_pipelined` `−{fetched, mem_load}`.** The mechanism is the
  `ex_regfile` one and I mis-applied it to the CPU: both are assigned after the head tick and
  read after the halt `if mem_wb.is_ecall { …; loop { tick } }`, which the fold made a node with
  a tick out-edge they were live across. I looked at what is live INTO the halt loop instead of
  what the fold carried OUT of it. Both are wires of the head's resumption (the memory results
  read through `data()` at the head).
- **sv-baseline: 0 of 245**, as predicted. The CPU's SV is byte-identical, so its 13-program
  anchor is untouched.
- **Verdicts:** none can change (no SV changed; the simulator and the interpreter do not read
  the register set).
- **P2, the poison run (`tests/reference_leg_measurements.rs::poison_run_for_the_register_proposition`),
  re-run: 111 modules clean, 0 flagged, 77 not eligible** (§D.4 had 47 clean; the D-8 rewrites
  put the plain-`Out` modules in the leg). The fold probe: `infer_registers` = {i, v}, the poison
  set is empty and the trace unchanged; forcing `v` into it is caught (`Poisoned("v")`) — the
  probe keeps its teeth as a positive. The pin was inverted accordingly. The modules whose sets
  shrank are in the clean set: `we`/`wa`/`wd`/`ra`/`xv`/`fetched`/`mem_load` are poisoned at every
  tick and no read sees the poison for `ex_regfile`/`ex_reg_then_wire`, which are leg-eligible.
  The pipelined CPU is not (a `SKIP` row: anchored by its own 13-program test), so for its
  `fetched`/`mem_load` the evidence is the byte-identical SV, not the poison run.
- `copper-codegen/tests/register_reconciliation.rs`: 110 clocked fixtures, no violation (the
  `ex_*` locals were absorbed comb wires before and still are).

### K.4 Predictions — (3) the two fixtures, (4) P13 on the corpus

Fixtures, `tests/fixtures/nested_loop_registers_dut.rs` (swept like every fixture):
`nested_loop_missed` (I.7.5's missed shape, the swept twin of `fold_defect`) and
`nested_loop_added` (the added shape: `w` assigned inside a nested `loop` and read only after it,
dead at every tick). The added shape could not be written as I.7.5 words it with a counted
`for` (`for _ in 0..3 { tick; w = x.read() }`): the transpiler refuses a counted body whose tick
is not last, and a wait loop whose tick is not last — so it is `loop { w = x.read(); if w[0] {
break } tick }`, where the value assigned before the interior tick is reassigned after it before
any read. Inferred (measured while writing them, so not a prediction): {i, v} and {} — the fold
would have given {i} and {w}.

**Observed while writing them (before the corpus run): `nested_loop_added`'s SV keeps a flop
`w`** (`w <= x` in both FSM states, never read: `q <= x + 8'd1` reads the forwarded value).
Control extraction hoists `w` because it is mentioned in two states, and the hoist appends it to
`fir.registers` itself. So P13 fails on this fixture by one unobservable flop, and the
reconciliation test, which allows no non-synthetic flop the inference does not name, fails on it.

P13 corpus measurement (`copper-codegen/tests/register_reconciliation.rs::corpus_realized_registers_are_inferred`,
new: the reconciliation over `examples/`, `tests/fixtures/`, `tests/ref_probes/`, `src/` — every
clocked module the transpiler accepts — reporting each codegen flop that is neither inferred nor
synthetic, by mechanism: a `<name>_r` flop is a promoted wire (`find_promoted_wires`), a bare name
a hoisted cross-state local (a pre-loop `let mut` the inference does not name lowers as a wire)):

- Predicted surplus: **`nested_loop_added`'s `w`, and nothing else.** The fixtures were already
  clean (110 modules); in `examples/` and `src/` I expect the extraction-route modules' hoisted
  locals to be live at a tick (they are read in a state other than the one that assigns them,
  which needs a tick between unless the transfer is a same-cycle `pc` transition — the
  `nested_loop_added` shape, which I know of nowhere else). The pipelined CPU's `fetched` /
  `mem_load` (no longer inferred, K.3) are read in the state that assigns them, so not hoisted.
- Promoted wires: predicted **none** outside the inferred set (a wire read in a later segment
  crosses a tick in the linear route by construction).

### K.5 (4) P13 on the corpus — measured

`corpus_realized_registers_are_inferred`: **146 clocked modules** of `examples/`, `tests/fixtures/`,
`tests/ref_probes/`, `src/`, every one transpiled. First run: 5 surplus entries. **Three are
artifacts of the matcher**, not flops outside the set: `rv32i_cpu_transpilable`'s `a0` and
`halted` (the source names its locals `a0_r` and `halted_r`, and the extractor's `_r`-stripping
turned the codegen flops `a0_r`/`halted_r` into `a0`/`halted`), and `uart_tx`'s `byte_sig` (the
legalizer's rename of the SV keyword `byte`; `byte` is inferred). The matcher now accepts both
spellings (shared with the fixture test). **Two are real surplus flops — P13 fails on the
corpus by two:**

| module | flop | mechanism | observable? |
|---|---|---|---|
| `tests/fixtures/nested_loop_registers_dut.rs::nested_loop_added` | `w` | hoisted: `hoist_cross_state_locals` moves `w` (mentioned in two FSM states) to a pre-loop `let mut`, appended to `fir.registers` by `transpile_fir` | **no** — `w <= x` in both states; nothing reads the flop (`q <= x + 8'd1` reads the forwarded value) |
| `tests/ref_probes/startup_probes.rs::trailing_fold` | `r_r` | promoted: `let r = Bits::from_u8(5)` (a pre-loop constant, I.7.2(b)) is a segment-0 wire; the trailing segment reads it, so `find_promoted_wires` gives it a carrier | **no** — `r_r <= r` (= 5) at every edge including the first, and its one reader `q <= r_r` is under `__copper_started`, i.e. after at least one edge |

Prediction against measurement: `w` predicted; **`r_r` missed** — I predicted no promoted wire
outside the set because a wire read in a later segment crosses a tick, and forgot that a
pre-loop constant is a wire of segment 0 that is never assigned in the loop (I.7.2(b) makes it a
constant, not a register, whatever it crosses). The promotion has been there since the trailing
segment became the next resumption's head (step 4: "trailing_fold's `r` becomes `r_r`", §H).
`missing` side (stated, not pinned): 6 inferred registers have no flop of their own — the
single-phase absorptions, array and struct mappings the fixture test already allows.

### K.6 Predictions — the two transpiler fixes (P13 made true)

(a) **Hoist by liveness.** `transpile_fir` stops appending a hoisted cross-state local to
`fir.registers` when the inference does not name it; `chir_lower` lowers such a pre-loop `let mut`
as a wire whose pre-loop value is a zero literal of its type (the initializer is kept for the type
only, as before; as a wire it is read, if at all, only before an assignment of the same
resumption, which the inference says never happens). The FSM states' assignments to it forward
within the state, as any wire's do. Predicted: SV changes for **`nested_loop_added` only** (the
only hoisted local outside the set, K.5): the `w` flop and its `case` disappear, `q`'s drive is
unchanged; it agrees on all legs.

(b) **No carrier for a pre-loop constant.** A pre-loop non-`mut` `let` is a constant (never
assigned in the loop; Rust forbids it), so its comb wire has the same value in every segment;
`find_promoted_wires` skips it (a new `CHIRSeqBody::pre_loop_constants`). Predicted: SV changes for
**`trailing_fold` only** — every promoted pre-loop constant is outside the inferred set (a
constant is never `defined_in_loop`), so K.5 lists them all and lists one; its `r_r` flop
disappears and `q <= r` (= 8'd5) under `__copper_started`; it agrees on all legs.
After both: P13 holds on the corpus (0 surplus) and the fixture reconciliation passes with
`nested_loop_added` in it.

### K.7 The two transpiler fixes — measured

- **First attempt, a miss that would have been a defect:** demoting every hoisted local outside
  the inferred set also demoted the counted-`for` desugar's `__copper_ctr*` counters (declared in
  the loop by `desugar_counted_loops_in`, hoisted with the same marker, and outside the
  inference's domain — they have no source name). sv-baseline caught it: `uart_rx_dut` and
  `pulse_registered` changed, their counters turned into `always_comb` wires (a broken FSM). The
  demotion is now restricted to a **source** local (one the pre-desugar FIR mentions,
  `control_extract::stmts_mention`); the passes' own state stays a register.
- **sv-baseline after both (against the post-(2) snapshot): 1 changed + 2 new of 247** —
  `trailing_fold` (CHANGED: `r_r` and its update gone, `q <= r` under `__copper_started`) and the
  two new fixtures, as predicted. `nested_loop_added` emits no `w` at all (the wire is dead once
  its reads are forwarded; an empty `always_comb case (pc)` is left behind, cosmetic).
- **P13 on the corpus: 146 modules, 0 surplus** — `corpus_realized_registers_are_inferred` pins the
  empty set. Fixture reconciliation: **112** clocked modules, no violation (the two new ones in).
- **Verdicts:** `nested_loop_missed`, `nested_loop_added` 6 / 6 PASS, reference leg 864 cycles
  agreed each; `trailing_fold` (startup probe, `tests/reference_leg_measurements.rs`) PASS, the
  interpreter agrees over 16 cycles.
- One more miss while writing the fixtures: `nested_loop_missed` first used `fold_defect`'s
  `v + Bits::from_usize(i)`, and Verilator refused it (WIDTHEXPAND/WIDTHTRUNC — `from_usize(i)` is
  32 bits): the open `as_usize`/`usize` width class (I.14.7 (3), left alone by step 5). The swept
  twin reads `i` in a comparison instead; `fold_defect` keeps the verbatim I.7.5 spelling.

### K.8 Predictions — (5) P14, the hoisted initializer

The claim (SPEC I.7.7, the `chir_lower` comment on `HOISTED_CROSS_STATE_ATTR`, 2026-10-29):
a hoisted register's power-on value is stated (its constant, or 0 for a non-constant
initializer) because "a registered port can forward it before the state assigns it". I.7.4(d)
says no execution reads a loop-born local before its `let`, so the value must be unobservable;
P14 asks whether the realization agrees.

Probe: `COPPER_PROBE_HOISTED_INIT_UNSTATED=1` (`chir_lower::P14_PROBE_ENV`) drops the `initial`
line of every hoisted register — the source locals the hoist moves and the counted-`for`
desugar's `__copper_ctr*` counters, which carry the same marker — and the sweep runs under
`COPPER_VERILATOR_X_INITIAL=1` (`--x-initial unique`, seeded random power-on values for every
unstated register) with and without the probe. Only the SV of the modules that have a hoisted
register changes under the probe: **17 existing modules + the 2 new fixtures** (sv-baseline
under the probe; `trailing_fold` is in that list only because the snapshot predates K.7):
`det_for` (both copies), `serializer`, `rx`, `uart_rx` + `uart_tx` (system), `uart_rx_dut`,
`skip_on_halt`, `counted_delay`, `counted_repetition`, `capture_after_wait`, `rom_paced`,
`for_empty`, `for_empty_start`, `fold_defect`, `pulse_registered`, `nested_loop_missed`.

- Predicted: **no verdict changes** between the two runs for any of them: every hoisted
  register is assigned in its declaring state before any state reads it (a source local by Rust
  scoping, a counter by the desugar's entry assignment), so P14 holds — the comment's
  "can forward it before the state assigns it" does not happen. Any change would be an
  admitted-and-divergent row candidate.

### K.9 (5) P14 — measured

Both runs over the 16 `_differential` cases of the 15 swept modules with a hoisted register
(`det_for` twice; `fold_defect` and `pulse_registered` are not swept), under
`COPPER_VERILATOR_X_INITIAL=1`: run A without the probe, 16 models built (cache miss); run B with
`COPPER_PROBE_HOISTED_INIT_UNSTATED=1`, 15 rebuilt — every probed SV differs (the one cache hit is
`nested_loop_added`, which has no hoisted register after K.7). **Every verdict is identical: 16/16
PASS in both, the same sequence counts, 0 DISAGREE** — as predicted. **P14 holds**: no hoisted
register's power-on value reaches a trace, so the hoist's stated initializer is a realization
choice with no observable meaning (I.7.4(d), (e)), and there is no admitted-and-divergent row
candidate. The `chir_lower` comment's justification ("a registered port can forward it before the
state assigns it", `mac_pipeline`'s `product_r`) was wrong twice over: nothing forwards it, and
`product_r` is a promoted wire of the linear route, not a hoisted local; the comment now says so.
The probe switch stays (`chir_lower::P14_PROBE_ENV`), off unless set, so the measurement can be
repeated. Not covered: a seed other than 1, and the CPUs (neither has a hoisted register).

### K.10 A third over-inference: a `let` inside an expression block (found, fixed)

Found while reading the K.0 modules, not in the brief: `struct_latch_cond` inferred `{bumped,
latch}` and `tuple_destructure` `{doubled, latch, target}`. `bumped` is a `let` inside the block
of `let next = if c { let bumped = …; Latch { …, data: bumped } } else { … }`: `collect_reads`
counted every path identifier of the initializer, so the outer `let`'s node used `bumped` with no
definition, it was live back to the previous tick, and `DefinedInLoop` (which sees the inner
`let`) made it a candidate. Dead at every tick — a wire of I.7.1, the third over-inference of P3
besides the head clause and the fold. The reconciliation test passed because the single-phase
absorption allowance accepted the comb nets `next_bumped`/`doubled`.

Fix: `collect_reads` returns the FREE reads only — names bound by an inner block's `let`, an arm or
closure pattern, a `for` binding or an `if let`/`while let` pattern are out of their scope; a
shadowing read before its binding (`{ let x = x + 1; … }`) is visited first and stays a read of the
outer local; macro tokens still all count (over, never under). Outside liveness, `uses` feeds only
`derivation_facts`, a report. Pinned by `cfg::tests::a_let_inside_an_expression_block_is_scoped`.

**Not predicted in writing before the run** (a miss of method: I ran the inferred-set diff before
writing this subsection; my working note had named the CPU's decode temporaries as the likely
bulk). Measured:

- **register-baseline: 3 of 177** — `rv32i_cpu_pipelined` `−{addr, b, f3, f7, from_ex_mem,
  from_mem_wb, instr, link, op, rd, result, rs1, rs1_val, rs2, rs2_val, taken, tgt, w,
  writes_reg}` (19 decode/execute temporaries bound inside `let … = if/match { let … }` blocks),
  `struct_latch_cond` `−bumped`, `tuple_destructure` `−doubled`.
- **sv-baseline: 0 of 247.** The CPU's SV is byte-identical (its anchor untouched); the transpiler
  realized these as wires already.
- Poison run: **113 clean, 0 flagged, 77 not eligible** (the two new fixtures added). None of the
  three changed modules is leg-eligible (`SKIP` / `REF_SKIP`), so the evidence for them is the
  unchanged SV. Reconciliation 112 / P13 146 modules: no violation, 0 surplus.

### K.11 The bare regression — measured

First bare `tools/regression.sh` on `a88ff82`: **REGRESSION FAIL** — one test,
`tests/timing_model_derivations.rs::the_two_phase_derivations_agree_except_where_extraction_hides_them`,
whose extracted set gained `for_empty`, `for_empty_start` and `nested_loop_added`. Cause: the test
replayed desugar / extract / lower by hand and skipped `transpile_fir`'s register append, so step
5's `for_empty*` failed to lower there (the hoisted `__copper_ctr` was not a register and its
initializer did not lower as a pre-loop wire) and its `Err` arm skipped them silently. They had
never been compared. Hoist-by-liveness (K.7) gives such a local a zero-valued wire, so they lowered
and were compared. Fixed by exposing `copper_codegen::lower_fir_to_shir` (the transpiler's own
pipeline to SHIR, which `lower_fir_to_vlir` now calls) and using it in the test. All three are
extracted into one `match pc`, like every member, and were added with that reason (`3e5145c`).

Second bare run on `3e5145c` (laptop): **REGRESSION OK (257 s)**. 1220 passed, 0 failed, 89 ignored,
141 test binaries; examples **32/32**; G-C 114 test files; G-D **186** differential cases (184 + the
two fixtures), 3 ignored-with-reason (unchanged `SKIP` rows); G-F 113 modules, 76 skipped; **G-G
differential 1467 PASS / 0 DISAGREE** (1455 + 12, the fixtures' 6 + 6), **reference 967 PASS**
(955 + 12), **pinned 0**; G-E 5 anchored, 184 transpiler-only. **No new `SKIP`, `REF_SKIP` or `PARAMS`
row.** Anchors: no anchored module's SV changed in any sv-baseline diff of this step (`mac_fsm`,
`det_010`, `det_110101`, `lfsr`, `sync_2ff` + twins, the BaseJump examples, the two-clock pins, the
pipelined CPU's SV byte-identical through all three inference changes). Their tests ran and passed
in this run.

### K.12 For the coordinator — SPEC items (SPEC files untouched)

1. **P3 — row status: MEASURED, true on the corpus up to the stated direction.** Three
   over-inferences removed: the head clause (K.1: 1 local left of I.7.5's 8, `trailing_local_into_head`'s
   `w`), the fold (K.3: one miss fixed — `fold_defect`'s `v` — and 7 surplus locals in 3 modules
   removed), and a third class I.7.5 does not name, a `let` inside an expression block (K.10: 22
   locals in 3 modules). What remains is path-insensitivity: a counted `for` whose range is not a
   literal non-empty range keeps its zero-iteration exit, and macro tokens all count — over,
   never under. I.7.5 / I.7.6's map is stale: no head clause (`cfg.rs` `compute_registers`), the
   register set is computed on `Cfg::reg_nodes` (the register graph, `Builder::expand_loops`,
   `build_loop_expanded`), the folded node remains for every other rule, and `collect_reads`
   returns free reads. The obligations of I.7.6 met here: "delete the head clause", "descend into
   the nested loop's CFG", "two reconciliation fixtures" (`tests/fixtures/nested_loop_registers_dut.rs`),
   retire the unit pin `post_tick_def_read_pre_tick_is_a_register` (restated as `…_is_a_wire`).
   REG2/REG9/X9 and SS's back-edge sentence are SPEC/inventory text — for you.
2. **I.7.2(g) as realized:** the register graph models a named `for` binding as I.8.5's counter
   (set at entry, read by the header), so `i` is a register whenever the loop ticks, read after the
   tick or not — D-10's reading. A `_` counter is nameless and outside the set (`__copper_ctr`).
   I.7.2(g)'s "(D-10: …)" note says exactly this; worth a sentence that the analysis implements it.
3. **P13 — row status: MEASURED (corpus), true after two fixes.**
   `copper-codegen/tests/register_reconciliation.rs::corpus_realized_registers_are_inferred`,
   146 modules, pinned empty. The first run found two emitted, unobservable surplus flops (K.5):
   a hoisted source local outside the set (`nested_loop_added`'s `w`) and a promoted pre-loop
   constant (`trailing_fold`'s `r_r`). Fixed (K.7): the hoist still moves by name mention, but a
   hoisted SOURCE local is a register only if the inference names it, otherwise a wire of each
   resumption; the passes' own counters stay registers; a pre-loop constant gets no carrier
   (`CHIRSeqBody::pre_loop_constants`). I.7.6's "by name mention rather than liveness" is
   therefore now "by name mention, realized by liveness". I.7.7's P13 bullet closes.
4. **P14 — row status: MEASURED, true; no II.3 row.** Probe K.8–K.9: 15 swept modules, every
   hoisted register's power-on value unstated and randomized → identical verdicts. The claim in the
   code comment was wrong (and its example, `mac_pipeline`'s `product_r`, is a promoted wire, not a
   hoisted local); corrected. The probe switch `COPPER_PROBE_HOISTED_INIT_UNSTATED` stays. III.1's
   ASSERTED count drops by one (P14).
5. **P2 — re-run: 113 clean, 0 flagged, 77 not eligible** (§D.4 had 47). The row's "re-run over the
   corpus once D-8's rewrite puts the plain-`Out` modules in the leg" is done. `fold_defect` is now a
   positive by forcing (`Poisoned("v")`), no longer a flagged under-inference.
6. **R11's domain shrank — a decision for you (stopped, not changed).** `check_register_initializers`
   asks only about pre-loop `let`s in the register set. With the head clause gone, a pre-loop
   `let mut w = a.read()` that is a resumption wire (`trailing_local_into_head`'s shape with a
   non-constant initializer) is now ADMITTED by both front-ends where both refused it before.
   Measured on a throwaway fixture: it agrees on all three legs (6/6 PASS, the interpreter 864
   cycles) — the first resumption reads the initializer once, as I.7.2(c) says. But `s_pre`'s
   grammar (I.2.3) has no non-constant `let`, and I.7.4(c) / D-12's R11 already says the rule is to
   be widened to every pre-loop `let`. That widening is now the only thing that would refuse this
   shape. Parity holds either way.
7. **Two transpiler defects found and fixed on the way**, neither in II.3's table: the pre-loop wire
   path emitted a context-width literal at 64 bits (K.1; latent until a `let mut` was first demoted);
   and P13's two surplus flops (item 3). The `as_usize`/`from_usize` width class (I.14.7 (3)) was
   met again (K.7) and left alone.
8. **I.7.5's "added" shape in its own words is not transpilable**: `for _ in 0..3 { tick; w =
   x.read() }` is refused (a counted body's tick must be last; a wait loop's likewise). The fixture
   uses the tick-last `loop { w = x.read(); if … { break } tick }`. The refusal is the existing
   lowering limitation; I.7.5's example could use the tick-last spelling.
9. **A test that replayed the pipeline by hand** (K.11) hid two step-5 modules from its comparison;
   `lower_fir_to_shir` now gives tests the real pipeline. Any other hand-rolled replay is worth an
   audit (not done here).
10. **Anchors:** no anchored module's register set or SV changed in any of the step's diffs —
   `mac_fsm`, `det_010`, `det_110101`, `lfsr` (the `assert_source_registers_match_reference_sv`
   users), `sync_2ff` / `flag_sync` / `sync_2ff_concrete` ({ff1}), the pipelined CPU (SV
   byte-identical through all three inference changes). The consumers checked: R11 (item 6),
   `fir.registers` (sv-baseline per change), the G2 helpers (unchanged sets), the interpreter
   (consumes no register set; the poison run does, item 5).

## L. Phase C step 7 — P1's relation `R` built per module, P15 (2026-11-07)

Branch `claude/phase-c-projection-268525` (from `paper` at `eb980f5`). D-20 step 7: "The static
test of P1's relation `R` per module (beside `pc_states_vs_tick_sites`), the initial state
included (P15)." The test is `tests/projection_relation.rs::the_projection_relation_holds_on_the_corpus`;
its header states the method in full. In short:

- **Source side** — H(M) from the `syn::ItemFn`, by a continuation-stack walker of Rust control
  flow written in the test; a continuation is a tick site (copper-ref's index-path names, the set
  cross-checked against `copper_ref::static_tick_sites`) plus, per counted `for` the next
  resumption leaves or re-enters, which. No `copper_codegen` code on this side; the one shared
  piece is `copper_analysis::consts` (SPEC I.14.4), to decide a range's static (non-)emptiness.
- **Emitted side** — CHIR from `copper_codegen::lower_source_module` (new: the same passes as
  `transpile_source`, every intermediate form kept), SHIR for the phase register's next values,
  `__copper_started`'s update and the power-on values, the SV text for the `initial` block.
  Extraction route: state = the `match pc` arm; linear route: state = the SHIR phase (phase 0
  runs the trailing segment first under `__copper_started`); single-tick = the linear route
  with one phase, so `R` is `{(0,!started)~Entry, (0,started)~t₁}`, checked like any other.
- **Comparison** — label sequences per path: `Run(p)` an atomic source statement (emitted
  statements mapped to the outermost atomic source statement containing their span's start,
  consecutive duplicates collapsed), `Test(p,k)` a control `if`/`match`/`while` outcome (emitted
  `if`/`match` is control iff it assigns `pc`), `Count(p,·)` a counted `for`'s entry and
  last-iteration outcomes (the source chooses `v+1 ≥ end` before the tick that ends the
  iteration — the same program, since the test is pure; the emitted side abstract-interprets each
  synthesized counter as `advanced?` × `at end?`). For each reached pair the two path sets must
  be equal; matched paths give the successor pairs. `R` is the reachable product.
- **P15** — the initial pair is (power-on `pc`/phase, power-on `__copper_started`) ~ `Entry`;
  power-on 0 in SHIR and in the emitted `initial`; `__copper_started` declared whenever read, set
  to 1 at phase 0's edge only; only the initial pair has `started = 0`; `Entry` in no other pair.

### L.0 Predictions (written before the first run)

1. **P1 holds on every clocked module both front-ends admit (0 transpiler control defects).**
   Reason: the sweep is green on 186 cases (G-G 1467/0), step 5 closed the startup and counted-`for`
   rows (f1)–(f6), and §D.3's ratio spread is the many-to-many shape `R` is meant to absorb.
2. **P15 holds on both routes on every module** (step 5's `__copper_started`, `pc`/`phase_r` at 0).
3. **The first run will NOT be clean, and the failures will be the test's own normalization, not
   the transpiler**: (a) CHIR statements `chir_lower` synthesizes with the module span or a span
   inside no source statement (submodule wires, memory capture plumbing) — "maps to no source
   statement"; (b) source statements that lower to no CHIR statement (a statement macro, a
   compile-time `const { assert!() }`, a `let _ = …`) — a `Run` only on the source side; (c) the
   `while` guard's negation folded by `chir_lower` for `while !c` — "not a negation". Each will be
   classified as one of: a normalization the test must state (fixed in the test, written below), or
   a finding.
4. **No unreachable `pc` state and no unreached tick site** outside a statically empty `for`.
5. **`R` is many-to-many on the modules §D.3 named**: `det_010_awaits`, `if_tick`, `match_tick`
   (several sites → `pc = 0`), `counted_repetition`, `uart_rx` (a site over several states).

### L.1 The first run — measured, predicted vs measured

First run (`cargo test --test projection_relation -- --nocapture`, commit `82f6d91`):
131 modules related, **6 failures, 2 notes**. Every failure was the test's
own reading of the source, none a transpiler defect — prediction 3's direction, but not its
members:

| failure | class | resolution |
|---|---|---|
| `shift_register` (example and fixture): the pre-loop `const { assert!(N - 1 == N_1) }` is a `Run` on the source side only | (b), predicted in kind | normalization: a `const { … }` statement is evaluated by rustc and is no statement of any resumption (§J.1 already called it "a compile-time item that lowers to nothing"); the test classes it with the initializers |
| `if_tick_explicit`, `branch_merge_explicit`, `match_tick_explicit`: "an emitted control `if`/`match` … that is no source control construct" | NOT predicted | the test's route detection read a USER's `let mut pc; loop { match pc {…}; tick }` (a hand-written FSM, single-tick, linear route) as the extraction route's `match pc`. Fixed by deciding the route on the source: extraction iff some tick is not a top-level statement of the module loop; the CHIR must then have the extraction shape (else a failure) |
| `nested_continue_match`: "an emitted statement at 64:20 maps to no source statement" | (a), predicted in kind, wrong member | a non-block `match` arm (`_ => q.write(…)`) is lowered as a statement with the ARM's span (pattern start); the test now gives such an arm body the arm's range and label position |

Prediction 3(c) (`while !c` folded) did **not** occur: every `while` guard reaches CHIR as a
negation. Prediction 3(a)'s other members (submodule wires, memory plumbing with a module span)
did not occur: every CHIR statement in a resumption carries a span inside a source statement.

Second run: **137 related, 0 failures, 2 notes** (the 6 above were not counted in the 131).

### L.2 The measurement

`projection relation: 137 clocked modules related (41 extracted, 17 linear, 79 single-tick),
51 combinational (no H(M) state), 1 structural (no coroutine), 0 uncovered with a reason;
|R| = 455 pairs, 831 transitions checked; sweep modules examined: 189/189.`

- **P1 holds on all 137** (prediction 1: as predicted). Every transition of every reached pair,
  every branch outcome, matched path for path.
- **P15 holds on all 137** (prediction 2: as predicted): `pc` / `phase_r` / `__copper_started`
  power on at 0 in SHIR and in the emitted `initial`; `__copper_started` is declared wherever read
  and set to 1 at phase 0's edge only; the entry transition is `s_pre`'s expression statements then
  the head (the two CPUs and `preloop_probe` / `preloop_linear` are the modules with a non-empty
  `s_pre` run — §J.1's 5 less `shift_register`'s const block); only the initial pair has
  `started = 0`.
- **Coverage.** Every one of the 189 sweep modules (build.rs's `COVERED`, now also written to
  `corpus_manifest.rs`) is examined: the 137 clocked ones related — the sweep's `SKIP`ped
  `rv32i_cpu_pipelined` included (3 states, 2 sites) — 51 combinational ones (no H(M) state: a
  combinational module has no `tick`, SPEC I.11) and the structural `two_domain_top` (no coroutine,
  I.13) counted and printed. `UNCOVERED` is empty. The sweep does not reach `src/` (its
  `sync_2ff` is covered by the `tests/fixtures/` twin `sync_2ff_concrete`, single-tick), nor do
  this test or G-D.
- **Many-to-many, measured** (prediction 5: as predicted). Columns: `pc` states / tick sites /
  |R projected onto (state, site)| (the entry counted) / |R| (with counter facts and `for` choices)
  / transitions.
  Several sites, fewer states — `det_010_awaits` 4/6/7/7/26, `if_tick` and `match_tick` 2/3/4/4/7,
  `arms_only_match` 6/5/6/6/19. One site over several states — `counted_repetition` 4/1/5/5/15,
  `counted_delay` 2/1/3/3/6, `nested_loop_missed` 2/1. Both at once — the UARTs (`rx`, `uart_rx`,
  `uart_rx_dut` 11/5/11/17/49; `uart_tx` 9/4/10/17/46), `det_for` 3/3/5/12/54, `serializer`
  5/4/6/8/15. So `R` is a relation, never a function either way, as III.1.3 corrected I.0.4.
  On the linear route `R` is the phase index against the top-level ticks plus the entry
  (`|R| = phases + 1` on all 17 multi-phase and 79 single-tick modules).
- **Negative controls** (each a throwaway edit, measured and reverted; `copper-codegen` clean):
  (m1) `if` arms swapped in `lower_into` → 39 modules fail; (m2) the (f1) hoisted-tick guard
  removed → `det_for` ×2, `for_empty`, `for_empty_start` fail (exactly the run-time-bounded
  `for`s); (m3) the pre-loop statements dropped in `chir_lower` ((f3) re-introduced) → the two
  CPUs, `preloop_probe`, `preloop_linear` fail at `(0 !started) ~ Entry`; (m4) a ticks-only-in-arms
  nested loop's fall-through made a state transition (II.3 (b) re-introduced) → `arms_only_b2`,
  `arms_only_match` fail; (m5) `__copper_started` powered on at 1 → P15 fails on every module that
  declares it. The test is not vacuous on any of the five defect classes it is meant to see.

### L.3 The finding — dead `pc` states (unobservable)

`arms_only_b2` and `arms_only_match` (`tests/fixtures/arms_only_loop_dut.rs`, II.3 (b)'s fix of
step 5) each have **one `pc` state in no pair of `R`**: state 1, the nested loop's head state.
`lower_into` allocates a head (`sm.new_state()`) for every nested loop; a loop whose ticks lie only
in arms is lowered as written, its entry is the lowered body cloned inline, and its fall-through is
`FallThrough::ZeroTime`, a goto that `splice_zero_time_gotos` replaces by the head's statements.
So nothing ever assigns `pc = 1`: the arm is in the emitted `case (pc)` (measured on
`arms_only_b2`'s SV: `8'd1:` arms in all three `case`s) and is unreachable from power-on.
**Unobservable**: `pc` powers on at 0 and takes only assigned values; the dead arm is equal to
state 0's. Cost: one `case` item per `always` block and nothing else. Not fixed (removing the
state means renumbering after allocation; no behaviour changes). Recorded; printed as a `note:` on
every run.

### L.4 What the test cannot see

1. **The content of an atomic statement** — its expressions, the forwarding of registers the
   resumption assigned, widths, the `always_comb`/`always_ff` split. That is P8, the sweep (G-D,
   G-G); this test sees only that the statement ran, in order, on the path.
2. **Data.** A branch is a label, never evaluated: two tests of one condition are two independent
   labels on both sides (identically over-approximated). The counter facts (`advanced?`, `at
   end?`) are the only data tracked, plus a range's static emptiness from `copper_analysis::consts`.
   The emitted counter is incremented BEFORE the tick (`v_emit = v_src + 1` at the iteration's
   tick); that variable correspondence is a datum, not checked here.
3. **A statement run twice in a row** — consecutive equal `Run` labels are collapsed on both
   sides, because one source statement lowers to several CHIR statements (struct and tuple lets).
4. **Below CHIR.** `shir_lower::lower_seq_body`'s composition (phase `k` = segment `k`, the
   trailing segment first in phase 0 under `__copper_started`) is RESTATED, cross-checked only
   against SHIR's phase count, the guarded trailing updates and the phase register's literal next
   values; VLIR/emit are read only through the `initial` block. The `pc` register's emitted next
   value is the CHIR `pc` assignment, not the SV `pc <=` mux.
5. **Pre-loop `let`s** are initializers (I.9.2), not statements: the test checks no pre-loop `let`
   is lowered into a resumption, not that its value is the stated power-on value (P16, the
   register-initializer rule).
6. **Combinational and structural modules** have no H(M) state; they are counted, not related.
7. **The source side shares one assumption with the extractor**: that a counted `for`'s tick is
   its body's last statement (the walker errs, by name, if a back edge is reached without that
   tick). Both front-ends refuse the other shape (II.1), so no admitted module is outside it.

### L.4b The bare regression — measured

`tools/regression.sh` on `aee5bcd` (the code as committed; later commits are documentation and a
comment): **REGRESSION OK (515 s)**. 1221 passed / 0 failed / 89 ignored across 142 binaries; G-C
115 files; G-D 186 differential cases (3 ignored-with-reason); **G-I ok: 137 clocked modules
related (41 extracted, 17 linear, 79 single-tick), 51 combinational, 1 structural, 0 uncovered;
|R| = 455 pairs, 831 transitions**; G-F 113 modules; **G-G differential 1467 PASS / 0 DISAGREE,
reference 967 PASS**, pinned 0, excused none; G-E 5 anchored; examples 32/32 (BaseJump, the two
CPUs' 13 programs, the CDC pins, `sync_2ff`). No code path that emits SV changed
(`lower_source_module` and `lower_fir_to_chir` are a split of the existing pipeline), so every
anchored trace is unchanged by construction and measured passing.

### L.5 For the coordinator — SPEC items (SPEC files untouched)

1. **P1 — row status: MEASURED (consistency), true on the corpus.** Evidence:
   `tests/projection_relation.rs::the_projection_relation_holds_on_the_corpus` — `R` built for all
   137 clocked sweep modules on both routes (41 extracted, 17 linear, 79 single-tick), 455 pairs,
   831 transitions checked, every branch outcome; five negative controls (§L.2). Guard: G-I in
   `tools/regression.sh`. The open-obligation cell's "a static test that builds `R` per module and
   checks each transition" is met; mechanization (III.1.4) remains proposed. Still consistency
   between the source and the extractor's output, never correctness — it is the projection, not
   the realization (P8).
2. **P15 — row status: MEASURED, true on both routes** (was "MEASURED false"; step 5 made it true on
   the two probes; this measures it on every clocked module): same test. III.1.2's counts line
   ("measured false today 3 (P11, P12, P15)") wants re-deriving: P15 leaves it here, and P11/P12
   moved in steps 2–4 by their own records (§F, §H).
3. **I.0.4 / III.1.3 can now cite the measurement**: many-to-many measured per module (§L.2's rows),
   and `R` on the linear route is the phase index against the top-level ticks plus the entry — the
   form I.0.4 calls "`phase_r` is a position in the textual split", with the entry the second state
   of phase 0 (`started = 0`). I.0.8's carried item "the emitted machine encodes H(M) (a static test
   of `pc` states against tick sites …)" closes.
4. **A dead-state remark for I.0.7 / II.3**: `lower_into` allocates a head state for a nested loop
   whose ticks lie only in arms that no transition reaches (§L.3). Unobservable; not a II.3 row
   (no divergence); a sentence in I.0.7's map at most, if the SPEC wants the state count read
   exactly.
5. **The counted `for` in H(M)'s terms**: the test reads Rust's post-tick exit test as a choice made
   before the tick (the same program, the test being pure). I.8.4's desugaring text could state this
   reading; it is what lets the emitted machine's `pc` relate state-by-state to the source sites.
6. **I.9.2's "a `const { … }` block before the loop"**: no clause names it. It is admitted by both
   front-ends and is no statement of the first resumption (compile-time). I.2.3's `s_pre` grammar or
   I.9.3 could say so.

## M. Phase C step 8 — SPEC I.14.7 (1)–(10): data, X, widths (2026-11-07)

Branch `claude/phase-c-data-14b4e6` (worktree `phase-c-data-14b4e6`, from `paper` at `df02f00`).
D-20 step 8: "I.14.7 (1)–(10)". Instrument for every emission claim: `sv-baseline` (snapshot of
the untouched tree at `df02f00`, 247 modules; `SV_BASELINE_DIR` added so two snapshots can be
`diff -r`'d). The D-19 probes are D-19 session spec-packet-i14's, verbatim, now
`tests/fixtures/data_i14_dut.rs` (swept) or `copper-ref/tests/interp_data.rs` (integer ports,
which the sweep's generator cannot stimulate). Predictions were written to the session's
scratch `predictions.md` before each measurement and are restated here with the outcome.

### M.1 (1) `eq_logic` / `ne_logic` IEEE-exact; P6 as a recorded table (`b54018d`)

`Bits::eq_logic` (`copper-core/src/types.rs`) and `RefBits::eq_logic` now return 0 when a known
bit pair differs, else X if any bit is X, else 1 (I.14.2(h)). **P6, standing:**
`tests/ieee1800_table.rs` + `tests/data/ieee1800_4state.txt` — 36 operators × 400 X-sprinkled rows
(D-19 (i-b)'s generator and seed, plus `derived !=`, `nor`/`xnor_reduce`, `zero_extend`), the
IEEE result recorded from Icarus 12.0. `copper_matches_the_recorded_ieee_table` needs no tool;
`the_recorded_table_is_what_icarus_computes` re-runs Icarus when installed (absent = a printed
skip, broken = a failure) so the file cannot drift; `COPPER_IEEE_TABLE_REGENERATE=1` rewrites it.
**Predicted / measured:** after — EQUAL on all 14 400 rows ✓; the same table against the old
`eq_logic` (negative control, `types.rs` reverted) — `eq_logic` and `ne_logic` 191 EQUAL / **209
DIFFER** each, every other operator 400/400 ✓ (exactly D-19's 209). `bits_property.rs` green
after both changes ✓. sv-baseline: no change ✓ (the transpiler already emits `==`). The derived
`==` is pinned against `===` only (it is IEEE's `===`, I.14.2(b)).

### M.2 (5) `/`, (6) the X literal, (4) width 1 — emission

- **`/`** lowers as `%` does: CHIR/VLIR `Div`, `(a / b)`, signed operands through `signed_binop`'s
  `%` arm. The macro admitted `/` all along (D-19: "the macro admits every probe"), so R15 had no
  macro-side refusal to keep; the interpreter's R15 refusal is replaced by `RefBits::div` (M.6).
- **X literal:** `Bits::x()` / `Bits::<W>::x()` / `Logic::X` as a value lower to a new
  CHIR/SHIR/VLIR `XLit` (`'x` context-sized, `W'bx` sized). A **comparison** with one
  (`v == Logic::X`) stays refused by the transpiler with its own message — it asks whether a value
  is X, `is_valid()`'s class (I.14.3(d)); the macro admits it: a REFUSED-one break, recorded
  (M.9 item 4), pinned in `unsupported_constructs.rs`.
- **Width 1:** `v[0]` (or a dynamic index) on a width-1 operand lowers to the operand;
  `Logic::from`, `Bits::from`, `Bits::<1>::from`, `Logic::from_bool`, `.into()` to the identity
  (a type-turbofish width resizes).
- **Predicted:** no corpus module changes (none uses these; every corpus module transpiles).
  **Measured:** one change, a miss — `tests/x_propagation_and_reset.rs::resettable` (not swept;
  an inline test module) flipped from REFUSED to emitted: its pre-loop `let mut r = Bits::x()` is
  a wire since step 6 (assigned on every path before any read) and was refused as an expression;
  it now emits nothing for it (the dead X is never observed). Its test's assertion (the
  `held` module carries no X initializer) is unaffected.

### M.3 (3) integer widths — predicted, then measured

The change: `as_u8 … as_u128` / `as_usize` lower to a zero-extension to the integer's width (a
`Resize`, none when the receiver already has it); `usize`/`isize` are 64 bits (`resolve_type`,
the suffix tables, `from_usize`); integer casts are width-explicit (`as` truncates or
zero-extends by Rust's rule, I.14.3(c)). At an **index, a memory address or a shift amount** the
view is peeled syntactically (`peel_index_views`: `x.as_usize()`, `x.as_u128() as usize`) —
value-identical, since an in-range index is below 2^64 and a view never truncates — so a direct
`arr[x.as_usize()]` emits as before.

**Predicted** (scan `scan_usize.py`: 71 candidate modules with an `as_*`/`usize`/`from_usize` in
the body): changed ⊆ the 71; unchanged where the view is only a direct index; anchors —
`dual_port_ram` unchanged, the two RV32I CPUs change (`usize` lets) and stay trace-identical,
`bsg_mux_one_hot` may change; lint risk (UNUSEDSIGNAL / WIDTHEXPAND where 64 meets narrower).

**First measurement — 27 changed, 4 sweep failures (lint).** Each was a width collision the
64-bit `usize` exposed, not a meaning change; fixed in the transpiler, each fix re-measured:
1. `mux`: `data_i[128'(sel_i)]` — the CHIR-level peel cannot compare a parametric receiver's
   width → the syntactic peel above. 27 → 27 modules but `mux` back to byte-identical.
2. **The unsized-literal sentinel was 64** (`DEFAULT_LIT_WIDTH`): every literal balanced against
   a 64-bit `usize` looked unsized and was re-sized by its context (`r != 0` → `r != 1'b0`,
   `back & 0xff` → `back & 8'd255` on a 64-bit `back`, `i * 3` → `i * 16'd3`). The sentinel is now
   129 (no `Bits<N>` has it, I.14.1(c)), emitted as 64 by `vlir_lower::width_of` — **measured zero
   emission change on every existing module** (sv-baseline against the 64-sentinel tree); the
   comparison operands are no longer re-sized by a value context; `from_usize`'s argument keeps
   its literals' balanced width; a `from_uN` type turbofish names the result's width, not the
   argument's.
3. Verilator gives a size cast's width to its operand's leaves (`64'((w >> 7) & 31)` →
   WIDTHEXPAND on `w`): a widening of an operator tree is emitted `64'(32'(…))` (`widening_resize`).
4. A 64-bit dynamic select index (`data[(64'(i) + rot) % 64'(N)]`, WIDTHTRUNC): cast to 32 bits
   (`select_index`; an in-range index is below 2^32 — the width `usize` index arithmetic had).
5. An operand narrower than a `usize` it meets (the SV `int` loop variable, `parameter int`):
   zero-extended (`widen_to_usize`); a provably wider operator tree written to a narrower target
   is narrowed explicitly (`resize_to_target`).

**Measured after the fixes — 18 modules changed by (3)–(6), the sweep green (317 passed / 0).**
Every change is one of four shapes, all value-preserving:
- a `usize` local or register declared 64 bits instead of 32, its literals `64'd…`:
  `rv32i_cpu_pipelined`, `rv32i_cpu_transpilable`, `dynamic_shift`, `manual_sra`,
  `const_generic_init`, `ifchain_on_usize`, `match_on_usize`, `wide_index_into_narrow_addr`,
  `rotate_right` ×2 (`rot`, plus `64'(i)` / `64'(N)` against it), `arr_same_element`
  (`j = 2'(64'(i))`);
- a view in a match scrutinee takes the integer's width (`case (8'(sel)) 8'd0:` for `as_u8()`,
  `case (64'(s)) 64'd0:` for `as_usize()`): `arms_only_match`, `nested_continue_match`,
  `cdo_match_default_arm`, `cdo_match_every_arm`;
- a view's zero-extension in a non-index operand: `bits_methods_rest_comb` (`x >> 64'(lo)` in a
  dynamic `part_select`), `wide_index_sole_consumer` (`10'(64'(…))`);
- `resettable` (M.2).
**Anchors:** `dual_port_ram`, `bsg_mux_one_hot` and every other BaseJump/golden anchor
byte-identical by this item ✓ (`bsg_mux_one_hot` predicted "may change" — it did not: its view
is a direct index); the two CPUs changed as predicted; their 13-program checks are in the
regression (M.8). **Probes:** `usize_width` and `usize_annot` (ADMITTED-AND-DIVERGENT, 64/64
cycles) **agree on all three legs** ✓; `c_narrow`, `c_narrow2` (lint-refused as emitted) **agree**
✓; the step-5 `as_usize` width note in `bits_methods_dut.rs` and step 6's `from_usize(n & 0xFF)`
note (K.7) are this class, now closed.

### M.4 (7) the remaining row-(e) methods; `checked_*` / `is_valid` OUTSIDE

- `saturating_add` / `_sub` / `_mul` on an **unsigned** integer: `s = a + b; s < a ? MAX : s`,
  `a < b ? 0 : a - b`, the product at 2N bits clamped (`lower_saturating`). A signed operand, or
  one wider than 64 bits, is refused by the transpiler (the macro admits): REFUSED-one, recorded
  (M.9 item 4); corpus 0.
- `x.as_array_mut()[i] = l` is the element write `x[i] = l`; `as_array_mut()` as a value is the
  receiver. `from_array` / `from_slice` were already the identity (step 5); `Logic::from_bool`
  (M.2).
- **`checked_*` and `is_valid()` refused as OUTSIDE in both front-ends:**
  `copper_analysis::check_outside_methods` (new `data.rs`), one message each
  (`CHECKED_METHOD_MESSAGE`, `IS_VALID_MESSAGE`), run first in the macro (all modes, before the
  `Option` idiom check so the message is the shared one) and in `checked_fir_for_target`; the
  transpiler's lowering keeps a defensive copy. Step 5 had lowered `is_valid()` to `1`; that is
  retired. **Predicted:** the corpus passes except the one fixture that used `is_valid()`;
  **measured:** exactly `bits_methods_rest_comb` (rewritten to use `Logic::from_bool`) ✓.
  Witnesses: ui/fail `outside_checked_add`, `outside_is_valid`; transpiler pins in
  `unsupported_constructs.rs`; parity audit `front_end_parity.rs` (new tests 3–4, every
  `#[hardware]` fn in every mode).
- Fixture `data_methods_seq` (saturating ×3, `from_bool`, `as_array_mut`, `Bits::from(Logic)`):
  agrees on all three legs, 12/12 sequences.

### M.5 (8) width constraints as elaboration-time checks; (9) the memory port index

- **(8)** `copper-codegen/src/width_checks.rs`: each `const { assert!(c, "msg") }` becomes
  `if (!(c')) begin : __copper_width_check_k $error("msg"); end` in the module body, `c'`
  translated from Rust (parameters and consts by name, literals, `+ - * / %`, comparisons,
  `&& || !`, and a file `const fn` the shared evaluator measures equal to `$clog2` on 0..=4096 —
  the corpus's `safe_clog2`); an assertion outside that grammar emits no check and stays under
  III.2.3's precondition. **Predicted:** every corpus constraint translates (7 files, the
  `safe_clog2` and `N - 1 == N_1` forms), changing exactly those modules by the check block;
  **measured:** 7/7 checks emitted; sv-baseline changes `mux`, `priority_encode` ×2,
  `rotate_right` ×2, `shift_register` ×2 by the block only ✓ (anchored modules — their example
  self-checks run in the regression). Fixture `tests/width_constraint_check.rs`: `rotate_right`
  at `N = 8, N_LOG = 2` is **refused by Verilator** at elaboration (`%Warning-USERERROR … N_LOG
  must equal safe_clog2(N)`, exit 1); at the sweep's `N = 8, N_LOG = 3` it lints clean. Measured
  on the side: **Icarus 12 ignores an elaboration-time `$error` in a generate block** (exit 0, no
  message) — the check binds Verilator, the sweep's tool.
- **(9)** `copper_analysis::check_memory_port_index`, both front-ends, one message: a literal
  `read_port::<k>()` / `write_port::<k>()` with `k ≥ R` / `≥ W` of the memory's literal generic
  arguments (declared `Memory::<…>::new`, annotated `let`, or a `Memory<…>` parameter); a count or
  index that is a const generic is the instance's (rustc's run-time index panic in the simulator
  remains for that case). ui/fail `memory_port_index`; transpiler pin with its in-range twin;
  parity audit. Corpus: 0 refused ✓ (predicted 0).

### M.6 (10) the interpreter; P7

- `copper-ref`: integer and `bool` port payloads (`Ty::bit_width`, `Value::to_bits`/`from_bits`:
  two's complement LSB first, an X bit on such a port refused — it has no X, I.14.2(a)); `/` on
  `Bits` (`RefBits::div`); `Logic::from`, `Bits::from` (`Logic`, an integer, an array), `.into()`;
  the unsized `Bits::all_ones()` as `Value::LitOnes`, sized at first contact (D-19 (iv)'s parity
  item); integer `saturating_mul`; the `as_array_mut()[i] = l` place.
- `copper-ref/tests/interp_data.rs` (new): D-19's `int_wrapping` (u8 ports, modular, value and
  bit interfaces), `int_overflow` (`200 + 100` on `u8` a run-time failure — the simulator panics
  likewise, D-19 (i)), a `bool`/`i16`/`u8` module with `saturating_*` and `/`, and `From` / `/` /
  unsized `all_ones` on `Bits` — each against Rust's arithmetic. 4/4 pass.
- **P7** `copper-ref/tests/bits_property_p7.rs` (new) — I.14.8's list: the constructors (`zero`,
  `all_ones`, `x`, `from_lit` incl. hex/underscore/`u128::MAX`, `from_u8 … from_u128` at widths
  where they compile, `from_usize`, `from_array`, `from_slice`), the views `as_u8 … as_usize`
  (and the panic on X ↔ `None`), shift amounts to 3N+5 (logical, arithmetic, operator and method
  spellings), `Logic`'s `!` and `as_bool`, `From`/`Into` between `Logic` and `Bits<1>`,
  `as_array`, the operator set at widths {2, 3, 5, 7, 12, 33, 63, 65, 100, 127}, and the
  interpreter's integer/`bool`/array port values. 6/6 pass. What P7 still does not cover: N > 128
  (OUTSIDE), the interpreter's integer *operators* beyond the four kernels (they are the
  interpreter's own Rust `i128` code, compared to the simulator by the sweep only on `Bits`
  ports).

### M.7 (2) the constant evaluator — what landed, what waits on (A)–(C)

Landed (independent of the decisions): `eval_const_usize` — the transpiler's constant-index
evaluator, integer literals only until now — and `Bits::get(k)` fold I.14.4's constants through
the same `fold_const_init` / `ConstEnv` the register fold uses (file consts, associated consts,
`const fn`s, any base, underscores). **Predicted** 0–3 corpus modules change; **measured 0** (every
corpus constant index is a literal); fixture `const_index` (`x[DI_K]`, `x[DI_HI]`,
`x[di_half(DI_HI) - 1]`, `x.get(1_0 / 5)`) emits static selects and agrees on all three legs.
The six `k_*` probes of I.14.7 (2) (`k_hex_body`, `k_hex_init`, `k_underscore`, `k_ones_nowidth`,
`k_ones_width`, `k_ones_init`) **all agree on all three legs** — five closed by step 5's fold;
`k_ones_nowidth` needed the unsized `all_ones` sized by its sibling (`x ^ ~64'd0` was a lint
failure) and the interpreter's `LitOnes`. **Not done (the decisions):** R11 and the interpreter
on one evaluator. Packet: M.10.

### M.8 The fixture and the bare regression

`tests/fixtures/data_i14_dut.rs`: 20 modules (the 16 D-19 probes of (3)–(6) verbatim —
`w1_index`, `w1_from`, `w1_comb`, the six `k_*`, `c_narrow`, `c_narrow2`, `c_signed`,
`usize_width`, `usize_annot`, `xp_div`, `xw_port` — plus `xp_div_nz`, `data_methods_seq`,
`w1_from_comb`, `const_index`). **Predicted** all agree; `xp_div`'s SV pair OUTSIDE from a zero
divisor in some sequences; `xw_port` OUTSIDE from obs 1 on every sequence (its `ql` is X);
`k_ones_nowidth` a lint failure (it was, and was fixed). **Measured:** every module PASS on every
leg it has, except `xp_div` 4 PASS / **4 OUTSIDE** of 8 and `xw_port` 0 PASS / **8 OUTSIDE** of 8 —
never DISAGREE; the two combinational modules have no reference leg (by class, no row).

**Bare `tools/regression.sh` on `a01bd15` (laptop): REGRESSION OK (372 s).** 1280 passed, 0 failed,
91 ignored, 146 test binaries (G-C: 119 test files); examples **32/32** (both RV32I CPUs, every
BaseJump anchor, `mux`, `rotate_right`, `priority_encode`, `shift_register` with their width
checks); G-D **206** differential cases (3 ignored-with-reason, unchanged `SKIP` rows); G-F 131
modules, 78 skipped with a reason; **G-G differential 1578 PASS / 12 OUTSIDE / 0 TRUNCATED / 0
EXCUSED / 0 DISAGREE** — the 12 OUTSIDE are exactly `xp_div`'s 4 and `xw_port`'s 8; **G-G reference
1088 PASS / 0 DISAGREE**; **G-I 155 clocked modules related** (41 extracted, 17 linear, 97
single-tick), 0 uncovered, |R| 491, 867 transitions; G-E 5 anchored, 204 transpiler-only. New
`SKIP` / `REF_SKIP` / `PARAMS` rows: none. Not run remotely (no push in this step), so the
372 s is a laptop number, not a paper number. sv-baseline against `df02f00`: 23 changed (M.2,
M.3, M.4, M.5, each explained there), 20 new (the fixture).

### M.9 For the coordinator — SPEC items (SPEC files untouched)

1. **I.14.2(h) realized**: `eq_logic`/`ne_logic` IEEE-exact; I.14.2's anchor sentence ("209/400
   rows more X") is history now — the standing check is `tests/ieee1800_table.rs` (P6: recorded,
   tool-optional). III.1's P6 row can move to MEASURED with that evidence.
2. **II.3 rows to close (the next II.3 edit, I.14.8's list):** `/` (R15) — lowered; II.1's R15
   becomes no rule (the macro never refused it; the interpreter's refusal is gone). `Bits::x()` /
   `Logic::X` in an expression — emitted as a value. The passthrough/`usize` widths
   (`usize_width`, `usize_annot`, `c_narrow`, `c_narrow2`) — closed. The width-1 select
   (`w1_index`, `w1_comb`) — closed. Row (e): `eq_logic`, `ne_logic` (step 5), `as_array_mut`,
   `from_array`, `from_slice`, the `From` conversions, `Logic::from_bool`, `saturating_*`
   (unsigned), `all_ones` (every spelling) — closed. Row (g)'s underscores — closed (step 5).
3. **II.1 rows to add:** the memory port index (I.14.6(c), `check_memory_port_index`);
   `checked_*` and `is_valid` (OUTSIDE, `check_outside_methods`). R16's N bound (`Bits<N>` N > 128
   or 0) is still not a rule in either front-end — not reached here.
4. **New REFUSED-one breaks, recorded, not fixed** (the macro admits, the transpiler refuses;
   corpus 0 each): (a) a comparison with the X literal (`v == Logic::X`) — the `is_valid` class;
   the natural fix is to refuse it in `check_outside_methods` too (a decision: is `== Logic::X` the
   derived `==`'s structural question and therefore OUTSIDE like `is_valid`?); (b)
   `saturating_*` on a signed integer or on more than 64 bits — fix: a signed clamp form.
5. **I.14.1(b) realized**: `usize`/`isize` 64 bits in the transpiler. I.14.5(b) realized: the views
   are zero-extensions. Flag for I.14.5(a) or I.14.7's map: the emitted SV keeps the `int` (32-bit)
   loop variable and `parameter int`, zero-extended where they meet a `usize`, and narrows a
   64-bit dynamic select index to 32 bits — a realization note, not a meaning.
6. **I.14.5(d) realized** for the corpus's constraint forms; the translation grammar (above) is
   narrower than I.14.4's; an untranslatable assertion leaves III.2.3's precondition in force for
   that module (none in the corpus). Icarus does not run the check (measured).
7. **The unsized-literal sentinel** (M.3 fix 2) is a transpiler internal, no SPEC text; recorded
   because it was a latent defect class that any 64-bit type would have hit.
8. **III.2.3's "≤ 64-bit harness port"** and the sweep's generator: the generator stimulates
   `Logic`/`Bits` ports only, so the five `u8` payload ports of the corpus and every integer- or
   `bool`-port probe are outside the sweep; the interpreter now supports them (M.6). Teaching the
   generator is a separate item.

### M.10 Decision packet — (A) constants in the macro, (B) one evaluator, (C) R11's scope (STOPPED)

Nothing below is implemented; each needs your decision. Measurements by a scratch scan over
`examples/`, `src/`, `tests/` (`syn` + `copper_analysis::{is_constant_expr, infer_registers}`):
**195 clocked modules; 243 pre-loop `let` initializers** (memories and clock clones excluded),
**all 243 constant-shaped** by R11's test, **0 non-constant** (register or wire). By kind: a
`Bits` constructor 166 (`zero` 130, `from_lit` 17 — three non-decimal/underscore, `x` 5,
`from_u32` 4, `all_ones` 2, `from_u8` 1), integer literal 35, `Logic` variant 14, enum variant or
associated const 14, array 9, a const/const-generic name 4, an associated-fn call 8 (7 are the
**non-`const`** `T::bubble()` struct constructors of `rv32i_cpu_pipelined` and
`struct_pipeline_dut`, 1 the `const fn` `CeCfg::half`). Constants: **93 `const` items in 35
hardware files** (`usize` 38, `u32` 22, `&str` 16 — test text, `u8` 4, arrays 8, `bool` 1),
**8 `const fn`s, 0 `Bits`/`Logic`-typed const items.**

**(A) Constants in the macro.** R11 (`check_register_initializers`) runs in the macro on the
annotated `ItemFn` only — a proc macro is not given the file — so it decides constant-*shape*
(`is_constant_expr`), not constant-*value*. Today it under-admits a single-segment `const fn`
call (`let mut n = f(3)`, refused by both; corpus 0) and over-admits a non-`const` associated fn
(`T::bubble()`, 7 corpus initializers, which the transpiler folds structurally).
- **A1 — keep the syntactic test** (status quo). Changes: nothing. Cost: the `f(3)` under-admission
  stays (corpus 0); the over-admission stays (7 initializers, harmless today: the transpiler folds
  them).
- **A2 — admit a single-segment call with constant-shaped arguments** in `is_constant_expr`
  (symmetric with the associated-fn case). Changes: one arm in `copper-analysis`. Effect: `f(3)`
  admitted by both; a call to a *non*-`const` free fn is then admitted by the macro and refused
  by the transpiler's `ConstEnv` (a REFUSED-one break, corpus 0). Cost 0 modules.
- **A3 — make rustc the judge: the macro emits each register initializer in a const context**
  (`let mut n = const { <init> };`, an inline const block, which may name the fn's const
  generics), keeping the syntactic test only for its message. Changes: every `Bits`/`Logic`
  constructor in `copper-core/src/types.rs` a `const fn` (`zero`, `all_ones`, `x`, `from_lit`,
  `from_u8 … from_u128`, `from_usize`, `from_array`; their loops as `while`); the 7 `bubble()`
  constructors `const fn` (2 files, one the anchored pipelined CPU — source-only change, its SV
  unaffected); the macro's expansion of every pre-loop register initializer (243; the simulated
  values are unchanged). Effect: R11 exact — every I.14.4 constant admitted (file consts, `T::K`,
  `const fn`s), every non-constant refused by rustc. The transpiler still needs its own fold
  (rustc's evaluation is not visible to it); parity between the two is then measured by the
  sweep (sim = rustc). **Not prototyped**: whether every constructor compiles as `const fn` on the
  workspace's toolchain (the `from_lit` post-mono assert and the `[Logic; N]` loops are the risk).
- **A4 — give the macro the file's items** (a file- or module-level registration macro, e.g.
  `copper::consts! { … }`, or an attribute on an enclosing `mod`). Changes: how users write a
  hardware file; the macro evaluates with `ConstEnv`. Cost: a new authoring form; every corpus
  file with consts (35) would opt in to benefit.
- **Recommendation: A3**, with A2 as the fallback if A3's `const fn` constructors do not compile.
  A3 is the only option where "constant" means rustc's constant — D-19's 4A ("Rust's constants")
  and the overview's "the simulator is rustc" — and it closes both directions (the `f(3)`
  under-admission and the `bubble()` over-admission) at a measured corpus cost of 7 `const`
  keywords.

**(B) One evaluator (I.14.4's direction, I.14.7 (2)).** Two evaluators exist: the interpreter's
(`copper-ref` `Exec::eval_const` / `lookup_const`: typed `Value`s, `Bits` widths, X, coercion to the
declared type) and `copper_analysis::consts` (`i128`, checked, integer/`bool`, `ConstEnv` over
const items, associated consts, `const fn`s), which the transpiler's fold, its counted-`for`
bounds and (since this step) its constant indices use; R11 uses neither (A).
- **B1 — the interpreter's typed evaluator becomes the shared one** (moved into `copper-analysis`
  or a new crate both depend on, with `RefBits`/`Value`). Changes: ~1 000 lines move; the
  transpiler's fold and `eval_const_usize` call it. Effect, measured on the sweep's patterns
  (G-G): a defect in the shared evaluator would then make the interpreter and the SV agree
  against the simulator (rustc), and the ledger would print it as `sim≠ref,SV=ref` — "simulator
  /macro" — a misattribution: the reference leg stops being independent of the transpiler for
  constants. The typed power (X and `Bits` widths in a constant) is used by **0** corpus const
  items.
- **B2 — one grammar, two evaluators, pinned against each other**: keep the interpreter's
  evaluator independent; the `copper-analysis` one is the front-ends' and the transpiler's;
  add a standing differential test evaluating every corpus constant both ways (93 const items,
  243 initializers, the counted-`for` bounds and constant indices) — and the sweep already
  compares both against rustc through the simulator. Changes: one test; I.14.4's sentence "one
  evaluator … used by R11, the transpiler's fold and the interpreter" reworded to "one grammar,
  the interpreter's evaluator independent of the front-ends'" (SPEC text — flagged, not edited).
  Cost 0 modules.
- **B3 — status quo** (two evaluators, no pin). Changes: nothing; I.14.4's direction stays unmet.
- **Recommendation: B2.** The reference leg's value is that it shares no code with the
  transpiler (`copper-ref`'s header; III.2's patterns attribute a disagreement by which legs
  agree); B1 trades that for a unification no corpus constant needs.

**(C) R11's scope.** Since step 6 (§K.12 item 6), R11 asks only about pre-loop `let`s in the
register set, so a pre-loop `let mut w = a.read()` that is a one-resumption wire is admitted by
both front-ends and agrees on all three legs (step 6's throwaway fixture, 6/6 PASS, 864
interpreter cycles) — but I.2.3's `s_pre` grammar has no non-constant `let`, and I.7.4(c) / D-12
said R11 is to be widened to every pre-loop `let`. Measured here: **0 corpus modules have a
non-constant pre-loop `let`** of either kind.
- **C1 — widen R11 to every pre-loop `let`** (the grammar as written). Changes: `init.rs`'s
  candidate set (all pre-loop `let`s, not the register set) and its message (it names a
  register today); both front-ends; a ui/fail case. Effect: the wire shape is refused although
  it has a meaning and agrees. Cost 0 corpus modules.
- **C2 — widen I.2.3 instead**: `s_pre` admits `let x = e` with `e` pure over the first
  resumption's inputs when `x` is not a register (it is then a wire of the first resumption,
  I.7.2(c); D-11 already put the pre-loop statements in the first resumption); R11 stays the
  power-on rule for registers. Changes: SPEC I.2.3 / I.7.4(c) text only; a fixture pinning the
  shape (step 6's, made permanent). Effect: admission of a non-constant pre-loop `let` depends
  on register inference — as R11's verdict already does.
- **C3 — status quo**: the code admits what the grammar does not — a recorded spec/code mismatch.
- **Recommendation: C2** — the shape has the coroutine's meaning, all three legs compute it, and
  R11's reason (a register needs a power-on value the SV can state) does not apply to a wire.
  C1 is the choice if `s_pre` should stay minimal by design.

## N. D-21 — the constants decisions A3, B2, C2 implemented (2026-11-07)

Branch `claude/phase-c-consts-867296` (from `paper` at `c1735ab`). The user decided §M.10 as
recommended: **A3** (rustc judges a register initializer), **B2** (one grammar, two evaluators,
pinned), **C2** (R11 for registers only; a non-register pre-loop `let` is a wire of the first
resumption). Recorded as D-21 in `SPEC_DECISIONS.md`. Commits: `a4a32d0` (A3 + C2's fixture), `22771f0`
(B2 + G-J), then docs.

### N.1 A3 — the prototype, then the change

**Prototype first (the packet's risk).** Every constructor of `copper-core/src/types.rs` became a
`const fn` — `zero`, `all_ones`, `x`, `from_array`, `from_u8 … from_u128`, `from_usize`, `from_lit`,
and `Logic::from_bool` — with `from_uint`'s `for` a `while` and `N.min(128)` an `if`. On rustc 1.92 all
compile; a scratch example evaluated each inside `const { }`, including `Bits::<N>::zero()` under a
fn's const generic, `U::Z` through a trait bound, `from_lit::<1>()` with `N` inferred from a later use,
`[Bits::<8>::x(); 2]`, a file `const fn` and `S::W`. `from_lit`'s post-mono assert and the
`[Logic; N]` repeat were no obstacle. **A3 held; no fallback to A2.** One cost: a `const fn`'s panic
takes a literal message, so `from_usize`'s run-time panic no longer formats the value
("value does not fit in Bits<N> (from_usize)"); no test matched the old text.

**The change.** The macro (`copper-macros/src/lib.rs`, `const_wrap_register_initializers`) wraps the
initializer of every pre-loop `let` in the register set in `const { … }`, spanned at the initializer.
R11's shared half (`copper-analysis/src/init.rs`) now refuses only an initializer that **reads a
run-time value** (a parameter or an earlier pre-loop binding), with R11's own message — rustc would
refuse it too (E0435), but its text is about `const` items. The transpiler adds
`check_register_initializer_calls_in`: every call in a register initializer must be a core
constructor (`is_core_const_ctor`) or a `const fn` of the file (`ConstEnv::is_const_fn`) — rustc's
E0015, which the fold alone would miss because it inlines an associated ctor structurally.

**Predicted:** the workspace builds except the 5 `bubble()` definitions (the packet's "7" counted
initializers: 4 in `rv32i_cpu_pipelined`, 3 in `struct_pipeline_dut`); simulated values unchanged; the
emitted SV unchanged corpus-wide. **Measured:** the first build failed on exactly the 4 CPU
`bubble()`s (E0015 at each `let mut … = X::bubble()`) and then the fixture's; with `const fn bubble()`
the workspace builds. **sv-baseline against `c1735ab` (snapshot from a clean worktree of it): 267
modules, no changes** — the anchored pipelined CPU byte-identical.

**What a user sees** (ui `.stderr`s):
- a non-`const` free fn (`register_init_not_constant.rs`, `let mut acc = seed();`):
  `error[E0015]: cannot call non-const function `seed` in constants`, pointing at `seed()`, with
  rustc's note "calls in constants are limited to constant functions, tuple structs and tuple variants";
- a non-`const` associated ctor (`register_init_non_const_ctor.rs`): E0015 on `Latch::bubble()`;
- an input read (`register_init_reads_input.rs`, `let mut acc = a.read();`): R11's message, "register
  `acc` is initialized from a method call that reads a run-time value, so the hardware has no power-on
  value for it: … Initialize `acc` with a Rust constant — a literal, a `const` item or associated const,
  a call to a `const fn` … — or with `Bits::x()` …".
A proc macro cannot attach a note to rustc's error, so E0015 stands alone; it names the call and says
it must be `const`, which is the fix. Judged clear enough; the run-time read, where rustc's E0435 text
("consider using `const` instead of `let`") misleads, keeps R11's message.

**Parity, measured on the same inputs.** `copper-codegen/tests/register_init_parity.rs` transpiles the
macro's own trybuild files: `ui/pass/register_init_rust_constants_ok.rs` (a file const, a free `const fn`
`seed_of(3)`, `Cfg::K`, `Cfg::twice(Cfg::K)`, a `const fn bubble()` struct register) is admitted and
its `initial` block states `via_fn = 8'd10`, `assoc = 8'd11`, `assoc_fn = 8'd22`, `from_const = 8'd7`;
each `ui/fail/register_init_*.rs` is refused with R11's reason (the run-time read: the shared
message; `seed`, `Latch::bubble`: "which is not a `const fn`"). The older
`ui/pass/register_init_constants_ok.rs` is **not** transpilable, for two reasons unrelated to R11 (N.5).
`front_end_parity.rs` gained `r11_halves_refuse_their_witnesses_and_admit_their_twins`.

**Swept** (`tests/fixtures/const_init_dut.rs`, 3 modules): `init_const_fn` (`Bits::from_u8(ci_seed(3))`,
`let mut k: u8 = ci_seed(5)` — refused by both before D-21), `init_file_const` (`CI_START`, and
`CI_STEP = CI_START / 30`), `init_assoc_const` (`CiCfg::K`, `CiCfg::twice(CiCfg::K)`). Predicted all
PASS on all three legs; measured all PASS (N.6).

### N.2 C2 — the wire fixture

`tests/fixtures/preloop_wire_dut.rs`: `preloop_wire_read` (step 6's throwaway, §K.12 item 6:
`trailing_local_into_head` with `let mut w: Bits<8> = a.read();`) and `preloop_wire_first_head`
(single tick: `let mut w = a.read(); loop { q.write(w); tick; w = a.read() + 1 }`). Both are I.7.2(c)'s
wire — reassigned in the loop before any read that follows a tick. Predicted PASS on all three legs;
measured PASS (and in N.6's regression). R11 asks neither (not registers); the macro does not wrap
them (they are not in the register set).

### N.3 B2 — the differential test

`tests/const_evaluators_agree.rs`. Entry points added, sharing no code: `copper_ref::ConstScope`
(`Module::new`'s item collection + `Exec::eval_const`, `eval_as` coercing to a `let`'s annotation) and
`ConstEnv::eval_expr`. Over the 126 files of `examples/`, `src/`, `tests/` that define a `#[hardware]` fn:
(1) every `const` item — file scope, `impl` blocks (`T::K`), and every nested container (a `mod`, a fn
body, the harness's included) over the file's items plus its own; (2) every pre-loop initializer of a
clocked module, against the transpiler's **stated** power-on value (the SHIR register's `init`, what the
`initial` block prints; struct registers field by field; an X start against an unstated one; an enum by
declaration index) and against `ConstEnv` wherever it evaluates; (3) every counted-`for` bound and
every index (`x[e]`, `x.get(e)`, `x.set(e, _)`) of every `#[hardware]` fn, both ways. Both evaluate →
equal or DISAGREE (fails); one only → an `ASYMMETRIC` row (exact set); neither → counted. Fails on an
empty class. Writes `COPPER_CONST_EVAL_REPORT`, read by **G-J** in `tools/regression.sh`.

**Predicted** (written before the first run, notes file `phase-c-consts-d21.md`): ~65 integer const
items compared, 0 disagreements, the `&str`/array consts outside both, 0–5 asymmetries; ~200 register
initializers vs the transpiler, 0 disagreements; ~30–60 bounds; 100+ indices, 0 disagreements.
**Measured:** const items **68 compared**, 16 outside both (11 `&str` test-text consts and 5 `usize`
arrays — two `RATIOS`, `two_clock_pins`' `P`/`Q`/`R`); 84 items in all against the packet's 93 (the
packet's scratch scan is not in the tree, so the 9 are unidentified; every container this test walks
is listed in its header); pre-loop initializers **221 stated registers vs the
transpiler + 42 vs `ConstEnv`**, 1 a const generic (stated as the parameter, `init_param`), 45 not
compared, each listed with its reason on every run (3 array-typed registers; 29 not constants — 27
`Memory` bindings and the 2 C2 wires reading `a`; 16 non-register constant wires outside `ConstEnv`'s
integer grammar — register files (memories in SHIR) and `Bits` constant wires; 4 in
`mem_latency_probe.rs`'s 2 modules the transpiler refuses); **91 bounds** compared, 15 not constants;
**68 indices** compared, 106 not constants; **0 DISAGREE**. Asymmetries: **7, pinned** — the 4
`rv32i_cpu_pipelined` `X::bubble()` initializers and the 3 `struct_pipeline_dut` `Latch::bubble()`s,
which the transpiler states field by field and the interpreter cannot evaluate ("unknown struct
`Self`", a `Self { .. }` literal in an associated fn — the gap build.rs's `REF_SKIP` rows for
`struct_latch` / `struct_latch_cond` / `tuple_destructure` already record). Prediction misses: indices
fewer (most corpus indices are locals), no item/bound asymmetry at all.
**Negative control:** `apply_binop`'s `+` changed to `a + b + 1` → **16 DISAGREE**, the test fails; reverted.

### N.4 Found, then DECIDED H1 — the held pre-loop read

Writing C2's fixture, a second shape was probed: `preloop_held_read`,
`let x = a.read(); loop { q.write(x); tick }`. **Predicted** (from I.7.1): `x` fails I.7.1 (i) (never
assigned in the loop), so the analysis calls it no register and R11 does not ask; the transpiler emits a
wire `x = a` of every resumption; the simulator and interpreter hold the first read → `sim=ref≠SV` from
obs 2. **Measured** (a throwaway fixture, removed): `infer_registers` = `[]`; the SV is
`always_comb x = a; … o <= x;`; agreement relation **6 sequences, 6 DISAGREE, pattern `sim=ref≠SV` at
cycle 1** (e.g. sim 115 ref 115 SV 72); the reference leg 864 cycles agreed with the simulator. Both
front-ends admit it today (since step 6 removed the head clause, §K.12 item 6 — before, `x` was live at
the loop head, a register, and R11 refused the read); **corpus 0**.

Why C2 does not settle it: C2's wire is I.7.2(c)'s (reassigned in the loop before any read after a
tick); `preloop_held_read` is I.7.2(b)'s shape — "a constant" by I.7.1, a classification that assumed
`s_pre`'s initializers were constants. With a non-constant initializer it is state the residue must
carry. Recorded OPEN in SPEC I.2.3; options:
- **H1 — refuse it**: R11 extended from "a register" to "a pre-loop `let` live at a tick" (registers ∪
  the held shape); one shared function, R11's message; a ui/fail case; corpus 0.
- **H2 — give it its coroutine meaning**: I.7.1 (i) counts a pre-loop `let` with a non-constant
  initializer that is live at a tick as state — a register loaded by the first resumption (assigned
  before any read, so no power-on value is needed, like I.7.2(d)'s loop-born register); the transpiler
  captures it under `!__copper_started`; a swept fixture. Corpus 0; a transpiler and an analysis change.
- **H3 — record it as an ADMITTED-AND-DIVERGENT II.3 row** until one of the above.
Recommendation: **H1** — it is the smallest change, it is R11's own reason (a value the hardware must
hold has to come from somewhere it can state), and it refuses nothing in the corpus; H2 if `s_pre`
should mean everything the coroutine means.

**N.4.1 Both prototyped (user: "explore both", 2026-11-07).** One detector shared by both,
`copper_analysis::held_preloop_values`: a pre-loop `let` the module loop never assigns but mentions,
whose initializer reads an `In` port or an earlier such binding; clock and `Memory` bindings excluded.
Predictions were written first (notes file `phase-c-consts-d21.md`).

| | **H1 — refuse** (branch `claude/phase-c-consts-h1`, `9112ef0`, `f5ec675`) | **H2 — register loaded by the first resumption** (branch `claude/phase-c-consts-h2`, `326bdbf`, `5f1b14e`, `5d9153d`) |
|---|---|---|
| Change | `check_held_preloop_values` in both front-ends, one message ("`x` is computed once, before the module loop, from a value read at run time, and read again inside the loop …"); ui/fail `held_preloop_read`; `front_end_parity` corpus scan | `Cfg::registers` counts a held value; R11's halves and the macro's `const { }` wrap skip it; the transpiler rewrites `let x: T = e;` → `let mut x: T = <X>; x = e;` (no `initial`, loaded under `!__copper_started`); the projection test reads a held `let` as a first-resumption statement; the B2 test skips it |
| Emitted SV of the witness | refused | `o <= (!__copper_started ? a : x); x <= (!__copper_started ? a : x);` — no power-on value for `x` |
| Witness and twins | refused by both | `preloop_held_read` 6/6, `preloop_held_two_ticks` 6/6, `preloop_held_cond` (chained `y = x + 1`, read on one path) 8/8 PASS on all three legs |
| Corpus | 272 `#[hardware]` fns scanned, **0 flagged** (predicted 0) | register sets of existing modules unchanged; sv-baseline: only new modules (predicted 0 changed) |
| False positive found | **both**: the first full run failed `unsupported_constructs::preload_reading_a_signal_is_rejected` — a `Memory::from_fn` preload reading a port was caught by the detector before I.10's "Initial contents must be constant" rule; fixed by excluding `Memory` bindings (not predicted) | same |
| Bare regression (after the fix) | **REGRESSION OK (443 s)**: G-G 1608 PASS / 12 OUTSIDE / 0 DISAGREE, ref 1118, G-I 160, G-D 211, G-J 0 DISAGREE, examples 32/32 | **REGRESSION OK (409 s)**: G-G 1628 PASS / 12 OUTSIDE / 0 DISAGREE (+20, the 3 fixtures), ref 1138, G-I 163, G-D 214, G-J 0 DISAGREE, examples 32/32 |
| Size | analysis +1 rule, both front-ends 1 call each | analysis register set, R11, macro wrap, transpiler source rewrite, 2 test walkers |
| Prototype limits | the detector is syntactic (a `let` in the loop that shadows the name counts as an assignment — under-refuses) | as H1's detector; the rewrite handles `Logic`/`Bits<N>` held values only, the type annotated or a port read; the projection test now uses the analysis's detector (a shared-code caveat for an independent check) |
| SPEC text it needs | R11 (II.1) widened: "a register, or a pre-loop `let` live at a tick"; I.2.3's OPEN note closes as REFUSED | I.7.1 (i) amended: a pre-loop `let` with a non-constant initializer that is live at a tick is a register with no power-on value (assigned by the first resumption, like I.7.2(d)'s loop-born register); I.7.4 gains that case; I.2.3's OPEN note closes as IN |

**Decided H1 (user, 2026-11-07): merged into `claude/phase-c-consts-867296`**; H2's branch is kept, unmerged. The
recommendation stood at **H1** on size and on what it asks of the spec (R11's existing reason); H2
is measured to work and gives the shape its coroutine meaning at the cost of a second kind of
register (one with no power-on value set before the loop).

### N.5 Found on the way (recorded, not fixed)

1. **A `u8` file const in a body expression emits as a 32-bit `localparam int`**:
   `c = c.wrapping_add(CI_STEP)` → `c <= (c + CI_STEP)` with `localparam int CI_STEP`, a `-Wall`
   WIDTHEXPAND/WIDTHTRUNC failure (the fixture uses `wrapping_add(1)` instead; the initializer path is
   unaffected — it folds to `8'd3`). A REFUSED-one break (Verilator refuses the SV). Corpus: not
   measured beyond the fixture.
2. **`ui/pass/register_init_constants_ok.rs` does not transpile**: a `match` as an expression with no
   wildcard arm ("unsupported construct"), and element writes `bits[0] = bits[1]` on a
   `[Logic; 4]` register refused as "would infer a latch" (II.3 row (c)'s note: a packed vector of
   one-bit elements). Both REFUSED-one; the second is the row-(c) class.
3. **The interpreter's `Self { .. }` gap** (N.3's 7 asymmetries) — an interpreter front-end item
   that would move three `REF_SKIP` rows; not touched here.
4. **The transpiler's call half is narrower than rustc** where its fold is: a `const` method call
   (`K.wrapping_add(1)`), a tuple-struct constructor, a `static` read — rustc admits each in
   `const { }`, the transpiler refuses (a lowering error, never a dropped value). Corpus 0 each; II.2.2's
   R11 row says so.

### N.6 The bare regression

**Bare `tools/regression.sh` on `22771f0` (the final code commit; laptop): REGRESSION OK (559 s).**
1296 passed, 0 failed, 91 ignored, 148 test binaries (G-C: 121 test files); examples **32/32**; G-D
**211** differential cases (3 ignored-with-reason, unchanged); G-F 136 modules, 78 skipped with a
reason; **G-G differential 1608 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0 DISAGREE**; **G-G
reference 1118 PASS / 0 DISAGREE**; **G-I 160 clocked modules related** (41 extracted, 18 linear, 101
single-tick), 0 uncovered; **G-J** as N.3, 0 DISAGREE; G-E 5 anchored, 209 transpiler-only.
**Predicted** against `a01bd15`'s numbers (§M.8): +5 swept modules (3 `const_init_dut`, 2
`preloop_wire_dut`), each 6 sequences on both comparisons, nothing else moving. **Measured:** G-D +5
(206 → 211), G-G differential +30 PASS (1578 → 1608), reference +30 (1088 → 1118), G-I +5 (155 →
160, the two wire fixtures linear, the three init fixtures single-tick), G-F +5; the 12 OUTSIDE
unchanged (`xp_div`, `xw_port`). New `SKIP` / `REF_SKIP` / `PARAMS` / `RESET` rows: none. Not run
remotely (nothing pushed), so 559 s is a laptop number. sv-baseline: N.1 (267 modules, no changes;
the fixtures are new modules, not changes).

## O. Phase C step 9 — the reference interpreter's remaining legs (2026-11-07)

Session phase-c-interp, branch `claude/phase-c-interp-6c865c` (cut from `paper` at `65c0f17`; nothing
pushed). D-20 item (9): the interpreter gets its memory, combinational and design legs, so the
excusing rows of III.2.10 shrink to the rows a sentence explains. One commit per leg: `b5e75e6`
(memory), `fa72765` (combinational + the body gaps), `560907a` (design), `13e7b1c` (G-F, III.2.10).
`copper-ref` still depends on `copper-core` and `copper-analysis` only — no transpiler code (D-21 B2).
Predictions were written before each first run (scratchpad `predictions.md`) and are quoted here.

### O.1 Leg 1 — the memory model (SPEC I.10, D-16)

`copper-ref/src/memory.rs`, derived from I.10's text with each behaviour cited: an array, per read
port a staged address `s_k` and an `L`-stage pipeline `P_k`, per write port a staged `w_j` and an
`M`-stage `Q_j` (I.10.1(a)); `read`/`write` stagings are effects of the resumption, last wins
(I.10.1(b), D-16 (b) B); the edge runs the read step and the write step in the mode's order
(I.10.1(c), I.10.2: ReadFirst read-then-write, WriteFirst write-then-read; higher write port wins);
`is_ready()`/`data()` read the output stage the last edge left, `data()` unready and an out-of-range
address `Runtime` failures (I.10.1(d), D-16 1A, I.14.6(b)); the initial array of `new`/`from_contents`/
`from_fn` (I.10.3), every pipeline empty. A binding outside `s_pre` is refused (I.10.1(a)); a
`Memory` **parameter** stays `Deferred` (I.10.5: the owner's storage). The user's decision (b)
(§G.3) needs no code: the two anchors are already array registers and the 13 unanchored modules
already carry the +1 cycle in their source spelling, which I.10.1(d) gives.

**Predicted:** the 17 small memory modules agree on every sequence; `rv32i_cpu_transpilable` stops on
some front-end gap of a 400-line body (low confidence). **Measured:** all **18** agree (the CPU
included — the CPU prediction was wrong), 0 truncated; 832–1312 cycles each, observation 0 compared
on every sequence. The 18 `REF_SKIP` memory rows are deleted. Hand-derived traces:
`copper-ref/tests/interp_memory.rs` (latency 1 and 2, unready `data()`, out-of-range address, last
staging wins, ReadFirst ≠ WriteFirst at a collision, `M = 2` commits at edge N+M, higher write port
wins, the two refusals) — 9/9.

**Teeth (mutations of the model, run on the whole leg, then reverted):**

| mutation | modules that disagree |
|---|---|
| the write step in I.10.1(c)'s sentence order (commit the oldest stage, then shift and take `w_j`) | 8: `ram_arms`, `wide_index_into_narrow_addr` (1 cycle), `ram_r1w2_wf`, `ram_r2w2`, `ram_r3w1`, `ram_read_first_mem`, `ram_write_first`, `ram_wf_priority` |
| the mode order swapped | 7: every RAM with a write except `wide_index_*` |
| latency ignored (observe stage 0) | 2: `ram_r2w2`, `ram_r3w1` (the `L > 1` RAMs) |

`rv32i_cpu_transpilable` is caught by none: under the sweep's random `rstn` its fetch and load never
collide with a store and its trace does not depend on the write order — a weak witness for memory
semantics, though it does exercise the model end to end.

**Finding (SPEC wording, O.7 item 1).** Read in order, I.10.1(c)'s write step ("commits the oldest
stage of every `Q_j` … then shifts `Q_j` and takes `w_j` into stage 0") puts a write staged before
edge N+1 into `A` at edge N+M+1. The same clause then states "a write staged in that resumption is
in `A` from edge N+M", and I.10.2's ReadFirst/WriteFirst distinction at `M = 1` needs the write to
commit at the edge its read captures. The interpreter implements the stated consequence (take `w_j`
into stage 0 and shift, then commit stage `M-1`); the literal order is the first mutation above and
disagrees with the simulator on 8 RAMs. Not a design question — the clause's consequence and I.10.2
settle it; the procedural sentence's order is the text to fix.

### O.2 Leg 2 — the combinational leg (SPEC I.11, D-14)

`Module::evaluate`: one evaluation runs the body once from a fresh environment (every local a wire
of that evaluation, I.7.2(e)) over the settled reads (I.11.1); writes set `Out` wires, last wins and
the dead write admitted (I.11.2(b)–(c)); a never-written port is refused at construction and a path
that leaves a port unwritten at the evaluation (I.11.3, D-14 A3); the fragment has no `loop`/`while`/
`break`/`continue` (I.2.7) and no `Clock`, `RegOut` or `Memory` parameter (I.11.1, I.10.6). The
harness evaluates once per vector (the vector is the settle); there is no observation 0 for a module
without a clock (the sweep records none). The combinational class of `ref_skip_reason` is deleted.

**Predicted:** all 52 agree, except 3–6 that stop on interpreter front-end gaps (BaseJump idioms);
no disagreement. **Measured, first run:** 199 of 206 ok; **7 stopped, all front-end gaps, 0
disagreements**: `const { assert!(…) }` refused by the construction scan (`mux`, `priority_encode`
×2, `rotate_right` ×2) and a bit assigned into a still-unsized `Bits::zero()` whose width rustc
infers from a later use (`gray_to_binary`, `ripple_carry_adder`). Fixed in the interpreter, no
meaning changed:
- `const { assert!(c) }` is elaboration (I.2.4): erased at run time as before, and now evaluated at
  construction with the interpreter's own constant evaluator at the bound parameters — a failing
  one is a `Parse` error, as rustc's monomorphization error is (pinned:
  `interp_comb.rs::a_failing_const_assert_refuses_the_instance`). This also closes the
  `shift_register` row (×2).
- an element assignment into an unsized literal sets that bit of the literal, which is sized at
  first contact (I.14.4); an index past the width it is later given fails when it is sized.
- `Self { .. }` in an associated fn names the impl's type (closes `struct_latch`,
  `struct_latch_cond`, `tuple_destructure`; N.5 item 3) — and G-J's seven `SELF_GAP` asymmetry
  rows with them (O.6).

**Measured, second run:** **211 of 211 eligible modules run, all agree; `REF_SKIP` is empty.** The 52
combinational modules: 256–260 vectors each (the truth table up to 8 bits). Mutation —
first-write-wins in place of I.11.2(b)'s last write: all six `cdo_*` cases disagree (128–254 of
~260 vectors), plus 5 clocked modules through I.6.2. Pins: `copper-ref/tests/interp_comb.rs` 5/5. The
poison run (`reference_leg_measurements.rs` (iii)) now reaches the memory modules: 157 clean,
0 flagged (combinational modules not eligible — no suspension).

### O.3 Leg 3 — the design leg (SPEC I.13, D-18)

`copper_ref::Design` (`copper-ref/src/design.rs`): a `structural` module is run as the design it
names (I.13.4(a)) — its body read as nets (`net`/`wire`/`registered_wire`, the `init` no value of the
net, I.13.4(c)) and instantiations of `#[hardware]` fns of the same file, each a `Module`; one driver
per wire, every net driven, payload types equal on both ends (I.13.1); a combinational cycle refused
naming its instances (I.11.4, D-14 A4). `settle()` evaluates every function once in a topological
order of the network from the sources (I.13.2(a)); `pre_edge(clk)` / `post_edge(clk)` are I.13.2(b)'s
cycle for the domain of the parent clock `clk` — settle, resume that domain's coroutines, edge (their
pending values and memories), settle.

The sweep has one structural module, `two_domain_top`, two-clock and `SKIP`ped; the harness drives
one module, not a design, so the design leg runs in its anchor's test.
**Predicted:** `two_domain_top` through `Design` equals the hand-wired simulation (`sim_trace`) at
2:1, 3:1, 1:1. **Measured:** identical at all three (10, 10, 16 slow cycles) —
`tests/two_domain_hierarchy_cdc.rs::reference_interpreter_design_matches_sim`. The 2:1 trace is the
one the independent `two_domain_hierarchy.sv` reference reproduces under Verilator, so the design leg
agrees with an anchor at 2:1. Pins (hand-derived): `copper-ref/tests/interp_design.rs` 4/4 — the Mealy
pair of I.11.5 (`obs_N(rdy) = ~full_N | drain_N`, and `obs_0 = f(init, x_1)`), one settle in
topological order whatever the body's order, the cycle refused, the second driver and the undriven
net refused.

**Multi-clock fell out at the design level and not below it.** A domain is the clock a child
receives, and only that domain's coroutines resume at its edge (I.12.1), so a two-clock parent of
one-clock children runs by cycling each clock. What step 10 still owes (I.12): (i) a kernel with two
`Clock` parameters stays `Deferred` in `Module::new` (corpus: none swept — the sweep `SKIP`s every
module with two clocks); (ii) the sweep harness drives one clock, so a two-clock design reaches the
reference leg only through a hand-written test like this one; a generated two-clock case needs a
stated clock ratio per module (a `PARAMS`-like table) and a testbench that drives two clocks;
(iii) a structural parent in the sweep at all needs the harness to hand-wire the design (or the
sim-as-unit form, I.13.8) — the interpreter side is done.

### O.4 Leg 4 — the classes, G-F, III.2.10

- `ref_skip_reason`'s **plain-`Out` class measured 0** once combinational modules were exempted (R8
  refuses a plain `Out` on a clocked module in both front-ends) and is deleted; the combinational
  class is deleted with leg 2. The remaining classes (a `Memory` parameter, a second clock, a
  structural parent) excuse 0 swept modules — each is reached only by a module `SKIP` names — and are
  kept as guards with I.10.5 / I.12 / I.13.4(a) sentences.
- **G-F extended:** build.rs counts the reference-leg cases it generates to run, by leg
  (`REF_LEG_EXPECTED` = 211 / 52 combinational / 18 memory-binding);
  `corpus_generated.rs::the_reference_leg_covers_every_leg` fails if any leg is empty and writes
  `COPPER_REF_LEG_REPORT`; the driver requires the `_reference_leg … ok` count to **equal** the
  generated run count, and the design leg's test to have run and passed (`--quick` checks the count
  too). Before, G-F failed only on zero.
- **III.2.10** re-derived from the code (the only SPEC text edited): `SKIP` 3, `RESET` 3, `REF_SKIP`
  0 rows, the signature classes excusing 0, the reference leg on 211 of 214.

**Remaining `REF_SKIP` rows: none.** The three reference-leg cases still ignored are the sweep's own
`SKIP` rows (`mod8`, `rv32i_cpu_pipelined`, `two_domain_top`), each with its sentence;
`two_domain_top` runs on the design leg outside the sweep.

### O.5 Findings

- **Simulator or transpiler bugs: none.** Every new reference-leg case agrees with the simulator on
  every sequence, and the simulator agrees with the SV on all of them (G-G differential unchanged);
  no `sim≠ref` pattern of any kind.
- **Interpreter gaps** (front end, no meaning): three, fixed (O.2). One interpreter-side refusal kept:
  `write_port::<j>().is_ready()` (the simulator answers `true`) has no clause in I.10, so the
  interpreter refuses it; corpus uses: 0.
- **SPEC wording:** I.10.1(c)'s write-step order (O.1). Settled by the clause's own consequence and
  I.10.2, measured by the mutation.

### O.6 The bare regression

The first bare run, on `13e7b1c`, **failed in G-J**, as that test is built to: closing the
`Self { .. }` gap made its seven `SELF_GAP` `ASYMMETRIC` rows stale (the `bubble()` initializers of
`struct_pipeline_dut` ×3 and the pipelined CPU ×4 now evaluate in both evaluators). Not predicted —
I had not looked for consumers of the gap outside `REF_SKIP`. The rows are deleted (`3f9fa29`); each
field now compares both ways, 0 DISAGREE.

**Bare `tools/regression.sh` on `3f9fa29` (the final code commit; laptop): REGRESSION OK (563 s).**
1393 passed, 0 failed, 16 ignored, 151 test binaries (G-C: 124 test files); examples **32/32**; G-D
**211** differential cases (3 ignored-with-reason, unchanged); **G-F 211 modules** (136 at §N.6),
coverage `run=211 clocked=159 memory=18 combinational=52 skipped=0`, design leg ran; **G-G
differential 1608 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0 DISAGREE** (unchanged, as
predicted — no SV-side change); **G-G reference 1620 PASS / 0 DISAGREE** (1118 at §N.6: +502
sequences, the 75 newly-run modules); pins 0; **G-I** 160 clocked related, 0 uncovered (unchanged);
**G-J** 0 asymmetric, 0 DISAGREE; G-E 5 anchored, 209 transpiler-only. Ignored tests 91 → 16 (the 75
`REF_SKIP`/class-ignored reference-leg cases now run). Not run remotely (nothing pushed).

### O.7 For the coordinator — SPEC items (SPEC files untouched except III.2.10)

1. **I.10.1(c):** the write step's sentence order contradicts its stated consequence ("in `A` from
   edge N+M") and I.10.2's collision; it should read "takes `w_j` into stage 0, shifting `Q_j`, and
   commits the stage that reaches `M-1`, in ascending port order" (O.1; the literal order measured
   against the simulator: 8 RAMs disagree).
2. **I.10.7 / I.10.8:** obligation (1) — the interpreter's memory model, the `REF_SKIP` memory rows
   deleted, `Deferred` narrowed to the parameter — is discharged; I.10.8's "the interpreter's memory
   leg and its guard count (G-F)" closes. I.10.7's "`copper-ref` defers … a binding or port access"
   is stale.
3. **I.11.6 / I.13.7:** the interpreter now has a combinational leg (`Module::evaluate`) and a design
   leg (`Design`); I.13.7's "`copper-ref` runs one coroutine; a design is an environment of III.2"
   is realized, and I.13.2(a)'s topological single evaluation is what `Design::settle` does.
4. **III.2.8's table:** the `sim = SV ≠ ref` row still cites `regout_trailing_single_tick`'s
   `REF_SKIP` row, gone since step 4 (the module agrees on three legs).
5. **I.14.4:** an element assignment does not size an unsized `Bits` value (it sets a bit of the
   literal, sized at the first contact that fixes a width) — the interpreter's reading; one sentence
   would state it.
6. **II.1 parity:** `write_port::<j>().is_ready()` exists in the simulator's API and in no clause.
7. **III.1 P5 / I.13.2(c):** order independence now has a third witness — the interpreter's settle
   evaluates each function once in a topological order and agrees with the levelized simulator on
   every combinational module and on `two_domain_top`.

## P. Phase C step 10 — the rules (2026-11-07)

Session phase-c-rules, branch `claude/phase-c-rules-bab023` (cut from `paper` at `853ae75`; nothing
pushed). D-20 item (10): the II.1.3 parity fixes and the audit's extension (P10); the CDC two-flop rule,
the unit domain removed, R10 (I.12.6 (2)–(5)); the static combinational-cycle refusal at spawn (I.11.6,
I.13.7 (4); P5); the structural shape rules and `net::<T, D>()` (D-18); the II.2.2 directions narrowed
or pinned (P22); and step 9's two-clock leftovers (§O.3). One commit per part: `c38a7cb` (parity),
`6ae104e` (CDC), `286eb03` (combinational cycles), `b0b61c5` (structural), `1bab280` (directions),
`dfaef28` (two clocks). Every probe verdict was predicted before it was run (scratchpad
`probes/predictions.md`); the emitted-SV effect of each part was measured against a baseline
transpiler built from `853ae75` over every `#[hardware]` module the CLI lists (271–273).

### P.1 Part 1 — front-end parity (SPEC II.1.3, P10)

**Baseline (853ae75), the transpiler over the macro's ui battery:** it **admitted 10 of 30** `ui/fail`
cases — `cdc_crossing`, `cdc_memory_parameter`, `function_typed_mut_input`,
`function_typed_mutable_ref_input`, `function_typed_unit_return`, `multiple_top_level_loops`,
`param_shadowing`, `retired_pretick_opt_out`, `sequential_unprotectable_read` (R19), and refused
`clock_construction` only by "cannot infer bit width" and `unsupported_idiom_panic` by "macro invocation
is not supported". The two `cdc_*` admissions are a parity break II.1.3 never listed (`check_cdc` was the
macro's alone). `ui/pass`: 7 of 8 admitted (`register_init_constants_ok`, known, §N).

**Probes for the rows the battery did not cover** (14 predictions, 14 hits): R5 (tick-free `for` to a
run-time bound) M admit / T admit (an `always_comb` `for (… _i < k …)`); R6 (recursion through a helper)
M admit / T "cannot inline recursive function"; R7 `break` of the module loop and `return` M admit / T
refuse; R10 both admit; R12 labelled `break` M admit / T refuse, valued `break` M admit / T refuse **by
accident** ("a tick inside a `let` initializer"); R13 `assert!` and R14 `println!("{:?}", a.read())`
M admit / T "macro invocation"; R15 `/` both admit (D-19 gave it a meaning, step 8 its emission — closed,
no rule); R16 tuple local both refuse, T "cannot resolve type"; D3 `Some(t)` both refuse, T "cannot
infer bit width"; `c_mixed` and `sync_1ff_oj` both admit (Part 2).

**The change.** The macro's syntactic rules move to `copper_analysis::front_end` and both front-ends
run them first, in one order, through `check_front_end_rules(f, mode)`: the header R20 (with R9's flag,
parsed by one `parse_hardware_args` both use), **R10** (new), R17, I.14.3's OUTSIDE methods, the memory
port index, D3, R16, R18, **R7**, **R12**, **R5**, **R13/R14** (new), then the mode's rule (the CDC port
rule; Part 2's two) and R19. R19's message no longer cites the retired read injection. Every moved and
new message names its clause (II.1.1's obligation; 13 `.stderr` files changed by exactly that line).
R13/R14 admit `matches!`, anything in `const { }`, and `vec![…]` as `from_contents`' argument (I.2.4's
DESUGARED preload). R5 is D-21's pattern: a shared syntactic half (each bound constant-shaped: a
literal, a const item's name, a const generic, arithmetic over them), rustc's verdict in the macro (each
tick-free `for` bound evaluated in `const { }`), `ConstEnv` in the transpiler.

**After:** the 14 probes are refused by both front-ends with one message, except R6 (macro admits —
**stopped**, P.8), R15 (admitted, correct) and the two Part 2 shapes. The battery
(`copper-codegen/tests/front_end_parity_ui.rs`, new): **every `ui/fail` case refused by the transpiler
with the macro's first line**, 3 named rustc-judged cases excepted (E0015 ×2, Part 4's E0308); every
`ui/pass` case admitted, `register_init_constants_ok` the one named row. New ui/fail: `module_loop_break`,
`module_return`, `two_clocks_sequential`, `labelled_break`, `valued_break`, `body_macro`,
`input_read_in_macro`, `runtime_for_bound`.

**Corpus:** every new rule refuses **0** modules (predicted 0). The 24 tick-free `for`s of the corpus all
have constant bounds (`0..N` 6, `0..4` 5, `0..WIDTH` 4, `1..N` 2, `0..N_1` 2, `0..8` 2, `1..WIDTH`,
`1..8`, `0..ELS`), and the whole tree compiles with them under `const { }`. `front_end_parity.rs` gains
`every_corpus_module_passes_every_shared_rule_in_its_mode`: 272 fns (71 combinational, 193 sequential,
7 synchronizer, 1 structural) pass the shared rules, R2/R3 on a clocked body and D2. **SV: 272 modules,
0 verdict changes, 0 byte changes.** Two `unsupported_constructs.rs` tests had encoded the transpiler's
old order (a probe without an output port; the no-tick case owned by reachability); each now gets the
header's message first, as the macro always gave.

### P.2 Part 2 — CDC (SPEC I.12.6 (2)–(5), D-17)

**Already closed by steps 2–3 (checked, not changed):** (1) the never-ticked startup —
`tests/two_clock_pins.rs::never_ticked_startup_one_resumption_agrees_on_all_three_legs` asserts B's
trace `[0, 2, 3, 4]` (`q = 2` after B's edge 1) with sim ≡ SV ≡ ref; P11's "false today" is stale;
(6) `sync_2ff`, `flag_sync` and the fixture in the tick-last spelling; (8) `cycle::<D>()` per domain
(the same test asserts 3 and 3).

**(2) The two-flop rule** — `front_end::check_two_flop`, run on every `synchronizer` by both front-ends:
a taint of the resumption's locals from foreign reads (an `In` whose domain is not the clock's), cleared
at a `tick`, the module loop walked twice (a resumption crosses the back edge), arms joined by union;
refused: a `RegOut` write whose value or guard is tainted. **Predicted:** the corpus's synchronizers
admitted, and the anchor test's deliberate `sync_1ff` refused (I.12.3 makes it a refusal pin).
**Measured:** exactly that — `tests/cdc_synchronizer_anchor.rs:258` was the one refusal; it is now
`a_one_flop_synchronizer_is_refused_by_both_front_ends` (both messages equal; the library spelling
admitted). `sync_2ff`, `flag_sync` (×2), `sync_2ff_concrete`, the `structural_hierarchy` copy,
`sync_2ff_tick_last` and the `sync_3ff` doc test pass. ui: `sync_one_flop` (D-17's `sync_1ff_oj`),
`sync_one_flop_guard` (`sync_guard` — refused through its value, the guarded assignment tainting `ff1`).
`src/sync.rs`'s header corrected (the single-bit sentence is `sync_2ff`'s signature; the requirement is
this rule).

**(3) The combinational one-domain rule** — `front_end::check_combinational_one_domain`: predicted 0
corpus, measured 0; ui `combinational_two_domains` (`c_mixed`).

**(5) R10** landed in Part 1.

**(4) The unit domain — STOPPED.** Removing `D = ()` from `In`/`Out` and bounding `D: ClockDomain` on
the port types refuses **32 corpus `combinational` modules (100 ports)** that write `In<T>` / `Out<T>`:
10 in `examples/` — `bsg_adder_one_hot`, `bsg_encode_one_hot`, `bsg_gray_to_binary`, `bsg_mux_one_hot`
(four of the five BaseJump combinational anchors), `full_adder`, `mux`, `one_bit_comparator`,
`priority_encode`, `ripple_carry_adder`, `rotate_right` — 16 in `tests/fixtures/`, 6 in `tests/*.rs`.
D-17 (iii) measured 0 because it counted the explicit `()` only. The spec does not owe these rewrites
(it believed there were none), and making them needs a choice (P.8). The struct change is reverted; the
scaffolding the spec named (the copper-core port tests, `copper-core/tests/port_and_type_edge_cases.rs`,
the executor's unit tests) is given explicit domains, which is right under either option.

**SV:** 271 modules, 0 changes.

### P.3 Part 3 — combinational cycles refused statically (SPEC I.11.4, I.11.6, I.13.7 (4); P5)

**The change.** `HardwareExecutor::refuse_comb_cycle` runs at every spawn under **both** schedulers: it
computes the SCC plan the levelized settle uses and panics at the spawn that closes a multi-task
component, naming its modules (`HardwareModule::__named`, which the macro now sets). D-14 reversed item 6
phase 5's "detection is not rejection", so `iterate_scc` is deleted and the levelized settle's
multi-task branch is `unreachable!`. The transpiler refuses the same cycle over a structural parent's
nets (`chir_lower::refuse_structural_comb_cycle`: combinational children only; a `RegOut` drive adds no
edge).

**Predicted:** corpus 0 cycles (D-14: no example wires two combinational modules into each other); the
cycle tests flip to refusal pins. **Measured:** exactly those — the executor's oscillating pair,
three-module cycle and convergent OR-latch (now `*_is_refused_at_spawn`, the three-module one asserting
the message names all three tasks), a new fixpoint-mode refusal test, `simulator_corner_cases`' inverter
pair (`` `invert_a` (task 0), `buffer_a` (task 1) form a combinational cycle ``), and
`levelized_differential`'s convergent latch (refused by both schedulers). `golden_traces`,
`poll_order_fuzz`, `two_domain_hierarchy_cdc`, `two_clock_pins`, `two_domain_counter_behavioral`,
`received_memory_abi`, `module_composition_hybrid`, `executor_guardrails` pass unchanged. The
transpiler: `structural_hierarchy.rs::combinational_cycle_through_a_structural_parent_is_refused` (a
ring of `inv` and `pass` refused naming `inv_0`, `pass_0`; a chain and a loop broken by a sequential
child's `RegOut` admitted). **SV:** 271 modules, 0 changes.

**Direction:** exact on the graph the testbench declares (`reads`, `dirties`); **under** for a cycle
through reads it does not declare — `executor.rs::fixpoint_oscillating_loop_trips_threshold` keeps the
fixpoint oracle's run-time oscillation guard for exactly that, and says so.

**Relation to the design leg.** Step 9's `copper_ref::Design` refuses the same cycle when it builds a
design from a structural body, naming the instances (§O.3). There are now three independent
implementations of one refusal, one per realization of I.13.4(a): the hand wiring (the executor, from
the declared graph, at spawn), the emitted hierarchy (the transpiler, over the parent's nets, while
lowering), and the interpreter's design (over the parsed body, at construction). None shares code with
another. P5's open item closes: the settle is refused, not iterated, wherever the network is not a DAG.

### P.4 Part 4 — structural shape rules and `net::<T, D>()` (SPEC I.13.4, D-18)

**`net::<T, D>()`** (`copper_core::port::net`): a net with no initial value whose driver end is a sealed
`NetDriver<T, D>` — `Out<T, D>` or `RegOut<T, D>` — inferred from the child port it reaches. The macro
writes the inferred argument (`net::<T, D>()` → `net::<T, D, _>()`), so the source spelling is the
spec's, and wraps each instantiation in `copper_sim::__instance`, so a plain `fn` call is a type error.
A structural body is never executed; `net` panics if called, saying so.

**`front_end::check_structural_body`**, both front-ends, one message each (the S0 rows of I.13.4(d)):
the body is `let n = net::<T, D>()` and `child(arg, …)` only, each argument a parent port, `n.0`, `n.1`,
or a `.clone()` of a clock, a parent `In` or a reader end; `wire`/`registered_wire` refused with their
own message (retired from the structural grammar; they stay the testbench's); no instance of the module
itself; every net driven by exactly one instance; every `Clock` threaded to a child. The transpiler's
`parse_wire_net` reads `net::<T, D>()` only — the substring match that admitted `registered_wire` by
accident (D-18 `s6b`) is gone.

**Predicted against D-18's table** (M / T, today → after): `s1` statements in the body M admit → refuse;
`s4` two drivers T admit → refuse (one message; rustc's E0382 behind it); `s6a` port kind — **vacuous**
with `net` (no ui case); `s8b` no clock T admit → refuse (the header is shared since Part 1); `s9`
undriven net both admit → refuse; `s12` plain `fn` call M admit → refuse **by rustc** (E0308 at
`__instance`; the transpiler's registry message differs — a named rustc-judged row); `s13`
self-instantiation both admit → refuse. **All measured as predicted.** ui/fail: `structural_body_statement`,
`structural_two_drivers`, `structural_undriven_net`, `structural_self_instance`,
`structural_plain_fn_call`, `structural_no_clock`, `structural_retired_wire`; ui/pass `structural_net_ok`
(an `Out` net and a `RegOut` net in one spelling, a reader end to two readers).

**Corpus:** the one structural module, `two_domain_top`, is rewritten to `net` (the spec owes it:
"the one structural module", I.13.7 (2)); its `--hierarchy` SV is **byte-identical** to the baseline's
on the old spelling, `two_domain_hierarchy_cdc` 3/3 (the 2:1 anchor), `two_clock_pins` 4/4, the example
passes. Test sources rewritten: `structural_hierarchy.rs` (its `bad_top` lost an unthreaded `rd_clk` —
the new clock rule fired before the CDC check the test is about) and `interp_design.rs` (text for
`Design`, which already read `net`).

**Found:** I.13.4(b)'s argument list does not name a cloned reader end or parent `In`, which a net with
two readers needs in Rust (an `In` moves); the rule admits `.clone()` of those (P.9).
**Not done:** I.13.7 (3), instantiation refused in behavioural bodies (`b2`) with `lower_hardware_call`
and `spawn_child` deleted — not in step 10's list. A syntactic "port used as a call argument" refusal
would hit **22 corpus modules** (`&clk` passed to `Memory::new`/`from_fn`, and to a helper
`preloaded(&clk)` in `tests/mem_latency_probe.rs`), so it needs a stated exemption first.

### P.5 Part 5 — the II.2.2 directions (P22)

| Predicate | Before step 10 | After | Evidence |
|---|---|---|---|
| R3 `may_exit_without_tick` | over (`livelock_a`, `livelock_b` refused by both) and under (literal-empty `for` counted as a boundary) | **narrowed both ways**: an all-paths walk (`cfg.rs::untick_seq`) — a `loop` is left untick'd only through a `break` reached before any tick, a `for` also through an untick'd fall-through or a literal-empty range. Still **over** by path-insensitivity (complementary `if`s), **under** at a range empty only through a constant (`0..EMPTY`) | ui/pass `livelock_a`, `livelock_b`; ui/fail `literal_empty_for_boundary`; swept fixture `tests/fixtures/all_paths_boundary_dut.rs` (2 modules, differential + reference leg PASS); `static_predicate_directions.rs` (4 tests) |
| D2 `check_definite_assignment` | under: a folded `for` counted as writing every port; a never-written port not flagged | **narrowed**: the combinational CFG expands its loops; the never-written clause (D-14). Still **over** by path-insensitivity | ui/fail `combinational_latch_in_for`, `combinational_never_written`; `static_predicate_directions.rs` (2); cfg tests `comb_output_never_written_is_refused`, `comb_write_under_an_if_in_a_for_is_partial` |
| two-flop (new) | — | over by path-insensitivity (arms joined by union) | `static_predicate_directions.rs` |
| comb-cycle refusal at spawn (new) | — | exact on the declared graph; under for undeclared reads | `fixpoint_oscillating_loop_trips_threshold` |
| R5 (new) | — | exact in the macro (rustc); in the transpiler over where `ConstEnv` is narrower than rustc (as R11) | corpus 24/24 bounds fold |
| `check_admissible` | under by design | unchanged | `admissible_calibration.rs` |
| R11 in the transpiler | over where the fold is narrower | unchanged | `register_init_parity.rs` |
| `infer_registers` | (step 6) over by path-insensitivity, never under | unchanged | `register_reconciliation.rs`, the poison run |

**Predicted:** `livelock_a/b` admitted by both; the literal-empty `for` refused by both; corpus 0
verdict changes. **Measured:** as predicted, with one miss — the baseline *transpiler* already refused
the literal-empty `for`, by the accident "sequential module loop body has no clk.tick().await" (I
predicted it admitted); both now refuse it with R3's message. **Found:** `d2_fold` was a **live parity
break** — the macro admitted the latch the transpiler's `check_no_latches` refused; now both refuse it
with D2's message. **Found while landing the never-written clause:** `check_definite_assignment` chose
"combinational" by the absence of a loop, so the transpiler refused `two_domain_top` (its `Out` is a
child's); it now reads the mode from the attribute when one is present. One cfg unit test
(`comb_output_never_written_not_flagged`) pinned the pre-D-14 position ("a submodule may drive it") and
is flipped — I.13.5 removed that exemption. D2's message gains its clause (3 `.stderr` files). **SV:**
273 modules, 270 unchanged, 0 byte changes, 3 verdicts (`two_domain_top`'s spelling and the two new
fixtures the baseline refused).

### P.6 Part 6 — step 9's two-clock leftovers (SPEC I.12.5, III.2.6)

The spec settles both and nothing was stopped. **A module with two clocks in one body** is refused by
decision (I.12.5; R10, Part 1, ui `two_clocks_sequential`). **A two-clock harness in the sweep:** III.2.6
(D-20) decided that the sweep stays one-clock — a generated case would have to invent the domains'
interleaving — and that the relation's multi-domain coverage is two dedicated pins, both of which exist
(`tests/two_clock_pins.rs`: the never-ticked startup on three legs; the interleavings P = [2;10] and
Q = [1,3,1,3,2,2,3,1,2,2] of `two_domain_top`, sim against the transpiled hierarchy per domain).
`two_domain_top`'s `SKIP` row now says that, and where the design is covered (its three children swept
one by one; `two_domain_hierarchy_cdc.rs` — hand-wired sim = transpiled hierarchy = the independent
reference at 2:1, `copper_ref::Design` = sim at 2:1, 3:1, 1:1; the two pins). The generator's and the
reference leg's second-clock sentences, and the interpreter's `Deferred` ("SPEC I.12 (TBD, Round 3)"),
cite R10 and III.2.6.

### P.7 The bare regression

**Bare `tools/regression.sh` on `dfaef28` (the final code commit; laptop): REGRESSION OK (609 s).**
1413 passed, 0 failed, 16 ignored, 153 test binaries (G-C: 126 test files); examples **32/32**; G-D
**213** differential cases (3 ignored-with-reason, unchanged rows); **G-F 213 modules** (`run=213
clocked=161 memory=18 combinational=52 skipped=0`, design leg ran) — +2, the new fixture; **G-G
differential 1622 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0 DISAGREE** (+14 sequences, the fixture);
**G-G reference 1634 PASS / 0 DISAGREE**; **G-I** 162 clocked related, 0 uncovered; **G-J** 0 DISAGREE;
G-E 5 anchored. Not run remotely (nothing pushed).

### P.8 Stopped — decisions the spec does not settle

1. **R6's macro side (recursion through a helper `fn`).** The macro sees only the module's `ItemFn`
   (I.2.1), never the helper, so it cannot refuse `fn dec(x) { … dec(…) … }` called from a body; the
   transpiler refuses it when it inlines. A terminating recursion simulates correctly, and the
   transpiler never emits a wrong text for it, so the break is a refusal-only asymmetry, not a
   divergence. Options: **(a)** record it as the one named exception to II.1.3's "iff" — a shape the
   kernel lacks that only the inliner can see (cost: none; II.1.3's sentence gains an exception);
   **(b)** an attribute required on every helper a body calls (`#[hardware_fn]`), which refuses direct
   self-recursion syntactically, plus a marker trait the body's calls are checked against — mutual
   recursion still invisible (cost: every helper in the corpus annotated); **(c)** give the macro the
   file (D-21's A4: read the source through the span/manifest — unstable or fragile). Recommendation:
   (a).
2. **The unit domain (I.12.6 (4)).** D-17 A1 retired `()`, believing no corpus port used it; 32
   combinational modules do (P.2). Options: **(A1)** rewrite each with a generic `D: ClockDomain`
   parameter — the module "lies in the domain of its ports" (I.12.4(c)) and stays usable in any domain;
   the domain is phantom, so the emitted SV is predicted byte-identical (cost: 32 signatures and their
   testbenches' `wire::<T, ()>` calls; the sweep's wrapper must name a domain for a generic one);
   **(A2)** rewrite each with a concrete domain (simpler, but a combinational module then lives in one
   domain only); **(B)** amend D-17 A1 to keep `()` on combinational modules as "domain-polymorphic"
   (no rewrite; the type can no longer refuse `In<T>` on a clocked module, which the macro would then
   refuse syntactically). Recommendation: (A1).

Not stopped, not done (outside step 10's list): I.13.7 (3) — instantiation refused in behavioural
bodies, `lower_hardware_call` and `spawn_child` deleted (P.4: a syntactic refusal costs 22 corpus
modules without a stated `&clk` exemption).

### P.9 For the coordinator — SPEC items (SPEC files untouched)

1. **II.1.2's table and II.1.3's open-break list** are stale: R5, R7, R10, R12, R13/R14 hold in both
   front-ends; R16, R17, R18, R19, R20 and D3 are one function each (`copper_analysis::front_end`);
   R15 is closed (`/` has I.14.3(b)'s meaning; both admit); the `cdc_*` rule was a T-admits break the
   list never named, now closed; R6's macro side is P.8 item 1. II.1.4's line numbers predate the move.
2. **II.1.3's audit description** — `front_end_parity.rs` now runs every shared rule, R2/R3 and D2 over
   all 272 fns in every mode, and `copper-codegen/tests/front_end_parity_ui.rs` is the
   transpiler-admits battery (every ui case, messages compared). P10's blind spots are closed.
3. **II.2.2's rows for R3 and D2** — the directions changed (P.5); four new rows (the two-flop rule,
   the combinational-cycle refusal, R5, the structural body rule — exact, syntactic).
4. **I.12.1 / I.12.6 (4):** "no port in `examples/` … uses the unit domain (measured 0)" is wrong — 32
   modules, 100 ports (P.2).
5. **P11** is no longer "false today": the never-ticked pin agrees on three legs (steps 2–3). **P5**'s
   open obligation and **P20**'s are discharged; **P22**'s pins exist (`static_predicate_directions.rs`).
6. **I.13.4(b):** the argument list should name a `.clone()` of a reader end or a parent `In` (a net
   with several readers needs one in Rust); the rule admits it.
7. **I.11.6 / I.13.7 (1), (2), (4):** discharged (this section); `LEVELIZED_SCHEDULING_SCOPE.md`'s
   "detection is not rejection" is reversed in the code (I.13.7 (8), the coordinator's edit).
8. **II.2 D2's implementation note:** `check_definite_assignment` reads the mode from the attribute when
   present; a structural body is not combinational though it has no loop.

## Q. Phase C step 11 — §P.8's two decisions implemented: R6 (a), the unit domain (A1) (2026-11-07)

Session phase-c-domains, branch `claude/phase-c-domains-f9b023` (cut from `paper` at `6c17e6e`; nothing
pushed). The user decided both stopped items of §P.8 as recommended: R6's macro side **(a)**, the unit
domain **(A1)**. Recorded as D-22. Commits: `1ebf010` (the unit domain), `5b22062` (R6), then docs.
Every verdict below was predicted before it was run; the emitted-SV effect was measured against a
transpiler built from `6c17e6e` over every `#[hardware]` module the CLI lists (273), each module alone
and with `--hierarchy`.

### Q.1 The unit domain removed (option A1; SPEC I.12.6 (4), I.12.4(c))

**The type.** `In`, `Out` and `RegOut` bound `D: ClockDomain` on the struct and on every `impl`; `In`
and `Out` lose `D = ()`; `wire` requires the bound (`registered_wire` and `net` already did). A
testbench wire in the unit domain is rustc's E0277, pinned by `copper-core/tests/ui/fail/unit_domain_refused`.

**The refusal on a module** — `copper_analysis::front_end::check_port_domains`, second in
`check_front_end_rules` (after the header), both front-ends, one message (`UNIT_DOMAIN_MESSAGE`): a
`Clock` needs one type argument, an `In`/`Out`/`RegOut`/`Memory` two, the last a path — not `()`, not
missing. Without it the macro's user would see rustc's E0107/E0277 at the port and the transpiler would
admit `In<T>` (it reads text). ui/fail `unit_domain_port` (`In<Logic, ()>`), `missing_domain_port`
(`In<Logic>`); `front_end_parity_ui.rs` requires the transpiler's message equal to the macro's
(measured: it is). The domain readers behind it (`check_cdc`, the one-domain and two-flop rules,
`port_domain`'s `None`) meet named domains only.

**The rewrites — predicted 32 modules / 100 ports (§P.2), measured 32.** A source rewrite gave every
`#[hardware(combinational)]` fn with a `()`-domain or one-argument port a leading `D: ClockDomain`
generic and `D` on each port: 10 in `examples/` (`bsg_adder_one_hot`, `bsg_encode_one_hot`,
`bsg_gray_to_binary`, `bsg_mux_one_hot`, `full_adder`, `mux`, `one_bit_comparator`,
`priority_encode`, `ripple_carry_adder`, `rotate_right`), 16 in `tests/fixtures/` (the six
`cdo_*` of `comb_default_override_dut.rs`, `datapath16`, `const_encoder`, `gray_to_binary`,
`logic_array_pack`, `mod8`, `one_bit_comparator`, `priority_encode`, `ripple_carry_adder`,
`rotate_right`, `wide_alu`), 6 in `tests/*.rs` (`add_one`, `passthrough` in `executor_guardrails.rs`; `add_one`, `invert`, `invert_a`, `buffer_a` in `simulator_corner_cases.rs`). Testbenches name a
concrete `MainClk` (declared in each example's `main`, at file scope in each test); a call with
const generics writes `_` for `D` (`mux::<_, WIDTH, ELS, LG_ELS>`; 12 call sites). Beyond the corpus:
two `ui/pass` cases (`combinational_mux_valid`, `function_typed_valid_tuple_return`), the
`combinational_latch*` trio, `unsupported_idiom_panic` and the four `function_typed_*` fail cases
(their `In<u8>`s given `MainClk` — refused first by the header, so each still tests its own claim;
their `.stderr` changed by the source line only), copper-core's `out_not_clone_single_driver`, and the
transpiler's inline test sources (`, ()>` → `, C>`, the convention their neighbours used; 12 files).

**The sweep.** `build.rs` maps a port whose domain is a type generic of the module to `SweepDomain`
(declared by the wrapper like any undeclared domain) and writes `_` for a type parameter in the
turbofish (`generic_slots`). The `()` filter on declared domains is deleted.

**Found — the structural call site (predicted, then measured).** `chir_lower::lower_structural_inst`'s
CDC check compared a child port's domain *text* with the net's domain, so a domain-generic child wired
in `ClkSlow` was refused as a crossing ("wiring `synced` (domain `ClkSlow`) into `slow_consumer` port
`flag_in` (domain `D`)"). Fixed: a type generic of the callee is bound per instance to its first net's
domain and every other `D` port of the instance is checked against the binding.
`structural_hierarchy.rs::a_domain_generic_child_takes_the_domain_of_its_nets`: the generic child's
`--hierarchy` text equals the concrete child's byte for byte, has no `parameter`; two instances in two
domains admitted; one instance across two domains refused as a crossing.

**SV — predicted byte-identical (the domain is phantom).** Measured: **270 modules transpiled, 270
byte-identical, and their 270 `--hierarchy` outputs byte-identical**; the three modules the baseline
refused (`ram_straddle`, `ram_prewrite`, `b_counted`) refused with the same messages; one new module
(Q.2's witness) refused by R6. No SV parameter for `D` (the texts are the old ones; `copper-transpile
examples/combinational/mux.rs` emits the same `#(parameter int WIDTH_P …)` header). BaseJump: the four
combinational anchors pass as examples and in the sweep (Q.3).

### Q.2 R6 — the one named exception (option (a); SPEC II.1.3, II.1.2 R6)

**Predicted:** the macro admits a combinational module calling a recursive helper; the transpiler
refuses it with "cannot inline recursive function"; the simulator runs a terminating instance as Rust
does; no corpus module reaches a recursive helper. **Measured:** all four. The witness,
`tests/recursive_helper_r6.rs`: `parity8<D>` writes `parity_of(a.read(), 8)`, `parity_of` recursing on
`k`; simulated over all 256 inputs against `u8::count_ones` (PASS); the transpiler's message is now
"cannot inline recursive function `parity_of` into hardware: " + `RECURSIVE_HELPER_MESSAGE` (it names
R6, S3, I.2.5(c) and II.1.3). The ui row: `ui/pass/recursive_helper_r6` (the macro admits), and
`front_end_parity_ui.rs`'s `NAMED_PARITY_EXCEPTION` requires the transpiler to refuse exactly it with
exactly that message. **Cannot widen:** `front_end_parity.rs::the_one_named_parity_exception_is_r6`
builds each file's helper call graph (free `fn`s at any `mod` depth, single-segment calls) and requires
the set of `#[hardware]` fns reaching a helper on a cycle to be `{tests/recursive_helper_r6.rs::parity8}`
(measured: it is); it also checks the scan on the ui row, a mutual recursion (found) and an acyclic
chain (not found). What the scan does not see: a recursion through a method or a trait call, which the
inliner does not inline either (it refuses those as unknown calls, a different message).

### Q.3 The bare regression

**Bare `tools/regression.sh` on `5b22062` (the final code commit; laptop): REGRESSION OK (580 s).**
1417 passed, 0 failed, 16 ignored, 154 test binaries (G-C: 127 test files — +1, `recursive_helper_r6.rs`);
examples **32/32** (the four BaseJump combinational anchors and the five combinational goldens on the
generic spelling); G-D **213** differential cases (3 ignored-with-reason, unchanged); **G-F 213**;
**G-G differential 1622 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0 DISAGREE**; **G-G reference
1634 PASS / 0 DISAGREE**; **G-I** 162 clocked related, 0 uncovered; **G-J** 0 DISAGREE; G-E 5 anchored —
every count equal to §P.7's except the test totals (+4 tests, +1 binary). `sv-baseline diff` against a
snapshot taken on a `6c17e6e` checkout: **1 of 274 modules differ — `NEW tests_recursive_helper_r6.rs__parity8`**
(a `REFUSED` sentinel, Q.2's witness); every other module byte-identical, as the scratch dump of Q.1
found. Not run remotely (nothing pushed).

### Q.4 SPEC items (§P.9) — applied or not

1. II.1.2's table / II.1.3's open-break list — **applied**: the Parity cell of R5, R7, R10, R12, R13,
   R14, R15, R16, R17, R18, R19, R20 annotated (D-22) with its shared function and ui case; II.1.3 gains
   a D-22 note with the rule order, the closed breaks (the `cdc_*` one named) and R6; II.1.4 notes its
   line numbers predate the move. D3 annotated in II.2.1.
2. II.1.3's audit description — **applied** (in the same D-22 note).
3. II.2.2's R3 and D2 rows narrowed, four new rows — **applied** (two-flop, comb-cycle refusal, R5,
   the structural body rule).
4. I.12.1 / I.12.6 (4)'s "measured 0" — **applied** (I.12.1's D-22 note: 32 modules, 100 ports, the
   generic spelling); I.12.6 (4) marked discharged; the map's port types restated; I.12.4(c) gains the
   generic module's per-instance domain.
5. P11, P5, P20, P22 — **applied** (each row's last cell annotated).
6. I.13.4(b) `.clone()` of a reader end / parent `In` — **applied**.
7. I.11.6 / I.13.7 (1), (2), (4) discharged — **applied** as annotations; (3) recorded not done;
   `LEVELIZED_SCHEDULING_SCOPE.md`'s "detection is not rejection" — **not applied**: I.13.7 (8) names it
   the coordinator's edit, and it is not a SPEC file.
8. II.2 D2's implementation note (the mode from the attribute) — **applied**.

**Found stale, not edited** (normative text outside this step's list): II.1.2's R8 and R9 rows still say
"both admit" (R8 is `check_no_plain_out` in both since step 3; R9's flag is refused, ui
`retired_pretick_opt_out`); R21's row says "unmeasured" (ui `preloop_tick` exists); R1–R4's and R11's
`M`/`T` line references predate the move to `front_end.rs`; the header messages of R20 still spell ports
`In<T>`, `Out<T>` (pinned by 13 `.stderr` files; a message change, not a rule change). I.13.7 (3)
(instantiation refused in behavioural bodies) stays open (§P.4).

## R. Phase C step 11 — the conformance ledger, SPEC §III.3 (2026-11-07)

Session phase-c-ledger, branch `claude/phase-c-ledger-441ef3` (cut from `paper` at `924793e`; nothing
pushed). D-20 step (11), in the shape II.3.7's first bullet decided. (`924793e`'s and `5b22062`'s
subjects say "step 11" for what was D-22's work, §Q; that was step 10b — this section is step 11.)

### R.1 Inventory

Four read-only passes over SPEC Parts I–II (one per slice: I.0–I.4, I.5–I.9, I.10–I.12, I.13–II.2),
each mapping every numbered clause to fixtures checked by grep against the tree, cross-read against
§E–§Q; II.3.3's rows and the retired rules mapped from §G.4 and §J. The SPEC requires **168** lines:
**91** numbered clauses of Parts I–II (the Implementation-map and Carried clauses excluded; I.5 has
none), II.1.2's **22** rule rows, II.2.1's **4** derived rules, **11** II.3.3 rows other than (e), the
**29** methods named in row (e)'s shape, and the **9** retired rules 7–15. Added beyond the required
set: the **7** methods row (e) owes (`eq_logic`, `ne_logic`, `as_array_mut`, `from_array`,
`from_slice`, `From`, `all_ones`) and **7** II.3 rows found in Phase C that II.3.3 does not carry —
181 lines.

### R.2 Predictions (written before the bare regression)

~175 lines, ~150 CLOSED, ~25 OPEN. Predicted OPEN: I.10.1, I.10.4, I.10.5, I.10.6 (the memory
obligations of I.10.7 (2)–(7)); I.11.2(c) (the double top-level write); I.12.4(e) (no multi-bit
witness); I.13.4 and I.13.5 (I.13.7 (3), (6)); I.14.3 (`== Logic::X`, signed `saturating_*`); R16 (the
N bound); II.1.3 (the parity breaks outside the battery); I.8.4 (R3 at a constant-empty range); I.2.4
and I.8.5 (the `continue` of a counted `for`); the two `saturating_*` lines; the II.3 found
REFUSED-one rows. Every II.3.3 row else CLOSED; the retired rules CLOSED. Regression numbers
predicted equal to §Q.3's except the test totals (+2 tests, the ledger's two;
+1 binary; G-C 128 files) and the new G-K line.

### R.3 Probes (measured before writing the lines; scratchpad, not in the tree)

- **`continue` of a tick-bearing counted `for`** (`for _i in 0..4 { if a { tick; continue; } …; tick
  }`): the macro **admits** (compiled as a root test target), the transpiler refuses ("unsupported
  construct: a `continue` inside a tick-bearing counted `for`"). A live REFUSED-one break outside
  `front_end_parity_ui.rs`'s battery — II.1.3's "one named exception" is not yet true.
- **A labelled `continue 'w`** of a nested loop: refused by both with R12's message
  (`front_end::check_break_forms`).
- **A combinational port written twice at the top level**: `assign o = a; assign o = b;` — I.11.2(c)
  still open.
- **A `u8` file const in a body expression** (`c.wrapping_add(CI_STEP)`): `localparam int CI_STEP`,
  Verilator 5.044 `-Wall` WIDTHEXPAND + WIDTHTRUNC — §N.5 item 1 still open.

### R.4 The check — G-K

`tests/conformance_ledger.rs` (two tests: the check, and its parsers against a hand-written ledger)
and G-K in `tools/regression.sh`; see SPEC III.3.1. Its first run against the drafted ledger failed
on **5** real faults, each fixed in the ledger or the test, not excused: `Bits` read as a row-(e)
method (the parser now takes lowercase identifiers only); `async fn det_010_awaits (` and `async fn
lfsr (` (a space before the parenthesis) unmatched by the `fn` finder; and `checked_sub`'s line named
no fixture that mentions it — a `checked_sub` witness was added to
`front_end_parity.rs::the_step_8_data_rules_refuse_their_witnesses_and_admit_their_twins`.

### R.5 The bare regression — measured

**Bare `tools/regression.sh` on `dca622d` (the code commit; laptop): REGRESSION OK (485 s).** 1419
passed, 0 failed, 16 ignored, 155 test binaries (G-C: 128 test files — +1, `conformance_ledger.rs`);
examples **32/32**; G-D **213** differential cases (3 ignored-with-reason, unchanged); **G-F 213**
(`run=213 clocked=161 memory=18 combinational=52 skipped=0`, design leg ran); **G-G differential
1622 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0 DISAGREE**; **G-G reference 1634 PASS / 0
DISAGREE**; pinned disagreements 0; **G-I** 162 clocked related, 0 uncovered; **G-J** 0 DISAGREE; G-E 5
anchored; **G-K** `181 lines — 160 CLOSED, 21 OPEN; 167 required by the SPEC; fixtures: 5 coreui, 3
example, 11 gone, 134 sweep, 102 test, 81 ui; 147 run checks`, 144 swept/test fixtures `ok` in the
run, the 3 example fixtures PASS. **Predicted vs measured:** every guard count equal to §Q.3's, as
predicted; tests +2 (1417 → 1419) and binaries +1, as predicted. Not run remotely (nothing pushed).

After the run, docs only: the `II.3 found preload-ok` line (§N.5 item 2, re-measured: the module's
first refusal is "match-as-expression without a wildcard/default arm"), the counts of III.3.6, the
II.3.3 pointer note, this section, CLAUDE.md. `cargo test --test conformance_ledger` re-run on the
final SPEC: PASS, **182 lines — 160 CLOSED, 22 OPEN**.

**The ledger against the prediction.** Predicted ~175 lines / ~150 CLOSED / ~25 OPEN; measured 182 /
160 / 22. Every predicted OPEN line is OPEN except two: **I.10.2** (its one open item, the stale
inference comment, was a comment and was corrected this step) and **II.2.2** (its `infer_registers`
row was stale wording, annotated this step). Not predicted: **I.13.4** (the "every child port
connected" row has no fixture), **II.3 found u8-const** and **comb-double-write** (predicted inside
I.11.2 and §N.5, but each is also its own found line), **II.3 found preload-ok**. By part: Part I 82
lines / 12 OPEN; II.1–II.2 31 / 2; II.3 60 / 8; retired 9 / 0.

### R.6 Stopped — decisions the spec does not settle (each line OPEN with owner "decision")

**(1) I.13.5 — refusing instantiation in a behavioural body (I.13.7 (3)).**
*Question:* by what test does each front-end refuse a child instantiated inside a `sequential`,
`synchronizer` or `combinational` body, given that the macro sees one `ItemFn` and cannot tell a
`#[hardware]` callee from a plain helper `fn` of the same shape?
*Docs say:* I.13.5 (D-18 A2): instantiation and nets are structural-only (S0), `lower_hardware_call`,
the `submodules` fields, `spawn_child` and `spawn_child!` deleted, `module_composition_hybrid.rs`
rewritten, a ui case from probe `b2`. D-18 (iv) measured `b2` (a child taking a parent `Out` in a
`sequential` body) **admitted by the macro, the child dropped and never run, and admitted by the
transpiler**, which instantiates it — a sim ≠ SV shape (corpus 0).
*Code does (924793e):* `spawn_child` (`copper-sim/src/executor.rs:508`), `spawn_child!`
(`copper-sim/src/lib.rs:69-75`), `lower_hardware_call` (`chir_lower.rs:5850`, from `:4189`) stand;
neither front-end refuses; `tests/module_composition_hybrid.rs` spawns children by hand (a testbench
form, not a body). §P.4: a syntactic "port passed as a call argument" refusal would hit **22** corpus
modules (`&clk` to `Memory::new` / `from_fn`, and to `preloaded(&clk)` in `tests/mem_latency_probe.rs`).
*Options:* **(a)** rustc decides, as for a structural body: the macro makes a `#[hardware]` fn's
return type `#[must_use]`-denied outside `copper_sim::__instance` (or returns a type with no use but
`__instance`), so `child(a, b);` in a body is a compile error in the macro; the transpiler refuses
any call it resolves to a `#[hardware]` fn (its registry) with the same message; `lower_hardware_call`
and the `submodules` fields deleted. Cost: one macro change, the hybrid test rewritten; no corpus
module. **(b)** a syntactic rule in both front-ends: a call whose argument is a port parameter
(`In`/`Out`/`RegOut`) — not a `Clock` — is refused; `&clk` to `Memory::new`/helpers stays admitted
(exempted by name: the clock is threaded, not wired). Cost: misses a child taking only a clock and
plain values; over-refuses a helper `fn` taking a port (R19 already refuses an `In` passed on, so
only `Out`/`RegOut` are new). **(c)** record it as a second named parity exception beside R6 (the
transpiler refuses via its registry, the macro cannot see) and delete `lower_hardware_call` so the
transpiler refuses instead of instantiating. Cost: II.1.3 gains a second exception; `b2` stays a
sim-only shape whose child silently never runs.
*Recommendation:* **(a)** — it reuses step 10's `__instance` mechanism, makes the refusal exact (it
is about the callee, not the argument types), and closes the never-running child; (c) leaves a silent
non-execution in the simulator.

**(2) A `continue` of a tick-bearing counted `for` (I.8.5, I.2.4; II.3 found continue-for).**
*Question:* realize it, or refuse it in both front-ends as a dated II.3 limitation?
*Docs say:* I.8.3 / I.8.5 (D-10 A2-i): a `continue` is a zero-time return to the loop's test, which
for a counted `for` runs `c += 1` then the test with the next `c` — IN. II.1.3: any shape one
front-end admits and the other refuses is a break (R6 the only exception).
*Code does:* the macro admits (rustc runs it); the transpiler refuses ("unsupported construct: a
`continue` inside a tick-bearing counted `for`", `control_extract.rs` ~`:1990`); measured by probe
2026-11-07 (§R.3); corpus 0; no ui case, so `front_end_parity_ui.rs` cannot see it.
*Options:* **(a)** realize: lower the `continue` as the desugaring's increment-then-test, the entry
copy that row (a) already uses for a nested `loop` (§J.4); fixture = the probe, swept. Cost: a
control_extract change, predicted no corpus change. **(b)** refuse in both with one message (a
shared `front_end` rule and a ui/fail case), a dated REFUSED-both II.3 row with fix direction (a).
Cost: a meaning D-10 gives is refused until (a) lands. **(c)** leave it (a known break). Cost: II.1.3
false.
*Recommendation:* **(b) now, (a) as its dated fix** — the same order as §J.15's row-2 residue: parity
first (one message, cheap, exact), the lowering when someone needs the shape.

**(3) `v == Logic::X` (I.14.3; §M.9 4(a)).**
*Question:* is comparing a value against the X literal with the derived `==` a value of the kernel, or
the "is this X?" question `is_valid()` asks — OUTSIDE (I.14.3(d))?
*Docs say:* I.14.2: X is a per-bit value; `==` on `Logic` is Rust's derived structural equality (X
equals X), while `eq_logic` is IEEE `==` (X-propagating, §M.1). I.14.3(d): `is_valid()` is OUTSIDE —
a 2-state realization cannot answer it. III.2.2: an X is outside every SV pair.
*Code does:* the macro admits (`#[derive(PartialEq)]`); the transpiler refuses (no lowering of an X
literal operand to `==`); corpus 0.
*Options:* **(a)** OUTSIDE: `check_outside_methods` also refuses `==`/`!=` with an X-literal operand,
one message citing I.14.3(d) — the `is_valid` class. Cost: none in the corpus. **(b)** a value: lower
`v == Logic::X` to constant false in the SV (2-state: no bit is X) — sim ≡ SV only where the
simulator holds no X, i.e. the shape leaves the relation exactly when it means something. **(c)**
leave the break.
*Recommendation:* **(a)** — it is `is_valid()` spelled with an operator, and (b) emits a text that is
right only when the comparison is pointless.

**Not a decision of this step (recorded):** D-21's H2 prototype branch `claude/phase-c-consts-h2`
(tip `5d9153d`) is unmerged; the user decided H1, which is in `paper` (ui `held_preloop_read`, R11's
line CLOSED) — whether to delete the branch is the user's. §N.5's follow-ups are ledger lines: item 1
`II.3 found u8-const`, item 4 `II.3 found const-call` (both OPEN, owner the transpiler, no decision
needed); item 3 (the interpreter's `Self { .. }` gap) is pinned by G-J's `ASYMMETRIC` rows and moves
no line (REF_SKIP is empty); item 2 (`ui/pass/register_init_constants_ok` does not transpile) is
`II.3 found preload-ok`.

### R.7 For the coordinator — SPEC items (normative text read stale, not edited here)

The four inventory passes found stale "today / owed to Phase C" sentences throughout Part I that no
step wrote back (their measurements are in §E–§Q). Edited in this step, as the brief listed: I.10.1(c)
(the write step's order, §O.7 item 1), I.10.7 (a dated note: done / open / moved), I.13.7 (a dated
note), III.2.8 (a dated note: no live instance of any pattern), II.1.2 R8, R9, R21 (Parity cells
annotated) and a note above the table with today's locations of R1–R4 and R11, II.2.2's
`infer_registers` row, R20's three messages (`In<T, D>`, `Out<T, D>`, `RegOut<T, D>`: **2** `.stderr`
files carry them, not 13 as §Q.4 said — the other R20 cases' first lines are other R20 messages), and
`LEVELIZED_SCHEDULING_SCOPE.md`'s reversal note (I.13.7 (8)). **Not edited** (each is a "Today …"
paragraph contradicted by a later step; none changes a meaning, and every one is now a CLOSED ledger
line that names the fixture): I.0.2 (L80, the any-domain first settle), I.0.4 (L100-104), I.0.5(c),
I.0.7 and I.0.8 (the barrier, the injection, the propositions), I.1.4 and I.1.6, I.2.4's table cells
(`/`, statement macros, `break` forms, the nested `continue`, the array write, the `Bits` methods, the
unit domain, the empty `for`), I.2.6, I.2.8, I.3.8, I.4.7, I.4.8, I.6.5, I.6.9 (`trailing_reg_outs`,
`bsg_*` "plain `Out`"), I.7.2(h), I.7.3, I.7.5, I.7.6, I.8.2–I.8.5, I.9.2–I.9.4, I.10.3, I.11.3
("Today both admit it"), I.11.4, I.11.6's map, I.12.2, I.12.3, I.12.5, I.12.6's map (and the per-domain
`TICK_RESOLVING` thread-local, `copper-core/src/types.rs:982-1000`, which I.12.6 (1) says is deleted —
a code item recorded here, not a ledger line: no clause's meaning depends on it), I.13.4(b)–(d)'s table,
I.14.1(b), I.14.2(e)/(h), I.14.3(b)/(d), I.14.5(b)–(d), I.14.6(c), I.14.7's (1), (3)–(10) not marked
done, I.14.8, II.1.1 ("none names its clause"), II.1.5 (R21, the hex parser), III.1's P6 row, and
III.2.1–III.2.7's "Today" sentences (obs_0, X, the truncated verdict, the two pins, the design leg —
all landed, §E, §O, §P.6). Also: the H2 branch is unmerged and decided against (§R.6, last paragraph).

### R.8 Commit subjects that misname the step

`1ebf010`, `5b22062`, `924793e` (D-22's work, §Q) say "Phase C step 11"; that work is step 10b. This
section is step 11.

## S. Phase C step 11b — D-23: the three decisions of §R.6 implemented (2026-11-08)

Session phase-c-r6-decisions, branch `claude/phase-c-r6-decisions-5895af` (cut from `paper` at
`c0f2a3c`; nothing pushed). The user took §R.6's three decisions as recommended — (1) I.13.5 = (a),
(2) continue-for = (b), (3) `== X` = (a) — recorded as D-23. Probes in the session scratchpad (a
crate `p11b` over this tree's `copper-core`/`copper-macros`/`copper-sim`, one bin per shape), not
in the tree.

### S.1 Predictions (written before any probe or code change)

| # | Prediction | Measured |
|---|---|---|
| P1 | `#[deny(unused_must_use)]` on every generated behavioural fn costs 0 compile errors across the workspace (every crate, test, example) | **0 errors, 0 `unused return value` warnings** — as predicted |
| P2 | `b2`'s sequential half (a child called pre-loop) refused by the macro (rustc, the note the message) and the transpiler, one message | as predicted (`ui/fail/behavioural_instantiation`) |
| P3 | `b2`'s combinational half (`one(o)`): the macro E0382 plus the must-use error; the transpiler refuses with I.13.5's message | **not as predicted**: both refuse it first with I.11.3's never-written message (`o` is passed, never written — D-14's rule, step 10); refused by both with one message, so not a break |
| P4 | `let _ = child(..)` / `let m = child(..)` in a sequential body: the macro admits (the child never runs), the transpiler refuses | as predicted — a REFUSED-one shape (§S.4) |
| P5 | Deleting `lower_hardware_call` and the behavioural `submodules`: every corpus module's SV byte-identical | **217 / 217 byte-identical** (`copper-transpile` on every module `--list` reports under `examples/`, `tests/fixtures/`, `src/`, before on `c0f2a3c` and after; every one exit 0) |
| P6 | continue-for: the ui case refused by both with one message; corpus 0; `control_extract`'s arm unreachable, kept as a backstop | as predicted; a `continue` of a loop nested in the `for` is admitted by both (it is row (a)'s) |
| P7 | `== Logic::X` / `Bits::<8>::x() != v` refused by both; corpus 0. A `Logic::X` *pattern*: the macro admits, the transpiler refuses | as predicted — the pattern a REFUSED-one shape (§S.4) |
| P8 | a `wire()` call in a behavioural body (D-18 `b1b`): the macro admits, the transpiler refuses | as predicted ("cannot infer bit width") — REFUSED-one (§S.4) |
| P9 | every guard count of §R.5 unchanged; G-K 182 → ~185 lines, 163 CLOSED, ~22 OPEN | G-K **186 lines — 164 CLOSED, 22 OPEN; 168 required** (one more CLOSED than predicted: the `continue-for` found line became II.3.3 row (h) and closed with it; one more found line than predicted, §S.4's fourth); guard counts §S.6 |

### S.2 (1) I.13.5 — rustc decides

**The mechanism.** Two were allowed: a type with no use but `__instance`, or a denied `#[must_use]`.
A type with no other use makes no statement an error by itself — a value of any type may be dropped —
so both routes end in `unused_must_use`. Put on the generated **fn** (not the `HardwareModule` struct),
rustc names the callee and carries the clause as the note:

```
error: unused return value of `child` that must be used
   = note: a `#[hardware]` module is instantiated only in a `#[hardware(structural)]` body: …
           (SPEC I.13.5, D-18 A2, D-23). Write a structural parent that wires this child …
```

The lint is denied by `#[deny(unused_must_use)]` on each behavioural fn, so it is an error in the body
and a warning in a testbench (where `exec.spawn_wired(child(..), ..)` consumes the value — no
testbench in the tree discards one: P1). The transpiler's refusal, `check_no_behavioural_instance`,
walks the body for a call whose last path segment the registry knows, and runs after the memory
rules (the macro's refusal is rustc's, after every macro rule). `front_end_parity_ui.rs`'s first line
rule cannot apply (rustc's first line is the lint's), so a `LINT_JUDGED` row requires the transpiler's
message to equal the `.stderr`'s `= note:` line — exact, not excused.

**Deleted.** `lower_hardware_call`; `LowerCtx`'s `submodules`, `inst_counters`, `next_inst_name`; the
`submodules` field of the combinational and sequential bodies at CHIR, SHIR and VLIR, with their
lowering, legalization and validation loops; the instance's expression-form fields (`inputs`,
`output_wire`, `output_ty` / `output_width`, `output_port`) and `emit::submodules`' legacy branch
(an instance is always emitted in the connection form now); `PreparedSource::direct_deps` reads a
structural body only; four `chir_lower` unit tests of the deleted form, replaced by one test of the
backstop refusal. In the simulator: `spawn_child`, `spawn_child!`, `ModuleInfo`, `module_info`,
`module_infos`, `ensure_module` and the `modules` map. **`spawn_child` is deleted, not kept:** its one
use was `tests/module_composition_hybrid.rs`, a testbench that recorded a "pipeline" parent which was
never a module; the test (`sequential_modules_compose_in_a_testbench`) wires the same two stages with
`spawn_wired` and passes on the same expected stream `[0, 8, 16, 24]`. Nothing else in the tree named
any of them.

### S.3 (2) and (3)

**continue-for.** `front_end::check_ticking_for_continue`, in `check_front_end_rules` after R12: a
`for` whose body holds a tick (`cfg::stmt_contains_tick`, any depth) and an unlabelled `continue`
that targets it (the walk does not enter nested loops, closures or items). `control_extract`'s arm
uses the shared message; it is unreachable from either front-end. Row (h) of II.3.3, REFUSED-both,
fix direction the lowering (increment, then test; row (a)'s entry copy, §J.4). Pinned by
`ui/fail/ticking_for_continue` and
`unsupported_constructs.rs::a_continue_of_a_tick_bearing_for_is_refused_with_the_shared_message`
(which also requires a nested loop's `continue` inside such a `for` to transpile).

**`== X`.** `check_outside_methods` refuses `==`/`!=` with an operand `Logic::X`, `Bits::x()` or
`Bits::<N>::x()` (through parentheses and `&`), the span on the X operand. `Bits` and `Logic` have no
`PartialOrd`, so no other comparison can take an X literal. `eq_logic(Logic::X)` is untouched (an
IEEE value). The test that pinned the transpiler's own message now pins the shared one.

### S.4 Found — four macro-admits / transpiler-refuses shapes (II.1.3 re-verified)

Re-verifying II.1.3's "R6 the only named exception" by probe: R6 is still the only *named* exception
and no probed shape is admitted by the transpiler and refused by the macro, but four shapes are
admitted by the macro and refused by the transpiler. None emits text. Each is an OPEN line of III.3.4;
none is fixed here.

| Line | Probe | Macro | Transpiler | Owner |
|---|---|---|---|---|
| `II.3 found let-instance` | `let _ = child(clk.clone(), o);` / `let _m = …` pre-loop in a `sequential` body | admits; the child never runs | I.13.5's message | decision (§S.5 (1)) |
| `II.3 found body-wire` | `let (_d, _r) = wire::<Logic, A>(Logic::Zero);` in a `sequential` body | admits | "cannot infer bit width" | the front-ends (I.13.5 already says a net is outside a behavioural body) |
| `II.3 found x-pattern` | `match a.read() { Logic::X => …, _ => … }` | admits | "unsupported pattern: Logic :: X" | decision (§S.5 (2)) |
| `II.3 found tickfree-continue` | `for i in 0..4u8 { if i == 2 { continue; } s = s.wrapping_add(i); }` in a ticking body | admits | "`continue` is not supported in hardware; the module's top-level `loop` runs forever" — a message about the wrong loop | copper-codegen (the unroller) |

**Surprises.** (i) rustc's own `help:` line under the I.13.5 refusal reads "use `let _ = ...` to
ignore the resulting value" — it steers the author to the one spelling the macro admits. (ii) The
tick-free `continue` was not predicted: I.2.4 makes a tick-free constant `for` an unrolling, and Rust's
`continue` in it is IN, but the transpiler's unroller refuses it and blames the module loop. (iii)
`b2`'s combinational half needs no I.13.5 rule at all: since step 10 the never-written rule of I.11.3
sees the port passed and not written (P3).

### S.5 Decision packets (not implemented; each line OPEN with owner "decision")

**(1) A bound instance in a behavioural body (`II.3 found let-instance`).**
*Question:* should `let _ = child(..)` / `let m = child(..)` in a `sequential`, `synchronizer` or
`combinational` body be refused by the macro, and how?
*Docs say:* I.13.5 (D-18 A2): a behavioural body contains no instantiation; D-23 (1) chose "rustc
decides"; II.1.3: a shape one front-end admits and the other refuses is a break.
*Code does (272159e):* the denied `unused_must_use` fires on an unused *statement* only; a `let`
binding is a use. The macro admits both spellings and the child's coroutine is dropped unstarted; the
transpiler refuses with I.13.5's message. rustc's help for the statement form suggests `let _ =`.
Corpus 0 (the deny costs nothing: §S.1 P1).
*Options:* **(a)** a run-time refusal in the simulator: `HardwareModule` gains a drop guard that panics
("a `#[hardware]` module was made and never started — SPEC I.13.5") when it is dropped unconsumed by a
spawn or `__instance`. Changes: copper-sim; every swept module still runs (a spawned module is consumed);
the shape becomes a loud simulator failure at the first resumption, not a compile error — still a
REFUSED-one break by II.1.3's letter (compile vs transpile), but no longer a silent non-execution.
**(b)** the macro refuses, syntactically, a `let` whose initializer is a bare call of a path that is
not a known non-module (`Bits::…`, `Memory::…`, a const fn): a `let` bound to a helper `fn`'s call is
an ordinary body form (I.2.4 inlines helpers), which the macro cannot tell from a module's, so (b)
over-refuses or needs a list of exempt callees — §R.6 (1) (b)'s exemption-by-name problem. How many
corpus bodies it would refuse is not measured here (a grep of `let x = f(..)` lines cannot separate
bodies from testbenches).
**(c)** record it as a second named parity exception beside R6 (the macro cannot see that a `let`'s
callee is a module; rustc can only see unused values). Changes: II.1.3 gains an exception; pinned by a
ui/pass row as R6 is.
*Recommendation:* **(c) with (a)** — (a) removes the silent non-execution, which is the harm §R.6 (1)
named; (c) states honestly that a compile-time refusal of the bound form is not available to a
`syn`-level macro without a whole-file view (D-21's A4), as R6 already does.

**(2) A `Logic::X` pattern (`II.3 found x-pattern`).**
*Question:* is `match v { Logic::X => … }` the OUTSIDE question D-23 (3) refused for `==`, or a value?
*Docs say:* I.14.3(e): a `Logic` scrutinee holding X matches `Logic::X` or `_` only — structurally, as
the derived `==`; I.14.3(d) and D-23 (3): `==`/`!=` with an X literal is OUTSIDE.
*Code does:* the macro admits (a structural match); the transpiler refuses ("unsupported pattern").
Corpus 0.
*Options:* **(a)** OUTSIDE: `check_outside_methods` also refuses a `Logic::X` (or `Bits::x()`-valued
const) pattern, one message citing I.14.3(d); I.14.3(e)'s sentence becomes "matches `_` only". **(b)** a
value: lower the arm as never taken in the 2-state SV — right only where the simulator holds no X.
**(c)** leave the break.
*Recommendation:* **(a)** — it is the same question as `== Logic::X` in a pattern's spelling, and D-23
(3)'s reason applies unchanged.

### S.6 The bare regression — measured

**Bare `tools/regression.sh` on `272159e` (the code commit; laptop): REGRESSION OK (544 s).** 1417
passed, 0 failed, 16 ignored, 155 test binaries (G-C: 128 test files); examples **32/32**; G-D **213**
differential cases (3 ignored-with-reason); **G-F 213**; **G-G differential 1622 PASS / 12 OUTSIDE / 0
TRUNCATED / 0 EXCUSED / 0 DISAGREE**; **G-G reference 1634 PASS / 0 DISAGREE**; **G-I** 162 clocked
related, 0 uncovered; **G-J** 0 DISAGREE; G-E 5 anchored; **G-K** `186 lines — 164 CLOSED, 22 OPEN; 168
required by the SPEC; fixtures: 5 coreui, 3 example, 11 gone, 137 sweep, 108 test, 86 ui; 151 run
checks`, the 3 example fixtures PASS. **Predicted vs measured:** every guard count equal to §R.5's, as
predicted; tests 1419 → 1417 (−4 `chir_lower` unit tests of the deleted expression form, +1 test of
its refusal, +1 `unsupported_constructs` test of row (h)); the three new ui/fail cases run inside the
one trybuild test. Not run remotely (nothing pushed).

After the run, docs only: D-23 (`SPEC_DECISIONS.md`), III.3.6's counts, this section, CLAUDE.md.

## T. Phase C step 11c — D-24: the two decisions of §S.5 implemented (2026-11-08)

Session phase-c-s5-decisions, branch `claude/phase-c-s5-decisions-c662aa` (cut from `paper` at
`7c9483f`; nothing pushed). The user took §S.5's two decisions as recommended — (1) the let-bound
instance = (c) + (a), (2) the `Logic::X` pattern = (a), OUTSIDE — recorded as D-24. Probes in the
session scratchpad (a crate `p11c` over this tree's `copper-core`/`copper-macros`/`copper-sim`, one bin
per shape, the transpiler leg by `copper-transpile`), not in the tree. Code commits `dbba229` and
`7589204`; docs after.

### T.1 Predictions (written before any code change or probe)

| # | Prediction | Measured |
|---|---|---|
| P1 | the drop guard trips on 0 legitimate paths (every swept case, test, example, `copper-ref`'s design leg) | **not as predicted — one**: `reference_leg_measurements.rs::future_size_bounds_the_register_storage` built seven module values to measure `size_of_val` and dropped them (the first bare regression, on `dbba229`, failed there: 601 passed, 1 failed). Fixed by consuming each with a spawn into a never-ticked executor (`7589204`); then `cargo test --workspace --no-fail-fast` 1424 passed / 0 failed and the bare regression REGRESSION OK. No swept case, example, sweep harness path or `copper-ref` leg tripped it |
| P2 | `let _ = child(..)` before the loop panics at the parent's first resumption, with the message | as predicted (`let_wildcard_instance_panics_at_the_first_resumption`: the first `tick_clock`; the payload ends "(module `child`)") |
| P3 | `let _m = child(..)` before the loop panics only when the parent's coroutine is dropped (the binding is live across the module loop) | as predicted: four ticks silent, then `drop(exec)` panics (`let_named_instance_panics_when_the_parent_is_dropped`) |
| P4 | the tree holds no behavioural body binding a `#[hardware]` call besides the witnesses | as predicted: the scan finds exactly `tests/let_instance_d24.rs::{parent_named, parent_wildcard}` |
| P5 | a `Logic::X` arm, or-pattern, `if let`, tuple sub-pattern: refused by both, one message; corpus 0 | as predicted, plus `matches!(v, Logic::X)` and a body `const U: Logic = Logic::X` pattern (probes `x1`–`x6`: macro and transpiler both print `X_PATTERN_MESSAGE`); the twin (`Logic::One =>`) admitted by both; corpus 0 (`every_corpus_module_passes_every_shared_rule_in_its_mode` and the sweep pass) |
| P6 | a pattern naming a file-level `const U: Logic = Logic::X`: the macro admits; the transpiler refuses with "unsupported pattern" | **half**: the macro admits; the transpiler refuses with a *different* message, "tuple-pattern match lowering is not yet implemented (M2)", and not because of X — see T.3 |
| P7 | the guard counts of §S.6 unchanged | as predicted: G-D 213 (3 ignored), G-F 213, G-G differential 1622 PASS / 12 OUTSIDE / 0 / 0 / 0, reference 1634 PASS / 0 DISAGREE, G-I 162 related / 0 uncovered, G-J 0 DISAGREE, G-E 5 |
| P8 | G-K 186 → 186 lines, 164 → 166 CLOSED, 22 → 20 OPEN | **187 lines — 166 CLOSED, 21 OPEN** (one more line than predicted: T.3's found shape); 168 required |
| P9 | tests 1417 → ~1421 | **1424** (+6 `let_instance_d24.rs`, +1 `unsupported_constructs.rs`; the extended parity tests are the same tests); 156 test binaries, G-C 129 test files |

### T.2 What changed

**(1) The drop guard.** `HardwareModule::future` is an `Option<F>`; `into_future` (the spawns) and
`__instance` take it; `impl Drop` panics with `UNSTARTED_MODULE_MESSAGE` plus "(module `<name>`)" when it
is still `Some`, unless `std::thread::panicking()` (pinned by
`the_guard_is_silent_while_another_panic_unwinds`, a `should_panic` test that drops an unconsumed module
during unwinding and does not abort). A hand-built module is guarded too. The guard fires when the
value is dropped, which is the parent's first resumption for `let _ =` and for a binding scoped inside
the loop body (at the end of that iteration), but for a `let m =` before the loop only when the parent's
coroutine is dropped — at the executor's drop, or never if a testbench leaks the executor or exits the
process first. The guard turns the silent non-execution into a loud failure *when the value dies*, not
at a fixed instant. **The named exception.** `front_end_parity_ui.rs`'s `NAMED_PARITY_EXCEPTIONS` (two
rows, each required to exist in ui/pass and to fail with its message); the R6 audit renamed
`the_named_parity_exceptions_are_r6_and_the_let_instance`, with a second scan (behavioural fns, any
depth including test fn bodies, binding in a `let` a call of a same-file `#[hardware]` name) whose set
must equal the two witnesses; ui/pass `let_instance_d24`. Side effect: `ui/fail/structural_plain_fn_call.stderr`
re-blessed — rustc's note quotes `__instance`'s signature line, now `pub fn __instance<F>(mut module:
HardwareModule<F>) {`.

**(2) The X pattern.** `check_outside_methods` gains `visit_pat` (a `Logic::X` path pattern anywhere;
an identifier pattern naming a body `const` whose initializer is an X literal) and parses `matches!`'s
pattern. `ConstEnv` is integer-valued and cannot hold X, so the "const whose value is X" clause is
syntactic over the body's own `const` items. ui/fail `outside_x_pattern`; witnesses and twins added to
`the_step_8_data_rules_refuse_their_witnesses_and_admit_their_twins`; the transpiler's
`unsupported pattern` arm is now a backstop.

### T.3 Found — one macro-admits / transpiler-refuses shape (II.1.3 re-verified)

| Line | Probe | Macro | Transpiler | Owner |
|---|---|---|---|---|
| `II.3 found const-pattern` | `x7` `const U: Logic = Logic::X;` (file); `x8` `const ONE: Logic = Logic::One;` (file); `x9` `const UB: Bits<2> = Bits::x();` (file); `x11` `const ONE: Logic = Logic::One;` (in the body) — each `match a.read() { K => …, _ => … }` | admits | "tuple-pattern match lowering is not yet implemented (M2)" — `parse_pattern` reads an uppercase identifier as an enum variant | copper-codegen |

An integer const pattern (`x12`, `const K: u8 = 3; … K =>`) transpiles. No text is emitted for any of
the four. The shape is not X-specific (`x8`, `x11`), so it is not D-24's: an OPEN line. A file-level
X-valued const (`x7`, `x9`) is outside the `ItemFn` both front-ends' rule holds; once the lowering
reads a const pattern's value, how that case is refused is a decision for that change.

**Surprises.** (i) P1: a test that only *measures* module values was a legitimate path the guard
refused — "every legitimate path consumes the value" was false of the tree by one test. (ii) The guard
grows every module value by 8 bytes (`Option`'s discriminant; the futures have no niche rustc uses):
measured on `future_size_bounds_the_register_storage` at `7c9483f` (in the r6-decisions worktree,
same commit) vs `7589204` — `counted_repetition` 112 → 120, `capture_after_wait` 104 → 112, `mac_fsm`
128 → 136, `uart_rx_dut` 128 → 136, `regout_multiphase` 104 → 112, `handshake` 104 → 112,
`regout_trailing_single_tick` 96 → 104; the test's bound (≥ 16) holds. The sizes are of the
`HardwareModule`, not the bare future, as before. (iii) P6: the file-level X const was refused for the
wrong reason, and the same refusal hits a `Logic::One` const.

### T.4 The bare regression — measured

**Bare `tools/regression.sh` on `dbba229`: REGRESSION FAIL** at `cargo test --workspace` (601 passed,
1 failed: `future_size_bounds_the_register_storage`, the drop guard, T.1 P1). **Bare
`tools/regression.sh` on `7589204` (the final code commit; laptop): REGRESSION OK (13259 s wall — the
laptop slept; 7 min user CPU, not a timing number).** 1424 passed, 0 failed, 16 ignored, 156 test
binaries (G-C: 129 test files); examples **32/32**; G-D **213** differential cases (3
ignored-with-reason); **G-F 213**; **G-G differential 1622 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED /
0 DISAGREE**; **G-G reference 1634 PASS / 0 DISAGREE**; **G-I** 162 clocked related, 0 uncovered;
**G-J** 0 DISAGREE; G-E 5 anchored; **G-K** `187 lines — 166 CLOSED, 21 OPEN; 168 required by the SPEC;
fixtures: 5 coreui, 3 example, 11 gone, 137 sweep, 116 test, 89 ui; 156 run checks`, the 3 example
fixtures PASS. Not run remotely (nothing pushed).

After the run, docs only: D-24 (`SPEC_DECISIONS.md`), III.3.6's counts, this section, CLAUDE.md.
Disk: the run needed the cargo `target/` dirs of six merged, idle worktrees deleted first (the user's
approval, ~51 GiB; 4.2 GiB was free).

## U. The copper-codegen-owned OPEN lines of SPEC §III.3 (2026-11-08)

Session open-lines-codegen, branch `claude/open-lines-codegen-2c913f` (cut from `paper` at `e4a9aae`;
nothing pushed). One of three parallel batches over III.3's 21 OPEN lines (§V is another session's).
Instrument for every prediction: `sv-baseline` (`copper-codegen/src/bin/sv-baseline.rs`), snapshot of
the untouched tree at `e4a9aae`, **277 modules**. Probes in the session scratchpad, not in the tree.
Commits (after the rebase of U.10): code `41931a5`, `f913713`, `244d49e`, `357c6f8`, `ecc26cf`, `4d69982`; ledger `83848cd`; docs after.

**Every change: predicted no corpus module's SV changes; measured — none did.** The final
`sv-baseline diff`: 13 of 290 modules differ, all 13 NEW (the fixture modules below and the
`tests/ref_probes` module); no corpus module changed, no REFUSED sentinel appeared or disappeared.

### U.1 `II.3 found tickfree-continue`

**Change.** `copper-codegen/src/for_continue.rs`, a FIR→FIR pass run first in `lower_fir_to_chir`: in a
tick-free `for` body, the first statement holding this loop's `continue` (not a nested loop's) is
rewritten — `continue;` drops the rest; `if c {T} else {E}` becomes `if c {elim(T ++ rest)} else
{elim(E ++ rest)}`; each `match` arm and a block likewise. It declines (leaves the `continue`, and
`chir_lower` refuses with a message about the `for`, not the module loop) a labelled `continue`, one
inside a `let` or an expression, and a rest that names a binding made in the branch (the copied rest
would capture it). **Found and fixed on the way:** copper-ref's `CombFragment` refused any `continue`
in a combinational body ("I.2.7: the tick-free fragment has none"); it now admits an unlabelled one
inside a `for` (the unrolled `for`'s, I.2.4); the sequential case already ran. Fixture
`tickfree_continue_dut.rs` (`tfc_sum_set_bits` sequential, `tfc_branches` combinational: `continue`
after work in an `if`, in an `else`-paired `if`, in a `match` arm). **Found, not fixed:**
`Bits::from_usize(i)` of a `for` variable emits as the 32-bit `i` (`s = (s + i)`, Verilator
WIDTHEXPAND/WIDTHTRUNC); the fixture uses `from_lit` instead.

### U.2 `II.3 found const-pattern`

**Change.** A pattern identifier that is neither a `localparam` (`params`) nor an enum variant and
names a body `const` (any depth; it shadows) or a file `const` is lowered by its value:
`pattern_const` parses the initializer and folds it with `fold_const_init` (the register-initializer
fold) at the declared type's width — a `CHIRPattern::Lit` (statement `match`, `size_patterns_to_
scrutinee`) or a `PatElem::Lit` (expression / tuple `match`). An integer file const stays the
`localparam` case label (`x12`, byte-identical). Probes `x7`–`x12` of §T re-run: `x8`, `x10`, `x11`
lower (`1'b1:`, `2'd2:`); `x7`, `x9` (file consts `Logic::X`, `Bits::x()`) are refused with D-24's
`X_PATTERN_MESSAGE`. Fixture `const_pattern_dut.rs` (`cp_statement`: `Logic`, `Bits<2>` and `u8` file
consts; `cp_tuple_expr`: a tuple expression `match` with a body const). Its `u8` arm lint-failed until
U.6 (the `case (n)` label `CP_K` was a 32-bit `localparam int`) — the two lines share a fixture.

**Decision packet — the file-level X const (line `II.3 found file-x-const-pattern`, OPEN).**
*Question:* the macro admits `const UX: Logic = Logic::X; … match v { UX => … }`; the transpiler now
refuses it as OUTSIDE (D-24 (2)). Is that the final disposition, and how does the macro refuse it?
*Docs:* I.14.3(d)–(e) and D-24 (2) make an X pattern OUTSIDE; the X-pattern row of I.14 says a file
const is outside the `ItemFn` the shared rule holds. *Code:* `check_outside_methods` sees body consts
only; the macro expands from the `ItemFn`; rustc admits the pattern (`Logic` is a structural-match
type). *Options:* (a) keep it a REFUSED-one break, a third named exception of II.1.3 (the transpiler
refuses, the simulator would match X structurally); (b) the macro emits, per single-identifier path
pattern, a `const _: () = assert!(…)` that X is not the value — needs a const-evaluable `is_x` on
`Logic`/`Bits` and must not fire on enum variants, which it cannot tell apart syntactically (it would
need the type, i.e. a trait-based check such as `<T as XProbe>::IS_X`); (c) refuse every pattern naming
a file const of type `Logic`/`Bits` in both front-ends (over-refuses `x8`/`x10`). *Recommendation:*
(a) now — corpus 0, no text emitted, the message is D-24's — and (b) if parity must be exact.

### U.3 `II.3 found preload-ok`

**Changes.** (1) An expression `match` that reaches the `case` form with no wildcard arm: rustc proved
it exhaustive and a case-compatible `match` has no guards, so the last arm's value is the `default`
and its labels are dropped (`shir_lower` refused a `Case` with no default). (2) A bit write `v[i] = e`
on a pre-loop register (`ctx.scalar_regs`, the `CHIRRegDecl`s) whose width is concrete becomes the
assignment `v = (v & ~(W'1 << i)) | (W'(e) << i)`, which the register path forwards like any other;
a wire keeps its `IndexAssign`. Measured before: `lv1` (`[Logic; 4]`), `lv3`, `lv4` (`Bits<4>`, the same
class — not only one-bit-element arrays), `lv5` (head writes) all refused as latches; after, each
lowers. `ui/pass/register_init_constants_ok.rs` transpiles and lints clean (`-Wall`, only
DECLFILENAME on the scratch file name); `front_end_parity_ui.rs`'s `PASS_NOT_TRANSPILED` is empty.
Fixture `packed_bit_write_dut.rs` (`pb_shift_and_phase`: a 3-variant enum `match` expression, a
`[Logic; 4]` shift by element writes; `pb_dynamic_bit`: a `Bits<4>` bit at a run-time index, read back
in the same resumption). The emitted expressions grow with each forwarded write (four writes in
`pb_shift_and_phase` nest four deep) — correct, not compact.

### U.4 `II.3 found array-for`

**Change.** In `chir_lower`'s `for` arm, a body that writes an element of an array register
(`stmts_write_array_reg`) and whose bounds fold (`eval_const_usize`) is unrolled: the body lowered
once per iteration with the loop variable bound (`ctx.bindings`) to a 32-bit literal, at most
`MAX_UNROLL` = 1024 iterations. Each iteration's write is its own statement and so its own write port
— row (c)'s form; a later iteration's read sees an earlier one's word through the staging-net mux.
Fixture `array_for_write_dut.rs` (`afw_accumulate`: the §J.6 witness; `afw_shift_cond`: a shift
through the array, a conditional write in a second `for`, reads after both — six write ports). The pin
`an_array_register_element_write_inside_a_combinational_for_is_unrolled_unless_the_bound_is_generic`
replaces the refusal pin.

**Decision packet — a const-generic bound (line `II.3 found array-for-generic`, OPEN).** *Question:*
`for j in 0..N { arr[j] = … }` with `N` a const generic: the transpiler still refuses (the old
message); the macro admits. *Docs:* I.2.4 — a tick-free constant-bound `for` is unrolled; `N` is a
constant of the instance, not of the definition the transpiler emits. *Code:* the SV is one
parametric module; the write-port form needs a port per iteration, i.e. the trip count. *Options:*
(a) refuse in both front-ends with one message (a shared rule: an array-register element write inside
a `for` whose bound is not a file constant or literal); (b) a new emitted form for this case — the
cycle's words computed into an array copy in `always_comb` (`arr_n = arr; for (…) arr_n[j] = …;`) and
committed whole in `always_ff`, which is a second array-register form beside row (c)'s; (c) emit per
instance (the transpiler takes the concrete parameters). Corpus 0. *Recommendation:* (a) — corpus 0,
and (b) adds a form for no live module.

### U.5 `II.3 found comb-double-write` and I.11.2

**Change.** `vlir_lower::lower_comb` routes into `always_comb` every port the top level drives two or
more times (`top_level_multiply_driven_ports`), joining §10.12's nested-drive set. Probes `cdw1`
(`assign o = a; assign o = b;` before) and `cdw2` (another port between) now emit one `always_comb`
with both writes in program order; a once-written port keeps its `assign`. Fixture modules
`comb_default_override_dut.rs::{cdo_double_write, cdo_double_write_mixed}`.

### U.6 `II.3 found u8-const`

**Prediction (written before the change):** the corpus's emitted `localparam`s are all `u32`/`usize`
(27 consts in 13 files), so no corpus module changes. **Change:** `ModuleLocalParam`/`VLIRLocalParam`
gain `sv_type`; a `u8`/`u16`/`i8`/`i16` file const emits `localparam logic [W-1:0] K = W'(e)`
(`signed` for `i8`/`i16`, the size cast so `CI_START / 30` does not trip WIDTHTRUNC), and the width
inference reads its width instead of 32 (`ctx.narrow_consts`). **Measured:** as predicted. Fixture
`const_init_dut.rs::init_file_const_in_body` (`c.wrapping_add(CI_STEP)`, the spelling §N.5 item 1
measured lint-failing, and `a ^ CI_START`) and `const_pattern_dut.rs::cp_statement`'s `CP_K` label.

### U.7 II.3.3 (e) `saturating_add`, `saturating_sub`

**Change.** A signed operand (a `SignCast`): on the raw bits, `s = a ± b` wrapping; overflow iff the
operands' sign bits are equal (`+`) / differ (`-`) and `s`'s sign differs from `a`'s; then MIN when
`a` is negative, MAX otherwise; the result re-wrapped signed. `saturating_mul` signed stays refused.
The unsigned clamp is lowered up to 128 bits (`saturating_mul` still ≤ 64). Fixture
`saturating_signed_wide_dut.rs::sat_signed` (`i8`, `i16`), swept. **Found — the `u128` case cannot be
swept:** (i) copper-ref's `IntTy::U128` range is `(0, i128::MAX)` and its `saturating_add` computes
`v + o` in `i128`, which panics (overflow) on the first operand ≥ 2^127 — both sweep cases of a `u128`
module died there (the differential case runs the interpreter too); (ii) the Verilator testbench
compares a port through a 64-bit integer (`verification.rs` `logic_vec_to_int`, a shift overflow on a
128-bit port). So the `u128` module is `tests/ref_probes/saturating_u128_probe.rs` (64-bit port halves,
`u128` arithmetic) and `tests/saturating_u128_equivalence.rs` checks sim vs SV on twelve operand pairs
that reach both clamps (and asserts the simulator against Rust's own `saturating_*`). Mutation check:
`max = u128::MAX - 1` makes the test fail. Interpreter `u128`/`i128` and the testbench's 64-bit port
limit are follow-ups (copper-ref, copper-sim), not fixed here.

### U.8 Ledger counts

Predicted 187 → 189 lines (two residue lines opened), 166 → 175 CLOSED (tickfree-continue,
const-pattern, preload-ok, array-for, comb-double-write, I.11.2, u8-const, `saturating_add`,
`saturating_sub`), 21 → 14 OPEN. Measured by `conformance_ledger.rs` on `7dc5c89` (before the rebase): **189 lines — 175
CLOSED, 14 OPEN; fixtures 5 coreui, 3 example, 11 gone, 156 sweep, 123 test, 89 ui; 174 run checks.**
I.2.4 stays OPEN, narrowed to array-for-generic; II.1.3 stays OPEN with four residues (const-call,
body-wire, array-for-generic, file-x-const-pattern).

### U.9 What the first bare regression found — G-J

**Bare `tools/regression.sh` on `7dc5c89` (before the rebase): REGRESSION FAIL** at `cargo test --workspace`
(`const_evaluators_agree.rs`, 1190 s wall). Not predicted. Two items, both in the test, neither a
semantic disagreement: (i) **1 DISAGREE** — `packed_bit_write_dut.rs::pb_shift_and_phase`'s
`let bits = [Logic::Zero; 4]`: the transpiler states 0 (the packed vector), the interpreter has an
array of four `1'0`, and the test's `ref_int` read no array. No transpiled corpus module had a
`[Logic; N]` pre-loop register before U.3. `ref_int` now packs an array of `Bits` with element 0 in
the low bits, the layout `fold_const_init`'s `[elem; n]` states → 0 DISAGREE (273 initializers vs
the transpiler, one more). (ii) **5 unpinned asymmetries**, the const-pattern fixture's `Logic` /
`Bits` consts (`CP_ONE`, `CP_ZERO`, `CP_TWO`, `CP_THREE`, `BODY_TWO`): `ConstEnv` is integer-valued, so
only the interpreter evaluates them. They are the corpus's first non-integer consts, and they are
pinned as `ASYMMETRIC` with that reason (the transpiler folds a pattern const with `fold_const_init`,
and the sweep checks that against rustc). Commit `Open lines (codegen): G-J reads a packed-vector …`.

### U.10 The rebase and the bare regression

**Rebase.** `paper` moved to `d022fbe` (the front-end batch, §V) during this work; the branch was
rebased onto it. The code commits applied without conflict (B's only `chir_lower.rs` hunk, the
`fold_const_init` arm, is apart from this batch's). Conflicts were in SPEC §III.3's found rows (both
batches' rows kept: B's `const-call` CLOSED and `const-call-body`, `runtime-empty-boundary` OPEN beside
this batch's), II.1.3's list (rewritten to the four residues after both batches), this file's tail
(§U placed before §V) and CLAUDE.md's parity paragraph. **Re-measured on the rebased tree:**
sv-baseline against a fresh snapshot of `d022fbe` (280 modules): 13 of 293 differ, all NEW (this
batch's fixture modules and the `tests/ref_probes` module) — no corpus module changed, B's
`const_call_dut` modules included. G-J's five `ASYMMETRIC` rows are still required after B widened
both evaluators (the test fails on a stale row): `ConstEnv` is still integer-valued for `Logic`/`Bits`.
Ledger predicted from `d022fbe`'s 189 / 171 / 18 as 191 / 180 / 11; measured as predicted.

**Bare `tools/regression.sh` on `f29fe46` (the rebased tip; the last code commit is `4d69982`, the one
after it docs; laptop): REGRESSION OK (427 s wall).** 1463 passed, 0 failed, 16 ignored, 157 test
binaries (G-C: 130 test files); examples **32/32**; G-D **228** differential cases (3
ignored-with-reason); **G-F 228** (3 skipped with a reason); **G-G differential 1732 PASS / 12 OUTSIDE /
0 TRUNCATED / 0 EXCUSED / 0 DISAGREE**; **G-G reference 1744 PASS / 0 DISAGREE**; **G-I** 172 clocked
related (43 extracted, 18 linear, 111 single-tick), 0 uncovered; **G-J** 73 const items compared / 22
outside both, 278 + 47 initializers, 5 asymmetric (pinned), **0 DISAGREE**; G-E 5 anchored; **G-K**
`191 lines — 180 CLOSED, 11 OPEN; 168 required by the SPEC; fixtures: 6 coreui, 3 example, 11 gone, 160
sweep, 130 test, 100 ui; 180 run checks`, the 3 example fixtures PASS. Against `c518b77`'s run (§V.8):
+12 swept modules (G-D 216 → 228: `tfc` ×2, `cp` ×2, `pb` ×2, `afw` ×2, `cdo_double_write*` ×2,
`init_file_const_in_body`, `sat_signed`), G-G +78 PASS on each comparison, +1 test file
(`saturating_u128_equivalence.rs`); the 12 OUTSIDE unchanged. New `SKIP` / `REF_SKIP` / `PARAMS` /
`RESET` rows: none. Not run remotely (nothing pushed). After this run, docs only: III.3.6's counts and
this subsection.

### U.11 D-26 — the two packets decided (2026-11-08)

Branch `claude/open-lines-codegen-d26`, cut from `paper` at `9c0272a`. The user took both packets as
recommended: U.4 = (a), U.2 = (a), recorded as D-26. **Predictions (written before the code):** corpus 0
for both shapes, so sv-baseline changes no corpus module; the rule trips no swept case, test or
example; ledger 191 → 191 lines, 180 → 183 CLOSED (I.2.4 and the two found lines), 11 → 8 OPEN.

**(1) `II.3 found array-for-generic` — REFUSED-both.** `front_end::check_array_write_in_generic_for`
(in `check_front_end_rules`, after R5), `ARRAY_WRITE_GENERIC_FOR_MESSAGE`. An array register is the
transpiler's classification read off the source: a pre-loop `let mut x = [e; n]` (or an array
literal) whose element is not one bit (`Logic::…`, `bool`, `Bits::<1>` — a packed vector, whose bit
writes §U.3 lowers) and that `infer_registers` calls a register; the rule fires on an element write
(`=` or a compound assignment) to one inside a tick-free `for` whose range names a const generic.
Probes: `afg` (`0..N`, `[Bits::<8>::zero(); 4]`) refused by both with the message; `afg_ok` (`0..4`)
and `afg_packed` (`0..N`, `[Logic::Zero; 4]`) transpile. ui/fail `array_write_generic_for` (blessed;
the overwrite touched no other `.stderr`); the twins (literal bound, packed vector, ticking `for`) in
the audit; the transpiler's pin now requires the shared message (its old lowering message is a
backstop).

**(2) `II.3 found file-x-const-pattern` — the third named exception.** ui/pass
`file_x_const_pattern_d26` with a `NAMED_PARITY_EXCEPTIONS` row (`X_PATTERN_MESSAGE`);
`tests/file_x_const_pattern_d26.rs` (`fx_logic`, `fx_bits`): the transpiler refuses both with the
message, and the simulator matches X structurally (an X input takes the `UX` arm, `0`/`1` take `_`).
The audit is renamed `the_three_named_parity_exceptions_have_exactly_their_witnesses` (its old name
said two) and gains a third scan — `#[hardware]` bodies whose identifier pattern names a file const
initialized `Logic::X` / `Bits::x()` / `Bits::<N>::x()` — required to find exactly the two witnesses;
probes in the test: an X `Logic` and an X `Bits<2>` const found, a `Logic::One` const not. The tree
scan found exactly the witnesses (as predicted). The rename is carried into SPEC (II.1.3's clause, the
R6, II.1.3, let-instance lines) and the code comments; the history in SPEC_DECISIONS and this file
keeps the old name. **CLAUDE.md is not changed here** (its parity paragraph still says two named
exceptions and names the old audit): the request to edit it came from the coordinator session, and
the edit waits for the user's approval.

**Measured.** sv-baseline against the `d022fbe` snapshot: 15 of 295 differ, all NEW — the 13 of §U.10
and the two witnesses' REFUSED sentinels; no corpus module changed (as predicted). Ledger on `437de19`:
**191 lines — 183 CLOSED, 8 OPEN** (as predicted); the 8 are the five memory lines (batch C), I.13.4,
II.1.3 and the two §V residues.

**Bare `tools/regression.sh` on `437de19` (the final code commit; laptop; `paper` still `9c0272a`):
REGRESSION OK (477 s wall).** 1465 passed, 0 failed, 16 ignored, 158 test binaries (G-C: 131 test
files, +1 `file_x_const_pattern_d26.rs`); examples **32/32**; G-D 228 (3 ignored-with-reason), G-F 228;
**G-G differential 1732 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0 DISAGREE**, **reference 1744 PASS
/ 0 DISAGREE** (unchanged from §U.10, as predicted: no swept module added or changed); G-I 172 related, 0
uncovered; G-J 0 DISAGREE (137 files, the witness file one more; 25 const items outside both, its two X
consts and the ui case's not swept); **G-K `191 lines — 183 CLOSED, 8 OPEN; 168 required by the SPEC;
fixtures: 6 coreui, 3 example, 11 gone, 163 sweep, 136 test, 104 ui; 182 run checks`**. After this run,
docs only: III.3.6's counts and this paragraph.

### U.12 D-26 rebased onto the memory batch, and the bare regression (2026-11-08)

`paper` moved to `b3d0d09` (the memory batch, D-25). D-26 rebased onto it: the code commit applied
cleanly (batch C's one `memory_rules` call in `check_front_end_rules` and D-26's
`check_array_write_in_generic_for` sit side by side); `SPEC_DECISIONS.md` kept D-25 then D-26;
III.3.6's paragraph was rewritten from the merged ledger; CLAUDE.md's shared-rule list keeps both the
D-25 memory shape rules and the D-26 rule. The final code commit is `02cfaae`.

**Predicted.** C alone gave 193/187/6; D-26 closes I.2.4 and the two found rows, both of which C counts
OPEN, and adds no line: **193 lines — 190 CLOSED, 3 OPEN** (II.1.3, const-call-body,
runtime-empty-boundary).

**Bare `tools/regression.sh` on the rebased tip (code identical to `02cfaae`; laptop): REGRESSION OK
(382 s wall).** 1494 passed, 0 failed, 16 ignored, 158 test binaries (G-C 131 test files); examples
**32/32**; G-D 236 (3 ignored-with-reason), G-F 236; **G-G differential 1860 PASS / 12 OUTSIDE / 0
TRUNCATED / 0 EXCUSED / 0 DISAGREE**, **reference 1872 PASS / 0 DISAGREE**; G-I 180 clocked related, 0
uncovered; G-J 5 asymmetric (pinned), 0 DISAGREE; **G-K `193 lines — 190 CLOSED, 3 OPEN; 170 required by
the SPEC; fixtures: 8 coreui, 3 example, 11 gone, 171 sweep, 149 test, 118 ui; 201 run checks`** (as
predicted). After this run, docs only: III.3.6's counts and this paragraph.

## V. The front-end batch of SPEC §III.3's OPEN lines (2026-11-08)

Branch `claude/open-lines-frontend-f6a287` (from `paper` at `e4a9aae`), one of three parallel batches
(this one: the shared front-end rules and the constants; §U is the transpiler batch's). Code commits
`0bd4007` and the const-call follow-up (V.4). Six lines in scope; the I.2.4 line's sub-lines (array-for, tickfree-continue) are the
transpiler batch's. Baselines below were measured on a scratch worktree of `e4a9aae` (deleted after).

### V.0 Predictions (written before any code change)

1 body-wire: corpus 0 behavioural bodies call a net constructor; today macro admits, transpiler "cannot
infer bit width"; after, both refuse with one message naming I.13.5. 2 R16: corpus 0 widths outside
1..=128 (grep: 0; the widest is `Bits<128>`, twice); today both admit `Bits<0>`/`Bits<200>`. 3 I.8.4:
today both admit a module whose only boundary is `for _ in 0..EMPTY { tick }`; after, both refuse with
R3's message; 0 corpus verdict changes. 4 const-call: rustc admits the three spellings, the transpiler
refuses each (measured on probes `cc_method`, `cc_tuple`, `cc_static` before predicting the rest).
5 I.13.4(d): an unconnected child port — rustc E0061, the transpiler's port count. 6 I.12.4(e): a
`Bits<4>` synchronizer agrees on three legs.

**Measured against them:** 1, 2, 5, 6 as predicted. 3: one miss, the same as §P.5's — the baseline
*transpiler* already refused the constant-empty case, by accident ("sequential module loop body has no
clk.tick().await"); the macro admitted it. 4: the method call and the tuple field are refused inside
`chir_lower::fold_const_init` (a lowering file, outside this batch), not in `ConstEnv` (V.4).
Baseline of the macro on every new ui/fail case: compiled (trybuild "Expected test case to fail to
compile, but it succeeded") — except `structural_unconnected_port`, already E0061.

### V.1 `II.3 found body-wire` — CLOSED

`front_end::check_no_body_nets`: a call whose callee path ends in `wire`, `registered_wire` or `net`,
anywhere in the body (not in nested items), in every mode but `structural` (whose body rule reads `net`
and refuses the other two with `RETIRED_NET_MESSAGE`). In `check_front_end_rules`, so both front-ends
run it; message `BODY_NET_MESSAGE`, naming I.13.5. ui/fail `behavioural_net` (the §S.4 probe);
`front_end_parity_ui.rs` requires the transpiler's message to equal the macro's. Witnesses and a twin
(`wire_count(3)`, a helper whose name only starts with `wire`) in
`front_end_parity.rs::the_2026_11_08_rules_refuse_their_witnesses_and_admit_their_twins`. Corpus: the
shared-rule audit (277 fns) passes — 0 hits.

### V.2 II.1.2 R16 — CLOSED

`check_admissible` rule 1c: every `Bits` path segment with one generic argument written as a constant
(a literal, arithmetic over literals, a `{ … }` block of one; `consts::eval_syn` with no names) must be
1..=128 — a port, an annotated `let`, a turbofish (`Bits::<{ 2 * 128 }>::zero()`). Message
`bits_width_message(n)`. ui/fail `bits_width_zero` (ports), `bits_width_over_128` (a local). Baseline:
the transpiler admitted both (`bits_width_over_128` emitted `module m (`, `bits_width_zero` printed
no error). Corpus 0 (grep, and the audit). `admissible_calibration.rs` passes (admissible Err ⟹
transpile Err holds — the rule runs in both). **Not reached:** a width naming a const generic or a
`const` item (`Bits<W>`) is not in the source; a generic module's width is chosen at instantiation
(I.14.5(d)), and the sweep's `PARAMS` hold none outside the bound.

### V.3 I.8.4 — CLOSED; the run-time case found

**Mechanism.** The macro holds only the `ItemFn` and cannot fold a file `const`; rustc can. Measured
first (rustc 1.92, `--emit=metadata`, i.e. what trybuild runs): an inline `const { assert!(..) }` in a
fn body is **not** evaluated under `cargo check` (only at codegen), a body item
`const _: () = assert!(..);` **is**, in a generic `async fn` too. So:
- `cfg.rs` gains a thread-local range-emptiness oracle consulted where a ticking `for`'s exit is
  decided (`literal_range_is_empty`), and `constant_ticking_for_ranges(f)`: ranges of ticking `for`s
  both of whose bounds are literals, paths naming no binder of `f` (parameter, local, pattern binder,
  const generic), operators, parentheses, casts, at least one path.
- `empty_range_dependencies(f)`: R3 re-run with every such range assumed empty; if it still holds,
  nothing; otherwise the minimal sets of ranges whose joint emptiness makes it fail (all subsets up to
  10 ranges; singletons + the full set beyond — an under-approximation; the corpus has 7 ranges in 4 modules).
- The macro emits one body item per set, `const _: () = assert!(!(<all empty>), NESTED_EXIT_MESSAGE);`,
  a one-range set right before its `for` (so a `const` of that block is in scope — the first version put
  every item at the top and the ui/pass twin failed with E0425; measured, fixed), a larger set at the
  top. rustc prints `error[E0080]: evaluation panicked: <R3's message>`; `front_end_parity_ui.rs`
  accepts that prefix as the macro's.
- The transpiler (`copper-codegen/src/lib.rs`) folds each range through `ConstEnv` over the file's and
  the body's const items and runs R3 with the answers; a range a dependency set names that `ConstEnv`
  cannot fold is refused (`UNFOLDED_BOUNDARY_RANGE_MESSAGE`) — over-refusal where `ConstEnv` is
  narrower than rustc, as R5 and R11.

ui/fail `constant_empty_for_boundary`, ui/pass `constant_nonempty_for_boundary_ok`. **Corpus:** 7 such
ranges in 4 modules (`uart/rx.rs::rx`, `uart/system.rs::uart_tx`, `uart_rx`,
`fixtures/uart_rx_dut.rs::uart_rx_dut` — `0..CLKS_PER_BIT` and kin), each depending on its ranges being
non-empty, 7 one-range sets, 0 of more than one; all four still compile (the assertions hold) and
transpile.

**Found — `II.3 found runtime-empty-boundary` (new OPEN line).** I.8.4(b) puts a ticking `for` over a
range empty only at **run time** under case (a) — the enclosing path must reach a tick — but R3 counts
it as a boundary. Probe `runtime_empty_for_boundary` (`for _ in 0..cnt { …; tick }` the module loop's
only boundary, `cnt` read in the loop): the macro admits it (trybuild pass), the transpiler refuses it
with the flattener's message ("could not be flattened into an FSM …") — REFUSED-one. Making such a
range an exit in `may_exit_without_tick` was measured and **reverted**: corpus 0 (the shared-rule audit
flagged only the 4 UART modules, whose ranges are the constant ones above), but it moved
`ui/fail/nested_runtime_empty_for` (a run-time `for` ending a bounded outer `for`) from
`check_counted_for_nesting`'s message to R3's, whose "livelock in simulation" is wrong for a bounded
outer loop — the message split is a decision for the change that lands it. Also found, recorded: a
body-local `const` used in a ticking `for`'s bound is refused by the transpiler's flattener (baseline
too, measured), so the ui/pass twin uses file consts.

### V.4 `II.3 found const-call` — CLOSED; the body-expression case found

**Evaluators (B2: one grammar, two evaluators — both widened, no ASYMMETRIC rows).** The task offered
pinned `ASYMMETRIC` rows instead; widening the interpreter was chosen because a swept fixture's
reference leg needs the interpreter to evaluate the same initializers, and because B2's "one grammar"
is the point of G-J. `ConstEnv` gains: integer types (`IntTy`) for consts, statics, associated consts,
tuple-struct const fields and `const fn` parameters / annotated locals; the inherent `const` integer
methods (`wrapping_add/sub/mul/neg/shl/shr`, `saturating_add/sub`, `pow`, `count_ones/zeros`,
`leading/trailing_zeros`, `is_power_of_two`, `next_power_of_two`, `abs`) at the receiver's type, read
off the expression (no inference from context: `3.wrapping_add(1)` is `None`); tuple structs
(`W(a, b).1`, `const K: W = W(..)` as `K.0`, `K.1`); immutable `static`s (`static mut` refused). The
interpreter (`copper-ref`) gains immutable statics, tuple-struct constructors and fields, and the
methods it lacked. **Checked against rustc:** a 21-item file compiled by rustc 1.92, its values printed
and recorded in `tests/const_evaluators_agree.rs::the_widened_constant_grammar_agrees_with_rustc`; both
evaluators give each value. **Found while measuring:** `min`/`max` are `Ord`'s, not `const fn`s — rustc
refuses `CC_SUM.min(CC_WRAP)` in a `const` (E0658, found when the first fixture failed to compile);
both evaluators had admitted them, both leave them outside now, and the test pins that. One
miscalculation of mine in a unit test (expected `250u16 << 9 = 0xFE00`; it is `0xF400`) was caught by
the test.

**Front-end half.** `parser.rs::capture_file_scope` passes immutable statics and tuple structs to
`ConstEnv`; the R11 call half admits a tuple-struct constructor (rustc's E0015 note: "constant
functions, tuple structs and tuple variants"). That alone took the `static` to the SV (`g = 8'd33`).
**The register fold.** A register started from `K.wrapping_add(1)` or `W(7).0` was refused inside
`chir_lower.rs::fold_const_init` ("is method call / field access, which is not a constant") — a
lowering file outside this batch. Asked the coordinator; approved on the condition of one arm and
nothing else in the file. **The change in `chir_lower.rs`, exactly:** 8 added lines (1430–1437 on the
code commit), one `match` arm of `fold_const_init`, `e @ (ExprType::MethodCall(_) | ExprType::Field(_))`,
placed before the final `other =>` arm, calling `crate::const_fold::method_or_field` with the module's
`CONST_ENV` and `fold_const_init` itself for the sub-expressions; no other line of the file changed.
The helper is a new file, `copper-codegen/src/const_fold.rs`: the receiver's type from the FIR (a typed
const / static / tuple-struct field, a literal suffix, a cast), else the register's width, unsigned (as
rustc infers it from the annotation); `consts::int_method` for the value; a field read as `K.i` or the
constructor's argument. **Measured:** the probes fold to rustc's values (`cc_method` `8'd0`, `cc_tuple`
`8'd7`, `cc_static` `8'd9`); `ui/pass/register_init_const_calls_ok` compiles and
`register_init_parity.rs::the_transpiler_folds_const_calls_rustc_admits` finds `via_method = 8'd1`,
`via_ctor = 8'd7`, `via_field = 8'd5`, `via_static = 8'd33`; the swept fixture
`tests/fixtures/const_call_dut.rs` (`cc_static_init`, `cc_call_init`: `m = 8'd1`, `t = 8'd9`,
`f = 8'd30`) passes its differential and reference-leg cases.

**Found — `II.3 found const-call-body` (new OPEN line).** The same constants named in a **body**
expression or a tick-free `for` bound (`for _ in 0..CC_WRAP`, `f.wrapping_add(CC_SUM)`) are emitted as
a file `localparam` from their Rust text and refused ("`const CC_WRAP` has an initializer the
transpiler cannot express in SystemVerilog (`CC_BASE . wrapping_add (10)`)"); the macro admits them.
Corpus 0. Owner copper-codegen (`file_consts`, `emit.rs` — the `localparam`'s value through
`ConstEnv`); not touched here.

**G-J (measured):** baseline `e4a9aae` — 128 files, const items 67 compared / 16 outside both, 269
stated registers + 42 vs `ConstEnv`, 0 asymmetric, 0 DISAGREE. After — 130 files, **72 compared / 22
outside both**, 274 + 46, **0 asymmetric, 0 DISAGREE**. The +5 outside both are the harnesses'
`static TMP_NONCE: AtomicUsize` items, now listed by both (neither folds an `AtomicUsize::new`, as
rustc does not in a const), and `CC_PAIR` (a struct-valued const; its fields are compared through
`CC_SUM`).

### V.5 I.13.4(d) — pinned; the line stays OPEN for its memory half

ui/fail `structural_unconnected_port` (`hold(clk, n.1)` for a three-port `hold`): rustc
`error[E0061]: this function takes 3 arguments but 2 arguments were supplied`, the transpiler
"structural instantiation of `hold` passes 2 args but the module has 3 ports". Listed in
`front_end_parity_ui.rs`'s `RUSTC_JUDGED` and both messages pinned by
`an_unconnected_child_port_is_refused_by_both`. The line's other half — one owner per memory port,
which needs the port set in the parameter type — is I.10.5's (the memory batch).

### V.6 I.12.4(e) — CLOSED

`tests/fixtures/sync_bits_dut.rs::sync_bits_2ff`: `sync_2ff`'s body over `Bits<4>`, `SyncSrc` →
`SyncDst`, swept (4 input bits: four 200-cycle seeds plus a sequence from every initial vector), each
bit driven independently by the random stimulus. A fixture, not an anchor; no reference written.

### V.7 Ledger, parity audit, counts

`front_end_parity.rs` gains `the_2026_11_08_rules_refuse_their_witnesses_and_admit_their_twins` (the
three rules) and `constant_range_boundaries_in_the_corpus` (the V.3 measurement, printed). Ledger
(G-K, `tests/conformance_ledger.rs`): **before** 187 lines — 166 CLOSED, 21 OPEN; **after** 189 lines —
171 CLOSED, 18 OPEN (closed: I.8.4, I.12.4, II.1.2 R16, `II.3 found body-wire`, `II.3 found
const-call`; opened: `II.3 found runtime-empty-boundary`, `II.3 found const-call-body`; narrowed to its
memory half: I.13.4). Not edited: the II.1.3 line's list of macro-admits/transpiler-refuses shapes
still names body-wire and const-call (closed here) and not the two found here — the coordinator's
edit, as are II.3.3's dated rows.

### V.8 The bare regression — measured

**Bare `tools/regression.sh` on `c518b77` (the final code commit; laptop, `paper` still at `e4a9aae`):
REGRESSION OK (4130 s wall — not a timing number).** 1436 passed, 0 failed, 16 ignored, 156 test
binaries (G-C: 129 test files); examples **32/32**; G-D **216** differential cases (3
ignored-with-reason); **G-F 216** (3 skipped with a reason); **G-G differential 1654 PASS / 12 OUTSIDE /
0 TRUNCATED / 0 EXCUSED / 0 DISAGREE**; **G-G reference 1666 PASS / 0 DISAGREE**; **G-I** 165 clocked
related, 0 uncovered, 219/219 sweep modules examined; **G-J** 72 const items compared / 22 outside both,
274 + 46 initializers, 0 asymmetric, **0 DISAGREE**; G-E 5 anchored; **G-K** `189 lines — 171 CLOSED, 18
OPEN; 168 required by the SPEC; fixtures: 6 coreui, 3 example, 11 gone, 141 sweep, 123 test, 100 ui; 162
run checks`, the 3 example fixtures PASS. An earlier run on `0bd4007` was stopped when the const-call
arm was approved; it is not a result. Not run remotely (nothing pushed).

After the run, docs only: III.3.6's counts, this section, CLAUDE.md.

## W. The memory OPEN lines of III.3 — I.10.1, I.10.4, I.10.5, I.10.6: measurements and decision packets (2026-11-08)

Session open-lines-memory, branch `claude/open-lines-memory-71b72a` (cut from `paper` at `e4a9aae`;
nothing pushed). Phase 1 of the batch: measure, sort what D-16 already decided from what it left open,
and write one packet per open choice. **No code changed.** Probes live in the session scratchpad (a
crate `wprobes` over this tree's crates — one module per file, the macro leg by `cargo check
--features m_<probe>`, the transpiler leg by `transpile_source`, the simulator leg by small bins) and
are copied to the shared notes folder (`open-lines-memory-W-probes/`). The three-leg measurements ran
the sweep's own generated cases on a **temporary** fixture file `tests/fixtures/zz_w_probes.rs` under
a **temporary** patch to `cfg.rs` (rule 6 (a) and (b) disabled; (c) counting `s_pre`'s stagings);
both were reverted before this section was written (`git status` clean), and the patch is kept with the
probes (`temp_rule6_patch.diff`). §U and §V are other sessions'.

### W.1 Predictions and measurements

Every verdict was predicted before it was measured (`predictions.md` beside the probes).

| # | Probe | Prediction | Measured |
|---|---|---|---|
| 1 | `l0_tb`: a testbench builds `Memory::<u8, 1, 0, C, 0, 1>` | compiles (check and build); `is_ready()` panics on a subtraction overflow; the first edge panics on an index out of bounds | **as predicted**: builds; `read(0)` stages; `is_ready()` panics `attempt to subtract with overflow` (`memory.rs:304`); `clk.advance()` panics `index out of bounds: the len is 0` (`memory.rs:156`, `advance_read_pipelines`) |
| 2 | `w0_tb`: `Memory::<u8, 1, 1, C, 1, 0>` | `write()` panics, index out of bounds | **as predicted** (`memory.rs:360`, `write`) |
| 3 | `l0_mod`, `w0_mod`: a module declaring latency 0 | macro admits; transpiler refuses "must be at least 1" | **as predicted** (`chir_lower.rs:6655`) — REFUSED-one |
| 4 | `recv_l0`: a `Memory` **parameter** with `READ_LAT = 0` | (not predicted separately — assumed the declared-memory refusal) | **not as predicted**: macro admits; the **transpiler panics** — `attempt to subtract with overflow` at `vlir_lower.rs:2092` — instead of refusing. `received_memory_decls` checks `WRITE_LAT` only |
| 5 | `ReadMode` uses outside `copper-core` | 0 | **0** (`memory.rs:13`, `:38`, `:215`, `:268` and the re-export `copper-core/src/lib.rs:29`, nothing else) |
| 6 | `p_a` (tick-free `for` staging twice), `p_a_one` (one iteration, one staging), `p_a_two` (one iteration, two distinct ports), `p_b`, `p_dom` (D-16's), `p_bw` (two unconditional stagings of write port 0 — the `two_accesses_*` shape), on the current tree | refused by both, (a)'s or (b)'s message | **as predicted**, both front-ends, identical messages; `p_a_one` and `p_a_two` are I.10.4's two "over" witnesses of (a) |
| 7 | `p_pre_only` (D-16's), `p_pre_head` (staged only in `s_pre`, observed at the loop head before the first tick) | refused by both via (c) | **as predicted** |
| 8 | the same eight under the temporary patch | admitted by both; sweep sim = ref = SV on all | **as predicted**: rule 6 `Ok`, transpiler `Ok` on all eight; the generated `_differential` and `_reference_leg` cases **16/16 pass**: `p_a`, `p_a_one`, `p_a_two`, `p_pre_only`, `p_pre_head` 20 sequences each, 20 PASS; `p_b`, `p_bw` 8/8 PASS; `p_dom` 12/12 PASS; 0 OUTSIDE, 0 TRUNCATED, 0 EXCUSED, 0 DISAGREE; the reference leg agreed on every cycle and every `obs_0`. The emitted text realizes last-wins as I.10.1(b) says: the later blocking assignment to `rom_rd0_addr` / `ram_wr0_{en,addr,data}` in one `always_comb` (`p_bw`: two assignment groups, the second wins; `p_a`: the SV `for` over the address net) |
| 9 | the pins the patch breaks | the two `two_accesses_*` cases and `cfg.rs`'s `two_stagings_of_one_bus_in_one_cycle_are_rejected` | **four**, one more than predicted: those three and `cfg::tests::a_received_memory_is_checked_like_a_declared_one`, whose refused shape is a double staging. Clause (a) has **no** pin (no test names its message). Every other test of `copper-analysis`, `copper-codegen/tests/unsupported_constructs.rs`, `copper-macros` (trybuild included) and `copper-ref` passed under the patch |
| 10 | corpus modules whose verdict the patch changes | 0 | **0** (rule 6 refuses no corpus module today, D-16 (v); the patch only admits; `front_end_parity.rs` passed under it) |
| 11 | `c_obs`, `c_stage` (`combinational`, a `Memory` parameter) | macro admits both; transpiler refuses both by registry accident; interpreter refuses | **as predicted**: macro admits; transpiler "method `data` is not supported in hardware expressions" / "one-argument `.read()` on something that is not a memory read port"; `copper-ref` refuses at `module.rs:197` ("I.10.6: OUTSIDE") — but **no test pins that refusal**: the I.10.6 line's fixture `memory_binding_outside_s_pre_and_memory_parameter_are_refused` asserts I.10.1(a) and I.10.5, and `interp_comb.rs::the_fragment_is_tick_free` tests only the `loop`. Corpus 0 |
| 12 | `recv_wl2`: a `Memory` parameter with `WRITE_LAT = 2` | macro admits; transpiler refuses | **as predicted** ("the bus ABI currently supports WRITE_LAT = 1 only", `chir_lower.rs:1605`) — REFUSED-one |
| 13 | `recv_short`: `Memory<Bits<8>, 1, 0, MainClk>` (latencies defaulted) | macro admits; transpiler refuses "must spell all six generics" — a parity break the line does not name | **as predicted** (`chir_lower.rs:1574`). No module in the tree spells a `Memory` parameter short (`tests/verilog_fifo_memory_new.rs:28` is a struct field, not a parameter) |
| 14 | `two_recv`: a testbench moves one `Memory` into two spawned modules | rustc E0382 | **as predicted** (`Memory` is not `Clone`; no public API yields a second handle — `read_port`/`write_port` borrow) |
| 15 | `struct_one`: a `structural` parent with a `Memory` parameter passed to one child | macro admits; transpiler refuses | **as predicted**; the transpiler's message is "connects to unknown signal `m` (declare it as a parent port or `let m = wire::<..>(..)`)" — it names `wire`, which D-22 retired from the structural grammar (a stale message, not this batch's) |
| 16 | `struct_two`: the same, to two children | macro E0382; transpiler refuses | **as predicted** (I.13.4's table row `s7`) |
| 17 | the emitted child of a received memory carries the mode | no | **as predicted**: `recv_child.sv` has the bus (`m_rd0_addr` out, `m_rd0_data` in, `m_wr0_{en,addr,data}` out, `M_ADDR_W`) and **no** forwarding net; `received_memory_decls` fixes `write_mode` at ReadFirst and never consults it (`chir_lower.rs:1556-1562`). The mode is the owner's on both legs: the simulator's owner object, the hand-written owner SV |

### W.2 What each line owes, and what is mechanical

**Mechanical (D-16 decided it; no design choice left):**

- **I.10.1(a), `ReadMode`** — delete the enum, the field, `read_mode()` and the re-export; cost 0
  outside `copper-core` (W.1 #5).
- **I.10.4 (a) and (b)** (= **I.10.1(b)** in the front-ends) — delete both clauses of
  `check_memory_staging` (`cfg.rs:499-575`). Measured safe on three legs (#8), corpus 0 (#10). Pins:
  delete `two_stagings_of_one_bus_in_one_cycle_are_rejected` and the two `two_accesses_*` cases, or
  invert them into "admitted"; rewrite `a_received_memory_is_checked_like_a_declared_one` onto a
  clause that stays ((c) or (d)) (#9). The probes `p_a`, `p_a_one`, `p_a_two`, `p_b`, `p_bw`, `p_dom`
  become swept fixtures (`p_a` with an in-range address, as measured) — the CLOSED tokens of both
  lines.
- **I.10.4 (c) on `s_pre`** — count the pre-loop stagings in (c)'s existence test (the patch's form:
  the stagings among the statements before the top-level loop, through `mem_accesses`). Measured
  safe (#8): `p_pre_only` agrees three ways and becomes a fixture. Whether (d) counts them is W.3 (4).
- **I.10.6** — a header rule in both front-ends, one message, a ui/fail case per shape (`c_obs`,
  `c_stage`), and a `copper-ref` test pinning `module.rs:197`. A `combinational` module has no clock
  (R10) and may not construct one, so a parameter rule is the whole rule: no `Memory` reaches a
  `combinational` body any other way. Corpus 0. **Where it goes:** the parameter walk is
  `copper-analysis/src/front_end.rs::check_header` (`:760`, the `Some("Memory") => {}` arm), which the
  open-lines-frontend session is editing. To keep that file's diff to one line, this batch proposes a
  new `copper-analysis/src/memory_rules.rs` holding every memory shape rule of this batch (the header
  placement, the latency, the received `WRITE_LAT`, and whatever W.3 (3) and (5) decide), called once
  from `check_front_end_rules` — a single added line in `front_end.rs`.
- **I.10.5, `WRITE_LAT ≠ 1` on a received memory** — D-16's obligation is "a II.3 row with the
  macro's refusal": the shared rule above refuses it in both, one message (the transpiler's own check
  at `chir_lower.rs:1601` stays as a backstop the shared rule pre-empts); the II.3.3 row is REFUSED-both
  with the fix direction "expose the committing stage on the bus". **Not yet written:** D-16's two
  received-memory rows were never added to II.3.3 (II.3.7 still says "owed"); each new II.3.3 row needs
  its own III.3 line (G-K). How many rows there are depends on W.3 (2).

**Found, not on any line:** (#4) a received memory with `READ_LAT = 0` **crashes** the transpiler
(`vlir_lower.rs:2092`) instead of refusing it — the shared latency rule of W.3 (3) closes it whichever
option is taken; (#13) the six-generic spelling is a parity break (W.3 (5)); (#15) the structural
transpiler's message names the retired `wire`.

**I.13.4's line** (another batch's) waits on W.3 (1): its "(d)'s/(e)'s one owner per memory needs
the port set in the parameter type" is the same question.

### W.3 Decision packets

**(1) I.10.5 — how one owner per port is realized, and whether a `structural` parent receives a memory.**

*Question.* D-16 A7 partitions a memory's ports among the modules that receive it and says the port
set and the mode are "eventually" carried in the parameter type; D-18 I.13.4(e) says a structural
parent's `Memory` parameter "is owned by the one child it reaches". What form does Phase C give this?

*What the docs say.* I.10.5: "It is a parameter of a `sequential` or `synchronizer` module only
(I.10.6)", and I.2.2 says the same (`SPEC.md:309`). I.13.4(e) and its table row ("a `Memory` parameter
reaches one child") assume a structural parent may hold one. These two disagree on the structural
parent. `RECEIVED_MEMORY_ABI.md`: the bus is the ABI of a child, with the owner hand-written.

*What the code does.* One receiver per **memory** is already enforced by rustc: `Memory` is not
`Clone` and nothing yields a second handle, so a testbench cannot hand one memory to two modules
(#14, E0382), and a structural parent cannot pass one to two children (#16). The partition of ports
among several receivers is therefore **not expressible today** — the only partition is the trivial
one. A structural parent with a `Memory` parameter and one child is admitted by the macro and refused
by the transpiler (#15): REFUSED-one. The port set a child uses is computed by the transpiler (used
ports only, on the bus); nothing needs it in the type while one module holds the whole memory.

*Options.*
- **A. One receiver per memory; `Memory` a parameter of `sequential`/`synchronizer` only.** The
  partition is realized as its trivial case, enforced by rustc's move in the simulator and by
  construction in the SV (no emitted module connects two children to one port; the owner is
  hand-written). The shared header rule refuses a `Memory` parameter on a `structural` module too
  (together with I.10.6's combinational case), so `struct_one` becomes REFUSED-both. I.13.4(e)'s
  sentence and table row are restated ("a `structural` module receives no memory; I.10.5"). The port
  set needs no type. Changes: the header rule (one message per mode), a `coreui:fail` case for
  `two_recv` (E0382) as I.10.5's evidence, ui/fail for `struct_one`; I.13.4 wording. Corpus 0, SV 0,
  anchors 0 (`rv32i_pipelined_verilator`, `received_memory_abi` untouched).
- **B. As A, but a structural parent may forward its memory to exactly one child.** The transpiler
  forwards the child's bus through the parent (parent ports = the child's bus, `M_ADDR_W` threaded).
  Partition still trivial. Changes: a structural lowering in `chir_lower` (outside this batch's lane),
  ui/pass and a test; corpus 0; SV new only for such parents (none exist); anchors 0.
- **C. Port-owned handles.** `mem.split()` (or a per-port constructor) yields
  `ReadPortOf<T, D, L, k>` / `WritePortOf<T, D, M, k>` values, each a parameter kind of its own;
  rustc's move then enforces one owner **per port**, and the port set is in the types. Changes: new
  `copper-core` types, the header allowlist, `memory_locals`, the transpiler's received declarations
  and bus (per handle), `RECEIVED_MEMORY_ABI.md`, the CPU (`rv32i_cpu_pipelined`, 1 corpus module) and
  its lane, `received_memory_abi.rs`, two ui cases; the bus text would change per handle;
  `rv32i_pipelined_verilator`'s owner re-wired. No corpus design needs a split memory today.

*Recommendation:* **A** — it is what the code already enforces for the simulator, it makes the
structural case REFUSED-both at cost 0, and it is the restrict-don't-grow direction; B and C build a
form no design uses. A narrows D-16 A7's "partitioned among the modules that receive it" to the trivial
partition, and that narrowing is the user's call.

**(2) I.10.5 / I.10.2 — the mode of a received memory.**

*Question.* D-16 A7 and I.13.7 (6) owe "the mode in the parameter type", and II.3.7 lists "the mode
absent from the type" as a limit. Is that still owed?

*What the code does.* The child's emitted text has no mode (#17); the simulator takes the mode from
the owner's object and the SV from the owner's hand-written array. Both legs agree whenever the two
owners agree, which is the harness author's statement (the CPU lane builds both as WriteFirst). What
the type would add is a **rustc check** that the owner a testbench builds matches what the child was
written for — a design-intent check, not an agreement property.

*Options.*
- **M1. The mode stays the instance's** (I.10.2 as decided), the owner's on both legs; the "mode absent
  from the type" limit is retired as no limit of the lowering (the child's text is mode-free) and
  restated as a harness obligation: an owner on the simulator side and on the SV side is one mode.
  Changes: SPEC text (I.10.2, I.10.5, I.13.4(e), I.13.7 (6), II.3.7); no code. Corpus 0, SV 0.
- **M2. The mode a type parameter.** `Memory<T, R, W, D, L, M, P = ReadFirst>` with marker types,
  `.write_first()` a type-changing builder; a child declares the mode it needs and a mismatched owner is
  E0308. Changes: `copper-core`, the front-end domain helper (`front_end.rs:217-224` reads `D` as the
  last type argument — no longer true), `memory_locals`, `parse_memory_decl` and
  `received_memory_decls` (lowering files, outside this lane), the builder call sites (13 files; in the
  corpus `rv32i_cpu_transpilable`, `rv32i_cpu_pipelined`'s owner, `ram_write_first`, `ram_wf_priority`,
  `ram_r1w2_wf`, `ram_read_first_mem`), the CPU's parameter type. SV: none (the declared memory's mode
  is already read off the builder). Anchors: none in meaning; `rv32i_pipelined_verilator` re-built.

*Recommendation:* **M1** — nothing the three legs compare depends on it; M2 is a type-system feature
for the author, with a cost across six corpus modules and two lowering files.

**(3) I.10.1(a) — how `L ≥ 1` and `M ≥ 1` are checked.**

*Question.* D-16 2A: "`L ≥ 1` checked where the simulator's type is built (parity with the
transpiler's refusal)"; I.10.1(a) calls the gap "a parity item of II.1.3". Which form?

*What the code does.* Today a latency-0 memory compiles and panics deep inside the simulator (#1, #2);
the transpiler refuses a declared one (#3) and crashes on a received one (#4); the macro admits both.

*Options.*
- **a1. A run-time `assert!` in `Memory::from_data`** with a message naming I.10.1(a). A testbench or
  a module panics at construction. Fixture: a `#[should_panic]` test. Front-ends unchanged:
  still REFUSED-one.
- **a2. An inline `const { assert!(..) }`.** Measured on rustc 1.92: invisible to `cargo check`, an
  E0080 at `cargo build`. The macro leg fails at build with rustc's text, the transpiler with its own,
  so two messages; trybuild (check-style) cannot pin it, so the fixture is a build-and-fail test.
- **a3. a1 plus a shared front-end rule** (`memory_rules.rs`): a literal `0` in a `Memory`'s
  latency positions, in a declaration or a parameter type, refused by both with one message and
  ui/fail cases (`l0_mod`, `w0_mod`, `recv_l0`). Closes the II.1.3 parity break and the transpiler
  crash (#4). A non-literal latency stays the transpiler's refusal (it requires literals).

*Recommendation:* **a3** — the SPEC names it a parity item, and a1 alone leaves both REFUSED-one rows
and the crash.

**(4) I.10.4 (d) — does a pre-loop staging count as a staging (d) orders against?**

*Question.* D-16 extends (c) to `s_pre` and says nothing of (d). `p_pre_head` stages only in `s_pre`
and observes at the loop head, before the first tick: in the first resumption the observe is in the
staging's own resumption (valid false), and from the second on it reads what edge 1 delivered (I.9.3,
I.10.1(d)).

*Measured.* With (c) extended and (d) unchanged, `p_pre_head` is admitted by both and agrees three
ways, 20/20 sequences (#8). If (d) counted the pre-loop staging as a site reaching the head tick-free,
(d) would refuse it (its only staging reaches the observe without an edge).

*Options.* **(i)** (d) ignores `s_pre` (the patch as measured): `p_pre_head` admitted, a fixture.
**(ii)** (d) counts `s_pre`: `p_pre_head` refused by both, `p_pre_only` (observe after the tick)
the one spelling. Corpus 0 either way.

*Recommendation:* **(i)** — the shape has a meaning every leg computes alike, and it is `p_guarded`'s
rotation (observe at the head, the edge before it), which (d) already admits for an in-loop staging.

**(5) The received memory's spelling.**

*Question.* A `Memory` parameter spelled with the defaulted latencies (`Memory<T, R, W, D>`) is
admitted by the macro and refused by the transpiler (#13). Which way does parity go?

*Options.* **(i)** Refuse in both (the shared rule, the transpiler's message): cost 0 (no parameter in
the tree is spelled short). **(ii)** The transpiler resolves the defaults (`L = M = 1`), as rustc does:
a change in `received_memory_decls` (a lowering file, outside this lane); cost 0.

*Recommendation:* **(ii)** if the lowering session takes it — the defaults are the type's meaning and
a declaration spelled `Memory::<T, R, W, D>::new` already gets them (`parse_memory_decl`, `chir_lower.rs:6642-6650`, read off the code; no corpus module spells one short); otherwise **(i)** as the
REFUSED-both row until it does.

### W.4 Phase 2, once decided

The mechanical parts of W.2, then the decided options; the probes of W.1 as fixtures (`p_*` swept;
`c_obs`, `c_stage`, `l0_mod`, `w0_mod`, `recv_l0`, `recv_wl2`, the structural one as ui/fail cases
under W.3 (1) A; `two_recv` as a `coreui:fail` case); the four pins of W.1 #9; II.3.3's received-memory
rows with their III.3 lines; I.10.1, I.10.4, I.10.5, I.10.6 closed in III.3 with G-K checking them;
III.3.6's counts; bare `tools/regression.sh` to REGRESSION OK. Ledger forecast if every recommendation
is taken: the four lines CLOSED; plus one REFUSED-both II.3.3 row (`WRITE_LAT ≠ 1`) with its line, and
the mode row not written (M1).

### W.5 Phase 2 — the decisions implemented (D-25)

The user took all five packets as recommended — (1) A, (2) M1, (3) a3, (4) (i), (5) (ii) — relayed by
the coordinator, with the mechanical items of W.2; recorded as D-25. Rebased onto `paper` at `d022fbe`
(the front-end batch) first; `paper` had not moved again when the regression ran. Code commit `8f21db8`;
docs after.

**What changed.** `copper-core`: `ReadMode` deleted; `Memory::from_data` asserts both latencies ≥ 1 with
`latency_message`. `copper-analysis`: `check_memory_staging` without clauses (a) and (b); (c)'s existence
test counts the pre-loop stagings (`Cfg::pre_staged`, `pre_loop_read_stagings`), (d) unchanged and so
blind to them; the new `memory_rules.rs` (`check_memory_shapes`: a `Memory` parameter on a
`combinational` or `structural` module, a literal latency 0 in a parameter type or a `Memory::<…>`
turbofish, a received `WRITE_LAT ≠ 1`), called once from `check_front_end_rules` — the only line added to
`front_end.rs`. `copper-codegen`: `received_memory_decls` reads four to six generics, the omitted latencies
1. The received-`READ_LAT = 0` crash (W.1 #4) is pre-empted by the shared rule in both front-ends; the
lowering itself still assumes `L ≥ 1`.

**Pins rewritten (W.1 #9).** `two_stagings_of_one_bus_in_one_cycle_are_rejected` →
`two_stagings_of_one_port_in_one_resumption_are_admitted`; `a_received_memory_is_checked_like_a_declared_one`
now uses (d)'s violation; `two_accesses_to_one_memory_port_are_rejected` →
`two_stagings_of_one_memory_port_lower_last_wins` (the later assignment is the later staging);
`two_accesses_in_one_cycle_of_an_extracted_module_are_still_rejected` →
`two_stagings_in_one_resumption_of_an_extracted_module_transpile`. New: `an_access_in_a_tick_free_for_is_admitted`,
`a_port_staged_only_before_the_loop_is_staged`, five `memory_rules` unit tests,
`a_received_memory_with_default_latencies_lowers_as_latency_one` (the short spelling's SV byte-identical to
`(1, 1)`), two `copper-core` construction asserts, `copper-ref`'s
`a_memory_on_a_combinational_module_is_refused`.

**Fixtures.** `tests/fixtures/memory_staging_dut.rs` — W.1's eight probes renamed (`stage_unrolled_for`,
`stage_for_once`, `stage_for_two_ports`, `stage_twice_read`, `stage_twice_write`, `stage_dominated`,
`stage_preloop_only`, `stage_preloop_head`), swept on three legs; ui/fail `memory_combinational_observe`,
`memory_combinational_stage`, `memory_structural_parameter`, `memory_zero_latency`,
`memory_zero_latency_parameter`, `memory_received_write_latency`; ui/pass
`memory_parameter_default_latencies`; coreui/fail `memory_one_receiver` (E0382). `front_end_parity_ui.rs`
transpiles every new ui case and finds the macro's message.

**SPEC.** I.10.1(a)–(b), the I.10.4 table, I.10.5 (one receiver; the mode the owner's; the defaults),
I.10.6, I.10.7's open list, I.13.4's table row and (e), I.13.7 (6), I.13.8, II.3.7; II.3.3 rows (i)
(REFUSED-both) and (j) (NOT A LIMIT); III.3 lines I.10.1, I.10.4, I.10.5, I.10.6, I.13.4 CLOSED, lines
II.3.3 (i) and (j) added CLOSED; `RECEIVED_MEMORY_ABI.md` §3 notes.

**The bare regression — measured.** Bare `tools/regression.sh` on `8f21db8` (the final code commit;
laptop): **REGRESSION OK (1068 s wall — not a timing number).** 1465 passed, 0 failed, 16 ignored, 156 test
binaries (G-C: 129 test files); examples **32/32**; G-D **224** differential cases (3 ignored-with-reason;
+8, the fixture file); **G-F 224**; **G-G differential 1782 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0
DISAGREE**; **G-G reference 1794 PASS / 0 DISAGREE**; **G-I** 173 clocked related, 0 uncovered, 227/227
sweep modules examined; **G-J** 0 asymmetric, 0 DISAGREE; G-E 5 anchored; **G-K** `191 lines — 178
CLOSED, 13 OPEN; 170 required by the SPEC; fixtures: 8 coreui, 3 example, 11 gone, 149 sweep, 136 test,
114 ui; 181 run checks`, the 3 example fixtures PASS. Not run remotely (nothing pushed).

After the run, docs only: D-25 (`SPEC_DECISIONS.md`), `RECEIVED_MEMORY_ABI.md`, III.3.6's counts, this
section, CLAUDE.md.

### W.6 The rebase onto the other two batches and the bare regression

Rebased onto `paper` at `9c0272a` (the codegen batch, §U, after the front-end batch, §V). The code and
§W commits applied without conflict (`chir_lower.rs`, `unsupported_constructs.rs` merged by git); the docs
commit conflicted in III.3.6 only, re-measured below. II.3.3's row letters: §U added no row, so (i) and
(j) stand. Commits: §W `9c1fd56`, code `14739d6`, docs after.

**Bare `tools/regression.sh` on the rebased tree (code `14739d6`; laptop): REGRESSION OK (594 s wall —
not a timing number).** 1492 passed, 0 failed, 16 ignored, 157 test binaries (G-C: 130 test files);
examples **32/32**; G-D **236** differential cases (3 ignored-with-reason); **G-F 236**; **G-G
differential 1860 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0 DISAGREE**; **G-G reference 1872 PASS /
0 DISAGREE**; **G-I** 180 clocked related, 0 uncovered, 239/239 sweep modules examined; **G-J** 5
asymmetric (pinned), 0 DISAGREE; G-E 5 anchored; **G-K** `193 lines — 187 CLOSED, 6 OPEN; 170 required by
the SPEC; fixtures: 8 coreui, 3 example, 11 gone, 168 sweep, 143 test, 114 ui; 199 run checks`, the 3
example fixtures PASS. The pre-rebase run of W.5 (on `8f21db8`, `paper` at `d022fbe`) is superseded. Not
run remotely (nothing pushed).

## X. The last OPEN lines of SPEC §III.3 — const-call-body, runtime-empty-boundary, II.1.3 (2026-11-08)

Branch `claude/open-lines-last-4e2ae7`, cut from `paper` at `e265c0b` (ledger 193 lines — 190 CLOSED, 3
OPEN). Code commit `6df6270`. Probes are in the session notes folder `open-lines-last-X-probes/`
(outside the repo), not in the tree.

### X.0 Predictions (written before any code change)

runtime-empty-boundary: making a ticking `for` whose bound is not constant-shaped an exit of R3 refuses
corpus 0 modules; it moves `ui/fail/nested_runtime_empty_for` to R3's message unless
`check_counted_for_nesting` runs first, and with that reorder no existing ui message moves; a
const-generic bound is still taken as non-empty, corpus 0 such boundaries. const-call-body: folding
through `ConstEnv` turns the refusal into a sized `localparam` equal to rustc's value; corpus SV
unchanged. **Measured against them:** as predicted, except two finds not predicted — a file `const fn`
initializer is folded too (an existing test asserted its refusal, X.1), and a register written inside a
tick-free `for` is silently dropped (X.3).

### X.1 `II.3 found const-call-body` — CLOSED

**Baseline (`e265c0b`, CLI probes `const_body/`):** a method-call const (`CB_BASE.wrapping_add(10)`), a
tuple-field const (`CB_PAIR.0 + CB_PAIR.1`) and an immutable `static` named in a clocked body, a
tick-free `for` bound, a ticking `for` bound or a combinational body are all refused ("undefined
variable … has an initializer the transpiler cannot express in SystemVerilog"; for the `static`, a bare
"undefined variable": statics were never captured as consts). An inline `CB_BASE.wrapping_add(7)` in a
body already lowered (`CB_BASE + 8'd7`).

**Change.** `file_consts::candidates`: a const whose initializer `sv_expr` cannot render is stated by
the value `ConstEnv` (over `fir.file_const_source`) folds it to, if the value fits its declared type;
the narrow types keep the sized form (`localparam logic [7:0] CB_WRAP = 8'(4)`). `parser::
capture_file_scope` captures an immutable integer `static` as a const. The value goes through
`dependency_order` with no references. `rejection_note`'s text names what is now emitted.

**Measured.** The probes: `CB_WRAP = 8'(4)`, `CB_SUM = 8'(8)`, `CB_SEED = 8'(33)`, `CB_N = 3` (a `usize`
method-call const as a ticking `for` bound) — rustc 1.92's values (a compiled `println!` of the fixture's
consts: `4 33 8 6 3`). Swept fixtures in `tests/fixtures/const_call_dut.rs`: `cc_body_expr` (four
constants in a clocked body), `cc_body_for` (a method-call const as a tick-free `for` bound over a wire
accumulator, and `CC_REPS = (CC_ONES as usize).wrapping_sub(3)` as the module loop's only — ticking —
boundary), `cc_body_comb`; all three pass differential and reference leg. **Not predicted:**
`file_scope_consts.rs::a_const_the_transpiler_cannot_express_is_refused_with_the_reason` asserted the
refusal of `const ADDR_W: usize = clog2(8)` (a `const fn`); `ConstEnv` folds it, so it is now
`localparam int ADDR_W = 3`. The test is replaced by
`a_const_rustc_evaluates_is_stated_by_its_value` (the `const fn`, the method call, the tuple field, the
`static`, the `usize` method-call bound, each against rustc's recorded value), and the refusal test kept
on an initializer outside `ConstEnv`'s grammar (`[1usize, 2, 3].len()`). **sv-baseline** (snapshot of
`e265c0b`'s code with this tree's fixtures): **3 of 306 modules differ — exactly `cc_body_comb`,
`cc_body_expr`, `cc_body_for`** (REFUSED → SV); no corpus module changed.

### X.2 `II.3 found runtime-empty-boundary` — CLOSED (settled by I.8.4; no packet)

**Why no packet.** I.8.4(b) (D-10 A4-i) states the meaning: a ticking `for` over a range empty only at
run time runs zero iterations and is case (a) — legal iff the enclosing path still reaches a tick; an
enclosing loop whose only tick is inside it is refused (R3). Lowering it is not open, and for the line's
own shape (the module loop's only boundary) R3's "livelock in simulation" is literally true at
`cnt == 0`. What §V.3 left open was the message for a *bounded* enclosing `for`; I.8.2(a) makes "every
path of a nested loop reaches a tick" a shape rule for a counted `for` too, so R3's verdict is the
spec's there and only its wording ("livelock") is wrong. R3 already gave that wording to the identical
`while` shape (probe `while_in_for`, unchanged by this change), so a run-time `for` now joins `while`
exactly.

**Change.** `cfg.rs::literal_range_is_empty` answers "may be empty" for a range with an end whose start
or end is not constant-shaped (`is_constant_expr`: a parameter's read, a local, a method call); a
constant-shaped bound the oracle does not know (a const generic) is taken as non-empty, as before.
`Cfg::counted_for` marks a counted `for`'s sub-CFG, and R3 names a zero-time cycle there with
`ZERO_TIME_FOR_ITERATION_MESSAGE` ("this `for`'s only clock boundary in an iteration is a nested loop
that can be left before it ticks … (SPEC I.8.2(a))") instead of `NESTED_EXIT_MESSAGE`.
`check_counted_for_nesting` (§J.15) moves ahead of R3 in both front-ends.

**Measured** (baseline = `e265c0b`):

| probe / ui case | baseline macro | baseline transpiler | now (both) |
|---|---|---|---|
| `runtime_empty_for_boundary` (module loop's only boundary) | admits | flattener's message | R3, `NESTED_EXIT_MESSAGE` |
| `runtime_empty_for_in_loop` (a nested `loop`'s only boundary) | admits | arms-only-loop message | R3, `NESTED_EXIT_MESSAGE` |
| `runtime_empty_for_in_bounded_for` (not last in a `for 0..2`) | refuses, nested-loop shape | same | R3, `ZERO_TIME_FOR_ITERATION_MESSAGE` |
| `nested_runtime_empty_for` (last in a `for 0..2`) | `check_counted_for_nesting` | same | unchanged |
| `while_in_for` (probe, a `while` in a `for 0..2`) | R3, `NESTED_EXIT_MESSAGE` | same | R3, `ZERO_TIME_FOR_ITERATION_MESSAGE` |
| `runtime_empty_for_with_tick_ok` (a module tick after the `for`) | admits | transpiles | unchanged |

The first version (predicate only, old order) moved `nested_runtime_empty_for` to R3's message — §V.3's
measurement, reproduced; the reorder restored it. No other ui/fail `.stderr` moved (trybuild,
`TRYBUILD=overwrite` wrote only the three new cases). `front_end_parity_ui.rs` requires the transpiler's
message to equal the macro's on the three new ui/fail cases. Corpus 0 (the shared-rule audit and the
sweep; I.8.6's "no corpus module has a run-time `for` bound").

### X.3 Found — a register written in a tick-free `for` is dropped (ADMITTED-AND-DIVERGENT)

While building `cc_body_for`: `let mut f: Bits<8> = …; loop { q.write(f); for _ in 0..4 { f = f + 1 };
f = f ^ a.read(); tick }` emits `f <= (f ^ a)` — the loop's assignments are gone — on both routes, with a
literal bound, a file const or a method-call const, on `Bits<8>` and `u8`; on `e265c0b` too (measured,
code reverted). A wire accumulator in the same position transpiles (`g = (g + 8'd1)` in `always_comb`),
which is why the corpus — every tick-free `for` in it accumulates into a wire — never shows it. Swept
once as a temporary fixture (`zz_reg_in_for`, deleted after): **6 of 6 sequences DISAGREE,
`sim=ref≠SV`**, SV off by the loop's +4 from cycle 1. Both front-ends admit it, so it is not a parity
break: line `II.3 found register-in-tickfree-for`, OPEN, owner copper-codegen, the one OPEN line that
emits wrong text. Not fixed here.

### X.4 II.1.3 re-verified — six shapes found

A 23-probe battery of ui/pass cases (`parity/zzp_*.rs`, deleted from the tree after the run) through
trybuild and `front_end_parity_ui.rs::the_transpiler_admits_every_shape_the_macro_admits`; each
refusal then narrowed by CLI variants (`variants/`), each variant also compiled under the macro
(trybuild: all admitted). **Predicted:** `zzp_01` (body-local const as a ticking bound, §V.3) and maybe
range patterns and tuple `let` REFUSED-one, the rest admitted by both. **Measured:** 16 admitted by both
(of them `zzp_25`, a register written in a tick-free `for`, transpiles to wrong text — X.3; the others: tuple `let`, shadowing, block/if/match values, signed casts, `u64`, `bool` operators, a module-loop
`continue`, a helper `fn`, a const generic in the body, a `while` wait, a `static` and a `const fn`
const as ticking bounds — the last two closed by X.1); **7 REFUSED-one**, in six classes, each I.2.4
IN or DESUGARED, corpus 0 (no corpus module is refused with these messages), never wrong text:

| line | shape | transpiler's refusal |
|---|---|---|
| `II.3 found body-const` | a body-local `const` in a body expression / tick-free bound / ticking bound | "undefined variable"; R5's "not a constant"; the flattener's |
| `II.3 found compound-assign` | `v += e`, `v ^= e`, `v <<= e` | "binary operator '+=' not supported in hardware" |
| `II.3 found field-assign` | `s.x = e` on a struct local (whole-struct assignment transpiles) | "complex assignment targets not supported" |
| `II.3 found array-literal` | `[c, …]` local (the `[c; n]` form transpiles) | "array literal is not supported in hardware" |
| `II.3 found range-pattern` | `0..=3 =>`, `0..4 =>` | "unsupported pattern" |
| `II.3 found assoc-const-body` | `T::K` in a body expression or `for` bound | "cannot lower literal: T :: K" |

II.1.3 stays **OPEN**: three named exceptions plus these six breaks. All seven new lines name
copper-codegen and the fix direction is a lowering (I.2.4 admits each); none waits on a decision.

### X.5 Ledger and counts

Before: **193 lines — 190 CLOSED, 3 OPEN**. After (G-K on `6df6270`): **200 lines — 192 CLOSED, 8
OPEN** — closed `II.3 found const-call-body`, `runtime-empty-boundary`; opened the six of X.4 and X.3's;
II.1.3 OPEN with the new list. I.8.4's line, I.8.4(b)'s realized note, II.2.2's R3 and R11 rows name the
closures.

### X.6 The bare regression — measured

**Bare `tools/regression.sh` on `6df6270` (the final code commit; laptop; `paper` at `e265c0b`):
REGRESSION OK (667 s wall).** 1501 passed, 0 failed, 16 ignored, 158 test binaries (G-C 131 test
files); examples **32/32**; G-D 239 differential cases (3 ignored-with-reason), G-F 239 (3 skipped with
a reason) — the three `cc_body_*` fixtures added; **G-G differential 1873 PASS / 12 OUTSIDE / 0
TRUNCATED / 0 EXCUSED / 0 DISAGREE**, **reference 1885 PASS / 0 DISAGREE**; G-I 182 clocked related, 0
uncovered; G-J 74 const items compared / 25 outside both, 0 DISAGREE; G-E 5 anchored; **G-K `200 lines —
192 CLOSED, 8 OPEN; 170 required by the SPEC; fixtures: 8 coreui, 3 example, 11 gone, 174 sweep, 153
test, 123 ui; 206 run checks`**, the 3 example fixtures PASS (as predicted in X.5). Not run remotely
(nothing pushed). After this run, docs only: III.3.6's counts, this section, CLAUDE.md.

## Y. `II.3 found register-in-tickfree-for` closed, and what the forwarding walk showed on the way (2026-11-08)

Branch `claude/reg-in-tickfree-for`, cut from `claude/open-lines-last-4e2ae7` at `b159abb` (ledger 200
lines — 192 CLOSED, 8 OPEN). Code commits `b149a32` (the fix, the fixture, the ledger lines) and `3e9db32` (one more OPEN line, §Y.6);
`b149a32`'s message says the fixture has 11 modules and 10 fail on `b159abb` — it has 10, and 9 fail. Probes are in the session notes folder
`reg-in-tickfree-for-Y-probes/` (outside the repo), not in the tree.

### Y.0 Predictions (written before reading the lowering)

The `for` survives to CHIR as `CHIRStmt::ForLoop` and the clocked segment's forwarding walk
(`shir_lower::extract_updates_from_stmts`, SPEC I.3.4) handles only straight-line statements, so a
`ForLoop` whose body assigns a register contributes no update and, its body lowering to nothing, is
dropped. **Measured:** as predicted — the walk's catch-all `_ => {}` skipped `ForLoop`, and
`lower_stmt_list` dropped the empty body. Not predicted: the same walk loses a wire accumulator whose
initializer reads a register assigned earlier in the segment (`f = f ^ a; let mut g = f; for … { g = g + 1 }`
emits `q <= (f ^ a)`), and a const-generic bound loses a register's writes the same way. The fixture's
first run predicted 7/7 agreeing; `tfr_const_bound_branch` did not (Y.2).

### Y.1 The fix — a combinational temp per assigned name

A first draft unrolled every tick-free `for` of a clocked body in `chir_lower` (I.2.4's desugaring, the
array-register path of §U.4 generalized). It fixed the probe and was **rejected on measurement**: the
forwarding walk substitutes expressions, so an unrolled body with branches grows exponentially —
`tfc_sum_set_bits` (8 iterations, two `if`s each) emitted a 44 k-token line Verilator refused ("Too many
preprocessor tokens") — and it turned wire accumulators that the SV `for` already lowered correctly into
the shape of Y.2.

What landed keeps the SV `for` (Verilator unrolls it; I.2.4's meaning is the program's) and makes every
name it assigns one it can carry:

- `chir_lower`: in a clocked body, each name a tick-free `for` assigns (`n = e`, `n[i] = e`, at any depth)
  and does not bind gets a fresh combinational temp — `let mut n__forK = n; for … { n__forK … }; n = n__forK;`.
  A struct local takes one wire per field (`p__for0_x = p_x`), the write-back a whole-struct copy. The
  register write is then one straight-line assignment after the loop, which the walk forwards like any
  other. `subst_in_place` gained a `ForLoop` arm (it skipped nested `for` bodies), with the loop variable
  shadowing a key of its name.
- `shir_lower`: an SV `for` body is lowered against a **frozen** copy of the forwarding with the body's own
  assigned and declared names removed — read by name, their current blocking value in the `always_comb`;
  after the loop the walk maps each to itself (`Var(n)`, its final comb value; for a temp, the value the
  loop leaves). A register still assigned in a body (a name the body also binds, which the rewrite leaves
  out) is refused with `FOR_REGISTER_WRITE_MESSAGE` rather than emitted without the loop's writes; no
  probe or corpus module reaches it.

The probe now emits `f__for0 = f; for (…) f__for0 = (f__for0 + 8'd1);` and `f <= (f__for0 ^ a)`; the
const-generic bound (`0..N`) the same with `_ < N`. Fixture `tests/fixtures/tickfree_for_register_dut.rs`,
10 modules, swept on three legs: the ledger's probe (`tfr_reg_accumulate`), a const-generic bound
(`tfr_generic_bound`, `PARAMS` N = 5), a file-const bound with a body `let` and a branch on the loop
variable, nested `for`s, the extraction route (a tick in each arm, a `for` in each), the forwarded wire
accumulator, a struct register, a bit-written wire, and Y.2's two. **On `b159abb` 9 of the 10
`_differential` cases fail, every `_reference_leg` passes (`sim=ref≠SV`); after, all 20 pass.** The
tenth, `tfr_bit_write`, agrees on `b159abb` too: it pins this change's first draft of Y.2, which
recorded `s` in the forwarding and so lost `s[0] = d` before the temp read it.

### Y.2 Found and closed — a wire reassigned from itself (`II.3 found wire-self-reassign`)

`let mut g = a.read(); g = g + 1; g = g + 3;` in a clocked segment emitted `g = (g + 8'd1); g = ((g + 8'd1)
+ 8'd3);` in `always_comb` and `q <= ((g + 8'd1) + 8'd3)` — the earlier increments counted twice, on every
consumer, in the segment and through a promoted wire after the tick (measured on `b159abb`:
`tfr_wire_self_reassign`, `tfr_wire_promoted` DISAGREE). The walk records a `let` wire only when
substitution changes its value; unrecorded, a later `g = g + 1` entered `g → g + 1` with the name `g`
standing for the wire's FINAL comb value. Fix: a declaration is also recorded when a later statement of
the same list (or, under `advance`, of the rest of the enclosing list) reassigns or redeclares the name
— a shadowing `let`, or the next copy of an unrolled body's `let`, is one SV signal. Not when the list also
bit-writes it: the walk does not track an `IndexAssign` (Y.4), and recording such a wire lost the bit
(`shift_register` first, then `tfr_bit_write`). Corpus 0: every corpus wire accumulator reassigned itself
only inside a `for`, which the walk did not see.

### Y.3 sv-baseline

Snapshot of `b159abb`'s transpiler with the fixture present, diff after: **20 of 315 modules differ** —
the fixture's 9 (2 NEW) and **11 corpus modules**: `sipo_block` (example), `shift_register` (example and
fixture), `f5_sipo`, `f5_wire`, `cc_body_for`, `pb_shift_and_phase`, `q1_b_sipo_block`,
`regout_multiphase`, `tfc_sum_set_bits` — each a wire accumulator now renamed to its temp (`s` →
`s__for0`, its hoisted default with it) — and `f5_branch_wire`, whose wire `t` (declared, then assigned in
a branch) is now forwarded (`q <= (… ? 8'd5 : 8'd0)` for `… ? 8'd5 : t`). All 11 agree on three legs in the
sweep and the two examples PASS. The draft of Y.1 had changed 12, including the three `stage_*` memory
fixtures; the temp form leaves those byte-identical.

### Y.4 Found — a forwarded wire's bit write (`II.3 found forwarded-wire-bit-write`, OPEN)

`f = f ^ a; let mut s = f; s[0] = d; q.write(s)` emits `q <= (f ^ a)`: `s` is forwarded (its
initializer reads the assigned `f`) and the walk has no `IndexAssign` arm, so the bit write never reaches
the edge consumer. Same text on `b159abb`. Every in-segment read of `s` is substituted, so `s` is left
unread and Verilator rejects the module (`UNUSEDSIGNAL` under `-Wall`): the failure is loud. A promoted
`s` (read after the tick) agrees. Both front-ends admit it; corpus 0. Fix direction: a whole-value form
of the bit write in the walk, as §U.3 gave a register's — it needs the wire's width there, which
`extract_updates_from_stmts` does not have. Not fixed here.

### Y.5 Found — a struct literal reading its own earlier field (`II.3 found struct-literal-self-read`, OPEN)

`p = Pr { x: p.y + a, y: p.x }` is lowered field by field, so `y` reads the new `x`: `p_y <= (p_y + a)`.
Swept as a temporary fixture (`zz_struct_swap`): **6 of 6 sequences DISAGREE, `sim=ref≠SV`**; same text on
`b159abb`. Rust evaluates every field from the old value. Both front-ends admit it; corpus 0 (no corpus
literal reads a field of the struct it is assigned to after that field is written). Found because a
struct register in a `for` takes the same per-field write-back (`tfr_struct` avoids the cross-field read).
Fix direction: evaluate the literal's fields into temporaries before the per-field assignments. Not fixed
here.

### Y.6 Found — a `RegOut` write in a tick-free `for` (`II.3 found port-write-in-for`, OPEN)

`for _ in 0..3 { q.write(a.read()) }` in a clocked body, with a literal or a const-generic bound, is
refused by the transpiler with the latch rule's message ("would infer a latch: q assigned on some
control paths but not all"), measured on this branch (probes `v_cport`, `v_genport`; the latch rule is untouched by §Y.1); the macro admits
it (`cargo check` of a throwaway crate, clean). REFUSED-one, never wrong text; I.2.4 DESUGARED (the
last iteration's write is the drive); corpus 0. §X.4's battery had no port write inside a `for`.

### Y.7 Ledger and counts

Before: **200 lines — 192 CLOSED, 8 OPEN**. After (G-K on `3e9db32`): **204 lines — 194 CLOSED, 10 OPEN**
— `II.3 found register-in-tickfree-for` CLOSED (8 sweep tokens); `wire-self-reassign` added CLOSED (3);
`forwarded-wire-bit-write`, `struct-literal-self-read` and `port-write-in-for` added OPEN, owner
copper-codegen; I.2.4's line gained `tfr_reg_accumulate`; II.1.3 OPEN with the seventh break named.
Sweep tokens 174 → 186, run checks 206 → 216.

### Y.8 The bare regression — measured

**Bare `tools/regression.sh` on `3e9db32` (the final code commit; laptop; `paper` at `b159abb`):
REGRESSION OK (210 s wall, warm caches).** 1521 passed, 0 failed, 16 ignored, 158 test binaries (G-C 131
test files); examples **32/32**; G-D 249 differential cases (3 ignored-with-reason), G-F 249 (3 skipped
with a reason) — the ten `tfr_*` fixtures added; **G-G differential 1939 PASS / 12 OUTSIDE / 0 TRUNCATED /
0 EXCUSED / 0 DISAGREE**, **reference 1951 PASS / 0 DISAGREE**; G-I 192 clocked related, 0 uncovered; G-J
75 const items compared / 25 outside both, 0 DISAGREE; G-E 5 anchored; **G-K `204 lines — 194 CLOSED, 10
OPEN; 170 required by the SPEC; fixtures: 8 coreui, 3 example, 11 gone, 186 sweep, 153 test, 123 ui; 216
run checks`**, the 3 example fixtures PASS. The same tree without §Y.6's ledger line (`b149a32`) also ran
bare to REGRESSION OK (549 s). Not run remotely (nothing pushed). After this run, docs only: III.3.6's
counts, two wordings of III.3.4, this section, CLAUDE.md.

## Z. The last codegen-owned OPEN lines of III.3 closed; II.1.3 re-verified on a 47-probe battery (2026-11-08)

Branch `claude/open-lines-e-60b2b2`, cut from `paper` at `fac8930` (ledger 204 lines — 194 CLOSED, 10
OPEN). Code commits `9806eff`, `08fcb2f`, `afb3b6b`, `869a6f5`, the ledger `89596a6`, then three test-side
fixes the bare regression found (Z.12) — `da7d655`, `4fb46eb`, `89b3ef4`, the final code commit. Probes are in the
session notes folder `open-lines-e-Z-probes/` (outside the repo): `battery/` (the 47 ui/pass probes),
`zz_found_dut.rs` (the two divergences of Z.9) and the CLI probes. "Before" is measured, never assumed: a
module's emission on `fac8930` comes from a transpiler built at that commit in a scratch worktree, and a
fixture's verdict with the change switched off (Z.1, Z.2) or on `fac8930`'s SV.

### Z.0 Predictions (written before each change)

Every change was predicted to change **0 corpus modules** (`sv-baseline` against `fac8930`). Measured:
the struct-literal temp changed 1 (`tfr_struct`, a field reading its OWN field — the check is "the
right-hand side reads the target", not "reads a field assigned earlier", so it is conservative; the
output is the same program through a temp); the six shapes, the port write, the bit-write walk, the
statement-match chain, the nested `_` and the binders changed 0, as predicted; the battery's `Bits`-const
`localparam`s changed 2 (`cp_statement`, `cp_tuple_expr` — not predicted: a pattern naming a `Bits`
const now emits the `localparam`'s name where it emitted the folded value) and the self-width cast of the
bit-write form changed 2 (`pb_dynamic_bit`, `pb_shift_and_phase` — predicted as "register bit writes",
without naming them). **Final sv-baseline (snapshot of `fac8930`'s transpiler, 316 modules; 350 now):
39 differ — 34 NEW (the five new fixture files) and 5 CHANGED, `tfr_struct`, `cp_statement`,
`cp_tuple_expr`, `pb_dynamic_bit`, `pb_shift_and_phase`, every one a swept fixture that agrees on three
legs.** No example's or `src/` module's SV changed.

### Z.1 `II.3 found struct-literal-self-read` — CLOSED (the miscompile)

A whole-struct assignment whose right-hand side reads the target (`expr_reads_ident`: a path, a field
base, a call argument, a nested block, a macro's tokens) is bound to a fresh struct temp first —
`let p__litK = <rhs>; p = p__litK;` — through the ordinary struct `let` (literal, conditional, ctor) and
the hazard-free copy from a struct local. Fixture `tests/fixtures/struct_literal_self_read_dut.rs`, 5
modules: the probe (`ssr_swap`), under a condition, through a `const fn` ctor, a struct wire swapped
twice, inside a tick-free `for` on the extraction route. **With the temp switched off: `ssr_swap` 6/6,
`ssr_conditional` 8/8, `ssr_for_and_phases` 8/8 sequences DISAGREE `sim=ref≠SV`; `ssr_ctor` (`p_x`
unread) and `ssr_wire` (`a` unread) lint-rejected (UNUSEDSIGNAL) — the wrong value left an input or a
field unread. After: 10 of 10 cases pass.**

### Z.2 `II.3 found forwarded-wire-bit-write` — CLOSED

The forwarding walk (`shir_lower::extract_updates_from_stmts`) gained an `IndexAssign` arm: a bit write to
a RECORDED wire updates the recorded value with `(v & ~(W'(1) << i)) | (W'(e) << i)`, cast to `W` (Z.8),
the width read from the wire's `Wire` declaration (a thread-local the walk fills; the walk's signature
has no symbol table). `vlir_lower::drop_unread_wires` no longer counts a bit write as a read of its own
base, so a wire every edge consumer reads through its forwarded value is dropped with its bit writes
(a port base is never dropped). Fixture `forwarded_wire_bit_write_dut.rs`, 4 modules: **with the arm off,
`fwb_probe`, `fwb_branch_dyn`, `fwb_phases` fail (Verilator lint, the wire unread); `fwb_promoted`
agrees; after, 8 of 8 cases pass.** Corpus 0.

### Z.3 `body-const`, `assoc-const-body`, `compound-assign` — CLOSED (a FIR desugaring)

`copper-codegen/src/desugar.rs`, run first in `lower_fir_to_chir`: a body-local `const` (any depth) and an
associated const `T::K` of an inherent `impl` (in an expression, a bound or — Z.8 — a pattern) are
hoisted into `file_consts` and `file_const_source` (`T::K` as `T__K`, by its `ConstEnv` value at its
declared type); `v op= e` becomes `v = v op e`. R5's transpiler fold (`lib.rs`) reads the body's consts. The
interpreter (`copper-ref`) collects a nested block's consts. Fixture `parity_shapes_dut.rs`
(`ps_body_const[_comb]`, `ps_assoc_const[_comb]`, `ps_compound[_comb]`). On `fac8930` all six refused with
§X.4's messages; after, all agree.

### Z.4 `II.3 found array-literal` — CLOSED

`[e0, …]` lowers to a `Concat` listing the elements last first (element 0 in the low bits, the layout of
`[c; n]` and `Bits<N>`). **Found on the way: an array value's bare literal elements were never sized** —
`retype_default_literals_in_values` did not descend into a `Concat`, so `let t: [u8; 4] = [7; 4]` emitted
`t = {64'd7, 64'd7, 64'd7, 64'd7}` on `fac8930` (lint-rejected). It now types each element at the element
width. `ps_array_literal[_comb]` agree.

### Z.5 `II.3 found range-pattern` — CLOSED; `stmt-match-guard` found and CLOSED

A range pattern becomes a guarded arm: `lo..=hi => b` → `_ if s >= lo && s <= hi => b` (or-lists, `n @`,
open ends, and a range element of a tuple pattern over a tuple scrutinee, the element replaced by `_`;
with no binder left the arm is a guarded `_`). `u >= 0` on an unsigned operand folds to true (Verilator's
UNSIGNED), `true && e` to `e`. **Found: a statement `match` with a guarded arm was refused by
`vlir_lower` ("tuple-pattern match lowering is not yet implemented"), with or without a range** — the
§X.4 battery had no guarded statement `match`. Such a `match` (and, Z.8, one with a tuple pattern holding a
wildcard or binder element) now lowers to the `if` chain arm order means
(`chir_lower::match_arms_as_if_chain`). **Also found: a tuple-pattern literal element was compared at 64
bits** (`k == 64'd1`, lint-rejected on `fac8930`); it takes its scrutinee element's width. The interpreter
gained `n @ p` patterns. `ps_range_pattern[_comb]`, `ps_stmt_match_guard` agree.

### Z.6 `II.3 found field-assign` — CLOSED

`s.x = e` (and `s.x[i] = e`) on a struct local assigns the field's net `s_x` (`struct_field_place`).
**Found: a struct literal binding's bare literal field was never sized** (`S { x: 1, y: 2 }` with `u8`
fields emitted `s_x = 64'd1`, lint-rejected on `fac8930`); it takes the field's width. **And in the
interpreter**, an unsuffixed integer field stayed unsuffixed, so `s.y.wrapping_add(1)` became `usize` and
then "mixed integer types u8 and usize"; it now takes the declared field type. `ps_field_assign[_comb]`
agree.

### Z.7 `II.3 found port-write-in-for` — CLOSED (no packet)

I.2.4 settles it: a tick-free `for` is unrolled and the resumption's last `write` is the drive (I.6), so a
write in an iteration that does not run leaves the port as it was. Lowered with §Y.1's temp and a flag:
`q__forK = 0; q__forK_w = 0; for … { q__forK = e; q__forK_w = 1; } if (q__forK_w) q <= q__forK`. Fixture
`port_write_in_for_dut.rs`, 5 modules (literal and const-generic bound — `PARAMS` N = 3 —, a conditional
write, a write before the loop and a second port, nested `for`s on the extraction route): **all 5 refused
on `fac8930` (the latch rule), all agree after.** **Found: two nested `for _` declared the SV loop variable
`_` twice (VARHIDDEN)**; a nested `_` takes a name of its depth (`__for_i1`).

### Z.8 II.1.3 re-verified — 47 probes; seven more breaks found and closed

Battery: §X.4's 23 ui/pass probes (rerun) and 24 new ones (`zzq_*`: the shapes above in more positions, and
their neighbours — a `Bits` const, a body `enum` and `struct`, `matches!`, `else if`, a tuple range with a
binder, an inclusive `for`, an element and a field compound assignment, a struct field's bit, an `impl`
method, an associated const pattern, a signed range, an array-literal register…); a 25th (`<<=` on `Bits`)
is refused by rustc (no `ShlAssign` on `Bits`) and dropped. Run through trybuild (the macro: **all 47
admitted**) and `front_end_parity_ui.rs::the_transpiler_admits_every_shape_the_macro_admits`.
**Predicted:** the six of §X.4 and the port write now admitted, nothing else refused. **Measured on the
first run:** 8 refused by the transpiler — not predicted — each I.2.4 IN or DESUGARED except the last:

| line | shape | `fac8930`'s transpiler | now |
|---|---|---|---|
| `stmt-match-binder` | `x if x > 9 => …`, `(0, y) => …` in a statement `match` | "undefined variable" / the tuple-pattern refusal | binder names the scrutinee (element) |
| `bits-const` | `const K: Bits<8> = …`, file-scope or body | "not an integer constant" | sized `localparam` of the folded value |
| `body-struct` | `struct P { … }` in the body | "cannot infer bit width" | joins the file structs |
| `impl-method` | `s.twice()`, `fn twice(self)` | "method `twice` is not supported" | inlined, `self` = the receiver |
| `assoc-const-pattern` | `T::K => …` | "unsupported pattern" | hoisted `T__K` |
| `array-literal-register` | `let mut r: [u8; 4] = [1, 2, 3, 4]` | R11's "not a constant" | register file, one stated word each |
| (`stmt-match-guard`, Z.5) | | | |
| `inclusive-for` | `for i in 0..=3`, tick-free or ticking | "only exclusive range" | **unchanged — packet Z.10** |

Sweeping the newly lowered shapes as a fixture (`parity_battery_dut.rs`, 6 modules) found four more
pre-existing wrong-text shapes, each lint-rejected and now fixed and CLOSED: `logic-array-index` (the
width-1 rule took a `[Logic; N]` local for a scalar and dropped a run-time index — `bit = l` — the repeat
form too; and an index wider than the select needs, narrower than 32, is cast to 32),
`negated-literal-width` (`v < -1` compared at 64 bits), `bitwrite-in-wider-context` (the whole-value
bit-write form, inside an `as_u8` cast, was sized to the cast; it is now cast to its own width — the 2
`packed_bit_write` fixtures), and an inclusive tick-free `for`, whose desugaring (to `a..(b + 1)`) I wrote
and then **reverted**: the interpreter refuses it citing I.2.4 ("exclusive ranges only"), so its meaning
is a decision (Z.10). Final battery run: **46 admitted by both, 1 shape REFUSED-one (the inclusive `for`,
2 probes)**; II.1.3 stays OPEN on it.

### Z.9 Found — two divergences both front-ends admit (OPEN, not fixed)

Each swept once as a temporary module of `parity_battery_dut.rs` (deleted after; in the notes folder),
each with byte-identical SV on `fac8930`, corpus 0:

- `II.3 found array-register-trailing-write`: `q.write(r[a]); tick; r[a] = d;` — the trailing write and
  the next head's read are one resumption (I.0.2), but the head's write-through mux sees only the
  previous resumption's staged write (`r_wr0_s1_*`). **8/8 sequences DISAGREE `sim=ref≠SV`.** The corpus's
  register files write before their tick. Not caused by this branch: the same with `[0u8; 4]`.
- `II.3 found signed-local`: `let v = x as i8; v < 3` — `v` is declared unsigned and read without
  `$signed`, so the compare is unsigned. **6/6 sequences DISAGREE `sim=ref≠SV`.** The corpus casts at the
  use.

### Z.10 Decision packet — `II.3 found inclusive-for`

**Question.** Is a `for` over an inclusive range (`for i in a..=b`), tick-free or tick-bearing, IN (with
Rust's meaning) or OUTSIDE (refused by both front-ends)?

**What the docs say.** I.2.4's table: the tick-bearing `for v in a..b` row says "exclusive ranges only";
the tick-free row names `a..b` with constant bounds, "unrolled `b − a` times". Part I's carried list:
"The inclusive counted `for` (`a..=b`): I.2.4 admits exclusive ranges only and the interpreter refuses
the inclusive one; corpus 0 → Phase C, with II.1's parity list." No decision states a meaning or a
refusal.

**What the code does.** The macro admits both (trybuild `zzq_16`, `zzq_25`); the simulator runs Rust's
meaning; the interpreter refuses ("an inclusive range in a `for` (I.2.4: exclusive ranges only)"); the
transpiler refuses ("only exclusive range `start..end` for-loops are supported"; for a ticking one the
counted-loop message). Corpus 0 (no corpus module uses `..=` in a `for` header).

**Options.**
- **(a) IN.** `D` gains `for v in a..=b` ≡ Rust's. Tick-free: unroll `b − a + 1` times (an SV
  `for (… ; i <= b; …)` needs no `b + 1`, so a bound at its type's maximum does not overflow).
  Tick-bearing: the counter's exit test `c > e` instead of `c >= e`; at `e` = the type's maximum the
  counter's increment overflows after the last iteration, so either the counter is one bit wider or that
  bound is refused (R3-style). Changes: I.2.4's two rows, the interpreter's `for`, the transpiler
  (`desugar.rs`, ~20 lines, written and reverted in Z.8; the counted-loop flattener for the ticking
  case), a ui/pass case and a swept fixture.
- **(b) OUTSIDE.** Both front-ends refuse with one message (a shared `front_end` rule, II.1's list);
  I.2.4's rows say so. Changes: one shared rule, a ui/fail case, `front_end_parity_ui.rs`; no lowering.

**Recommendation: (a), tick-free and tick-bearing**, refusing (as R3 would) only a ticking bound at its
type's maximum: the Rust meaning is unambiguous, I.2.4's DESUGARED class is exactly this kind of form, and
an author writing `0..=N-1` has no reason to expect a refusal. (b) is the smaller change if the user
prefers to keep `D` minimal.

### Z.11 Ledger and counts

Before: **204 lines — 194 CLOSED, 10 OPEN**. After (G-K, `cargo test --test conformance_ledger` on
`89596a6`): **219 lines — 215 CLOSED, 4 OPEN** — closed 9 of the 10 (all but II.1.3); added 12 found and
closed (`stmt-match-guard`, `stmt-match-binder`, `bits-const`, `body-struct`, `impl-method`,
`assoc-const-pattern`, `array-literal-register`, `logic-array-index`, `negated-literal-width`,
`bitwrite-in-wider-context`, `nested-for-underscore`, `unsized-aggregate-literal`) and 3 OPEN
(`inclusive-for` — owner decision, Z.10; `array-register-trailing-write`, `signed-local` — copper-codegen,
Z.9). II.1.3 OPEN on one shape. Sweep tokens 186 → 234, run checks 216 → 249.

### Z.12 The bare regression — measured

The first bare run (on `da7d655`'s parent, `89596a6`) failed in G-J: the new fixtures' `Bits`/`Logic`
consts are evaluated by the interpreter only (ConstEnv is integer-valued, the class of the five pinned
`const_pattern_dut` rows) and `PS_TRIPS`, a const declared in a loop body, was read by neither side of the
test's analysis half (which took top-level body items only). `da7d655`: the test reads nested body consts,
as both evaluators now do, and pins the three. The second (on `da7d655`) failed on a full disk ("No space
left on device"; the other worktrees' `target/` directories were deleted with the user's approval, 62 GiB
freed). The third (on `da7d655`) failed in G-I: the projection test's source walker could not fold a
ticking `for` bound naming a nested body const or `T::K`, explored the empty-range path and found no tick
(`ps_body_const`, `ps_assoc_const`), and `ssr_ctor`'s struct temp carried the constructor body's spans,
which map to no source statement. `4fb46eb`: the walker reads the module's body consts at any depth and a
multi-segment path by its full name; the temp takes the statement's span (SV unchanged). The fourth (on
`4fb46eb`) failed in `register_reconciliation`: `copper_analysis::reference_sv_registers` took the guard
comparison `(v <= 8'd99)` in `ps_range_pattern`'s `always_ff` for a nonblocking assignment, so `v` and `k`
looked like surplus flops. `89b3ef4`: an `ident <=` after an open parenthesis is no target.

**The fifth: bare `tools/regression.sh` on `89b3ef4` (the final code commit; laptop; `paper` at `fac8930`): REGRESSION
OK (224 s wall, warm caches).** 1589 passed, 0 failed, 16 ignored, 158 test binaries (G-C 131 test files);
examples **32/32**; G-D 283 differential cases (3 ignored-with-reason), G-F 283 (3 skipped with a reason) —
the 34 new fixture modules added; **G-G differential 2139 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0
DISAGREE**, **reference 2151 PASS / 0 DISAGREE**; G-I 216 clocked related, 0 uncovered; G-J 82 const items
compared / 25 outside both, 8 asymmetric (pinned), 0 DISAGREE; G-E 5 anchored; **G-K `219 lines — 215
CLOSED, 4 OPEN; 170 required by the SPEC; fixtures: 8 coreui, 3 example, 11 gone, 234 sweep, 153 test, 123
ui; 249 run checks`**, the 3 example fixtures PASS. Not run remotely (nothing pushed). After this run, docs
only: III.3.6's counts, this section, CLAUDE.md.

## AA. The last four OPEN lines of III.3: two miscompiles fixed, the inclusive `for` (D-27) lowered, II.1.3 closed (2026-11-09)

Branch `claude/open-lines-f-2e105a`, cut from `paper` at `f790444` (ledger 219 lines — 215 CLOSED, 4 OPEN).
Code commits `57450d5` (the two miscompiles), `d382025` (D-27), the ledger and decision `2cc6022`, and
`f09dfb6` (a test pin AA.7's first run found), the final code commit — the bare regression of AA.7 ran on
it; after it, docs only. "Before" is measured: a
fixture's verdict with the change switched off (a temporary environment switch, removed before the
commit), and every module's emission against `sv-baseline snapshot` taken on `f790444`. Probes and the
predictions file are in the session's scratch; the probes of §Z are in the notes folder
`open-lines-e-Z-probes/`.

### AA.0 Predictions (written before each change)

| change | predicted corpus SV changes | measured |
|---|---|---|
| AA.1 trailing array write | 0 (only a body whose trailing region writes an array register; §Z.9 says corpus 0) | **0** |
| AA.2 signed local | "the signedness fixtures and possibly the RV32I CPUs" (a guess, named before measuring) | **1**, `c_signed` (`data_i14_dut.rs`): `q <= s` → `q <= $unsigned(s)`, the same bits at the same width — **not** the modules guessed; the CPUs cast at the use and changed nothing |
| AA.3 D-27 | 0 (no corpus `for` header is inclusive) | **0** |
| AA.4 battery | 47/47 admitted by both | **47/47** |

Final sv-baseline against `f790444`: **21 of 370 modules differ — 20 NEW (the three new fixture files) and
1 CHANGED, `c_signed`, which sweep-agrees on three legs.** No example's or `src/` module's SV changed.

### AA.1 `II.3 found array-register-trailing-write` — CLOSED (the miscompile)

The trailing region (after the last tick) is the head of the next resumption (I.0.2) and `shir_lower`
lowers it into phase 0 ahead of segment 0. `chir_lower` lowers in source order, head first, so a head read
of an array register saw only the committed array and — through `forward_*_mem_index` — the previous
resumption's staged write (`r_wr0_s1_*`), never the trailing write of its own resumption. **Fix**
(`chir_lower::lower_seq_body`): a body whose trailing region writes an array register is lowered twice;
the second pass seeds the head's write-through count (`array_written`) with the trailing region's port
count `t`, so a head read muxes over ports `0..t` (the trailing writes' comb staging nets) inside the
`s1` forwarding, and the head's own writes take ports from `t`. A pre-loop element write shares the low
ports (`!__copper_started` vs `__copper_started`: never one resumption). The emitted read of the probe:
`q <= ((r_wr0_en && (a == r_wr0_addr)) ? r_wr0_data : ((r_wr0_s1_v && …) ? r_wr0_s1_data : r[a]))`.

Fixture `tests/fixtures/array_trailing_write_dut.rs`, 6 modules: the probe; a head write and a trailing
write to one array (the head's at port 1); two trailing writes, one conditional; two phases; a pre-loop
write and a trailing write; a branch-nested tick (the extraction route). **With the seed off: 4 of 6
`_differential` cases fail (`atw_probe`, `atw_head_and_trailing`, `atw_two_trailing`, `atw_two_phases`);
`atw_preloop_and_trailing` passes — its pre-loop write already took port 0, and the old code never
cleared that count before the head, so the head muxed over port 0 by accident; `atw_extraction` passes
— on the extraction route the post-tick write and the head are arms of one `match pc`, a path this
change does not touch. With it: 12/12 cases pass.** `phase_sensitive_checks.rs`: the tick refusal moved
to `lower_seq_body_seeded` (its entry renamed; `lower_seq_body` now carries a `debug_assert_eq!`, which the
scan skips).

### AA.2 `II.3 found signed-local` — CLOSED; `signed-widening-cast` found and CLOSED

`let v = a.as_u8() as i8` declares `v` as a plain `logic [7:0]` and a read of it was a plain `Var`, so
`signed_binop` saw no `SignCast` and `v < 3` compared unsigned. **Fix** (`chir_lower::signed_read`): an
operand of a binary or unary operator, or of a cast, that is a `Var` whose symbol type is `SInt` (a
signed local, register or const) is wrapped in `SignCast { signed: true }`, exactly as `as i8` at the use
is. **Found with it:** a widening cast of a signed value. `let x = v as i16` emitted `x = $signed(v)` for
a 16-bit `x` (Verilator's WIDTHEXPAND — the fixture's first run failed there) and `v as u16` emitted
`$unsigned(v)` in a 16-bit context, zero-extending where Rust sign-extends. Now `$signed(16'($signed(v)))`
and `$unsigned(16'($signed(v)))`; a plain operand widened `as i*` is zero-extended first
(`widening_resize`). Fixture `tests/fixtures/signed_local_dut.rs`, 6 modules (the probe; compares between
locals, against a negative literal, an annotated local; `>>`, `/`, `%`, a wrapping sum and difference;
widening casts; a signed register; a combinational body). **With the change off: 6 of 6 `_differential`
cases fail; with it 12/12 pass.** (`wrapping_neg` is not a lowered method — the fixture uses
`wrapping_sub`; unchanged, row (e)'s list.)

### AA.3 `II.3 found inclusive-for` — CLOSED (D-27); `loop-var-type` found, OPEN

D-27 (decided by the user 2026-11-08, packet §Z.10 option (a); recorded in `SPEC_DECISIONS.md`):
- **Tick-free** — `CHIRStmt`/`SHIRStmt`/`VLIRStmt::ForLoop` carry `inclusive`; the emitter writes
  `for (int i = a; i <= b; i++)`, no `b + 1`; the array-register unroll runs `b − a + 1` copies. The
  interpreter does not increment past `e` (a frame flag), so `250u8..=255u8` runs six iterations.
- **Tick-bearing** — `control_extract::expand_counted_for` takes `a..=b`, its two exit tests and the
  hoisted tick's guard `c > e`, the statically-empty test `a > b`. The interpreter runs I.8.5's form with
  `c > e`, its increment checked.
- **II.1.2 R23** — `copper_analysis::constant_ticking_inclusive_ranges` (constant-shaped bounds, as R3's,
  and *typed*: a path, a suffixed literal or a cast among the leaves) and `INCLUSIVE_MAX_MESSAGE`. The
  macro places `const _: () = ::core::assert!({ let r = (a)..=(b); (*r.end()).checked_add(1).is_some() },
  MSG);` right before the `for` (rustc types `r` as the `for` does; E0080 under `cargo check`, so trybuild
  sees it). The transpiler folds the end at `ty_of(end).or(ty_of(start))` through `ConstEnv`; an end or
  type it cannot fold is refused (`UNFOLDED_INCLUSIVE_END_MESSAGE`). Measured on the CLI: `0u8..=255u8`,
  `0..=255u8`, `0..=TOPI` (`i16` 32767), `-3i8..=127` refused; `0..=(TOP - 1)` and the untyped `0..=255`
  admitted.
- ui/pass `inclusive_for` (both forms, an end at `u8::MAX` tick-free, a typed const end 254 ticking),
  ui/fail `inclusive_ticking_for_at_max` (`250..=TOP`, `TOP: u8 = 255`); `front_end_parity_ui.rs`
  passes (the transpiler's message equals rustc's panic text).
- Fixture `tests/fixtures/inclusive_for_dut.rs`, 8 modules (the battery's two probes; a const end and a
  nested pair; the end at `u8::MAX`; an array-register unroll; a const end and a typed end below its
  maximum, ticking; a run-time end, empty when `n < 2`; an author's `break`). **On `f790444` the
  transpiler refused all 8; now 16/16 cases pass.** The tests that walk `for` headers learned the
  form: `projection_relation.rs` (G-I) reads `c > e` as "the counter passed its range" and an inclusive
  range as empty when `a > b`; `copper_analysis::reference_sv_registers` no longer takes `i <= 3` in a
  `for (` header for a nonblocking target (`register_reconciliation` saw `i`, `j`, `k` as surplus flops).

**Found — `II.3 found loop-var-type` (OPEN, pre-existing).** The fixture's first run failed on two
modules; both reproduce with **exclusive** ranges (a temporary fixture, deleted): a loop variable whose
Rust type is narrower than the transpiler's 32-bit counter / the interpreter's `usize`, read as an
operand without a cast — `zz_excl_typed_var` (`for i in 250u8..255u8 { s.wrapping_add(i ^ a) }`):
Verilator's WIDTHEXPAND on the transpiled SV; `zz_excl_break` (`if a.as_u8() == i` with `i` inferred
`u8`): the interpreter stops, "mixed integer types u8 and usize", and the SV lint-fails too. Corpus 0 (the
corpus casts at the use). The fixture casts (`i as u8`).

### AA.4 II.1.3 — CLOSED

§Z.8's 47 probes (notes folder `open-lines-e-Z-probes/battery`), copied as `ui/pass/zzbat_*` and removed
after: **trybuild admits all 47; `front_end_parity_ui.rs::the_transpiler_admits_every_shape_the_macro_admits`
passes with them — 47/47 admitted by both.** `front_end_parity.rs`'s three-named-exceptions test passes.
The line closes: no shape outside R6, the bound instance and the file-level X-const pattern is known to
one front-end only. The two lines this section found are not parity breaks (both front-ends admit).

### AA.5 Decision packet — `II.3 found inclusive-for-runtime-max`

**Question.** A tick-bearing `for v in a..=b` whose end equals its type's maximum **at run time** (`for i
in 0..=n` with `n: u8` read from an input and 255), or whose constant end is untyped and the body infers a
type it is the maximum of (`for i in 0..=255 { …from_u8(i)… }`): what is its meaning?

**What the docs say.** D-27: "IN, with Rust's meaning"; "a ticking bound at its type's maximum is refused in
both front-ends, R3-style, because the counter would overflow". I.8.5's `D` (as written for D-27):
`… c = c + 1 ; s` with exit `c > e` — at `e = MAX` the increment overflows, which I.14.3 makes a run-time
failure. The two sentences agree for a constant typed end (refused) and disagree at run time: Rust's
`RangeInclusive` runs the last iteration and stops; `D` fails.

**What the code does.** The simulator runs Rust (no failure). The interpreter realizes `D`: its checked
increment fails. **Measured** (a temporary fixture, deleted: `for i in 0xfeu8..=n` with `n = a | 0xfc`
from an 8-bit input): **6/6 sequences DISAGREE, pattern `sim≠ref`** on both the `_reference_leg` and the
`_differential` case — "the interpreter failed at resumption 3 (integer overflow: 256 is outside u8) and
the simulator did not" (III.2.4 requires both to fail at the same resumption). The transpiler's counter
is `u32` (`COUNTER_TY`): for an 8- or 16-bit type it does not wrap; by its construction, for a 32-bit or
wider type at its maximum it wraps to 0 and the loop does not exit (not measured: no stimulus reaches a
32-bit maximum). R23 cannot see a run-time end; for an untyped constant end neither front-end names the
type (the interpreter's unsuffixed integer is unbounded, so it agrees with Rust there). Corpus 0; no
sweep stimulus reaches it in the fixture (its run-time end is a 2-bit input).

**Options.**
- **(a) Rust's meaning everywhere.** `D` tests before it increments past the end: `loop { if c > e {
  break } ; let v = c ; s' }` with the step `if c == e { break-after-s } else { c = c + 1 }` (or a done
  flag). Changes: I.8.5's form, the interpreter (the tick-free code path already does this), the
  transpiler's counter (a done bit, or a counter one bit wider than the range's type), R23 retired or kept
  as a realization limit only. Nothing refuses; the simulator is already right.
- **(b) `D` as written: a run-time failure.** The macro realizes the failure in the simulator (a rewritten
  header that panics when the end is the maximum, or a checked counter), so the three legs end at the same
  resumption; the transpiler emits nothing for it (outside the trace). Changes: the macro's `for`
  rewriting (the simulator would panic where plain Rust does not), the transpiler's `u32` counter for
  wide types still wraps — unobservable only because the trace ends.
- **(c) Refuse what cannot be decided.** Both front-ends refuse a ticking inclusive `for` whose end is not
  a constant typed range of R23 (a run-time end, an untyped literal). Changes: one shared rule; it
  over-refuses every run-time end below the maximum, including the fixture's `2..=n`.

**Recommendation: (a)** — it is what D-27's headline ("Rust's meaning") says, the simulator already
does it, the interpreter's tick-free path already does it, and it removes the only place where `D`
fails where Rust does not; R23 would then remain only if the transpiler keeps a range-typed counter.
(b) preserves the letter of the refusal's reason but makes the simulator diverge from plain Rust; (c) is
the smallest change and the largest over-refusal.

### AA.6 Ledger and counts

Before: **219 lines — 215 CLOSED, 4 OPEN**. After (`cargo test --test conformance_ledger` on `2cc6022`):
**223 lines — 221 CLOSED, 2 OPEN; 171 required by the SPEC; fixtures: 8 coreui, 3 example, 11 gone, 256
sweep, 156 test, 128 ui; 269 run checks** (and the same from G-K in AA.7's run). Closed: all four (`II.3 found array-register-trailing-write`,
`signed-local`, `inclusive-for`, II.1.3). Added: `II.1.2 R23` (a new rule row, CLOSED — required, so 170 →
171), `II.3 found signed-widening-cast` (found and CLOSED), `II.3 found loop-var-type` (OPEN, copper-codegen
and copper-ref) and `II.3 found inclusive-for-runtime-max` (OPEN, a **decision**, AA.5).

### AA.7 The bare regression — measured

The first bare run (on `2cc6022`) failed in `tests/timing_model_derivations.rs`: the set of modules the
extraction route hides gained `ifr_ticking_break` and `ifr_ticking_runtime` — the inclusive twins of
`det_for` and `for_empty`, whose guarded hoisted tick (an author's `break`; a run-time end) takes the
extraction route as theirs does. `f09dfb6` adds them to the expected set with that reason.

**The second: bare `tools/regression.sh` on `f09dfb6` (the final code commit; laptop; `paper` at `f790444`):
REGRESSION OK (253 s).** 1629 passed, 0 failed, 16 ignored, 158 test binaries (G-C 131 test files);
examples **32/32**; G-D 303 differential cases (3 ignored-with-reason), G-F 303 (3 skipped with a reason) —
the 20 new fixture modules added; **G-G differential 2285 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0
DISAGREE**, **reference 2297 PASS / 0 DISAGREE**; G-I 234 clocked related (55 extracted, 19 linear, 160
single-tick), 0 uncovered; G-J 84 const items compared / 25 outside both, 180 counted-`for` bounds compared,
8 asymmetric (pinned), 0 DISAGREE; G-E 5 anchored; **G-K `223 lines — 221 CLOSED, 2 OPEN; 171 required by
the SPEC; fixtures: 8 coreui, 3 example, 11 gone, 256 sweep, 156 test, 128 ui; 269 run checks`**, the 3
example fixtures PASS. Not run remotely (nothing pushed). After this run, docs only: III.3.6's counts,
this section, CLAUDE.md; the AA.5 probe was swept as a temporary fixture after the run and deleted.

## AB. The last two OPEN lines of III.3 closed: D-28 (the inclusive `for` at its maximum) and the loop variable's type; three more found and closed (2026-11-09)

Branch `claude/open-lines-g-e2f9fd`, cut from `paper` at `5aaa6ca` (ledger 223 lines — 221 CLOSED, 2 OPEN).
Code commits `23931ec` (the code, the fixtures and the ledger lines), `2da5fe8` and `170bc8c` (two pins
the first two regression runs found, AB.8) and `4700ca4` (the shadow renamed instead of the loop variable,
AB.5), the final code commit — the bare regression of AB.8 ran on it; after it, docs only. "Before" is measured: every new fixture swept on a
scratch worktree of `5aaa6ca` (the fixture files copied in), and every module's emission against an
`sv-baseline snapshot` taken on `5aaa6ca` before any change. Probes are in the notes folder
`open-lines-g-AB-probes/`.

### AB.0 Predictions (written before any code change)

| change | predicted | measured |
|---|---|---|
| before, `loop_var_type_dut` (6 modules) | lint failures (WIDTHEXPAND) on the typed/used-`u8` ones, "mixed integer types" on the interpreter, the negative `i8` range wrong on a `u32` counter, a `usize` underflow of `i - 2` in the interpreter | as predicted, except `lvt_typed_ticking` **passed** (a `u32` counter read in `i ^ a` was widened cleanly) and `lvt_signed`'s reference leg passed: **8 of 12 cases failed** |
| before, `inclusive_max_dut` (7) | `sim≠ref` overflow at `u8`/`u16`/`i8`; the `u32` counter wraps; the `u64` truncates; tick-free wide unrolling fails | `sim≠ref` at `u8`, `u16`, `u32`, `u64`, `i8` (the interpreter fails first); `imx_u32_runtime` `sim≠SV` too (the counter wrapped: `q` 0, 1, 2 … where 7 was due); `imx_u64_runtime` refused ("4294967296 does not fit in the 32-bit operand"); **`imx_tickfree_wide` refused** as first written — not predicted: the transpiler could not read `u32::MAX` (AB.4) — and, with the maxima spelled as literals (the fixture as committed), lint-rejected (the `int` loop variable assigned a 64-bit constant); the constant `u8` end R23-refused as written (swept with its end lowered to 254 to compile): **11 of 14 cases failed** |
| D-28's corpus SV | the 4 tick-bearing inclusive `ifr_*` modules | **0** by D-28 itself: a range type below 32 bits keeps its 32-bit counter, so `c > e` is unchanged |
| loop-var typing's corpus SV | the typed-narrow `ifr_*` and "the untyped default `i32` read signed", guessed 10–25 | **6** (AB.2) after two refinements made on measurements, not predicted: a file const bound types `v` at the width the transpiler reads it (first cut: 15 modules, the UART counters widened to 64 bits by a `usize` const), and an index use types `v` `usize`, realized at 32 bits (`i32` for every untyped variable: 11 more modules) |
| battery | 47/47 + the new probes admitted by both | **63/63**, after one break found and closed (AB.4) |

### AB.1 `II.3 found inclusive-for-runtime-max` — CLOSED (D-28)

D-28 (decided by the user 2026-11-09, packet §AA.5 option (a); recorded in `SPEC_DECISIONS.md`): Rust's
meaning everywhere. I.8.5's inclusive `D` tests a done flag and never increments past the end:
`let mut c = a ; let e = b ; let mut d = c > e ; loop { if d { break } ; let v = c ; if c == e { d = true }
else { c = c + 1 } ; s }`.
- **Interpreter** (`copper-ref` `exec_for`): the done flag is a hidden local `__for@<depth>d` declared with
  the counter, so it survives a tick; the tick-free frame flag of D-27 is gone (one path).
- **Transpiler, tick-bearing** (`control_extract::expand_counted_for`): the counter's type holds `e + 1`,
  so the unchanged `c > e` exits sees the step past the end without wrapping — `u32`/`i32` for a range type
  below 32 bits (as before: no corpus change), `u64`/`i64` for 32 bits, `u128`/`i128` for 64; with both
  bounds constant, the narrowest of 32/64/128 bits that holds their values. A 128-bit range type has no
  wider Rust type: there the done flag itself is emitted (`__copper_ctr<n>_done`, the step `if c == e
  { d = true } else { c = c + 1 }`, the hoisted tick guarded by `d` / `a <= e`). A first try at a 129-bit
  internal counter type lost its width downstream (registers declared 64 bits); dropped for the flag.
  Bounds are cast to the counter's type (a constant one written as a literal of it: a cast of `-2i8`
  zero-extended `8'd254`). `const_value` now folds a unary minus.
- **Transpiler, tick-free**: a range SV's `int` cannot carry (`0xffff_fffdu32..=0xffff_ffffu32`) or with a
  negative bound gets `chir::ForCounter` through CHIR/SHIR/VLIR: the emitter declares `logic [w:0]` (or
  `logic signed`, or `int`) and writes the folded bounds.
- **II.1.2 R23 retired** (D-28): realized on every leg, so the refusal had no reason left. Deleted:
  `place_inclusive_max_assertions` (macro), `constant_ticking_inclusive_ranges` and
  `INCLUSIVE_MAX_MESSAGE` (analysis), the fold and `UNFOLDED_INCLUSIVE_END_MESSAGE` (transpiler). ui
  `inclusive_ticking_for_at_max` moved from `fail` to `pass`, admitted by both.
- Fixture `tests/fixtures/inclusive_max_dut.rs`, 7 modules, each run-time end OR-ing a narrow input into
  the top of the range so the sweep's stimulus reaches the maximum (`u8` `a | 0xfc`, `u16` with an author's
  `break`, `u32`, `u64` plus a wide exclusive range, `i8`), a constant `255u8` end and an untyped `254..=255`
  typed `u8` by its use, tick-free `u32`/`u64` maxima: **14/14 cases pass** (before: AB.0). `tests/
  inclusive_max_u128.rs`: a `u128` run-time end reaching `u128::MAX`, the done flag, simulator against
  Verilator over 64 cycles of every input value (the loop reaches 0xff and exits) — not swept: the
  interpreter has no `i128` and stops `u128` at `i128::MAX` (recorded there, not a line).
- `timing_model_derivations.rs`: 8 new modules take the extraction route (run-time ends, `break`s, two
  ticking `for`s in one loop) — added to its expected set with that reason.

### AB.2 `II.3 found loop-var-type` — CLOSED

`v` has the range's type (SPEC I.14.1; I.8.5's paragraph): `chir_lower::for_var_type` reads it as rustc
does — a typed bound's (a suffix, a cast, a typed local or port view, `uN::MAX`; a file const or const
generic at the 32 bits the transpiler reads a `localparam int`/`parameter int` at, an 8/16-bit const at its
own), else the first use that unifies it with a typed operand (a binary operator's other side, a
`wrapping_*`-family receiver or argument, a `Bits::from_uN` argument, an annotated `let`, a typed
assignment target), following untyped `let` copies and operands of unknown type (`i + j` of two loop
variables); `usize` when the only use is an index, realized as the 32-bit counter (the value of every
in-range index); `i32` when nothing types it. The transpiler rewrites the body's reads to `v as T` where
`T` differs from the declaration (an index read is left alone); the tick-bearing counter is typed through
`ForTyping` (the module's ports, consts and every `let`, before `chir_lower`'s context exists). The
interpreter keeps two unsuffixed bounds unsuffixed (was `usize`), so `v` takes its type at the use.
Fixture `tests/fixtures/loop_var_type_dut.rs`, 9 modules (the two §AA.3 probes as written, a use-typed
comb loop with a `let` copy, a typed ticking range, a signed `i8` range tick-free and ticking, rustc's
`i32` default with `i - 2 < 0`, a `u16` receiver, a negative `i64` range, a shadowing `let` — AB.5):
**18/18 cases pass** (before: AB.0, and 3 of the 3 added later failed). sv-baseline, 6 corpus modules
changed, each a read at the variable's Rust type, each sweep-agreeing: `cdo_for_override` (`8'(64'(i))`,
`from_usize(i)` makes `i` a `usize`), `ifr_tickfree_seq` (`a >> 8'(i)`, a `u8` const bound),
`ifr_ticking_break` (`$unsigned(8'(i))`, the default `i32` cast `as u8`), `ifr_ticking_const` (the `u8`
const bounds written as `u32` literals; the two `localparam`s no longer referenced), `tfr_nested`
(`64'(k) == (64'(i) + 64'(j))`, `as_usize() == i + j`), and `fold_defect` (`tests/ref_probes`, not swept:
`8'((64'(v) + 64'(i)))`, the same low byte). `comb_definite_assignment_parity.rs`'s pin `o = 8'(i)` is
now `o = 8'(64'(i))`.

### AB.3 Found and closed — `II.3 found signed-narrowing-cast`

Reading a 32-bit counter `as i8` showed it: a narrowing cast to a signed type emitted `$signed(x)` at the
operand's width, and from a signed one `$unsigned(x)` — right in an assignment of the target's width,
wrong anywhere wider (`(x as i8) as i32` did not truncate; `(x as i16) < 0` compared at 32 bits); an
unsigned widening of an `$unsigned(…)` was not stated either. Now `$signed(8'(x))`, `$unsigned(8'(s))`,
`16'($unsigned(8'(s)))`. Fixture `tests/fixtures/signed_narrowing_cast_dut.rs::snc_probe`: on `5aaa6ca`
`sim=ref≠SV` from cycle 0 (`q` expected 4294956489, got 13210569); now 2/2. **Corpus 0** (no corpus SV
changed).

### AB.4 Found and closed — `II.3 found primitive-assoc-const` (a parity break)

The new probes (`zzg_01`, `zzg_02`, `zzg_07`) found that the transpiler read no primitive associated
constant — `u8::MAX`, `i8::MIN`, `u16::BITS` (I.14.4's `T::K`) — which rustc and the interpreter read:
"cannot lower literal: u8 :: MAX", and R3's fold refused `250u8..=u8::MAX` as unfoldable. Pre-existing
(the corpus spells no such constant). `copper_analysis::consts::primitive_assoc_const` feeds `ConstEnv`
(`get`, `eval`, `ty_of`) and the transpiler's literal lowering and width inference (`u128::MAX` handled
apart: outside `i128`). Fixture `tests/fixtures/primitive_assoc_const_dut.rs`, 2 modules (as a value, in a
file const, as a ticking range's end; `i8::MIN`/`MAX`, `u16::BITS` in a combinational body): on `5aaa6ca`
both refused by the transpiler; now 4/4. II.1.3 (CLOSED) is re-verified with it (AB.6).

### AB.5 Found and closed — `II.3 found for-var-shadow`

`for i in 0u8..4 { let i = i + 1; … }` (`zzg_09`): the shadow's net took the loop variable's name — the SV
`always_comb` assigned the `int` loop variable itself (`i = (8'(i) + 8'd1)`; Verilator VARHIDDEN, WIDTH),
and in a ticking `for` the shadow would have been the counter. Pre-existing (the same lint failures on
`5aaa6ca`). A top-level `let` of the body that shadows the variable is renamed `<v>__shadow` (its
initializer still reads the loop variable; the later reads are renamed); a shadow inside a nested block is
not seen (recorded, not a line). Two attempts on the way, both measured: renaming the *loop variable*
instead (`<v>__iter`) made the tick-bearing counter a register the shared inference does not name
(`register_reconciliation`, P13: `j__iter` surplus); renaming the shadow then made the shadow a flop,
because the extraction route hoists a body `let` read in two states and the register authority judges a
hoisted name only when the *source* mentions it — `transpile_fir` now reads `<v>__shadow` as the source
local `v`, so the inference decides (dead at every tick: a wire). Rewritten reads keep their own spans.
Swept as `loop_var_type_dut.rs::lvt_shadow` (tick-free and ticking): lint-rejected on `5aaa6ca`, 2/2 now.

### AB.6 II.1.3 re-verified

§Z.8's 47 probes (notes folder `open-lines-e-Z-probes/battery`) and 16 new ones around this change
(`zzg_01`–`zzg_16`: `u8::MAX` as a bound and a value, constant and run-time maxima at `u8`/`u16`/`u128`, an
untyped end typed by its use, a `u16` receiver, a signed ticking range, an annotated `let`, a shadowing
`let`, a `match` on the variable, a `u64` exclusive run-time range, a negative `i64` range, two loop
variables in one index, the variable as a shift amount, an `i8` range at its minimum), copied as ui/pass
cases and removed after: **trybuild admits all 63; `front_end_parity_ui.rs::
the_transpiler_admits_every_shape_the_macro_admits` passes with them — 63/63 admitted by both.** On the
first transpiler pass 2 probes were refused (`zzg_01`, `zzg_02`: AB.4) and `zzg_07` too; after AB.4 all
16 transpile, and all 16 are Verilator `-Wall` clean once AB.5 and the `u128` done flag landed (re-checked on `4700ca4`) (on
`5aaa6ca`: 8 refused, 3 lint-rejected). The three named exceptions' test passes in AB.8's run.

### AB.7 Ledger and counts

Before: **223 lines — 221 CLOSED, 2 OPEN**. After (`cargo test --test conformance_ledger` on `23931ec`, and G-K in AB.8's run):
**226 lines — 226 CLOSED, 0 OPEN; 171 required by the SPEC; fixtures: 8 coreui, 3 example, 13 gone, 278
sweep, 158 test, 128 ui; 290 run checks.** Closed: `II.3 found inclusive-for-runtime-max` (D-28) and
`II.3 found loop-var-type`; `II.1.2 R23` stays a CLOSED line as a retired rule (`gone:` tokens, the ui case
in `pass`). Added and CLOSED: `signed-narrowing-cast`, `primitive-assoc-const`, `for-var-shadow`. No line is
OPEN.

### AB.8 The bare regression — measured

The first bare run (on `23931ec`) failed in G-J: the `u128` probe's bound `u128::MAX - 1` is one evaluator's
only — `ConstEnv` (over `i128`) declines it, the interpreter gives a value (its `u128` stops at
`i128::MAX`, so not this one). `2da5fe8` pins it `ASYMMETRIC` with that reason (9 rows). The second (on
`2da5fe8`) failed in `timing_model_derivations.rs`: `lvt_shadow`, added after the first workspace run, takes
the extraction route too; `170bc8c` adds it. The third (on `170bc8c`) failed in `register_reconciliation`
(AB.5); `4700ca4` is the fix.

**The fourth: bare `tools/regression.sh` on `4700ca4` (the final code commit; laptop; `paper` at `5aaa6ca`):
REGRESSION OK (331 s).** 1668 passed, 0 failed, 16 ignored, 159 test binaries (G-C 132 test files);
examples **32/32**; G-D 322 differential cases (3 ignored-with-reason), G-F 322 (3 skipped with a reason) —
the 19 new fixture modules added; **G-G differential 2395 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0
DISAGREE**, **reference 2407 PASS / 0 DISAGREE**; G-I 250 clocked related (66 extracted, 19 linear, 165
single-tick), 0 uncovered; G-J 85 const items compared / 25 outside both, 218 counted-`for` bounds compared,
9 asymmetric (pinned), 0 DISAGREE; G-E 5 anchored; **G-K `226 lines — 226 CLOSED, 0 OPEN; 171 required by
the SPEC; fixtures: 8 coreui, 3 example, 13 gone, 278 sweep, 158 test, 128 ui; 290 run checks`**, the 3
example fixtures PASS. Not run remotely (nothing pushed). After this run, docs only: III.3.6's counts,
D-28 in `SPEC_DECISIONS.md`, this section, CLAUDE.md.

## AC. A shadow in a nested block: `for-var-shadow-nested` and the wider `scoped-shadow` closed; two unrelated findings OPEN (2026-11-09)

Branch `claude/nested-for-shadow-c6819e`, cut from `paper` at `0751544` (ledger 226 lines — 226 CLOSED,
0 OPEN, but with §AB.5's unledgered gap). `7fb1772` added the OPEN line `II.3 found for-var-shadow-nested`
before any code change (ledger 227 — 226 CLOSED, 1 OPEN). `e6a3b66` is the code, the fixture and the
ledger lines; `4b32ed8` (AC.9: the `continue` test the first regression run failed, and `ssh_continue_rest`), the final code commit — the bare regression of AC.9 ran on it; after it, docs only.
"Before" is measured: the probes and the fixture swept against `0751544`'s `copper-codegen/src` (checked
out over the committed tree and restored), every module's emission against an `sv-baseline snapshot`
taken on `0751544`'s codegen. Predictions and probe notes: notes folder `nested-for-shadow-AC.md`.

### AC.0 Predictions (written before reading any probe output)

| shape | predicted | measured |
|---|---|---|
| tick-free `for`, nested shadow of its variable (combinational; clocked linear; clocked extraction) × (`if`, `match` arm, bare block) | the shadow is one SV signal with the loop variable; inside `for (int i …)` the name resolves to the SV loop variable, so the shadow assigns it — VARHIDDEN + WIDTH, lint-rejected | **as predicted, 9 of 9** (VARHIDDEN, WIDTHEXPAND, WIDTHTRUNC, UNUSEDSIGNAL) |
| tick-bearing `for` over a `u8` range, same three blocks | `var` IS the counter in `expand_counted_for`: the shadow writes it, the trip count changes; silent, or a width refusal | the counter is written (`i <= (8'(i) + 8'd1) + 32'd1`), **lint-rejected** (WIDTHEXPAND: the 8-bit shadow into the 32-bit counter), 3 of 3 |
| tick-bearing `for` over a `u32` range (the shadow's width the counter's) | Verilator-clean, `sim=ref≠SV` | **as predicted: `sim=ref≠SV` on 20 of 20 sequences, Verilator-clean** (`ssh_tick32_if` from cycle 0–4, `q` sim 35 SV 51; `ssh_tick32_block` from cycle 0) — the feared silent case |
| wider: a nested shadow of a local (comb), of a register, of a port-read local, across a tick; a top-level body `let` shadowing a register; an outer loop's variable shadowed in an inner loop; an inner `for` rebinding the outer variable | shir_lower's own comment, "one SV signal either way": `sim=ref≠SV` for the locals and the register, lint for the loop-variable cases | locals: **`sim=ref≠SV`** (comb local, port-read local, across a tick); registers: **a duplicate SV declaration** (Verilator `%Error`), not a silent divergence — one wrong prediction; loop variables: lint-rejected (VARHIDDEN), as predicted |

The answer to the question this section was opened for: **yes — a nested shadow in a tick-bearing `for`
whose range type is 32 bits wide was `sim=ref≠SV` and Verilator-clean**, and so was a nested shadow of an
ordinary local, a port-read local or a local live across a tick, in any body. Every variant over a
narrower range or in a tick-free `for` was caught by Verilator's lint.

### AC.1 The mechanism

§AB.5 renamed a top-level `let` of a `for` body that shadows its variable (`cast_loop_var_reads`, in both
`chir_lower` and `control_extract::expand_counted_for`). Below that, and for every other binding, the
lowering maps every binding of one name onto one SV signal (`shir_lower::stmts_reassign`: "a shadowing
`let` … one SV signal either way"). That is right for a re-`let` in the same block — the later binding
hides the earlier one for the rest of the block, as a reassignment does — and wrong for a shadow in an
inner block, which ends at the block's end, after which the outer binding is visible again, unchanged.

### AC.2 The fix — `desugar::rename_scoped_shadows`

The last step of `desugar`, before any other pass looks at the body: a scoped walk over the FIR in which
every `let` (a tuple `let` per binder) and every `for` pattern that shadows a binding of an OUTER scope
gets a name of its own, `<x>__shadow` (then `<x>__shadow1`, … for a name already in the module), for its
scope; a same-block re-`let` keeps the name the block already lowers it under. Its initializer is
rewritten before the binding takes effect (`let x = x + 1` reads the outer `x`). Scopes: a block, a branch,
a loop body, a `for` body (inside the pattern's scope — so a top-level body `let` shadows the variable, as
§AB.5's did), a `match` arm. A `match` arm's binders, an `if let`'s and a closure's are not renamed but
mask an outer binding of their name in their scope, so a read there is never redirected. §AB.5's two
lessons kept: the SHADOW is renamed, never the loop variable (the tick-bearing counter keeps the source
name the register inference knows); and a hoisted shadow is judged as a register by its own liveness, not
by name — `desugar::shadows_live_at_tick` (a statement of the shadow's scope holds a tick and that
statement or a later one reads the shadow; a shadow's scope is entered afresh on every pass through its
block, so no back edge carries it; a renamed tick-bearing `for` variable is its counter, always live).
The first cut appended a hoisted shadow whenever the inference named its source local — `lvt_shadow`'s
dead `j__shadow` would have become a flop (AB.5's measured failure) — and was replaced before any
measurement was recorded. A second slip, caught by sv-baseline: the `for` body first shared the pattern's
scope, so a top-level body `let i` counted as a same-block re-`let` and `lvt_shadow` regressed to the
pre-§AB.5 SV; the body is now its own scope.

§AB.5's top-level rename (`body_shadows`, the shadow branch of `cast_loop_var_reads`) is deleted: one
mechanism. `lib.rs` reads a hoisted `<x>__shadow[k]` as the source local `x` (`desugar::shadow_source`);
`register_reconciliation.rs` (P13) accepts `<x>__shadow[k]` as a spelling of an inferred `x`, beside
`_r` and `_sig` (the inference is by name and gives both bindings one entry); `timing_model_derivations.rs`
adds `ssh_for_shadows_local` to its extraction-route set (a ticking `for` and a tick after it in one module
loop, as `lvt_shadow`).

### AC.3 Fixture `tests/fixtures/scoped_shadow_dut.rs` — 22 modules

| module | shape | on `0751544` | now |
|---|---|---|---|
| `ssh_comb_if`, `ssh_comb_match`, `ssh_comb_block` | tick-free `for`, combinational; the block one with a shadow of the shadow two blocks in and a same-block re-shadow | lint-rejected (VARHIDDEN, WIDTH) | 2/2 each |
| `ssh_lin_if` (both branches shadow), `ssh_lin_match` (an arm and a bare block) | tick-free `for`, clocked, linear route | lint-rejected | 2/2 each |
| `ssh_ext_if`, `ssh_ext_match` | tick-free `for`, clocked, extraction route | lint-rejected | 2/2 each |
| `ssh_tick_if`, `ssh_tick_match`, `ssh_tick_live` (the shadow live across the `for`'s tick: a register) | tick-bearing `for`, `u8` | lint-rejected (WIDTHEXPAND) | 2/2 each |
| `ssh_tick32_if`, `ssh_tick32_block` | tick-bearing `for`, `u32` | **`sim=ref≠SV`, 20/20 sequences** | 2/2 each |
| `ssh_outer_var` | an outer loop's variable shadowed in an inner loop's branch; an inner `for` rebinding it | lint-rejected | 2/2 |
| `ssh_inner_rebinds_counter` | a tick-free inner `for` rebinding a tick-bearing outer one's variable | lint-rejected (VARHIDDEN) | 2/2 |
| `ssh_comb_local` | a nested shadow of a local; a tuple `let` shadowing two | **`sim=ref≠SV`** | 2/2 |
| `ssh_arm_binder` | a `match` arm's binder inside a branch that shadows the local | **`sim=ref≠SV`** | 2/2 |
| `ssh_reg` | a register shadowed in a branch | duplicate SV declaration | 2/2 |
| `ssh_reg_top` | a top-level loop-body `let` shadowing a register (`let mut r`) | passed as first written with an immutable pre-loop `r` (a wire); the committed `let mut r` is the duplicate-declaration case of the probe `wsp_reg_top` | 2/2 |
| `ssh_port_local` | a port-read local shadowed in a branch | **`sim=ref≠SV`** (the probe's `match` form — AC.6) | 2/2 |
| `ssh_across_tick` | a local shadowed in a branch, live across a tick (extraction) | **`sim=ref≠SV`** | 2/2 |
| `ssh_for_shadows_local` | a tick-bearing `for` whose variable shadows an outer local, read after the loop | lint-rejected (UNUSEDSIGNAL) | 2/2 |

| `ssh_continue_rest` | a tick-free `for`'s `continue` whose rest names a local shadowed in the branch (AC.9) | refused (the `continue` rewrite declined) | 2/2 |

On `0751544`: **20 of the first 21 differential cases failed** (every reference leg passed), and `ssh_continue_rest` was refused. Now **44/44**.

### AC.4 sv-baseline against `0751544`

**0 corpus modules changed**; the 22 fixture modules are new. (The probe files were swept and deleted
before the diff; `lvt_shadow` changed in the first cut — AC.2 — and is unchanged in `e6a3b66`.)

### AC.5 The wider class — `II.3 found scoped-shadow` (CLOSED)

The task's step 5: shadowing beyond a `for`'s own variable. Same mechanism (AC.1), so closed by the same
rename: the seven `ssh_` modules of the last rows of AC.3. One prediction was wrong: a register shadow
was predicted `sim=ref≠SV` and is a loud SV error (the shadow's declaration duplicates the register's).

### AC.6 Found, not fixed — `II.3 found stmt-match-wire` (OPEN, copper-codegen)

The probe `wsp_port_local` (a nested shadow in a `match` arm) still failed with the rename in place, with
the same values as before it (`q` sim 95 SV 80). Isolated without any shadow: in a clocked body, a wire
local (a loop-body `let mut s`) assigned inside a statement `match`'s arm and read after the `match` in
the same resumption is dropped from the forwarded value — `match a & 3 { 1 => { s = 5; } _ => {} }
q.write(s)` emits `q <= 8'd0` — on every clocked route (single-tick, two-phase linear, extraction;
probes `arm_a`–`arm_d`, `arm_port_scrut`, `arm_lin2`, `arm_ext` in the notes folder). The same assignment
under an `if`, the same `match` in a combinational body, and a REGISTER assigned in the arm are right.
`sim=ref≠SV` on 20 of 20 sequences, Verilator-clean, pre-existing, corpus 0 (the sweep agrees on every
corpus module). A different mechanism (forwarding through a `Match`), so left OPEN per the task; fix
direction recorded on the line. (A deferred `let s;` assigned in both arms is refused, "undefined
variable"; not lined.) The fixture's port-read case uses an `if`.

### AC.7 Found, not fixed — `II.3 found nested-ticking-for` (OPEN, copper-codegen) — a parity break

A first fixture module had a tick-bearing inner `for i` rebinding a tick-bearing outer `for i`. The
transpiler refused it, and with distinct names too (probe `nt_distinct`): "a repeating wait must be written
as `loop { <test>; clk.tick().await; }`", from `chir_lower.rs`'s copy of the shape rule after the
counted-`for` desugar. The macro admits it and the simulator and interpreter agree on it (20/20), so it is
a macro-admits / transpiler-refuses shape: **II.1.3 is OPEN again** (owner copper-codegen), pointing at
the line. Pre-existing, corpus 0. Fix direction: lower it, or refuse it in both front-ends with one
message — a user decision if the second. The fixture keeps a tick-free inner `for` rebinding a ticking
outer one's variable (`ssh_inner_rebinds_counter`).

### AC.8 Ledger and counts

Before: **226 lines — 226 CLOSED, 0 OPEN** (`0751544`). After: **230 lines — 227 CLOSED, 3 OPEN; 171
required by the SPEC; fixtures: 8 coreui, 3 example, 13 gone, 300 sweep, 155 test, 127 ui; 312 run
checks.** Added and CLOSED: `for-var-shadow-nested` (added OPEN in `7fb1772`, closed in `e6a3b66`),
`scoped-shadow`. Added OPEN: `stmt-match-wire`, `nested-ticking-for`. Reopened: II.1.3 (its tokens no
longer counted: `test` 158 → 155, `ui` 128 → 127). `for-var-shadow`'s text says what does it now.

### AC.9 The bare regression — measured

The first bare run (on `e6a3b66`) failed in `copper-codegen/tests/unsupported_constructs.rs::
a_tickfree_for_continue_whose_rest_names_a_branch_binding_is_refused_with_its_own_message` (1498 passed,
1 failed): the test expected `for_continue` to decline a `continue` whose rest names a binding made in the
branch (the copy of the rest would capture it). `rename_scoped_shadows` runs first, and such a binding is
necessarily a shadow of an outer one, so it has its own name and the rest lowers reading the outer
binding — a shape refused before, now IN; the decline is a backstop. `4b32ed8` renames the test
(`…_shadowed_binding_lowers`, asserting the branch reads `t__shadow` and the rest `t`), sweeps the module
as `ssh_continue_rest` (2/2) and moves line `II.3 found tickfree-continue`'s token to both.

**The second: bare `tools/regression.sh` on `4b32ed8` (the final code commit; laptop; `paper` at
`0751544`): REGRESSION OK (320 s).** 1712 passed, 0 failed, 16 ignored, 159 test binaries (G-C 132 test
files); examples **32/32**; G-D 344 differential cases (3 ignored-with-reason), G-F 344 (3 skipped with a
reason) — the 22 new fixture modules added; **G-G differential 2702 PASS / 12 OUTSIDE / 0 TRUNCATED / 0
EXCUSED / 0 DISAGREE**, **reference 2714 PASS / 0 DISAGREE**; G-I 265 clocked related (76 extracted, 21
linear, 168 single-tick), 0 uncovered; G-J 85 const items compared / 25 outside both, 256 counted-`for`
bounds compared, 9 asymmetric (pinned), 0 DISAGREE; G-E 5 anchored; **G-K `230 lines — 227 CLOSED, 3
OPEN; 171 required by the SPEC; fixtures: 8 coreui, 3 example, 13 gone, 300 sweep, 155 test, 127 ui; 312
run checks`**, the 3 example fixtures PASS. Not run remotely (nothing pushed). After this run, docs only:
III.3.6's counts, this section, CLAUDE.md.

## AD. The three OPEN lines of §AC: `stmt-match-wire` and `nested-ticking-for` closed, three more found, two closed; II.1.3 stays OPEN (2026-11-09)

Branch `claude/open-lines-h-d93d51`, cut from `paper` at `730a815` (ledger 230 lines — 227 CLOSED, 3 OPEN).
`0862490` and `60f6225` added the OPEN lines found while probing, before any fix (`for-bound-loop-var`,
`forwarded-narrow-widening`; `ticking-for-ends-in-loop`). "Before" is measured: every fixture module swept
against `730a815`'s `copper-codegen/src` (checked out over the tree and restored), every module's emission
against an `sv-baseline snapshot` taken on `730a815`. Predictions, probes and before-logs: notes folder
`open-lines-h-AD.md`, `open-lines-h-AD-probes/`.

### AD.1 `II.3 found stmt-match-wire` — CLOSED

**Mechanism** (read before measuring): `shir_lower::extract_updates_from_stmts`'s `Match` arm ran each arm
from a copy of the forwarding map and merged the arms' REGISTER updates into a `Case`, but dropped the
copies — a wire an arm assigned kept, after the `match`, the value it had before it. The `If` arm merges
both (registers, then every wire whose forwarded value changed in a branch). **Fix:** the `Match` arm keeps
each arm's map and merges every changed wire into a `Case` over the arms, guards resolved once.

Fixture `tests/fixtures/stmt_match_wire_dut.rs`, 13 modules (linear single-tick, two-phase linear,
extraction, the trailing region; an arm that assigns and one that does not; every arm; a guarded and a
binding arm; a `match` in an `if`, an `if` and a `match` in an arm; a `match` in a tick-free `for`; a wire
whose initializer reads a register; a port write inside the arm; a condition and a register update after
the `match`; a `Bits` bit write in an arm).

| prediction (before measuring) | measured on `730a815` |
|---|---|
| `sim=ref≠SV`, Verilator-clean: every module whose wire an unguarded `match` arm assigns and something after the `match` reads | **11 of 13 fail, all as predicted but one**; every reference leg passes |
| `smw_guard_bind` agrees (`desugar` lowers a guarded/binding `match` into an `if` chain) | agrees |
| `smw_in_for` agrees (after a tick-free `for` a name is read by name) | agrees |
| `smw_write_in_arm` agrees (the port write is inside the arm) | **fails — the one wrong prediction**: its other arm's `if s == 50` follows a nested `match` inside the arm, the same mechanism one level down |

Now 26/26. **sv-baseline against `730a815`: 0 corpus modules changed** (predicted: 0 — §AC.6, the sweep
agreed on every corpus module, so none read an arm-assigned forwarded wire after a `match`). The `if` chain
§Z lowers a guarded or binding `match` into was already right (`smw_guard_bind`), and no other statement
form drops writes: the walk's other compound arms are `If` (merges) and `ForLoop` (reads by name after).

### AD.2 `II.3 found nested-ticking-for` — CLOSED (the SPEC gives it a meaning: lowered)

**Does the SPEC decide it?** Yes. I.8.2 makes every tick of a nested loop — a `loop`, a desugared `while`,
a counted `for` — a suspension site of the kernel, and I.8.2(b) admits a nested body whose only bare tick is
its last statement or that ends in a loop ending at one; I.8.5's `D` is a rewrite that composes. A tick-bearing
`for` in a tick-bearing `for`'s body, followed by statements and the body's tick, meets both; the macro's
`check_nested_loop_shape` is that rule on the source. So it is lowered, not a decision packet.

**Mechanism:** `desugar_counted_loops` expands the inner `for` first, to `let c; loop { … }; tick` — its
tick hoisted after the loop (the §8 cause-K comment of `expand_counted_for`: with the tick inside, a `for`
LAST in its body has an empty continuation, and the flattener's fall-off costs a cycle). In the middle of
the enclosing body that hoisted tick is a bare statement, so the outer body reads `W; tick; C; tick` and
the flattener's copy of the shape rule (`body_ends_at_a_clock_boundary`) refuses it. Narrowed on probes:
an outer body ENDING in the inner `for`, a `loop` wait in a `for`, a `loop` in a `loop` all transpiled; a
`for` in a nested `loop` followed by a tick did not. **Fix:** a tick-bearing `for` that is not the last
statement of a nested loop's body keeps its last tick inside its loop — `loop { <exit test>; B; c = c + 1;
if <passed> { tick; break } tick }` — when its hoisted tick would be bare (a range non-empty by its
constants, no author `break`): its exit continues into statements that reach the body's boundary, so cause K
cannot arise, and no bare tick sits mid-body. The first cut was I.8.5's `D` plain (`loop { <exit test>; B;
c = c + 1; tick }`): the sweep agreed, but G-I's P1 failed on four modules — the source walker labels a
path with the `for`'s "last iteration?" decision before the tick (`for=Last` / `for=More`), and plain `D`
takes it at the next resumption's head; the arm form takes it where the source does, and P1 holds. A guarded hoisted tick (`if … { tick }`,
an author `break` or a run-time range) was never bare and keeps its form.

Fixture `tests/fixtures/nested_ticking_for_dut.rs`, 9 modules:

| module | shape | predicted on `730a815` | measured on `730a815` | now |
|---|---|---|---|---|
| `ntf_distinct` | §AC.7's probe | refused | refused | 2/2 |
| `ntf_interact` | inner `0..i` (zero iterations at `i = 0`), both counters read | refused | **transpiled, lint-rejected** (AD.3) | 2/2 |
| `ntf_break` | an inner `break` before its tick | refused | **2/2** (guarded hoisted tick) | 2/2 |
| `ntf_runtime_empty` | an inner range empty at run time | refused | **2/2** (guarded) | 2/2 |
| `ntf_three` | three levels, an anonymous middle counter, an inclusive inner range | refused | refused | 2/2 |
| `ntf_in_loop_while` | the inner `for` in a nested `loop` and in a `while` | refused | refused | 2/2 |
| `ntf_if_inner` | the inner `for` inside an `if` | agrees | agrees | 2/2 |
| `ntf_then_for_then_wait` | an inner `for` followed by another; one followed by a wait and a tick | refused | refused | 2/2 |
| `ntf_bound_var` | AD.3's shape | — | lint-rejected | 2/2 |

Three predictions wrong (`ntf_break`, `ntf_runtime_empty`: a guarded hoisted tick is an `if`, so the body's
only bare tick stays last; `ntf_interact`: the same, then AD.3). Every reference leg passed before and
after. **sv-baseline: 0 corpus modules changed** by this part (predicted 0: the new form applies only where
the hoisted bare tick made the transpiler refuse). The fix first keyed on a following BARE tick; the battery
(AD.6, `zzh_06`: an inner `for` followed by another tick-bearing `for`) refused, and the rule became "not the
body's last statement" (the body ends at its boundary by the shape rule).

### AD.3 Found and closed — `II.3 found for-bound-loop-var`

`ntf_interact`'s first measurement: Verilator WIDTHEXPAND/WIDTHTRUNC. `for j in 0..i`, `i` an enclosing
`for`'s `u8` variable: rustc types `j` `u8`; `chir_lower::bound_int_type` looked `i` up in the symbol table,
where a loop variable is not, so `j` fell back to `i32`. Probes (tick-free nests with a variable bound are
R5's refusal in both front-ends, so only a tick-bearing inner `for` reaches it): `bvp_tick_both` (= the
fixture's `ntf_bound_var`) lint-rejected — loud, never silent in the forms tried. OPEN line added in
`0862490`, before the fix. **Fix:** `desugar_counted_loops` keeps the enclosing tick-bearing `for`s'
variables whose type is determined (`Consts::enclosing`, `for_var_scope`) and `ForTyping::var_type` puts
them in scope. Both modules 2/2; corpus 0.

### AD.4 Found and closed — `II.3 found forwarded-narrow-widening` (a silent miscompile)

Probing a run-time inner bound (`let n: u8 = …; for j in 8..n`) for AD.3: `bvp_tick_inner` lint-rejected
(WIDTHEXPAND inside `32'(((8'(a) & 8'd3) + 8'd9))`), predicted to agree — wrong. The wrapping variant
`bvp_rt_wrap` (`n = a.wrapping_add(250); for _ in 250u8..n`) is **Verilator-clean and `sim=ref≠SV` on 20 of
20 sequences**: `n` is 0 in Rust for `a = 6`, and `32'(8'(a) + 8'd250)` is 256 — IEEE 1800 sizes a cast's
operand by the cast's width. Not a `for` mechanism: a register assigned a wrapping `u8` and compared `as u32`
in the same resumption (`bvp_fwd_reg_cmp`, `_cmp1`) fails the same way. **Mechanism:**
`chir_lower::widening_resize` gives a compound operand its own width before widening (`32'(8'(e))`) but a
name none (`32'(r)`, a name is self-sized); `shir_lower::subst_vars` then substitutes `r`'s forwarded value
under the cast. All pre-existing (the same five probes fail on `730a815`). OPEN line added in `0862490`,
before the fix. **Fix:** a name directly under a widening `Resize` (or `Resize(SignCast(name))`) whose
substitute is not self-sized keeps its own width (`shir_lower::at_own_width`; the widths of the body's
registers, wires and promoted carriers, `seq_var_widths`).

Fixture `tests/fixtures/forwarded_width_dut.rs`, 8 modules. **Predicted on `730a815`:** `fnw_reg_cmp`,
`_cmp1`, `_signed` (`as i32` of a wrapping product), `_match_value` (a `match`-merged wire widened after the
`match`), `_for_bound_wrap` fail silently, `_for_bound_narrow` lint-rejected, the two controls (a port-read
bound, a bound in the header) agree. **Measured: as predicted, 6 fail, 2 agree; every reference leg passes.**
Now 16/16. **sv-baseline: predicted a subset of the 55 baseline modules whose SV contains `N'((`; measured 1
corpus module changed, `pbt_aggregates`** (`8'((__copper_started ? … : s_x))` → `8'(4'(…))`, a trailing-region
mux whose arms are 4 bits — the same values), and it agrees.

### AD.5 Found, not closed — `II.3 found ticking-for-ends-in-loop` (OPEN, a parity break)

The battery's `zzh_05`: a tick-bearing `for` whose body ENDS in a hand-written wait loop, an inner `for`
ticking first (so R3 holds). The macro admits it; `expand_counted_for` declines it (no last bare tick to
hoist). OPEN line added in `60f6225`, before any fix. Tried: `D` with a counter of its own (`let v = c;
c = c + 1; B`, the loop ending at `B`'s wait). The transpiler then declined in `extract_control`:
`splice_zero_time_gotos` found a zero-time goto cycle — through the inner `for`'s empty-range exit (never
taken: the range is constant and non-empty, which R3 knows), the skippable wait, and the back edge. The
hand-written twin `h1` (a nested `loop` with a counter, no bare tick, ending in a wait) is admitted by the
macro and refused the same way. The flattener's cycle test is structural, R3's is not: a lowering change of
its own, or a refusal in both front-ends (a user decision). Reverted; the line stays OPEN with both probes.
The same bodies with no earlier tick are R3's refusal in both (measured: `fe_top`, `fe_while`, `h2`).

### AD.6 II.1.3 re-verified — stays OPEN

§Z.8's 47 probes, §AB.6's 16 and 16 new (`zzh_01`–`zzh_16`: nested tick-bearing `for`s, a `for` in a nested
`loop` and `while`, three levels, an inner `for` in an `if`, an inner `break`, interacting and inclusive
bounds, an inner `for` then a wait, then another `for`, statement-`match` wires with guards, widening casts,
anonymous counters, a run-time inner range, a nested `for` in the trailing region), copied into
`copper-macros/tests/ui/pass` and removed after. **Predicted:** all 79 admitted by the macro; the transpiler
admits all but possibly `zzh_05` and `zzh_06`. **Measured:** trybuild admits 79/79; the transpiler refused
exactly `zzh_05` and `zzh_06`; after AD.2's widening, 78/79 — `zzh_05` is AD.5. II.1.3 stays OPEN, its owner
text now pointing at `ticking-for-ends-in-loop`.

### AD.7 Ledger and counts

Before: **230 lines — 227 CLOSED, 3 OPEN** (`730a815`). After (G-K in AD.8's run): **233 lines — 231
CLOSED, 2 OPEN; 171 required by the SPEC; fixtures: 8 coreui, 3 example, 13 gone, 331 sweep, 155 test, 127
ui; 342 run checks.** Closed: `stmt-match-wire`, `nested-ticking-for`. Added OPEN before any fix, then
closed: `for-bound-loop-var`, `forwarded-narrow-widening` (`0862490`). Added OPEN: `ticking-for-ends-in-loop`
(`60f6225`). II.1.3 stays OPEN, now on `ticking-for-ends-in-loop`.

### AD.8 The bare regression — measured

A workspace `cargo test` before the code commit found two harness checks failing on the first cut. G-I's P1,
on four `ntf_` modules (AD.2: plain `D` takes the "last iteration?" decision at the next head; the arm form
replaced it). And `timing_model_derivations`, whose expected extraction-route set gains `fnw_for_bound_*` and
`ntf_in_loop_while`, `ntf_then_for_then_wait` (new modules of the set's kind, as `for_empty` and `det_for`).
**Bare `tools/regression.sh` on `77f5c65` (the final code commit; laptop; `paper` at `730a815`): REGRESSION
OK (330 s).** 1772 passed, 0 failed, 16 ignored, 159 test binaries (G-C 132 test files); examples **32/32**;
G-D 374 differential cases (3 ignored-with-reason), G-F 374 (3 skipped with a reason) — the 30 new fixture
modules; **G-G differential 3302 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0 DISAGREE**, **reference 3314
PASS / 0 DISAGREE**; G-I 295 clocked related (90 extracted, 23 linear, 182 single-tick), 0 uncovered; G-J 85
const items compared / 25 outside both, 303 counted-`for` bounds compared, 9 asymmetric (pinned), 0
DISAGREE; G-E 5 anchored; **G-K `233 lines — 231 CLOSED, 2 OPEN; 171 required by the SPEC; fixtures: 8
coreui, 3 example, 13 gone, 331 sweep, 155 test, 127 ui; 342 run checks`**, the 3 example fixtures PASS. Not
run remotely (nothing pushed). sv-baseline against `730a815` on `77f5c65`: 30 new modules, 1 changed
(`pbt_aggregates`, AD.4). After this run, docs only: III.3.6's counts, this section, CLAUDE.md.

## AE. D-29: `ticking-for-ends-in-loop` closed (lowered); `ticking-for-generic-range` found, OPEN with a decision packet; II.1.3 stays OPEN (2026-11-09)

Branch `claude/open-lines-i-a66403`, cut from `paper` at `e71f7d5` (ledger 233 lines — 231 CLOSED, 2 OPEN).
`e4961c4` added the OPEN line found while probing, before any fix (`ticking-for-generic-range`; widened in
`c4fd345` to its measured class). "Before" is measured on `e71f7d5`'s code (the same binary, then
`control_extract.rs` checked out over the tree and restored for zzi_29–34); every module's emission against an
`sv-baseline snapshot` taken on `e71f7d5`. Predictions, probes and before-logs: notes folder `open-lines-i-AE.md`,
`open-lines-i-AE-probes/`.

### AE.0 Mechanism (read before measuring)

R3 already decides the shape the D-29 way in both front-ends: a ticking `for` over a literal non-empty range
enters its body (`cfg.rs::statically_nonempty_range`); a constant-shaped named range is decided by `ConstEnv` in
the transpiler (the empty-range oracle) and by a rustc `const _: () = assert!(..)` per dependency set in the macro
(`empty_range_dependencies`, I.8.4(b), §V.3); a range whose bound is not constant-shaped is an exit. So the break
was in the realization only: (1) `expand_counted_for` declined a `for` whose body ends in a nested loop (no last
tick to hoist); (2) the unhoisted form of §AD.2 kept its exit test at entry, dead over a constant non-empty range,
and `splice_zero_time_gotos`'s structural cycle test walked through it (head → inner empty exit → skippable wait →
back edge). No front-end rule changes.

### AE.1 `II.3 found ticking-for-ends-in-loop` — CLOSED (D-29, lowered)

**Fix** (`c4fd345`): a tick-bearing `for` whose body ends in a nested tick-bearing loop takes I.8.5's `D` with no
hoisted tick — `loop { <exit test>; B; c = c + 1 }`, and over a range `ConstEnv` proves non-empty
`loop { B; c = c + 1; if <passed> { break } }` (the entry test is never taken there; dropping it leaves the cycle
test no dead path). The unhoisted form drops its entry test (its last iteration leaves through
`if <passed> { tick; break }`). `body_ends_at_a_clock_boundary` — the flattener's own copy of the shape rule —
admits tick-free statements after a body's last nested loop (they run in the `break` continuation, before the
zero-time return to the head; the source is gated by the shared `check_nested_loop_shape` in both front-ends, so
no source shape is widened). G-I's source walker, which required a tick to end every counted-`for` iteration,
takes the Last/More decision at a back edge reached without one — where the emitted `D` tests. Without that, the
first projection run would have failed on every new module (the walker's error), so it was changed with the
lowering. G-I notes one unreachable emitted state per new module (the head state of a nested loop with no
top-level tick, entered only through its inlined copy) — the note `arms_only_*` already carry.

Fixture `tests/fixtures/ticking_for_ends_in_loop_dut.rs`, 8 modules, all on the extraction route (a nested
ticking loop always is — there is no linear-route form of the shape):

| module | shape |
|---|---|
| `tfl_outer_ends_wait` | §AD.5's `zzh_05` |
| `tfl_handwritten` | §AD.5's `h1`, a hand-written `loop` with its own counter |
| `tfl_const_items` | outer range through a file `const`, inner through a body-local `const` |
| `tfl_loop_while_const` | a hand-written `loop` with an inner `0..TFL_K`; a `while` of the same shape |
| `tfl_outer_runtime` | an outer range known only at run time (zero iterations), followed by a tick |
| `tfl_break_inclusive_two` | an author `break`; arithmetic-literal and inclusive inner ranges; two inner `for`s |
| `tfl_three_arms` | three levels, the middle `for` ending in the wait; the inner `for` in one arm |
| `tfl_waits_wide_max` | wait, inner `for` over `u32` consts, wait; the variable read after a wait; end `u8::MAX` |

Predicted on `e71f7d5`: all 8 refused by the transpiler, admitted by the macro — measured (the probes they were
built from, zzi_01–28: see AE.2). Now 16/16 (differential and reference legs). ui: `fail/runtime_for_ends_in_loop`
(an inner run-time range: R3's message, one message in both — `front_end_parity_ui` checks the transpiler's) and
its twin `pass/ticking_for_ends_in_loop_ok`.

**sv-baseline against `e71f7d5`:** predicted exactly `ntf_distinct`, `ntf_three`, `ntf_in_loop_while`,
`ntf_then_for_then_wait` (the only modules with an unhoisted form over a constant range; `ntf_interact`,
`ntf_bound_var` have run-time inner bounds, `ntf_break` a `break`, `ntf_if_inner` its `for` in an arm).
**Measured: those 4 changed** (before the new fixture was added: 4 of 442), all sweep-agree; plus the 8 new modules.
`timing_model_derivations`' extraction-route set gains `tfl_break_inclusive_two`, `tfl_handwritten`,
`tfl_outer_runtime` (new modules of its kind, as §AD.8's).

### AE.2 The zzi battery — predictions and measurements

28 probes written and predicted before any measurement (notes `open-lines-i-AE.md`), then 6 more around the
const-generic finding (zzi_29–34). Macro: trybuild ui/pass copies (removed after). Transpiler: `copper-transpile`.

| probes | predicted | measured on `e71f7d5` | after `c4fd345` |
|---|---|---|---|
| 01–03, 06, 07, 09, 11–13, 15, 17, 18, 20, 21, 23, 24, 26, 28 (a `for`/`loop`/`while` ending in a wait, R3 through a constant inner `for`; 06/20 an outer run-time range) | macro admits, transpiler refuses; after: both | as predicted | both admit |
| 05 (inner run-time range), 10 (inner `break` before its tick), 14 (inner `0..Z`, `Z = 0`), 16 (`0..two()`, a `const fn`) | refused by both, one message | as predicted (R3's message; the macro's for 14 is its E0080 assertion with the same text) | unchanged |
| 22 (a bare tick mid-body, then a wait), 27 (an `if { wait } else { tick }` last) | refused by both (shape) [uncertain] | as predicted | unchanged |
| 08, 19, 25 | both admit | as predicted | both admit |
| 04 (inner `0..W`, `W` a const generic) | macro admits, transpiler refuses, before AND after | as predicted | **refused** — AE.5 |
| 29–32, 34 (a const-generic range R3's verdict depends on) | — (written after 04) | macro admits all; transpiler refuses all | refused — AE.5 |
| 33 (a const-generic inner range followed by a tick) | — | both admit | both admit |

Every prediction held, including the two marked uncertain.

### AE.3 Not a new line: the outer run-time range

zzi_06 and zzi_20 (an outer `for` over a run-time range ending in a wait, R3 holding through a constant inner
`for`, a tick after the `for`) were refused by the same mechanism as `zzh_05` (`expand_counted_for` declines) —
recorded under `ticking-for-ends-in-loop` (its text widened), and lowered by the same change with the exit test at
the top (`tfl_outer_runtime`).

### AE.4 II.1.3 re-verified — stays OPEN

§AD.6's 79 probes and the 34 zzi probes, 113 modules: **the macro admits 107; the transpiler 101.** The 79 old
probes are admitted by both (`zzh_05` now too). The six both refuse are refused with one message each. The six
the transpiler alone refuses are all line `ticking-for-generic-range` (zzi_04, 29, 30, 31, 32, 34). II.1.3 stays
OPEN, its owner text now pointing at that line.

### AE.5 Found, not closed — `II.3 found ticking-for-generic-range` (OPEN, a parity break) — decision packet

**Question.** A tick-bearing `for` over a range bounded by a const generic (`0..W`), where R3's verdict depends on
the range being non-empty — the module loop's only boundary (`loop { q.write(..); for _ in 0..W { tick } }`,
zzi_29), followed by a skippable wait (zzi_30), inside a hand-written nested loop ending in one (zzi_31), or either
`for` of `ticking-for-ends-in-loop`'s shape (zzi_04, 32, 34). Admitted (as R3 assumes) or refused?

**What the docs say.** I.8.4(b) and II.2.2: a constant-shaped range no front-end folds is taken as non-empty — the
"under" direction, recorded. D-29 (the user, 2026-11-09): a range *proven* non-empty by `ConstEnv` ticks on every
path; a range known only at run time stays possibly-empty. A const generic is neither: it is fixed per instance
(I.14.5(d)), unknown to the one parametric SV module the transpiler emits.

**What the code does.** R3 in both front-ends: `literal_range_is_empty` answers "not empty" for `0..W`
(`is_constant_expr` — a SCREAMING_CASE name); the macro emits no assertion (`constant_ticking_for_ranges` excludes
generics: a `const _` item cannot name an outer generic). So the macro admits, and simulation with `W = 0` would
livelock. The transpiler keeps the `for`'s empty-range exit (the guarded hoisted tick `if c != a { tick }`, or
`D`'s entry test), and `splice_zero_time_gotos` finds the zero-time cycle through it: refused with the flattener's
message. Pre-existing (measured on `e71f7d5` for every probe; zzi_04/32/34 there by the older `expand_counted_for`
decline). Corpus 0: no corpus module's R3 verdict depends on a generic range (`det_for`'s generic `for` is followed
by a tick; `counted_delay` uses a literal).

**Options.**
- **(a) Admit in both; check the hypothesis per instance.** The transpiler treats a const-generic range the way R3
  does — non-empty — wherever R3's verdict depends on it (the do-while `D`, no guard), and the hypothesis is
  checked where `W` gets its value: the macro emits an inline `const { assert!(W > 0, NESTED_EXIT_MESSAGE) }` in the
  body (rustc evaluates it at monomorphization, so a `W = 0` instance fails to build — not under `cargo check` or
  trybuild, which never instantiate), the transpiler an elaboration-time check in the parametric SV
  (`if (!(0 < W)) $error(..)` in a `generate`, so Verilator refuses `-GW=0`). Changes: two emissions and their pins;
  the parametric-delay idiom (`for _ in 0..N { tick }` as a generic module's only boundary) becomes legal. Corpus
  SV: none expected (only shapes refused today change).
- **(b) Refuse in both.** R3 stops assuming: a range with a const-generic bound counts as possibly empty, as a
  run-time one does, so every probe above is refused by both with R3's message. Changes: `literal_range_is_empty`
  and II.2.2's recorded direction for generics; corpus cost 0 (measured above); the parametric-delay idiom needs a
  tick after the `for`.
- **(c) A fourth named parity exception.** Keep both behaviours; witness-pinned as the three are. Not recommended:
  the macro's side livelocks at `W = 0` with no check anywhere.

**Recommendation: (a)** — it is D-29's rule applied where the constant is known, which for a generic is the
instance; it keeps a natural idiom, and both front-ends then refuse exactly the instance whose range is empty.
(b) is the smaller change if the per-instance checks are unwanted.

### AE.6 Ledger and counts

Before: **233 lines — 231 CLOSED, 2 OPEN** (`e71f7d5`). After (G-K in AE.7's run): **234 lines — 232 CLOSED, 2 OPEN; 171 required by the SPEC; fixtures: 8 coreui, 3 example, 13 gone, 339 sweep, 155 test, 129 ui; 350 run checks**. Closed:
`ticking-for-ends-in-loop`. Added OPEN before any fix: `ticking-for-generic-range` (`e4961c4`). II.1.3 stays OPEN,
now on `ticking-for-generic-range`.

### AE.7 The bare regression — measured

Before the code commit: `projection_relation`, `timing_model_derivations` (AE.1's three additions),
`front_end_parity_ui`, `front_end_parity`, `register_reconciliation`, `conformance_ledger` and the trybuild ui
suite, each passing. The first bare run (under `nohup`) failed at step 2 in 7 s: `nohup` leaves SIGPIPE ignored,
so the driver's `echo "$SV" | grep -q` got EPIPE under `pipefail` — the environment, not the tree; re-run with
SIGPIPE restored. **Bare `tools/regression.sh` on `c4fd345` (the final code commit; laptop; `paper` at
`e71f7d5`): REGRESSION OK (590 s).** 1788 passed, 0 failed, 16 ignored, 159 test binaries (G-C 132 test files);
examples **32/32**; G-D 382 differential cases (3 ignored-with-reason), G-F 382 (3 skipped with a reason) — the 8
new fixture modules; **G-G differential 3462 PASS / 12 OUTSIDE / 0 TRUNCATED / 0 EXCUSED / 0 DISAGREE**,
**reference 3474 PASS / 0 DISAGREE**; G-I 303 clocked related (98 extracted, 23 linear, 182 single-tick), 0
uncovered, 2236 transitions; G-J 90 const items compared / 25 outside both, 336 counted-`for` bounds compared,
9 asymmetric (pinned), 0 DISAGREE; G-E 5 anchored; **G-K `234 lines — 232 CLOSED, 2 OPEN; 171 required by the
SPEC; fixtures: 8 coreui, 3 example, 13 gone, 339 sweep, 155 test, 129 ui; 350 run checks`**, the 3 example
fixtures PASS. Not run remotely (nothing pushed). sv-baseline against `e71f7d5` on `c4fd345`: 12 of 450 differ —
8 new (`tfl_*`), 4 changed (`ntf_distinct`, `ntf_three`, `ntf_in_loop_while`, `ntf_then_for_then_wait`, AE.1).
After this run, docs only: III.3.6's counts, this section, CLAUDE.md.

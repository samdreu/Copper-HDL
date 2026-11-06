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

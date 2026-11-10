# Implementation-defined behavior inventory — 2026-11-03 baseline

> **Snapshot (pointer added 2026-11-09).** SPEC.md cites this file by item number, so it is kept
> as it was. Much of the code it inventories was deleted in Phase C (2026-11-06/07): the read
> barrier and the two-job executor (D-8), `spawn_child` (D-23), `ReadMode`, and the plain-`Out`
> emission forms (step 4). What the code decides today is SPEC.md's, and the dated changes are
> `MEASUREMENTS.md` §E–§AG.

Read-only inventory of every point where the implementation decides meaning, taken at
commit `ea54d26` from the code and its comments (not the design docs). Paths are
relative to the repository root. Abbreviations: EX=`copper-sim/src/executor.rs`,
SL=`copper-sim/src/lib.rs`, TY=`copper-core/src/types.rs`, PO=`copper-core/src/port.rs`,
ME=`copper-core/src/memory.rs`, MA=`copper-macros/src/lib.rs`,
AN=`copper-analysis/src/cfg.rs`, SH=`copper-codegen/src/shir_lower.rs`,
VL=`copper-codegen/src/vlir_lower.rs`, CX=`copper-codegen/src/control_extract.rs`,
EM=`copper-codegen/src/emit.rs`.

## Summary

**Counts per group:** (1) tick/phases 16, (2) wire semantics 10, (3) RegOut 7, (4) In
read and barrier 12, (5) macro injection 20, (6) Logic/Bits 32, (7) Memory 20, (8)
combinational evaluation 9, (9) multi-domain/synchronizer 12, (10) scheduling 18, (11)
codegen drive forms 28, (12) control extraction 16 — 200 behaviors, plus 36 that look
like semantic decisions with no stated rule (the last section).

**The most important unstated behaviors:**
- **What runs when.** A tick is ready only in the post-edge settle, and its target is
  fixed when `tick()` is called. So each `tick_clock` moves each task past exactly one
  tick, and post-tick code runs after the edge until the next tick or the next deferred
  read. The deferred read parks until the next pre-edge settle and pulls all following
  code into it.
- **The barrier is per read, not per loop.** It is placed only before reads classified
  Deferred (a tick follows within the iteration), except for passthrough writes in
  modules with no registers. Reads inside macro invocations are never deferred, and a
  classification mismatch silently becomes a plain read in release builds. Some code
  comments claim an end-of-loop barrier and a "wrap counter"; neither exists.
- **Levelized is the default scheduler.** It polls each task once per phase in
  topological order and iterates only real SCCs. It is only as correct as the `reads`
  and dirty handles declared at spawn: `spawn_untracked`, `spawn_child` or a missing
  read gives stale values with no diagnostic.
- **X rules.** `==` is structural (`X==X` is true, `X==1` is false). Converting X to bool
  panics. Any X in an arithmetic operand makes the result all-X, and division by zero
  gives X. Width conversions and index errors panic at run time.
- **Power-on mismatches.** A RegOut's sim initial is the wire initial, while codegen
  powers it (and every hold or registered port) on at 0. Memory `data()` panics before
  ready, while the SV stage registers read 0. `ReadMode::Async` has no behavior, and
  WriteFirst is never inferred despite the doc.
- **Multi-domain.** Phase flags are per domain, but every settle polls all tasks. A
  never-ticked domain defaults to "pre-edge, ticks resolvable", so its barriers open
  during other domains' settles. `exec.cycle` counts all domains together. The CDC
  check is skipped for multi-clock modules and for one-argument `In<T>`, and the
  synchronizer single-bit rule is not enforced.
- **Codegen form choice.** A drive is "opening" if no In read precedes it in its head or
  pre-tick segment, tracked per arm. Opening drives decide between the mux-plus-hold
  form and the edge-sampled flop, alongside continuous `assign` and the phase-guarded
  flop.
- **Possible bug in control extraction.** Pre-loop expression statements are moved into
  state 0, which the module loop re-enters every iteration. They appear to run every
  iteration in SV but once in the sim. A code comment also says the linear path drops
  them. (→ `MEASUREMENTS.md`.)

## (1) Clock tick and its phases — 16 items
1. `tick_clock::<D>` runs `pre_edge_settle::<D>` and then `post_edge_settle::<D>`. Nothing else happens between them except the edge. EX:813-829
2. Pre-edge sets D's phase to PreEdge, sets `TICK_RESOLVING[D]=false`, then calls `poll_tasks()` once. EX:836-840
3. Post-edge first calls `clk.advance()`, then sets PostEdge and `TICK_RESOLVING[D]=true`, polls, and finally increments `exec.cycle`. EX:845-853
4. `Clock::advance` increments the cycle under a mutex. It then calls `on_posedge` on every live listener in registration order and drops dead (Weak) ones. RegOut commit and memory pipeline shifts happen here, between the two settles. TY:886-900
5. `clk.tick()` fixes `target = cycle+1` when `tick()` is called, not on first poll. TY:913-921
6. `ClockTick` is Ready only if `cycle >= target` and `TICK_RESOLVING[D]` is true. So a tick can never resolve in a pre-edge settle. TY:997-1007
7. `TICK_RESOLVING` defaults to `true` for a domain that has never been set. TY:979-983
8. `POLL_PHASE` defaults to PreEdge for a domain that has never been set. SL:47-52
9. Consequence of 5-6: each `tick_clock` moves each task past exactly one tick. After the tick resolves in post-edge, the task runs on until its next `tick()` (target = new cycle+1, Pending) or its next Deferred-read barrier (Pending, because the phase is PostEdge).
10. Post-edge continuation: code after a tick runs in the same `tick_clock`, after the edge. TB observes after `tick_clock` returns. EX:814-821, EX:929-939 (counter reads 1, 2, 3)
11. Futures are not polled at spawn. A module's pre-loop code and its first segment run in the first `poll_tasks` call, normally the first pre-edge settle. EX:364-416
12. `exec.advance()` advances the clock and `exec.cycle` with no phase change and no polling. EX:805-808
13. `exec.cycle` is a single global counter, incremented once per `tick_clock` of any domain. EX:852
14. `poll_tasks()` can be called by the testbench outside `tick_clock`. It then runs under whatever phase and resolving flags were last set. EX:520
15. A task future that returns Ready panics with "malformed module". EX:563-569, 599-605, 767-773
16. The waker is a noop. Every scheduled task is polled unconditionally, with no wake tracking. EX:547, 754; TY:988-996

## (2) Wire value semantics — 10 items
1. `wire(initial)` creates one shared `Arc<Mutex<T>>`. The initial value is visible to `In::read` before any write, and dirty starts false. PO:135-148
2. `Out::write` takes effect immediately and is visible to every `In` clone. There is no delta buffering. PO:86-92
3. A write sets dirty only if `new != old`. Rewriting the same value is not a change. PO:88-91
4. Within a delta pass, the last write wins. A reader polled earlier in the same pass saw the earlier value (order-dependent under Fixpoint). PO:86-92, EX:765-781
5. `In::read` returns a clone of the current cell value, not a snapshot. PO:121-123
6. `Out` is not Clone, which gives a single driver. `In` is Clone, so readers are unlimited. PO:79-83, 114-118
7. All `DirtyHandle`s of one Out share one flag, so the first `take()` clears it for everyone. PO:46-48, 308-316
8. `WireId` is the cell's pointer address, and `Out`/`RegOut`/`In` on the same wire compare equal. PO:14-15, 103-105, 130-132, 213-215
9. Dirty flags are only consumed by the executor for handles passed at spawn. A testbench-held `Out`'s dirty flag is never consumed and has no effect. EX:570-572, 606, 774
10. Under Levelized, dirty flags of singleton tasks are taken and ignored, so they never cause a re-poll. EX:570-572

## (3) RegOut commit — 7 items
1. `RegOut::write` stores `Some(v)` into `pending`. The last write in a cycle wins, it sets no dirty, and it is not visible to readers. PO:194-196
2. At each posedge of the registering clock, `pending.take()` is committed to the observed cell. Dirty is set only if the committed value changed. PO:173-183
3. With no pending write the register holds. PO:181
4. A write in a pre-edge settle commits at that `tick_clock`'s edge. A write in a post-edge settle commits at the next `tick_clock`'s edge. This follows from EX:845-850 and PO:173.
5. The RegOut initial value is the `registered_wire(clock, initial)` argument. The listener is registered at construction, using the clock passed then. PO:220-235
6. The RegOut dirty handle is tagged `Registered`, so it makes no dependency-graph edge. It is set at the edge, so under Fixpoint the first post-edge pass sees it as dirty, causing an extra pass and a `consecutive_dirty` increment. PO:198-204, EX:665
7. Codegen gives a RegOut port a power-on value of 0 in `port_inits`, whatever the sim initial was. VL:519-531, EM:428-430

## (4) In read and the barrier — 12 items
1. In `sequential`/`synchronizer` modules, every `<inparam>.read()` is rewritten by a static Deferred/Immediate tag. MA:686-729, 832
2. Deferred becomes `{ pre_edge_barrier::<D>().await; p.read() }`. Immediate stays a plain `p.read()`. MA:718-725
3. Rule: a read is Deferred iff some path after it, within the same loop iteration, reaches a tick. The loop tail is treated as "no tick after". AN:3758-3763, 3828
4. In an `if` or `match`, a condition or scrutinee read is Deferred if any branch or arm ticks. A `while` condition is Deferred if its body ticks. Nested loop bodies are classified as fresh iterations. AN:3973-4016
5. Passthrough exception: if the whole module has no registers, a read inside `<comb Out>.write(...)` is always Immediate. AN:3880-3887, 3945-3955, 3815
6. Reads inside macro invocations are neither classified nor rewritten, so they are plain reads with no barrier. AN:3922-3925. `find_unprotectable_in_uses` also does not look inside macro tokens. MA:620-660
7. Tags are matched to reads by index. A count mismatch is only a `debug_assert`; in release, an unmatched read silently becomes Immediate. MA:712-725, 841-848
8. `PreEdgeBarrier<D>` is a fresh future each time. It is Ready on any poll while D is PreEdge (no park if reached in pre-edge) and Pending while D is PostEdge. SL:105-114
9. A Deferred read reached in post-edge parks until the next pre-edge settle. It then sees the value at the moment this task is polled in that settle: after the testbench writes between calls, and, under Levelized, after its producers in topological order. Code after it runs in that pre-edge.
10. The gating domain D is the port's declared domain if a `Clock<D>` parameter has it. Otherwise it is the module's only clock. A port whose domain is unticked with 0 or ≥2 clocks is a compile error. MA:802-827
11. Use restriction: an In param may only appear as `p.read()` with no arguments. A clone, rebinding or call argument is a compile error. This applies only to sequential and synchronizer modules. MA:620-660, 744-761
12. Combinational modules get no rewrite, so all their reads are plain. MA:1371-1384

## (5) What the macro injects, per mode — 20 items
1. Argument parsing strips spaces and splits on commas. The first token is the mode (`sequential|combinational|synchronizer|structural`); the only flag is `allow_pretick_alignment`. MA:50-80
2. Checks for all modes run in this order: validate_hardware_fn, no clock construction, no Option/`?`/panic family, `check_admissible`, no param shadowing. MA:906-939
3. validate: sequential-like must be async and combinational must not. No return value, no `self`, named params only, param types restricted to Clock/In/Out/RegOut/Memory, at least one Out or RegOut, sequential needs a Clock, combinational must not have one, structural needs a Clock. MA:1403-1521
4. Sequential-like: every `Clock` param must have a `name.tick().await` somewhere. There must be exactly one top-level `loop`, and it must be the final statement. MA:1523-1574
5. `TickAwaitVisitor` records any `ident.tick().await`, whether or not `ident` is a Clock param. Noted in the code as a latent gap. MA:516-541
6. Clock construction is rejected: any call path ending `Clock::new` or `Clock::default`. Cloning is allowed. MA:279-307
7. Option rejection covers `Some(..)`, the path `None`, `.unwrap`/`.unwrap_or`/`.expect` on anything, `Some`/`None` patterns, and the `Option` type. It also rejects `?` and `panic!`/`unreachable!`/`todo!`/`unimplemented!`. MA:414-513
8. Sequential only (not synchronizer): `check_cdc`. MA:946-950
9. Sequential and synchronizer, in order: reachability, nested-loop shape, awaits, register initializers, memory staging, memory result driving a plain Out. Then multi_write_collapse, then the flag-waivable rules: unprotected_pretick_out_write, pretick_out_write_before_update, trailing_regout_reads_input, trailing_out_reads_input, linear_out_write_anchor_mismatch, multi_phase_out_write, out_first_driven_late. MA:963-1350
10. multi_write_collapse cannot be waived by `allow_pretick_alignment`; the other seven rules can. MA:1026-1043 vs 1065-1350
11. After the checks, `inject_synced_reads`, then `wrap_in_module`. MA:1364-1369
12. No barrier is injected at the loop end, despite SL:116-118 saying the macro does this. Only per-read barriers exist. MA:717-726
13. MA:731-737 still describes a "hidden wrap counter/tracker"; the code has neither. MA:834-850
14. Combinational: `check_definite_assignment`, then the body becomes `loop { <body>; ::copper_sim::delta_yield().await; }`, made async, then `wrap_in_module`. MA:1378-1383, 853-868
15. `wrap_in_module` turns the function into `fn m(..) -> HardwareModule<impl Future<Output=()>> { HardwareModule::__new(async move { body }) }`. Ports are moved in, and nothing runs until polled. MA:878-889
16. Structural: the function is emitted unchanged, not wrapped, so it cannot be spawned. MA:1385-1398
17. Synchronizer differs from Sequential only in skipping `check_cdc`. MA:942-950
18. The "single-bit Logic only" restriction on synchronizers is stated in `src/sync.rs:9-12` but not enforced in the macro.
19. `HardwareModule::__new` is the only constructor, so spawning requires the macro. EX:48-64
20. `check_cdc` skips any module with ≠1 clock parameter. MA:204-210

## (6) Logic/Bits data semantics — 32 items
1. Logic has three values, `Zero=0, One=1, X`. There is no Z. TY:14-19
2. `Logic::as_bool(X)` panics, and so do `Into<bool>` and `if`-style conversion. TY:28-35, 51-55
3. `!X = X`. TY:57-67
4. AND: 0 dominates, `1&1=1`, everything else X. TY:69-79
5. OR: 1 dominates, `0|0=0`, everything else X. TY:87-97
6. XOR: any X gives X. TY:105-115
7. `==` on Logic and Bits is the derived structural equality: `X==X` is true, `X==One` is false, and the result is a bool, not Logic. TY:14, 144
8. Bits has no `PartialOrd`, so `<` is unavailable. Only the `*_logic` comparisons exist.
9. `eq/ne/lt/le/gt/ge_logic` return X if either operand has any X bit, even when other bits already decide the result. Comparisons are unsigned. TY:400-426
10. `ne_logic = !eq_logic`, so X stays X. TY:405
11. Bitwise ops are per-bit Logic ops. TY:626-672
12. `+ - *` return all-X if any operand bit is X. Otherwise they wrap mod 2^N. TY:676-719
13. `/` and `%` return all-X on any X or a zero divisor. TY:721-737
14. Unary `-` gives all-X on X, otherwise the two's complement. TY:739-746
15. Arithmetic goes through `as_u128`, which const-asserts N≤128, so arithmetic on `Bits<N>` with N>128 fails at monomorphization. Only `+` and `-` have `&Bits` versions. TY:281-284, 685-710
16. `<<` and `>>` take `usize`; vacated bits are 0. A shift ≥N gives all zeros with no panic. X bits move like any other bit. TY:332-348, 750-758
17. Arithmetic shift right fills with bits[N-1], which may be X. A shift ≥N gives all-sign. TY:388-395
18. Reductions: AND fold from One, OR fold from Zero, XOR fold from Zero. The NAND/NOR/XNOR forms are negations, so X stays X. TY:367-383
19. `mux(X, a, b)` keeps bits where `a==b` and makes the rest X. TY:494-506
20. Bits are LSB-first. `get`/`set` with i≥N panics. Slice indexing via Deref panics with Rust's message. TY:304-314, 762-769
21. `zero_extend`, `sign_extend` and `truncate` check the width at run time (assert panic), not at compile time. TY:431-454
22. `sign_extend` copies an X sign bit into the extension. TY:441-444
23. `part_select` panics if out of bounds. `concat` puts self in the MSBs and panics if OUT≠N+M. `replicate` panics if OUT%N≠0. All run-time. TY:459-488
24. `from_uN` / `as_uN` check widths at compile time (post-mono const). `as_*` panics on any X bit. TY:195-218, 261-301
25. `from_usize` panics if the value does not fit. TY:222-226
26. `from_lit::<V>` rejects too-narrow N at compile time. TY:247-251
27. `from_uint` silently zeroes bits ≥128 when N>128. TY:253-259
28. `From<u128> for Bits<N>` only compiles for N≥128 (through `from_u128`). TY:812-814
29. `Bits::default()` is zero. Logic has no Default impl. TY:509-513
30. `all_ones()` is 2^N-1. `x()` is all X. TY:173-180
31. Hex and octal formatting print `x`/`X` for any digit containing an X bit. TY:558-622
32. The `HasUnknown` docs say the executor substitutes X on oscillation. The executor panics instead. TY:771-777 vs EX:625-633

## (7) Memory — 20 items
1. Memory takes const generics R, W, READ_LAT=1, WRITE_LAT=1 and a domain D. ≥1 is documented but not enforced; `READ_LAT-1` with READ_LAT=0 would fail at run time. ME:18-35, 304, 311
2. `read_port::<I>()` and `write_port::<I>()` do not check I<R or I<W at compile time; an out-of-range I panics on access. ME:195-201, 297, 360
3. Contents: `new` fills with `T::default()`, `from_contents` takes a Vec, `from_fn` calls f(i). ME:229-251
4. Defaults are ReadFirst and `read_mode=Sync`. ME:261, 268
5. `ReadMode` has no setter and no behavioral effect; Async is never selectable. ME:12-16, 215-217
6. `write_first()` / `read_first()` set a shared atomic flag. The doc says the macro infers it; no macro code does. ME:322-335
7. The edge listener is registered at construction; the Memory holds the Arc and the clock holds a Weak. ME:263-264
8. `read(addr)` panics at the call if out of range. Otherwise it stages `pending_read_addr[I]`, so the last staging in a cycle wins. ME:290-298, 116-138
9. `write(addr, v)` panics if out of range. Otherwise it sets `write_pipeline[I][0]`, so the last write in a cycle per port wins. ME:353-361
10. On posedge, ReadFirst runs the read pipelines and then the write pipelines; WriteFirst runs writes first. ME:73-91
11. Write pipeline per port: commit stage WRITE_LAT-1 into `data`, then shift. With WRITE_LAT=1 the staged write commits at the very next edge. ME:94-114
12. Write ports commit in ascending index order, so the highest port wins on an address collision. ME:103
13. Read pipeline per port: shift, then capture `pending.take()` into stage 0. If nothing was staged, stage 0 becomes None. ME:140-160
14. The address must be re-presented every cycle, otherwise valid drains. ME:50-53, 156-158
15. `is_ready()` means the output stage is Some. Before any staging, and before READ_LAT edges have passed, it returns false. ME:303-305
16. `data()` panics (expect) when not ready. It does not return `T::default()`. ME:310-314
17. `WritePort::is_ready()` is always true. ME:365-367
18. Data reads take effect only at edges; `data()` is stable across a settle. With READ_LAT=1, data read in the post-edge of cycle N reflects the address staged before edge N.
19. Codegen's read and valid stage registers power on at 0, so SV `data` before ready is 0 while the sim panics. VL:2991-2994
20. Codegen: ReadFirst is a continuous array read; WriteFirst is a forwarding mux with the highest write port outermost, taken from the committing stage. Pipelines and commits are not phase-guarded. VL:3077-3112, 2904-2912, 3118-3120

## (8) Combinational module evaluation — 9 items
1. Surface Rust is `loop { body; delta_yield().await; }`. MA:853-868
2. DeltaYield is Pending on its first poll and Ready on the second, so exactly one body evaluation happens per executor poll after the first. SL:70-81
3. The first-ever poll runs the body once and parks.
4. Levelized: a combinational task is polled exactly once per settle, so the body runs once in pre-edge and once in post-edge per `tick_clock`, after its topological producers. EX:546-556
5. Inside a multi-task SCC it is re-polled until a full SCC pass sets no dirty flag. It runs once more after the last change, then stops. EX:588-635
6. Fixpoint: every task is polled every pass until a full pass produces no dirty flag, so each body runs k+1 times, with the last pass making no change. EX:757-790
7. A body that unconditionally writes a new value each evaluation oscillates and panics. SL:60-65
8. Definite assignment: a combinational Out must be written on all paths or on none, checked before wrapping. MA:1378-1380
9. Combinational modules have no CDC check and no In-use restriction. MA:942-950, 1371-1384

## (9) Multi-domain and synchronizer — 12 items
1. Phase and tick-resolving state are thread-local maps keyed by `TypeId::of::<D>()`. SL:33-45, TY:966-977
2. `tick_clock::<A>` polls all tasks, of every domain, but only changes A's flags. EX:520, 836-850
3. During A's settles, a B-domain task sees B's last state. After B was ticked that is PostEdge with resolving=true, so its barriers stay parked. B's ticks don't resolve, because B's cycle is unchanged.
4. A never-ticked domain defaults to PreEdge and resolving=true, so its barriers resolve during any other domain's settles. SL:49, TY:981
5. `exec.cycle` counts all domains together. EX:852
6. RegOut and Memory commit only on their own clock's `advance`. PO:231, ME:264
7. Sequential modules: if there is exactly one clock, In/Out/RegOut/Memory ports whose explicit domain differs from the clock's are rejected. MA:190-257
8. One-argument `In<T>` or `Out<T>` (domain `()`) counts as domain-agnostic and is accepted. MA:141-144, 171-175
9. Domain comparison is by the last path-segment identifier string, so a generic `Src` is foreign. MA:160-163, 1750-1761
10. A synchronizer may declare any foreign-domain port; its reads are gated by its single clock's domain. MA:946, 811-815
11. cdc.rs has no code. The type-level guarantees are pinned in trybuild UI tests: mismatched domains give E0308, a moved Out gives E0382, RegOut has no Clone (E0599). Gap 1 (forwarding a foreign value with no synchronizer) is closed by the macro, Gap 2 (clock construction) by MA:295. `copper-core/src/cdc.rs`:22-62, 197-226
12. `sync_2ff` writes q=ff2, ticks, then sets ff2=ff1 and ff1=d.read() (Immediate trailing read), with power-on 0. `src/sync.rs`:69-84

## (10) Scheduling — 18 items
1. The default scheduler is Levelized. The `COPPER_SCHEDULER` env var (fixpoint|levelized, case-insensitive) is read at `HardwareExecutor::new()`; other values are ignored. EX:111-141, 283
2. Graph edges: producer→consumer only when the producer's dirty handle is Comb and its WireId is in the consumer's declared `reads`. Edges are deduplicated and self-edges dropped. EX:657-687
3. If two tasks hold Comb handles to the same WireId, the later spawn silently overwrites the producer entry. EX:666
4. Tarjan SCC is iterative, then Kahn on the condensation with ties broken by a min-heap on each component's minimum task index. EX:152-210, 700-749
5. Members of an SCC are polled in ascending index order. EX:712-714, 597
6. The plan is rebuilt lazily on the first `poll_tasks` after any spawn. EX:639-645, 374, 415
7. A singleton component is polled once, with no oscillation check. EX:561-573
8. A multi-task SCC is iterated until clean. It panics when any member's `consecutive_dirty` ≥ 20, or at 1000 passes. EX:588-635
9. Levelized correctness depends on the declared `reads` and dirties being accurate. A missing read or dirty means no edge, possibly a consumer polled before its producer, and a stale value for that phase with no re-poll.
10. `spawn_untracked` and `spawn_child` register no dirties, so they have no out-edges. EX:364-375, 463
11. Fixpoint: polls every task in PollOrder each pass, ORs dirty flags, and stops after a pass with none. It resets every task's counter on convergence. EX:753-803
12. Fixpoint oscillation: panics if any task's `consecutive_dirty` ≥ 20 after a pass. The counter resets on any clean poll of that task. EX:776-801
13. Dirty that alternates among tasks never reaches the threshold and instead hits the 1000-pass limit. EX:757-761
14. PollOrder is Insertion, Reversed, or Seeded (SplitMix64, Fisher-Yates reseeded per delta). It affects Fixpoint only. EX:226-339, 303-309
15. `comb_cycles()` returns SCCs of size >1. It is static: it reports without rejecting, so convergent loops are legal. EX:490-495
16. Each settle phase is an independent `poll_tasks` call; nothing carries between pre and post except the counter reset. EX:839, 850
17. `iterate_scc` resets only SCC members' counters. EX:616-618
18. `spawn_child` names a child by `stringify!` of the future expression when no name is given. SL:131-133

## (11) Codegen per-drive form decision table — 28 items
Opening flag (SH):
1. `opening = forward_opening_drives && !seen_in_read`. It is true only in head or pre-tick segments, and only before any In-port read in the segment walk. SH:374-386
2. The mark is set before lowering a non-branch statement, so a drive whose own value reads an In counts as closing. SH:335-339
3. If/match arms start from the condition's (or scrutinee and guards') reads alone, so marking is per arm. After the branch, the whole statement's mark carries on. SH:399-404, 435-443, 519-521
4. Trailing segments and the combinational path never set opening. SH:665, 790, 220-230
5. A PortDrive carries `value`: committed (unforwarded) for closing drives and the forwarded form for opening ones. SH:374-378
6. It also carries `edge_value`, the forwarded value. For a trailing-segment RegOut, `edge_value` is the committed value instead. SH:363-367
7. If and Match carry both `condition` and `edge_condition`. A branch that only gates memory staging uses the edge condition for both. SH:405-431
8. Memory addresses and data always take the edge (forwarded) form. SH:497-511

Segments and phases:
9. Ticks split the loop body into N+1 segments, and N=0 is an error. SH:583-588, 875-885
10. A tick inside a branch is an error on the linear path. SH:926-958
11. Single tick: comb is trailing(seg1) placed before head(seg0), with head defs hoisted as needed. post_edge holds the seg0 and seg1 updates with a shared forwarding map. SH:642-709, 1078-1167
12. Multi-tick: seg_k maps to phase_k. The trailing segment's updates go to phase N-1, and its comb statements are prepended to phase 0. `phase_r` advances (k+1)%N and starts at 0. Promoted cross-phase wires become `<w>_r`, captured at the declaring phase's edge, with init being the literal or else 0. SH:710-845, 612-625

Emission forms (VL; conditions, then form):
13. Combinational module, top-level PortDrive with no nested drive of that port: module-level `assign`. VL:265-266, 738-741
14. Sequential, top-level PortDrive of a port in neither hold_outs nor in_comb: continuous `assign port = value`, using `value` and dropping `edge_value`. VL:738-741
15. hold_outs = RegOut ports plus phase-scoped ports. A phase-scoped port is a plain Out written in some phases but not all, whose value reads a phase-local wire. Its drives stay a PortAssign in the phase. VL:329-333, 1530-1566, 725-734
16. Single phase, port also driven inside an if/match/for: the unconditional drive stays in always_comb in program order. VL:369-372
17. cond_outs = ports driven on some but not all paths of the phase (after hoisting Moore defaults), plus hold_outs. VL:404-405, 1753-1756
18. Moore hoist: a currently-conditional port driven on all paths of every explicit arm of a defaultless case (and of the default, if present) gets a top-of-block default copied from its first direct arm. An empty `default:` is added. VL:1781-1857
19. Multi-phase, port driven on every path through if/match: it becomes a candidate for the hold form; if none of its drives are opening, it gets the edge-sampled flop. VL:438-453
20. Mux+hold form: the port is in cond_outs or every-path candidates, not in hold_outs, has ≥1 opening drive, and no drive inside a `for`. A `<port>_hold` register (init 0) is created; the default `port = <port>_hold` goes at the top of always_comb (multi-phase, outside guards) or at the phase head (single phase). Opening drives stay combinational under their guards. Closing drives are removed from comb. Every drive also loads `_hold <= edge_value` under edge conditions, phase-guarded when multi-phase. VL:448-517, 1579-1745
21. Edge-sampled flop form (implicit hold): remaining cond_outs are moved to always_ff as `port <= edge_value` under edge conditions, with an empty case default meaning hold; phase-guarded when multi-phase. The port gets a port_init of 0. VL:519-541, 2107-2203
22. RegOut: always in hold_outs, so never assign or hold form. It gets the flop with `edge_value`, or the committed value when trailing. VL:138-143, 1586
23. `split_output_reg` does not descend into ForLoop, so a conditional drive inside a `for` stays combinational. VL:2201
24. Latch check: any signal assigned on some paths but not all is an error, except those in comb_defaults or the hold registers. Registers and RegOuts are exempt from branch-local default hoisting. VL:552-559, 2384-2395, 2273-2377
25. A port with continuous assigns from more than one phase is a MultiplyDrivenOutput error. VL:581-586
26. Multi-phase: a plain Out driven from memory data or valid is an error. VL:1465-1488
27. Register inits are resized to the register's width; a register without an init gets no `initial` line (X power-on, e.g. `Bits::x()`). VL:291-302, EM:418-432
28. Multi-phase: every declared comb wire is defaulted to `'0` at the top of always_comb. EM:248-253

## (12) Control extraction cycle accounting — 16 items
1. Fires only when a tick sits inside a top-level statement that is not itself a tick, or the body has a `continue` for the module loop. CX:1577-1581
2. Declines (falls back to the linear path) when an unflattenable construct is found, when a zero-time goto cycle is found, or when no tick is found. CX:520-522, 561-563, 598-600
3. A `while` containing a tick becomes `loop { if !cond { break; } body }`. A tick-free `while` is untouched. CX:113-177
4. Output shape: `let mut pc: uN = 0; loop { match pc { 0.., k.., _ => {} } clk.tick().await; }`. pc is u8, u16 or u32 depending on the state count; its final name is `pc` or `pc_N` to avoid collisions. CX:567-662, 75-81
5. Only a tick advances pc. A non-final tick allocates a new state and emits `pc = next`; the rest of the segment becomes that state. CX:1210-1223
6. A trailing tick emits `pc = ctx.head` with no extra state. CX:1213-1215
7. `break` inlines the enclosing loop's continuation in the same cycle, lowered once per loop and then cached. CX:1236-1255, 1133-1152
8. `continue` emits a zero-time goto to the head, which is later spliced with the head state's body (same cycle). CX:1226-1233, 777-829
9. A nested loop with a top-level tick at t gets a head state containing the rotated `body[t+1..] ; body[..t]`. Entry runs `body[..t]` in the current cycle. CX:1276-1288
10. A nested loop with ticks only in branches: its lowered body is cloned into the entry and is also the head state. CX:1289-1300
11. An if or match that diverges (contains a tick, break or continue) gets the continuation inlined into every arm. A tick-free arm runs that continuation in the same cycle. CX:1305-1345
12. Fall-through at the end of a list: under AfterTick (a nested loop whose trailing tick was rotated away) it emits `pc = head` and spends a cycle. Under ZeroTime (the module loop) it emits a goto to the head and spends none. CX:1351-1357, 1162-1197
13. A local defined in one state and mentioned in another is hoisted to a pre-loop `let mut` (becoming a register). CX:914-960
14. Pre-loop expression statements are moved into state 0's body. CX:526-549
15. The match has a wildcard `_ => {}` arm: an unreachable pc does nothing and leaves pc unchanged. CX:577-582
16. The pc register starts at 0. CX:618-636

## Behaviors that look like semantic decisions with no stated rule
1. A comparison involving X through `==`: `X==X` is true and `X==One` is false (derived PartialEq, a bool result). Only `eq_logic` gives X. TY:14, 144, 400
2. `if` or a bool conversion on X panics. TY:33
3. Any X in an arithmetic operand makes the whole result X. Division or remainder by zero gives all-X rather than panicking. TY:679, 725, 734
4. Width conversions (`zero_extend`, `sign_extend`, `truncate`, `concat`, `replicate`, `part_select`) fail at run time, not compile time. TY:432-482
5. An index out of range panics, both bit index and memory address. TY:305, ME:130-138
6. A shift by ≥N gives zero (or all-sign for arithmetic shift) with no error. TY:332-395
7. Arithmetic on N>128 does not compile; `from_uint` silently drops bits ≥128. TY:253-284
8. A RegOut's sim initial is the wire initial; its SV power-on is always 0. The same holds for `_hold` registers and every port_init. PO:224, VL:468-470, 522-531
9. Memory `data()` before ready panics, while SV stages read back as 0. ME:310-314, VL:2991
10. `ReadMode::Async` has no behavior, and WriteFirst is never inferred despite the doc. ME:215, 322-329
11. A read address not re-presented makes valid drop after READ_LAT edges. ME:156-158
12. Simultaneous writes to one address: the highest port index wins. ME:103
13. A combinational module's body runs exactly once per phase under Levelized. Under Fixpoint it runs until a full pass is clean, plus once more.
14. The barrier exists only before Deferred reads, not at the loop end, contrary to SL:116-118. MA:717-726
15. A read inside a macro invocation is never deferred. AN:3922-3925
16. A classification mismatch silently becomes Immediate in release builds. MA:724, 841
17. Under Levelized, a missing `reads` or dirty declaration silently produces stale values, with no diagnostic. EX:657-687
18. With duplicate Comb producers on one WireId, the last spawn wins in the graph map. EX:666
19. A never-ticked domain's barriers and ticks are "open" during other domains' settles (defaults PreEdge and resolving=true). SL:49, TY:981
20. `exec.cycle` counts across all domains. EX:852
21. A tick's target is fixed when `tick()` is called, not when it is first polled. TY:915
22. Listener commit order follows registration order (construction order). TY:891-899
23. `HasUnknown` claims the executor writes X on an oscillating signal; it actually panics. TY:771-777
24. Oscillation needs 20 consecutive dirty passes per task. Dirty alternating between tasks runs to 1000 passes. EX:20-23
25. RegOut dirty set at the edge forces an extra Fixpoint pass but does nothing under Levelized. PO:178
26. Domain-agnostic `In<T>`/`Out<T>` bypass CDC, multi-clock sequential modules skip CDC entirely, and the synchronizer single-bit rule is unenforced. MA:204-210, 171-175; `src/sync.rs`:9-12
27. `TickAwaitVisitor` accepts `x.tick().await` on any identifier, not only Clock params. MA:530-536
28. `multi_write_collapse` cannot be waived; the other pre-tick rules can. MA:1026 vs 1065
29. **Possible bug:** control extraction moves pre-loop expression statements into state 0. The module loop's back edge (trailing tick, `pc=0`, or the ZeroTime goto to 0) re-enters state 0, so they appear to run every iteration in SV while the sim runs them once. CX:526-549, 1215, 1356. CX:530-531 also says the linear path's CHIR scan "does not lower" pre-loop expression statements, so they may be dropped there.
30. An unreachable pc value hits `_ => {}` and stays stuck. CX:577-582
31. A conditional drive inside a `for` is excluded from the hold form and not split to always_ff. VL:1590, 2201
32. A trailing segment's comb statements belong to phase 0, placed before the head; the single-tick path uses the same order. SH:648-683, 767-836
33. A promoted cross-phase wire with no literal init powers on at 0. SH:612-625
34. A register initialized with `Bits::x()` gets no `initial` line, so it powers on as X in SV. VL:300, EM:419
35. The Verilator profile wraps always_ff in `lint_off`/`lint_on MULTIDRIVEN` whenever any init or preload exists. EM:200-203, 491-504
36. Stale docs: MA:731-737 (wrap counter/tracker); EX:362, 399, 426 ("phase 1 records only, no scheduling effect") contradict that Levelized is the default and uses `reads`.

# Audit of D-1 … D-7 against `SPEC_OVERVIEW.md` (2026-11-04)

Each sub-decision is sorted into one of four kinds:

- **consequence** — follows from the one page (§1 the coroutine, §5 the principles) and
  the listed choices; keep, no further decision needed;
- **choice** — a genuine design choice; it must appear in `SPEC_OVERVIEW.md` §4 (it does,
  with its number) and depends on Q1 where marked;
- **accident** — the decision ratified what the implementation happened to do, with no
  reason on the one page; re-examine when its section is reached, or with Q1;
- **bookkeeping** — vocabulary, routing to another section, or a format rule; harmless.

| Decision | Sub-decision | Kind | Note |
|---|---|---|---|
| D-1 | A: two instants per cycle | **choice 1** | the two-job body; **depends on Q1** |
| D-1 | A: drive / tick / observe environment, drives only between an observation and the next tick | consequence | the protocol has no other place for a drive to land |
| D-1 | A1: `obs_0` in the trace | consequence | the first resumption exists and writes wires; hiding its result was the **accident** the harness still has |
| D-1 | A2: per-domain edge numbering | consequence | clocks are per domain |
| D-1 | B refused (one instant, every output registered) | — | this is Prost's one job; refused on the passthrough anchor that D-4 later removed — **re-opened as Q1** |
| D-2 | A: the kernel is the desugared surface, the simulator runs it | consequence | "the coroutine is the object" and "the sim is plain Rust" leave no other object |
| D-2 | A1: IN / DESUGARED / ERASED / OUTSIDE / NOT-YET | bookkeeping | |
| D-2 | A2: types deferred to I.14 | bookkeeping | |
| D-2 | A3: one kernel, three modes | consequence | |
| D-2 | A4: `s_pre ; loop { s }` | consequence | shape principle |
| D-2 | OUTSIDE rows (`Option`, `?`, panics, `Clock::new`, other `await`s, tickless loops, `return`) | consequence | shape principle, all four clauses |
| D-3 | A: segments defined on executions | consequence | a segment is what one cycle's two resumptions run |
| D-3 | A1: the instant is per execution path | consequence | the barrier is a suspension; whether it parks is where the resumption reaches it |
| D-3 | A2: "commit edge" out of Part I | bookkeeping | |
| D-3 | A3: "two iterations suffice" to III | bookkeeping | |
| D-3 | A4: zero-iteration loop to I.8 | bookkeeping | |
| D-3 | trailing region first, C then A one segment | consequence | the post-edge resumption runs from the tick through the back edge |
| D-4 | A: a read is by position (the wire at the instant) | consequence | a resumption reads the wire when it runs |
| D-4 | A1: a site is deferred iff some path reaches a tick before the back edge | **choice 2** | the barrier placement rule *is* the two-job mechanism; **decided with Q1**, not inherited from `classify_reads` |
| D-4 | A2-i: no passthrough exception; register-free passthrough refused | **choice 4** (one spelling) | the exception was an **accident** of the executor (D2 fix 2026-08-21) that D-4 removed; the refusal is a design choice, admitting it as documented one-cycle-late was possible |
| D-4 | cycle 0 reads see `x_1` | consequence | first resumption is pre-edge |
| D-4 | index-matched tags with release fallback; reads in macro tokens never deferred | **accident** | flagged for Phase C (hard error; II.1 refusal) — correct outcome, keep |
| D-5 | A: last write at or before, sampled once at `obs_N` | consequence | a wire; one observation per cycle |
| D-5 | A1: hold in every position | **choice 3** | Chisel's last-connect with no default; Esterel/Spade would reset — anchored at one position (`bsg_dff_en`), extended by decision |
| D-5 | A2-i: the dead closing write refused | **choice 4** | every effect observable; admitting it as dead code was possible |
| D-5 | A3-i: `init(o)` zero, `wire(zero)` a precondition | **choice 6** | the simulator's initial being the wire's argument was an **accident** turned into a precondition |
| D-5 | A4: one meaning, five forms | consequence | realizability principle |
| D-5 | A5-i: a plain-`Out` rule cites a clause or is a dated II.3 limitation | consequence | the realizability principle, stated for the first time here |
| D-5 | A6: `Out` wire / `RegOut` register port / combinational wire | consequence | |
| D-6 | A: a `RegOut` write at any instant takes effect at edge N+1 | consequence of **choice 5** | the port kind is the choice; the edge follows from "pending value commits at the edge step" |
| D-6 | A1: last write before one edge wins | consequence | |
| D-6 | A2-i: `init(q)` zero | **choice 6** | as D-5 A3-i |
| D-6 | A3: cycle-0 write in `obs_1` | consequence | |
| D-6 | A4-i: input-sampling refusals are II.3 limitations with a carrier fix | consequence | realizability principle; the shapes have a meaning under choice 2 |
| D-6 | A5-i, A6, A7 | bookkeeping / consequence | |
| D-7 | A: coroutine-as-definition, two suspension kinds, H(M) by tick site | consequence | this *is* §1 of the one page |
| D-7 | A1-i: register = live at a tick suspension; carriers separate | **choice 7** | the alternative (liveness with the back-edge clause as the definition) ratified `infer_registers`; the chosen one defines from the coroutine |
| D-7 | A2: wires' values carried in I.3.3 | consequence | H(M) needs them (hold) |
| D-7 | per-domain gating, never-ticked default (deferred to I.12) | **accident**, resolved by D-17 | decided from §1 in I.12.2 (2026-11-05): a coroutine is resumed by its own domain's edges only and has executed nothing before its first; the thread-local defaults are realization details, and the any-domain first settle they produce is a measured sim ≠ SV startup divergence owed to Phase C |
| D-7 | "a fixpoint re-poll runs no statement", `poll_tasks` outside a tick | **accident**, resolved by D-18 | SPEC I.13.2(b) fixes the order per cycle (drive, settle, resumptions, edge, settle, observation); a re-poll is an evaluation of a function (I.13.3) and runs no kernel statement |

## What the audit says

- Seven choices carry the language; everything else is consequence or bookkeeping. The
  seven are on the one page.
- Choices 1 and 2 are one choice seen twice: the two-job body and its mechanism. D-1
  refused the one-job alternative on an anchor (the passthrough) that D-4 then removed, so
  the refusal no longer has its reason. That is **Q1**, and it is decided before any
  further clause.
- Four accidents were ratified and then corrected by later decisions (`obs_0` hidden, the
  passthrough exception, the release-mode fallback, the simulator's initial values); two
  remain pending (per-domain defaults, executor re-poll facts) and must be decided from §1
  when I.12 and III.2 are written, not transcribed.
- No decision needs reversing today. If Q1 is answered "one job", D-1 A, D-4 A1 and A2-i,
  D-5 A1 and the II.3 list are re-derived; the rest stands.

## After D-8 (2026-11-04): the one-job body

Q1 was decided B (D-8). The table above is read against it as follows.

| Row | Status after D-8 |
|---|---|
| D-1 A: two instants per cycle (choice 1) | **reversed** — one instant per cycle, `t_{N+1}⁻` |
| D-1 A: drive / tick / observe environment; A1 `obs_0`; A2 per-domain numbering | stand (consequences) |
| D-1 B refused (one instant, every output registered) | the refusal is withdrawn: B is D-8's model |
| D-2, all rows | stand; `Out<T, D>` is a `combinational` parameter only (I.2.2), `allow_pretick_alignment` retired — a note, not a reversal |
| D-3 A: segments on executions; trailing region first, C then A one segment | stand — a segment is now one resumption |
| D-3 A1: the instant per execution path | **vacuous** (one instant); a note |
| D-3 A2–A4 | stand (bookkeeping) |
| D-4 A: a read is by position | stands, with one position |
| D-4 A1: deferred iff a path reaches a tick (choice 2) | **reversed** — no site is deferred, no barrier |
| D-4 A2-i: the register-free passthrough refused (choice 4) | **reversed as vacuous** — a plain `Out` is not a sequential parameter |
| D-4 cycle 0 reads see `x_1` | stands |
| D-4 index-matched tags, release fallback (accident) | **closed** — the injection goes entirely |
| D-5 A, A1 (choice 3), A2-i (choice 4), A3-i (choice 6), A4, A5-i, A6 | **reversed as a whole** — the plain `Out` leaves the sequential module; last-write-wins and `init(o)` = zero move to I.11; hold is D-6's |
| D-6 A (choice 5), A1, A2-i (choice 6), A3, A6, A7 | stand — D-6 is the output rule; "any instant" reads "the one instant" |
| D-6 A4-i: input-sampling refusals as II.3 limitations | **vacuous** — the trailing read sees `x_{N+1}`, the flop's sample; rule 11 retired |
| D-6 A5-i: the `e19e`/`e19l`/`e19k` II.3 rows | **vacuous** — registers of I.7 and the one forwarded phase; re-measured in Phase C |
| D-7 A: coroutine-as-definition, H(M) by tick site | stands |
| D-7 A: two suspension kinds | **reversed** — one kind, the `tick` site |
| D-7 A1-i: register = live at a tick suspension; carriers separate (choice 7) | stands; the carrier set is empty (barrier carrier, `_hold`, the input carrier all gone; `ff2` a wire of one resumption) |
| D-7 A2: wires' values in I.3.3 | **reversed with D-5** — the wires leave I.3.3 and H(M) |
| D-7 per-domain gating, never-ticked default (accident, pending) | **resolved** — D-17 (2026-11-05), I.12.2: decided from §1, own-domain resumption only; the any-domain first settle measured then is gone since Phase C (`copper-sim/src/executor.rs`: `tick_clock::<D>` resumes only `D`'s coroutines) |
| D-7 "a fixpoint re-poll runs no statement", `poll_tasks` outside a tick (accident, pending) | **resolved** — D-18 (2026-11-05): I.13.2(b) states the per-cycle order and I.13.3 makes the evaluation count unobservable; `poll_tasks` outside a tick is a III.2 precondition |

What changes in the reading of the one page: choices 1–3 of §4 collapse into one (one
instant, one job, `RegOut` hold), which is what the audit predicted ("choices 1 and 2 are
one choice seen twice"); the realizability principle names one resumption; §6's lowering
list is empty. The two pending accidents were then as before; both were resolved later (D-17, D-18, rows above). No further decision is
reversed by D-8; D-2, D-3 and D-6 carry notes only.

D-15 (2026-11-05, packet 15): §I.0–§I.6 consolidated under D-8 — no row of either table changes; the two-job text the "After D-8" table reads against now lives only in the decision log's D-1…D-8 entries and the packet-15 snapshot, and SPEC §I.5 is a forwarding stub whose table routes each I.5 clause to its surviving content.

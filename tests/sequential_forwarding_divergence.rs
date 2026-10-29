//! **D1: guarded (narrowed 2026-08-26). D2: fixed.** Two silent sim ≠
//! synthesized-SV divergences, both from the pre-edge/post-edge alignment of the
//! coroutine's loop segments. Every assertion here records *today's* behaviour so
//! it flips loudly when either changes — do not "fix" this file by relaxing it.
//!
//! **Plan of record: `design_docs/PRETICK_ALIGNMENT_GUARDRAIL.md`** — the variant
//! map, the hardware adjudication, the corpus status table, the rejected fixes,
//! the guardrail acceptance criteria, and the phased plan. Read
//! `SYNCHRONOUS_SEMANTICS.md` §Output timing for context; this is a third member of
//! the same family as the multi-write-around-a-tick collapse.
//!
//! **What changed after the sections below were written (2026-08-26, cycle-dataflow
//! phases B and D — `design_docs/PAIRED_IMPLEMENTATION_SCOPE.md`).** The transpiler
//! now emits a *forwarded* continuous assign for an opening-prefix drive (the D1
//! shape with no `In` read reaching the write), which gives the SV the meaning the
//! simulator always had — so the bare V1 shape, V5 and V7 AGREE today
//! (`hazard_v1_assign_then_write_dissolved`, `pre_tick_update_forwarding_agrees_end_to_end`,
//! `d_narrowing_battery_verdicts`), and the rule was narrowed to the READ-PRECEDED,
//! path-dependent class (W4, `probe_fsm`) plus the constant-write clause
//! (`pc_arm_*`, `branch_merge_explicit`). The independent `ref_fast_counter`
//! reference anchors the *corrected spelling* the real designs migrated to (update
//! after the tick), which is what "the simulator is wrong" below adjudicated. The
//! "49 clocked modules / 8" figures are the 2026-08-21 sweep, kept as history; the
//! current exact sets live in `copper-analysis/tests/pretick_alignment_corpus.rs`.
//!
//! **2026-10-26, arm-granular opening (`e9`).** Codegen's opening-prefix walk
//! (`shir_lower::lower_stmt_list`) now seeds each arm of an `if`/`match` from the
//! reads of its condition alone, instead of marking the whole statement — under
//! control extraction the whole body is one `match pc`, so one input read anywhere
//! had demoted every conditional plain-`Out` drive to the edge-sampled form. Four
//! pinned one-cycle lags flipped to agreement on the first run and are re-blessed
//! below: `e9`, `e2x`, and the D1 constant-write clause's own witnesses `pc_arm_write`
//! / `pc_arm_toggle` (and the `#[ignore]`d `probe_fsm_sim_matches_verilog`, the W4
//! witness, passes). The *rules* were unchanged then except `multi_phase_out_write`,
//! whose "input read anywhere" clause existed only for the statement-granular walk.
//!
//! **2026-10-28, D1 re-derived (`Cfg::unprotected_pretick_out_write`,
//! `CYCLE_DATAFLOW_SEMANTICS.md` §10).** The `d*` battery at the end of this file
//! derived and measured every head-segment shape the two-anchor model
//! distinguishes. The rule now refuses exactly two hazards — (A) a port with MIXED
//! anchors (a read reaches some drive while some drive has a read-free path;
//! unobservable only for a single every-path drive that forwards no register it
//! reads) and (B) an opening drive under a guard that tests a register assigned
//! earlier on the path (the comb copy tests the committed value). The old
//! constant-write clause and its "unprotected register assignment" conjunct are
//! gone: `pc_arm_write`, `pc_arm_toggle` and `probe_fsm` compile without an opt-out,
//! `w4_mixed_alignment` stays refused, and four measured divergences the old rule
//! missed (`d1g`, `d2a`, `d2b`, `d4c`) are refused now.
//!
//! # Divergence 1 — a pre-tick register update is forwarded to a same-segment read
//!
//! ```text
//! loop { r = r + 1; o.write(r); clk.tick().await; }
//!   sim = [2, 3, 4, 5, 6, 7]
//!   sv  = [1, 2, 3, 4, 5, 6]     ← off by one, silently
//! ```
//!
//! The simulator's *sequential forwarding* makes the pre-tick assignment visible to
//! the write that follows it in the same segment. Codegen emits `r <= r + 1`
//! (non-blocking) with `assign o = r`, so the SV drives the *pre*-update value — a
//! flip-flop's Q cannot reflect its own D within the cycle.
//!
//! **The discriminator is a leading `In` read.** A leading read is classified
//! `Deferred` (impl-plan item 3) and injects `pre_edge_barrier()`, which pins the
//! pre-tick segment to the pre-edge phase. With no input read at all there is no
//! barrier, the loop top also runs during the previous tick's post-edge settle, and
//! the update lands an extra time. `add_with_leading_read` below is the identical
//! design plus one input read, and it agrees with its SV exactly.
//!
//! That is why the corpus is green: a sweep of 49 clocked modules found 8 with the
//! update-then-read shape, and the 6 that are equivalence-tested (`lfsr`,
//! `det_110101`, `shift_register`) all gate on input reads. Only `fast_counter`
//! (3 copies) has the shape *without* one.
//!
//! **Adjudicated against independent hardware, not by argument.**
//! `examples/cdc/sv/two_domain_hierarchy.sv::ref_fast_counter` is a hand-written
//! reference committed in `0d67f9e` (item 4, 2026-07-30) — long before this
//! divergence was known, so it is a genuine outside opinion. It agrees with the
//! transpiled SV and disagrees with the simulator: **the simulator is the one that
//! is wrong.**
//!
//! # Divergence 2 — FIXED 2026-08-21
//!
//! `loop { out.write(inp.read()); clk.tick().await; }` transpiles to
//! `assign out = inp;` — zero cycles. Its leading read used to classify `Deferred`,
//! so it sampled at the *pre*-edge while a clocked producer updates at the
//! *post*-edge, and the passthrough lagged one cycle. Adjudicated against
//! independent hand-written Verilog (a clocked producer feeding a passthrough gives
//! `mid == out`), then fixed in `classify_reads`: a read feeding a combinational
//! `Out` in a segment that assigns no register is `Immediate`, because there is no
//! register for a `pre_edge_barrier` to pin. `d2_is_fixed_and_d1_still_demonstrates_the_hazard`
//! guards the fix.
//!
//! # Why nothing caught either one (historical)
//!
//! `tests/two_domain_hierarchy_cdc.rs` — the independent-hardware anchor for the
//! whole dual-clock design — used to be green because these two divergences
//! **cancelled**: D1 made the flag assert a cycle early, D2 made the consumer a cycle
//! late, and the observable boundary landed on the reference's cycle. It took a
//! deliberate experiment (correct one side, watch the boundary move 5 → 6) to see it.
//! Both are resolved now and that anchor passes for the right reason; D1's fixture
//! here keeps the shape expressible via `allow_pretick_alignment` so the hazard stays
//! demonstrable.

mod common;
use common::{verilator_available, verilator_command};
use copper::sync_2ff;
use copper_core::port::{wire, In, Out, RegOut};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;
use copper_sim::HardwareExecutor;
use std::path::Path;

/// Per-invocation nonce for temporary directories. Two tests in one binary can
/// transpile or Verilate the same top module at the same moment, and a directory
/// keyed on the process id alone is then shared: one test's cleanup deletes the
/// file the other's Verilator is about to read (seen on a 96-core host,
/// 2026-09-03). Same rule as the Verilator work dir in CLAUDE.md.
static TMP_NONCE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct C;
impl ClockDomain for C {}
struct ClkFast;
impl ClockDomain for ClkFast {}
struct ClkSlow;
impl ClockDomain for ClkSlow {}

const CYCLES: usize = 13;

// ── Divergence 1: minimal case ────────────────────────────────────────────────

const ADD_SRC: &str = r#"
#[hardware(sequential)]
async fn add_then_write(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = r + Bits::from_lit::<1>();
        o.write(r);
        clk.tick().await;
    }
}
"#;

#[hardware(sequential)]
async fn add_then_write(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = r + Bits::from_lit::<1>();
        o.write(r);
        clk.tick().await;
    }
}

/// The *same* design plus one leading `In` read, which installs the pre-edge
/// barrier. Held identical in every other respect so the read is the only variable.
const ADD_READ_SRC: &str = r#"
#[hardware(sequential)]
async fn add_with_leading_read(clk: Clock<C>, en: In<Logic, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        if en.read() == Logic::One {
            r = r + Bits::from_lit::<1>();
        }
        o.write(r);
        clk.tick().await;
    }
}
"#;

#[hardware(sequential)]
async fn add_with_leading_read(clk: Clock<C>, en: In<Logic, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        if en.read() == Logic::One {
            r = r + Bits::from_lit::<1>();
        }
        o.write(r);
        clk.tick().await;
    }
}

fn sim_add_then_write() -> Vec<u8> {
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (p, obs) = wire::<Bits<8>, C>(Bits::zero());
    let dh = p.dirty_handle();
    exec.spawn_wired(add_then_write(clk.clone(), p), vec![dh], vec![]);
    (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            obs.read().as_u128() as u8
        })
        .collect()
}

fn sim_add_with_leading_read() -> Vec<u8> {
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (en_drv, en_in) = wire::<Logic, C>(Logic::One);
    let (p, obs) = wire::<Bits<8>, C>(Bits::zero());
    let dh = p.dirty_handle();
    let reads = vec![en_in.wire_id()];
    exec.spawn_wired(add_with_leading_read(clk.clone(), en_in, p), vec![dh], reads);
    en_drv.write(Logic::One);
    (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            obs.read().as_u128() as u8
        })
        .collect()
}

// ── Verilator plumbing ────────────────────────────────────────────────────────

/// Verilate `sv` (already written to `sv_path`) as `top`, tick `clk_port`, and
/// collect the whitespace-separated values each `probe` line prints per cycle.
fn run_sv(sv_path: &Path, top: &str, clk_port: &str, probes: &[&str], extra_init: &str) -> Vec<Vec<u32>> {
    // Unique PER INVOCATION, not per top module: two tests in this binary
    // Verilate the same top in parallel (`trailing_update` is built by both the
    // d1-trailing gap test and the m2 model-lowering test), and a shared
    // directory lets them clobber each other's build — the false-PASS/false-FAIL
    // mechanism the repo's obj_dir convention exists to prevent (see CLAUDE.md).
    static NONCE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let work =
        std::env::temp_dir().join(format!("copper_fwd_{top}_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();

    let mut tb = format!(
        "#include \"V{top}.h\"\n#include \"verilated.h\"\n#include <iostream>\n\
         int main(int c, char** v) {{ Verilated::commandArgs(c, v);\n\
         V{top}* t = new V{top}(); t->{clk_port} = 0; {extra_init} t->eval();\n"
    );
    let body: String = probes.iter().map(|p| format!("std::cout << (int)t->{p} << \" \";")).collect();
    for _ in 0..CYCLES {
        tb.push_str(&format!(
            "t->{clk_port}=0; t->eval(); t->{clk_port}=1; t->eval(); {body} std::cout << std::endl;\n"
        ));
    }
    tb.push_str("return 0; }\n");
    let tb_path = work.join("tb.cpp");
    std::fs::write(&tb_path, tb).unwrap();

    let out = verilator_command()
        .current_dir(&work)
        .args([
            "--cc", "--exe", "--build", "--top-module", top,
            "-Wno-DECLFILENAME", "-Wno-WIDTHEXPAND", "-CFLAGS", "-std=c++14",
        ])
        .arg(std::fs::canonicalize(sv_path).unwrap())
        .arg(&tb_path)
        .output()
        .unwrap();
    assert!(out.status.success(), "verilator build failed:\n{}", String::from_utf8_lossy(&out.stderr));

    let run = std::process::Command::new(work.join(format!("obj_dir/V{top}"))).output().unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&work);
    stdout
        .lines()
        .map(|l| l.split_whitespace().filter_map(|x| x.parse().ok()).collect())
        .collect()
}

/// Transpile `src`'s `top` to a temp `.sv` and run it, returning one column.
fn transpile_and_run(src: &str, top: &str, clk: &str, probe: &str, init: &str) -> Vec<u8> {
    let sv = copper_codegen::transpile_source(src, Some(top), &copper_codegen::EmitConfig::default())
        .expect("transpile");
    let dir = std::env::temp_dir().join(format!("copper_fwd_src_{top}_{}_{}", std::process::id(), TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{top}.sv"));
    std::fs::write(&p, &sv).unwrap();
    let rows = run_sv(&p, top, clk, &[probe], init);
    let _ = std::fs::remove_dir_all(&dir);
    rows.into_iter().map(|r| r[0] as u8).collect()
}

// ── Divergence 1 ──────────────────────────────────────────────────────────────

#[test]
fn pre_tick_update_forwarding_agrees_end_to_end() {
    // RE-BLESSED 2026-08-26, cycle-dataflow phase B (PAIRED_IMPLEMENTATION_SCOPE.md):
    // divergence 1 is DISSOLVED. Codegen now emits the FORWARDED continuous assign
    // (`assign o = (r + 8'd1)`) for an opening-prefix drive — the meaning the
    // simulator always had — so the shape is a real equivalence check now. The
    // trace pins stay exact so a regression on either side is loud.
    let sim = sim_add_then_write();
    let expected: Vec<u8> = (2..).take(CYCLES).collect();
    assert_eq!(sim, expected, "simulator behaviour changed");

    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(ADD_SRC, "add_then_write", "clk", "o", "");
    assert_eq!(
        sim, sv,
        "sim and SV must AGREE on the forwarded reading — phase B's emission \
         (opening-prefix drives use edge_value) has regressed"
    );
}

#[test]
fn a_leading_input_read_removes_the_divergence() {
    // The discriminator, isolated: identical design, one added input read, and the
    // divergence disappears. This is what makes the gap narrow rather than general,
    // and it is why the equivalence-tested corpus is green.
    let sim = sim_add_with_leading_read();
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(ADD_READ_SRC, "add_with_leading_read", "clk", "o", "t->en = 1;");
    assert_eq!(
        sim, sv,
        "the leading-read form must stay sim ≡ SV — if this breaks, the pre-edge \
         barrier no longer pins the pre-tick segment"
    );
    assert_eq!(sim, (1..).take(CYCLES).collect::<Vec<u8>>());
}

// ── The hardware adjudication ─────────────────────────────────────────────────

const FAST_COUNTER_SRC: &str = r#"
#[hardware(sequential)]
async fn fast_counter(clk: Clock<ClkFast>, count_out: Out<Bits<8>, ClkFast>, flag_out: Out<Logic, ClkFast>) {
    let mut count: Bits<8> = Bits::zero();
    let mut latched = Logic::Zero;
    loop {
        if count[3] == Logic::One { latched = Logic::One; }
        count_out.write(count);
        flag_out.write(latched);
        clk.tick().await;
        count = count + Bits::from_lit::<1>();
    }
}
"#;

#[hardware(sequential)]
async fn fast_counter(
    clk: Clock<ClkFast>,
    count_out: Out<Bits<8>, ClkFast>,
    flag_out: Out<Logic, ClkFast>,
) {
    let mut count: Bits<8> = Bits::zero();
    let mut latched = Logic::Zero;
    loop {
        if count[3] == Logic::One {
            latched = Logic::One;
        }
        count_out.write(count);
        flag_out.write(latched);
        clk.tick().await;
        count = count + Bits::from_lit::<1>();
    }
}

fn sim_fast_counter() -> Vec<(u8, u8)> {
    let mut clk = Clock::<ClkFast>::new();
    let mut exec = HardwareExecutor::new();
    let (cp, co) = wire::<Bits<8>, ClkFast>(Bits::zero());
    let (fp, fo) = wire::<Logic, ClkFast>(Logic::Zero);
    let (dc, df) = (cp.dirty_handle(), fp.dirty_handle());
    exec.spawn_wired(fast_counter(clk.clone(), cp, fp), vec![dc, df], vec![]);
    (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            (co.read().as_u128() as u8, u8::from(fo.read() == Logic::One))
        })
        .collect()
}

#[test]
fn independent_hardware_anchors_the_corrected_spelling() {
    // The adjudication, run rather than argued. `ref_fast_counter` was committed in
    // `0d67f9e` (2026-07-30) as an outside implementation of this design, long
    // before the divergence was known — so it is a genuine third opinion, not a
    // restatement of either side.
    if !verilator_available() {
        return;
    }
    let sim = sim_fast_counter();

    let dir = std::env::temp_dir().join(format!("copper_fwd_ref_{}_{}", std::process::id(), TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let full = std::fs::read_to_string("examples/cdc/sv/two_domain_hierarchy.sv")
        .expect("read the independent reference");
    let start = full.find("module ref_fast_counter").expect("ref_fast_counter present");
    let end = full[start..].find("endmodule").unwrap() + start + "endmodule".len();
    let ref_path = dir.join("ref_fast_counter.sv");
    std::fs::write(&ref_path, &full[start..end]).unwrap();
    let independent: Vec<(u8, u8)> = run_sv(&ref_path, "ref_fast_counter", "wr_clk", &["count_o", "flag_o"], "")
        .into_iter()
        .map(|r| (r[0] as u8, r[1] as u8))
        .collect();

    let sv = copper_codegen::transpile_source(
        FAST_COUNTER_SRC,
        Some("fast_counter"),
        &copper_codegen::EmitConfig::default(),
    )
    .expect("transpile");
    let p = dir.join("fast_counter.sv");
    std::fs::write(&p, &sv).unwrap();
    let transpiled: Vec<(u8, u8)> = run_sv(&p, "fast_counter", "clk", &["count_out", "flag_out"], "")
        .into_iter()
        .map(|r| (r[0] as u8, r[1] as u8))
        .collect();
    let _ = std::fs::remove_dir_all(&dir);

    // RE-POINTED 2026-08-26, cycle-dataflow phase B, with a recorded rationale
    // (PAIRED_IMPLEMENTATION_SCOPE.md phase B; R7: anchors are re-verified, never
    // silently re-blessed). What the adjudication established is that the English
    // description — "a counter with a sticky flag" — maps to the REGISTERED form,
    // which under the model is the write-then-update SPELLING
    // (`fast_counter_corrected`). The reference therefore anchors THAT spelling.
    // The witness spelling (update-then-write) legitimately means the forwarded
    // trace, and as of phase B both implementations agree on it — one cycle ahead
    // of the reference, which is the two spellings being two different programs,
    // not a divergence.
    let corrected_sv = copper_codegen::transpile_source(
        FAST_COUNTER_CORRECTED_SRC,
        Some("fast_counter_corrected"),
        &copper_codegen::EmitConfig::default(),
    )
    .expect("transpile corrected");
    let dir2 = std::env::temp_dir().join(format!("copper_fwd_corr_{}_{}", std::process::id(), TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir2);
    std::fs::create_dir_all(&dir2).unwrap();
    let pc = dir2.join("fast_counter_corrected.sv");
    std::fs::write(&pc, &corrected_sv).unwrap();
    let corrected: Vec<(u8, u8)> =
        run_sv(&pc, "fast_counter_corrected", "clk", &["count_out", "flag_out"], "")
            .into_iter()
            .map(|r| (r[0] as u8, r[1] as u8))
            .collect();
    let _ = std::fs::remove_dir_all(&dir2);
    assert_eq!(
        corrected, independent,
        "the independent reference anchors the CORRECTED spelling — codegen for it \
         has drifted from the outside implementation"
    );

    // The witness spelling now agrees END TO END on the forwarded reading…
    assert_eq!(
        sim, transpiled,
        "the witness spelling must agree sim ≡ SV under forwarded emission (phase B)"
    );
    // …and leads the reference by exactly one cycle: two spellings, two programs.
    let sim_flag = sim.iter().position(|&(_, f)| f == 1).unwrap();
    let ref_flag = independent.iter().position(|&(_, f)| f == 1).unwrap();
    assert_eq!(sim_flag + 1, ref_flag, "sim = {sim:?}\nref = {independent:?}");
}

// ── Divergence 2, and the compensation ────────────────────────────────────────

#[hardware(sequential)]
async fn slow_consumer(clk: Clock<ClkSlow>, flag_in: In<Logic, ClkSlow>, out: Out<Logic, ClkSlow>) {
    loop {
        out.write(flag_in.read());
        clk.tick().await;
    }
}

const FAST_COUNTER_CORRECTED_SRC: &str = r#"
#[hardware(sequential)]
async fn fast_counter_corrected(
    clk: Clock<ClkFast>,
    count_out: Out<Bits<8>, ClkFast>,
    flag_out: Out<Logic, ClkFast>,
) {
    let mut count: Bits<8> = Bits::zero();
    let mut latched = Logic::Zero;
    loop {
        count_out.write(count);
        flag_out.write(latched);
        clk.tick().await;
        if count[3] == Logic::One {
            latched = Logic::One;
        }
        count = count + Bits::from_lit::<1>();
    }
}
"#;

/// The corrected counter: sticky update moved after the tick, so it reads the
/// pre-edge `count` exactly as `if (cnt[3]) latch <= 1;` does.
#[hardware(sequential)]
async fn fast_counter_corrected(
    clk: Clock<ClkFast>,
    count_out: Out<Bits<8>, ClkFast>,
    flag_out: Out<Logic, ClkFast>,
) {
    let mut count: Bits<8> = Bits::zero();
    let mut latched = Logic::Zero;
    loop {
        count_out.write(count);
        flag_out.write(latched);
        clk.tick().await;
        if count[3] == Logic::One {
            latched = Logic::One;
        }
        count = count + Bits::from_lit::<1>();
    }
}

/// The `two_domain_counter` chain at 2:1, returning the cycle each stage asserts:
/// `(flag_raw, sync_q, consumer)`.
fn chain_assert_cycles(corrected: bool) -> (usize, usize, usize) {
    let mut cf = Clock::<ClkFast>::new();
    let mut cs = Clock::<ClkSlow>::new();
    let mut exec = HardwareExecutor::new();

    let (cp, _co) = wire::<Bits<8>, ClkFast>(Bits::zero());
    let (fp, fi) = wire::<Logic, ClkFast>(Logic::Zero);
    let (qp, qi) = wire::<Logic, ClkSlow>(Logic::Zero);
    let (op, oo) = wire::<Logic, ClkSlow>(Logic::Zero);
    let (flag_obs, syncq_obs) = (fi.clone(), qi.clone());

    let (dc, df, dq, dop) =
        (cp.dirty_handle(), fp.dirty_handle(), qp.dirty_handle(), op.dirty_handle());
    let sync_reads = vec![fi.wire_id()];
    let cons_reads = vec![qi.wire_id()];

    if corrected {
        exec.spawn_wired(fast_counter_corrected(cf.clone(), cp, fp), vec![dc, df], vec![]);
    } else {
        exec.spawn_wired(fast_counter(cf.clone(), cp, fp), vec![dc, df], vec![]);
    }
    exec.spawn_wired(sync_2ff(cs.clone(), fi, qp), vec![dq], sync_reads);
    exec.spawn_wired(slow_consumer(cs.clone(), qi, op), vec![dop], cons_reads);

    let (mut f, mut q, mut o) = (None, None, None);
    for i in 0..12 {
        exec.tick_clock(&mut cf);
        exec.tick_clock(&mut cf);
        exec.tick_clock(&mut cs);
        if f.is_none() && flag_obs.read() == Logic::One {
            f = Some(i);
        }
        if q.is_none() && syncq_obs.read() == Logic::One {
            q = Some(i);
        }
        if o.is_none() && oo.read() == Logic::One {
            o = Some(i);
        }
    }
    (f.unwrap(), q.unwrap(), o.unwrap())
}

#[test]
fn d2_is_fixed_and_d1_still_demonstrates_the_hazard() {
    // HISTORY. `tests/two_domain_hierarchy_cdc.rs` used to pass for the WRONG reason:
    // the counter asserted a cycle early (D1) and the passthrough consumer a cycle
    // late (D2), and the two cancelled at the boundary. This test pinned that
    // cancellation. Both are now resolved — D1 is a compile error (this file's
    // `fast_counter` opts out precisely so it can still demonstrate it), and D2 is
    // fixed in `classify_reads`. What is asserted here is the post-fix state.

    // D2 IS FIXED: a combinational passthrough now TRACKS its producer instead of
    // lagging it. Adjudicated against independent hand-written Verilog — a clocked
    // producer feeding a passthrough gives `mid == out` in hardware.
    let (_f, q, o) = chain_assert_cycles(false);
    assert_eq!(
        o, q,
        "the combinational passthrough must track the synchronizer output, not lag it \
         — if this regresses, D2 is back"
    );

    // D1 IS STILL DEMONSTRABLE: `fast_counter` here carries
    // `allow_pretick_alignment`, so the divergent shape is still expressible and its
    // effect is still visible — the flag asserts a cycle before the corrected form.
    let (fc, qc, oc) = chain_assert_cycles(true);
    let (f, _, _) = chain_assert_cycles(false);
    assert_eq!(f + 1, fc, "the opted-out fast_counter should still assert a cycle early");
    assert_eq!(oc, qc, "the corrected chain must also have a tracking passthrough");
}

// ── D1 in a MIDDLE segment: a plain `Out` driven in two phases ────────────────
//
// The guardrail plan recorded the middle-segment gap as theoretical (Q5: "no
// instance in the corpus. If one turns up it should be measured before the rule is
// widened"). One turned up — a one-cycle output pulse, found while writing the
// first equivalence test for the UART receiver — and this is the measurement.
//
// The shape is a plain `Out` written on BOTH sides of a `clk.tick().await`. It is
// already refused on the linear multi-tick path ("output port `dv` is driven in
// more than one phase … hold it in a register"), but control extraction rewrites a
// body whose ticks live inside branches or loops into a SINGLE-tick `match pc` FSM,
// so that check counts one tick and passes while the `pc` states are the phases it
// was meant to count.
//
// Widening the D1 rule instead was measured and rejected: extending it to every
// post-tick segment flags 36 of 120 corpus modules, ~30 of them with passing
// equivalence tests (`det_010`, `mac_pipeline`, `dual_port_ram`, `bsg_dff_en`,
// every memory fixture). Writing a plain `Out` after a tick is the ordinary
// multi-phase pattern and is correct; writing it in *two* phases is not.

const PULSE_PLAIN_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn pulse_plain(clk: Clock<C>, dv: Out<Logic, C>) {
    loop {
        for _ in 0..3 { clk.tick().await; }
        dv.write(Logic::One);
        clk.tick().await;
        dv.write(Logic::Zero);
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn pulse_plain(clk: Clock<C>, dv: Out<Logic, C>) {
    loop {
        for _ in 0..3 { clk.tick().await; }
        dv.write(Logic::One);
        clk.tick().await;
        dv.write(Logic::Zero);
    }
}

const PULSE_REG_SRC: &str = r#"
#[hardware(sequential)]
async fn pulse_registered(clk: Clock<C>, dv: RegOut<Logic, C>) {
    loop {
        for _ in 0..3 { clk.tick().await; }
        dv.write(Logic::One);
        clk.tick().await;
        dv.write(Logic::Zero);
    }
}
"#;
#[hardware(sequential)]
async fn pulse_registered(clk: Clock<C>, dv: RegOut<Logic, C>) {
    loop {
        for _ in 0..3 { clk.tick().await; }
        dv.write(Logic::One);
        clk.tick().await;
        dv.write(Logic::Zero);
    }
}

fn sim_pulse_plain() -> Vec<u8> {
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (o, obs) = wire::<Logic, C>(Logic::Zero);
    let dh = o.dirty_handle();
    exec.spawn_wired(pulse_plain(clk.clone(), o), vec![dh], vec![]);
    (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            u8::from(obs.read() == Logic::One)
        })
        .collect()
}

fn sim_pulse_registered() -> Vec<u8> {
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (o, obs) = copper_core::port::registered_wire::<Logic, C>(&clk, Logic::Zero);
    let dh = o.dirty_handle();
    exec.spawn_wired(pulse_registered(clk.clone(), o), vec![dh], vec![]);
    (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            u8::from(obs.read() == Logic::One)
        })
        .collect()
}

/// The measurement the rule rested on: a plain `Out` pulse driven in two phases
/// diverged from its own transpiled SystemVerilog by exactly one cycle — until
/// phase C's emission form (2026-10-23, `vlir_lower::opening_hold_outputs`): an
/// opening drive in a conditional arm is now a forwarded combinational mux with
/// a hold register instead of an edge-sampled register, and this witness AGREES.
/// Phase E (2026-10-23) then derived and measured the whole class (the `e*`
/// battery at the end of this file): `multi_phase_out_write` was NARROWED, not
/// retired — an input-free extracted module like this one is accepted, the mixed,
/// register-free and linear classes stay refused. This test pins the admitted
/// witness.
#[test]
fn a_plain_out_driven_in_two_phases_agrees_under_phase_c_emission() {
    let sim = sim_pulse_plain();
    // Period 4 (three delay ticks plus the pulse tick); the simulator observes the
    // write in the post-edge settle of the cycle it was issued in.
    assert_eq!(
        sim,
        vec![0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0],
        "simulator behaviour changed"
    );

    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(PULSE_PLAIN_SRC, "pulse_plain", "clk", "dv", "");
    assert_eq!(
        sim, sv,
        "sim and SV DIVERGE again for a plain `Out` driven in two phases — phase C's \
         emission form (an opening drive stays combinational with a hold register) \
         regressed. Before 2026-10-23 the SV trace was the sim trace shifted one \
         cycle: [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0]."
    );
}

/// The discriminator, isolated: identical design, `RegOut` instead of `Out`, and
/// the divergence disappears. This is what makes the diagnostic's advice correct.
#[test]
fn the_registered_output_form_agrees() {
    let sim = sim_pulse_registered();
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(PULSE_REG_SRC, "pulse_registered", "clk", "dv", "");
    assert_eq!(
        sim, sv,
        "the `RegOut` form must stay sim ≡ SV — it is the remedy the guard points at"
    );
}

/// …and the rule no longer flags EITHER form (phase E, 2026-10-23): with no input
/// read in the module every drive is opening on both sides, phase C's mux shows
/// it, and the measured agreement above is what admitted the class. The module
/// keeps `allow_pretick_alignment` for `out_first_driven_late` (both writes are
/// constants past a folded tick-bearing loop), which is a startup rule and not
/// this one. The rule's own positive witnesses are the `e*` probes below.
#[test]
fn the_rule_no_longer_flags_the_input_free_pulse() {
    let plain: syn::ItemFn = syn::parse_str(PULSE_PLAIN_SRC).expect("parses");
    let reg: syn::ItemFn = syn::parse_str(PULSE_REG_SRC).expect("parses");
    assert!(
        copper_analysis::multi_phase_out_write(&plain).is_empty(),
        "pulse_plain is flagged again — clause (i) of the narrowed rule (no input read ⇒ accepted) regressed"
    );
    assert!(
        copper_analysis::multi_phase_out_write(&reg).is_empty(),
        "`RegOut` must be exempt — it is excluded by construction, like multi_write_collapse"
    );
    assert_eq!(
        copper_analysis::out_first_driven_late(&plain),
        vec!["dv".to_string()],
        "pulse_plain's remaining flag is out_first_driven_late; if that changed, its opt-out can go"
    );
}

// ── D1 in the TRAILING segment — a measured gap, NOT yet guarded ──────────────
//
// `unprotected_pretick_out_write` examines head → first tick. The statements
// AFTER the loop's last tick run in the same cycle as the head segment (falling
// off the end and re-entering costs no clock), so they are exposed to the same
// phase question — and D1's canonical shape, moved there, diverges:
//
//     loop { for _ in 0..2 { tick } n = n + 1; o.write(n); }
//
// The rule does not flag it. Two widenings were measured and REJECTED:
//
//   * merging the trailing segment into the head region flags **25** further
//     corpus modules, all passing — including `sync_2ff`, `dual_port_ram`, and
//     `fast_counter_corrected`, which is the module D1's OWN REMEDY produces
//     ("move the register update after the `clk.tick().await`"). The hazard needs
//     the output write and the register assignment in the SAME segment; split
//     across the two, it is the fix, not the bug.
//   * applying the two clauses to the trailing segment as a separate region cuts
//     that to **10**, all memory modules — e.g. `rom_from_fn`, whose trailing
//     segment is `if ready { q = data() } data.write(q)`: structurally identical
//     to the DUT below, and it AGREES.
//
// So the discriminator between this DUT and `rom_from_fn` is unidentified, and a
// leading `In` read is not it — the same DUT plus one was measured and still
// diverges. Until a condition has a flipping witness (requirement R3 of
// design_docs/PRETICK_ALIGNMENT_GUARDRAIL.md), widening the rule would reject
// correct designs, which is requirement R1.

const TRAILING_D1_SRC: &str = r#"
#[hardware(sequential)]
async fn trailing_update(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        for _ in 0..2 { clk.tick().await; }
        n = n + Bits::from_lit::<1>();
        o.write(n);
    }
}
"#;
#[hardware(sequential)]
async fn trailing_update(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        for _ in 0..2 { clk.tick().await; }
        n = n + Bits::from_lit::<1>();
        o.write(n);
    }
}

/// Phase C landed 2026-10-23: the trailing register update's plain-`Out` drive
/// is an opening drive in the extracted trailing state, and `vlir_lower` now
/// emits it as a forwarded combinational mux with a hold register (the m2 hand
/// model below), so sim ≡ SV, and `unprotected_trailing_out_write` is retired.
#[test]
fn trailing_segment_register_update_agrees() {
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (o, obs) = wire::<Bits<8>, C>(Bits::zero());
    let dh = o.dirty_handle();
    exec.spawn_wired(trailing_update(clk.clone(), o), vec![dh], vec![]);
    let sim: Vec<u8> = (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            obs.read().as_u128() as u8
        })
        .collect();

    // Guarded from 2026-08-25 to 2026-10-23 by `unprotected_trailing_out_write`,
    // retired when phase C landed. The head-segment rule must still not reach into
    // the trailing region: widening it was measured at 25 false positives (§5.4).
    let f: syn::ItemFn = syn::parse_str(TRAILING_D1_SRC).expect("parses");
    assert!(
        copper_analysis::unprotected_pretick_out_write(&f).is_empty(),
        "the HEAD rule now covers the trailing segment too — check that is deliberate; \
         the two were kept separate because widening the head region cost 25 false \
         positives (§5.4)"
    );

    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(TRAILING_D1_SRC, "trailing_update", "clk", "o", "");
    assert_eq!(
        sim, sv,
        "sim and SV diverge again for a trailing-segment register update — phase C's \
         emission form regressed (before 2026-10-23 the SV lagged by one cycle)"
    );
}

// ── Divergence 3 — a write in a state-machine arm, and D1's constant exemption ──
//
// FOUND 2026-08-25 by tests/corpus_equivalence.rs, on random stimulus, in
// `branch_merge_explicit` — a fixture that had been in the tree for weeks with only
// a STRUCTURAL check on it. The sharpest form of the finding: that module and its
// async twin `branch_merge` transpile to BYTE-IDENTICAL SystemVerilog, the twin
// agrees with that SV for 200 random cycles, and the explicit one leads it by a
// cycle. The simulator therefore disagrees with itself depending on how the same
// hardware is spelled.
//
// The DUTs below are that shape, minimised. `o` is written only in the `pc == 1`
// arm, which belongs to the NEXT cycle — but the simulator runs the next
// iteration's pre-tick segment during the post-edge settle of this tick, so the
// write is observable a cycle before the hardware performs it.
//
// WHY THE D1 GUARD DOES NOT FIRE: `unprotected_pretick_out_write` exempts a write
// of a CONSTANT, on the grounds that a constant is idempotent across the phase
// shift. That premise holds only if the write happens every cycle. Here which arm
// runs is chosen by a register (`pc`), so the constant is not the whole story — in
// `pc_arm_write` the other path leaves the port HOLDING, and in `pc_arm_toggle` it
// writes a DIFFERENT constant. Either way, *when* the write lands is observable.
//
// Narrowing the exemption is a rule widening whose corpus cost has to be measured
// first (see §5.4 for two widenings that were measured and rejected), so these pin
// today's behaviour and flip loudly when it changes.
//
// IT CHANGED (2026-10-26): the lead was codegen's statement-granular opening walk,
// not the phase shift it was read as. The `sel.read()` in the `pc == 0` arm marked
// the whole `match pc`, so the `pc == 1` arm's constant was edge-sampled one edge
// after the simulator's opening-instant write. With the walk arm-granular the drive
// is opening and phase C's mux + hold shows it at the same observation — both DUTs
// below AGREE, trace for trace. The D1 clause they justified is untouched here (the
// first test pins that it still fires); whether it should be re-derived is a rule
// decision with its own corpus pin, not a side effect of an emission change.

const PC_ARM_SRC: &str = r#"
#[hardware(sequential)]
async fn pc_arm_write(clk: Clock<C>, sel: In<Logic, C>, o: Out<Logic, C>) {
    let mut pc: u8 = 0;
    loop {
        match pc {
            0u8 => { if sel.read() == Logic::One { pc = 1; } }
            1u8 => { o.write(Logic::One); pc = 0; }
            _ => {}
        }
        clk.tick().await;
    }
}
"#;

#[hardware(sequential)]
async fn pc_arm_write(clk: Clock<C>, sel: In<Logic, C>, o: Out<Logic, C>) {
    let mut pc: u8 = 0;
    loop {
        match pc {
            0u8 => { if sel.read() == Logic::One { pc = 1; } }
            1u8 => { o.write(Logic::One); pc = 0; }
            _ => {}
        }
        clk.tick().await;
    }
}

/// The same machine with the other arm driving the port low, so the lead is visible
/// on every cycle instead of only the first — `pc_arm_write`'s output latches high
/// and can only show it once.
const PC_ARM_TOGGLE_SRC: &str = r#"
#[hardware(sequential)]
async fn pc_arm_toggle(clk: Clock<C>, sel: In<Logic, C>, o: Out<Logic, C>) {
    let mut pc: u8 = 0;
    loop {
        match pc {
            0u8 => { o.write(Logic::Zero); if sel.read() == Logic::One { pc = 1; } }
            1u8 => { o.write(Logic::One); pc = 0; }
            _ => {}
        }
        clk.tick().await;
    }
}
"#;

#[hardware(sequential)]
async fn pc_arm_toggle(clk: Clock<C>, sel: In<Logic, C>, o: Out<Logic, C>) {
    let mut pc: u8 = 0;
    loop {
        match pc {
            0u8 => { o.write(Logic::Zero); if sel.read() == Logic::One { pc = 1; } }
            1u8 => { o.write(Logic::One); pc = 0; }
            _ => {}
        }
        clk.tick().await;
    }
}

fn sim_pc_arm(toggle: bool) -> Vec<u8> {
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (sel_drv, sel_in) = wire::<Logic, C>(Logic::One);
    let (o, obs) = wire::<Logic, C>(Logic::Zero);
    let dh = o.dirty_handle();
    let reads = vec![sel_in.wire_id()];
    if toggle {
        exec.spawn_wired(pc_arm_toggle(clk.clone(), sel_in, o), vec![dh], reads);
    } else {
        exec.spawn_wired(pc_arm_write(clk.clone(), sel_in, o), vec![dh], reads);
    }
    sel_drv.write(Logic::One);
    (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            u8::from(obs.read() == Logic::One)
        })
        .collect()
}

/// The corpus instance's shape: `o` is written only in the `pc == 1` arm, so the
/// other path leaves it holding. Until 2026-10-26 the simulator showed the write a
/// cycle before the hardware performed it (sim `[1, 1, …]`, SV `[0, 1, 1, …]`),
/// once, because the output latches high — which is exactly why a hand-written
/// vector set never caught it and 200 cycles of random stimulus did. The cause was
/// codegen's statement-granular opening walk: the `sel.read()` in the OTHER arm
/// demoted this arm's drive to an edge-sampled register. Arm-granular, the drive is
/// opening in its own arm and phase C's mux + hold form shows it at the opening
/// observation — AGREES.
#[test]
fn a_write_in_a_state_arm_agrees_under_arm_granular_opening() {
    let sim = sim_pc_arm(false);
    assert_eq!(sim, vec![1u8; CYCLES], "simulator behaviour changed");

    // Re-derived 2026-10-28: the write sits in an arm no read reaches (the
    // `sel.read()` is in the sibling arm), so its anchor is opening on both
    // sides and the rule ADMITS it — the DUT compiles without an opt-out and
    // is the flip of `d2a` (the same machine with a second, closing write of
    // `o` in the reading arm, which diverges).
    let f: syn::ItemFn = syn::parse_str(PC_ARM_SRC).expect("parses");
    assert!(
        copper_analysis::unprotected_pretick_out_write(&f).is_empty(),
        "the re-derived D1 rule refuses pc_arm_write again — it agrees with its SV \
         (measured below); see the d* battery and CYCLE_DATAFLOW_SEMANTICS.md §10"
    );

    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(PC_ARM_SRC, "pc_arm_write", "clk", "o", "t->sel = 1;");
    assert_eq!(
        sim, sv,
        "sim and SV diverge again for a constant written in one state arm — the \
         arm-granular opening walk (shir_lower::lower_stmt_list) or phase C's \
         mux + hold form regressed (before 2026-10-26 the SV lagged by one cycle)"
    );
}

/// The same machine with the other arm driving the port low, so a lag would show
/// on every cycle rather than once. Before 2026-10-26 the two traces were each
/// other shifted by exactly one cycle for all 13 (a phase shift, not an
/// initialisation artifact); now they are equal for all 13.
#[test]
fn the_state_arm_toggle_agrees_on_every_cycle() {
    let sim = sim_pc_arm(true);
    let expected_sim: Vec<u8> = (0..CYCLES).map(|i| u8::from(i % 2 == 0)).collect();
    assert_eq!(sim, expected_sim, "simulator behaviour changed");

    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(PC_ARM_TOGGLE_SRC, "pc_arm_toggle", "clk", "o", "t->sel = 1;");
    assert_eq!(
        sim, sv,
        "sim and SV diverge again for the toggling state-arm constant — see \
         a_write_in_a_state_arm_agrees_under_arm_granular_opening"
    );
}

// ── branch_merge — the live twin's seed-masked `tail_o` lag ────────────────────
//
// `tests/fixtures/control_extraction_dut.rs::branch_merge` is the ASYNC twin of
// `branch_merge_explicit`: byte-identical SystemVerilog (asserted by
// control_extraction_structural.rs), and no D1 clause reaches it (its single
// `tail_o` write sits after the branch merge, behind a read on every path of the
// head region). The guardrail's §5.5 note of 2026-10-26 recorded that the corpus
// sweep's per-module seed drives `sel == 0` first, under which `tail_o` goes high
// at the closing edge and holds — so the lag that the explicit twin's SKIP row
// describes was invisible for the live twin, and it passed the sweep by its seed
// alone. The two DUTs below are a verbatim copy of the fixture's async twin; the
// probe drives `sel == 1` on the FIRST cycle and measures.
//
// MEASURED 2026-10-28 (this probe), `(head_o, mid_o, tail_o)` per cycle with
// `sel = 1` on cycle 0 and `0` after:
//
//   sim  [(1, 0, 1), (1, 0, 1), (1, 1, 1), (1, 1, 1), …]
//   sv   [(1, 0, 0), (1, 0, 1), (1, 1, 1), (1, 1, 1), …]
//
// `tail_o` is 1 in the simulator from cycle 0 and 0 in the SV at cycle 0 — one
// cycle, once, because the port latches high. `head_o` and `mid_o` agree on every
// cycle. The same DUT under `sel = 0` first agrees on
// every port for every cycle (the sweep's historical verdict, re-pinned here so
// the two readings stay side by side).
//
// This is what the sweep's multi-sequence stimulus policy (2026-10-28,
// `design_docs/CORPUS_DIFFERENTIAL_SWEEP.md` §3.2) exists to catch: the module's
// input is one bit wide, so the sweep now starts one sequence from EACH initial
// input vector, and the `sel = 1` sequence fails. The fixture's `build.rs` SKIP
// row records that verdict; this test pins the trace behind it.

const BRANCH_MERGE_SRC: &str = r#"
#[hardware(sequential)]
async fn branch_merge(
    clk: Clock<C>,
    sel: In<Logic, C>,
    head_o: Out<Logic, C>,
    mid_o: Out<Logic, C>,
    tail_o: Out<Logic, C>,
) {
    loop {
        head_o.write(Logic::One);
        if sel.read() == Logic::One {
            clk.tick().await;
        } else {
            mid_o.write(Logic::One);
        }
        tail_o.write(Logic::One);
        clk.tick().await;
    }
}
"#;

#[hardware(sequential)]
async fn branch_merge(
    clk: Clock<C>,
    sel: In<Logic, C>,
    head_o: Out<Logic, C>,
    mid_o: Out<Logic, C>,
    tail_o: Out<Logic, C>,
) {
    loop {
        head_o.write(Logic::One);
        if sel.read() == Logic::One {
            clk.tick().await;
        } else {
            mid_o.write(Logic::One);
        }
        tail_o.write(Logic::One);
        clk.tick().await;
    }
}

/// `(head_o, mid_o, tail_o)` per cycle under the given per-cycle `sel`.
fn sim_branch_merge(sel: &[u8]) -> Vec<(u8, u8, u8)> {
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (sel_drv, sel_in) = wire::<Logic, C>(Logic::Zero);
    let (h, h_obs) = wire::<Logic, C>(Logic::Zero);
    let (m, m_obs) = wire::<Logic, C>(Logic::Zero);
    let (t, t_obs) = wire::<Logic, C>(Logic::Zero);
    let handles = vec![h.dirty_handle(), m.dirty_handle(), t.dirty_handle()];
    let reads = vec![sel_in.wire_id()];
    exec.spawn_wired(branch_merge(clk.clone(), sel_in, h, m, t), handles, reads);
    sel.iter()
        .map(|&s| {
            sel_drv.write(if s == 1 { Logic::One } else { Logic::Zero });
            exec.tick_clock(&mut clk);
            (
                u8::from(h_obs.read() == Logic::One),
                u8::from(m_obs.read() == Logic::One),
                u8::from(t_obs.read() == Logic::One),
            )
        })
        .collect()
}

/// Like `run_sv`, but with `sel` driven per cycle from `sel`.
fn sv_branch_merge(sel: &[u8]) -> Vec<(u8, u8, u8)> {
    let sv = copper_codegen::transpile_source(
        BRANCH_MERGE_SRC,
        Some("branch_merge"),
        &copper_codegen::EmitConfig::default(),
    )
    .expect("transpile");
    let n = TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let work = std::env::temp_dir()
        .join(format!("copper_fwd_branch_merge_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("branch_merge.sv"), &sv).unwrap();
    let mut tb = String::from(
        "#include \"Vbranch_merge.h\"\n#include \"verilated.h\"\n#include <iostream>\n\
         int main(int c, char** v) { Verilated::commandArgs(c, v);\n\
         Vbranch_merge* t = new Vbranch_merge(); t->clk = 0; t->sel = 0; t->eval();\n",
    );
    for &s in sel {
        tb.push_str(&format!(
            "t->sel = {s}; t->clk=0; t->eval(); t->clk=1; t->eval(); \
             std::cout << (int)t->head_o << \" \" << (int)t->mid_o << \" \" << (int)t->tail_o << std::endl;\n"
        ));
    }
    tb.push_str("return 0; }\n");
    std::fs::write(work.join("tb.cpp"), tb).unwrap();
    let out = verilator_command()
        .current_dir(&work)
        .args([
            "--cc", "--exe", "--build", "--top-module", "branch_merge",
            "-Wno-DECLFILENAME", "-Wno-WIDTHEXPAND", "-CFLAGS", "-std=c++14",
            "branch_merge.sv", "tb.cpp",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "verilator build failed:\n{}", String::from_utf8_lossy(&out.stderr));
    let run = std::process::Command::new(work.join("obj_dir/Vbranch_merge")).output().unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&work);
    stdout
        .lines()
        .map(|l| {
            let v: Vec<u8> = l.split_whitespace().map(|x| x.parse().unwrap()).collect();
            (v[0], v[1], v[2])
        })
        .collect()
}

/// `sel == 1` on the first cycle: the live twin leads its own SV by one cycle on
/// `tail_o`, once. The seed-masked blind spot of guardrail §5.5, measured on the
/// unflagged module itself.
#[test]
fn branch_merge_tail_leads_its_sv_when_the_first_cycle_takes_sel_one() {
    let mut sel = vec![0u8; CYCLES];
    sel[0] = 1;
    let sim = sim_branch_merge(&sel);
    // Cycle 0 takes the `sel == 1` path (two ticks: `mid_o` stays low through
    // cycle 1); every later iteration takes the other path and writes all three.
    let mut expected_sim = vec![(1u8, 1u8, 1u8); CYCLES];
    expected_sim[0] = (1, 0, 1);
    expected_sim[1] = (1, 0, 1);
    assert_eq!(sim, expected_sim, "simulator behaviour changed");

    if !verilator_available() {
        return;
    }
    let sv = sv_branch_merge(&sel);
    let mut expected_sv = expected_sim.clone();
    expected_sv[0] = (1, 0, 0); // tail_o one cycle late — the only difference
    assert_eq!(
        sv, expected_sv,
        "the emitted SV's trace changed — re-measure the sweep's SKIP row for branch_merge"
    );
    assert_ne!(sim, sv, "sim and SV now AGREE under sel == 1 first: the branch_merge SKIP row in build.rs is stale");
    assert_eq!(
        sim.iter().map(|c| c.2).collect::<Vec<_>>()[1..],
        sv.iter().map(|c| c.2).collect::<Vec<_>>()[1..],
        "the lag is no longer confined to cycle 0"
    );
}

/// `sel == 0` on the first cycle: every port agrees on every cycle — the sweep's
/// historical single-seed verdict, which is why the lag was invisible.
#[test]
fn branch_merge_agrees_when_the_first_cycle_takes_sel_zero() {
    let mut sel = vec![0u8; CYCLES];
    sel[1] = 1;
    sel[4] = 1;
    let sim = sim_branch_merge(&sel);
    if !verilator_available() {
        return;
    }
    let sv = sv_branch_merge(&sel);
    assert_eq!(sim, sv, "sim and SV diverge under sel == 0 first — a new divergence, not the §5.5 one");
}

// ── m1 / V8 — a write between a leading read and the register update ──────────
//
// `design_docs/DERIVATION_TABLE.md` F2, measured (m1 in its §5). The
// cycle-dataflow model derives a divergent shape D1 does not guard: in a
// closing-anchored segment the barrier parks the task AT THE READ SITE, so a
// plain-`Out` write placed after the read runs in the pre-edge settle — and if it
// also precedes the update of the register it reads, it writes the PREVIOUS
// generation's value, which the emitted `assign o = r` (Q) never shows at any
// observation instant. D1 exempts the segment precisely because the read
// comb-reaches the update, so nothing flags it.
//
// This is the CPU sweep's `program_counter` divergence #1 (TODO cause Q) reduced
// to its minimal pair. Position of the write is the only variable across the
// three DUTs:
//
//   V8a  read; write; update   → predicted DIVERGE (SV leads by one)
//   V8b  read; update; write   → predicted agree (lfsr's shape: forwarded value
//                                 ≡ committing value ≡ Q at the next observation)
//   V8c  write; read+update    → predicted agree (the write executes BEFORE the
//                                 barrier point, at the opening, reading
//                                 committed state)

const V8A_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn v8a_read_write_update(clk: Clock<C>, step: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        let s = step.read();
        o.write(r);
        r = r + s;
        clk.tick().await;
    }
}
"#;

#[hardware(sequential, allow_pretick_alignment)]
async fn v8a_read_write_update(clk: Clock<C>, step: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        let s = step.read();
        o.write(r);
        r = r + s;
        clk.tick().await;
    }
}

const V8B_SRC: &str = r#"
#[hardware(sequential)]
async fn v8b_read_update_write(clk: Clock<C>, step: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        let s = step.read();
        r = r + s;
        o.write(r);
        clk.tick().await;
    }
}
"#;

#[hardware(sequential)]
async fn v8b_read_update_write(clk: Clock<C>, step: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        let s = step.read();
        r = r + s;
        o.write(r);
        clk.tick().await;
    }
}

const V8C_SRC: &str = r#"
#[hardware(sequential)]
async fn v8c_write_read_update(clk: Clock<C>, step: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        o.write(r);
        r = r + step.read();
        clk.tick().await;
    }
}
"#;

#[hardware(sequential)]
async fn v8c_write_read_update(clk: Clock<C>, step: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        o.write(r);
        r = r + step.read();
        clk.tick().await;
    }
}

fn sim_v8(which: u8) -> Vec<u8> {
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (step_drv, step_in) = wire::<Bits<8>, C>(Bits::zero());
    let (p, obs) = wire::<Bits<8>, C>(Bits::zero());
    let dh = p.dirty_handle();
    let reads = vec![step_in.wire_id()];
    match which {
        0 => exec.spawn_wired(v8a_read_write_update(clk.clone(), step_in, p), vec![dh], reads),
        1 => exec.spawn_wired(v8b_read_update_write(clk.clone(), step_in, p), vec![dh], reads),
        _ => exec.spawn_wired(v8c_write_read_update(clk.clone(), step_in, p), vec![dh], reads),
    };
    step_drv.write(Bits::from_lit::<1>());
    (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            obs.read().as_u128() as u8
        })
        .collect()
}

#[test]
fn v8a_write_between_leading_read_and_update_diverges_known_gap() {
    let sim = sim_v8(0);
    let expected_sim: Vec<u8> = (0..).take(CYCLES).collect();
    assert_eq!(
        sim, expected_sim,
        "simulator behaviour changed; if it now starts at 1 the gap is FIXED"
    );

    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(V8A_SRC, "v8a_read_write_update", "clk", "o", "t->step = 1;");
    let expected_sv: Vec<u8> = (1..).take(CYCLES).collect();
    assert_eq!(sv, expected_sv, "transpiled SV behaviour changed");
    assert_ne!(
        sim, sv,
        "sim and SV now AGREE — F2's derived shape is FIXED. Update \
         DERIVATION_TABLE.md F2 and the write-position rule it proposes."
    );
}

#[test]
fn v8b_moving_the_write_after_the_update_removes_the_divergence() {
    let sim = sim_v8(1);
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(V8B_SRC, "v8b_read_update_write", "clk", "o", "t->step = 1;");
    assert_eq!(
        sim, sv,
        "the write-after-update form must stay sim ≡ SV — this is lfsr's shape \
         (legal shape 2 in DERIVATION_TABLE.md §1)"
    );
    assert_eq!(sim, (1..).take(CYCLES).collect::<Vec<u8>>());
}

#[test]
fn v8c_moving_the_write_before_the_read_removes_the_divergence() {
    let sim = sim_v8(2);
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(V8C_SRC, "v8c_write_read_update", "clk", "o", "t->step = 1;");
    assert_eq!(
        sim, sv,
        "the write-before-the-read form must stay sim ≡ SV — the write executes \
         before the barrier point, at the opening, reading committed state"
    );
    assert_eq!(sim, (1..).take(CYCLES).collect::<Vec<u8>>());
}

// V8d — V8a with the update routed through a same-cycle temp (`let next = …`).
// This is the shape of the `single_loop_local_ok.rs` UI fixture, which the new
// rule flagged on landing: the fixture existed to exercise the single-loop and
// param-shadowing guardrails and was COMPILE-ONLY — nothing had ever measured its
// behaviour. The rename changes nothing: the write still captures pre-update
// `count` at the pre-edge while `assign out = count` shows the committed value.

const V8D_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn v8d_temp_renamed_update(clk: Clock<C>, step: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut count: Bits<8> = Bits::zero();
    loop {
        let next = count + step.read();
        o.write(count);
        count = next;
        clk.tick().await;
    }
}
"#;

#[hardware(sequential, allow_pretick_alignment)]
async fn v8d_temp_renamed_update(clk: Clock<C>, step: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut count: Bits<8> = Bits::zero();
    loop {
        let next = count + step.read();
        o.write(count);
        count = next;
        clk.tick().await;
    }
}

#[test]
fn v8d_temp_renamed_update_diverges_like_v8a_known_gap() {
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (step_drv, step_in) = wire::<Bits<8>, C>(Bits::zero());
    let (p, obs) = wire::<Bits<8>, C>(Bits::zero());
    let dh = p.dirty_handle();
    let reads = vec![step_in.wire_id()];
    exec.spawn_wired(v8d_temp_renamed_update(clk.clone(), step_in, p), vec![dh], reads);
    step_drv.write(Bits::from_lit::<1>());
    let sim: Vec<u8> = (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            obs.read().as_u128() as u8
        })
        .collect();
    let expected_sim: Vec<u8> = (0..).take(CYCLES).collect();
    assert_eq!(sim, expected_sim, "simulator behaviour changed");

    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(V8D_SRC, "v8d_temp_renamed_update", "clk", "o", "t->step = 1;");
    let expected_sv: Vec<u8> = (1..).take(CYCLES).collect();
    assert_eq!(sv, expected_sv, "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE — see the V8a test's note");
}

// ── m2 — the model's lowering for the trailing segment, hand-written ──────────
//
// `design_docs/DERIVATION_TABLE.md` §5 m2, the §5.3 technique (hand-write the
// candidate lowering, run it under Verilator, compare against the simulator).
//
// The cycle-dataflow model places `trailing_update`'s divergence in the
// **dissolved** bucket: the trailing statements belong to the cycle the LAST tick
// opens, their register update commits AT that edge (the trailing-statements rule
// in SYNCHRONOUS_SEMANTICS.md), and the port write publishes the forwarded value —
// which equals the register committed at that same edge. Hand-lowered, that is an
// FSM whose trailing increment lands at the edge closing the final wait state,
// with `assign o = n` reading the committed register:
//
//   always_ff @(posedge clk) begin
//     state <= ~state;
//     if (state == 1) n <= n + 1;   // the edge that OPENS the trailing cycle
//   end
//   assign o = n;
//
// Prediction: this matches the simulator's existing trace cycle-for-cycle —
// i.e. the sim was already implementing the model, and today's codegen (which
// diverges by one, `d1_in_the_trailing_segment_is_an_unguarded_gap`) is the side
// the paired fix changes.

const TRAILING_MODEL_SV: &str = r#"
module trailing_update_model(input logic clk, output logic [7:0] o);
  logic [7:0] n = 8'd0;
  logic state = 1'b0;
  always_ff @(posedge clk) begin
    state <= ~state;
    if (state == 1'b1) n <= n + 8'd1;
  end
  assign o = n;
endmodule
"#;

#[test]
fn m2_model_forwarded_lowering_matches_the_simulator_for_trailing_update() {
    // The simulator's trace, now pinned exactly: the write lands at the
    // observation of every even edge, so obs k shows floor(k/2).
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (o, obs) = wire::<Bits<8>, C>(Bits::zero());
    let dh = o.dirty_handle();
    exec.spawn_wired(trailing_update(clk.clone(), o), vec![dh], vec![]);
    let sim: Vec<u8> = (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            obs.read().as_u128() as u8
        })
        .collect();
    let expected: Vec<u8> = (1..=CYCLES).map(|k| (k / 2) as u8).collect();
    assert_eq!(sim, expected, "simulator behaviour changed");

    if !verilator_available() {
        return;
    }

    let dir = std::env::temp_dir()
        .join(format!("copper_fwd_m2_{}_{}", std::process::id(), TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("trailing_update_model.sv");
    std::fs::write(&p, TRAILING_MODEL_SV).unwrap();
    let model: Vec<u8> = run_sv(&p, "trailing_update_model", "clk", &["o"], "")
        .into_iter()
        .map(|r| r[0] as u8)
        .collect();
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(
        sim, model,
        "the hand-lowered MODEL emission no longer matches the simulator — the m2 \
         derivation (DERIVATION_TABLE.md, trailing_update row) is wrong or the \
         executor's trailing-segment behaviour changed"
    );

    // And since phase C (2026-10-23) codegen agrees with both: the complete m2
    // statement is sim ≡ model ≡ SV. (The sim ≡ SV half is also asserted by
    // trailing_segment_register_update_agrees.)
    let sv = transpile_and_run(TRAILING_D1_SRC, "trailing_update", "clk", "o", "");
    assert_eq!(
        model, sv,
        "today's transpiled SV no longer matches the hand-lowered model — phase C's \
         emission form (`vlir_lower::opening_hold_outputs`) regressed; before it \
         landed the SV lagged the model by one cycle"
    );
}

// ── The V-battery re-measured under forwarded emission (phase D evidence) ─────
//
// Narrowing D1 (`unprotected_pretick_out_write`) after phase B requires knowing
// which of its measured-divergent shapes the forwarded emission DISSOLVED. The
// guardrail's §3.1 verdicts predate both the trailing-forwarding fix
// (2026-08-25) and phase B, so they are re-measured here and pinned. V1 is
// covered by `pre_tick_update_forwarding_agrees_end_to_end`; these are the
// discriminating variants for the narrowing's clauses.

const V5_SRC: &str = r#"
#[hardware(sequential)]
async fn v5_trailing_read(clk: Clock<C>, i: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = r + Bits::from_lit::<1>();
        o.write(r);
        let _late = i.read();
        clk.tick().await;
    }
}
"#;

#[hardware(sequential)]
async fn v5_trailing_read(clk: Clock<C>, i: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        r = r + Bits::from_lit::<1>();
        o.write(r);
        let _late = i.read();
        clk.tick().await;
    }
}

const V7_SRC: &str = r#"
#[hardware(sequential)]
async fn v7_escape_across_tick(clk: Clock<C>, i: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    let mut s: Bits<8> = Bits::zero();
    loop {
        r = r + Bits::from_lit::<1>();
        o.write(s);
        clk.tick().await;
        s = r;
    }
}
"#;

#[hardware(sequential)]
async fn v7_escape_across_tick(clk: Clock<C>, _i: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    let mut s: Bits<8> = Bits::zero();
    loop {
        r = r + Bits::from_lit::<1>();
        o.write(s);
        clk.tick().await;
        s = r;
    }
}

const W4_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn w4_mixed_alignment(clk: Clock<C>, i: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    let mut r: Bits<8> = Bits::zero();
    loop {
        if phase == 0 { r = i.read(); phase = 1; }
        else { r = r + Bits::from_lit::<1>(); phase = 0; }
        o.write(r);
        clk.tick().await;
    }
}
"#;

#[hardware(sequential, allow_pretick_alignment)]
async fn w4_mixed_alignment(clk: Clock<C>, i: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    let mut r: Bits<8> = Bits::zero();
    loop {
        if phase == 0 { r = i.read(); phase = 1; }
        else { r = r + Bits::from_lit::<1>(); phase = 0; }
        o.write(r);
        clk.tick().await;
    }
}

fn sim_probe(which: u8) -> Vec<u8> {
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (i_drv, i_in) = wire::<Bits<8>, C>(Bits::zero());
    let (p, obs) = wire::<Bits<8>, C>(Bits::zero());
    let dh = p.dirty_handle();
    let reads = vec![i_in.wire_id()];
    match which {
        0 => exec.spawn_wired(v5_trailing_read(clk.clone(), i_in, p), vec![dh], reads),
        1 => exec.spawn_wired(v7_escape_across_tick(clk.clone(), i_in, p), vec![dh], reads),
        _ => exec.spawn_wired(w4_mixed_alignment(clk.clone(), i_in, p), vec![dh], reads),
    };
    i_drv.write(Bits::from_lit::<3>());
    (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            obs.read().as_u128() as u8
        })
        .collect()
}

/// Measured 2026-08-26 under forwarded emission (phase B), pinning the phase-D
/// narrowing's discriminator: V5 and V7 AGREE (dissolved — V7's 2026-08-21
/// verdict was stale even before phase B: the 2026-08-25 shared
/// trailing-forwarding map already emits `s <= r + 1`), W4 DIVERGES (the
/// read-preceded / path-dependent-boundary class the narrowed rule retains —
/// sim holds `i + 1`, the SV alternates).
#[test]
fn d_narrowing_battery_verdicts() {
    if !verilator_available() {
        return;
    }
    let v5_sim = sim_probe(0);
    let v5_sv = transpile_and_run(V5_SRC, "v5_trailing_read", "clk", "o", "t->i = 3;");
    assert_eq!(v5_sim, v5_sv, "V5 (trailing read) must stay dissolved: the write is an opening-prefix drive");
    assert_eq!(v5_sim, (2..).take(CYCLES).collect::<Vec<u8>>());

    let v7_sim = sim_probe(1);
    let v7_sv = transpile_and_run(V7_SRC, "v7_escape_across_tick", "clk", "o", "t->i = 3;");
    assert_eq!(v7_sim, v7_sv, "V7 (escape across the tick) must stay dissolved: the shared trailing-forwarding map covers `s = r`");
    assert_eq!(v7_sim, (1..).take(CYCLES).collect::<Vec<u8>>());

    let w4_sim = sim_probe(2);
    let w4_sv = transpile_and_run(W4_SRC, "w4_mixed_alignment", "clk", "o", "t->i = 3;");
    assert_eq!(w4_sim, vec![4u8; CYCLES], "simulator behaviour changed for the mixed-alignment witness");
    assert_ne!(
        w4_sim, w4_sv,
        "sim and SV now AGREE for MIXED alignment — the path-dependent-boundary \
         class is fixed; re-bless the narrowed D1 rule, EXPECTED_FLAGGED, and the \
         ui/fail/pretick_alignment case, which all pin this divergence"
    );
}

// ── The linear-trailing probe (phase-C decision evidence, 2026-08-27) ─────────
//
// §5.4's divergence was only ever measured on the EXTRACTED path (the folded
// counted `for`). The linear multi-tick path commits trailing register updates
// at the last phase's edge (the 2026-08-25 shared-map work), so the model
// predicted the linear spelling of the identical shape AGREES — measured so
// (2026-08-27). The extracted class diverged until phase C (2026-10-23), see
// `branch_trailing`; both routes now agree and no rule refuses the shape.

const LINEAR_TRAILING_SRC: &str = r#"
#[hardware(sequential)]
async fn linear_trailing(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        clk.tick().await;
        clk.tick().await;
        n = n + Bits::from_lit::<1>();
        o.write(n);
    }
}
"#;

#[hardware(sequential)]
async fn linear_trailing(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        clk.tick().await;
        clk.tick().await;
        n = n + Bits::from_lit::<1>();
        o.write(n);
    }
}

#[test]
fn linear_trailing_probe() {
    // The linear multi-tick spelling was measured AGREEING on 2026-08-27 (the
    // linear path commits trailing updates at the right edge); since phase C
    // (2026-10-23) the extracted spelling agrees too and the trailing rule is
    // retired, so this probe is the linear route's standing equivalence witness.

    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (o, obs) = wire::<Bits<8>, C>(Bits::zero());
    let dh = o.dirty_handle();
    exec.spawn_wired(linear_trailing(clk.clone(), o), vec![dh], vec![]);
    let sim: Vec<u8> = (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            obs.read().as_u128() as u8
        })
        .collect();
    let expected: Vec<u8> = (1..=CYCLES).map(|k| (k / 2) as u8).collect();
    assert_eq!(sim, expected, "simulator behaviour changed");

    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(LINEAR_TRAILING_SRC, "linear_trailing", "clk", "o", "");
    eprintln!("linear_trailing: sim = {sim:?}\n                 sv  = {sv:?}  -> {}", if sim == sv { "AGREE" } else { "DIVERGE" });
    assert_eq!(
        sim, sv,
        "…but the LINEAR lowering handles the shape correctly, so the flag is a \
         false positive on this path — an extraction lowering limitation, not a \
         semantics rule (the phase-C reframing)"
    );
}

// The same trailing body behind BRANCH-NESTED ticks — extraction fires with a
// top-level LAST tick. Measuring this was blocked until 2026-08-27 by an
// emission-legality bug: the cause-N `edge_condition` substitution puts a
// compound expression under the branch condition's `Index`, and the emitter
// rendered `pc <= (((n + 8'd1)[0] == 1'b1) ? …)` — a bit-select on a
// parenthesized expression, which SV forbids. Fixed in `emit.rs` (`select_legal`
// gates the `[..]` syntax; compound bases emit the width-cast form
// `1'((n + 8'd1))`), which unblocked the measurement:
//
//   VERDICT (2026-08-27): DIVERGE — the SV trace is the sim trace delayed by
//   one cycle. The extracted route commits the trailing update one edge late
//   even when the LAST tick is top-level, so the placement error is a property
//   of the extraction route as a whole, not just its rotation placement. The
//   trailing rule's flag on this class is a TRUE positive; only the LINEAR
//   class (above) is over-flagged, so the phase-C/D narrowing exempts the
//   linear lowering route alone.

const BRANCH_TRAILING_SRC: &str = r#"
#[hardware(sequential)]
async fn branch_trailing(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        if n[0] == Logic::One {
            clk.tick().await;
        } else {
            clk.tick().await;
        }
        clk.tick().await;
        n = n + Bits::from_lit::<1>();
        o.write(n);
    }
}
"#;

#[hardware(sequential)]
async fn branch_trailing(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        if n[0] == Logic::One {
            clk.tick().await;
        } else {
            clk.tick().await;
        }
        clk.tick().await;
        n = n + Bits::from_lit::<1>();
        o.write(n);
    }
}

#[test]
fn branch_trailing_probe() {
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (o, obs) = wire::<Bits<8>, C>(Bits::zero());
    let dh = o.dirty_handle();
    exec.spawn_wired(branch_trailing(clk.clone(), o), vec![dh], vec![]);
    let sim: Vec<u8> = (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            obs.read().as_u128() as u8
        })
        .collect();
    let expected: Vec<u8> = (1..=CYCLES).map(|k| (k / 2) as u8).collect();
    assert_eq!(sim, expected, "simulator behaviour changed");

    // The emission bug this probe used to pin (`(n + 8'd1)[0]`, illegal SV) is
    // fixed — selects over compound bases now emit the width-cast form — so
    // the text must stay select-legal.
    let sv_text = copper_codegen::transpile_source(
        BRANCH_TRAILING_SRC,
        Some("branch_trailing"),
        &copper_codegen::EmitConfig::default(),
    )
    .expect("transpiles");
    assert!(
        !sv_text.contains(")["),
        "a select over a parenthesized expression is back in the emitted SV — \
         the emit.rs select_legal fallback regressed"
    );

    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(BRANCH_TRAILING_SRC, "branch_trailing", "clk", "o", "");
    eprintln!("branch_trailing: sim = {sim:?}\n                 sv  = {sv:?}  -> {}", if sim == sv { "AGREE" } else { "DIVERGE" });
    //   VERDICT (2026-10-23, phase C): AGREE. The trailing state's plain-`Out`
    //   drive is emitted as a forwarded combinational mux with a hold register,
    //   so it is visible from the opening observation exactly as the simulator
    //   shows it. The one-cycle-late trace `(0..CYCLES).map(|k| k / 2)` is what
    //   the extracted route produced before.
    assert_eq!(
        sim, sv,
        "sim and SV diverge again for extracted trailing after a TOP-LEVEL last \
         tick — phase C's emission form regressed"
    );
}

// ── V8t — the TRAILING write-before-update, stage-machine spelling ────────────
//
// Added while extending `pretick_out_write_before_update` with its trailing
// clause (2026-08-27): the exact-set pin flagged `module_composition_hybrid`'s
// pipeline stages alongside the measured `out_from_reg_before_commit`, and
// those stages' own tests are SIM-ONLY — their sim ≡ SV status was unmeasured.
// This is their shape, minimal: publish the register, then recompute it from an
// input, all in the trailing segment. The mixed-generation derivation says the
// write publishes the PREVIOUS generation for the cycle while `assign o = reg`
// shows the one committed at the opening edge.

const V8T_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn v8t_stage_publish_then_load(clk: Clock<C>, i: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut reg: Bits<8> = Bits::zero();
    loop {
        clk.tick().await;
        o.write(reg);
        reg = i.read() + Bits::from_lit::<1>();
    }
}
"#;

#[hardware(sequential, allow_pretick_alignment)]
async fn v8t_stage_publish_then_load(clk: Clock<C>, i: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut reg: Bits<8> = Bits::zero();
    loop {
        clk.tick().await;
        o.write(reg);
        reg = i.read() + Bits::from_lit::<1>();
    }
}

#[test]
fn v8t_trailing_stage_shape_verdict() {
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (i_drv, i_in) = wire::<Bits<8>, C>(Bits::zero());
    let (p, obs) = wire::<Bits<8>, C>(Bits::zero());
    let dh = p.dirty_handle();
    let reads = vec![i_in.wire_id()];
    exec.spawn_wired(v8t_stage_publish_then_load(clk.clone(), i_in, p), vec![dh], reads);
    let sim: Vec<u8> = (0..CYCLES)
        .map(|k| {
            i_drv.write(Bits::from_usize(k + 1));
            exec.tick_clock(&mut clk);
            obs.read().as_u128() as u8
        })
        .collect();
    if !verilator_available() {
        return;
    }
    let sv_rows = {
        let sv = copper_codegen::transpile_source(
            V8T_SRC,
            Some("v8t_stage_publish_then_load"),
            &copper_codegen::EmitConfig::default(),
        )
        .expect("transpile");
        let dir = std::env::temp_dir().join(format!("copper_v8t_{}_{}", std::process::id(), TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("v8t_stage_publish_then_load.sv");
        std::fs::write(&p, &sv).unwrap();
        let mut tb = String::from(
            "#include \"Vv8t_stage_publish_then_load.h\"\n#include \"verilated.h\"\n#include <iostream>\n\
             int main(int c, char** v) { Verilated::commandArgs(c, v);\n\
             Vv8t_stage_publish_then_load* t = new Vv8t_stage_publish_then_load(); t->clk = 0; t->eval();\n",
        );
        for k in 0..CYCLES {
            tb.push_str(&format!(
                "t->i = {}; t->clk=0; t->eval(); t->clk=1; t->eval(); std::cout << (int)t->o << std::endl;\n",
                k + 1
            ));
        }
        tb.push_str("return 0; }\n");
        let tbp = dir.join("tb.cpp");
        std::fs::write(&tbp, tb).unwrap();
        let out = verilator_command()
            .current_dir(&dir)
            .args(["--cc", "--exe", "--build", "--top-module", "v8t_stage_publish_then_load",
                   "-Wno-DECLFILENAME", "-Wno-WIDTHEXPAND", "-CFLAGS", "-std=c++14"])
            .arg(std::fs::canonicalize(&p).unwrap())
            .arg(&tbp)
            .output()
            .unwrap();
        assert!(out.status.success(), "verilator build failed:\n{}", String::from_utf8_lossy(&out.stderr));
        let run = std::process::Command::new(dir.join("obj_dir/Vv8t_stage_publish_then_load")).output().unwrap();
        let s = String::from_utf8_lossy(&run.stdout).into_owned();
        let _ = std::fs::remove_dir_all(&dir);
        s.lines().filter_map(|l| l.trim().parse::<u8>().ok()).collect::<Vec<u8>>()
    };
    // Measured 2026-08-27, first run: the sim publishes the PRE-update register
    // for each cycle while the SV shows the value committed at the opening edge
    // — the one-cycle lead, trailing edition. The three module_composition
    // stages had this exact shape with SIM-ONLY tests; they were migrated to
    // the canonical write/read/tick/update spelling the same day.
    let expected_sim: Vec<u8> = std::iter::once(0u8).chain(2..).take(CYCLES).collect();
    assert_eq!(sim, expected_sim, "simulator behaviour changed");
    let expected_sv: Vec<u8> = (2..).take(CYCLES).collect();
    assert_eq!(sv_rows, expected_sv, "transpiled SV behaviour changed");
    assert_ne!(
        sim, sv_rows,
        "sim and SV now AGREE for the trailing publish-then-load shape — the \
         trailing clause of pretick_out_write_before_update guards a fixed \
         divergence; re-bless the rule, EXPECTED_WRITE_BEFORE_UPDATE, and the \
         out_phase ledger entries together"
    );
}

// ── Phase E — a plain `Out` driven in two or more clock phases ────────────────
//
// `design_docs/CYCLE_DATAFLOW_SEMANTICS.md` "Phase E". The derivation classifies
// each writing segment by ANCHOR (opening: no `In` read reaches the write, so the
// simulator runs it at the cycle's opening instant; closing: a read's barrier
// parks it at the pre-edge settle) and predicts, from the write-window invariant
// and the phase-C emission form, which combinations agree. Every probe below is
// an inline module (not swept), so these tests ARE the measurement. Verdicts are
// recorded on each test; the exact traces are pinned so a change on either side
// is loud.

const E2_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e2_two_registers(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut a: Bits<8> = Bits::zero();
    let mut b: Bits<8> = Bits::zero();
    loop {
        a = a + Bits::from_lit::<1>();
        b = b + Bits::from_lit::<3>();
        for _ in 0..2 { clk.tick().await; }
        o.write(a);
        clk.tick().await;
        o.write(b);
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e2_two_registers(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut a: Bits<8> = Bits::zero();
    let mut b: Bits<8> = Bits::zero();
    loop {
        a = a + Bits::from_lit::<1>();
        b = b + Bits::from_lit::<3>();
        for _ in 0..2 { clk.tick().await; }
        o.write(a);
        clk.tick().await;
        o.write(b);
    }
}

const E3_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e3_forwarded_then_committed(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        for _ in 0..2 { clk.tick().await; }
        r = r + Bits::from_lit::<1>();
        o.write(r);
        clk.tick().await;
        o.write(r);
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e3_forwarded_then_committed(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        for _ in 0..2 { clk.tick().await; }
        r = r + Bits::from_lit::<1>();
        o.write(r);
        clk.tick().await;
        o.write(r);
    }
}

const E4_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e4_closing_then_opening(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
        for _ in 0..2 { clk.tick().await; }
        o.write(Bits::zero());
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e4_closing_then_opening(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
        for _ in 0..2 { clk.tick().await; }
        o.write(Bits::zero());
    }
}

const E5_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e5_two_closing(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read() + cnt);
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e5_two_closing(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read() + cnt);
    }
}

const E6_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e6_closing_via_wire_then_opening(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        let r = x.read();
        o.write(r);
        for _ in 0..2 { clk.tick().await; }
        o.write(Bits::zero());
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e6_closing_via_wire_then_opening(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        let r = x.read();
        o.write(r);
        for _ in 0..2 { clk.tick().await; }
        o.write(Bits::zero());
    }
}

const E4I_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e4i_register_free_passthrough_then_opening(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    loop {
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
        for _ in 0..2 { clk.tick().await; }
        o.write(Bits::zero());
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e4i_register_free_passthrough_then_opening(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    loop {
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
        for _ in 0..2 { clk.tick().await; }
        o.write(Bits::zero());
    }
}

const E7_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e7_linear_two_registers(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut a: Bits<8> = Bits::zero();
    let mut b: Bits<8> = Bits::zero();
    loop {
        a = a + Bits::from_lit::<1>();
        b = b + Bits::from_lit::<3>();
        clk.tick().await;
        o.write(a);
        clk.tick().await;
        o.write(b);
        clk.tick().await;
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e7_linear_two_registers(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut a: Bits<8> = Bits::zero();
    let mut b: Bits<8> = Bits::zero();
    loop {
        a = a + Bits::from_lit::<1>();
        b = b + Bits::from_lit::<3>();
        clk.tick().await;
        o.write(a);
        clk.tick().await;
        o.write(b);
        clk.tick().await;
    }
}

const E7W_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e7w_linear_phase_local_wires(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut a: Bits<8> = Bits::zero();
    loop {
        a = a + Bits::from_lit::<1>();
        clk.tick().await;
        let v: Bits<8> = a + Bits::from_lit::<2>();
        o.write(v);
        clk.tick().await;
        let w: Bits<8> = a + Bits::from_lit::<5>();
        o.write(w);
        clk.tick().await;
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e7w_linear_phase_local_wires(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut a: Bits<8> = Bits::zero();
    loop {
        a = a + Bits::from_lit::<1>();
        clk.tick().await;
        let v: Bits<8> = a + Bits::from_lit::<2>();
        o.write(v);
        clk.tick().await;
        let w: Bits<8> = a + Bits::from_lit::<5>();
        o.write(w);
        clk.tick().await;
    }
}


const E5D_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e5d_two_closing_deferred(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read() + cnt);
        clk.tick().await;
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e5d_two_closing_deferred(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read() + cnt);
        clk.tick().await;
    }
}

const E8_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e8_single_phase_trailing_immediate_read(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read() + cnt);
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e8_single_phase_trailing_immediate_read(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read() + cnt);
    }
}

const E9_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e9_read_in_another_state_opening_write(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        for _ in 0..2 { clk.tick().await; }
        r = x.read();
        for _ in 0..2 { clk.tick().await; }
        o.write(r);
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e9_read_in_another_state_opening_write(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        for _ in 0..2 { clk.tick().await; }
        r = x.read();
        for _ in 0..2 { clk.tick().await; }
        o.write(r);
    }
}


const E2X_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e2x_two_registers_plus_unrelated_read(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut a: Bits<8> = Bits::zero();
    let mut b: Bits<8> = Bits::zero();
    loop {
        a = x.read();
        b = b + Bits::from_lit::<3>();
        for _ in 0..2 { clk.tick().await; }
        o.write(a);
        clk.tick().await;
        o.write(b);
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e2x_two_registers_plus_unrelated_read(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut a: Bits<8> = Bits::zero();
    let mut b: Bits<8> = Bits::zero();
    loop {
        a = x.read();
        b = b + Bits::from_lit::<3>();
        for _ in 0..2 { clk.tick().await; }
        o.write(a);
        clk.tick().await;
        o.write(b);
    }
}

const E5F_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e5f_two_closing_register_free(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    loop {
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read() + Bits::from_lit::<1>());
        clk.tick().await;
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e5f_two_closing_register_free(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    loop {
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read() + Bits::from_lit::<1>());
        clk.tick().await;
    }
}

const E10_SRC: &str = r#"
#[hardware(sequential)]
async fn e10_middle_phase_write_before_update(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        for _ in 0..2 { clk.tick().await; }
        let s = x.read();
        o.write(r);
        r = r + s;
        clk.tick().await;
    }
}
"#;
#[hardware(sequential)]
async fn e10_middle_phase_write_before_update(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop {
        for _ in 0..2 { clk.tick().await; }
        let s = x.read();
        o.write(r);
        r = r + s;
        clk.tick().await;
    }
}

/// Simulate a free-running probe (no inputs), one `Bits<8>` output.
fn sim_e_free<F>(build: impl FnOnce(Clock<C>, Out<Bits<8>, C>) -> copper_sim::HardwareModule<F>) -> Vec<u8>
where
    F: std::future::Future<Output = ()> + 'static,
{
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (p, obs) = wire::<Bits<8>, C>(Bits::zero());
    let dh = p.dirty_handle();
    exec.spawn_wired(build(clk.clone(), p), vec![dh], vec![]);
    (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            obs.read().as_u128() as u8
        })
        .collect()
}

/// The stimulus the input-driven probes share: cycle k drives `x = 10 + k`
/// before its edge (the harness convention), so every edge carries a value the
/// others do not and an off-by-one sample is loud.
fn e_stimulus(k: usize) -> u8 {
    (10 + k) as u8
}

/// Simulate an input-driven probe, one `Bits<8>` input, one `Bits<8>` output.
fn sim_e_in<F>(build: impl FnOnce(Clock<C>, In<Bits<8>, C>, Out<Bits<8>, C>) -> copper_sim::HardwareModule<F>) -> Vec<u8>
where
    F: std::future::Future<Output = ()> + 'static,
{
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (x_drv, x_in) = wire::<Bits<8>, C>(Bits::zero());
    let (p, obs) = wire::<Bits<8>, C>(Bits::zero());
    let dh = p.dirty_handle();
    let reads = vec![x_in.wire_id()];
    exec.spawn_wired(build(clk.clone(), x_in, p), vec![dh], reads);
    (0..CYCLES)
        .map(|k| {
            x_drv.write(Bits::from_usize(e_stimulus(k) as usize));
            exec.tick_clock(&mut clk);
            obs.read().as_u128() as u8
        })
        .collect()
}

/// `transpile_and_run` with the per-cycle `x` stimulus of [`e_stimulus`].
fn transpile_and_run_driven(src: &str, top: &str) -> Vec<u8> {
    let sv = copper_codegen::transpile_source(src, Some(top), &copper_codegen::EmitConfig::default())
        .expect("transpile");
    let dir = std::env::temp_dir().join(format!(
        "copper_fwd_e_{top}_{}_{}",
        std::process::id(),
        TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{top}.sv"));
    std::fs::write(&p, &sv).unwrap();
    let mut tb = format!(
        "#include \"V{top}.h\"\n#include \"verilated.h\"\n#include <iostream>\n\
         int main(int c, char** v) {{ Verilated::commandArgs(c, v);\n\
         V{top}* t = new V{top}(); t->clk = 0; t->eval();\n"
    );
    for k in 0..CYCLES {
        tb.push_str(&format!(
            "t->x = {}; t->clk=0; t->eval(); t->clk=1; t->eval(); std::cout << (int)t->o << std::endl;\n",
            e_stimulus(k)
        ));
    }
    tb.push_str("return 0; }\n");
    let tbp = dir.join("tb.cpp");
    std::fs::write(&tbp, tb).unwrap();
    let out = verilator_command()
        .current_dir(&dir)
        .args(["--cc", "--exe", "--build", "--top-module", top,
               "-Wno-DECLFILENAME", "-Wno-WIDTHEXPAND", "-CFLAGS", "-std=c++14"])
        .arg(std::fs::canonicalize(&p).unwrap())
        .arg(&tbp)
        .output()
        .unwrap();
    assert!(out.status.success(), "verilator build failed:\n{}", String::from_utf8_lossy(&out.stderr));
    let run = std::process::Command::new(dir.join(format!("obj_dir/V{top}"))).output().unwrap();
    let s = String::from_utf8_lossy(&run.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&dir);
    s.lines().filter_map(|l| l.trim().parse::<u8>().ok()).collect()
}

/// The verdict table, as a test: which probe sources the narrowed rule flags.
/// Every DIVERGE row below is flagged and every AGREE row is clean, and the
/// single-phase probes (`e8`, `e9`, `e10`) are outside the rule whatever their
/// verdict. `pulse_plain`, the rule's original witness, is clean now — it is
/// still opted out because `out_first_driven_late` flags it (its writes are
/// constants past a folded tick-bearing loop), not because of this rule.
#[test]
fn phase_e_the_narrowed_rule_flags_exactly_the_diverging_probes() {
    let flagged = |src: &str| {
        let f: syn::ItemFn = syn::parse_str(src).expect("parses");
        copper_analysis::multi_phase_out_write(&f)
    };
    let o = vec!["o".to_string()];
    // AGREE — accepted
    assert!(flagged(PULSE_PLAIN_SRC).is_empty(), "pulse_plain: two constants, no input");
    assert!(flagged(E2_SRC).is_empty(), "e2: two committed registers, no input");
    assert!(flagged(E3_SRC).is_empty(), "e3: forwarded then committed, no input");
    assert!(flagged(E5D_SRC).is_empty(), "e5d: both writes behind a deferred read");
    assert!(flagged(E2X_SRC).is_empty(), "e2x: a read in another state reaches no write (arm-granular, 2026-10-26)");
    // DIVERGE or REFUSED one IR late — flagged
    assert_eq!(flagged(E4_SRC), o, "e4: deferred read then a constant (mixed)");
    assert_eq!(flagged(E5_SRC), o, "e5: trailing immediate read (mixed)");
    assert_eq!(flagged(E6_SRC), o, "e6: deferred read via a wire then a constant (mixed)");
    assert_eq!(flagged(E4I_SRC), o, "e4i: register-free passthrough then a constant");
    assert_eq!(flagged(E5F_SRC), o, "e5f: register-free, both reads passthrough");
    assert_eq!(flagged(E7_SRC), o, "e7: linear route (refused by vlir_lower)");
    assert_eq!(flagged(E7W_SRC), o, "e7w: linear route, phase-local wires");
    // Single-phase: not this rule's business
    assert!(flagged(E8_SRC).is_empty());
    assert!(flagged(E9_SRC).is_empty());
    assert!(flagged(E10_SRC).is_empty());
}

/// AGREE. Two committed registers written in two phases, no input anywhere:
/// both drives are opening on both sides, and phase C emits `o = a` / `o = b`
/// under `pc` guards with a hold register. The derivation's clause (i).
#[test]
fn e2_two_registers_in_two_phases_agrees() {
    let sim = sim_e_free(e2_two_registers);
    assert_eq!(sim, vec![0, 1, 3, 3, 2, 6, 6, 3, 9, 9, 4, 12, 12], "simulator behaviour changed");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(E2_SRC, "e2_two_registers", "clk", "o", "");
    assert_eq!(sim, sv, "sim and SV diverge for register-valued opening drives in two phases — phase C's mux + hold emission regressed");
}

/// AGREE. A forwarded next-state write (`r = r + 1; o.write(r)`) in one phase and
/// the committed register in the next: the mux arm carries `r + 1` for the first
/// and `r` for the second, which is what the simulator shows.
#[test]
fn e3_forwarded_then_committed_write_agrees() {
    let sim = sim_e_free(e3_forwarded_then_committed);
    assert_eq!(sim, vec![0, 1, 1, 1, 2, 2, 2, 3, 3, 3, 4, 4, 4], "simulator behaviour changed");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(E3_SRC, "e3_forwarded_then_committed", "clk", "o", "");
    assert_eq!(sim, sv, "sim and SV diverge for a forwarded-then-committed two-phase write — the forwarded form of the mux arm regressed");
}

/// AGREE. Both writes sit behind a deferred `In` read (a tick follows each) in a
/// module with a register: both closing on both sides, and the edge-sampled
/// `o <= x` / `o <= x + cnt` under `pc` guards shows each at the closing
/// observation exactly as the barrier-parked write does. Clause (ii).
#[test]
fn e5d_two_closing_writes_in_two_phases_agree() {
    let sim = sim_e_in(e5d_two_closing_deferred);
    assert_eq!(sim, vec![0, 0, 12, 12, 15, 15, 15, 17, 17, 21, 21, 21, 22], "simulator behaviour changed");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E5D_SRC, "e5d_two_closing_deferred");
    assert_eq!(sim, sv, "sim and SV diverge for two closing drives in two phases — the edge-sampled form regressed");
}

/// AGREE (since 2026-10-26) — accepted. `e2` plus one input read in the head
/// state, which no write of `o` is reached by. Until the arm-granular opening walk
/// codegen's opening-prefix scan covered the whole extracted `match pc`, so that
/// one read demoted every drive to an edge-sampled register and the SV showed the
/// simulator shifted by one cycle (`[0, 0, 10, 3, 3, 13, …]`). Arm-granular, both
/// drives are opening on both sides and phase C emits `o = a` / `o = b` under `pc`
/// guards with a hold register. The rule's former clause (an input read anywhere
/// while a write is opening) existed only for that limitation and is retired; the
/// only difference from `e2` is `a = x.read()` for `a = a + 1`.
#[test]
fn e2x_an_unrelated_read_in_another_state_agrees() {
    let sim = sim_e_in(e2x_two_registers_plus_unrelated_read);
    assert_eq!(sim, vec![0, 10, 3, 3, 13, 6, 6, 16, 9, 9, 19, 12, 12], "simulator behaviour changed");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E2X_SRC, "e2x_two_registers_plus_unrelated_read");
    assert_eq!(sim, sv, "sim and SV diverge for e2x — the arm-granular opening walk or phase C's mux + hold form regressed (before 2026-10-26 the SV lagged by one cycle); if the lag is back, the retired clause of multi_phase_out_write must come back with it");
}

/// DIVERGE — flagged. A deferred read feeds the first write (closing, agrees at
/// its own observation) and a constant the second (opening): the closing drive
/// drags the whole port into the edge-sampled form and the constant shows one
/// cycle late. The mixed class.
#[test]
fn e4_closing_then_opening_write_diverges_known_gap() {
    let sim = sim_e_in(e4_closing_then_opening);
    assert_eq!(sim, vec![0, 0, 12, 0, 0, 0, 16, 0, 0, 0, 20, 0, 0], "simulator behaviour changed");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E4_SRC, "e4_closing_then_opening");
    assert_eq!(sv, vec![0, 0, 12, 12, 0, 0, 16, 16, 0, 0, 20, 20, 0], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for the mixed class — re-derive multi_phase_out_write and re-bless EXPECTED_MULTI_PHASE");
}

/// DIVERGE — flagged. `e4` with the read routed through a same-segment wire
/// (`let r = x.read(); o.write(r)`): the wire changes nothing, as in V8d.
#[test]
fn e6_closing_via_wire_then_opening_write_diverges_known_gap() {
    let sim = sim_e_in(e6_closing_via_wire_then_opening);
    assert_eq!(sim, vec![0, 0, 12, 0, 0, 0, 16, 0, 0, 0, 20, 0, 0], "simulator behaviour changed");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E6_SRC, "e6_closing_via_wire_then_opening");
    assert_eq!(sv, vec![0, 0, 12, 12, 0, 0, 16, 16, 0, 0, 20, 20, 0], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for e6 — see e4");
}

/// DIVERGE — flagged. `e5d` without its trailing tick: the second read has no
/// tick after it in the iteration, so `classify_reads` makes it IMMEDIATE and the
/// simulator runs the write at the opening instant with `x_N`, while codegen
/// (which sees a read before the write) edge-samples `x_{N+1}`. One tick moved
/// flips `e5d`'s verdict — the flipping pair for clause (ii).
#[test]
fn e5_trailing_immediate_read_makes_the_second_write_opening_diverges_known_gap() {
    let sim = sim_e_in(e5_two_closing);
    assert_eq!(sim, vec![0, 0, 12, 14, 14, 14, 16, 19, 19, 19, 20, 24, 24], "simulator behaviour changed");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E5_SRC, "e5_two_closing");
    assert_eq!(sv, vec![0, 0, 12, 12, 15, 15, 16, 16, 20, 20, 20, 20, 25], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for e5 — a trailing immediate read feeding a plain Out is realized; re-derive clause (ii)");
}

/// DIVERGE — flagged. A register-free module: a read feeding a plain `Out` is a
/// passthrough-immediate read (the D2 rule in `classify_reads`), so the simulator
/// writes `x_N` at the opening while codegen edge-samples `x_{N+1}`. The
/// register-free clause's witness alongside `e5f`.
#[test]
fn e4i_register_free_passthrough_then_opening_write_diverges_known_gap() {
    let sim = sim_e_in(e4i_register_free_passthrough_then_opening);
    assert_eq!(sim, vec![0, 11, 11, 0, 0, 15, 15, 0, 0, 19, 19, 0, 0], "simulator behaviour changed");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E4I_SRC, "e4i_register_free_passthrough_then_opening");
    assert_eq!(sv, vec![0, 0, 12, 12, 0, 0, 16, 16, 0, 0, 20, 20, 0], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for e4i — the passthrough-immediate class is realized; re-derive the register-free clause");
}

/// DIVERGE — flagged. `e5d` with its register removed: the same two writes, now
/// both passthrough-immediate in the simulator. One `let mut` flips `e5d`'s
/// verdict — the flipping pair for the register-free clause.
#[test]
fn e5f_register_free_two_closing_writes_diverge_known_gap() {
    let sim = sim_e_in(e5f_two_closing_register_free);
    assert_eq!(sim, vec![0, 11, 11, 14, 14, 14, 16, 16, 19, 19, 19, 21, 21], "simulator behaviour changed");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E5F_SRC, "e5f_two_closing_register_free");
    assert_eq!(sv, vec![0, 0, 12, 12, 15, 15, 15, 17, 17, 20, 20, 20, 22], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for e5f — see e4i");
}

/// REFUSED one IR late — flagged. The linear route (every tick a bare top-level
/// statement) has no mux form: `vlir_lower` refuses two continuous assigns to one
/// port as multiply driven. The source rule keeps refusing it so the macro and the
/// transpiler agree (front-end parity); the simulator trace is pinned so the
/// shape's meaning is on record for when the linear path learns the mux.
#[test]
fn e7_linear_route_two_phase_write_is_refused_by_the_lowering() {
    let sim = sim_e_free(e7_linear_two_registers);
    assert_eq!(sim, vec![1, 3, 3, 2, 6, 6, 3, 9, 9, 4, 12, 12, 5], "simulator behaviour changed");
    let err = copper_codegen::transpile_source(E7_SRC, Some("e7_linear_two_registers"), &copper_codegen::EmitConfig::default())
        .err()
        .expect("the linear path refuses a port driven in two phases");
    assert!(err.contains("driven in more than one phase"), "unexpected refusal: {err}");
}

/// DIVERGE — flagged. The linear route with phase-local wire values: the hold
/// path (`phase_scoped_output_ports`) registers the port under phase guards, and
/// the opening drives show one cycle after the simulator.
#[test]
fn e7w_linear_route_phase_local_wires_diverge_known_gap() {
    let sim = sim_e_free(e7w_linear_phase_local_wires);
    assert_eq!(sim, vec![3, 6, 6, 4, 7, 7, 5, 8, 8, 6, 9, 9, 7], "simulator behaviour changed");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(E7W_SRC, "e7w_linear_phase_local_wires", "clk", "o", "");
    assert_eq!(sv, vec![0, 3, 6, 6, 4, 7, 7, 5, 8, 8, 6, 9, 9], "transpiled SV behaviour changed");
    assert_eq!(sim[..CYCLES - 1], sv[1..], "the lag is no longer a clean one-cycle shift");
    assert_ne!(sim, sv, "sim and SV now AGREE for e7w — the linear hold path forwards; re-derive the linear clause");
}

// ── Found by the phase-E battery, OUTSIDE the multi-phase rule ────────────────
//
// Two single-phase shapes diverged when the battery landed (2026-10-23) and no
// rule flagged them. One is fixed: `e9` was the statement-granular opening walk
// (any `In` read anywhere in the extracted `match pc` edge-sampled every
// conditional plain-`Out` drive), and the arm-granular walk of 2026-10-26 flips it
// to agreement below. The other, `e8`, is a different mechanism — an immediate
// trailing read feeding the drive — and is pinned as it was measured.

/// DIVERGE — REFUSED since 2026-10-26 (`trailing_out_reads_input`; unguarded when
/// phase E found it). A trailing write `o.write(x.read() + cnt)` after a wait
/// loop: the read has no tick after it, so it is immediate and the simulator
/// shows `x_N + cnt` at the opening observation; codegen edge-samples
/// `x_{N+1} + cnt` one observation later. The `e8l`/`e8lr`/`e8m`/`e8p` probes
/// below scope the rule.
#[test]
fn e8_single_phase_trailing_immediate_read_diverges_refused() {
    let sim = sim_e_in(e8_single_phase_trailing_immediate_read);
    assert_eq!(sim, vec![0, 12, 12, 15, 15, 18, 18, 21, 21, 24, 24, 27, 27], "simulator behaviour changed");
    let f: syn::ItemFn = syn::parse_str(E8_SRC).unwrap();
    assert!(copper_analysis::multi_phase_out_write(&f).is_empty(), "single-phase: not the multi-phase rule's business");
    assert_eq!(copper_analysis::trailing_out_reads_input(&f), vec!["o".to_string()], "the rule must flag e8");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E8_SRC, "e8_single_phase_trailing_immediate_read");
    assert_eq!(sv, vec![0, 0, 13, 13, 16, 16, 19, 19, 22, 22, 25, 25, 28], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for e8 — the immediate-read class is realized; retire trailing_out_reads_input and record it in CYCLE_DATAFLOW_SEMANTICS.md §9.4");
}

/// AGREE (since 2026-10-26). The read is in a DIFFERENT state (`r = x.read()` two
/// ticks earlier) and the write reads only the committed register. Until the
/// arm-granular opening walk the drive was edge-sampled because the whole-`match`
/// scan had seen a read, and the SV lagged the simulator by one cycle
/// (`[0, 0, 0, 0, 12, …]` against `[0, 0, 0, 12, …]`) — so phase C's mux + hold
/// form was reached only by input-free extracted modules. Now the drive is opening
/// in its own arm and the form shows it, exactly as for `trailing_update` (this
/// shape without the read). Single-phase, so outside the multi-phase rule.
#[test]
fn e9_a_read_in_another_state_leaves_an_opening_write_opening_agrees() {
    let sim = sim_e_in(e9_read_in_another_state_opening_write);
    assert_eq!(sim, vec![0, 0, 0, 12, 12, 12, 12, 16, 16, 16, 16, 20, 20], "simulator behaviour changed");
    let f: syn::ItemFn = syn::parse_str(E9_SRC).unwrap();
    assert!(copper_analysis::multi_phase_out_write(&f).is_empty(), "single-phase: not the multi-phase rule's business");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E9_SRC, "e9_read_in_another_state_opening_write");
    assert_eq!(sim, sv, "sim and SV diverge for e9 — the arm-granular opening walk (shir_lower::lower_stmt_list) or phase C's mux + hold form regressed (before 2026-10-26 the SV lagged by one cycle)");
}

/// AGREE. The V8a shape (read; publish; update) in a MIDDLE phase, which neither
/// clause of `pretick_out_write_before_update` covers (head and trailing only).
/// It agrees because in an extracted arm the drive is edge-sampled, and the flop
/// samples the pre-update register exactly as the barrier-parked write reads it;
/// the head-segment V8a diverges only because its drive is a continuous assign.
/// Pinned so the V8 rule's head/trailing scope is on record as sufficient.
#[test]
fn e10_middle_phase_write_before_update_agrees() {
    let sim = sim_e_in(e10_middle_phase_write_before_update);
    assert_eq!(sim, vec![0, 0, 0, 0, 0, 12, 12, 12, 27, 27, 27, 45, 45], "simulator behaviour changed");
    let f: syn::ItemFn = syn::parse_str(E10_SRC).unwrap();
    assert!(copper_analysis::pretick_out_write_before_update(&f).is_empty(), "the V8 rule's scope is head + trailing; this is a middle phase");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E10_SRC, "e10_middle_phase_write_before_update");
    assert_eq!(sim, sv, "sim and SV diverge for a middle-phase write-before-update — the edge-sampled arm form regressed, and the V8 rule needs a middle clause");
}

// ── e8 variants: the trailing-input shapes that scope `trailing_out_reads_input` (2026-10-26) ──

const E8L_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e8l_linear_passthrough(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    loop {
        clk.tick().await;
        o.write(x.read());
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e8l_linear_passthrough(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    loop {
        clk.tick().await;
        o.write(x.read());
    }
}

const E8LR_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e8lr_linear_with_register(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        clk.tick().await;
        o.write(x.read() + cnt);
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e8lr_linear_with_register(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        clk.tick().await;
        o.write(x.read() + cnt);
    }
}

const E8M_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e8m_linear_two_ticks(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        clk.tick().await;
        clk.tick().await;
        o.write(x.read() + cnt);
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e8m_linear_two_ticks(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        clk.tick().await;
        clk.tick().await;
        o.write(x.read() + cnt);
    }
}

const E8P_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn e8p_extracted_passthrough(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    loop {
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn e8p_extracted_passthrough(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    loop {
        for _ in 0..2 { clk.tick().await; }
        o.write(x.read());
    }
}

/// AGREE. The single-tick linear body: the trailing region is the whole phase,
/// the drive is a continuous assign of the immediate read, live every cycle —
/// the D2 passthrough form. Exempt from `trailing_out_reads_input`.
#[test]
fn e8l_single_tick_linear_passthrough_agrees() {
    let sim = sim_e_in(e8l_linear_passthrough);
    assert_eq!(sim, vec![10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22], "simulator behaviour changed");
    let f: syn::ItemFn = syn::parse_str(E8L_SRC).unwrap();
    assert!(copper_analysis::trailing_out_reads_input(&f).is_empty(), "the single-tick body is exempt");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E8L_SRC, "e8l_linear_passthrough");
    assert_eq!(sim, sv, "e8l no longer agrees — the single-tick exemption of trailing_out_reads_input is unsound");
}

/// AGREE. The same with a register in the expression: still one phase, still a
/// continuous assign over the input and the committed register.
#[test]
fn e8lr_single_tick_linear_with_register_agrees() {
    let sim = sim_e_in(e8lr_linear_with_register);
    assert_eq!(sim, vec![11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31, 33, 35], "simulator behaviour changed");
    let f: syn::ItemFn = syn::parse_str(E8LR_SRC).unwrap();
    assert!(copper_analysis::trailing_out_reads_input(&f).is_empty(), "the single-tick body is exempt");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E8LR_SRC, "e8lr_linear_with_register");
    assert_eq!(sim, sv, "e8lr no longer agrees — the single-tick exemption of trailing_out_reads_input is unsound");
}

/// DIVERGE — REFUSED (`trailing_out_reads_input`). The linear TWO-tick route: the
/// trailing drive is emitted as a continuous assign over the input, ungated by
/// the phase, so it tracks `x + cnt` every cycle where the simulator holds the
/// value it computed at the opening observation for the rest of the loop. Not a
/// one-cycle shift but the same family — an immediate read the drive form cannot
/// sample at the right instant.
#[test]
fn e8m_linear_two_ticks_diverges_refused() {
    let sim = sim_e_in(e8m_linear_two_ticks);
    assert_eq!(sim, vec![0, 12, 12, 15, 15, 18, 18, 21, 21, 24, 24, 27, 27], "simulator behaviour changed");
    let f: syn::ItemFn = syn::parse_str(E8M_SRC).unwrap();
    assert_eq!(copper_analysis::trailing_out_reads_input(&f), vec!["o".to_string()], "the rule must flag the linear two-tick shape");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E8M_SRC, "e8m_linear_two_ticks");
    assert_eq!(sv, vec![11, 12, 14, 15, 17, 18, 20, 21, 23, 24, 26, 27, 29], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for e8m — the linear multi-tick trailing drive is phase-gated; re-derive trailing_out_reads_input");
}

/// DIVERGE — REFUSED (`trailing_out_reads_input`). The register-free extracted
/// passthrough: edge-sampled one observation late, like `e8`.
#[test]
fn e8p_extracted_passthrough_diverges_refused() {
    let sim = sim_e_in(e8p_extracted_passthrough);
    assert_eq!(sim, vec![0, 11, 11, 13, 13, 15, 15, 17, 17, 19, 19, 21, 21], "simulator behaviour changed");
    let f: syn::ItemFn = syn::parse_str(E8P_SRC).unwrap();
    assert_eq!(copper_analysis::trailing_out_reads_input(&f), vec!["o".to_string()], "the rule must flag the extracted passthrough");
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run_driven(E8P_SRC, "e8p_extracted_passthrough");
    assert_eq!(sv, vec![0, 0, 12, 12, 14, 14, 16, 16, 18, 18, 20, 20, 22], "transpiled SV behaviour changed");
    // Not a value-preserving shift: the late sample sees the NEXT stimulus value
    // (`x_{N+1}`), so the vectors differ in value as well as in timing, like `e8`.
    assert_ne!(sim, sv, "sim and SV now AGREE for e8p — the immediate-read class is realized; retire trailing_out_reads_input");
}

// ── The D1 re-derivation battery (`d*`, 2026-10-28) ────────────────────────────
//
// `Cfg::unprotected_pretick_out_write` re-derived against the arm-granular
// opening walk (`CYCLE_DATAFLOW_SEMANTICS.md` §10). Same method as phase E:
// every verdict below was written from the two-anchor model before the probe
// ran, then measured. Head-segment plain `Out` drives only, one `Bits<8>` input
// `x` driven `10 + k` on cycle k (`e_stimulus`) where the probe reads one.
//
// The model, per drive W of a plain `Out` in the head segment of a body with
// registers (so every head read is deferred):
//   * simulator anchor on a path: CLOSING iff an `In` read precedes W on that
//     path (the barrier parks the task), OPENING otherwise;
//   * codegen anchor: CLOSING iff some read comb-reaches W on ANY path (the walk
//     is arm-granular but carries a read from any arm past the branch);
//   * port form: driven on every path → `always_comb` (a continuous assign, or a
//     mux whose guards are the COMMITTED registers); driven on some paths → mux +
//     hold when every drive is opening, else an edge-sampled register whose
//     guards take the edge (forwarded) form.
// Two hazards fall out. (A) MIXED ANCHORS: a read reaches some drive of the
// port while some drive has a read-free path — the opening write lands one
// observation late in the SV (or, for a continuous assign, shows the committed
// register where the simulator forwarded one). (B) FORWARDED GUARD: an opening
// drive under a branch whose test reads a register assigned earlier on that
// path — the comb copy tests the committed register, the simulator the
// forwarded one.

/// (d1b) Constant written on SOME paths, the read in the writing arm's own
/// condition, an unprotected register assignment before it. Closing on both
/// sides (edge-sampled, edge-form guard). Predicted AGREE. The pre-2026-10-28
/// rule refused it (constant-write clause).
const D1B_SRC: &str = r#"
#[hardware(sequential)]
async fn d1b_const_read_in_arm(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        if x.read()[0] == Logic::One { o.write(Bits::from_lit::<7>()); }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential)]
async fn d1b_const_read_in_arm(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        if x.read()[0] == Logic::One { o.write(Bits::from_lit::<7>()); }
        clk.tick().await;
    }
}

/// (d1c) Constants written on SOME paths, no input anywhere, guards on the
/// committed state register. All opening → mux + hold. Predicted AGREE. The
/// pre-2026-10-28 rule refused it (constant-write clause: `pc` is assigned unprotected).
const D1C_SRC: &str = r#"
#[hardware(sequential)]
async fn d1c_const_no_read(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut pc: u8 = 0;
    loop {
        match pc {
            0u8 => { pc = 1; }
            1u8 => { o.write(Bits::from_lit::<7>()); pc = 2; }
            2u8 => { o.write(Bits::from_lit::<3>()); pc = 0; }
            _ => {}
        }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential)]
async fn d1c_const_no_read(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut pc: u8 = 0;
    loop {
        match pc {
            0u8 => { pc = 1; }
            1u8 => { o.write(Bits::from_lit::<7>()); pc = 2; }
            2u8 => { o.write(Bits::from_lit::<3>()); pc = 0; }
            _ => {}
        }
        clk.tick().await;
    }
}

/// (d1d) Constants written on EVERY path behind the read. Closing on both
/// sides; a comb mux over the input. Predicted AGREE. Admitted by both the old and the re-derived rule.
const D1D_SRC: &str = r#"
#[hardware(sequential)]
async fn d1d_const_all_paths_read(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        if x.read()[0] == Logic::One { o.write(Bits::from_lit::<7>()); } else { o.write(Bits::from_lit::<3>()); }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential)]
async fn d1d_const_all_paths_read(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        if x.read()[0] == Logic::One { o.write(Bits::from_lit::<7>()); } else { o.write(Bits::from_lit::<3>()); }
        clk.tick().await;
    }
}

/// (d1e) Constants written on EVERY path, no input, committed-state guard.
/// Opening on both sides; a comb mux. Predicted AGREE. Admitted by both the old and the re-derived rule.
const D1E_SRC: &str = r#"
#[hardware(sequential)]
async fn d1e_const_all_paths_no_read(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut pc: u8 = 0;
    loop {
        if pc == 0 { o.write(Bits::from_lit::<7>()); pc = 1; } else { o.write(Bits::from_lit::<3>()); pc = 0; }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential)]
async fn d1e_const_all_paths_no_read(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut pc: u8 = 0;
    loop {
        if pc == 0 { o.write(Bits::from_lit::<7>()); pc = 1; } else { o.write(Bits::from_lit::<3>()); pc = 0; }
        clk.tick().await;
    }
}

/// (d1f) The PATH-DEPENDENT constant, hold form: the read sits on one arm of
/// the first branch only, and the port is written after the merge on the
/// read-free path. Codegen carries the read past the branch (closing, edge-
/// sampled); the simulator runs that write at the opening. Predicted DIVERGE
/// (the SV one observation late). Hazard (A).
const D1F_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn d1f_const_path_dependent_hold(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    loop {
        if phase == 0 { let _ = x.read(); phase = 1; } else { phase = 0; }
        if phase == 0 { o.write(Bits::from_lit::<7>()); }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn d1f_const_path_dependent_hold(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    loop {
        if phase == 0 { let _ = x.read(); phase = 1; } else { phase = 0; }
        if phase == 0 { o.write(Bits::from_lit::<7>()); }
        clk.tick().await;
    }
}

/// (d1g) The path-dependent constant, ALL-PATHS form: a comb mux whose closing
/// arm's write is overwritten by the next iteration's opening write before any
/// observation. Predicted DIVERGE (sim holds 7, SV alternates). Hazard (A).
/// Not refused by the old rule (written on every path); refused by the re-derived one.
const D1G_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn d1g_const_path_dependent_all_paths(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    loop {
        if phase == 0 { let _ = x.read(); phase = 1; } else { phase = 0; }
        if phase == 0 { o.write(Bits::from_lit::<7>()); } else { o.write(Bits::from_lit::<3>()); }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn d1g_const_path_dependent_all_paths(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    loop {
        if phase == 0 { let _ = x.read(); phase = 1; } else { phase = 0; }
        if phase == 0 { o.write(Bits::from_lit::<7>()); } else { o.write(Bits::from_lit::<3>()); }
        clk.tick().await;
    }
}

/// (d2a) MIXED anchors through two drives, hold form, with NO unprotected
/// register assignment: an opening drive guarded by committed state, then a
/// closing drive behind the read. The port is edge-sampled, so the opening
/// write lands one observation late. Predicted DIVERGE. Hazard (A). Not refused by the
/// old rule (every register assignment sits behind the read); refused by the re-derived one.
const D2A_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn d2a_mixed_two_drives_hold(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut pc: u8 = 0;
    loop {
        if pc == 1 { o.write(Bits::from_lit::<5>()); }
        if x.read()[0] == Logic::One { o.write(Bits::from_lit::<9>()); pc = 1; } else { pc = 0; }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn d2a_mixed_two_drives_hold(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut pc: u8 = 0;
    loop {
        if pc == 1 { o.write(Bits::from_lit::<5>()); }
        if x.read()[0] == Logic::One { o.write(Bits::from_lit::<9>()); pc = 1; } else { pc = 0; }
        clk.tick().await;
    }
}

/// (d2b) MIXED anchors through two drives, ALL-PATHS form: one arm writes
/// behind its read, the other writes with no read. Predicted DIVERGE (sim shows
/// the opening arm's value only; the SV mux alternates). Hazard (A). Not
/// refused by the old rule; refused by the re-derived one.
const D2B_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn d2b_mixed_two_drives_all_paths(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    loop {
        if phase == 0 { let _ = x.read(); o.write(Bits::from_lit::<1>()); phase = 1; }
        else { o.write(Bits::from_lit::<2>()); phase = 0; }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn d2b_mixed_two_drives_all_paths(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    loop {
        if phase == 0 { let _ = x.read(); o.write(Bits::from_lit::<1>()); phase = 1; }
        else { o.write(Bits::from_lit::<2>()); phase = 0; }
        clk.tick().await;
    }
}

/// (d2c) d2a with the committed-state drive moved AFTER the read: both drives
/// closing, edge-sampled with edge-form guards. Predicted AGREE. The flipping
/// pair of d2a.
const D2C_SRC: &str = r#"
#[hardware(sequential)]
async fn d2c_two_closing_drives_hold(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut pc: u8 = 0;
    loop {
        if x.read()[0] == Logic::One { o.write(Bits::from_lit::<9>()); pc = 1; } else { pc = 0; }
        if pc == 1 { o.write(Bits::from_lit::<5>()); }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential)]
async fn d2c_two_closing_drives_hold(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut pc: u8 = 0;
    loop {
        if x.read()[0] == Logic::One { o.write(Bits::from_lit::<9>()); pc = 1; } else { pc = 0; }
        if pc == 1 { o.write(Bits::from_lit::<5>()); }
        clk.tick().await;
    }
}

/// (d3a) W4 with the read-free arm assigning only a register the write does
/// NOT read: the opening write publishes the committed `r`, which is what the
/// continuous assign shows. Predicted AGREE. The pre-2026-10-28 rule refused it
/// (register-reading clause: `phase = 0` is an unprotected assignment).
const D3A_SRC: &str = r#"
#[hardware(sequential)]
async fn d3a_w4_unrelated_register(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    let mut r: Bits<8> = Bits::zero();
    loop {
        if phase == 0 { r = x.read(); phase = 1; } else { phase = 0; }
        o.write(r);
        clk.tick().await;
    }
}
"#;
#[hardware(sequential)]
async fn d3a_w4_unrelated_register(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    let mut r: Bits<8> = Bits::zero();
    loop {
        if phase == 0 { r = x.read(); phase = 1; } else { phase = 0; }
        o.write(r);
        clk.tick().await;
    }
}

/// (d3b) W4 with a read on EVERY path: all closing, `assign o = r` shows the
/// value the pre-edge write forwarded. Predicted AGREE. Admitted by both the old and the re-derived rule.
const D3B_SRC: &str = r#"
#[hardware(sequential)]
async fn d3b_w4_read_on_every_path(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    let mut r: Bits<8> = Bits::zero();
    loop {
        if phase == 0 { r = x.read(); phase = 1; } else { let _ = x.read(); r = r + Bits::from_lit::<1>(); phase = 0; }
        o.write(r);
        clk.tick().await;
    }
}
"#;
#[hardware(sequential)]
async fn d3b_w4_read_on_every_path(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    let mut r: Bits<8> = Bits::zero();
    loop {
        if phase == 0 { r = x.read(); phase = 1; } else { let _ = x.read(); r = r + Bits::from_lit::<1>(); phase = 0; }
        o.write(r);
        clk.tick().await;
    }
}

/// (d3c) W4 in the HOLD form: the register-reading write sits under a guard
/// after the path-dependent merge, on the read-free path. Edge-sampled against
/// an opening write. Predicted DIVERGE. Hazard (A); refused by both the old and the re-derived rule.
const D3C_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn d3c_w4_hold(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    let mut r: Bits<8> = Bits::zero();
    loop {
        if phase == 0 { r = x.read(); phase = 1; } else { r = r + Bits::from_lit::<1>(); phase = 0; }
        if phase == 0 { o.write(r); }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn d3c_w4_hold(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut phase: u8 = 0;
    let mut r: Bits<8> = Bits::zero();
    loop {
        if phase == 0 { r = x.read(); phase = 1; } else { r = r + Bits::from_lit::<1>(); phase = 0; }
        if phase == 0 { o.write(r); }
        clk.tick().await;
    }
}

/// (d4a) FORWARDED GUARD, hold form, no input: the opening drive's branch tests
/// a register assigned just before it. The comb copy of the mux tests the
/// committed `cnt`, which never reaches 3. Predicted DIVERGE (the SV never
/// fires). Hazard (B); refused by both the old and the re-derived rule.
const D4A_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn d4a_forwarded_guard_hold(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        if cnt == Bits::from_lit::<3>() { o.write(Bits::from_lit::<7>()); cnt = Bits::zero(); }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn d4a_forwarded_guard_hold(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        if cnt == Bits::from_lit::<3>() { o.write(Bits::from_lit::<7>()); cnt = Bits::zero(); }
        clk.tick().await;
    }
}

/// (d4b) The same pulse with the guard on the COMMITTED register (the update
/// after the test). Opening on both sides, mux + hold. Predicted AGREE. The
/// flipping pair of d4a; the pre-2026-10-28 rule refused it (constant-write clause).
const D4B_SRC: &str = r#"
#[hardware(sequential)]
async fn d4b_committed_guard_hold(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        if cnt == Bits::from_lit::<2>() { o.write(Bits::from_lit::<7>()); cnt = Bits::zero(); }
        else { cnt = cnt + Bits::from_lit::<1>(); }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential)]
async fn d4b_committed_guard_hold(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        if cnt == Bits::from_lit::<2>() { o.write(Bits::from_lit::<7>()); cnt = Bits::zero(); }
        else { cnt = cnt + Bits::from_lit::<1>(); }
        clk.tick().await;
    }
}

/// (d4c) Forwarded guard, ALL-PATHS form: the comb mux tests the committed
/// register. Predicted DIVERGE. Hazard (B); not refused by the old rule, refused by the re-derived one.
const D4C_SRC: &str = r#"
#[hardware(sequential, allow_pretick_alignment)]
async fn d4c_forwarded_guard_all_paths(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        if cnt == Bits::from_lit::<3>() { o.write(Bits::from_lit::<7>()); cnt = Bits::zero(); }
        else { o.write(Bits::from_lit::<3>()); }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential, allow_pretick_alignment)]
async fn d4c_forwarded_guard_all_paths(clk: Clock<C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        if cnt == Bits::from_lit::<3>() { o.write(Bits::from_lit::<7>()); cnt = Bits::zero(); }
        else { o.write(Bits::from_lit::<3>()); }
        clk.tick().await;
    }
}

/// (d4d) Forwarded guard behind a read on every path: closing, edge-sampled
/// with the edge-form guard. Predicted AGREE. Admitted by both the old and the re-derived rule.
const D4D_SRC: &str = r#"
#[hardware(sequential)]
async fn d4d_forwarded_guard_closing(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        let _ = x.read();
        cnt = cnt + Bits::from_lit::<1>();
        if cnt == Bits::from_lit::<3>() { o.write(Bits::from_lit::<7>()); cnt = Bits::zero(); }
        clk.tick().await;
    }
}
"#;
#[hardware(sequential)]
async fn d4d_forwarded_guard_closing(clk: Clock<C>, x: In<Bits<8>, C>, o: Out<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        let _ = x.read();
        cnt = cnt + Bits::from_lit::<1>();
        if cnt == Bits::from_lit::<3>() { o.write(Bits::from_lit::<7>()); cnt = Bits::zero(); }
        clk.tick().await;
    }
}

/// The verdict table, as a test: which probe sources the re-derived rule
/// flags. Every DIVERGE row is flagged and every AGREE row is clean, and no
/// other pre-tick rule claims any `d*` probe (so an admitted shape is admitted
/// outright, and a refused one is refused by this rule alone).
#[test]
fn d_the_rederived_rule_flags_exactly_the_diverging_probes() {
    let d1 = |src: &str| {
        let f: syn::ItemFn = syn::parse_str(src).expect("parses");
        copper_analysis::unprotected_pretick_out_write(&f)
    };
    let o = vec!["o".to_string()];
    // AGREE — admitted
    assert!(d1(PC_ARM_SRC).is_empty(), "pc_arm_write: constant in an arm no read reaches");
    assert!(d1(PC_ARM_TOGGLE_SRC).is_empty(), "pc_arm_toggle: both constants opening");
    assert!(d1(D1B_SRC).is_empty(), "d1b: constant behind its read, held");
    assert!(d1(D1C_SRC).is_empty(), "d1c: constants in state arms, no input");
    assert!(d1(D1D_SRC).is_empty(), "d1d: mux behind the read");
    assert!(d1(D1E_SRC).is_empty(), "d1e: mux on committed state, no input");
    assert!(d1(D2C_SRC).is_empty(), "d2c: two closing drives");
    assert!(d1(D3A_SRC).is_empty(), "d3a: W4 with an unrelated register on the read-free arm");
    assert!(d1(D3B_SRC).is_empty(), "d3b: W4 with a read on every path");
    assert!(d1(D4B_SRC).is_empty(), "d4b: committed guard");
    assert!(d1(D4D_SRC).is_empty(), "d4d: forwarded guard behind a read");
    // DIVERGE — flagged
    assert_eq!(d1(W4_SRC), o, "w4: register-reading continuous assign, path-dependent");
    assert_eq!(d1(D1F_SRC), o, "d1f: path-dependent constant, held");
    assert_eq!(d1(D1G_SRC), o, "d1g: path-dependent constant mux");
    assert_eq!(d1(D2A_SRC), o, "d2a: mixed anchors through two drives, held");
    assert_eq!(d1(D2B_SRC), o, "d2b: mixed anchors through two drives, every path");
    assert_eq!(d1(D3C_SRC), o, "d3c: W4 held");
    assert_eq!(d1(D4A_SRC), o, "d4a: forwarded guard, held");
    assert_eq!(d1(D4C_SRC), o, "d4c: forwarded guard, every path");
    // No other pre-tick rule owns any of them.
    for src in [D1B_SRC, D1C_SRC, D1D_SRC, D1E_SRC, D1F_SRC, D1G_SRC, D2A_SRC, D2B_SRC, D2C_SRC,
                D3A_SRC, D3B_SRC, D3C_SRC, D4A_SRC, D4B_SRC, D4C_SRC, D4D_SRC] {
        let f: syn::ItemFn = syn::parse_str(src).expect("parses");
        assert!(copper_analysis::multi_phase_out_write(&f).is_empty());
        assert!(copper_analysis::pretick_out_write_before_update(&f).is_empty());
        assert!(copper_analysis::trailing_out_reads_input(&f).is_empty());
        assert!(copper_analysis::trailing_regout_reads_input(&f).is_empty());
        assert!(copper_analysis::out_first_driven_late(&f).is_empty());
    }
}

/// AGREE. Constant behind its own read, held: closing on both sides.
#[test]
fn d1b_constant_behind_its_read_agrees() {
    let sim = sim_e_in(d1b_const_read_in_arm);
    assert_eq!(sim, vec![0, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run_driven(D1B_SRC, "d1b_const_read_in_arm");
    assert_eq!(sim, sv, "d1b no longer agrees — a closing held constant is not edge-sampled with its edge-form guard any more");
}

/// AGREE. Constants in state arms, no input: all opening, mux + hold.
#[test]
fn d1c_state_arm_constants_no_input_agree() {
    let sim = sim_e_free(d1c_const_no_read);
    assert_eq!(sim, vec![7, 3, 3, 7, 3, 3, 7, 3, 3, 7, 3, 3, 7], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run(D1C_SRC, "d1c_const_no_read", "clk", "o", "");
    assert_eq!(sim, sv, "d1c no longer agrees — phase C's mux + hold form regressed");
}

/// AGREE. A mux over the input, both arms behind the read in its condition.
#[test]
fn d1d_mux_behind_read_agrees() {
    let sim = sim_e_in(d1d_const_all_paths_read);
    assert_eq!(sim, vec![3, 7, 3, 7, 3, 7, 3, 7, 3, 7, 3, 7, 3], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run_driven(D1D_SRC, "d1d_const_all_paths_read");
    assert_eq!(sim, sv, "d1d no longer agrees");
}

/// AGREE. A mux on committed state, no input, written on every path.
#[test]
fn d1e_mux_on_committed_state_agrees() {
    let sim = sim_e_free(d1e_const_all_paths_no_read);
    assert_eq!(sim, vec![3, 7, 3, 7, 3, 7, 3, 7, 3, 7, 3, 7, 3], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run(D1E_SRC, "d1e_const_all_paths_no_read", "clk", "o", "");
    assert_eq!(sim, sv, "d1e no longer agrees");
}

/// DIVERGE — REFUSED (clause A). The path-dependent constant, held: the SV
/// edge-samples the carried-closing drive one observation after the simulator's
/// opening write; visible once, because the port latches.
#[test]
fn d1f_path_dependent_constant_held_diverges_refused() {
    let sim = sim_e_in(d1f_const_path_dependent_hold);
    assert_eq!(sim, vec![7; CYCLES], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run_driven(D1F_SRC, "d1f_const_path_dependent_hold");
    assert_eq!(sv, vec![0, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for d1f — the path-dependent held constant is realized; re-derive clause A of unprotected_pretick_out_write");
}

/// DIVERGE — REFUSED (clause A). The path-dependent constant as a comb mux: the
/// closing arm's write is overwritten by the next iteration's opening write
/// before any observation, so the simulator holds 7 while the SV mux alternates.
#[test]
fn d1g_path_dependent_constant_mux_diverges_refused() {
    let sim = sim_e_in(d1g_const_path_dependent_all_paths);
    assert_eq!(sim, vec![7; CYCLES], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run_driven(D1G_SRC, "d1g_const_path_dependent_all_paths");
    assert_eq!(sv, vec![3, 7, 3, 7, 3, 7, 3, 7, 3, 7, 3, 7, 3], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for d1g — re-derive clause A of unprotected_pretick_out_write");
}

/// DIVERGE — REFUSED (clause A). Mixed anchors through two drives of a held
/// port, with every register assignment behind the read: the closing drive
/// edge-samples the port, and the opening drive (on committed state) lands one
/// observation late. The simulator's closing write is never observed (the next
/// iteration's opening write overwrites it); the SV shows both. Not refused by
/// the pre-2026-10-28 rule.
#[test]
fn d2a_mixed_two_drives_held_diverges_refused() {
    let sim = sim_e_in(d2a_mixed_two_drives_hold);
    assert_eq!(sim, vec![0, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run_driven(D2A_SRC, "d2a_mixed_two_drives_hold");
    assert_eq!(sv, vec![0, 9, 5, 9, 5, 9, 5, 9, 5, 9, 5, 9, 5], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for d2a — re-derive clause A of unprotected_pretick_out_write");
}

/// DIVERGE — REFUSED (clause A). Mixed anchors through two drives on every
/// path: a comb mux whose closing arm is never observed in the simulator.
#[test]
fn d2b_mixed_two_drives_every_path_diverges_refused() {
    let sim = sim_e_in(d2b_mixed_two_drives_all_paths);
    assert_eq!(sim, vec![2; CYCLES], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run_driven(D2B_SRC, "d2b_mixed_two_drives_all_paths");
    assert_eq!(sv, vec![2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2, 1, 2], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for d2b — re-derive clause A of unprotected_pretick_out_write");
}

/// AGREE. `d2a` with the committed-state drive moved behind the read: two
/// closing drives, edge-sampled with edge-form guards. The flip of `d2a`.
#[test]
fn d2c_two_closing_drives_agree() {
    let sim = sim_e_in(d2c_two_closing_drives_hold);
    assert_eq!(sim, vec![0, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run_driven(D2C_SRC, "d2c_two_closing_drives_hold");
    assert_eq!(sim, sv, "d2c no longer agrees — two closing drives of a held port stopped agreeing");
}

/// AGREE. W4 with the read-free arm assigning only a register the write does
/// not read: the opening write publishes the committed `r`, and so does the
/// continuous assign. The flip of `w4_mixed_alignment`, one assignment apart.
#[test]
fn d3a_w4_unrelated_register_agrees() {
    let sim = sim_e_in(d3a_w4_unrelated_register);
    assert_eq!(sim, vec![10, 10, 12, 12, 14, 14, 16, 16, 18, 18, 20, 20, 22], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run_driven(D3A_SRC, "d3a_w4_unrelated_register");
    assert_eq!(sim, sv, "d3a no longer agrees — an opening continuous-assign drive of a committed register regressed");
}

/// AGREE. W4 with a read on every path: all closing; `assign o = r` shows the
/// value the pre-edge write forwarded.
#[test]
fn d3b_w4_read_on_every_path_agrees() {
    let sim = sim_e_in(d3b_w4_read_on_every_path);
    assert_eq!(sim, vec![10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run_driven(D3B_SRC, "d3b_w4_read_on_every_path");
    assert_eq!(sim, sv, "d3b no longer agrees");
}

/// DIVERGE — REFUSED (clause A). W4 in the held form: the register-reading
/// write under a guard after the path-dependent merge, edge-sampled against the
/// simulator's opening write of the forwarded `r + 1`.
#[test]
fn d3c_w4_held_diverges_refused() {
    let sim = sim_e_in(d3c_w4_hold);
    assert_eq!(sim, vec![11, 11, 13, 13, 15, 15, 17, 17, 19, 19, 21, 21, 23], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run_driven(D3C_SRC, "d3c_w4_hold");
    assert_eq!(sv, vec![0, 11, 11, 13, 13, 15, 15, 17, 17, 19, 19, 21, 21], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for d3c — re-derive clause A of unprotected_pretick_out_write");
}

/// DIVERGE — REFUSED (clause B). An opening held drive whose guard tests a
/// register assigned just before it: the mux's comb copy tests the committed
/// `cnt` (never 3) and only the hold register, loaded under the edge-form guard,
/// catches the pulse — one observation late.
#[test]
fn d4a_forwarded_guard_held_diverges_refused() {
    let sim = sim_e_free(d4a_forwarded_guard_hold);
    assert_eq!(sim, vec![0, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run(D4A_SRC, "d4a_forwarded_guard_hold", "clk", "o", "");
    assert_eq!(sv, vec![0, 0, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for d4a — the comb copy of an opening drive's guard takes the edge form; re-derive clause B of unprotected_pretick_out_write");
}

/// AGREE. The same pulse with the update after the test: the guard reads the
/// committed register on both sides. The flip of `d4a`.
#[test]
fn d4b_committed_guard_pulse_agrees() {
    let sim = sim_e_free(d4b_committed_guard_hold);
    assert_eq!(sim, vec![0, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run(D4B_SRC, "d4b_committed_guard_hold", "clk", "o", "");
    assert_eq!(sim, sv, "d4b no longer agrees — phase C's mux + hold form regressed");
}

/// DIVERGE — REFUSED (clause B). The forwarded guard over a comb mux written on
/// every path: no hold register to catch it, so the SV never shows the pulse.
#[test]
fn d4c_forwarded_guard_mux_diverges_refused() {
    let sim = sim_e_free(d4c_forwarded_guard_all_paths);
    assert_eq!(sim, vec![3, 7, 3, 3, 7, 3, 3, 7, 3, 3, 7, 3, 3], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run(D4C_SRC, "d4c_forwarded_guard_all_paths", "clk", "o", "");
    assert_eq!(sv, vec![3; CYCLES], "transpiled SV behaviour changed");
    assert_ne!(sim, sv, "sim and SV now AGREE for d4c — re-derive clause B of unprotected_pretick_out_write");
}

/// AGREE. The forwarded guard behind a read on every path: the drive is closing,
/// edge-sampled with the edge-form guard. The flip of `d4a` by anchor.
#[test]
fn d4d_forwarded_guard_behind_read_agrees() {
    let sim = sim_e_in(d4d_forwarded_guard_closing);
    assert_eq!(sim, vec![0, 0, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7, 7], "simulator behaviour changed");
    if !verilator_available() { return; }
    let sv = transpile_and_run_driven(D4D_SRC, "d4d_forwarded_guard_closing");
    assert_eq!(sim, sv, "d4d no longer agrees — a closing drive's edge-form guard regressed");
}

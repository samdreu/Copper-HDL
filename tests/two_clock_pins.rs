//! The two two-clock pins Phase C step 2 owes (SPEC §III.2.6, D-17, D-20 obligation
//! (2)). The sweep drives one clock, so the multi-domain cases are hand-written:
//!
//! - **(i) the never-ticked startup** (D-17 probe (i), SPEC §I.12.2, P11): a module
//!   in `ClkB`, with another domain ticked three times before `ClkB`'s first edge and
//!   the module's input changed in between. The module's coroutine must have run
//!   nothing before `ClkB`'s first settle, so after `ClkB`'s edge 1 it shows the
//!   input standing before that edge (`q = 2`), as the transpiled flop does and as
//!   the reference interpreter does. Three legs, through the relation harness.
//! - **(vi) two interleavings** (D-17 probe (vi), SPEC §I.12.1, P11):
//!   `two_domain_counter`'s hierarchy under P = [2;10] and Q = [1,3,1,3,2,2,3,1,2,2]
//!   (fast ticks per slow tick), the simulator against the transpiled two-clock
//!   hierarchy under Verilator, per domain: the fast trace per fast edge and the slow
//!   trace per slow edge.
//!
//! Both run under each resumption mode (`ResumptionMode`, `COPPER_RESUMPTION`), set
//! explicitly per executor, so the pins state both today's two-job behaviour and the
//! one-resumption protocol, whatever the compiled default. Each known divergence is
//! pinned as measured: when the default flips (Phase C step 3) the `Two` arm is the
//! old path and the `One` arm is the language.

mod common;

use common::{verilator_available, EquivalenceTest, Pattern};
use copper_core::port::{registered_wire, wire, In, Out, RegOut};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;
use copper_sim::{HardwareExecutor, ResumptionMode};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

struct ClkA;
impl ClockDomain for ClkA {}
struct ClkB;
impl ClockDomain for ClkB {}

include!("ref_probes/two_clock_probes.rs");
const PROBES_SRC: &str = include_str!("ref_probes/two_clock_probes.rs");

static TMP_NONCE: AtomicUsize = AtomicUsize::new(0);

// ── (i) the never-ticked startup ───────────────────────────────────────────────

thread_local! {
    /// Statement executions of `b_counted`'s loop head (a side-effect counter;
    /// sim only — the transpiled module is `b_reg`, the same body without it).
    static RESUMPTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[hardware(sequential)]
async fn b_counted(clk: Clock<ClkB>, x: In<Bits<8>, ClkB>, q: RegOut<Bits<8>, ClkB>) {
    loop {
        RESUMPTIONS.with(|c| c.set(c.get() + 1));
        let t = x.read();
        q.write(t);
        clk.tick().await;
    }
}

/// A native task for `ClkA`'s settles: a free-running registered counter.
#[hardware(sequential)]
async fn a_counter(clk: Clock<ClkA>, q: RegOut<Bits<8>, ClkA>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        n = n + Bits::from_lit::<1>();
        q.write(n);
        clk.tick().await;
    }
}

/// The schedule of D-17 (i): x = 1; A ×3; x = 2; B; x = 3; B; x = 4; B. Returns the
/// resumption count after each step and the B-domain trace (obs_0, obs_1, …) with
/// the input each B instant saw, recorded through the relation harness.
fn never_ticked(mode: ResumptionMode) -> (Vec<usize>, Vec<u8>, common::Verdicts) {
    RESUMPTIONS.with(|c| c.set(0));
    let mut eq = EquivalenceTest::differential_only("b_reg", PROBES_SRC, Some("b_reg"))
        .with_reference_interpreter();
    let mut clk_a = Clock::<ClkA>::new();
    let mut clk_b = Clock::<ClkB>::new();
    let mut exec = HardwareExecutor::new().with_resumption_mode(mode);
    let (x_drv, x_in) = wire::<Bits<8>, ClkB>(Bits::zero());
    let (q_out, q_obs) = registered_wire::<Bits<8>, ClkB>(&clk_b, Bits::zero());
    let dh = q_out.dirty_handle();
    exec.spawn_wired(b_counted(clk_b.clone(), x_in.clone(), q_out), vec![dh], vec![x_in.wire_id()]);
    let (a_out, _a_obs) = registered_wire::<Bits<8>, ClkA>(&clk_a, Bits::zero());
    let dha = a_out.dirty_handle();
    exec.spawn_wired(a_counter(clk_a.clone(), a_out), vec![dha], vec![]);

    let count = || RESUMPTIONS.with(|c| c.get());
    let mut counts = vec![count()];
    x_drv.write(Bits::from_u8(1));
    for _ in 0..3 {
        exec.tick_clock(&mut clk_a);
        counts.push(count());
    }
    let mut trace = Vec::new();
    for (k, x) in [2u8, 3, 4].into_iter().enumerate() {
        x_drv.write(Bits::from_u8(x));
        let xb = Bits::<8>::from_u8(x);
        if k == 0 {
            exec.pre_edge_settle::<ClkB>();
            let q0 = q_obs.read();
            trace.push(q0.as_u8());
            eq.record_observation_zero(&[("q", &q0[..])]);
            exec.post_edge_settle(&mut clk_b);
        } else {
            exec.tick_clock(&mut clk_b);
        }
        counts.push(count());
        let q = q_obs.read();
        trace.push(q.as_u8());
        eq.record_differential(&[("x", &xb[..])], &[("q", &q[..])]);
    }
    assert_eq!(exec.cycle::<ClkA>(), 3, "the per-domain counter counts A's edges only");
    assert_eq!(exec.cycle::<ClkB>(), 3, "the per-domain counter counts B's edges only");
    (counts, trace, eq.finish_report())
}

#[test]
fn never_ticked_startup_one_resumption_agrees_on_all_three_legs() {
    if !verilator_available() {
        panic!("Verilator is absent: this pin needs the SV leg and cannot be skipped silently");
    }
    let (counts, trace, v) = never_ticked(ResumptionMode::One);
    // B's coroutine ran nothing during A's three ticks (SPEC §I.12.2), then once per B tick.
    assert_eq!(counts, vec![0, 0, 0, 0, 1, 2, 3], "resumption counts under One");
    // obs_0 = init(q); obs_1 = x_1 of B = 2 (the input standing before B's edge 1).
    assert_eq!(trace, vec![0, 2, 3, 4], "B's trace under One");
    assert!(v.transpiled, "sim ≡ SV under One:\n{}", v.report);
    assert!(matches!(v.interpreter, Some(Ok(_))), "sim ≡ ref under One: {:?}\n{}", v.interpreter, v.report);
    assert_eq!(v.pattern, None, "{}", v.report);
}

/// Today's two-job executor, pinned as the known divergence D-17 (i) measured: B's
/// first resumption runs in A's first settle and reads x = 1, so `q = 1` after B's
/// edge 1 where the flop and the reference show 2. Pattern sim≠ref, SV=ref (a
/// simulator bug, SPEC §III.2.8). When the default flips, this arm documents the
/// old path; delete it with the `Two` executor.
#[test]
fn never_ticked_startup_two_job_is_the_pinned_divergence() {
    if !verilator_available() {
        panic!("Verilator is absent: this pin needs the SV leg and cannot be skipped silently");
    }
    let (counts, trace, v) = never_ticked(ResumptionMode::Two);
    assert_eq!(counts, vec![0, 1, 1, 1, 2, 3, 4], "resumption counts under Two (first one in A's settle)");
    assert_eq!(trace, vec![0, 1, 3, 4], "B's trace under Two: q = 1 after B's edge 1");
    assert!(!v.transpiled, "sim ≠ SV under Two (the startup divergence)");
    assert!(matches!(v.interpreter, Some(Err(_))), "sim ≠ ref under Two: {:?}", v.interpreter);
    assert_eq!(v.pattern, Some(Pattern::SimNeRefSvEqRef), "{}", v.report);
    // obs_1 is recorded cycle 0: every failing line, on either leg, names it.
    let fails: Vec<&str> = v.report.lines().filter(|l| l.starts_with("FAIL: Cycle")).collect();
    assert!(!fails.is_empty(), "{}", v.report);
    assert!(
        fails.iter().all(|l| l.starts_with("FAIL: Cycle 0 ")),
        "the divergence is at obs_1 only:\n{}",
        v.report
    );
    v.record_pin("b_reg: never-ticked startup under the two-job executor (D-17 (i), P11)");
}

// ── (vi) two interleavings on two_domain_counter ──────────────────────────────

// Kept identical to `examples/cdc/two_domain_hierarchy.rs`; the transpiler reads
// that file from disk, so the two never silently drift (the same arrangement as
// `tests/two_domain_hierarchy_cdc.rs`).
#[hardware(sequential)]
async fn fast_counter(clk: Clock<ClkA>, count_out: Out<Bits<8>, ClkA>, flag_out: Out<Logic, ClkA>) {
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

#[hardware(synchronizer)]
async fn flag_sync(clk: Clock<ClkB>, d: In<Logic, ClkA>, q: Out<Logic, ClkB>) {
    let mut ff1 = Logic::Zero;
    let mut ff2 = Logic::Zero;
    loop {
        q.write(ff2);
        clk.tick().await;
        ff2 = ff1;
        ff1 = d.read();
    }
}

#[hardware(sequential)]
async fn slow_consumer(clk: Clock<ClkB>, flag_in: In<Logic, ClkB>, out: Out<Logic, ClkB>) {
    loop {
        out.write(flag_in.read());
        clk.tick().await;
    }
}

const DESIGN_FILE: &str = "examples/cdc/two_domain_hierarchy.rs";
const P: [usize; 10] = [2; 10];
const Q: [usize; 10] = [1, 3, 1, 3, 2, 2, 3, 1, 2, 2];
/// Not D-17's: a third interleaving with slow periods of zero fast ticks, which the
/// cancellation below depends on.
const R: [usize; 14] = [3, 0, 2, 0, 3, 0, 1, 2, 0, 0, 3, 0, 1, 2];

/// Per-domain traces: `count_out` after every fast edge, `flag_sync_out` (the
/// consumer) after every slow edge.
#[derive(Debug, PartialEq, Eq, Clone)]
struct Traces {
    fast: Vec<u8>,
    slow: Vec<u8>,
}

fn sim_traces(mode: ResumptionMode, sched: &[usize]) -> Traces {
    let mut clk_a = Clock::<ClkA>::new();
    let mut clk_b = Clock::<ClkB>::new();
    let mut exec = HardwareExecutor::new().with_resumption_mode(mode);
    let (count_port, count_obs) = wire::<Bits<8>, ClkA>(Bits::zero());
    let (flag_port, flag_in) = wire::<Logic, ClkA>(Logic::Zero);
    let (sync_q_port, sync_in) = wire::<Logic, ClkB>(Logic::Zero);
    let (consumer_port, consumer_obs) = wire::<Logic, ClkB>(Logic::Zero);
    let dh = (
        count_port.dirty_handle(),
        flag_port.dirty_handle(),
        sync_q_port.dirty_handle(),
        consumer_port.dirty_handle(),
    );
    let sync_reads = vec![flag_in.wire_id()];
    let cons_reads = vec![sync_in.wire_id()];
    exec.spawn_wired(fast_counter(clk_a.clone(), count_port, flag_port), vec![dh.0, dh.1], vec![]);
    exec.spawn_wired(flag_sync(clk_b.clone(), flag_in, sync_q_port), vec![dh.2], sync_reads);
    exec.spawn_wired(slow_consumer(clk_b.clone(), sync_in, consumer_port), vec![dh.3], cons_reads);
    let mut t = Traces { fast: Vec::new(), slow: Vec::new() };
    for &k in sched {
        for _ in 0..k {
            exec.tick_clock(&mut clk_a);
            t.fast.push(count_obs.read().as_u128() as u8);
        }
        exec.tick_clock(&mut clk_b);
        t.slow.push(u8::from(consumer_obs.read() == Logic::One));
    }
    assert_eq!(exec.cycle::<ClkA>() as usize, sched.iter().sum::<usize>());
    assert_eq!(exec.cycle::<ClkB>() as usize, sched.len());
    t
}

/// The transpiled hierarchy (`two_domain_top`, `wr_clk` = fast, `rd_clk` = slow)
/// under Verilator, the same schedule, the same per-domain observations: the
/// testbench prints them and the test parses them, so the SV trace is measured, not
/// asserted in C++.
fn sv_traces(sched: &[usize]) -> Traces {
    let src = std::fs::read_to_string(DESIGN_FILE).expect("read example source");
    let sv = copper_codegen::transpile_source_hierarchy(
        &src,
        Some("two_domain_top"),
        &copper_codegen::EmitConfig::default(),
    )
    .expect("transpile hierarchy");
    let work = std::env::temp_dir().join(format!(
        "copper_two_clock_pins_{}_{}",
        std::process::id(),
        TMP_NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    let sv_path = work.join("two_domain_top.sv");
    std::fs::write(&sv_path, &sv).unwrap();
    let mut tb = String::new();
    tb.push_str("#include \"Vtwo_domain_top.h\"\n#include \"verilated.h\"\n#include <iostream>\n\n");
    tb.push_str("int main(int argc, char** argv) {\n    Verilated::commandArgs(argc, argv);\n");
    tb.push_str("    Vtwo_domain_top* top = new Vtwo_domain_top();\n");
    tb.push_str("    top->wr_clk = 0; top->rd_clk = 0; top->eval();\n");
    for &k in sched {
        for _ in 0..k {
            tb.push_str("    top->wr_clk = 0; top->eval(); top->wr_clk = 1; top->eval();\n");
            tb.push_str("    std::cout << \"F \" << (int)top->count_out << std::endl;\n");
        }
        tb.push_str("    top->rd_clk = 0; top->eval(); top->rd_clk = 1; top->eval();\n");
        tb.push_str("    std::cout << \"S \" << (int)top->flag_sync_out << std::endl;\n");
    }
    tb.push_str("    delete top;\n    return 0;\n}\n");
    let tb_path = work.join("tb_two_domain_top.cpp");
    std::fs::write(&tb_path, tb).unwrap();
    let out = common::verilator_command()
        .current_dir(&work)
        .args([
            "--cc", "--exe", "--build", "--top-module", "two_domain_top", "-Wall",
            "-Wno-DECLFILENAME", "-CFLAGS", "-std=c++14",
        ])
        .arg(&sv_path)
        .arg(&tb_path)
        .output()
        .expect("spawn verilator");
    assert!(
        out.status.success(),
        "verilator build failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let run = Command::new(work.join("obj_dir").join("Vtwo_domain_top")).output().expect("run model");
    assert!(run.status.success(), "model run failed");
    let mut t = Traces { fast: Vec::new(), slow: Vec::new() };
    for line in String::from_utf8_lossy(&run.stdout).lines() {
        let (tag, v) = line.split_once(' ').expect("tagged line");
        let v: u8 = v.trim().parse().expect("value");
        match tag {
            "F" => t.fast.push(v),
            "S" => t.slow.push(v),
            _ => panic!("unexpected line {line:?}"),
        }
    }
    let _ = std::fs::remove_dir_all(&work);
    t
}

/// The interleaving changes nothing a domain can see on its own: the fast trace per
/// fast edge is one sequence, whatever the slow clock does (P11, SPEC §I.12.1), and
/// the slow trace per slow edge is the same under P and Q because the flag standing
/// at each slow edge is (D-17 (vi), measured on the two-job executor 2026-11-05).
#[test]
fn interleavings_give_one_trace_per_domain_under_both_modes() {
    for mode in [ResumptionMode::Two, ResumptionMode::One] {
        let p = sim_traces(mode, &P);
        let q = sim_traces(mode, &Q);
        let n = p.fast.len().min(q.fast.len());
        assert_eq!(p.fast[..n], q.fast[..n], "{mode:?}: fast trace per fast edge");
        assert_eq!(p.slow, q.slow, "{mode:?}: slow trace per slow edge");
        println!("{mode:?}: fast P {:?}", p.fast);
        println!("{mode:?}: slow P {:?} Q {:?}", p.slow, q.slow);
    }
}

/// The simulator against the transpiled two-clock hierarchy under P and Q, per
/// domain. Under the two-job executor the two agree (the 2:1 case is the anchored
/// `two_domain_hierarchy_cdc`). Under one resumption the design's modules are still
/// in the two-job spelling — `fast_counter`'s `count_out`/`flag_out` and
/// `flag_sync`'s `q` and `slow_consumer`'s `out` are plain `Out`s written at the
/// loop head after a tick — and the macro step (Phase C step 3) rewrites them; the
/// arm measures what the one-resumption executor does with them as they stand and
/// pins it.
#[test]
fn interleavings_sim_against_the_transpiled_hierarchy() {
    if !verilator_available() {
        panic!("Verilator is absent: this pin needs the SV leg and cannot be skipped silently");
    }
    for (name, sched) in [("P", &P[..]), ("Q", &Q[..]), ("R", &R[..])] {
        let sv = sv_traces(sched);
        let two = sim_traces(ResumptionMode::Two, sched);
        assert_eq!(two, sv, "Two, interleaving {name}: sim ≡ SV per domain");
        let one = sim_traces(ResumptionMode::One, sched);
        println!("interleaving {name}: SV fast {:?}\n  One fast {:?}\n  SV slow {:?}\n  One slow {:?}", sv.fast, one.fast, sv.slow, one.slow);
        // The two-job spelling under one resumption: a plain-`Out` write at the loop
        // head after the tick runs at t_{N+1}⁻ and is visible from obs_{N+1}, one
        // observation after the emitted `assign` of the register shows it.
        let lagged: Vec<u8> = std::iter::once(0).chain(sv.fast.iter().copied()).take(sv.fast.len()).collect();
        assert_eq!(one.fast, lagged, "One, {name}: count_out lags the SV by one fast observation");
        // Measured 2026-11-06 (predicted to differ under P and Q; it does not): the
        // slow chain agrees under P and Q BY CANCELLATION. `flag_out` shows one fast
        // observation late (the head write above), and `flag_sync`'s trailing
        // `d.read()` now runs at the slow t_{N+1}⁻, after the period's fast ticks,
        // instead of at the slow t_N⁺ before them — one fast tick later on each side
        // when the period has at least one fast tick. R has empty periods, and there
        // the two do not cancel: the synchronized flag rises one slow observation late.
        // The same design agreed by cancellation once before (the module comment of
        // `tests/two_domain_hierarchy_cdc.rs`); step 3's rewrite removes both terms.
        let slow_lagged: Vec<u8> =
            std::iter::once(0).chain(sv.slow.iter().copied()).take(sv.slow.len()).collect();
        match name {
            "R" => assert_eq!(one.slow, slow_lagged, "One, R: the slow chain is one slow observation late"),
            _ => assert_eq!(one.slow, sv.slow, "One, {name}: the slow chain (agreement by cancellation)"),
        }
    }
}

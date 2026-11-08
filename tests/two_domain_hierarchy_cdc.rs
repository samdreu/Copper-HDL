//! Item 4 — dual-clock hierarchy: Verilator equivalence + independent-SV anchor.
//!
//! Fills the G1 pattern-5 (CDC) gap. Three things are cross-checked on the same
//! dual-clock design (`examples/cdc/two_domain_hierarchy.rs`), under a custom
//! **two-clock** testbench (the built-in `verify_with_verilator` is single-clock):
//!
//!   1. the Copper **simulator** trace (ground truth for Copper's semantics),
//!   2. the **transpiled hierarchy** (`two_domain_top` + co-emitted children) run
//!      under Verilator — the sim↔transpile equivalence on the real hierarchy,
//!   3. an **independent hand-written SystemVerilog reference**
//!      (`examples/cdc/sv/two_domain_hierarchy.sv`, `two_domain_ref`) — the
//!      non-circular anchor: Copper's CDC timing vs an outside implementation.
//!
//! **Repaired 2026-08-21 — this anchor used to agree by COINCIDENCE.** Two silent
//! sim ≠ SV divergences cancelled inside the chain, so all three views matched at the
//! boundary without the chain being right:
//!
//!   * `fast_counter` asserted its flag one cycle EARLY — its sticky `latched` was
//!     updated in the pre-tick segment with no input read to pin that segment's clock
//!     phase (D1). Fixed by moving the update after the tick, the form measured to
//!     match the independent reference.
//!   * `slow_consumer` lagged one cycle LATE — its *leading* read is classified
//!     `Deferred` and samples at the pre-edge, while its producer updates at the
//!     post-edge, where the transpiled `assign out = flag_in` is immediate (D2).
//!     Fixed by reading after the tick, which classifies `Immediate`.
//!
//! Both are corrected in the design now, so the three views agree **for the right
//! reason** rather than by cancellation. Verified: correcting only one of them moved
//! the boundary 5 → 6 and broke this test.
//!
//! The underlying defects are still tracked — D1 is guarded at compile time
//! (`copper_analysis::unprotected_pretick_out_write`), D2 is not, and the analysis
//! behind both is `design_docs/PRETICK_ALIGNMENT_GUARDRAIL.md`. The pinned
//! measurements live in `tests/sequential_forwarding_divergence.rs`.
//!
//! If Verilator is not installed the Verilator arms are skipped (the sim arm and
//! its invariant checks still run), mirroring the rest of the suite.

use copper_core::port::{registered_wire, wire, In, Out, RegOut};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;
use copper_sim::HardwareExecutor;

mod common;
use common::verilator_available;
use std::path::Path;
use std::process::Command;

/// Per-invocation nonce for temporary directories. Two tests in one binary can
/// transpile or Verilate the same top module at the same moment, and a directory
/// keyed on the process id alone is then shared: one test's cleanup deletes the
/// file the other's Verilator is about to read (seen on a 96-core host,
/// 2026-09-03). Same rule as the Verilator work dir in CLAUDE.md.
static TMP_NONCE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

// ── The design (real modules, for the simulator) ──────────────────────────────
// Kept identical to `examples/cdc/two_domain_hierarchy.rs`; the transpiler reads
// that example file from disk (see DESIGN_FILE) so the two never silently drift.

struct ClkFast;
impl ClockDomain for ClkFast {}
struct ClkSlow;
impl ClockDomain for ClkSlow {}

#[hardware(sequential)]
async fn fast_counter(
    clk: Clock<ClkFast>,
    count_out: RegOut<Bits<8>, ClkFast>,
    flag_out: RegOut<Logic, ClkFast>,
) {
    // One job per cycle (D-8): the resumption that ends at fast edge N+1 updates
    // the counter and the sticky flag and writes both `RegOut`s, which show from
    // obs_{N+1} — `count_o = cnt` and `flag_o = latch` of the reference.
    let mut count: Bits<8> = Bits::zero();
    let mut latched = Logic::Zero;
    loop {
        if count[3] == Logic::One {
            latched = Logic::One;
        }
        count = count + Bits::from_lit::<1>();
        count_out.write(count);
        flag_out.write(latched);
        clk.tick().await;
    }
}

#[hardware(synchronizer)]
async fn flag_sync(clk: Clock<ClkSlow>, d: In<Logic, ClkFast>, q: RegOut<Logic, ClkSlow>) {
    // `copper::sync_2ff`'s one-job spelling: `ff1` the first flop, the `RegOut`
    // `q` the second, `ff2` a wire of the resumption.
    let mut ff1 = Logic::Zero;
    loop {
        let ff2 = ff1;
        ff1 = d.read();
        q.write(ff2);
        clk.tick().await;
    }
}

#[hardware(combinational)]
fn slow_consumer(flag_in: In<Logic, ClkSlow>, out: Out<Logic, ClkSlow>) {
    // A passthrough is a wire, so it is a combinational module (D-8: a clocked
    // body has no plain `Out`); the reference's `flag_sync_out` is the
    // synchronizer's `q` itself.
    out.write(flag_in.read());
}

/// (count_out, flag_sync_out) observed each slow cycle under a `fast_per_slow`
/// interleaving. Ground-truth Copper semantics.
fn sim_trace(fast_per_slow: usize, slow_cycles: usize) -> Vec<(u8, u8)> {
    let mut clk_fast = Clock::<ClkFast>::new();
    let mut clk_slow = Clock::<ClkSlow>::new();
    let mut exec = HardwareExecutor::new();

    let (count_port, count_obs) = registered_wire::<Bits<8>, ClkFast>(&clk_fast, Bits::zero());
    let (flag_port, flag_to_sync) = registered_wire::<Logic, ClkFast>(&clk_fast, Logic::Zero);
    let (sync_q_port, sync_to_consumer) = registered_wire::<Logic, ClkSlow>(&clk_slow, Logic::Zero);
    let (consumer_port, consumer_obs) = wire::<Logic, ClkSlow>(Logic::Zero);

    let dh_count = count_port.dirty_handle();
    let dh_flag = flag_port.dirty_handle();
    let dh_sync_q = sync_q_port.dirty_handle();
    let dh_consumer = consumer_port.dirty_handle();

    let sync_reads = vec![flag_to_sync.wire_id()];
    let consumer_reads = vec![sync_to_consumer.wire_id()];
    exec.spawn_wired(fast_counter(clk_fast.clone(), count_port, flag_port), vec![dh_count, dh_flag], vec![]);
    exec.spawn_wired(flag_sync(clk_slow.clone(), flag_to_sync, sync_q_port), vec![dh_sync_q], sync_reads);
    exec.spawn_wired(slow_consumer(sync_to_consumer, consumer_port), vec![dh_consumer], consumer_reads);

    let mut trace = Vec::with_capacity(slow_cycles);
    for _ in 0..slow_cycles {
        for _ in 0..fast_per_slow {
            exec.tick_clock(&mut clk_fast);
        }
        exec.tick_clock(&mut clk_slow);
        let count = count_obs.read().as_u128() as u8;
        let flag = if consumer_obs.read() == Logic::One { 1 } else { 0 };
        trace.push((count, flag));
    }
    trace
}

const DESIGN_FILE: &str = "examples/cdc/two_domain_hierarchy.rs";
const REFERENCE_SV: &str = "examples/cdc/sv/two_domain_hierarchy.sv";

/// Generate a self-checking dual-clock C++ testbench: for each slow cycle, tick
/// `wr_clk` `fast_per_slow` times, then `rd_clk` once, then check `count_out` /
/// `flag_sync_out` against `expected`. Returns non-zero on any mismatch.
fn dual_clock_testbench(top: &str, fast_per_slow: usize, expected: &[(u8, u8)]) -> String {
    let mut tb = String::new();
    tb.push_str(&format!("#include \"V{top}.h\"\n#include \"verilated.h\"\n#include <iostream>\n\n"));
    tb.push_str("int main(int argc, char** argv) {\n");
    tb.push_str("    Verilated::commandArgs(argc, argv);\n");
    tb.push_str(&format!("    V{top}* top = new V{top}();\n"));
    tb.push_str("    int failures = 0;\n");
    // Settle initial state.
    tb.push_str("    top->wr_clk = 0; top->rd_clk = 0; top->eval();\n\n");
    for (i, (count, flag)) in expected.iter().enumerate() {
        tb.push_str(&format!("    // slow cycle {i}\n"));
        for _ in 0..fast_per_slow {
            tb.push_str("    top->wr_clk = 0; top->eval(); top->wr_clk = 1; top->eval();\n");
        }
        tb.push_str("    top->rd_clk = 0; top->eval(); top->rd_clk = 1; top->eval();\n");
        tb.push_str(&format!(
            "    if (top->count_out != {count}) {{ std::cout << \"FAIL cycle {i} count_out exp {count} got \" << (int)top->count_out << std::endl; failures++; }}\n"
        ));
        tb.push_str(&format!(
            "    if (top->flag_sync_out != {flag}) {{ std::cout << \"FAIL cycle {i} flag_sync_out exp {flag} got \" << (int)top->flag_sync_out << std::endl; failures++; }}\n"
        ));
    }
    tb.push_str("    delete top;\n");
    tb.push_str("    if (failures == 0) { std::cout << \"All tests passed!\" << std::endl; return 0; }\n");
    tb.push_str("    std::cout << failures << \" failure(s)\" << std::endl; return 1;\n");
    tb.push_str("}\n");
    tb
}



/// Verilate `sv_file` with `top`, build against `tb_src`, run it, and return
/// whether the self-checking testbench passed. Runs in an isolated work dir.
fn run_dual_clock_verilator(sv_file: &str, top: &str, tb_src: &str) -> Result<bool, String> {
    let work = std::env::temp_dir().join(format!("copper_tdh_{top}_{}_{}", std::process::id(), TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| format!("mkdir: {e}"))?;

    let sv_abs = std::fs::canonicalize(sv_file).map_err(|e| format!("canonicalize {sv_file}: {e}"))?;
    let tb_path = work.join(format!("tb_{top}.cpp"));
    std::fs::write(&tb_path, tb_src).map_err(|e| format!("write tb: {e}"))?;

    let out = common::verilator_command()
        .current_dir(&work)
        .args(["--cc", "--exe", "--build", "--top-module", top, "-Wall", "-Wno-DECLFILENAME", "-CFLAGS", "-std=c++14"])
        .arg(&sv_abs)
        .arg(&tb_path)
        .output()
        .map_err(|e| format!("verilator: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "verilator build failed for {top}:\n{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    let exe = work.join("obj_dir").join(format!("V{top}"));
    let run = Command::new(&exe).output().map_err(|e| format!("run {exe:?}: {e}"))?;
    let stdout = String::from_utf8_lossy(&run.stdout);
    let _ = std::fs::remove_dir_all(&work);
    if run.status.success() {
        Ok(true)
    } else {
        Err(format!("testbench mismatch for {top}:\n{stdout}"))
    }
}

#[test]
fn sim_trace_is_stable_and_synchronizes() {
    // Ground-truth sim: 2:1 interleaving, flag synchronizes at slow cycle 5.
    let trace = sim_trace(2, 10);
    let expected: Vec<(u8, u8)> = (0..10)
        .map(|i| (2 * (i as u8 + 1), if i >= 5 { 1 } else { 0 }))
        .collect();
    assert_eq!(trace, expected, "sim trace changed unexpectedly");
}

#[test]
fn transpiled_hierarchy_matches_sim_under_verilator() {
    if !verilator_available() {
        return;
    }
    let expected = sim_trace(2, 10);

    // Transpile the REAL example file's hierarchy (parent + co-emitted children).
    let src = std::fs::read_to_string(DESIGN_FILE).expect("read example source");
    let sv = copper_codegen::transpile_source_hierarchy(&src, Some("two_domain_top"), &copper_codegen::EmitConfig::default())
        .expect("transpile hierarchy");
    let work = std::env::temp_dir().join(format!("copper_tdh_sv_{}_{}", std::process::id(), TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(&work).unwrap();
    let sv_path = work.join("two_domain_top.sv");
    std::fs::write(&sv_path, &sv).unwrap();

    let tb = dual_clock_testbench("two_domain_top", 2, &expected);
    run_dual_clock_verilator(sv_path.to_str().unwrap(), "two_domain_top", &tb)
        .expect("transpiled hierarchy must match the sim trace under Verilator");
    let _ = std::fs::remove_dir_all(&work);
}

#[test]
fn independent_reference_matches_sim_under_verilator() {
    if !verilator_available() {
        return;
    }
    assert!(Path::new(REFERENCE_SV).exists(), "missing independent reference {REFERENCE_SV}");
    let expected = sim_trace(2, 10);

    // The non-circular anchor: an outside hand-written implementation must
    // reproduce Copper's dual-clock/CDC timing cycle-for-cycle.
    let tb = dual_clock_testbench("two_domain_ref", 2, &expected);
    run_dual_clock_verilator(REFERENCE_SV, "two_domain_ref", &tb)
        .expect("independent reference SV must match the sim trace under Verilator");
}

/// The design leg (Phase C step 9, SPEC §I.13; D-20 item 9): the structural parent
/// run by the reference interpreter as the design it names (I.13.4(a)) — its
/// three children composed over its nets, each domain cycled in I.13.2(b)'s order
/// (settle, resume the domain's coroutines, edge, settle), the settle evaluating the
/// one function in topological order — against the simulator's hand wiring of the
/// same instances, at three interleavings. Needs no Verilator. The interpreter reads
/// the EXAMPLE file (the parent and its children), not this file's copies.
#[test]
fn reference_interpreter_design_matches_sim() {
    let src = std::fs::read_to_string(DESIGN_FILE).expect("read example source");
    for (k, n) in [(2usize, 10usize), (3, 10), (1, 16)] {
        let mut d = copper_ref::Design::new(&src, "two_domain_top").expect("the design leg builds two_domain_top");
        let mut trace = Vec::new();
        for _ in 0..n {
            for _ in 0..k {
                d.tick("wr_clk").expect("fast-domain cycle");
            }
            d.tick("rd_clk").expect("slow-domain cycle");
            let obs = d.observe();
            let get = |p: &str| {
                let v = &obs.iter().find(|(x, _)| x == p).unwrap().1;
                v.to_bits().unwrap().iter().enumerate().fold(0u8, |a, (i, l)| a | ((*l == Logic::One) as u8) << i)
            };
            trace.push((get("count_out"), get("flag_sync_out")));
        }
        assert_eq!(trace, sim_trace(k, n), "design leg vs hand-wired simulation at {k}:1");
        println!("design leg: two_domain_top {k}:1, {n} slow cycles agreed with the hand-wired simulation");
    }
}

//! **A stimulus sequence starts from the design's initial state — on both sides.**
//!
//! The corpus sweep's multi-sequence stimulus policy
//! (`design_docs/CORPUS_DIFFERENTIAL_SWEEP.md` §3.2) drives several sequences through
//! ONE compiled testbench of one Verilated model, and its verdict depends on each
//! sequence starting fresh: `HardwareTest::begin_sequence` makes the testbench
//! destroy the model and construct a new one at the boundary. This pins that
//! mechanism against a hand-written SystemVerilog counter — a design with state, so
//! a sequence that did NOT restart would read the previous sequence's count — and
//! the negative control shows the same trace without the boundary failing, so the
//! test can tell the two apart. The single-sequence testbench text is also pinned
//! byte-for-byte against the pre-2026-10-28 form, because the built-model cache keys
//! on it: a change there would invalidate every cached model for no reason.

mod common;

use common::verilator_available;
use copper_core::Logic;
use copper_sim::verification::generate_testbench;
use copper_sim::{verify_with_verilator_report, SimulationTrace, VerilatorCachePolicy};
use std::sync::atomic::{AtomicUsize, Ordering};

const MODULE: &str = "seq_counter";
static NONCE: AtomicUsize = AtomicUsize::new(0);

/// A 4-bit counter that starts at 0 (an explicit initializer, so the pre-reset
/// value is defined on both sides) and counts on every edge.
fn write_counter() -> String {
    let dir = std::path::PathBuf::from("target/verilator");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!(
        "{MODULE}_{}_{}.sv",
        std::process::id(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(
        &path,
        format!(
            "module {MODULE}(input logic clk, output logic [3:0] q);\n  \
             /* verilator lint_off PROCASSINIT */\n  logic [3:0] r = 4'd0;\n  \
             /* verilator lint_on PROCASSINIT */\n  always_ff @(posedge clk) r <= r + 4'd1;\n  \
             assign q = r;\nendmodule\n"
        ),
    )
    .unwrap();
    path.to_string_lossy().into_owned()
}

fn bits4(v: u8) -> Vec<Logic> {
    (0..4).map(|i| if (v >> i) & 1 == 1 { Logic::One } else { Logic::Zero }).collect()
}

/// `lengths` sequences, each expecting the counter to read 1, 2, … from a fresh
/// start; with `mark == false` the same expectations are recorded without the
/// boundaries (the counter would then keep counting, so later sequences must fail).
fn trace(lengths: &[usize], mark: bool) -> SimulationTrace {
    let mut t = SimulationTrace::new();
    let mut cycle = 0;
    for (k, n) in lengths.iter().enumerate() {
        if mark {
            t.begin_sequence(&format!("sequence {k}"));
        }
        for i in 0..*n {
            t.add_cycle(cycle, vec![], vec![("q".to_string(), bits4((i + 1) as u8))]);
            cycle += 1;
        }
    }
    t
}

#[test]
fn every_sequence_starts_from_the_initial_state() {
    if !verilator_available() {
        return;
    }
    let sv = write_counter();
    let run = verify_with_verilator_report(&sv, MODULE, &trace(&[5, 3, 4], true), None, &[], VerilatorCachePolicy::from_env())
        .expect("Verilator ran");
    assert!(run.passed, "a sequence did not start from the initial state:\n{}", run.output);

    // Negative control: the same expectations with no boundary. The second sequence
    // would read 6, 7, 8 and the third 9, 10, 11, 12 — and the failure must name
    // nothing about sequences, because the trace has none.
    let run = verify_with_verilator_report(&sv, MODULE, &trace(&[5, 3, 4], false), None, &[], VerilatorCachePolicy::from_env())
        .expect("Verilator ran");
    assert!(!run.passed, "without a boundary the counter must keep counting");
    assert!(run.output.contains("FAIL: Cycle 5 q expected 1 got 6"), "{}", run.output);
    assert!(!run.output.contains("sequence"), "{}", run.output);
}

/// A mismatch inside a sequence is reported with the sequence's index and label
/// and the cycle within it, which is what makes a sweep failure reproducible from
/// its FAIL line (`COPPER_SWEEP_SEQUENCE=<index>`).
#[test]
fn a_mismatch_names_its_sequence_and_local_cycle() {
    if !verilator_available() {
        return;
    }
    let sv = write_counter();
    let mut t = trace(&[2, 3], true);
    // Corrupt the second sequence's second expectation (global cycle 3, local 1).
    t.cycles[3].outputs[0].1 = bits4(9);
    let run = verify_with_verilator_report(&sv, MODULE, &t, None, &[], VerilatorCachePolicy::from_env())
        .expect("Verilator ran");
    assert!(!run.passed);
    assert!(
        run.output.contains("FAIL: Cycle 3 q expected 9 got 2 (sequence 1 \"sequence 1\", cycle 1)"),
        "{}",
        run.output
    );
}

/// A trace with no sequence marks produces exactly the pre-policy testbench: no
/// restart, no sequence comments, no suffix on the FAIL lines. Pinned because the
/// text is the model cache's key material (CLAUDE.md, the model-cache bullet).
#[test]
fn a_single_sequence_testbench_is_unchanged() {
    let plain = generate_testbench(MODULE, &trace(&[3], false), true, false);
    assert!(!plain.contains("sequence"), "{plain}");
    assert!(!plain.contains("delete top;\n    top = new"), "{plain}");
    assert_eq!(plain.matches(&format!("new V{MODULE}()")).count(), 1);
    assert!(plain.contains("std::cout << \"FAIL: Cycle 1 q expected 2 got \" << (int)top->q << \"\" << std::endl;"), "{plain}");

    // With marks: one construction per sequence, one restart per boundary, and the
    // suffix on every FAIL line.
    let marked = generate_testbench(MODULE, &trace(&[3, 2], true), true, false);
    assert_eq!(marked.matches(&format!("new V{MODULE}()")).count(), 2, "{marked}");
    assert_eq!(marked.matches("delete top;\n    top = new").count(), 1, "{marked}");
    assert!(marked.contains("// ── sequence 0: sequence 0 ──"), "{marked}");
    assert!(marked.contains("// ── sequence 1: sequence 1 ──"), "{marked}");
    assert!(marked.contains("(sequence 1 \\\"sequence 1\\\", cycle 0)"), "{marked}");
    // A traced run writes one VCD per sequence after the first.
    let traced = generate_testbench(MODULE, &trace(&[3, 2], true), true, true);
    assert!(traced.contains(".seq1"), "{traced}");
}

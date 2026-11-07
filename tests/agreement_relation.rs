//! **The agreement relation's verdicts, pinned** (SPEC §III.2; measured in
//! `design_docs/review/MEASUREMENTS.md` §E): each probe of
//! `tests/ref_probes/agreement_probes.rs` driven exactly as a generated sweep case
//! drives a module — observation 0 read between `pre_edge_settle` and
//! `post_edge_settle`, every step run under `run_step` so a run-time failure is
//! caught — under a stimulus chosen to leave the relation at a known observation.
//!
//! (ii)  `xp_rem` — an X observation: OUTSIDE from it on (III.2.2), sim ≡ ref still
//!       compared exactly;
//! (iii) `trunc_x_cond`, `trunc_oob`, `trunc_data` — the three run-time failures:
//!       TRUNCATED at the failing resumption (III.2.4).
//! Plus the verdicts' own plumbing: an X never reaches the testbench (the relation
//! testbench refuses one), and a panic that is not a run-time failure fails.

mod common;

use common::{run_step, verilator_available, EquivalenceTest, Rng};
use copper_core::port::{registered_wire, wire, In, RegOut};
use copper_core::{Bits, Clock, ClockDomain, Logic, Memory};
use copper_macros::hardware;
use copper_sim::HardwareExecutor;

struct MainClk;
impl ClockDomain for MainClk {}

include!("ref_probes/agreement_probes.rs");
const SRC: &str = include_str!("ref_probes/agreement_probes.rs");

type Sig = Vec<(&'static str, Vec<Logic>)>;

fn refs(v: &Sig) -> Vec<(&str, &[Logic])> {
    v.iter().map(|(n, b)| (*n, &b[..])).collect()
}

/// The generated case's cycle loop, by hand: `drive(i)` writes cycle `i`'s inputs
/// and returns them, `read()` the outputs.
fn run(
    eq: &mut EquivalenceTest,
    exec: &mut HardwareExecutor,
    clk: &mut Clock<MainClk>,
    cycles: usize,
    mut drive: impl FnMut(usize) -> Sig,
    read: impl Fn() -> Sig,
) {
    eq.begin_sequence("probe");
    for i in 0..cycles {
        let ins = drive(i);
        let step = if i == 0 {
            match run_step(|| exec.pre_edge_settle::<MainClk>()) {
                Ok(()) => {
                    let o = read();
                    eq.record_observation_zero(&refs(&o));
                    run_step(|| exec.post_edge_settle(clk))
                }
                Err(m) => Err(m),
            }
        } else {
            run_step(|| exec.tick_clock(clk))
        };
        if let Err(m) = step {
            eq.record_failure(&refs(&ins), &m);
            break;
        }
        let o = read();
        eq.record_differential(&refs(&ins), &refs(&o));
    }
}

fn need_verilator() {
    if !verilator_available() {
        panic!("Verilator is absent: these verdicts need the SV leg and cannot be skipped silently");
    }
}

/// (ii) PREDICTED: OUTSIDE from recorded cycle 3 (b = 0 is x_4; resumption 4
/// writes the all-X remainder, shown at obs_4), port `q`; the reference agrees
/// with the simulator on every observation, X included; the SV agrees before.
#[test]
fn an_x_observation_takes_the_sv_pair_outside() {
    need_verilator();
    let mut eq = EquivalenceTest::differential_only("xp_rem", SRC, Some("xp_rem")).with_reference_interpreter();
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let (a_drv, a_in) = wire::<Bits<8>, MainClk>(Bits::zero());
    let (b_drv, b_in) = wire::<Bits<8>, MainClk>(Bits::zero());
    let (q_out, q_obs) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
    let dh = q_out.dirty_handle();
    let reads = vec![a_in.wire_id(), b_in.wire_id()];
    exec.spawn_wired(xp_rem(clk.clone(), a_in, b_in, q_out), vec![dh], reads);
    let mut rng = Rng::new(11);
    run(
        &mut eq,
        &mut exec,
        &mut clk,
        12,
        |i| {
            let a = Bits::<8>::from_u8(rng.u8());
            let b = Bits::<8>::from_u8(if i == 3 { 0 } else { rng.u8() | 1 });
            a_drv.write(a);
            b_drv.write(b);
            vec![("a", a.as_array().to_vec()), ("b", b.as_array().to_vec())]
        },
        || vec![("q", q_obs.read().as_array().to_vec())],
    );
    let v = eq.finish_report();
    println!("{}", v.report);
    assert!(v.all_ok(), "OUTSIDE is not a failure:\n{}", v.report);
    assert_eq!(v.sequences.len(), 1);
    let s = &v.sequences[0];
    assert_eq!(s.verdict(), "OUTSIDE", "{}", s.line("xp_rem"));
    assert_eq!(s.outside, Some((3, Some(3), "q".to_string())), "{}", s.line("xp_rem"));
    assert!(matches!(v.interpreter, Some(Ok(12))), "sim ≡ ref compares the X exactly: {:?}", v.interpreter);
}

/// (iii-a) PREDICTED: b = 0 on cycle 5 → resumption 6 feeds X to `as_bool`; both
/// the simulator and the interpreter fail there → TRUNCATED at resumption 6; the
/// SV agrees on obs_0..obs_5.
#[test]
fn an_x_fed_to_a_condition_truncates_on_both_sides() {
    need_verilator();
    let mut eq =
        EquivalenceTest::differential_only("trunc_x_cond", SRC, Some("trunc_x_cond")).with_reference_interpreter();
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let (a_drv, a_in) = wire::<Bits<8>, MainClk>(Bits::zero());
    let (b_drv, b_in) = wire::<Bits<8>, MainClk>(Bits::zero());
    let (q_out, q_obs) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
    let dh = q_out.dirty_handle();
    let reads = vec![a_in.wire_id(), b_in.wire_id()];
    exec.spawn_wired(trunc_x_cond(clk.clone(), a_in, b_in, q_out), vec![dh], reads);
    let mut rng = Rng::new(12);
    run(
        &mut eq,
        &mut exec,
        &mut clk,
        12,
        |i| {
            let a = Bits::<8>::from_u8(rng.u8());
            let b = Bits::<8>::from_u8(if i == 5 { 0 } else { rng.u8() | 1 });
            a_drv.write(a);
            b_drv.write(b);
            vec![("a", a.as_array().to_vec()), ("b", b.as_array().to_vec())]
        },
        || vec![("q", q_obs.read().as_array().to_vec())],
    );
    let v = eq.finish_report();
    println!("{}", v.report);
    assert!(v.all_ok(), "an agreed truncation is not a failure:\n{}", v.report);
    let s = &v.sequences[0];
    assert_eq!(s.verdict(), "TRUNCATED", "{}", s.line("trunc_x_cond"));
    let (r, m) = s.truncated.clone().unwrap();
    assert_eq!(r, 6, "{}", s.line("trunc_x_cond"));
    assert!(m.contains("Cannot convert X to bool"), "{m}");
    assert!(matches!(v.interpreter, Some(Ok(5))), "five cycles compared before the failure: {:?}", v.interpreter);
}

/// (iii-b) PREDICTED: addresses 0..=6 on cycles 0..=6, 12 on cycle 7 → the read
/// at resumption 8 is out of range → TRUNCATED at resumption 8, reference leg
/// not run (a `Memory` binding); the SV agrees on obs_0..obs_7.
#[test]
fn an_out_of_range_address_truncates() {
    need_verilator();
    let mut eq = EquivalenceTest::differential_only("trunc_oob", SRC, Some("trunc_oob"));
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let (a_drv, a_in) = wire::<Bits<4>, MainClk>(Bits::zero());
    let (q_out, q_obs) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
    let dh = q_out.dirty_handle();
    let reads = vec![a_in.wire_id()];
    exec.spawn_wired(trunc_oob(clk.clone(), a_in, q_out), vec![dh], reads);
    run(
        &mut eq,
        &mut exec,
        &mut clk,
        12,
        |i| {
            let a = Bits::<4>::from_usize(if i == 7 { 12 } else { i });
            a_drv.write(a);
            vec![("addr", a.as_array().to_vec())]
        },
        || vec![("q", q_obs.read().as_array().to_vec())],
    );
    let v = eq.finish_report();
    println!("{}", v.report);
    assert!(v.all_ok(), "{}", v.report);
    let s = &v.sequences[0];
    assert_eq!(s.verdict(), "TRUNCATED", "{}", s.line("trunc_oob"));
    let (r, m) = s.truncated.clone().unwrap();
    assert_eq!(r, 8, "{}", s.line("trunc_oob"));
    assert!(m.contains("out of range") && m.contains("reference leg not run"), "{m}");
}

/// (iii-c) PREDICTED: `go` = 1 on cycles 0..=5, 0 on cycle 6 → no read is issued
/// for edge 7 and the `data()` after it is undelivered. The two-job simulator ran
/// the post-tick statement at t_7⁺ (resumption 7, inside cycle 6's `tick_clock`);
/// since the flip (Phase C step 3) the statement after a tick is the head of the
/// NEXT resumption, which runs at t_8⁻ — inside cycle 7's `tick_clock` — so the
/// trace is TRUNCATED at resumption 8 (measured), reference leg not run; the SV
/// agrees on every observation before it.
#[test]
fn an_undelivered_data_truncates() {
    need_verilator();
    let mut eq = EquivalenceTest::differential_only("trunc_data", SRC, Some("trunc_data"));
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let (g_drv, g_in) = wire::<Logic, MainClk>(Logic::Zero);
    let (a_drv, a_in) = wire::<Bits<4>, MainClk>(Bits::zero());
    let (q_out, q_obs) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
    let dh = q_out.dirty_handle();
    let reads = vec![g_in.wire_id(), a_in.wire_id()];
    exec.spawn_wired(trunc_data(clk.clone(), g_in, a_in, q_out), vec![dh], reads);
    run(
        &mut eq,
        &mut exec,
        &mut clk,
        12,
        |i| {
            let g = if i == 6 { Logic::Zero } else { Logic::One };
            let a = Bits::<4>::from_usize(i * 3 % 16);
            g_drv.write(g);
            a_drv.write(a);
            vec![("go", vec![g]), ("addr", a.as_array().to_vec())]
        },
        || vec![("q", q_obs.read().as_array().to_vec())],
    );
    let v = eq.finish_report();
    println!("{}", v.report);
    assert!(v.all_ok(), "{}", v.report);
    let s = &v.sequences[0];
    assert_eq!(s.verdict(), "TRUNCATED", "{}", s.line("trunc_data"));
    let (r, m) = s.truncated.clone().unwrap();
    assert_eq!(r, 8, "{}", s.line("trunc_data"));
    assert!(m.contains("not yet valid"), "{m}");
}

/// The relation testbench refuses an X expected value outright: under the relation
/// an X observation is cut before generation, and the legacy form's "compare it as
/// 1" (`logic_vec_to_int`) must be unreachable.
#[test]
fn the_relation_testbench_never_compares_an_x() {
    let mut t = copper_sim::SimulationTrace::new();
    t.relation = true;
    t.add_cycle(0, vec![], vec![("q".into(), vec![Logic::X])]);
    let r = std::panic::catch_unwind(|| copper_sim::verification::generate_testbench("m", &t, true, false));
    let msg = r.err().and_then(|p| p.downcast_ref::<String>().cloned()).unwrap_or_default();
    assert!(msg.contains("OUTSIDE"), "an X reached the relation testbench: {msg:?}");
}

/// A panic that is not one of III.2.4's run-time failures is not a truncation.
#[test]
#[should_panic(expected = "not with a run-time failure")]
fn an_unrecognized_panic_is_a_failure_not_a_truncation() {
    let mut eq = EquivalenceTest::differential_only("xp_rem", SRC, Some("xp_rem"));
    eq.begin_sequence("probe");
    eq.record_failure(&[], "Combinational loop detected: task 0 output changed on 100 consecutive delta cycles");
}

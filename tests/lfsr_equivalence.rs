//! Behavioral-equivalence test for a 32-bit LFSR.
//!
//! Exercises the M2 features that made this example transpilable: `.as_bool()`,
//! constant bit-indexing (`state[0]`), `Bits::from_u32` constructors, pre-loop
//! combinational constants (`xor_mask`), parenthesised shift/xor expressions,
//! and `if`-expressions with block branches.
//!
//! See `tests/common/mod.rs` for the shared harness and how to read a failure.

mod common;

use common::{transpile_fixture, verilator_available, EquivalenceTest};
use copper_core::port::{wire, In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;
use copper_sim::HardwareExecutor;

struct MainClk;
impl ClockDomain for MainClk {}

include!("fixtures/lfsr_dut.rs");
const DUT_SRC: &str = include_str!("fixtures/lfsr_dut.rs");

/// Reference model in plain `u32`, independent of the DUT's bit representation.
fn next_lfsr(state: u32, yumi: bool) -> u32 {
    if !yumi {
        return state;
    }
    let mask: u32 = (1 << 31) | (1 << 29) | (1 << 26) | (1 << 25);
    if state & 1 == 1 {
        (state >> 1) ^ mask
    } else {
        state >> 1
    }
}

#[test]
fn lfsr_sim_matches_transpiled_verilog() {
    // Also G2 structural reg-match vs the independent BaseJump STL reference: its
    // flip-flop is `o_r` (`xor_mask`/`o_n` are combinational `assign`s), vs Copper's
    // `state` — StorageEquivalent (count = 1). Directly exercises constant
    // exclusion: `xor_mask` is a pre-loop constant, correctly NOT a register on
    // either side.
    let mut eq = EquivalenceTest::new("lfsr", DUT_SRC).with_reference_registers(
        "examples/sequential/sv/lfsr.sv",
        copper_analysis::RegMatch::StorageEquivalent,
    );

    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();

    let (reset_drv, reset_in) = wire::<Logic, MainClk>(Logic::Zero);
    let (yumi_drv, yumi_in) = wire::<Logic, MainClk>(Logic::Zero);
    let (o_out, o_obs) = wire::<Bits<32>, MainClk>(Bits::from_u32(1));
    let dh = o_out.dirty_handle();
    let reads = vec![reset_in.wire_id(), yumi_in.wire_id()];
    exec.spawn_wired(lfsr(clk.clone(), reset_in, yumi_in, o_out), vec![dh], reads);

    // One reset cycle, then advance.
    let stimulus: Vec<(Logic, Logic)> = std::iter::once((Logic::One, Logic::Zero))
        .chain((0..16).map(|_| (Logic::Zero, Logic::One)))
        .collect();

    let mut ref_state: u32 = 1;

    for &(reset, yumi) in stimulus.iter() {
        reset_drv.write(reset);
        yumi_drv.write(yumi);
        exec.tick_clock(&mut clk);

        ref_state = if reset == Logic::One {
            1
        } else {
            next_lfsr(ref_state, yumi == Logic::One)
        };
        let expected = Bits::<32>::from_u32(ref_state);

        eq.record(
            &[("reset_i", &[reset]), ("yumi_i", &[yumi])],
            &[("o", o_obs.read().as_array())],
            &[("o", expected.as_array())],
        );
    }

    eq.finish();
}

/// **Before its reset, the simulator and the emitted SystemVerilog disagree** — found
/// by the corpus sweep's multi-sequence stimulus policy on 2026-10-28 (the single
/// seed it had before happened to assert `reset_i` on cycle 0). The module starts
/// `state` at a Rust initial value, `Bits::from_u32(1)`, which the SV does not
/// carry: its flop has no initializer and no reset value outside the `reset_i`
/// branch, so the Verilated model starts at 0 and, being an LFSR, stays there until
/// reset. Measured: with `reset_i = 0, yumi_i = 0` the simulator reads `o = 1` and
/// the SV `0` on every pre-reset cycle; with `yumi_i = 1` the simulator advances
/// from 1 (`2785017856`, then `1392508928`) while the SV stays at 0. The sweep
/// therefore starts its comparison at the reset (`build.rs`'s `RESET` row). This
/// pins the difference so it is a recorded fact rather than a lucky seed; whether a
/// Rust initializer should reach the SV is a transpiler question, not settled here.
#[test]
fn before_reset_the_simulator_holds_its_rust_initial_value_and_the_sv_holds_zero() {
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let (reset_drv, reset_in) = wire::<Logic, MainClk>(Logic::Zero);
    let (yumi_drv, yumi_in) = wire::<Logic, MainClk>(Logic::Zero);
    let (o_out, o_obs) = wire::<Bits<32>, MainClk>(Bits::zero());
    let dh = o_out.dirty_handle();
    let reads = vec![reset_in.wire_id(), yumi_in.wire_id()];
    exec.spawn_wired(lfsr(clk.clone(), reset_in, yumi_in, o_out), vec![dh], reads);

    // Never reset: three held cycles, then three shifting ones.
    let stimulus: Vec<Logic> = vec![Logic::Zero, Logic::Zero, Logic::Zero, Logic::One, Logic::One, Logic::One];
    let mut trace = copper_sim::SimulationTrace::new();
    let mut sim: Vec<u32> = Vec::new();
    for (cycle, &yumi) in stimulus.iter().enumerate() {
        reset_drv.write(Logic::Zero);
        yumi_drv.write(yumi);
        exec.tick_clock(&mut clk);
        let o = o_obs.read();
        sim.push(o.as_array().iter().enumerate().fold(0u32, |acc, (i, b)| {
            if *b == Logic::One { acc | (1 << i) } else { acc }
        }));
        trace.add_cycle(
            cycle,
            vec![("reset_i".into(), vec![Logic::Zero]), ("yumi_i".into(), vec![yumi])],
            vec![("o".into(), o.as_array().to_vec())],
        );
    }
    assert_eq!(sim, vec![1, 1, 1, 2785017856, 1392508928, 696254464], "simulator behaviour changed");

    if !verilator_available() {
        return;
    }
    let sv = transpile_fixture("lfsr", DUT_SRC, None);
    let run = copper_sim::verify_with_verilator_report(
        sv.to_str().unwrap(),
        "lfsr",
        &trace,
        None,
        &[],
        copper_sim::VerilatorCachePolicy::from_env(),
    )
    .expect("Verilator ran");
    assert!(
        !run.passed,
        "the SV now agrees with the simulator before reset — the lfsr RESET row in build.rs \
         is no longer needed, and the Rust initializer reaches the SV"
    );
    for (cycle, expected) in sim.iter().enumerate() {
        assert!(
            run.output.contains(&format!("FAIL: Cycle {cycle} o expected {expected} got 0")),
            "cycle {cycle}: expected the SV at 0 before reset; Verilator said:\n{}",
            run.output
        );
    }
}

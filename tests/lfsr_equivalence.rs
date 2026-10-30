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

/// **Before its reset, the emitted SystemVerilog holds the Rust initial value** — the
/// module starts `state` at `Bits::from_u32(1)`, and since 2026-10-29 the transpiler
/// states that value as the flop's `initial` assignment (`initial begin state = 32'd1;
/// end`), so the Verilated model starts where the simulator does and the pre-reset
/// cycles agree: with `reset_i = 0, yumi_i = 0` both read `o = 1`, and with `yumi_i =
/// 1` both advance from 1 (`2785017856`, then `1392508928`).
///
/// Until that day this test pinned the opposite — the initializer was silently
/// dropped (only an integer literal or an enum variant reached the SV), the flop had
/// no power-on value, Verilator zero-filled it and an LFSR at 0 stays at 0 — a gap the
/// multi-sequence stimulus policy found on 2026-10-28 (the single seed before it
/// happened to assert `reset_i` on cycle 0), recorded then as a `RESET` row in
/// `build.rs`. The row is gone; this is the assertion that keeps it gone. A
/// Verilator-less environment still checks the emitted text.
#[test]
fn before_reset_the_sv_holds_the_rust_initial_value_like_the_simulator() {
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

    let sv = transpile_fixture("lfsr", DUT_SRC, None);
    let text = std::fs::read_to_string(&sv).expect("read the transpiled lfsr");
    assert!(
        text.contains("initial begin\n        state = 32'd1;\n    end"),
        "the Rust initializer `Bits::from_u32(1)` must reach the SV as `state`'s initial value; \
         emitted:\n{text}"
    );

    if !verilator_available() {
        return;
    }
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
        run.passed,
        "the SV disagrees with the simulator before reset — the register's initial value no \
         longer reaches the emitted SystemVerilog (the 2026-10-28 gap is back); Verilator \
         said:\n{}",
        run.output
    );
}

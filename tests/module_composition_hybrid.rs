//! Module-composition tests: combinational logic as plain functions, sequential
//! modules composed by a testbench (`spawn_wired`, the hand wiring of a design,
//! SPEC I.13.3) and spawned as peers (`spawn_untracked`). Uses the current
//! `In<T,D>` / `RegOut<T,D>` wire port model.
//!
//! Until Phase C step 11b the pipeline was spawned with `spawn_child!`, which also
//! recorded a parent/child `ModuleInfo` for a parent that was never a module. A
//! hierarchy is a structural module (SPEC I.13.5, D-18 A2, D-23), so `spawn_child`,
//! `spawn_child!` and `module_info` were deleted; the children are wired here as a
//! testbench wires any design, which stays legal.

use copper_core::port::{wire, In, RegOut, registered_wire};
use copper_core::{Clock, ClockDomain};
use copper_macros::hardware;
use copper_sim::HardwareExecutor;

struct MainClk;
impl ClockDomain for MainClk {}

// Combinational logic is just plain Rust functions — no hardware wrapper needed.
fn add_one(x: u8) -> u8 {
    x.wrapping_add(1)
}

fn double(x: u8) -> u8 {
    x.wrapping_add(x)
}

fn affine_mix(x: u8) -> u8 {
    double(add_one(x))
}

// A registered pipeline stage: drives `output` with the previous cycle's value,
// then latches `add_one(input)` into its register.
#[hardware(sequential)]
async fn stage_add_one(clk: Clock<MainClk>, input: In<u8, MainClk>, output: RegOut<u8, MainClk>) {
    // The one-job registered stage (D-8): the `RegOut` IS the stage register —
    // `always_ff output <= add_one(input)` — so a two-stage pipeline keeps its
    // two-cycle latency, standalone and composed.
    loop {
        output.write(add_one(input.read()));
        clk.tick().await;
    }
}

#[hardware(sequential)]
async fn stage_double(clk: Clock<MainClk>, input: In<u8, MainClk>, output: RegOut<u8, MainClk>) {
    // The one-job registered stage (D-8): the `RegOut` IS the stage register —
    // `always_ff output <= double(input)` — so a two-stage pipeline keeps its
    // two-cycle latency, standalone and composed.
    loop {
        output.write(double(input.read()));
        clk.tick().await;
    }
}

// `step` is a compile-time constant (a module parameter in hardware), so it is a
// const generic rather than a runtime port — which also lets it be a `#[hardware]`
// module (the macro requires ports to be `Clock`/`In`/`Out`).
#[hardware(sequential)]
async fn counter_by<const STEP: u8>(clk: Clock<MainClk>, output: RegOut<u8, MainClk>) {
    let mut reg = 0u8;
    loop {
        reg = reg.wrapping_add(STEP);
        output.write(reg);
        clk.tick().await;
    }
}

#[test]
fn combinational_modules_are_plain_functions() {
    assert_eq!(add_one(0), 1);
    assert_eq!(double(7), 14);
    assert_eq!(affine_mix(3), 8);
    assert_eq!(affine_mix(255), 0);
}

#[test]
fn sequential_modules_compose_in_a_testbench() {
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();

    // in_data → stage_add_one → mid → stage_double → out_data
    let (in_drv, in_port) = wire::<u8, MainClk>(0);
    let (mid_out, mid_in) = registered_wire::<u8, MainClk>(&clk, 0);
    let (out_drv, out_obs) = registered_wire::<u8, MainClk>(&clk, 0);

    let s1_reads = vec![in_port.wire_id()];
    let s1_dirties = vec![mid_out.dirty_handle()];
    exec.spawn_wired(stage_add_one(clk.clone(), in_port, mid_out), s1_dirties, s1_reads);
    let s2_reads = vec![mid_in.wire_id()];
    let s2_dirties = vec![out_drv.dirty_handle()];
    exec.spawn_wired(stage_double(clk.clone(), mid_in, out_drv), s2_dirties, s2_reads);

    // Two-stage registered pipeline: affine_mix has a TWO-FLOP latency — the
    // input driven before edge k is sampled at k's pre-edge, commits through
    // stage 1 at edge k and stage 2 at edge k+1, so it appears at observation
    // k+1. Re-blessed with the canonical stage spelling (2026-08-27): the old
    // stream had one extra cycle, which was the stages' measured divergence
    // from their own emitted SV, not the pipeline's latency.
    let inputs = [3u8, 7, 11, 1];
    let expected_outputs = [0u8, 8, 16, 24];

    for (input, expected) in inputs.iter().zip(expected_outputs.iter()) {
        in_drv.write(*input);
        exec.tick_clock(&mut clk);
        let observed = out_obs.read();
        assert_eq!(observed, *expected, "cycle {}", clk.cycle());
    }
}

#[test]
fn peer_modules_continue_using_spawn_untracked() {
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();

    let (out1_drv, out1_obs) = registered_wire::<u8, MainClk>(&clk, 0);
    let (out2_drv, out2_obs) = registered_wire::<u8, MainClk>(&clk, 0);
    let (out3_drv, out3_obs) = registered_wire::<u8, MainClk>(&clk, 0);

    exec.spawn_untracked(counter_by::<1>(clk.clone(), out1_drv), vec![]);
    exec.spawn_untracked(counter_by::<2>(clk.clone(), out2_drv), vec![]);
    exec.spawn_untracked(counter_by::<5>(clk.clone(), out3_drv), vec![]);

    // Re-blessed with the head-write migration: the first observation shows the
    // count committed at the first edge (the `counter` example's anchored
    // semantics), not the pre-update initial value.
    let expected = [(1u8, 2u8, 5u8), (2, 4, 10), (3, 6, 15), (4, 8, 20)];

    for (exp1, exp2, exp3) in expected {
        exec.tick_clock(&mut clk);
        assert_eq!(out1_obs.read(), exp1, "counter1 cycle {}", clk.cycle());
        assert_eq!(out2_obs.read(), exp2, "counter2 cycle {}", clk.cycle());
        assert_eq!(out3_obs.read(), exp3, "counter3 cycle {}", clk.cycle());
    }
}

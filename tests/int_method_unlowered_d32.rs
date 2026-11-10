//! SPEC II.1.3's fourth named parity exception (D-32 (1), 2026-11-10; MEASUREMENTS §AJ): a Rust
//! integer method the transpiler does not lower.
//!
//! The macro sees no types, and rustc type-checks every inherent method of a Rust integer, so a
//! body calling one compiles and simulates with Rust's meaning (I.14.3(c)). The transpiler lowers
//! the methods on `copper_codegen::LOWERED_INT_METHODS` and refuses every other one with
//! ``method `<m>` `` and `copper_analysis::UNLOWERED_METHOD_MESSAGE` — in the `.`-form and the
//! path-call form (`u16::isqrt(v)`) — and never emits text. Lowering a method and adding it to
//! the list shrinks the exception.
//!
//! `copper-analysis/tests/front_end_parity.rs::the_four_named_parity_exceptions_have_exactly_their_witnesses`
//! transpiles a probe of every method on and off the list, and requires this file to hold the
//! only modules calling an unlowered one; `copper-macros/tests/ui/pass/int_method_unlowered_d32.rs`
//! is its ui row.

use copper_core::port::{registered_wire, wire, In, Out, RegOut};
use copper_core::{Clock, ClockDomain};
use copper_macros::hardware;
use copper_sim::HardwareExecutor;

struct A;
impl ClockDomain for A {}

/// A sequential body calling `wrapping_pow` (a product chain; not lowered).
#[hardware(sequential)]
async fn um_seq(clk: Clock<A>, a: In<u16, A>, q: RegOut<u16, A>) {
    loop {
        q.write(a.read().wrapping_pow(3));
        clk.tick().await;
    }
}

/// A combinational body calling `isqrt` in the path-call form.
#[hardware(combinational)]
fn um_comb_path<D: ClockDomain>(a: In<u16, D>, q: Out<u16, D>) {
    q.write(u16::isqrt(a.read()));
}

const SRC: &str = include_str!("int_method_unlowered_d32.rs");

#[test]
fn the_transpiler_refuses_both_with_the_fourth_exceptions_message() {
    for (top, m) in [("um_seq", "wrapping_pow"), ("um_comb_path", "isqrt")] {
        let err = copper_codegen::transpile_source(SRC, Some(top), &copper_codegen::EmitConfig::default())
            .expect_err("the transpiler does not lower the method");
        let msg = format!("method `{m}` {}", copper_analysis::UNLOWERED_METHOD_MESSAGE);
        assert!(err.contains(&msg), "{top}: {err}");
        assert!(!copper_codegen::LOWERED_INT_METHODS.contains(&m), "{m} is lowered now: pick another witness");
    }
}

/// What the macro admits, the simulator runs with Rust's meaning.
#[test]
fn the_simulator_runs_them_as_rust_does() {
    let mut exec = HardwareExecutor::new();
    let (drv, a) = wire::<u16, A>(0);
    let (o, obs) = wire::<u16, A>(0);
    let dh = o.dirty_handle();
    exec.spawn_wired(um_comb_path(a, o), vec![dh], vec![]);
    for v in (0u16..=u16::MAX).step_by(7) {
        drv.write(v);
        exec.poll_tasks();
        assert_eq!(obs.read(), v.isqrt(), "isqrt({v})");
    }

    let mut clk = Clock::<A>::new();
    let mut exec = HardwareExecutor::new();
    let (drv, a) = wire::<u16, A>(0);
    let (q, obs) = registered_wire::<u16, A>(&clk, 0);
    let dh = q.dirty_handle();
    let reads = vec![a.wire_id()];
    exec.spawn_wired(um_seq(clk.clone(), a, q), vec![dh], reads);
    for v in [0u16, 1, 2, 3, 41, 255, 4097, u16::MAX] {
        drv.write(v);
        exec.tick_clock(&mut clk);
        assert_eq!(obs.read(), v.wrapping_pow(3), "wrapping_pow({v}, 3)");
    }
}

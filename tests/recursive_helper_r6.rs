//! SPEC II.1 R6 — recursion through a helper `fn` — the first named exception to
//! II.1.3's "a module compiles under `#[hardware]` iff it transpiles" (D-22, Phase C
//! step 11; MEASUREMENTS §Q).
//!
//! The macro holds the module's `ItemFn` only (I.2.1), never a helper's body, and
//! rustc admits a non-`async` recursion, so `parity8` below compiles. The transpiler
//! inlines every helper (I.2.4) and refuses the cycle there, with R6's message, so
//! no wrong SystemVerilog is ever emitted. What is left is a refusal-only asymmetry,
//! not a divergence: a terminating recursion simulates as Rust runs it, which this
//! test checks over the whole input space against `u8::count_ones`.
//!
//! `copper-analysis/tests/front_end_parity.rs::the_three_named_parity_exceptions_have_exactly_their_witnesses`
//! scans the tree's helper call graph and requires this file to hold the only
//! `#[hardware]` module that reaches a recursive helper, so the exception cannot
//! widen silently; `copper-macros/tests/ui/pass/recursive_helper_r6.rs` is its ui row.

use copper_core::port::{wire, In, Out};
use copper_core::{Bits, ClockDomain, Logic};
use copper_macros::hardware;
use copper_sim::HardwareExecutor;

/// The parity of `x`'s low `k` bits — a recursion that terminates (`k` falls to 0).
fn parity_of(x: Bits<8>, k: usize) -> Logic {
    if k == 0 { Logic::Zero } else { x[k - 1] ^ parity_of(x, k - 1) }
}

#[hardware(combinational)]
fn parity8<D: ClockDomain>(a: In<Bits<8>, D>, p: Out<Logic, D>) {
    p.write(parity_of(a.read(), 8));
}

struct Dom;
impl ClockDomain for Dom {}

const SRC: &str = include_str!("recursive_helper_r6.rs");

#[test]
fn the_macro_admits_it_and_the_simulator_runs_it_as_rust_does() {
    let mut exec = HardwareExecutor::new();
    let (a_drv, a_in) = wire::<Bits<8>, Dom>(Bits::zero());
    let (p_out, p_obs) = wire::<Logic, Dom>(Logic::Zero);
    let dh = p_out.dirty_handle();
    let reads = vec![a_in.wire_id()];
    exec.spawn_wired(parity8(a_in, p_out), vec![dh], reads);
    for v in 0u16..256 {
        a_drv.write(Bits::from_u8(v as u8));
        exec.poll_tasks();
        let want = if (v as u8).count_ones() % 2 == 1 { Logic::One } else { Logic::Zero };
        assert_eq!(p_obs.read(), want, "parity of {v:#04x}");
    }
}

#[test]
fn the_transpiler_refuses_it_with_r6s_message() {
    let err = copper_codegen::transpile_source(SRC, Some("parity8"), &copper_codegen::EmitConfig::default())
        .expect_err("the transpiler inlines the helper and meets the cycle");
    assert!(err.contains("cannot inline recursive function `parity_of`"), "{err}");
    assert!(err.contains(copper_analysis::front_end::RECURSIVE_HELPER_MESSAGE), "{err}");
}

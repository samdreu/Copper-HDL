//! SPEC II.1.3's third named parity exception (D-26 (2), 2026-11-08; MEASUREMENTS §U.2,
//! §U.11): a pattern naming a FILE-level `const` whose value is X.
//!
//! An X pattern is OUTSIDE (I.14.3(d)–(e), D-24 (2)): `Logic::X` written in a pattern, or a
//! body `const` with an X initializer, is refused by both front-ends (`check_outside_methods`,
//! `X_PATTERN_MESSAGE`). A file const is outside the `ItemFn` the macro holds, so it cannot see
//! the value and admits the pattern; rustc matches `Logic` structurally, so the simulator takes
//! the arm when the scrutinee holds X. The transpiler folds the const's value (§U.2) and refuses
//! it with the same message. No text is emitted.
//!
//! `copper-analysis/tests/front_end_parity.rs::the_four_named_parity_exceptions_have_exactly_their_witnesses`
//! scans the tree and requires this file to hold the only modules matching a file const whose
//! value is X; `copper-macros/tests/ui/pass/file_x_const_pattern_d26.rs` is its ui row.

use copper_core::port::{wire, In, Out};
use copper_core::{Bits, ClockDomain, Logic};
use copper_macros::hardware;
use copper_sim::HardwareExecutor;

struct A;
impl ClockDomain for A {}

const UX: Logic = Logic::X;
const BX: Bits<2> = Bits::x();

/// A `Logic` scrutinee matched against an X-valued file const.
#[hardware(combinational)]
fn fx_logic<D: ClockDomain>(a: In<Logic, D>, o: Out<Logic, D>) {
    match a.read() {
        UX => o.write(Logic::One),
        _ => o.write(Logic::Zero),
    }
}

/// A `Bits<2>` scrutinee matched against an X-valued file const.
#[hardware(combinational)]
fn fx_bits<D: ClockDomain>(a: In<Bits<2>, D>, o: Out<Logic, D>) {
    match a.read() {
        BX => o.write(Logic::One),
        _ => o.write(Logic::Zero),
    }
}

const SRC: &str = include_str!("file_x_const_pattern_d26.rs");

#[test]
fn the_transpiler_refuses_both_with_the_x_pattern_message() {
    for top in ["fx_logic", "fx_bits"] {
        let err = copper_codegen::transpile_source(SRC, Some(top), &copper_codegen::EmitConfig::default())
            .expect_err("the transpiler folds the const and sees X");
        assert!(err.contains(copper_analysis::X_PATTERN_MESSAGE), "{top}: {err}");
    }
}

/// What the macro admits, the simulator runs: rustc's structural match takes the X arm for an
/// X scrutinee and `_` for every 2-state one — the reading I.14.3(e) puts outside the language.
#[test]
fn the_simulator_matches_x_structurally() {
    for (input, want) in [(Logic::X, Logic::One), (Logic::Zero, Logic::Zero), (Logic::One, Logic::Zero)] {
        let mut exec = HardwareExecutor::new();
        let (drv, a) = wire::<Logic, A>(input);
        let (o, obs) = wire::<Logic, A>(Logic::Zero);
        let dh = o.dirty_handle();
        exec.spawn_wired(fx_logic(a, o), vec![dh], vec![]);
        drv.write(input);
        exec.poll_tasks();
        assert_eq!(obs.read(), want, "input {input:?}");
    }
}

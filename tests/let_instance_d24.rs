//! SPEC II.1.3's second named parity exception (D-24 (1), Phase C step 11c;
//! MEASUREMENTS §T): a `let`-bound instance in a behavioural body —
//! `let _ = child(..)` / `let m = child(..)`.
//!
//! D-23 (1) made `child(..);` a compile error through the generated fn's denied
//! `#[must_use]`; a `let` binding is a use, so the bound forms compile, and the macro
//! cannot tell a `let`'s callee that is a module from a helper `fn` (it holds the
//! module's `ItemFn` only). The transpiler refuses both with I.13.5's message from its
//! registry. What would be left is a silent non-execution — the child's coroutine
//! dropped unstarted — and the simulator refuses that at run time: a `HardwareModule`
//! dropped without being consumed by a spawn or `copper_sim::__instance` panics with
//! `UNSTARTED_MODULE_MESSAGE` (the drop guard, D-24 (1) (a)).
//!
//! `copper-analysis/tests/front_end_parity.rs::the_named_parity_exceptions_are_r6_and_the_let_instance`
//! scans the tree and requires this file to hold the only behavioural bodies that
//! bind a `#[hardware]` call; `copper-macros/tests/ui/pass/let_instance_d24.rs` is its
//! ui row.

use copper_core::port::{wire, In, RegOut};
use copper_core::{Clock, ClockDomain, Logic};
use copper_macros::hardware;
use copper_sim::{HardwareExecutor, HardwareModule, UNSTARTED_MODULE_MESSAGE};

struct A;
impl ClockDomain for A {}

#[hardware(sequential)]
async fn child(clk: Clock<A>, o: RegOut<Logic, A>) {
    loop {
        o.write(Logic::One);
        clk.tick().await;
    }
}

/// `let _ = child(..)`: the value is dropped at the `let`, in the first resumption.
#[hardware(sequential)]
async fn parent_wildcard(clk: Clock<A>, a: In<Logic, A>, o: RegOut<Logic, A>) {
    let _ = child(clk.clone(), o);
    loop {
        let _v = a.read();
        clk.tick().await;
    }
}

/// `let m = child(..)` before the loop: `_m` is live across the never-ending loop,
/// so the value is dropped with the parent's coroutine, when the executor drops it.
#[hardware(sequential)]
async fn parent_named(clk: Clock<A>, a: In<Logic, A>, o: RegOut<Logic, A>) {
    let _m = child(clk.clone(), o);
    loop {
        let _v = a.read();
        clk.tick().await;
    }
}

const SRC: &str = include_str!("let_instance_d24.rs");

fn ports() -> (In<Logic, A>, RegOut<Logic, A>) {
    let (_drv, a) = wire::<Logic, A>(Logic::Zero);
    let clk = Clock::<A>::new();
    let (o, _obs) = copper_core::port::registered_wire::<Logic, A>(&clk, Logic::Zero);
    (a, o)
}

fn panic_text(r: Box<dyn std::any::Any + Send>) -> String {
    r.downcast_ref::<String>().cloned().or_else(|| r.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
}

#[test]
fn let_wildcard_instance_panics_at_the_first_resumption() {
    let r = std::panic::catch_unwind(|| {
        let mut clk = Clock::<A>::new();
        let mut exec = HardwareExecutor::new();
        let (a, o) = ports();
        exec.spawn_wired(parent_wildcard(clk.clone(), a, o), vec![], vec![]);
        exec.tick_clock(&mut clk);
        unreachable!("the first resumption drops the child unstarted");
    });
    let msg = panic_text(r.expect_err("the drop guard fires"));
    assert!(msg.starts_with(UNSTARTED_MODULE_MESSAGE), "{msg}");
    assert!(msg.ends_with("(module `child`)"), "{msg}");
}

#[test]
fn let_named_instance_panics_when_the_parent_is_dropped() {
    let mut clk = Clock::<A>::new();
    let mut exec = HardwareExecutor::new();
    let (a, o) = ports();
    exec.spawn_wired(parent_named(clk.clone(), a, o), vec![], vec![]);
    // The binding holds the value across the loop: the run itself is silent.
    for _ in 0..4 {
        exec.tick_clock(&mut clk);
    }
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(exec)));
    let msg = panic_text(r.expect_err("dropping the parent drops the child unstarted"));
    assert!(msg.starts_with(UNSTARTED_MODULE_MESSAGE), "{msg}");
}

#[test]
fn a_spawned_or_instantiated_module_is_consumed() {
    let mut clk = Clock::<A>::new();
    let mut exec = HardwareExecutor::new();
    let (a, o) = ports();
    let _ = a;
    exec.spawn_wired(child(clk.clone(), o), vec![], vec![]);
    exec.tick_clock(&mut clk);
    drop(exec);
    let (_a, o) = ports();
    copper_sim::__instance(child(clk.clone(), o));
}

#[test]
fn a_hand_built_module_dropped_unstarted_panics_too() {
    let r = std::panic::catch_unwind(|| drop(HardwareModule::__new(async {})));
    let msg = panic_text(r.expect_err("the guard is on the type"));
    assert!(msg.starts_with(UNSTARTED_MODULE_MESSAGE) && msg.ends_with("(module `a hand-built module`)"), "{msg}");
}

/// A module dropped while another panic unwinds does not panic again: a panic in a
/// drop during unwinding would abort the process (`std::thread::panicking()`).
#[test]
#[should_panic(expected = "the first panic")]
fn the_guard_is_silent_while_another_panic_unwinds() {
    let clk = Clock::<A>::new();
    let (_a, o) = ports();
    let _m = child(clk.clone(), o);
    panic!("the first panic");
}

#[test]
fn the_transpiler_refuses_both_with_i_13_5s_message() {
    for top in ["parent_wildcard", "parent_named"] {
        let err = copper_codegen::transpile_source(SRC, Some(top), &copper_codegen::EmitConfig::default())
            .expect_err("the transpiler's registry knows `child`");
        assert!(err.contains(copper_analysis::front_end::BEHAVIOURAL_INSTANCE_MESSAGE), "{top}: {err}");
    }
    copper_codegen::transpile_source(SRC, Some("child"), &copper_codegen::EmitConfig::default()).expect("the child transpiles");
}

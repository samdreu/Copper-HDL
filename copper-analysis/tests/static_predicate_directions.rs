//! SPEC II.2.2 / P22 (Phase C step 10, `review/MEASUREMENTS.md` §P.5): the direction of
//! every static predicate that is not exact, pinned by a witness — "narrowed in Phase C
//! or pinned as a direction by a test" (II.2.3). A test here failing means a predicate's
//! direction moved: re-derive the II.2.2 row before changing the assertion.
//!
//! Narrowed in step 10 (the witnesses now pass the other way):
//! * R3 `may_exit_without_tick` — *over* at a statement holding a tick without being
//!   one (`livelock_a`, `livelock_b`): narrowed by the all-paths walk; *under* at a
//!   literal-empty `for`: narrowed by evaluating literal ranges.
//! * D2 `check_definite_assignment` — *under* at a folded `for` (a write under an `if`
//!   inside it counted as a write of every port) and at a never-written port: both
//!   narrowed (the combinational CFG expands its loops; the never-written clause).
//!
//! Pinned as directions (what stays approximate, and which way):
//! * R3 *over* by path-insensitivity (two `if`s on correlated conditions), *under* at a
//!   range empty only through a constant (`0..N`, `N = 0`).
//! * D2 *over* by path-insensitivity.
//! * the two-flop rule (I.12.3) *over* by path-insensitivity.
//! * the combinational-cycle refusal at spawn — *under* for reads the testbench does not
//!   declare (`copper-sim/src/executor.rs::fixpoint_oscillating_loop_trips_threshold`);
//!   exact on the declared graph.
//! * `check_admissible` (R16) — *under* by design: `copper-codegen/tests/admissible_calibration.rs`.
//! * R11 in the transpiler — *over* where `ConstEnv`'s grammar is narrower than rustc's:
//!   `copper-codegen/tests/register_init_parity.rs`.
//! * `infer_registers` — *over* by path-insensitivity, never under:
//!   `copper-codegen/tests/register_reconciliation.rs` (P13) and the poison run (P2).

use copper_analysis::front_end::check_two_flop;
use copper_analysis::{check_definite_assignment, check_reachability};
use syn::ItemFn;

fn f(src: &str) -> ItemFn {
    syn::parse_str(src).expect("witness parses")
}

const LIVELOCK_A: &str = "async fn m(clk: Clock<A>, c: In<Logic, A>, q: RegOut<Bits<8>, A>) { let mut n: Bits<8> = Bits::zero(); loop { loop { let t = c.read(); if t == Logic::One { n = n + Bits::from_lit::<1>(); q.write(n); clk.tick().await; } q.write(n); clk.tick().await; } } }";
const LIVELOCK_B: &str = "async fn m(clk: Clock<A>, c: In<Logic, A>, d: In<Logic, A>, q: RegOut<Bits<8>, A>) { let mut n: Bits<8> = Bits::zero(); loop { loop { let t = c.read(); if t == Logic::One { n = n + Bits::from_lit::<1>(); clk.tick().await; } else { clk.tick().await; } let done = d.read(); if done == Logic::One { break; } clk.tick().await; } q.write(n); } }";

#[test]
fn r3_over_refusal_narrowed_livelock_a_and_b_admitted() {
    assert!(check_reachability(&f(LIVELOCK_A)).is_ok(), "{:?}", check_reachability(&f(LIVELOCK_A)));
    assert!(check_reachability(&f(LIVELOCK_B)).is_ok(), "{:?}", check_reachability(&f(LIVELOCK_B)));
}

#[test]
fn r3_under_admission_narrowed_at_a_literal_empty_range() {
    for range in ["0..0", "5..3", "4..=3"] {
        let src = format!(
            "async fn m(clk: Clock<A>, q: RegOut<Bits<8>, A>) {{ loop {{ q.write(Bits::zero()); for _i in {range} {{ clk.tick().await; }} }} }}"
        );
        assert!(check_reachability(&f(&src)).is_err(), "{range}: the module loop's only boundary never runs");
    }
    let nonempty = "async fn m(clk: Clock<A>, q: RegOut<Bits<8>, A>) { loop { q.write(Bits::zero()); for _i in 0..3 { clk.tick().await; } } }";
    assert!(check_reachability(&f(nonempty)).is_ok());
}

/// Pinned: still *under* where the range is empty only through a constant — the walk is
/// syntactic and does not run the constant evaluator, so `0..EMPTY` with `EMPTY = 0`
/// counts as a boundary (a zero-time cycle the simulator would livelock on).
#[test]
fn r3_stays_under_at_a_range_empty_through_a_constant() {
    let src = "async fn m(clk: Clock<A>, q: RegOut<Bits<8>, A>) { loop { q.write(Bits::zero()); for _i in 0..EMPTY { clk.tick().await; } } }";
    assert!(check_reachability(&f(src)).is_ok(), "direction moved: re-derive II.2.2's R3 row");
}

/// Pinned: still *over* by path-insensitivity — two `if`s on complementary conditions
/// tick on every path, but each alone ticks on some path only, so the `break` after
/// them reads as reachable untick'd.
#[test]
fn r3_stays_over_by_path_insensitivity() {
    let src = "async fn m(clk: Clock<A>, c: In<Logic, A>, q: RegOut<Bits<8>, A>) { loop { loop { let t = c.read(); if t == Logic::One { clk.tick().await; } if t != Logic::One { clk.tick().await; } break; } q.write(Bits::zero()); } }";
    assert!(check_reachability(&f(src)).is_err(), "direction moved: re-derive II.2.2's R3 row");
}

#[test]
fn d2_under_admissions_narrowed() {
    let never = "#[hardware(combinational)] fn c(a: In<Logic, A>, o: Out<Logic, A>, p: Out<Logic, A>) { o.write(a.read()); }";
    assert!(check_definite_assignment(&f(never)).is_err());
    let in_for = "#[hardware(combinational)] fn c(a: In<Bits<4>, A>, o: Out<Logic, A>) { let t = a.read(); for i in 0..4 { if t[i] == Logic::One { o.write(Logic::One); } } }";
    assert!(check_definite_assignment(&f(in_for)).is_err());
}

/// Pinned: D2 stays *over* by path-insensitivity — complementary conditions write `o`
/// on every path, but the analysis joins each `if` alone.
#[test]
fn d2_stays_over_by_path_insensitivity() {
    let src = "#[hardware(combinational)] fn c(a: In<Logic, A>, o: Out<Logic, A>) { let t = a.read(); if t == Logic::One { o.write(Logic::One); } if t != Logic::One { o.write(Logic::Zero); } }";
    assert!(check_definite_assignment(&f(src)).is_err(), "direction moved: re-derive II.2.2's D2 row");
}

/// Pinned: the two-flop rule is *over* by path-insensitivity — the foreign read and the
/// write stand under complementary guards, so no execution writes `q` from `t`, but the
/// taint joins the arms.
#[test]
fn two_flop_is_over_by_path_insensitivity() {
    let src = "async fn s(clk: Clock<Slow>, d: In<Logic, Fast>, e: In<Logic, Slow>, q: RegOut<Logic, Slow>) { let mut t = Logic::Zero; loop { let g = e.read(); if g == Logic::One { t = d.read(); } if g != Logic::One { q.write(t); } clk.tick().await; } }";
    assert!(check_two_flop(&f(src)).is_err(), "direction moved: re-derive the two-flop rule's II.2.2 row");
}

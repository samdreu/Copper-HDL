use copper_core::{ClockDomain, Logic};
use copper_core::port::{In, Out};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC I.14.3(d)–(e) (D-24 (2)): a `Logic::X` pattern asks whether a value holds the
// simulator's third value — `is_valid()` spelled as a pattern, the same question as
// `v == Logic::X` (D-23 (3)) — and is OUTSIDE. Until 2026-11-08 the macro admitted it
// (a structural match) and the transpiler refused it ("unsupported pattern"); both now
// refuse it in `check_outside_methods`, with one message.
#[hardware(combinational)]
fn m(a: In<Logic, A>, q: Out<Logic, A>) {
    q.write(match a.read() {
        Logic::X => Logic::One,
        _ => Logic::Zero,
    });
}

fn main() {}

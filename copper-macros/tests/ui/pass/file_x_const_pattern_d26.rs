use copper_core::{ClockDomain, Logic};
use copper_core::port::{In, Out};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

const UX: Logic = Logic::X;

// Should pass: II.1.3's third named exception (D-26 (2)). A pattern naming a FILE-level
// `const` whose value is X is OUTSIDE (I.14.3(d)–(e), D-24 (2)), but the macro holds the
// module's `ItemFn` only and cannot see the const's value, so it admits it (rustc matches
// `Logic` structurally). The transpiler folds the const and refuses it with
// `X_PATTERN_MESSAGE` (`front_end_parity_ui.rs` pins that refusal). A body `const` with an X
// value, or `Logic::X` written in the pattern, is refused by both (`ui/fail/outside_x_pattern`).
#[hardware(combinational)]
fn m<D: ClockDomain>(a: In<Logic, D>, o: Out<Logic, D>) {
    match a.read() {
        UX => o.write(Logic::One),
        _ => o.write(Logic::Zero),
    }
}

fn main() {}

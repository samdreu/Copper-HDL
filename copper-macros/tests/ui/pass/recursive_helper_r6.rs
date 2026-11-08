use copper_core::{Bits, ClockDomain, Logic};
use copper_core::port::{In, Out};
use copper_macros::hardware;

// Should pass: R6, the first named exception to II.1.3 (D-22). The macro sees only the
// module, never the helper's body, so recursion through a helper `fn` compiles; the
// transpiler refuses it when it inlines the helper (`front_end_parity_ui.rs` pins
// that refusal and its message), and a terminating recursion simulates as Rust runs
// it (`tests/recursive_helper_r6.rs`).
fn parity_of(x: Bits<8>, k: usize) -> Logic {
    if k == 0 { Logic::Zero } else { x[k - 1] ^ parity_of(x, k - 1) }
}

#[hardware(combinational)]
fn parity8<D: ClockDomain>(a: In<Bits<8>, D>, p: Out<Logic, D>) {
    p.write(parity_of(a.read(), 8));
}

fn main() {}

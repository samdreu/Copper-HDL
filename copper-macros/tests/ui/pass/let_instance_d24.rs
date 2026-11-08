use copper_core::{Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

#[hardware(sequential)]
async fn child(clk: Clock<A>, o: RegOut<Logic, A>) {
    loop {
        o.write(Logic::One);
        clk.tick().await;
    }
}

// Should pass: II.1.3's second named exception (D-24 (1)). A `let` binding is a use,
// so the generated fn's denied `#[must_use]` (D-23 (1)) does not fire, and the macro
// cannot tell a `let`'s callee that is a module from a helper `fn`. The transpiler
// refuses it with I.13.5's message (`front_end_parity_ui.rs` pins that refusal), and
// the simulator refuses it at run time: the child's `HardwareModule` dropped unstarted
// panics (`tests/let_instance_d24.rs`).
#[hardware(sequential)]
async fn parent(clk: Clock<A>, a: In<Logic, A>, o: RegOut<Logic, A>) {
    let _ = child(clk.clone(), o);
    loop {
        let _v = a.read();
        clk.tick().await;
    }
}

fn main() {}

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

// SPEC I.13.5 (D-18 A2, D-23 (1)): instantiation is a structural body's form only.
// D-18 probe b2: a child called in a `sequential` body was admitted by the macro — its
// `HardwareModule` dropped, the child never run — and instantiated by the transpiler.
// rustc now refuses the call (the generated fn's `#[must_use]`, denied in every
// behavioural body; the note is I.13.5's message); the transpiler refuses it from its
// registry with the same message.
#[hardware(sequential)]
async fn parent(clk: Clock<A>, a: In<Logic, A>, o: RegOut<Logic, A>) {
    child(clk.clone(), o);
    loop {
        let _v = a.read();
        clk.tick().await;
    }
}

fn main() {}

use copper_core::{Clock, ClockDomain, Logic};
use copper_core::port::{net, In, Out, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

#[hardware(sequential)]
async fn src_r(clk: Clock<A>, q: RegOut<Logic, A>) {
    loop {
        q.write(Logic::One);
        clk.tick().await;
    }
}

#[hardware(combinational)]
fn sink_c(a: In<Logic, A>, o: Out<Logic, A>) {
    o.write(a.read());
}

// SPEC I.2.2 for the mode (R20), I.13.4(d): a structural module has a clock to thread.
// D-18 probe s8b: the macro refused it, the transpiler admitted it — shared since Phase C step 10.
#[hardware(structural)]
async fn top(i: In<Logic, A>, o: Out<Logic, A>) {
    sink_c(i, o);
}

fn main() {}

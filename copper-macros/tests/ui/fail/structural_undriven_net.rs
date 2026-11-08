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

// SPEC I.13.1 (exactly one driver), I.13.4(d): every net driven. D-18 probe s9: both
// front-ends admitted it (Verilator UNDRIVEN).
struct B;
impl ClockDomain for B {}

#[hardware(sequential)]
async fn hold(clk: Clock<A>, d: In<Logic, A>, q: RegOut<Logic, A>) {
    loop {
        let t = d.read();
        q.write(t);
        clk.tick().await;
    }
}

#[hardware(structural)]
async fn top(clk: Clock<A>, q: RegOut<Logic, A>) {
    let n = net::<Logic, A>();
    hold(clk, n.1, q);
}

fn main() {}

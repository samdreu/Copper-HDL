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

// SPEC I.13.4(d), I.2.5(c): no instance of the module itself. D-18 probe s13: both
// front-ends admitted it (the dropped future is no recursion to rustc; Verilator UNSUPPORTED).
#[hardware(structural)]
async fn top(clk: Clock<A>, o: Out<Logic, A>) {
    let n = net::<Logic, A>();
    src_r(clk.clone(), n.0);
    sink_c(n.1, o);
    let m = net::<Logic, A>();
    top(clk, m.0);
}

fn main() {}

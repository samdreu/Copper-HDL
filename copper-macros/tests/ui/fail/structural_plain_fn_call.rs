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

// SPEC I.13.4(d): an instantiation names a `#[hardware]` module. D-18 probe s12: the
// macro admitted a plain fn call; it now wraps each instantiation in `copper_sim::__instance`,
// so rustc refuses it (E0308) — the transpiler refuses it from its registry.
fn helper() {}

#[hardware(structural)]
async fn top(clk: Clock<A>, o: Out<Logic, A>) {
    let n = net::<Logic, A>();
    helper();
    src_r(clk, n.0);
    sink_c(n.1, o);
}

fn main() {}

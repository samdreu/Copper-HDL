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

// SPEC I.13.1, II.1 R22 (I.13.4(d)): one driver per net. D-18 probe s4: the transpiler
// admitted two instances driving one net (Verilator MULTIDRIVEN); rustc refused it (E0382).
#[hardware(structural)]
async fn top(clk: Clock<A>, o: Out<Logic, A>) {
    let n = net::<Logic, A>();
    src_r(clk.clone(), n.0);
    src_r(clk, n.0);
    sink_c(n.1, o);
}

fn main() {}

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

// SPEC I.13.4(b), (d) (D-18; II.1 S0): a structural body is nets and instantiations only.
// D-18 probe s1: the macro emitted the fn unchanged; the transpiler refused it.
#[hardware(structural)]
async fn top(clk: Clock<A>, o: Out<Logic, A>) {
    let n = net::<Logic, A>();
    let x = Logic::One;
    src_r(clk, n.0);
    sink_c(n.1, o);
}

fn main() {}

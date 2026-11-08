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

// SPEC I.13.4(c) (D-18 A3): `wire`/`registered_wire` are retired from the structural
// grammar; a net is `net::<T, D>()`, with no initial value.
use copper_core::port::wire;

#[hardware(structural)]
async fn top(clk: Clock<A>, o: Out<Logic, A>) {
    let n = wire::<Logic, A>(Logic::Zero);
    src_r(clk, n.0);
    sink_c(n.1, o);
}

fn main() {}

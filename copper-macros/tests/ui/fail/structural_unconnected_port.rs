use copper_core::{Clock, ClockDomain, Logic};
use copper_core::port::{net, In, Out, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

#[hardware(sequential)]
async fn hold(clk: Clock<A>, d: In<Logic, A>, q: RegOut<Logic, A>) {
    loop {
        let t = d.read();
        q.write(t);
        clk.tick().await;
    }
}

#[hardware(combinational)]
fn drive(o: Out<Logic, A>) {
    o.write(Logic::One);
}

// SPEC I.13.4(d): every port of a child is connected. `hold`'s `q` is left out of
// its instantiation (MEASUREMENTS §V.5).
#[hardware(structural)]
async fn top(clk: Clock<A>, _q: RegOut<Logic, A>) {
    let n = net::<Logic, A>();
    drive(n.0);
    hold(clk, n.1);
}

fn main() {}

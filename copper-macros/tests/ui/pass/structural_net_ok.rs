use copper_core::{Clock, ClockDomain, Logic};
use copper_core::port::{net, In, Out, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}
struct B;
impl ClockDomain for B {}

#[hardware(sequential)]
async fn src_r(clk: Clock<A>, q: RegOut<Logic, A>) {
    loop {
        q.write(Logic::One);
        clk.tick().await;
    }
}

#[hardware(synchronizer)]
async fn sync2(clk: Clock<B>, d: In<Logic, A>, q: RegOut<Logic, B>) {
    let mut ff1 = Logic::Zero;
    loop {
        let ff2 = ff1;
        ff1 = d.read();
        q.write(ff2);
        clk.tick().await;
    }
}

#[hardware(combinational)]
fn sink_c(a: In<Logic, B>, o: Out<Logic, B>) {
    o.write(a.read());
}

#[hardware(combinational)]
fn inv(a: In<Logic, B>, o: Out<Logic, B>) {
    o.write(!a.read());
}

// SPEC I.13.4(b)–(c) (D-18 A3, Phase C step 10): `net::<T, D>()` is one spelling for a
// `RegOut`-driven net (`a`, `s`) and an `Out`-driven one (`c`): the driver kind is the
// child port's, inferred; the net takes no initial value. A net's reader end goes to
// several readers; a clock reaching two children is cloned.
#[hardware(structural)]
async fn top(fast: Clock<A>, slow: Clock<B>, o: Out<Logic, B>, p: Out<Logic, B>) {
    let a = net::<Logic, A>();
    let s = net::<Logic, B>();
    let c = net::<Logic, B>();
    src_r(fast, a.0);
    sync2(slow, a.1, s.0);
    inv(s.1, c.0);
    sink_c(c.1.clone(), o);
    sink_c(c.1, p);
}

fn main() {}

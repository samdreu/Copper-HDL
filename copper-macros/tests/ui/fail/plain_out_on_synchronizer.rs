use copper_core::port::{In, Out};
use copper_core::{Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct Src;
impl ClockDomain for Src {}
struct Dst;
impl ClockDomain for Dst {}

// SPEC II.1 R8 (D-8): a `synchronizer` is a clocked module too — its output is the
// last flop, a `RegOut` (`copper::sync_2ff`).
#[hardware(synchronizer)]
async fn sync_plain(clk: Clock<Dst>, d: In<Logic, Src>, q: Out<Logic, Dst>) {
    let mut ff1 = Logic::Zero;
    loop {
        let ff2 = ff1;
        ff1 = d.read();
        q.write(ff2);
        clk.tick().await;
    }
}

fn main() {}

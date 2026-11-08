use copper_core::{Bits, ClockDomain, Logic};
use copper_core::port::{In, Out};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC II.2 D2, II.2.2's folded-loop direction (Phase C step 10): a port written only under
// an `if` inside a counted `for` is a latch. The macro's fold counted the loop as writing
// every port and admitted it; the transpiler's `check_no_latches` refused it.
#[hardware(combinational)]
fn any_set(a: In<Bits<4>, A>, o: Out<Logic, A>) {
    let t = a.read();
    for i in 0..4 {
        if t[i] == Logic::One {
            o.write(Logic::One);
        }
    }
}

fn main() {}

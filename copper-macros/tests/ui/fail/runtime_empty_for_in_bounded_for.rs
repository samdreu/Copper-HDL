use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC I.8.4(b), I.8.2(a), R3 (MEASUREMENTS §X.2): a ticking `for` over a run-time range
// that is not the last statement of a bounded enclosing `for` (that case is
// `nested_runtime_empty_for`, §J.15's rule): with `cnt == 0` an enclosing iteration takes no
// time, an iteration with no boundary. Refused by R3 since 2026-11-08 with its bounded-`for`
// wording, as `while g { … }` in its place is; before, both front-ends refused it with
// `check_nested_loop_shape`'s message (the `for` body does not end at its tick).
#[hardware(sequential)]
async fn m(clk: Clock<A>, c: In<Bits<2>, A>, g: In<Logic, A>, q: RegOut<Bits<8>, A>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        let cnt: u32 = c.read().as_u32();
        for _ in 0..2 {
            for _ in 0..cnt {
                q.write(n);
                clk.tick().await;
            }
            n = n + Bits::from_lit::<1>();
        }
        q.write(Bits::from_lit::<255>());
        clk.tick().await;
    }
}

fn main() {}

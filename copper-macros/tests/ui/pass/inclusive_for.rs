use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_core::port::{In, Out, RegOut};
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

const LAST: u8 = 254;

// D-27 (2026-11-09): a `for` over an inclusive range is IN with Rust's meaning — tick-free
// (unrolled `b − a + 1` times, an end at its type's maximum included) and tick-bearing (I.8.5,
// exiting on `c > e`; a typed constant end below its type's maximum passes II.1.2 R23).
#[hardware(combinational)]
fn popcount_low<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let mut s: u8 = 0;
    for i in 0..=3 {
        if a.read()[i] == Logic::One {
            s = s.wrapping_add(1);
        }
    }
    for i in 250u8..=255u8 {
        s = s ^ i;
    }
    o.write(Bits::from_u8(s));
}

#[hardware(sequential)]
async fn delays(clk: Clock<A>, a: In<Bits<8>, A>, q: RegOut<Bits<8>, A>) {
    let mut n: u8 = 0;
    loop {
        for _ in 0..=2 {
            q.write(Bits::from_u8(n));
            clk.tick().await;
        }
        for i in 250..=LAST {
            q.write(Bits::from_u8(i));
            clk.tick().await;
        }
        n = n ^ a.read().as_u8();
    }
}

fn main() {}

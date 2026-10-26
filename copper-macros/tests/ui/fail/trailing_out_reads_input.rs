#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: `o` is a plain `Out` written after the last tick from `x`, read in
// that same trailing region, in a body that crosses two clock edges. The
// simulator reads `x` the instant the write executes and shows `x + cnt` at the
// next observation; the emitted drive samples `x` one observation later, so the
// two disagree by one cycle (the e8 class). A single-tick body
// (`loop { clk.tick().await; o.write(x.read() + cnt); }`) agrees and is not
// refused; through a register (`r = x.read(); … o.write(r)`) the value is held
// and the two agree.
#[hardware(sequential)]
async fn trailing_out_from_input(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: Out<Bits<8>, MainClk>) {
    let mut cnt: Bits<8> = Bits::zero();
    loop {
        cnt = cnt + Bits::from_lit::<1>();
        for _ in 0..2 {
            clk.tick().await;
        }
        o.write(x.read() + cnt);
    }
}

fn main() {}

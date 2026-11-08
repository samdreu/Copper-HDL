use copper_core::port::{In, RegOut};
use copper_core::{Bits, Clock, ClockDomain};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail (D-21 §N.4 H1, prototype): `x` is computed once from an input before the
// loop and read in every resumption — state with the input's value at power-on. The
// simulator and the interpreter hold the first read; the transpiler emitted a wire
// (measured 6/6 sim=ref≠SV before this rule). Same function, same message, in the transpiler.
#[hardware(sequential)]
async fn preloop_held_read(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    let x = a.read();
    loop {
        o.write(x);
        clk.tick().await;
    }
}

fn main() {}

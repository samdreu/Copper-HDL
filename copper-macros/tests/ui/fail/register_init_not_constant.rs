use copper_core::port::{In, RegOut};
use copper_core::{Bits, Clock, ClockDomain};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

fn seed() -> Bits<8> {
    Bits::from_u8(7)
}

// Should fail: `acc` is a register (reassigned in the loop, live across the tick),
// and its initializer is a call to a free function — not a constant the emitted
// SystemVerilog can state as the flip-flop's power-on value. The simulator would
// start at whatever `seed()` returns and the hardware at nothing in particular. The
// fix the message names: a constant (`Bits::<8>::from_u8(7)`), or `Bits::x()` plus a
// reset.
#[hardware(sequential)]
async fn seeded_accumulator(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    let mut acc = seed();
    loop {
        clk.tick().await;
        acc = acc + a.read();
        o.write(acc);
    }
}

fn main() {}

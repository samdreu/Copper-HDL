// SPEC I.10.1(a) (D-25 (3)): a received memory with READ_LAT = 0 — admitted by the macro and a
// transpiler crash (vlir_lower, a subtraction overflow) before D-25; now refused by both.
use copper_core::{Bits, Clock, ClockDomain, Memory};
use copper_core::port::{In, Out, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

#[hardware(sequential)]
async fn recv_l0(clk: Clock<MainClk>, m: Memory<Bits<8>, 1, 1, MainClk, 0, 1>, a: In<Bits<4>, MainClk>, d: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        m.write_port::<0>().write(a.read().as_usize(), d.read());
        m.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        if m.read_port::<0>().is_ready() { q.write(m.read_port::<0>().data()); }
    }
}

fn main() {}

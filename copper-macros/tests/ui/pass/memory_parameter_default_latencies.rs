// SPEC I.10.1(a), I.10.5 (D-25 (5)): a `Memory` parameter may leave READ_LAT and WRITE_LAT at the
// type's defaults (1, 1) — admitted by both front-ends; the transpiler reads the defaults.
use copper_core::{Bits, Clock, ClockDomain, Memory};
use copper_core::port::{In, Out, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

#[hardware(sequential)]
async fn recv_short(clk: Clock<MainClk>, m: Memory<Bits<8>, 1, 0, MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        m.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        if m.read_port::<0>().is_ready() { q.write(m.read_port::<0>().data()); }
    }
}

fn main() {}

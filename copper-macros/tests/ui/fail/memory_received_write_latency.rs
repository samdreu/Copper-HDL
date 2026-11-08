// SPEC I.10.5, II.3.3 row (k) (D-16, D-25): a received memory's WRITE_LAT must be 1 — the bus
// carries the freshly staged write nets. REFUSED-both; fix direction: expose the committing stage.
use copper_core::{Bits, Clock, ClockDomain, Memory};
use copper_core::port::{In, Out, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

#[hardware(sequential)]
async fn recv_wl2(clk: Clock<MainClk>, m: Memory<Bits<8>, 1, 1, MainClk, 1, 2>, a: In<Bits<4>, MainClk>, d: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        m.write_port::<0>().write(a.read().as_usize(), d.read());
        m.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        if m.read_port::<0>().is_ready() { q.write(m.read_port::<0>().data()); }
    }
}

fn main() {}

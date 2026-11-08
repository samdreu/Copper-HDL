// SPEC I.10.5 (D-25 (1)): a memory is a parameter of a `sequential` or `synchronizer`
// module only, received by exactly one module; a `structural` parent receives none.
use copper_core::{Bits, Clock, ClockDomain, Memory};
use copper_core::port::{In, Out, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

#[hardware(sequential)]
async fn recv_child(clk: Clock<MainClk>, m: Memory<Bits<8>, 1, 0, MainClk, 1, 1>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        m.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        if m.read_port::<0>().is_ready() { q.write(m.read_port::<0>().data()); }
    }
}

#[hardware(structural)]
async fn struct_one(clk: Clock<MainClk>, m: Memory<Bits<8>, 1, 0, MainClk, 1, 1>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    recv_child(clk, m, a, q);
}

fn main() {}

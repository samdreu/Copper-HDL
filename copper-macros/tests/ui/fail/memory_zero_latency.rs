// SPEC I.10.1(a) (D-16 2A, D-25 (3)): READ_LAT = 0 is OUTSIDE — a synchronous port cannot
// answer in zero cycles. Refused by both front-ends (and by the simulator's type at construction).
use copper_core::{Bits, Clock, ClockDomain, Memory};
use copper_core::port::{In, Out, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

#[hardware(sequential)]
async fn l0_mod(clk: Clock<MainClk>, a: In<Bits<4>, MainClk>, q: RegOut<Bits<16>, MainClk>) {
    let rom = Memory::<Bits<16>, 1, 0, MainClk, 0, 1>::from_fn(clk.clone(), 16, |i| Bits::from_usize(i * 3 + 7));
    loop {
        rom.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        if rom.read_port::<0>().is_ready() { q.write(rom.read_port::<0>().data()); }
    }
}

fn main() {}

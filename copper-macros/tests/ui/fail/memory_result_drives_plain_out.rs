#![allow(unused_imports)]
use copper_core::port::{In, Out};
use copper_core::{Bits, Clock, ClockDomain, Logic, Memory};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail: a plain `Out` driven straight from a memory read result in a phase
// that does not stage it. The read pipeline re-captures on every clock edge, so
// the output either tracks it into the phase that does not observe it or holds
// one edge late — measured a full cycle late. The remedy is `RegOut` (the
// `mp_regout` fixture) or a register between the result and the port. Until
// 2026-10-23 only the transpiler refused this; it simulated and then failed to
// transpile.
#[hardware(sequential)]
async fn mp_rom(clk: Clock<MainClk>, addr: In<Bits<4>, MainClk>, data: Out<Bits<16>, MainClk>) {
    let rom = Memory::<Bits<16>, 1, 0, MainClk, 1, 1>::from_fn(clk.clone(), 16, |i| {
        Bits::from_usize(i * 3 + 7)
    });
    loop {
        rom.read_port::<0>().read(addr.read().as_usize());
        clk.tick().await;
        data.write(rom.read_port::<0>().data());
        clk.tick().await;
    }
}

fn main() {}

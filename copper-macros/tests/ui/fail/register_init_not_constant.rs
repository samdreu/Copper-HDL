use copper_core::port::{In, RegOut};
use copper_core::{Bits, Clock, ClockDomain};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

fn seed() -> Bits<8> {
    Bits::from_u8(7)
}

// Should fail: `acc` is a register (reassigned in the loop, live across the tick),
// and its initializer is a call to a free function that is not a `const fn` — not a
// constant the emitted SystemVerilog can state as the flip-flop's power-on value.
// Since D-21 A3 (2026-11-07) rustc judges it: the macro evaluates every pre-loop
// register initializer in a `const { }` block, and the refusal is rustc's E0015
// pointing at the call. (Until D-21 the macro refused it syntactically, with R11's
// message; that message now covers only an initializer reading a run-time value —
// `register_init_reads_input.rs`.) The transpiler refuses the same source with
// `copper_analysis::register_init_call_message` (copper-codegen/tests/register_init_parity.rs).
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

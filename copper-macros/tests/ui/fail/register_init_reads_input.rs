use copper_core::port::{In, RegOut};
use copper_core::{Bits, Clock, ClockDomain};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should fail with R11's own message (D-21 A3): `acc` is a register and its
// initializer reads an input port, a run-time value the hardware does not have at
// power-on. rustc would refuse it too inside the macro's `const { }` (E0435), but its
// message is about `const` items; the macro's syntactic half of R11 runs first and
// says why a register needs a constant. Same function, same message, in the
// transpiler (copper-codegen/tests/register_init_parity.rs).
#[hardware(sequential)]
async fn input_seeded_accumulator(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    let mut acc = a.read();
    loop {
        clk.tick().await;
        acc = acc + a.read();
        o.write(acc);
    }
}

fn main() {}

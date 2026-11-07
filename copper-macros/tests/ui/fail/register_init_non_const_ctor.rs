use copper_core::port::{In, RegOut};
use copper_core::{Bits, Clock, ClockDomain};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

#[derive(Clone, Copy)]
struct Latch {
    valid: bool,
    data: Bits<8>,
}

impl Latch {
    // Not a `const fn`.
    fn bubble() -> Self {
        Self { valid: false, data: Bits::zero() }
    }
}

// Should fail (D-21 A3): the struct register `latch` starts from an associated ctor that
// is not `const`. Until D-21 R11's syntactic test admitted it (a multi-segment call with
// constant arguments) and the transpiler inlined it structurally — the over-admission of
// MEASUREMENTS §M.10 (A). rustc now refuses it in the macro's `const { }` (E0015), and
// the transpiler refuses it with `register_init_call_message` (it is not a `const fn` of
// the file). The fix is one keyword: `const fn bubble()` — as the pipelined CPU's are.
#[hardware(sequential)]
async fn latch_from_runtime_ctor(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    let mut latch = Latch::bubble();
    loop {
        clk.tick().await;
        o.write(if latch.valid { latch.data } else { Bits::zero() });
        latch = Latch { valid: true, data: x.read() };
    }
}

fn main() {}

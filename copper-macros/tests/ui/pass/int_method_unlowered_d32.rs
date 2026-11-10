use copper_core::{Clock, ClockDomain};
use copper_core::port::{In, RegOut};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

// Should pass: an integer method the transpiler does not lower, the fourth named exception to
// II.1.3 (D-32 (1)). rustc type-checks `wrapping_pow` and the macro sees no types, so this
// compiles and simulates with Rust's meaning; the transpiler refuses it with one message,
// `copper_analysis::UNLOWERED_METHOD_MESSAGE` (`front_end_parity_ui.rs` pins that refusal),
// and never emits text. Its extent is `copper_codegen::LOWERED_INT_METHODS`' complement.
#[hardware(sequential)]
async fn int_method_unlowered(clk: Clock<MainClk>, a: In<u16, MainClk>, q: RegOut<u16, MainClk>) {
    loop {
        q.write(a.read().wrapping_pow(3));
        clk.tick().await;
    }
}

fn main() {}

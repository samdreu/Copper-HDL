use copper_core::{Bits, Clock, ClockDomain};
use copper_core::port::RegOut;
use copper_macros::hardware;

struct A;
impl ClockDomain for A {}

// SPEC I.14.4, II.1.2 R11 (D-21 A3): rustc admits each initializer in the macro's
// `const { }` — a `const` integer method call, a tuple-struct constructor's field, a
// tuple-struct const's field, an immutable `static` (line `II.3 found const-call`,
// 2026-11-08, MEASUREMENTS §V.4). `copper-codegen/tests/register_init_parity.rs`
// requires the transpiler to fold each to rustc's value.
const K: u8 = 255;
struct W(u8, u8);
const PAIR: W = W(3, 5);
static SEED: u8 = 0x21;

#[hardware(sequential)]
async fn m(clk: Clock<A>, q: RegOut<Bits<8>, A>) {
    let mut via_method: u8 = K.wrapping_add(2);
    let mut via_ctor: u8 = W(7, 1).0;
    let mut via_field: u8 = PAIR.1;
    let mut via_static: u8 = SEED;
    loop {
        q.write(Bits::from_u8(via_method ^ via_ctor ^ via_field ^ via_static));
        clk.tick().await;
        via_method = via_method.wrapping_add(1);
        via_ctor = via_ctor.wrapping_add(1);
        via_field = via_field.wrapping_add(1);
        via_static = via_static.wrapping_add(1);
    }
}

fn main() {}

use copper_core::port::{In, RegOut};
use copper_core::{Bits, Clock, ClockDomain};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

const SEED: u8 = 7;

// D-21 A3 (2026-11-07): rustc judges a register initializer (the macro wraps it in
// `const { }`), so every Rust constant is admitted — a file const, a free `const fn`
// call (which the syntactic test refused until then), an associated const, an
// associated `const fn`, a `const` struct ctor. The transpiler admits this file too and
// states the same power-on values (copper-codegen/tests/register_init_parity.rs); the
// same spellings are swept on all three legs as tests/fixtures/const_init_dut.rs.
const fn seed_of(k: u8) -> u8 {
    k * 3 + 1
}

struct Cfg;
impl Cfg {
    const K: u8 = 11;
    const fn twice(x: u8) -> u8 {
        x * 2
    }
}

#[derive(Clone, Copy)]
struct Latch {
    valid: bool,
    data: Bits<8>,
}
impl Latch {
    const fn bubble() -> Self {
        Self { valid: false, data: Bits::zero() }
    }
}

#[hardware(sequential)]
async fn rust_constant_starts(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    o: RegOut<Bits<8>, MainClk>,
) {
    let mut from_const = Bits::<8>::from_u8(SEED);
    let mut via_fn: Bits<8> = Bits::from_u8(seed_of(3));
    let mut assoc: u8 = Cfg::K;
    let mut assoc_fn = Bits::<8>::from_u8(Cfg::twice(Cfg::K));
    let mut latch = Latch::bubble();
    loop {
        clk.tick().await;
        from_const = from_const + a.read();
        via_fn = via_fn + a.read();
        assoc = assoc.wrapping_add(1);
        assoc_fn = assoc_fn ^ a.read();
        let shown = if latch.valid { latch.data } else { Bits::zero() };
        latch = Latch { valid: !latch.valid, data: a.read() };
        o.write(from_const + via_fn + assoc_fn + shown + Bits::from_u8(assoc));
    }
}

fn main() {}

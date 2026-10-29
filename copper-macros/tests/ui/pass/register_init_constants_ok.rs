use copper_core::port::{In, RegOut};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;

struct MainClk;
impl ClockDomain for MainClk {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Busy,
}

const SEED: u8 = 7;

// Every initializer here is ACCEPTED by the register power-on rule: each is a
// constant the transpiler states as the register's `initial` value — or `Bits::x()`,
// which states nothing and says the register starts undefined (it is reset below
// before it is read).
#[hardware(sequential)]
async fn constant_starts(
    clk: Clock<MainClk>,
    reset: In<Logic, MainClk>,
    a: In<Bits<8>, MainClk>,
    o: RegOut<Bits<8>, MainClk>,
) {
    let mut lit: u8 = 0;
    let mut from_u32: Bits<32> = Bits::from_u32(1);
    let mut from_u8 = Bits::<8>::from_u8(3);
    let mut from_lit: Bits<8> = Bits::from_lit::<0>();
    let mut zero: Bits<8> = Bits::zero();
    let mut from_const = Bits::<8>::from_u8(SEED);
    let mut bit = Logic::Zero;
    let mut phase = Phase::Idle;
    let mut undefined: Bits<8> = Bits::x();
    let mut bits = [Logic::Zero; 4];
    loop {
        clk.tick().await;
        if reset.read() == Logic::One {
            undefined = Bits::zero();
        } else {
            undefined = undefined + a.read();
        }
        lit = lit.wrapping_add(1);
        from_u32 = from_u32 + Bits::from_u32(1);
        from_u8 = from_u8 + a.read();
        from_lit = from_lit + a.read();
        zero = zero + a.read();
        from_const = from_const + a.read();
        bit = if bit == Logic::One { Logic::Zero } else { Logic::One };
        phase = match phase {
            Phase::Idle => Phase::Busy,
            Phase::Busy => Phase::Idle,
        };
        bits[0] = bits[1];
        bits[1] = bit;
        let _ = (lit, from_u32, phase, bits);
        o.write(undefined + from_u8 + from_lit + zero + from_const);
    }
}

fn main() {}

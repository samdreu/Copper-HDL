// Shapes the II.1.3 re-verification of 2026-11-08 found the macro admitting and the transpiler
// refusing (MEASUREMENTS §Z.8, a 46-probe battery extending §X.4's 23), each IN or DESUGARED
// under SPEC I.2.4, each lowered now and swept here on three legs: a binder in a statement
// `match`, a `Bits`/`Logic` const (file-scope or body-local), a body-local `struct` item, an
// inclusive tick-free `for`, an `impl` method call on a struct local, an associated const as a
// pattern, an array-literal register, a dynamic index into a `[Logic; N]` local; and their
// neighbours the battery also probed (an `@`
// range, a tuple range with a binder, a `Logic` array literal, a compound assignment to an
// element or a field, a struct field's bit, a signed range, a `RegOut` write in a combinational
// `for`).

const PB_MASK: Bits<8> = Bits::from_u8(0x33);
const PB_ONE: Logic = Logic::One;

struct PbT;

impl PbT {
    const K: u8 = 7;
}

#[derive(Clone, Copy)]
struct PbS {
    x: u8,
}

impl PbS {
    fn twice(self) -> u8 {
        self.x.wrapping_add(self.x)
    }
    fn plus(&self, k: u8) -> u8 {
        self.x.wrapping_add(k)
    }
}

#[derive(Clone, Copy)]
struct PbBits {
    x: Bits<4>,
    y: Bits<4>,
}

/// A binder in a statement `match`, alone and in a tuple, read by its guard and body.
#[hardware(sequential)]
async fn pbt_stmt_binder(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, b: In<Bits<2>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: u8 = 0;
    loop {
        let v = a.read().as_u8();
        let k = b.read().as_u8();
        match v {
            0 => n = 1,
            x if x > 9 => n = n ^ x,
            _ => n = n.wrapping_add(1),
        }
        match (k, v) {
            (0, y) => n = n.wrapping_add(y),
            (z, 5) => n = n ^ z,
            _ => {}
        }
        q.write(Bits::from_u8(n));
        clk.tick().await;
    }
}

/// A `Bits` and a `Logic` file const, a body-local `Bits` const, all in expressions and one in a
/// pattern.
#[hardware(combinational)]
pub fn pbt_bits_const<D: ClockDomain>(a: In<Bits<8>, D>, c: In<Logic, D>, o: Out<Bits<8>, D>) {
    const PB_LOCAL: Bits<8> = Bits::from_u8(0x0F);
    let v = a.read() ^ PB_MASK;
    let w = if c.read() == PB_ONE { v & PB_LOCAL } else { v };
    let r = match c.read() {
        PB_ONE => w,
        _ => w ^ PB_LOCAL,
    };
    o.write(r);
}

/// A body-local `struct` item and an `impl` method call (`self`, `&self`) on a struct local.
#[hardware(combinational)]
pub fn pbt_body_struct_method<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    #[derive(Clone, Copy)]
    struct PbP {
        x: u8,
        y: u8,
    }
    let p = PbP { x: a.read().as_u8(), y: 5 };
    let s = PbS { x: p.x ^ p.y };
    o.write(Bits::from_u8(s.twice() ^ s.plus(3)));
}

/// An associated const as a pattern and in an expression, an `@` range, a tuple range with a
/// binder, a signed range (a negated literal endpoint).
#[hardware(combinational)]
pub fn pbt_patterns_ranges<D: ClockDomain>(a: In<Bits<8>, D>, b: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let s: u8 = PbT::K.wrapping_mul(2);
    let r: u8 = match a.read().as_u8() {
        PbT::K => 1,
        n @ 1..=9 => n,
        n @ 200.. => n - 200,
        _ => 0,
    };
    let t: u8 = match (a.read().as_u8(), b.read().as_u8()) {
        (0..=9, y) => y,
        (x, 100..) => x,
        _ => 3,
    };
    let g: u8 = match b.read().as_u8() as i8 {
        -128..=-1 => 1,
        0..=9 => 2,
        _ => 4,
    };
    o.write(Bits::from_u8(s ^ r ^ t ^ g));
}

/// An array-literal register: four stated power-on words, one rewritten per cycle (before the
/// tick: a write after it read back in the next iteration is line `II.3 found
/// array-register-trailing-write`).
#[hardware(sequential)]
async fn pbt_array_literal_register(clk: Clock<MainClk>, a: In<Bits<2>, MainClk>, d: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut r: [u8; 4] = [1, 2, 30, 4];
    loop {
        q.write(Bits::from_u8(r[a.read().as_usize()]));
        r[(a.read().as_usize() + 1) & 3] = d.read().as_u8();
        clk.tick().await;
    }
}

/// A `Logic` array literal, a compound assignment to an element and to a field, a struct
/// field's bit, a struct literal reading the struct it is assigned to in a combinational body.
#[hardware(sequential)]
async fn pbt_aggregates(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, d: In<Logic, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut s = PbBits { x: Bits::zero(), y: Bits::zero() };
    loop {
        let l: [Logic; 4] = [Logic::One, Logic::Zero, d.read(), Logic::One];
        let mut t: [u8; 2] = [3, 4];
        t[1] += a.read().as_u8() & 0x0F;
        let mut p = PbS { x: t[0] };
        p.x ^= t[1];
        let bit = l[(a.read().as_u8() & 3) as usize];
        q.write(Bits::from_u8(p.x) ^ Bits::from_u8(s.x.as_u8() | (s.y.as_u8() << 4)) ^ Bits::from_u8(if bit == Logic::One { 0x80 } else { 0 }));
        clk.tick().await;
        s.x[1] = d.read();
        s.y = s.x;
    }
}

/// A struct literal reading the struct it is assigned to, and a port written in a `for`, in a
/// combinational body.
#[hardware(combinational)]
pub fn pbt_comb_self_and_for<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>, p: Out<Bits<8>, D>) {
    let mut s = PbS { x: a.read().as_u8() };
    let mut u = PbBits { x: Bits::from_lit::<1>(), y: Bits::from_lit::<6>() };
    u = PbBits { x: u.y, y: u.x };
    s = PbS { x: s.x.wrapping_add(u.y.as_u8()) };
    o.write(Bits::from_u8(s.x));
    p.write(Bits::zero());
    for i in 0..2 {
        if a.read()[i] == Logic::One {
            p.write(a.read());
        }
    }
}


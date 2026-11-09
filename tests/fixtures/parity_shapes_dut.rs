// Six shapes the macro admits and SPEC I.2.4 admits (IN or DESUGARED), which the transpiler
// refused until 2026-11-08 (MEASUREMENTS §X.4, §Z.3–§Z.6; the OPEN lines `II.3 found
// body-const`, `compound-assign`, `field-assign`, `array-literal`, `range-pattern`,
// `assoc-const-body` of SPEC §III.3.4). Each now lowers: a body-local `const` and an
// associated const `T::K` are hoisted into the file-const machinery (`desugar.rs`), `v op= e`
// is `v = v op e`, a range pattern is a guard, `s.x = e` assigns the field's net, `[c, …]` is
// the packed vector its type describes.

struct PsK;

impl PsK {
    const STEP: u8 = 7;
    const REPS: usize = 3;
}

#[derive(Clone, Copy)]
struct PsPair {
    x: u8,
    y: u8,
}

/// A body-local const in a clocked body: an expression, a tick-free bound, a ticking bound.
#[hardware(sequential)]
async fn ps_body_const(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    const PS_ADD: u8 = 5;
    let mut n: u8 = 0;
    loop {
        const PS_TRIPS: usize = 2;
        let mut s: u8 = n;
        for _ in 0..PS_TRIPS {
            s = s.wrapping_add(PS_ADD);
        }
        for _ in 0..PS_TRIPS {
            q.write(Bits::from_u8(s ^ n));
            clk.tick().await;
        }
        n = n.wrapping_add(a.read().as_u8());
    }
}

/// A body-local const in a combinational body.
#[hardware(combinational)]
pub fn ps_body_const_comb<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    const PS_MASK: u8 = 0x5A;
    o.write(Bits::from_u8(a.read().as_u8() ^ PS_MASK));
}

/// Compound assignment on a `u8` register, a `Bits` register and a wire.
#[hardware(sequential)]
async fn ps_compound(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: u8 = 1;
    let mut b: Bits<8> = Bits::zero();
    loop {
        q.write(Bits::from_u8(n) ^ b);
        clk.tick().await;
        let mut w: u8 = a.read().as_u8();
        w &= 0x3F;
        w |= 0x40;
        n ^= w;
        n <<= 1;
        n >>= 1;
        b ^= a.read();
    }
}

/// Compound assignment in a combinational body.
#[hardware(combinational)]
pub fn ps_compound_comb<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let mut v: u8 = a.read().as_u8() & 0x7F;
    v += 3;
    v ^= 1;
    v -= 1;
    o.write(Bits::from_u8(v));
}

/// A field assignment on a struct register and on a struct wire; a field's bit and `+=`.
#[hardware(sequential)]
async fn ps_field_assign(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut s = PsPair { x: 0, y: 0 };
    loop {
        q.write(Bits::from_u8(s.x ^ s.y));
        clk.tick().await;
        let mut t = PsPair { x: a.read().as_u8(), y: 1 };
        t.y = t.x.wrapping_add(s.y);
        s.x = t.y;
        s.y = s.y.wrapping_add(1);
        s.x ^= t.x;
    }
}

/// A field assignment in a combinational body.
#[hardware(combinational)]
pub fn ps_field_assign_comb<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let mut s = PsPair { x: 1, y: 2 };
    s.x = a.read().as_u8();
    o.write(Bits::from_u8(s.x ^ s.y));
}

/// Array literals of `u8` and `Bits` elements, and the repeat form `[9; 4]` of `u8` elements
/// (which emitted 64-bit words until 2026-11-08, §Z.4), each read at a run-time index.
#[hardware(sequential)]
async fn ps_array_literal(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let t: [u8; 4] = [1, 2, 30, 4];
        let u: [Bits<8>; 2] = [Bits::from_u8(0x11), Bits::from_u8(0x80)];
        let r: [u8; 4] = [9; 4];
        let i = (a.read().as_u8() & 3) as usize;
        let j = ((a.read().as_u8() >> 2) & 3) as usize;
        let k = ((a.read().as_u8() >> 4) & 1) as usize;
        q.write(Bits::from_u8(t[i] ^ r[j]) ^ u[k]);
        clk.tick().await;
    }
}

/// An array literal in a combinational body, read at a run-time index.
#[hardware(combinational)]
pub fn ps_array_literal_comb<D: ClockDomain>(a: In<Bits<2>, D>, o: Out<Bits<8>, D>) {
    let t: [Bits<8>; 4] = [Bits::from_u8(3), Bits::from_u8(5), Bits::from_u8(7), Bits::from_u8(11)];
    o.write(t[a.read().as_usize()]);
}

/// Range patterns: inclusive, exclusive, open-ended, in an or-list, with `@`, with a guard, in
/// a tuple, in a statement `match`.
#[hardware(sequential)]
async fn ps_range_pattern(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, b: In<Bits<2>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut acc: u8 = 0;
    loop {
        let v = a.read().as_u8();
        let k = b.read().as_u8();
        let r: u8 = match v {
            0..=3 => 1,
            4..10 => 2,
            12 | 20..=29 => 3,
            n @ 30..=39 => n - 30,
            40..=49 if k == 0 => 4,
            200.. => 5,
            _ => 6,
        };
        let s: u8 = match (k, v) {
            (0..=1, 0..=127) => 7,
            (2, _) => 8,
            _ => 9,
        };
        match v {
            0..=99 => acc = acc.wrapping_add(1),
            _ => acc = acc ^ 0x10,
        }
        q.write(Bits::from_u8(r ^ (s << 4) ^ acc));
        clk.tick().await;
    }
}

/// A range pattern in a combinational body.
#[hardware(combinational)]
pub fn ps_range_pattern_comb<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let r: u8 = match a.read().as_u8() {
        0..=3 => 1,
        4..=9 => 2,
        ..=99 => 3,
        _ => 4,
    };
    o.write(Bits::from_u8(r));
}

/// Associated consts in a clocked body: an expression, a tick-free bound, a ticking bound.
#[hardware(sequential)]
async fn ps_assoc_const(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: u8 = 0;
    loop {
        let mut s: u8 = n;
        for _ in 0..PsK::REPS {
            s = s.wrapping_add(PsK::STEP);
        }
        for _ in 0..PsK::REPS {
            q.write(Bits::from_u8(s));
            clk.tick().await;
        }
        n = n.wrapping_add(PsK::STEP) ^ a.read().as_u8();
    }
}

/// An associated const in a combinational body.
#[hardware(combinational)]
pub fn ps_assoc_const_comb<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    o.write(Bits::from_u8(a.read().as_u8() ^ PsK::STEP));
}

/// A guarded arm in a statement `match` (found by §Z.5: refused downstream as a "tuple-pattern
/// match" before, with or without a range): the arms are an `if` chain.
#[hardware(sequential)]
async fn ps_stmt_match_guard(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, b: In<Bits<2>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut acc: u8 = 0;
    loop {
        let v = a.read().as_u8();
        let k = b.read().as_u8();
        match (k, v) {
            (0, 3) => acc = acc.wrapping_add(1),
            (_, 7) if k == 1 => acc = 4,
            _ if v > 200 => acc = acc ^ 0x21,
            (2, _) => acc = acc.wrapping_sub(v),
            _ => acc = acc ^ 0x10,
        }
        q.write(Bits::from_u8(acc));
        clk.tick().await;
    }
}

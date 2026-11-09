// A `for` loop variable has its Rust type (SPEC I.14.1: every expression has its Rust type; line
// `II.3 found loop-var-type`, MEASUREMENTS §AA.3, §AB.2). rustc types `v` in `for v in a..b` as the
// range's element type: a typed bound's, or — both bounds untyped — the type a use unifies it with
// (`a.as_u8() == i` makes it `u8`), `i32` when nothing does. Read as an operand without a cast it is
// that type's width and signedness. Until 2026-11-09 the transpiler read it as its 32-bit counter
// (Verilator's WIDTHEXPAND against an 8-bit operand) and the interpreter typed untyped bounds
// `usize` ("mixed integer types u8 and usize"). Exclusive and inclusive alike; each module is swept
// on three legs.

/// The §AA.3 probe `zz_excl_typed_var`: a typed range, tick-free, the variable an operand.
#[hardware(sequential)]
async fn lvt_typed_tickfree(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut s: u8 = 0;
        for i in 250u8..255u8 {
            s = s.wrapping_add(i ^ a.read().as_u8());
        }
        q.write(Bits::from_u8(s));
        clk.tick().await;
    }
}

/// The §AA.3 probe `zz_excl_break`: untyped bounds, the type fixed by a use (`== i`), a `break`
/// the author wrote, tick-bearing.
#[hardware(sequential)]
async fn lvt_use_ticking_break(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut t: u8 = 0;
    loop {
        for i in 0..4 {
            if a.read().as_u8() == i {
                break;
            }
            t = t.wrapping_add(1);
            q.write(Bits::from_u8(t));
            clk.tick().await;
        }
        q.write(Bits::from_u8(0x80 | t));
        clk.tick().await;
    }
}

/// Untyped bounds, the type fixed by a use, tick-free, in a combinational body; the variable also
/// read through an untyped `let` copy and in arithmetic with a literal.
#[hardware(combinational)]
fn lvt_use_comb<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let mut s: u8 = 0;
    for i in 0..=3 {
        let k = i + 1;
        if a.read().as_u8() > i {
            s = s.wrapping_add(k);
        }
    }
    o.write(Bits::from_u8(s));
}

/// A typed range, tick-bearing: the counter is a register, the variable read at `u8`, and `u16`
/// through a widening use.
#[hardware(sequential)]
async fn lvt_typed_ticking(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<16>, MainClk>) {
    loop {
        for i in 3u8..7 {
            let w: u16 = (i ^ a.read().as_u8()) as u16;
            q.write(Bits::from_u16(w.wrapping_mul(i as u16)));
            clk.tick().await;
        }
    }
}

/// A signed range: `i8`, negative values, a signed compare and a signed sum, tick-free and ticking.
#[hardware(sequential)]
async fn lvt_signed(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let x = a.read().as_u8() as i8;
        let mut s: i8 = 0;
        for i in -3i8..3 {
            if i < x {
                s = s.wrapping_add(i);
            }
        }
        q.write(Bits::from_u8(s as u8));
        clk.tick().await;
        for j in -2i8..=1 {
            if j < x {
                q.write(Bits::from_u8(j as u8));
            }
            clk.tick().await;
        }
    }
}

/// Untyped bounds and no use that fixes the type: rustc's `i32`, a negative intermediate
/// (`i - 2 < 0`) compared signed.
#[hardware(sequential)]
async fn lvt_default_i32(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut s: u8 = 0;
        for i in 0..4 {
            if i - 2 < 0 {
                s = s ^ ((i as u8) << 2);
            }
        }
        q.write(Bits::from_u8(s ^ a.read().as_u8()));
        clk.tick().await;
    }
}

/// A use that types the variable `u16` (a `wrapping_add` receiver).
#[hardware(sequential)]
async fn lvt_receiver_u16(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<16>, MainClk>) {
    loop {
        let mut s: u16 = a.read().as_u16() << 7;
        for i in 0..5 {
            s = s.wrapping_add(i);
        }
        q.write(Bits::from_u16(s));
        clk.tick().await;
    }
}

/// A negative `i64` range, tick-free in a combinational body: the SV `int` variable read as a
/// sign-extended 64-bit value.
#[hardware(combinational)]
fn lvt_i64_negative<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let mut s: i64 = a.read().as_u8() as i64;
    for i in -3i64..3 {
        s = s.wrapping_mul(i).wrapping_add(i);
    }
    o.write(Bits::from_u8(s as u8));
}

/// A body `let` that shadows the loop variable, tick-free and tick-bearing: the shadow's
/// initializer reads the variable, what follows reads the shadow (MEASUREMENTS §AB.5).
#[hardware(sequential)]
async fn lvt_shadow(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut s: u8 = a.read().as_u8();
        for i in 0u8..4 {
            let i = (i as u16) * 3;
            s = s.wrapping_add(i as u8);
        }
        q.write(Bits::from_u8(s));
        clk.tick().await;
        for j in 1u8..3 {
            let j = j ^ a.read().as_u8();
            q.write(Bits::from_u8(j));
            clk.tick().await;
        }
    }
}

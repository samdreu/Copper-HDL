// A signed LOCAL read where signedness is observable (line `II.3 found signed-local`,
// MEASUREMENTS §Z.9, §AA.2): `let v = x as i8; v < 3`. The local is a plain net in the SV; its
// reads carry the signedness (`$signed`, `>>>`), as the cast at the use does. Until 2026-11-09 the
// read was unsigned and the probe disagreed on 6/6 sequences, `sim=ref≠SV`. Compares, shifts,
// arithmetic, widening casts, a signed register and a combinational body, each swept on three legs.

/// The probe of §Z.9.
#[hardware(sequential)]
async fn sl_probe(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v = a.read().as_u8() as i8;
        q.write(Bits::from_u8(if v < 3 { 1 } else { 2 }));
        clk.tick().await;
    }
}

/// Compares between two signed locals, against a negative literal, and with the local on the
/// right; an annotated local.
#[hardware(sequential)]
async fn sl_compares(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, b: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let v = a.read().as_u8() as i8;
        let w: i8 = b.read().as_u8() as i8;
        let mut k: u8 = 0;
        if v < w {
            k |= 1;
        }
        if v >= -2 {
            k |= 2;
        }
        if 5 > w {
            k |= 4;
        }
        if w <= v {
            k |= 8;
        }
        if v > 0 && w < 0 {
            k |= 16;
        }
        q.write(Bits::from_u8(k));
        clk.tick().await;
    }
}

/// Shifts and arithmetic: `>>` of a signed local is arithmetic; a wrapping sum of two signed
/// locals is itself signed when compared; `/` and `%` by a positive constant round toward zero.
#[hardware(sequential)]
async fn sl_shift_arith(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    b: In<Bits<8>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
    p: RegOut<Bits<8>, MainClk>,
    r: RegOut<Bits<8>, MainClk>,
) {
    loop {
        let v = a.read().as_u8() as i8;
        let w = b.read().as_u8() as i8;
        let s = v.wrapping_add(w);
        let h = v >> 2;
        let n = w.wrapping_sub(v);
        q.write(Bits::from_u8(h as u8));
        p.write(Bits::from_u8(if s < 0 { 1 } else if n > 10 { 2 } else { 3 }));
        r.write(Bits::from_u8(((v / 3) as u8) ^ ((v % 5) as u8)));
        clk.tick().await;
    }
}

/// Widening casts of a signed local sign-extend (`as i16`, and `as u16`, which is Rust's
/// sign-extension too); the narrowed result is compared and output.
#[hardware(sequential)]
async fn sl_widen(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<16>, MainClk>, p: RegOut<Bits<8>, MainClk>) {
    loop {
        let v = a.read().as_u8() as i8;
        let x = v as i16;
        let y = v as u16;
        q.write(Bits::from_u16(y ^ (x as u16)));
        p.write(Bits::from_u8(if x < -100 { 1 } else if (v as i32) >> 1 < -3 { 2 } else { 3 }));
        clk.tick().await;
    }
}

/// A signed REGISTER: an accumulator compared against zero and shifted.
#[hardware(sequential)]
async fn sl_register(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut acc: i8 = 0;
    loop {
        acc = acc.wrapping_add(a.read().as_u8() as i8);
        q.write(Bits::from_u8(if acc < 0 { (acc >> 1) as u8 } else { 0x7f }));
        clk.tick().await;
    }
}

/// The combinational form.
#[hardware(combinational)]
fn sl_comb<D: ClockDomain>(a: In<Bits<8>, D>, b: In<Bits<8>, D>, q: Out<Bits<8>, D>) {
    let v = a.read().as_u8() as i8;
    let w = b.read().as_u8() as i8;
    let m = if v < w { v } else { w };
    q.write(Bits::from_u8((m >> 1) as u8));
}

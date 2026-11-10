// 128-bit integers on three legs (SPEC III.3.4 `II.3 found ref-no-i128`, MEASUREMENTS §AG): `u128`
// values at and above 2^127 and `i128` values at its bounds, in a clocked body's locals, operands and a
// loop counter. Until §AG the reference interpreter had no `i128` and its `u128` stopped at `i128::MAX`,
// so the first two modules (§U.7, §AB.1) lived in `tests/ref_probes/` and were checked simulator against
// Verilator only. The ports stay at most 64 bits wide (halves), which the testbench compares directly.

/// `saturating_add` / `saturating_sub` on `u128` (II.3.3 (e), §U.7): the unsigned clamp at 128 bits.
/// `tests/saturating_u128_equivalence.rs` also drives it on the operand pairs that reach each clamp.
#[hardware(sequential)]
async fn sat_u128(
    clk: Clock<MainClk>,
    a_hi: In<Bits<64>, MainClk>,
    a_lo: In<Bits<64>, MainClk>,
    b_hi: In<Bits<64>, MainClk>,
    b_lo: In<Bits<64>, MainClk>,
    s_hi: RegOut<Bits<64>, MainClk>,
    s_lo: RegOut<Bits<64>, MainClk>,
    t_hi: RegOut<Bits<64>, MainClk>,
    t_lo: RegOut<Bits<64>, MainClk>,
) {
    loop {
        let x: u128 = ((a_hi.read().as_u64() as u128) << 64) | (a_lo.read().as_u64() as u128);
        let y: u128 = ((b_hi.read().as_u64() as u128) << 64) | (b_lo.read().as_u64() as u128);
        let s: u128 = x.saturating_add(y);
        let t: u128 = x.saturating_sub(y);
        s_hi.write(Bits::from_u64((s >> 64) as u64));
        s_lo.write(Bits::from_u64(s as u64));
        t_hi.write(Bits::from_u64((t >> 64) as u64));
        t_lo.write(Bits::from_u64(t as u64));
        clk.tick().await;
    }
}

/// A tick-bearing `for` over a `u128` inclusive range whose run-time end reaches `u128::MAX` (D-28,
/// §AB.1): the last iteration runs and the loop stops (I.8.5's done flag in the SV). The sweep's
/// first-cycle sequences take every value of `a`, so the end is `u128::MAX` on one of them.
/// `tests/inclusive_max_u128.rs` also drives it.
#[hardware(sequential)]
async fn imx_u128_done(clk: Clock<MainClk>, a: In<Bits<2>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let n: u128 = u128::MAX - 3 + (a.read().as_u8() as u128);
        q.write(Bits::from_u8(0x33));
        clk.tick().await;
        for i in (u128::MAX - 1)..=n {
            q.write(Bits::from_u8(i as u8));
            clk.tick().await;
        }
    }
}

/// `i128` at its bounds: operands built from a sign-extended byte shifted to the top of the word, so the
/// saturating sum and difference clamp at `i128::MIN`/`MAX` on a quarter of the inputs each; a signed
/// order, an arithmetic `>>`, a wrapping sum, and casts to `u128` and back to a byte.
#[hardware(sequential)]
async fn wint_i128_arith(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    b: In<Bits<8>, MainClk>,
    s_hi: RegOut<Bits<64>, MainClk>,
    d_hi: RegOut<Bits<64>, MainClk>,
    w_hi: RegOut<Bits<64>, MainClk>,
    f: RegOut<Bits<8>, MainClk>,
) {
    loop {
        let x: i128 = ((a.read().as_u8() as i8) as i128) << 120;
        let y: i128 = ((b.read().as_u8() as i8) as i128) << 120;
        let s: i128 = x.saturating_add(y);
        let d: i128 = x.saturating_sub(y);
        let w: i128 = x.wrapping_add(y);
        let sh: i128 = d >> 121;
        let mut flags: u8 = (sh as u8) & 0x0f;
        if x < y {
            flags = flags | 0x10;
        }
        if s < 0 {
            flags = flags | 0x20;
        }
        if s == i128::MAX || s == i128::MIN {
            flags = flags | 0x40;
        }
        if d > w {
            flags = flags | 0x80;
        }
        s_hi.write(Bits::from_u64(((s as u128) >> 64) as u64));
        d_hi.write(Bits::from_u64(((d as u128) >> 64) as u64));
        w_hi.write(Bits::from_u64(((w as u128) >> 64) as u64));
        f.write(Bits::from_u8(flags));
        clk.tick().await;
    }
}

/// `u128` values above `i128::MAX` read as numbers: an unsigned order against 2^127 and `u128::MAX`'s
/// neighbours, a division, a logical `>>`, a wrapping product, a register carrying a `u128` across the tick.
#[hardware(sequential)]
async fn wint_u128_order(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    b: In<Bits<8>, MainClk>,
    h: RegOut<Bits<64>, MainClk>,
    p: RegOut<Bits<64>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut last: u128 = 0;
    loop {
        let x: u128 = ((a.read().as_u8() as u128) << 120) | (b.read().as_u8() as u128);
        let top: u128 = 1u128 << 127;
        let mut r: u8 = 0;
        if x >= top {
            r = r | 1;
        }
        if x / 3 > u128::MAX / 7 {
            r = r | 2;
        }
        if x > last {
            r = r | 4;
        }
        if u128::MAX - x < top {
            r = r | 8;
        }
        h.write(Bits::from_u64((x >> 70) as u64));
        p.write(Bits::from_u64((x.wrapping_mul(last) >> 64) as u64));
        q.write(Bits::from_u8(r));
        last = x;
        clk.tick().await;
    }
}

/// A tick-bearing `for` over a constant `i128` range starting at `i128::MIN` (the counter's width is chosen
/// from the constant bounds, `control_extract::expand_counted_for`) and a run-time `i128` end near the top.
#[hardware(sequential)]
async fn wint_i128_for(clk: Clock<MainClk>, a: In<Bits<2>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in i128::MIN..(i128::MIN + 3) {
            q.write(Bits::from_u8((i as u8) ^ 0x40));
            clk.tick().await;
        }
        let n: i128 = i128::MAX - 3 + (a.read().as_u8() as i128);
        for j in (i128::MAX - 2)..=n {
            q.write(Bits::from_u8(j as u8));
            clk.tick().await;
        }
    }
}

/// 128-bit ports, swept directly: the sweep's testbench carries a port of any width (the 64-bit
/// comparison of MEASUREMENTS §U.7 (ii) is `verify_with_verilator`'s, `EquivalenceTest::new`, not the
/// sweep's). `u128` values read from the ports above `i128::MAX`, an `i128` clamp written back.
#[hardware(sequential)]
async fn wint_port128(
    clk: Clock<MainClk>,
    a: In<Bits<128>, MainClk>,
    b: In<Bits<128>, MainClk>,
    q: RegOut<Bits<128>, MainClk>,
    s: RegOut<Bits<128>, MainClk>,
    g: RegOut<Bits<8>, MainClk>,
) {
    loop {
        let x: u128 = a.read().as_u128();
        let y: u128 = b.read().as_u128();
        q.write(Bits::from_u128(x.wrapping_add(y) ^ (x >> 3)));
        s.write(Bits::from_u128((x as i128).saturating_sub(y as i128) as u128));
        g.write(if x > y { Bits::from_u8(1) } else { Bits::from_u8(0) });
        clk.tick().await;
    }
}

/// `count_ones` of a negative narrow signed value (the interpreter counted the `i128`'s 128 bits until
/// §AG; Rust counts the type's 8). Compared by G-J (`tests/const_evaluators_agree.rs`), the transpiler
/// having no `count_ones` lowering.
const WINT_ONES: u32 = (-1i8).count_ones();
const WINT_ONES_NEG: u32 = (-128i16).count_ones() + (5u8).count_ones();

/// `!` on integers at every width class (the interpreter's `!` on an unsigned type below 128 bits
/// reported an overflow until §AG; Rust's is the complement at the type).
#[hardware(sequential)]
async fn wint_not(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>, r: RegOut<Bits<8>, MainClk>) {
    loop {
        let x: u8 = a.read().as_u8();
        let s: i8 = x as i8;
        let w: u128 = !(x as u128);
        let v: i128 = !(s as i128);
        q.write(Bits::from_u8(!x));
        r.write(Bits::from_u8((!s) as u8 ^ ((w >> 120) as u8) ^ (v as u8)));
        clk.tick().await;
    }
}

// `Bits` methods with their I.14.3(d) meanings (D-19), lowered by the transpiler.
//
// II.3 row (e), REFUSED-one per method (the transpiler; the macro admits) until Phase
// C step 5 (2026-11-06, `design_docs/review/MEASUREMENTS.md` §J.8): "method `…` is not
// supported in hardware expressions". Twelve of the row's methods now lower; the rest
// are listed in §J.8.

#[hardware(combinational)]
fn bits_methods_comb(
    a: In<Bits<8>, MainClk>,
    s: In<Bits<3>, MainClk>,
    t: In<Bits<4>, MainClk>,
    red: Out<Bits<6>, MainClk>,
    bit: Out<Logic, MainClk>,
    bit3: Out<Logic, MainClk>,
    shl: Out<Bits<8>, MainClk>,
    shr: Out<Bits<8>, MainClk>,
    zx: Out<Bits<12>, MainClk>,
    sx: Out<Bits<12>, MainClk>,
    tr: Out<Bits<4>, MainClk>,
) {
    let x = a.read();
    let mut r = [Logic::Zero; 6];
    r[0] = x.and_reduce();
    r[1] = x.or_reduce();
    r[2] = x.xor_reduce();
    r[3] = x.nand_reduce();
    r[4] = x.nor_reduce();
    r[5] = x.xnor_reduce();
    red.write(Bits::from_slice(&r));
    bit.write(x.get(s.read().as_usize()));
    bit3.write(x.get(3));
    shl.write(x.shift_left(s.read().as_usize()));
    // A 4-bit amount reaches n >= 8 (all zeros). Not `s.read().as_usize() + 2`: the
    // transpiler keeps `as_usize` at the receiver's 3 bits, so that sum wraps in the
    // SV where Rust's `usize` does not (MEASUREMENTS §J.8, a width finding).
    shr.write(x.shift_right(t.read().as_usize()));
    zx.write(x.zero_extend::<12>());
    sx.write(x.sign_extend::<12>());
    tr.write(x.truncate::<4>());
}

/// The reductions on a register, through the clocked path (both legs).
#[hardware(sequential)]
async fn bits_methods_seq(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    q: RegOut<Bits<4>, MainClk>,
) {
    let mut acc: Bits<8> = Bits::zero();
    loop {
        acc = acc ^ a.read();
        let mut r = [Logic::Zero; 4];
        r[0] = acc.or_reduce();
        r[1] = acc.xnor_reduce();
        r[2] = acc.get(7);
        r[3] = acc.shift_right(4).nand_reduce();
        q.write(Bits::from_slice(&r));
        clk.tick().await;
    }
}

/// The rest of the row's methods (MEASUREMENTS §J.14), combinational.
#[hardware(combinational)]
fn bits_methods_rest_comb(
    a: In<Bits<8>, MainClk>,
    b: In<Bits<4>, MainClk>,
    s: In<Logic, MainClk>,
    lo: In<Bits<2>, MainClk>,
    cat: Out<Bits<12>, MainClk>,
    ps: Out<Bits<4>, MainClk>,
    psd: Out<Bits<4>, MainClk>,
    rep: Out<Bits<16>, MainClk>,
    mx: Out<Bits<8>, MainClk>,
    lsb: Out<Bits<8>, MainClk>,
    msb: Out<Bits<8>, MainClk>,
    cmp: Out<Bits<7>, MainClk>,
    ones: Out<Bits<8>, MainClk>,
    ones_ctx: Out<Bits<8>, MainClk>,
) {
    let x = a.read();
    let y = b.read();
    let c: Bits<12> = x.concat(&y);
    cat.write(c);
    ps.write(x.part_select::<4>(2));
    psd.write(x.part_select::<4>(lo.read().as_usize()));
    rep.write(y.replicate::<16>());
    mx.write(Bits::mux(s.read(), &x, &Bits::from_u8(0x5A)));
    lsb.write(x.with_lsb(s.read()));
    msb.write(x.with_msb(s.read()));
    let z = Bits::<8>::from_u8(0x40);
    let mut r = [Logic::Zero; 7];
    r[0] = x.eq_logic(&z);
    r[1] = x.ne_logic(&z);
    r[2] = x.lt_logic(&z);
    r[3] = x.le_logic(&z);
    r[4] = x.gt_logic(&z);
    r[5] = x.ge_logic(&z);
    // `is_valid()` is OUTSIDE since Phase C step 8 (I.14.3(d), refused in both
    // front-ends); `Logic::from_bool` (lowered since step 8) takes its bit.
    r[6] = Logic::from_bool(x.and_reduce() == Logic::One);
    cmp.write(Bits::from_slice(&r));
    ones.write(Bits::<8>::all_ones() ^ Bits::from_slice(x.as_array()));
    // Unsized: the width is the context's (the interpreter refuses this spelling,
    // an I.14.4 parity item; this module has no interpreter leg by class).
    let o: Bits<8> = Bits::all_ones();
    ones_ctx.write(o ^ x);
}

/// `set`, an `all_ones` power-on value, a register through `concat` / `part_select`.
#[hardware(sequential)]
async fn bits_methods_rest_seq(
    clk: Clock<MainClk>,
    a: In<Bits<4>, MainClk>,
    i: In<Bits<3>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut acc: Bits<8> = Bits::<8>::all_ones();
    loop {
        let mut t = acc.part_select::<4>(4).concat(&(a.read() ^ acc.part_select::<4>(0)));
        t.set(i.read().as_usize(), Logic::One);
        acc = t;
        q.write(acc);
        clk.tick().await;
    }
}

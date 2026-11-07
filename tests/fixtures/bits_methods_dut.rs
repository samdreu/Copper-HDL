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

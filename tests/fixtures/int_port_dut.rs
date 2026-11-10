// Integer-typed ports in the sweep (SPEC III.3.4 `II.3 found int-port-unswept`, MEASUREMENTS §AI):
// `In<u8>`, `In<i16>`, `RegOut<i32>`, `Out<u128>`, … driven with seeded random values at their own
// type and compared on all three legs. Until §AI `build.rs::is_stimulable` admitted `Logic` and
// `Bits<N>` only, so a module with such a port was `#[ignore]`d and the class had no witness. The
// transpiler emits an integer port as `logic [N-1:0]` (its two's-complement bits); signed arithmetic
// and comparisons live in the body's casts. Every operation here is total at its type (wrapping or
// widening), so no sequence ends in a run-time failure.

/// Every width class on a clocked module: unsigned and signed inputs from 8 to 128 bits, mixed
/// into outputs of other integer types — sign extension (`i16 as i32`), a signed comparison, a
/// wrapping product, truncation, and a `u128` above `i128::MAX` passed through and halved.
#[hardware(sequential)]
async fn ip_seq_widths(
    clk: Clock<MainClk>,
    a: In<u8, MainClk>,
    b: In<i16, MainClk>,
    c: In<u32, MainClk>,
    d: In<i64, MainClk>,
    e: In<u128, MainClk>,
    f: In<i128, MainClk>,
    q8: RegOut<i8, MainClk>,
    q32: RegOut<i32, MainClk>,
    q64: RegOut<u64, MainClk>,
    q128: RegOut<u128, MainClk>,
    s128: RegOut<i128, MainClk>,
    qsz: RegOut<usize, MainClk>,
) {
    loop {
        let b32: i32 = b.read() as i32;
        q8.write((a.read() as i8).wrapping_add(b.read() as i8));
        q32.write(if b32 < 0 { b32.wrapping_mul(3) } else { (c.read() as i32).wrapping_sub(b32) });
        q64.write((d.read() as u64) ^ ((c.read() as u64) << 32));
        q128.write(e.read() >> 1);
        s128.write(f.read().wrapping_add(d.read() as i128));
        qsz.write((a.read() as usize).wrapping_add(c.read() as usize));
        clk.tick().await;
    }
}

/// A register at an integer port's type: an `i32` running sum of a signed input, written to a
/// signed output, and a `u16` that counts the cycles the input was negative.
#[hardware(sequential)]
async fn ip_seq_register(clk: Clock<MainClk>, x: In<i8, MainClk>, sum: RegOut<i32, MainClk>, neg: RegOut<u16, MainClk>) {
    let mut acc: i32 = 0;
    let mut n: u16 = 0;
    loop {
        acc = acc.wrapping_add(x.read() as i32);
        if x.read() < 0 {
            n = n.wrapping_add(1);
        }
        sum.write(acc);
        neg.write(n);
        clk.tick().await;
    }
}

/// The same classes in a combinational body: a signed minimum, an unsigned maximum by comparison,
/// `isize`, and a 128-bit signed output — `Out` ports of integer type.
#[hardware(combinational)]
fn ip_comb_widths(
    a: In<i8, MainClk>,
    b: In<i8, MainClk>,
    c: In<u16, MainClk>,
    d: In<isize, MainClk>,
    lo: Out<i8, MainClk>,
    hi: Out<u16, MainClk>,
    w: Out<i128, MainClk>,
    z: Out<isize, MainClk>,
) {
    let x: i8 = a.read();
    let y: i8 = b.read();
    lo.write(if x < y { x } else { y });
    hi.write(if c.read() > 0x8000 { c.read() } else { 0x8000 });
    w.write((d.read() as i128) << 64 | (c.read() as i128));
    z.write(d.read().wrapping_sub(x as isize));
}

/// Integer ports next to `Bits` and `Logic` ports on one module: the integer's bits reach a `Bits`
/// output and back, and a `Logic` selects between them.
#[hardware(sequential)]
async fn ip_mixed_kinds(
    clk: Clock<MainClk>,
    sel: In<Logic, MainClk>,
    v: In<i32, MainClk>,
    raw: In<Bits<32>, MainClk>,
    as_bits: RegOut<Bits<32>, MainClk>,
    as_int: RegOut<i32, MainClk>,
) {
    loop {
        let from_bits: i32 = raw.read().as_u32() as i32;
        as_bits.write(Bits::from_u32(v.read() as u32));
        as_int.write(if sel.read() == Logic::One { from_bits } else { 0i32.wrapping_sub(v.read()) });
        clk.tick().await;
    }
}

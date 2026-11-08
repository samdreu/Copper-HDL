// `saturating_add` / `saturating_sub` on a signed integer (SPEC II.3.3 (e), MEASUREMENTS §M.9
// 4(b), §U.7). Until 2026-11-08 the transpiler refused it; the signed clamp is now the overflow
// test on the sign bits. The unsigned clamp wider than 64 bits (`u128`) is lowered too, but its
// module is not swept — the reference interpreter's `u128` stops at `i128::MAX` (§U.7) — so it is
// checked sim vs Verilator by `tests/saturating_u128_equivalence.rs`.

/// `i8` and `i16` operands (casts of the input views), clamped at MIN and MAX.
#[hardware(combinational)]
fn sat_signed<D: ClockDomain>(
    a: In<Bits<8>, D>,
    b: In<Bits<8>, D>,
    c: In<Bits<16>, D>,
    d: In<Bits<16>, D>,
    add8: Out<Bits<8>, D>,
    sub8: Out<Bits<8>, D>,
    add16: Out<Bits<16>, D>,
    sub16: Out<Bits<16>, D>,
) {
    let x = a.read().as_u8() as i8;
    let y = b.read().as_u8() as i8;
    add8.write(Bits::from_u8(x.saturating_add(y) as u8));
    sub8.write(Bits::from_u8(x.saturating_sub(y) as u8));
    let u = c.read().as_u16() as i16;
    let v = d.read().as_u16() as i16;
    add16.write(Bits::from_u16(u.saturating_add(v) as u16));
    sub16.write(Bits::from_u16(u.saturating_sub(v) as u16));
}

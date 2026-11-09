// A narrowing cast to or from a signed type truncates (SPEC I.14.3(c); line `II.3 found
// signed-narrowing-cast`, MEASUREMENTS §AB.3). Until 2026-11-09 the transpiler kept the operand's
// width — `$signed(x)` for `x as i8` of a 32-bit `x` — the right bits in an assignment of the
// target's width, the wrong value in a wider context: `(x as i8) as i32` did not truncate, and a
// comparison of `x as i8` was made at 32 bits. Each module is swept on three legs.

/// `(x as i8) as i32`, `(x as i16) < 0`, `(s as u8) as u16` of a signed `s`.
#[hardware(sequential)]
async fn snc_probe(clk: Clock<MainClk>, a: In<Bits<16>, MainClk>, q: RegOut<Bits<32>, MainClk>, f: RegOut<Logic, MainClk>) {
    loop {
        let x = (a.read().as_u32() as i32) << 8;
        let y = (x as i8) as i32;
        let z = ((x >> 4) as i16) as i32;
        let s = a.read().as_u16() as i16;
        let u = (s as u8) as u16;
        q.write(Bits::from_u32((y.wrapping_add(z) as u32) ^ (u as u32)));
        f.write(Logic::from_bool((x as i16) < 0 || ((s >> 3) as i8) < -5));
        clk.tick().await;
    }
}

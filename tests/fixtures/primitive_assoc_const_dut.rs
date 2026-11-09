// A primitive integer type's associated constant — `u8::MAX`, `i8::MIN`, `u16::BITS` — is I.14.4's
// `T::K` (line `II.3 found primitive-assoc-const`, MEASUREMENTS §AB.4). rustc (the macro) and the
// interpreter read it; until 2026-11-09 the transpiler refused it ("cannot lower literal: u8 :: MAX",
// and R3's fold could not read a range ending at it). Each module is swept on three legs.

const PAC_TOP: u8 = u8::MAX - 1;

/// As a value, in a constant, and as the end of a ticking inclusive range (D-28).
#[hardware(sequential)]
async fn pac_values(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        q.write(Bits::from_u8(a.read().as_u8() ^ u8::MAX));
        clk.tick().await;
        q.write(Bits::from_u8(PAC_TOP.wrapping_add(a.read().as_u8())));
        clk.tick().await;
        for i in 253u8..=u8::MAX {
            q.write(Bits::from_u8(i));
            clk.tick().await;
        }
    }
}

/// Signed minima and `BITS`, in a combinational body.
#[hardware(combinational)]
fn pac_signed<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    let x = a.read().as_u8() as i8;
    let lo = if x == i8::MIN { 1u8 } else { 0u8 };
    let hi = if x == i8::MAX { 2u8 } else { 0u8 };
    o.write(Bits::from_u8(lo | hi | ((u16::BITS as u8) << 2)));
}

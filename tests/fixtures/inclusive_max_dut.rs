// A `for v in a..=b` whose end is its type's maximum (D-28, 2026-11-09; line `II.3 found
// inclusive-for-runtime-max`, MEASUREMENTS §AA.5, §AB.1): Rust's meaning everywhere — the last
// iteration runs and the loop stops; nothing overflows. I.8.5's form tests a done flag rather than
// incrementing past the end. The run-time ends below reach the maximum under the sweep's stimulus
// (a narrow input OR-ed into the top of the range), at `u8`, `u16`, `u32`, `u64` and `i8`; a
// constant end at the maximum, once refused by II.1.2 R23, and an untyped one the body types `u8`
// are swept too, as are a wide exclusive range and tick-free ends at the maximum.

/// The §AA.5 probe: a `u8` run-time end that reaches 255 (`a | 0xfc`).
#[hardware(sequential)]
async fn imx_u8_runtime(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let n = a.read().as_u8() | 0xfc;
        q.write(Bits::from_u8(0x11));
        clk.tick().await;
        for i in 0xfeu8..=n {
            q.write(Bits::from_u8(i));
            clk.tick().await;
        }
    }
}

/// A constant, typed end at the maximum, `255u8` (refused by II.1.2 R23 until D-28) and an untyped one the
/// body types `u8` (`from_u8(i)`).
#[hardware(sequential)]
async fn imx_u8_const(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        for i in 253u8..=255u8 {
            q.write(Bits::from_u8(i ^ a.read().as_u8()));
            clk.tick().await;
        }
        for i in 254..=255 {
            q.write(Bits::from_u8(i));
            clk.tick().await;
        }
    }
}

/// A `u16` run-time end reaching 65535, and an author's `break`.
#[hardware(sequential)]
async fn imx_u16_break(clk: Clock<MainClk>, a: In<Bits<2>, MainClk>, b: In<Logic, MainClk>, q: RegOut<Bits<16>, MainClk>) {
    loop {
        let n = 0xfffcu16 | a.read().as_u16();
        q.write(Bits::from_u16(0x1234));
        clk.tick().await;
        for i in 0xfffdu16..=n {
            if b.read().as_bool() && i == 0xfffe {
                break;
            }
            q.write(Bits::from_u16(i));
            clk.tick().await;
        }
    }
}

/// A `u32` run-time end reaching 0xffff_ffff: the transpiler's counter was a `u32` that wrapped.
#[hardware(sequential)]
async fn imx_u32_runtime(clk: Clock<MainClk>, a: In<Bits<2>, MainClk>, q: RegOut<Bits<32>, MainClk>) {
    loop {
        let n = 0xffff_fffcu32 | a.read().as_u32();
        q.write(Bits::from_u32(7));
        clk.tick().await;
        for i in 0xffff_fffeu32..=n {
            q.write(Bits::from_u32(i));
            clk.tick().await;
        }
    }
}

/// A `u64` run-time end reaching its maximum, and a wide exclusive range a 32-bit counter truncated.
#[hardware(sequential)]
async fn imx_u64_runtime(clk: Clock<MainClk>, a: In<Bits<2>, MainClk>, q: RegOut<Bits<64>, MainClk>) {
    loop {
        let n = 0xffff_ffff_ffff_fffcu64 | a.read().as_u64();
        q.write(Bits::from_u64(9));
        clk.tick().await;
        for i in 0xffff_ffff_ffff_fffeu64..=n {
            q.write(Bits::from_u64(i));
            clk.tick().await;
        }
        for j in 0x1_0000_0000u64..(0x1_0000_0000u64 | a.read().as_u64()) {
            q.write(Bits::from_u64(j));
            clk.tick().await;
        }
    }
}

/// An `i8` run-time end reaching 127.
#[hardware(sequential)]
async fn imx_i8_runtime(clk: Clock<MainClk>, a: In<Bits<2>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let n = 124i8 + (a.read().as_u8() as i8);
        q.write(Bits::from_u8(0x22));
        clk.tick().await;
        for i in 126i8..=n {
            q.write(Bits::from_u8(i as u8));
            clk.tick().await;
        }
    }
}

/// Tick-free ends at the maximum of a 32- and a 64-bit type: an SV `int` loop variable cannot
/// carry them (the maxima are spelled as literals — `u32::MAX` is not read by the transpiler's
/// constant evaluator, MEASUREMENTS §AB.4).
#[hardware(sequential)]
async fn imx_tickfree_wide(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<64>, MainClk>) {
    loop {
        let mut s: u32 = 0;
        for i in 0xffff_fffdu32..=0xffff_ffffu32 {
            s = s ^ i;
        }
        let mut t: u64 = 0;
        for j in 0xffff_ffff_ffff_fffeu64..=0xffff_ffff_ffff_ffffu64 {
            t = t.wrapping_add(j);
        }
        q.write(Bits::from_u64(t ^ (s as u64) ^ a.read().as_u64()));
        clk.tick().await;
    }
}


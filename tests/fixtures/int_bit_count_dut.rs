// The integer bit-count methods on run-time values (SPEC III.3.4 `II.3 found int-bit-count`,
// MEASUREMENTS §AI): `count_ones`, `count_zeros`, `leading_zeros`, `trailing_zeros`,
// `leading_ones`, `trailing_ones` and `is_power_of_two`, with Rust's meaning (I.14.3(c)) — a `u32`
// counted over the receiver type's own bits, a signed receiver on its two's-complement bits, and
// `leading_zeros`/`trailing_zeros` of 0 equal to the width. Until §AI the macro admitted them and the
// transpiler refused them: a parity break of II.1.3.
//
// Every module builds its operands the same way: `sel` picks 0, all ones, a byte shifted to a
// run-time position, or that value's complement, so 0 and all ones each occur on about a quarter of
// the cycles of every random sequence; each width takes a window of the 128-bit value at an offset
// that follows the shift, so the byte lands anywhere inside the narrower types too. Every count is at
// most 128, so one byte of an output holds it.

/// The 128-bit operand of every module here and its window at each width (see the header).
fn ibc_operand(a: u8, sh: u8, sel: u8) -> u128 {
    let p: u128 = (a as u128) << (sh as u32);
    if sel == 0 {
        0
    } else if sel == 1 {
        u128::MAX
    } else if sel == 2 {
        p
    } else {
        !p
    }
}

/// The six counts of a `u8`/`u16`/… value packed one per byte, `is_power_of_two` in byte 6.
#[hardware(sequential)]
async fn ibc_seq_unsigned(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    sh: In<Bits<7>, MainClk>,
    sel: In<Bits<2>, MainClk>,
    c8: RegOut<Bits<64>, MainClk>,
    c16: RegOut<Bits<64>, MainClk>,
    c32: RegOut<Bits<64>, MainClk>,
    c64: RegOut<Bits<64>, MainClk>,
    c128: RegOut<Bits<64>, MainClk>,
) {
    loop {
        let k: u8 = sh.read().as_u8();
        let x: u128 = ibc_operand(a.read().as_u8(), k, sel.read().as_u8());
        let x8: u8 = (x >> ((k & 0x78) as u32)) as u8;
        let x16: u16 = (x >> ((k & 0x70) as u32)) as u16;
        let x32: u32 = (x >> ((k & 0x60) as u32)) as u32;
        let x64: u64 = (x >> ((k & 0x40) as u32)) as u64;
        c8.write(Bits::from_u64(
            (x8.count_ones() as u64)
                | ((x8.count_zeros() as u64) << 8)
                | ((x8.leading_zeros() as u64) << 16)
                | ((x8.trailing_zeros() as u64) << 24)
                | ((x8.leading_ones() as u64) << 32)
                | ((x8.trailing_ones() as u64) << 40)
                | ((x8.is_power_of_two() as u64) << 48),
        ));
        c16.write(Bits::from_u64(
            (x16.count_ones() as u64)
                | ((x16.count_zeros() as u64) << 8)
                | ((x16.leading_zeros() as u64) << 16)
                | ((x16.trailing_zeros() as u64) << 24)
                | ((x16.leading_ones() as u64) << 32)
                | ((x16.trailing_ones() as u64) << 40)
                | ((x16.is_power_of_two() as u64) << 48),
        ));
        c32.write(Bits::from_u64(
            (x32.count_ones() as u64)
                | ((x32.count_zeros() as u64) << 8)
                | ((x32.leading_zeros() as u64) << 16)
                | ((x32.trailing_zeros() as u64) << 24)
                | ((x32.leading_ones() as u64) << 32)
                | ((x32.trailing_ones() as u64) << 40)
                | ((x32.is_power_of_two() as u64) << 48),
        ));
        c64.write(Bits::from_u64(
            (x64.count_ones() as u64)
                | ((x64.count_zeros() as u64) << 8)
                | ((x64.leading_zeros() as u64) << 16)
                | ((x64.trailing_zeros() as u64) << 24)
                | ((x64.leading_ones() as u64) << 32)
                | ((x64.trailing_ones() as u64) << 40)
                | ((x64.is_power_of_two() as u64) << 48),
        ));
        c128.write(Bits::from_u64(
            (x.count_ones() as u64)
                | ((x.count_zeros() as u64) << 8)
                | ((x.leading_zeros() as u64) << 16)
                | ((x.trailing_zeros() as u64) << 24)
                | ((x.leading_ones() as u64) << 32)
                | ((x.trailing_ones() as u64) << 40)
                | ((x.is_power_of_two() as u64) << 48),
        ));
        clk.tick().await;
    }
}

/// The same six counts on `i8` … `i128`: a negative value counts its two's-complement bits (`-1i8`
/// has 8 ones, `i16::MIN` 15 trailing zeros), never the sign extension.
#[hardware(sequential)]
async fn ibc_seq_signed(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    sh: In<Bits<7>, MainClk>,
    sel: In<Bits<2>, MainClk>,
    c8: RegOut<Bits<64>, MainClk>,
    c16: RegOut<Bits<64>, MainClk>,
    c32: RegOut<Bits<64>, MainClk>,
    c64: RegOut<Bits<64>, MainClk>,
    c128: RegOut<Bits<64>, MainClk>,
) {
    loop {
        let k: u8 = sh.read().as_u8();
        let x: u128 = ibc_operand(a.read().as_u8(), k, sel.read().as_u8());
        let s8: i8 = (x >> ((k & 0x78) as u32)) as u8 as i8;
        let s16: i16 = (x >> ((k & 0x70) as u32)) as u16 as i16;
        let s32: i32 = (x >> ((k & 0x60) as u32)) as u32 as i32;
        let s64: i64 = (x >> ((k & 0x40) as u32)) as u64 as i64;
        let s128: i128 = x as i128;
        c8.write(Bits::from_u64(
            (s8.count_ones() as u64)
                | ((s8.count_zeros() as u64) << 8)
                | ((s8.leading_zeros() as u64) << 16)
                | ((s8.trailing_zeros() as u64) << 24)
                | ((s8.leading_ones() as u64) << 32)
                | ((s8.trailing_ones() as u64) << 40),
        ));
        c16.write(Bits::from_u64(
            (s16.count_ones() as u64)
                | ((s16.count_zeros() as u64) << 8)
                | ((s16.leading_zeros() as u64) << 16)
                | ((s16.trailing_zeros() as u64) << 24)
                | ((s16.leading_ones() as u64) << 32)
                | ((s16.trailing_ones() as u64) << 40),
        ));
        c32.write(Bits::from_u64(
            (s32.count_ones() as u64)
                | ((s32.count_zeros() as u64) << 8)
                | ((s32.leading_zeros() as u64) << 16)
                | ((s32.trailing_zeros() as u64) << 24)
                | ((s32.leading_ones() as u64) << 32)
                | ((s32.trailing_ones() as u64) << 40),
        ));
        c64.write(Bits::from_u64(
            (s64.count_ones() as u64)
                | ((s64.count_zeros() as u64) << 8)
                | ((s64.leading_zeros() as u64) << 16)
                | ((s64.trailing_zeros() as u64) << 24)
                | ((s64.leading_ones() as u64) << 32)
                | ((s64.trailing_ones() as u64) << 40),
        ));
        c128.write(Bits::from_u64(
            (s128.count_ones() as u64)
                | ((s128.count_zeros() as u64) << 8)
                | ((s128.leading_zeros() as u64) << 16)
                | ((s128.trailing_zeros() as u64) << 24)
                | ((s128.leading_ones() as u64) << 32)
                | ((s128.trailing_ones() as u64) << 40),
        ));
        clk.tick().await;
    }
}

/// The unsigned counts in a combinational body (`always_comb`), at 8, 32 and 128 bits.
#[hardware(combinational)]
fn ibc_comb_unsigned(
    a: In<Bits<8>, MainClk>,
    sh: In<Bits<7>, MainClk>,
    sel: In<Bits<2>, MainClk>,
    c8: Out<Bits<64>, MainClk>,
    c32: Out<Bits<64>, MainClk>,
    c128: Out<Bits<64>, MainClk>,
) {
    let k: u8 = sh.read().as_u8();
    let x: u128 = ibc_operand(a.read().as_u8(), k, sel.read().as_u8());
    let x8: u8 = (x >> ((k & 0x78) as u32)) as u8;
    let x32: u32 = (x >> ((k & 0x60) as u32)) as u32;
    c8.write(Bits::from_u64(
        (x8.count_ones() as u64)
            | ((x8.count_zeros() as u64) << 8)
            | ((x8.leading_zeros() as u64) << 16)
            | ((x8.trailing_zeros() as u64) << 24)
            | ((x8.leading_ones() as u64) << 32)
            | ((x8.trailing_ones() as u64) << 40)
            | ((x8.is_power_of_two() as u64) << 48),
    ));
    c32.write(Bits::from_u64(
        (x32.count_ones() as u64)
            | ((x32.count_zeros() as u64) << 8)
            | ((x32.leading_zeros() as u64) << 16)
            | ((x32.trailing_zeros() as u64) << 24)
            | ((x32.leading_ones() as u64) << 32)
            | ((x32.trailing_ones() as u64) << 40)
            | ((x32.is_power_of_two() as u64) << 48),
    ));
    c128.write(Bits::from_u64(
        (x.count_ones() as u64)
            | ((x.count_zeros() as u64) << 8)
            | ((x.leading_zeros() as u64) << 16)
            | ((x.trailing_zeros() as u64) << 24)
            | ((x.leading_ones() as u64) << 32)
            | ((x.trailing_ones() as u64) << 40)
            | ((x.is_power_of_two() as u64) << 48),
    ));
}

/// The signed counts in a combinational body, at 16, 64 and 128 bits.
#[hardware(combinational)]
fn ibc_comb_signed(
    a: In<Bits<8>, MainClk>,
    sh: In<Bits<7>, MainClk>,
    sel: In<Bits<2>, MainClk>,
    c16: Out<Bits<64>, MainClk>,
    c64: Out<Bits<64>, MainClk>,
    c128: Out<Bits<64>, MainClk>,
) {
    let k: u8 = sh.read().as_u8();
    let x: u128 = ibc_operand(a.read().as_u8(), k, sel.read().as_u8());
    let s16: i16 = (x >> ((k & 0x70) as u32)) as u16 as i16;
    let s64: i64 = (x >> ((k & 0x40) as u32)) as u64 as i64;
    let s128: i128 = x as i128;
    c16.write(Bits::from_u64(
        (s16.count_ones() as u64)
            | ((s16.count_zeros() as u64) << 8)
            | ((s16.leading_zeros() as u64) << 16)
            | ((s16.trailing_zeros() as u64) << 24)
            | ((s16.leading_ones() as u64) << 32)
            | ((s16.trailing_ones() as u64) << 40),
    ));
    c64.write(Bits::from_u64(
        (s64.count_ones() as u64)
            | ((s64.count_zeros() as u64) << 8)
            | ((s64.leading_zeros() as u64) << 16)
            | ((s64.trailing_zeros() as u64) << 24)
            | ((s64.leading_ones() as u64) << 32)
            | ((s64.trailing_ones() as u64) << 40),
    ));
    c128.write(Bits::from_u64(
        (s128.count_ones() as u64)
            | ((s128.count_zeros() as u64) << 8)
            | ((s128.leading_zeros() as u64) << 16)
            | ((s128.trailing_zeros() as u64) << 24)
            | ((s128.leading_ones() as u64) << 32)
            | ((s128.trailing_ones() as u64) << 40),
    ));
}

/// A count is a `u32`: an unannotated `let` of one, shifted so a wider value would keep bits a
/// `u32` drops (`128 << 26` is 0 as a `u32`), and a `u32` register accumulating counts across ticks.
#[hardware(sequential)]
async fn ibc_u32_result(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    sh: In<Bits<7>, MainClk>,
    sel: In<Bits<2>, MainClk>,
    z: RegOut<Bits<64>, MainClk>,
    acc_o: RegOut<Bits<32>, MainClk>,
) {
    let mut acc: u32 = 0;
    loop {
        let k: u8 = sh.read().as_u8();
        let x: u128 = ibc_operand(a.read().as_u8(), k, sel.read().as_u8());
        let lz = x.leading_zeros();
        let tz = (x as i8).trailing_zeros();
        let wide = ((lz << 26) as u64) | (((tz << 29) as u64) << 32);
        z.write(Bits::from_u64(wide));
        acc = acc.wrapping_add(x.count_ones()).wrapping_add((x as u16).count_zeros());
        acc_o.write(Bits::from_u32(acc));
        clk.tick().await;
    }
}

/// The same methods called through their type's path (`u16::count_ones(v)`), which rustc reads as
/// the method call (line `II.3 found int-method-path-call`), next to two arithmetic methods the
/// `.`-form already lowered; an unannotated `let` of a path-form count is a `u32` too.
#[hardware(sequential)]
async fn ibc_path_call(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    sh: In<Bits<7>, MainClk>,
    sel: In<Bits<2>, MainClk>,
    c: RegOut<Bits<64>, MainClk>,
    w: RegOut<Bits<16>, MainClk>,
) {
    loop {
        let k: u8 = sh.read().as_u8();
        let x: u128 = ibc_operand(a.read().as_u8(), k, sel.read().as_u8());
        let x16: u16 = (x >> ((k & 0x70) as u32)) as u16;
        let s32: i32 = (x >> ((k & 0x60) as u32)) as u32 as i32;
        let n = u16::leading_zeros(x16);
        c.write(Bits::from_u64(
            (u16::count_ones(x16) as u64)
                | ((u16::count_zeros(x16) as u64) << 8)
                | (((n << 27) as u64) << 16)
                | ((i32::trailing_zeros(s32) as u64) << 48)
                | ((u128::leading_ones(x) as u64) << 56)
                | ((u16::is_power_of_two(x16) as u64) << 63)
                | ((i32::trailing_ones(s32) as u64) << 8 << 32),
        ));
        w.write(Bits::from_u16(u16::wrapping_add(x16, k as u16) ^ u16::saturating_sub(x16, 0x1234)));
        clk.tick().await;
    }
}

// Constant evaluations of the same methods, for G-J (`tests/const_evaluators_agree.rs`): the two
// constant evaluators against each other, at a negative narrow value and at 0.
const IBC_LZ_NEG: u32 = (-1i8).leading_zeros() + (-128i16).trailing_zeros();
const IBC_ZERO: u32 = 0u64.leading_zeros() + 0u128.trailing_zeros() + 0u8.count_zeros();
const IBC_ONES: u32 = (-1i16).leading_ones() + 0x0fu8.trailing_ones() + 0xf0u8.leading_ones();

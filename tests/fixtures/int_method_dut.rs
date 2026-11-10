// The integer methods lowered in MEASUREMENTS §AJ (D-32), on run-time values with Rust's meaning
// (I.14.3(c)): the bit permutations `rotate_left`, `rotate_right`, `swap_bytes`, `reverse_bits`;
// the compare-selects `min`, `max`, `abs_diff`; the sign family `signum`, `is_positive`,
// `is_negative`, `wrapping_abs`, `unsigned_abs`; and `wrapping_neg`, `wrapping_shl`,
// `wrapping_shr`. Until §AJ the macro admitted them and the transpiler refused them (32 OPEN lines
// of III.3.4); D-32 made every integer method the transpiler does not lower a named exception, and
// these leave it.
//
// Every operand is picked the same way: `sa`/`sb` choose 0, all ones (-1 signed), the top bit
// alone (the signed minimum), or the random port value, so the edges occur on about a quarter of
// the cycles each; `b` can also equal `a`. Shift and rotate amounts are a random `u32` (mostly far
// past the width, which Rust takes modulo the width) and its low 8 bits.

/// 0, all ones, the top bit of a `w`-bit value, or `a`, by `sel`.
fn im_pick(a: u128, sel: u8, w: u32) -> u128 {
    if sel == 0 {
        0
    } else if sel == 1 {
        u128::MAX
    } else if sel == 2 {
        1u128 << (w - 1)
    } else {
        a
    }
}

/// `b`'s pick: 0, all ones, the top bit, `b` (`sel_b` 0..3), or `a`'s own pick (4..7).
fn im_pick_b(a: u128, b: u128, sel_a: u8, sel_b: u8, w: u32) -> u128 {
    if sel_b >= 4 { im_pick(a, sel_a, w) } else { im_pick(b, sel_b, w) }
}

/// The bit permutations at every width, unsigned and signed, by a run-time and a constant amount.
#[hardware(sequential)]
async fn im_perm(
    clk: Clock<MainClk>,
    a: In<u128, MainClk>,
    sa: In<Bits<2>, MainClk>,
    s: In<u32, MainClk>,
    r8: RegOut<u64, MainClk>,
    r16: RegOut<u64, MainClk>,
    r32: RegOut<u128, MainClk>,
    r64: RegOut<u128, MainClk>,
    r64b: RegOut<u128, MainClk>,
    r128: RegOut<u128, MainClk>,
    r128b: RegOut<u128, MainClk>,
    r128c: RegOut<u128, MainClk>,
    si: RegOut<u128, MainClk>,
) {
    loop {
        let sel: u8 = sa.read().as_u8();
        let k: u32 = s.read();
        let k8: u32 = k & 0xff;
        let x8: u8 = im_pick(a.read(), sel, 8) as u8;
        let x16: u16 = im_pick(a.read(), sel, 16) as u16;
        let x32: u32 = im_pick(a.read(), sel, 32) as u32;
        let x64: u64 = im_pick(a.read(), sel, 64) as u64;
        let x128: u128 = im_pick(a.read(), sel, 128);
        let y8: i8 = x8 as i8;
        let y16: i16 = x16 as i16;
        let y32: i32 = x32 as i32;
        let y64: i64 = x64 as i64;
        let y128: i128 = x128 as i128;
        r8.write(
            (x8.rotate_left(k) as u64)
                | ((x8.rotate_right(k8) as u64) << 8)
                | ((x8.swap_bytes() as u64) << 16)
                | ((x8.reverse_bits() as u64) << 24)
                | ((y8.rotate_left(3) as u8 as u64) << 32)
                | ((y8.rotate_right(k) as u8 as u64) << 40)
                | ((y8.reverse_bits() as u8 as u64) << 48)
                | ((x8.rotate_left(9) as u64) << 56),
        );
        r16.write(
            (x16.rotate_left(k) as u64)
                | ((x16.rotate_right(5) as u64) << 16)
                | ((x16.swap_bytes() as u64) << 32)
                | ((x16.reverse_bits() as u64) << 48),
        );
        r32.write(
            (x32.rotate_left(k8) as u128)
                | ((x32.rotate_right(k) as u128) << 32)
                | ((x32.swap_bytes() as u128) << 64)
                | ((x32.reverse_bits() as u128) << 96),
        );
        r64.write((x64.rotate_left(k) as u128) | ((x64.rotate_right(63) as u128) << 64));
        r64b.write((x64.swap_bytes() as u128) | ((x64.reverse_bits() as u128) << 64));
        r128.write(x128.rotate_left(k));
        r128b.write(x128.rotate_right(k8) ^ x128.swap_bytes());
        r128c.write(x128.reverse_bits());
        // Signed results widened by `as i128`: a permutation must keep the sign of `T`.
        si.write(
            ((y16.swap_bytes() as i128) as u128)
                ^ (((y32.rotate_right(k) as i64) as u128) << 1)
                ^ ((y64.reverse_bits() as i128) as u128)
                ^ ((y128.rotate_left(k8) >> 3) as u128)
                ^ ((y16.reverse_bits() as i32) as u128),
        );
        clk.tick().await;
    }
}

/// `min`, `max`, `abs_diff` at every width, unsigned and signed.
#[hardware(sequential)]
async fn im_cmp(
    clk: Clock<MainClk>,
    a: In<u128, MainClk>,
    b: In<u128, MainClk>,
    sa: In<Bits<2>, MainClk>,
    sb: In<Bits<3>, MainClk>,
    m8: RegOut<u64, MainClk>,
    m16: RegOut<u64, MainClk>,
    m32: RegOut<u128, MainClk>,
    m64: RegOut<u128, MainClk>,
    m64s: RegOut<u128, MainClk>,
    m128: RegOut<u128, MainClk>,
    m128s: RegOut<u128, MainClk>,
    d128: RegOut<u128, MainClk>,
) {
    loop {
        let sel_a: u8 = sa.read().as_u8();
        let sel_b: u8 = sb.read().as_u8();
        let x: u128 = a.read();
        // `b`'s pick: 0, all ones, the top bit, the port value, or (4..7) equal to `a`'s pick.
        let y: u128 = b.read();
        let a8: u128 = im_pick(x, sel_a, 8);
        let b8: u128 = im_pick_b(x, y, sel_a, sel_b, 8);
        let a16: u128 = im_pick(x, sel_a, 16);
        let b16: u128 = im_pick_b(x, y, sel_a, sel_b, 16);
        let a32: u128 = im_pick(x, sel_a, 32);
        let b32: u128 = im_pick_b(x, y, sel_a, sel_b, 32);
        let a64: u128 = im_pick(x, sel_a, 64);
        let b64: u128 = im_pick_b(x, y, sel_a, sel_b, 64);
        let a128: u128 = im_pick(x, sel_a, 128);
        let b128: u128 = im_pick_b(x, y, sel_a, sel_b, 128);
        let u8a: u8 = a8 as u8;
        let u8b: u8 = b8 as u8;
        let i8a: i8 = a8 as u8 as i8;
        let i8b: i8 = b8 as u8 as i8;
        let u16a: u16 = a16 as u16;
        let u16b: u16 = b16 as u16;
        let i16a: i16 = a16 as u16 as i16;
        let i16b: i16 = b16 as u16 as i16;
        let u32a: u32 = a32 as u32;
        let u32b: u32 = b32 as u32;
        let i32a: i32 = a32 as u32 as i32;
        let i32b: i32 = b32 as u32 as i32;
        let u64a: u64 = a64 as u64;
        let u64b: u64 = b64 as u64;
        let i64a: i64 = a64 as u64 as i64;
        let i64b: i64 = b64 as u64 as i64;
        let i128a: i128 = a128 as i128;
        let i128b: i128 = b128 as i128;
        m8.write(
            (u8a.min(u8b) as u64)
                | ((u8a.max(u8b) as u64) << 8)
                | ((u8a.abs_diff(u8b) as u64) << 16)
                | ((i8a.min(i8b) as u8 as u64) << 24)
                | ((i8a.max(i8b) as u8 as u64) << 32)
                | ((i8a.abs_diff(i8b) as u64) << 40)
                | ((u8a.min(7) as u64) << 48)
                | ((i8a.max(-3) as u8 as u64) << 56),
        );
        m16.write(
            (u16a.min(u16b) as u64)
                | ((u16a.max(u16b) as u64) << 16)
                | ((i16a.abs_diff(i16b) as u64) << 32)
                | ((i16a.min(i16b) as u16 as u64) << 48),
        );
        m32.write(
            (u32a.abs_diff(u32b) as u128)
                | ((i32a.max(i32b) as u32 as u128) << 32)
                | ((i32a.abs_diff(i32b) as u128) << 64)
                | ((u32a.max(u32b) as u128) << 96),
        );
        m64.write((u64a.min(u64b) as u128) | ((u64a.abs_diff(u64b) as u128) << 64));
        m64s.write((i64a.max(i64b) as u64 as u128) | ((i64a.abs_diff(i64b) as u128) << 64));
        m128.write(a128.max(b128) ^ a128.min(b128).rotate_left(1));
        m128s.write((i128a.min(i128b) as u128) ^ ((i128a.max(i128b) as u128) >> 1));
        d128.write(a128.abs_diff(b128) ^ i128a.abs_diff(i128b).rotate_left(64));
        clk.tick().await;
    }
}

/// The sign family on every signed width, `wrapping_neg` signed and unsigned.
#[hardware(sequential)]
async fn im_sign(
    clk: Clock<MainClk>,
    a: In<u128, MainClk>,
    sa: In<Bits<2>, MainClk>,
    f: RegOut<Bits<32>, MainClk>,
    v8: RegOut<u64, MainClk>,
    v16: RegOut<u64, MainClk>,
    v32: RegOut<u128, MainClk>,
    v64: RegOut<u128, MainClk>,
    v128: RegOut<u128, MainClk>,
    v128b: RegOut<u128, MainClk>,
) {
    loop {
        let sel: u8 = sa.read().as_u8();
        let x8: i8 = im_pick(a.read(), sel, 8) as u8 as i8;
        let x16: i16 = im_pick(a.read(), sel, 16) as u16 as i16;
        let x32: i32 = im_pick(a.read(), sel, 32) as u32 as i32;
        let x64: i64 = im_pick(a.read(), sel, 64) as u64 as i64;
        let x128: i128 = im_pick(a.read(), sel, 128) as i128;
        let mut flags: u32 = (x8.is_positive() as u32)
            | ((x8.is_negative() as u32) << 1)
            | ((x16.is_positive() as u32) << 2)
            | ((x16.is_negative() as u32) << 3)
            | ((x32.is_positive() as u32) << 4)
            | ((x32.is_negative() as u32) << 5)
            | ((x64.is_positive() as u32) << 6)
            | ((x64.is_negative() as u32) << 7)
            | ((x128.is_positive() as u32) << 8)
            | ((x128.is_negative() as u32) << 9);
        flags |= ((x8.signum() as u8 as u32) << 10) | ((x128.signum() as u8 as u32) << 18);
        flags |= ((x16.signum() + 1) as u32) << 26;
        f.write(Bits::from_u32(flags));
        v8.write(
            (x8.wrapping_abs() as u8 as u64)
                | ((x8.unsigned_abs() as u64) << 8)
                | ((x8.wrapping_neg() as u8 as u64) << 16)
                | (((x8 as u8).wrapping_neg() as u64) << 24)
                | ((x8.signum() as u8 as u64) << 32),
        );
        v16.write(
            (x16.wrapping_abs() as u16 as u64)
                | ((x16.unsigned_abs() as u64) << 16)
                | ((x16.wrapping_neg() as u16 as u64) << 32)
                | ((x16.signum() as u16 as u64) << 48),
        );
        v32.write(
            (x32.wrapping_abs() as u32 as u128)
                | ((x32.unsigned_abs() as u128) << 32)
                | (((x32 as u32).wrapping_neg() as u128) << 64)
                | ((x32.signum() as u32 as u128) << 96),
        );
        v64.write((x64.unsigned_abs() as u128) | ((x64.wrapping_abs() as u64 as u128) << 64));
        v128.write(x128.unsigned_abs() ^ ((x128.signum() as u128) << 1));
        v128b.write((x128.wrapping_abs() as u128) ^ (x128 as u128).wrapping_neg().rotate_right(7));
        clk.tick().await;
    }
}

/// `wrapping_shl` / `wrapping_shr` by a run-time amount at every width, unsigned and signed
/// (`>>` arithmetic on a signed one).
#[hardware(sequential)]
async fn im_shift(
    clk: Clock<MainClk>,
    a: In<u128, MainClk>,
    sa: In<Bits<2>, MainClk>,
    s: In<u32, MainClk>,
    h8: RegOut<u64, MainClk>,
    h16: RegOut<u64, MainClk>,
    h32: RegOut<u128, MainClk>,
    h64: RegOut<u128, MainClk>,
    h128: RegOut<u128, MainClk>,
    h128s: RegOut<u128, MainClk>,
) {
    loop {
        let sel: u8 = sa.read().as_u8();
        let k: u32 = s.read();
        let k8: u32 = k & 0xff;
        let x8: u8 = im_pick(a.read(), sel, 8) as u8;
        let x16: u16 = im_pick(a.read(), sel, 16) as u16;
        let x32: u32 = im_pick(a.read(), sel, 32) as u32;
        let x64: u64 = im_pick(a.read(), sel, 64) as u64;
        let x128: u128 = im_pick(a.read(), sel, 128);
        h8.write(
            (x8.wrapping_shl(k) as u64)
                | ((x8.wrapping_shr(k8) as u64) << 8)
                | (((x8 as i8).wrapping_shl(k8) as u8 as u64) << 16)
                | (((x8 as i8).wrapping_shr(k) as u8 as u64) << 24)
                | ((x8.wrapping_shl(11) as u64) << 32),
        );
        h16.write(
            (x16.wrapping_shl(k8) as u64)
                | ((x16.wrapping_shr(k) as u64) << 16)
                | (((x16 as i16).wrapping_shr(k8) as u16 as u64) << 32)
                | (((x16 as i16).wrapping_shl(k) as u16 as u64) << 48),
        );
        h32.write(
            (x32.wrapping_shl(k) as u128)
                | ((x32.wrapping_shr(k8) as u128) << 32)
                | (((x32 as i32).wrapping_shr(k) as u32 as u128) << 64)
                | (((x32 as i32).wrapping_shr(31) as u32 as u128) << 96),
        );
        h64.write((x64.wrapping_shr(k) as u128) | (((x64 as i64).wrapping_shr(k8) as u64 as u128) << 64));
        h128.write(x128.wrapping_shl(k) ^ x128.wrapping_shr(k8));
        h128s.write(((x128 as i128).wrapping_shr(k) as u128) ^ ((x128 as i128).wrapping_shl(k8) as u128));
        clk.tick().await;
    }
}

/// A combinational body: the same methods on `i16`/`u64`, and their path-call forms
/// (`u16::rotate_left(v, k)`, read as the method call, MEASUREMENTS §AI.2).
#[hardware(combinational)]
fn im_comb<D: ClockDomain>(
    a: In<u64, D>,
    b: In<u64, D>,
    s: In<u32, D>,
    o1: Out<u64, D>,
    o2: Out<u64, D>,
    o3: Out<u64, D>,
    o4: Out<u64, D>,
) {
    let x: u64 = a.read();
    let y: u64 = b.read();
    let k: u32 = s.read();
    let p: i16 = x as u16 as i16;
    let q: i16 = y as u16 as i16;
    o1.write(x.rotate_right(k) ^ y.swap_bytes() ^ x.min(y).reverse_bits());
    o2.write(
        (p.abs_diff(q) as u64)
            | ((p.wrapping_abs() as u16 as u64) << 16)
            | ((p.signum() as u16 as u64) << 32)
            | ((q.unsigned_abs() as u64) << 48),
    );
    o3.write(
        (u16::rotate_left(x as u16, k) as u64)
            | ((i16::max(p, q) as u16 as u64) << 16)
            | ((u64::abs_diff(x, y) & 0xffff) << 32)
            | ((i16::wrapping_shr(p, k) as u16 as u64) << 48),
    );
    o4.write(u64::wrapping_neg(x) ^ u64::reverse_bits(y) ^ (i16::is_negative(p) as u64) ^ ((p.is_positive() as u64) << 1));
}

/// Path-call forms in a sequential body, and an unannotated `let` of each result kind (`T`, the
/// unsigned type of `abs_diff`/`unsigned_abs`, `bool`).
#[hardware(sequential)]
async fn im_path_call(
    clk: Clock<MainClk>,
    a: In<i32, MainClk>,
    b: In<i32, MainClk>,
    s: In<u32, MainClk>,
    o: RegOut<u128, MainClk>,
    acc_o: RegOut<u32, MainClk>,
) {
    let mut acc: u32 = 0;
    loop {
        let x: i32 = a.read();
        let y: i32 = b.read();
        let d = i32::abs_diff(x, y);
        let u = x.unsigned_abs();
        let n = i32::is_negative(x);
        let r = i32::rotate_left(x, s.read());
        let w = d >> 31;
        o.write((d as u128) | ((u as u128) << 32) | ((r as u32 as u128) << 64) | ((n as u128) << 96) | ((w as u128) << 100));
        acc = acc.rotate_left(1) ^ u32::swap_bytes(d) ^ (i32::min(x, y) as u32);
        acc_o.write(acc);
        clk.tick().await;
    }
}

// Constant evaluations of the `const fn`s among them, for G-J (`tests/const_evaluators_agree.rs`):
// the two constant evaluators against each other at the edges (`min`/`max` are `Ord`'s, not
// `const`, and outside the grammar).
const IM_PERM: u32 = 0x1234u16.rotate_left(20) as u32 + 0x80u8.rotate_right(9) as u32 + 0x12345678u32.swap_bytes() + (0x01u8.reverse_bits() as u32);
const IM_SIGN: i16 = (-5i16).signum() + (-128i16).wrapping_abs() + (-128i8).unsigned_abs() as i16 + (i16::MIN.wrapping_abs()).signum();
const IM_DIFF: u32 = (-100i8).abs_diff(100) as u32 + 3u16.abs_diff(9) as u32 + (-7i64).wrapping_abs() as u32 + (-3i32).unsigned_abs() + i64::MIN.unsigned_abs().count_ones();
const IM_FLAGS: u8 = (-1i64).is_negative() as u8 + (0i8).is_positive() as u8 * 2 + (1i128).is_positive() as u8 * 4;
const IM_REV: i16 = (-2i16).reverse_bits() ^ (0x7f00i16).swap_bytes() ^ (-1i16).rotate_left(3);

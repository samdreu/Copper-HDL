// Constants are Rust's constants (SPEC I.14.4, D-19 4A), folded by one evaluator.
//
// II.3 rows (d) and (g), REFUSED-one (the transpiler; the macro admits) until Phase C
// step 5 (2026-11-06, `design_docs/review/MEASUREMENTS.md` §J.7): the transpiler's fold
// read a `const` only when its body was a literal, and a `from_lit` turbofish only in
// decimal. Both now go through `copper_analysis::consts`. Since the second sitting
// (§J.13): associated consts, `const fn`s (through a const item, or as an associated
// fn — a single-segment call is refused by R11 in both front-ends, §J.13), and a
// const generic as a register's power-on value.

const CE_BASE: u8 = 1 + 2;
const CE_START: u8 = CE_BASE * 4 + 0x1;
const CE_ON: bool = true;

#[hardware(sequential)]
async fn const_init_expr(
    clk: Clock<MainClk>,
    x: In<Bits<8>, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
    r: RegOut<Bits<16>, MainClk>,
) {
    let mut n: u8 = CE_START;
    let mut m: Bits<8> = Bits::from_lit::<0xA5>();
    let mut w: Bits<16> = Bits::from_lit::<1_000>();
    let mut on: bool = CE_ON;
    loop {
        q.write(Bits::from_u8(n) ^ m);
        r.write(w);
        n = n.wrapping_add(1);
        m = x.read() ^ Bits::from_lit::<0b1010_0101>();
        w = w + Bits::from_lit::<0x0_10>();
        if on {
            on = false;
        }
        clk.tick().await;
    }
}

const fn ce_clog2(n: usize) -> usize {
    match n {
        0 | 1 => 0,
        _ => {
            let mut bits = 0;
            let mut v = n - 1;
            while v > 0 {
                v >>= 1;
                bits += 1;
            }
            bits
        }
    }
}

struct CeCfg;
impl CeCfg {
    const DEPTH: u8 = 12;
    const fn half(n: u8) -> u8 {
        n / 2
    }
}

const CE_W: u8 = ce_clog2(CeCfg::DEPTH as usize) as u8;

/// An associated const, a `const fn` through a const item, an associated `const fn`.
#[hardware(sequential)]
async fn const_assoc_fn(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut a: u8 = CeCfg::DEPTH;
    let mut b: u8 = CE_W;
    let mut c: u8 = CeCfg::half(201);
    loop {
        q.write(Bits::from_u8(a) ^ Bits::from_u8(b) ^ Bits::from_u8(c));
        a = a.wrapping_add(1);
        b = b ^ x.read().as_u8();
        c = c.wrapping_add(b);
        clk.tick().await;
    }
}

/// A const generic as a register's power-on value (the sweep instance: `START = 5`).
/// `n` is observed through comparisons, not converted to `Bits` — a `usize` reaching
/// a `Bits` operand is step 8's width class (I.14.7 (3)), not this row's.
#[hardware(sequential)]
async fn const_generic_init<const START: usize>(clk: Clock<MainClk>, x: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut n: usize = START;
    loop {
        if n == 5 {
            q.write(x.read());
        } else if n % 3 == 0 {
            q.write(Bits::from_u8(77));
        } else {
            q.write(Bits::zero());
        }
        n = n + 1;
        clk.tick().await;
    }
}

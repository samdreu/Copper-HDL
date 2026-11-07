// Constants are Rust's constants (SPEC I.14.4, D-19 4A), folded by one evaluator.
//
// II.3 rows (d) and (g), REFUSED-one (the transpiler; the macro admits) until Phase C
// step 5 (2026-11-06, `design_docs/review/MEASUREMENTS.md` §J.7): the transpiler's fold
// read a `const` only when its body was a literal, and a `from_lit` turbofish only in
// decimal. Both now go through `copper_analysis::consts`. Not yet covered (recorded in
// §J.7): an associated const, a const generic as a value, a `const fn`.

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

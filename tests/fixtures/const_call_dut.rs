// SPEC I.14.4 — Rust's constants — through the spellings the transpiler's fold refused
// until 2026-11-08 (line `II.3 found const-call`, MEASUREMENTS §N.5 item 4, §V.4). Both
// constant evaluators now fold each — `copper_analysis::consts::ConstEnv` (the
// transpiler's) and the interpreter's (`copper-ref`), one grammar, two evaluators (D-21
// B2) — and `tests/const_evaluators_agree.rs` (G-J) compares the two on these items:
// a `const` integer method (`CC_BASE.wrapping_add(10)`), a tuple-struct constructor and
// its field (`CcPair(3, 5)`, `CC_PAIR.0`), an immutable `static`.
//
// The module starts a register from the `static` — the spelling that reaches the
// emitted SV today. A register started from the method call or the tuple field is
// still refused by the transpiler's register fold (`chir_lower::fold_const_init`, the
// line's remaining half, owned by copper-codegen).

const CC_BASE: u8 = 250;
const CC_WRAP: u8 = CC_BASE.wrapping_add(10); // 4
static CC_SEED: u8 = 0x21;
struct CcPair(u8, u8);
const CC_PAIR: CcPair = CcPair(3, 5);
const CC_SUM: u8 = CC_PAIR.0 + CC_PAIR.1; // 8
const CC_ONES: u32 = CC_BASE.count_ones(); // 6

/// A register whose power-on value is an immutable `static` (0x21), shown at obs_1.
#[hardware(sequential)]
async fn cc_static_init(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut g: u8 = CC_SEED;
    loop {
        q.write(Bits::from_u8(g));
        clk.tick().await;
        g = g ^ a.read().as_u8();
    }
}

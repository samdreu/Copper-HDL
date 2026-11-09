// SPEC I.14.4 — Rust's constants — through the spellings the transpiler's fold refused
// until 2026-11-08 (line `II.3 found const-call`, MEASUREMENTS §N.5 item 4, §V.4). Both
// constant evaluators now fold each — `copper_analysis::consts::ConstEnv` (the
// transpiler's) and the interpreter's (`copper-ref`), one grammar, two evaluators (D-21
// B2) — and `tests/const_evaluators_agree.rs` (G-J) compares the two on these items:
// a `const` integer method (`CC_BASE.wrapping_add(10)`), a tuple-struct constructor and
// its field (`CcPair(3, 5)`, `CC_PAIR.0`), an immutable `static`.
//
// The modules start registers from each spelling; the transpiler's register fold
// (`chir_lower::fold_const_init`) delegates the method call and the field to
// `ConstEnv` (`copper-codegen/src/const_fold.rs`), and each value shows at obs_1.

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

/// Registers started from a `const` method call (on a typed const, and on the
/// register's own type), a tuple-struct constructor's field, and a tuple-struct
/// const's field.
#[hardware(sequential)]
async fn cc_call_init(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    o: RegOut<Bits<8>, MainClk>,
    p: RegOut<Bits<8>, MainClk>,
    s: RegOut<Bits<8>, MainClk>,
) {
    let mut m: u8 = CC_BASE.wrapping_add(7); // 1
    let mut t: u8 = CcPair(9, 1).0; // 9
    let mut f: u8 = CC_PAIR.1.wrapping_mul(CC_ONES as u8); // 30
    loop {
        o.write(Bits::from_u8(m));
        p.write(Bits::from_u8(t));
        s.write(Bits::from_u8(f));
        clk.tick().await;
        m = m.wrapping_add(1);
        t = t ^ a.read().as_u8();
        f = f.wrapping_add(3);
    }
}

// The same constants named in a body (line `II.3 found const-call-body`, MEASUREMENTS §V.4,
// §X.1): until 2026-11-08 the transpiler emitted a file const as a `localparam` from its
// Rust text only, so one whose initializer is a method call or a tuple field (or an
// immutable `static`, never captured as a `localparam`) was refused wherever a body
// expression or a `for` bound named it. It is now stated by the value `ConstEnv` folds it
// to (a sized `localparam` for `u8`); rustc's values are pinned by
// `tests/const_evaluators_agree.rs::the_widened_constant_grammar_agrees_with_rustc`.
const CC_REPS: usize = (CC_ONES as usize).wrapping_sub(3); // 3

/// Each constant in a clocked body expression: a method-call const, a tuple-field const, a
/// `static` and a `u32` method-call const narrowed by a cast.
#[hardware(sequential)]
async fn cc_body_expr(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>, p: RegOut<Bits<8>, MainClk>) {
    let mut m: u8 = 0;
    let mut n: u8 = 0;
    loop {
        o.write(Bits::from_u8(m));
        p.write(Bits::from_u8(n));
        clk.tick().await;
        m = m.wrapping_add(CC_WRAP) ^ a.read().as_u8();
        n = n.wrapping_add(CC_SUM).wrapping_add(CC_SEED) ^ (CC_ONES as u8);
    }
}

/// A method-call const as a tick-free `for` bound (accumulating into a wire of the
/// resumption) and as a ticking `for` bound — the module loop's only boundary, non-empty
/// through the constant (I.8.4(b), §V.3).
#[hardware(sequential)]
async fn cc_body_for(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, o: RegOut<Bits<8>, MainClk>) {
    let mut m: u8 = 0;
    loop {
        let mut acc: u8 = CC_SUM;
        for _ in 0..CC_WRAP {
            acc = acc.wrapping_add(a.read().as_u8());
        }
        m = m ^ acc;
        for _ in 0..CC_REPS {
            o.write(Bits::from_u8(m));
            clk.tick().await;
        }
    }
}

/// The constants in a combinational body.
#[hardware(combinational)]
pub fn cc_body_comb<D: ClockDomain>(a: In<Bits<8>, D>, o: Out<Bits<8>, D>) {
    o.write(Bits::from_u8(a.read().as_u8().wrapping_add(CC_WRAP).wrapping_add(CC_SUM) ^ CC_SEED));
}

// A whole-struct assignment whose right-hand side reads the struct it assigns (line `II.3
// found struct-literal-self-read`, MEASUREMENTS §Y.5, §Z.1). Rust evaluates every field of
// `Ssr { x: p.y + a, y: p.x }` from the OLD `p`, then assigns. Until 2026-11-08 the
// transpiler assigned the literal field by field, and the forwarding walk gave `y` the new
// `x` (`p_y <= (p_y + a)`): 6 of 6 sequences DISAGREE, `sim=ref≠SV`. The transpiler now binds
// such a right-hand side to a fresh struct temp first (`let p__litK = <rhs>; p = p__litK;`).

#[derive(Clone, Copy)]
struct Ssr {
    x: Bits<8>,
    y: Bits<8>,
}

const fn ssr_make(x: Bits<8>, y: Bits<8>) -> Ssr {
    Ssr { x, y }
}

/// The ledger line's probe: a register swap with an input folded into one field.
#[hardware(sequential)]
async fn ssr_swap(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut p = Ssr { x: Bits::zero(), y: Bits::zero() };
    loop {
        q.write(p.x ^ (p.y << 1));
        p = Ssr { x: p.y + a.read(), y: p.x };
        clk.tick().await;
    }
}

/// The same read of an earlier field, the literal under a condition (the other arm keeps `p`).
#[hardware(sequential)]
async fn ssr_conditional(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    c: In<Logic, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut p = Ssr { x: Bits::zero(), y: Bits::zero() };
    loop {
        p = if c.read().as_bool() { Ssr { x: p.x ^ a.read(), y: p.x } } else { p };
        q.write(p.x + p.y);
        clk.tick().await;
    }
}

/// Through a constructor call whose arguments read the target.
#[hardware(sequential)]
async fn ssr_ctor(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    let mut p = Ssr { x: Bits::zero(), y: Bits::zero() };
    loop {
        p = ssr_make(p.y ^ a.read(), p.x + Bits::from_lit::<1>());
        q.write(p.x ^ p.y);
        clk.tick().await;
    }
}

/// A struct wire of the segment, swapped twice: the second literal reads the first's result.
#[hardware(sequential)]
async fn ssr_wire(clk: Clock<MainClk>, a: In<Bits<8>, MainClk>, b: In<Bits<8>, MainClk>, q: RegOut<Bits<8>, MainClk>) {
    loop {
        let mut w = Ssr { x: a.read(), y: b.read() };
        w = Ssr { x: w.y, y: w.x + w.y };
        w = Ssr { x: w.y, y: w.x };
        q.write(w.x - w.y);
        clk.tick().await;
    }
}

/// Inside a tick-free `for` (the §Y.1 temp's per-field form) and on the extraction route.
#[hardware(sequential)]
async fn ssr_for_and_phases(
    clk: Clock<MainClk>,
    a: In<Bits<8>, MainClk>,
    c: In<Logic, MainClk>,
    q: RegOut<Bits<8>, MainClk>,
) {
    let mut p = Ssr { x: Bits::zero(), y: Bits::from_lit::<1>() };
    loop {
        q.write(p.x ^ p.y);
        if c.read().as_bool() {
            for _ in 0..2 {
                p = Ssr { x: p.y + a.read(), y: p.x };
            }
            clk.tick().await;
        } else {
            p = Ssr { x: p.y, y: p.x ^ a.read() };
            clk.tick().await;
        }
    }
}

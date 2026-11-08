// A `match` pattern naming a non-integer `const` (line `II.3 found const-pattern`, MEASUREMENTS
// §T.3, §U.2): until 2026-11-08 the transpiler read the uppercase name as an enum variant and
// refused ("tuple-pattern match lowering is not yet implemented"); it now lowers the pattern by
// the const's value. An integer file const (`CP_K`) stays a `localparam` case label, as before.
// A const whose value is X is OUTSIDE (D-24; `copper-macros/tests/ui/fail/outside_x_pattern.rs`
// for the body spelling, `copper-codegen/tests/unsupported_constructs.rs` for the file one).

const CP_ONE: Logic = Logic::One;
const CP_ZERO: Logic = Logic::Zero;
const CP_TWO: Bits<2> = Bits::from_lit::<2>();
const CP_THREE: Bits<2> = Bits::from_lit::<3>();
const CP_K: u8 = 5;

/// Statement `match`es on file consts: a `Logic`, a `Bits<2>` and an integer one.
#[hardware(combinational)]
pub fn cp_statement<D: ClockDomain>(
    l: In<Logic, D>,
    s: In<Bits<2>, D>,
    n: In<Bits<8>, D>,
    o: Out<Bits<4>, D>,
) {
    let mut r: Bits<4> = Bits::zero();
    match l.read() {
        CP_ONE => r = r | Bits::from_lit::<1>(),
        _ => {}
    }
    match s.read() {
        CP_TWO => r = r | Bits::from_lit::<2>(),
        CP_THREE => r = r | Bits::from_lit::<6>(),
        _ => {}
    }
    match n.read().as_u8() {
        CP_K => r = r ^ Bits::from_lit::<8>(),
        _ => {}
    }
    o.write(r);
}

/// An expression `match` over a tuple, with a body `const`, in a ticking body.
#[hardware(sequential)]
async fn cp_tuple_expr(
    clk: Clock<MainClk>,
    l: In<Logic, MainClk>,
    s: In<Bits<2>, MainClk>,
    q: RegOut<Bits<4>, MainClk>,
) {
    const BODY_TWO: Bits<2> = Bits::from_lit::<2>();
    loop {
        let v: Bits<4> = match (l.read(), s.read()) {
            (CP_ZERO, BODY_TWO) => Bits::from_lit::<9>(),
            (CP_ONE, CP_THREE) => Bits::from_lit::<5>(),
            (CP_ONE, _) => Bits::from_lit::<1>(),
            _ => Bits::from_lit::<14>(),
        };
        q.write(v);
        clk.tick().await;
    }
}

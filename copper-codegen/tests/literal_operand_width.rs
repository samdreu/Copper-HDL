//! An integer literal that is a direct operand of an arithmetic / bitwise /
//! comparison operator, or an arm of an `if`-expression, takes the width of the
//! operand it combines with — including a literal whose width the transpiler
//! had GUESSED from its constructor (`Bits::from_usize(1)` is 32 bits by the
//! `usize` rule) rather than read off the Rust type.
//!
//! The motivating failure (2026-10-20 artifact audit): `simple_counter`'s
//! `count = count + Bits::from_usize(1)` on a `Bits<8>` emitted
//! `count + 32'd1`, and the corpus sweep's `-Wall` Verilator build rejected it
//! with WIDTHEXPAND / WIDTHTRUNC. In the Rust source the `1` IS a `Bits<8>`
//! (rustc infers the constructor's `N` from `count`; `Bits<N>`'s operators are
//! same-type), so the 32 was never the author's width.
//!
//! A literal that does not fit the adopted width is a spanned error, never a
//! silent truncation — the simulator's `Bits::from_usize` panics on the same
//! value, so both front-ends refuse the same program.

fn transpile(src: &str) -> Result<String, String> {
    copper_codegen::transpile_source(src, None, &copper_codegen::EmitConfig::default())
        .map_err(|e| e.to_string())
}

const COUNTER: &str = r#"
struct MainClk;
impl ClockDomain for MainClk {}

#[hardware(sequential)]
async fn simple_counter(
    clk: Clock<MainClk>,
    reset_i: In<Logic, MainClk>,
    en_i: In<Logic, MainClk>,
    count_o: Out<Bits<8>, MainClk>,
) {
    let mut count = Bits::<8>::x();
    loop {
        if reset_i.read() == Logic::One {
            count = Bits::zero();
        } else if en_i.read() == Logic::One {
            count = count + Bits::from_usize(1)
        }
        count_o.write(count);
        clk.tick().await;
    }
}
"#;

/// The counter's increment adds at `count`'s width.
#[test]
fn from_usize_literal_takes_the_register_operands_width() {
    let sv = transpile(COUNTER).expect("transpiles");
    assert!(sv.contains("(count + 8'd1)"), "got:\n{sv}");
    assert!(!sv.contains("32'd1"), "the usize guess must not survive, got:\n{sv}");
}

/// The same rule for a comparison and for a bitwise op on an input.
#[test]
fn from_usize_literal_takes_an_input_operands_width() {
    let src = r#"
#[hardware(combinational)]
fn f(i: In<Bits<12>, ()>, eq_o: Out<Logic, ()>, and_o: Out<Bits<12>, ()>) {
    let x = i.read();
    eq_o.write(if x == Bits::from_usize(5) { Logic::One } else { Logic::Zero });
    and_o.write(x & Bits::from_usize(0xFF0));
}
"#;
    let sv = transpile(src).expect("transpiles");
    assert!(sv.contains("12'd5"), "got:\n{sv}");
    assert!(sv.contains("12'd4080"), "got:\n{sv}");
    assert!(!sv.contains("32'd"), "got:\n{sv}");
}

/// An `if`-expression arm is one value at two spellings: a guessed-width
/// literal arm takes the other arm's width.
#[test]
fn from_usize_literal_arm_takes_the_other_arms_width() {
    let src = r#"
#[hardware(combinational)]
fn f(sel: In<Logic, ()>, i: In<Bits<6>, ()>, o: Out<Bits<6>, ()>) {
    let x = i.read();
    o.write(if sel.read() == Logic::One { Bits::from_usize(3) } else { x });
}
"#;
    let sv = transpile(src).expect("transpiles");
    assert!(sv.contains("6'd3"), "got:\n{sv}");
    assert!(!sv.contains("32'd3"), "got:\n{sv}");
}

/// A literal that needs more bits than the operand it combines with is refused,
/// with the value and the width in the message and a source position in front.
#[test]
fn a_literal_that_does_not_fit_the_operands_width_is_a_spanned_error() {
    let src = r#"
#[hardware(combinational)]
fn f(i: In<Bits<8>, ()>, o: Out<Bits<8>, ()>) {
    o.write(i.read() + Bits::from_usize(300));
}
"#;
    let err = transpile(src).expect_err("300 does not fit in 8 bits");
    assert!(err.contains("300") && err.contains("8-bit"), "got: {err}");
    // `CHIRLowerError`'s Display prefixes `line:col: `.
    assert!(
        err.split(':').take(2).all(|part| part.trim().parse::<usize>().is_ok()),
        "expected a `line:col:` prefix, got: {err}"
    );
}

/// The ceiling is inclusive: the largest value that fits is accepted.
#[test]
fn the_largest_fitting_literal_is_accepted() {
    let src = r#"
#[hardware(combinational)]
fn f(i: In<Bits<8>, ()>, o: Out<Bits<8>, ()>) {
    o.write(i.read() + Bits::from_usize(255));
}
"#;
    let sv = transpile(src).expect("255 fits in 8 bits");
    assert!(sv.contains("8'd255"), "got:\n{sv}");
}

//! The combinational leg (SPEC §I.11, D-14): a `combinational` module is a function
//! evaluated once per settle. Each case is a sentence of I.11 made concrete.

use copper_core::Logic;
use copper_ref::{Module, RefError, Value};

const PRELUDE: &str = "
struct C; impl ClockDomain for C {}
";

fn module(src: &str, name: &str) -> Result<Module, RefError> {
    Module::new(&format!("{PRELUDE}{src}"), name, &[])
}

fn eval_u(m: &mut Module, ins: &[(&str, u128)]) -> Result<u128, RefError> {
    let vals: Vec<(&str, Value)> = ins.iter().map(|(n, v)| (*n, Value::Lit(*v))).collect();
    m.evaluate(&vals)?;
    let o = &m.observe()[0].1;
    o.to_bits().map(|b| b.iter().enumerate().fold(0, |a, (i, l)| a | ((*l == Logic::One) as u128) << i))
}

/// I.11.2(b), (c): the last write on the path is the wire's value; the dead
/// default it overrides is admitted (D-14 A1).
#[test]
fn last_write_wins_and_the_dead_write_is_admitted() {
    let src = "
#[hardware(combinational)]
fn dflt(sel: In<Logic, C>, x: In<Bits<4>, C>, o: Out<Bits<4>, C>) {
    o.write(Bits::from_u8(3));
    if sel.read() == Logic::One { o.write(x.read()); }
}";
    let mut m = module(src, "dflt").unwrap();
    assert_eq!(eval_u(&mut m, &[("sel", 0), ("x", 9)]).unwrap(), 3);
    assert_eq!(eval_u(&mut m, &[("sel", 1), ("x", 9)]).unwrap(), 9);
    // I.11.2(d): no hold — the next evaluation is a function of its reads alone.
    assert_eq!(eval_u(&mut m, &[("sel", 0), ("x", 9)]).unwrap(), 3);
}

/// I.11.1, I.7.2(e): every local is a wire of its evaluation — nothing survives it.
#[test]
fn no_local_survives_an_evaluation() {
    let src = "
#[hardware(combinational)]
fn acc(x: In<Bits<4>, C>, o: Out<Bits<4>, C>) {
    let mut s = Bits::<4>::zero();
    for i in 0..4 { if x.read()[i] == Logic::One { s = s + Bits::from_u8(1); } }
    o.write(s);
}";
    let mut m = module(src, "acc").unwrap();
    assert_eq!(eval_u(&mut m, &[("x", 0b1011)]).unwrap(), 3);
    assert_eq!(eval_u(&mut m, &[("x", 0b1011)]).unwrap(), 3);
}

/// I.11.3, D-14 A3: a declared `Out` no path writes is refused; a path that leaves
/// a port unwritten has no value.
#[test]
fn the_never_written_and_the_unwritten_port_are_refused() {
    let never = "
#[hardware(combinational)]
fn nw(x: In<Logic, C>, o: Out<Logic, C>, p: Out<Logic, C>) { o.write(x.read()); }";
    let e = module(never, "nw").unwrap_err();
    assert!(matches!(e, RefError::Unsupported(ref s) if s.contains("I.11.3") && s.contains("`p`")), "{e}");
    let some_path = "
#[hardware(combinational)]
fn sp(x: In<Logic, C>, o: Out<Logic, C>) { if x.read() == Logic::One { o.write(Logic::One); } }";
    let mut m = module(some_path, "sp").unwrap();
    m.evaluate(&[("x", Value::logic(Logic::One))]).unwrap();
    let e = m.evaluate(&[("x", Value::logic(Logic::Zero))]).unwrap_err();
    assert!(matches!(e, RefError::Unsupported(ref s) if s.contains("I.11.3")), "{e}");
}

/// I.2.7: the tick-free fragment has no `loop`; I.10.6: no memory on a combinational module.
#[test]
fn the_fragment_is_tick_free() {
    let lp = "
#[hardware(combinational)]
fn lp(x: In<Logic, C>, o: Out<Logic, C>) { loop { o.write(x.read()); break; } }";
    let e = module(lp, "lp").unwrap_err();
    assert!(matches!(e, RefError::Unsupported(ref s) if s.contains("I.2.7")), "{e}");
}

/// I.14.4: a `const { assert!(…) }` is evaluated at the bound parameters and a
/// failing one refuses the instance, as rustc does at monomorphization.
#[test]
fn a_failing_const_assert_refuses_the_instance() {
    let src = "
#[hardware(combinational)]
fn ca<const N: usize, const M: usize>(x: In<Bits<N>, C>, o: Out<Bits<N>, C>) {
    const { assert!(M == N - 1, \"M is N - 1\") };
    o.write(x.read());
}";
    let full = format!("{PRELUDE}{src}");
    assert!(Module::new(&full, "ca", &[("N", 4), ("M", 3)]).is_ok());
    let e = Module::new(&full, "ca", &[("N", 4), ("M", 2)]).unwrap_err();
    assert!(matches!(e, RefError::Parse(ref s) if s.contains("monomorphization")), "{e}");
}

//! The interpreter's data obligations of SPEC I.14.7 (10) (Phase C step 8,
//! `design_docs/review/MEASUREMENTS.md` §M): integer and `bool` ports, `From`, `/`,
//! the unsized `all_ones`. Each kernel is D-19's probe or a twin of one, run under
//! the one-resumption protocol and checked against Rust's own arithmetic (the
//! simulator *is* rustc's lowering, SPEC_OVERVIEW §1). The corpus sweep cannot drive
//! these: its generator stimulates `Logic`/`Bits` ports only, so the integer-port
//! probes `int_wrapping` / `int_overflow` live here. The `Bits` probes (`xp_div`,
//! `w1_from`, `k_ones_nowidth`) run on all three legs in
//! `tests/fixtures/data_i14_dut.rs`.

use copper_core::Logic;
use copper_ref::{IntTy, Module, Value};

const PRELUDE: &str = "
struct C; impl ClockDomain for C {}
";

fn module(src: &str, name: &str) -> Module {
    Module::new(&format!("{PRELUDE}{src}"), name, &[]).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn out(m: &Module, port: &str) -> Value {
    m.observe().into_iter().find(|(n, _)| n == port).unwrap().1
}

/// D-19's `int_wrapping`: `u8` ports, `wrapping_add` — modular at 8 bits.
#[test]
fn int_wrapping_on_u8_ports() {
    let src = "
#[hardware(sequential)]
async fn int_wrapping(clk: Clock<C>, a: In<u8, C>, b: In<u8, C>, q: RegOut<u8, C>) {
    loop {
        let x = a.read();
        let y = b.read();
        q.write(x.wrapping_add(y));
        clk.tick().await;
    }
}";
    let mut m = module(src, "int_wrapping");
    assert_eq!(out(&m, "q"), Value::Int(0, IntTy::U8), "obs_0 is the payload's zero (I.6.6)");
    for (a, b) in [(1u8, 2u8), (200, 100), (255, 1), (0, 0), (128, 128)] {
        m.resume(&[("a", Value::Int(a as i128, IntTy::U8)), ("b", Value::Int(b as i128, IntTy::U8))])
            .unwrap();
        m.edge();
        assert_eq!(out(&m, "q"), Value::Int(a.wrapping_add(b) as i128, IntTy::U8), "{a} + {b}");
    }
    // The same through the bit-level port interface the sweep's harness uses.
    let bits = |v: u8| (0..8).map(|i| if (v >> i) & 1 == 1 { Logic::One } else { Logic::Zero }).collect::<Vec<_>>();
    let (a, b) = (bits(250), bits(10));
    m.resume_bits(&[("a", &a), ("b", &b)]).unwrap();
    m.edge();
    let q = m.observe_bits().into_iter().find(|(n, _)| n == "q").unwrap().1;
    assert_eq!(q, bits(4), "250 + 10 wraps to 4");
}

/// D-19's `int_overflow`: `+` on `u8` overflows — a run-time failure (I.14.3(c); the
/// simulator panics "attempt to add with overflow" in the sweep's debug profile).
#[test]
fn int_overflow_is_a_run_time_failure() {
    let src = "
#[hardware(sequential)]
async fn int_overflow(clk: Clock<C>, a: In<u8, C>, b: In<u8, C>, q: RegOut<u8, C>) {
    loop {
        let x = a.read();
        let y = b.read();
        q.write(x + y);
        clk.tick().await;
    }
}";
    let mut m = module(src, "int_overflow");
    m.resume(&[("a", Value::Int(100, IntTy::U8)), ("b", Value::Int(100, IntTy::U8))]).unwrap();
    m.edge();
    assert_eq!(out(&m, "q"), Value::Int(200, IntTy::U8));
    let r = m.resume(&[("a", Value::Int(200, IntTy::U8)), ("b", Value::Int(100, IntTy::U8))]);
    assert!(r.is_err(), "200 + 100 on u8 is Rust's overflow, a run-time failure: {r:?}");
}

/// A `bool` port, an `i16` port and `saturating_*`, `/`.
#[test]
fn bool_and_signed_ports_saturating_and_division() {
    let src = "
#[hardware(sequential)]
async fn mixed(clk: Clock<C>, en: In<bool, C>, s: In<i16, C>, a: In<u8, C>, b: In<u8, C>,
               q: RegOut<i16, C>, sat: RegOut<u8, C>, d: RegOut<u8, C>, f: RegOut<bool, C>) {
    loop {
        if en.read() { q.write(s.read().saturating_mul(3)); }
        sat.write(a.read().saturating_sub(b.read()));
        d.write(a.read() / (b.read() | 1));
        f.write(!en.read());
        clk.tick().await;
    }
}";
    let mut m = module(src, "mixed");
    for (en, s, a, b) in [(true, 20000i16, 9u8, 4u8), (false, -5, 3, 200), (true, -20000, 255, 0), (true, -7, 7, 7)] {
        let q_before = out(&m, "q");
        m.resume(&[
            ("en", Value::Bool(en)),
            ("s", Value::Int(s as i128, IntTy::I16)),
            ("a", Value::Int(a as i128, IntTy::U8)),
            ("b", Value::Int(b as i128, IntTy::U8)),
        ])
        .unwrap();
        m.edge();
        let q_want = if en { Value::Int(s.saturating_mul(3) as i128, IntTy::I16) } else { q_before };
        assert_eq!(out(&m, "q"), q_want, "saturating_mul {s}");
        assert_eq!(out(&m, "sat"), Value::Int(a.saturating_sub(b) as i128, IntTy::U8));
        assert_eq!(out(&m, "d"), Value::Int((a / (b | 1)) as i128, IntTy::U8));
        assert_eq!(out(&m, "f"), Value::Bool(!en));
    }
}

/// `Logic::from(Bits<1>)`, `Bits::from(Logic)`, `.into()`, `/` on `Bits` (X on a
/// zero divisor), and the unsized `Bits::all_ones()` (I.14.4) — each refused by the
/// interpreter before Phase C step 8.
#[test]
fn from_division_and_unsized_all_ones() {
    let src = "
#[hardware(sequential)]
async fn bits_data(clk: Clock<C>, a: In<Bits<8>, C>, b: In<Bits<8>, C>, l: In<Logic, C>,
                   q: RegOut<Bits<8>, C>, o: RegOut<Bits<8>, C>, w: RegOut<Logic, C>, w2: RegOut<Bits<1>, C>) {
    loop {
        q.write(a.read() / b.read());
        o.write(a.read() ^ Bits::all_ones());
        let one: Bits<1> = Bits::from(l.read());
        w.write(Logic::from(one));
        w2.write(l.read().into());
        clk.tick().await;
    }
}";
    let mut m = module(src, "bits_data");
    let bits = |v: u8| (0..8).map(|i| if (v >> i) & 1 == 1 { Logic::One } else { Logic::Zero }).collect::<Vec<_>>();
    for (a, b, l) in [(200u8, 7u8, Logic::One), (9, 0, Logic::Zero), (255, 255, Logic::One)] {
        let (ab, bb, lb) = (bits(a), bits(b), vec![l]);
        m.resume_bits(&[("a", &ab), ("b", &bb), ("l", &lb)]).unwrap();
        m.edge();
        let obs = m.observe_bits();
        let get = |n: &str| obs.iter().find(|(k, _)| k == n).unwrap().1.clone();
        if b == 0 {
            assert!(get("q").iter().all(|x| *x == Logic::X), "a / 0 is all X (I.14.3(b))");
        } else {
            assert_eq!(get("q"), bits(a / b), "{a} / {b}");
        }
        assert_eq!(get("o"), bits(!a), "a ^ all_ones");
        assert_eq!(get("w"), vec![l]);
        assert_eq!(get("w2"), vec![l]);
    }
}

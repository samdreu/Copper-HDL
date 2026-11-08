//! The design leg (SPEC §I.13, D-18): a structural module run as the design it
//! names. Each case a sentence of I.13 (or I.11.4) made concrete, by hand.

use copper_core::Logic;
use copper_ref::{Design, RefError, Value};

const PRELUDE: &str = "
struct C; impl ClockDomain for C {}
";

fn bit(d: &Design, p: &str) -> u8 {
    let o = d.observe();
    let v = &o.iter().find(|(n, _)| n == p).unwrap().1;
    (v.to_bits().unwrap()[0] == Logic::One) as u8
}

fn l(b: u8) -> Value {
    Value::logic(if b == 1 { Logic::One } else { Logic::Zero })
}

const MEALY: &str = "
#[hardware(sequential)]
async fn core(clk: Clock<C>, x: In<Logic, C>, full: RegOut<Logic, C>) {
    let mut f = Logic::Zero;
    loop {
        if x.read() == Logic::One { f = !f; }
        full.write(f);
        clk.tick().await;
    }
}
#[hardware(combinational)]
fn ready(full: In<Logic, C>, drain: In<Logic, C>, r: Out<Logic, C>) {
    r.write(!full.read() | drain.read());
}
#[hardware(structural)]
async fn top(clk: Clock<C>, x: In<Logic, C>, drain: In<Logic, C>, rdy: Out<Logic, C>) {
    let full = net::<Logic, C>();
    core(clk, x, full.0);
    ready(full.1, drain, rdy);
}";

/// I.11.5, I.13.2(b): the Mealy pair. `obs_N(rdy) = ~full_N | drain_N`, `full` the
/// `RegOut` as committed at edge N (a source of the settle, I.6.4). x = [1, 0, 1, 1],
/// drain = [0, 0, 0, 1]: full = 1, 1, 0, 1 → rdy = 0, 0, 1, 1; obs_0 = ~init | drain_1 = 1.
#[test]
fn the_mealy_pair_reads_the_committed_register_and_the_current_input() {
    let mut d = Design::new(&format!("{PRELUDE}{MEALY}"), "top").unwrap();
    let (xs, ds) = ([1u8, 0, 1, 1], [0u8, 0, 0, 1]);
    let mut rdy = Vec::new();
    for i in 0..4 {
        d.drive("x", l(xs[i])).unwrap();
        d.drive("drain", l(ds[i])).unwrap();
        d.pre_edge("clk").unwrap();
        if i == 0 {
            assert_eq!(bit(&d, "rdy"), 1, "obs_0 = f(init(full), x_1)");
        }
        d.post_edge("clk").unwrap();
        rdy.push(bit(&d, "rdy"));
    }
    assert_eq!(rdy, vec![0, 0, 1, 1]);
}

/// I.13.2(a): the settle is a function of the sources, reached by one evaluation of
/// each function in a topological order — whatever order the body lists them in.
#[test]
fn one_settle_evaluates_in_topological_order() {
    let src = "
#[hardware(combinational)]
fn inv(a: In<Logic, C>, o: Out<Logic, C>) { o.write(!a.read()); }
#[hardware(combinational)]
fn buf(a: In<Logic, C>, o: Out<Logic, C>) { o.write(a.read()); }
#[hardware(structural)]
async fn chain(clk: Clock<C>, x: In<Logic, C>, y: Out<Logic, C>) {
    let n1 = net::<Logic, C>();
    let n2 = net::<Logic, C>();
    buf(n2.1, y);
    inv(n1.1, n2.0);
    inv(x, n1.0);
}";
    let mut d = Design::new(&format!("{PRELUDE}{src}"), "chain").unwrap();
    for x in [1u8, 0, 1] {
        d.drive("x", l(x)).unwrap();
        d.settle().unwrap();
        assert_eq!(bit(&d, "y"), x, "y = !!x after one settle");
    }
}

/// I.11.4, D-14 A4: a combinational cycle is OUTSIDE — refused, naming its instances.
#[test]
fn a_combinational_cycle_is_refused() {
    let src = "
#[hardware(combinational)]
fn inv(a: In<Logic, C>, o: Out<Logic, C>) { o.write(!a.read()); }
#[hardware(structural)]
async fn ring(clk: Clock<C>, x: In<Logic, C>, y: Out<Logic, C>) {
    let n1 = net::<Logic, C>();
    let n2 = net::<Logic, C>();
    inv(n1.1, n2.0);
    inv(n2.1, n1.0);
    inv(x, y);
}";
    let e = Design::new(&format!("{PRELUDE}{src}"), "ring").err().expect("refused");
    assert!(matches!(e, RefError::Unsupported(ref s) if s.contains("I.11.4") && s.contains("inv#0")), "{e}");
}

/// I.13.1, R22: one driver per wire; I.13.4(d): every net driven.
#[test]
fn one_driver_per_wire_and_every_net_driven() {
    let two = "
#[hardware(combinational)]
fn buf(a: In<Logic, C>, o: Out<Logic, C>) { o.write(a.read()); }
#[hardware(structural)]
async fn two(clk: Clock<C>, x: In<Logic, C>, y: Out<Logic, C>) {
    buf(x, y);
    buf(x, y);
}";
    let e = Design::new(&format!("{PRELUDE}{two}"), "two").err().expect("refused");
    assert!(matches!(e, RefError::Unsupported(ref s) if s.contains("second driver")), "{e}");
    let undriven = "
#[hardware(combinational)]
fn buf(a: In<Logic, C>, o: Out<Logic, C>) { o.write(a.read()); }
#[hardware(structural)]
async fn und(clk: Clock<C>, x: In<Logic, C>, y: Out<Logic, C>) {
    let n = net::<Logic, C>();
    buf(n.1, y);
}";
    let e = Design::new(&format!("{PRELUDE}{undriven}"), "und").err().expect("refused");
    assert!(matches!(e, RefError::Unsupported(ref s) if s.contains("no driver")), "{e}");
}

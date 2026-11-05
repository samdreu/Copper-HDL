//! Hand-checked traces of small kernels under the one-resumption protocol — the
//! interpreter's own unit tests, each a sentence of SPEC §I.0–§I.9 made concrete.
//! The harness convention: cycle N drives `x_{N+1}`, then `resume`, `edge`,
//! `observe` = `obs_{N+1}`.

use copper_core::Logic;
use copper_ref::{Module, RefError, Value};
use std::collections::HashSet;

const PRELUDE: &str = "
struct C; impl ClockDomain for C {}
";

fn run(src: &str, module: &str, stim: &[&[(&str, &[Logic])]]) -> Vec<Vec<(String, Vec<Logic>)>> {
    let full = format!("{PRELUDE}{src}");
    let mut m = Module::new(&full, module, &[]).unwrap_or_else(|e| panic!("{module}: {e}"));
    let mut out = Vec::new();
    for (i, inputs) in stim.iter().enumerate() {
        m.resume_bits(inputs).unwrap_or_else(|e| panic!("{module} cycle {i}: {e}"));
        m.edge();
        out.push(m.observe_bits());
    }
    out
}

fn bits(v: u128, w: usize) -> Vec<Logic> {
    (0..w).map(|i| if (v >> i) & 1 == 1 { Logic::One } else { Logic::Zero }).collect()
}
const ZERO: &[Logic] = &[Logic::Zero];
const ONE: &[Logic] = &[Logic::One];

fn port(obs: &[(String, Vec<Logic>)], name: &str) -> u128 {
    let v = &obs.iter().find(|(n, _)| n == name).unwrap().1;
    v.iter().enumerate().fold(0u128, |acc, (i, b)| acc | ((*b == Logic::One) as u128) << i)
}

/// I.6.1 / I.6.4: a write before the tick in the resumption that ends at edge N+1
/// is shown from obs_{N+1}; I.4.1: the read sees x_{N+1}.
#[test]
fn registered_passthrough_shows_this_cycles_input_next_observation() {
    let src = "
#[hardware(sequential)]
async fn pass(clk: Clock<C>, x: In<Bits<8>, C>, q: RegOut<Bits<8>, C>) {
    loop { q.write(x.read()); clk.tick().await; }
}";
    let s3 = bits(3, 8);
    let s9 = bits(9, 8);
    let s200 = bits(200, 8);
    let obs = run(src, "pass", &[&[("x", &s3)], &[("x", &s9)], &[("x", &s200)]]);
    assert_eq!(port(&obs[0], "q"), 3);
    assert_eq!(port(&obs[1], "q"), 9);
    assert_eq!(port(&obs[2], "q"), 200);
}

/// I.0.2: the trailing region is the head of the NEXT resumption, which reads
/// x_{N+1} — `tick; r = x.read()` then `q.write(r)` shows x_{N+1} at obs_{N+1}.
#[test]
fn trailing_read_belongs_to_the_next_resumption() {
    let src = "
#[hardware(sequential)]
async fn tr(clk: Clock<C>, x: In<Bits<8>, C>, q: RegOut<Bits<8>, C>) {
    let mut r: Bits<8> = Bits::zero();
    loop { q.write(r); clk.tick().await; r = x.read(); }
}";
    let (a, b, c) = (bits(5, 8), bits(6, 8), bits(7, 8));
    let obs = run(src, "tr", &[&[("x", &a)], &[("x", &b)], &[("x", &c)]]);
    // Resumption 1 (entry): q.write(0), tick → obs_1 = 0.
    assert_eq!(port(&obs[0], "q"), 0);
    // Resumption 2 reads x_2 = 6 into r, writes q = 6, ticks → obs_2 = 6.
    assert_eq!(port(&obs[1], "q"), 6);
    assert_eq!(port(&obs[2], "q"), 7);
}

/// I.6.3: a RegOut no write reaches holds (bsg_dff_en in its RegOut spelling).
#[test]
fn regout_holds_when_unwritten() {
    let src = "
#[hardware(sequential)]
async fn dff_en(clk: Clock<C>, d: In<Bits<8>, C>, en: In<Logic, C>, q: RegOut<Bits<8>, C>) {
    loop { if en.read() == Logic::One { q.write(d.read()); } clk.tick().await; }
}";
    let (a, b) = (bits(0x11, 8), bits(0x22, 8));
    let obs = run(src, "dff_en", &[&[("d", &a), ("en", ONE)], &[("d", &b), ("en", ZERO)], &[("d", &b), ("en", ONE)]]);
    assert_eq!(port(&obs[0], "q"), 0x11);
    assert_eq!(port(&obs[1], "q"), 0x11, "held");
    assert_eq!(port(&obs[2], "q"), 0x22);
}

/// I.7.5: sync_2ff in the one-job spelling — ff2 a wire, ff1 a register, q the
/// second flop: one destination cycle of latency beyond the input sample.
#[test]
fn sync_2ff_one_job_is_two_flops() {
    let src = "
#[hardware(synchronizer)]
async fn sync_2ff(clk: Clock<C>, d: In<Logic, C>, q: RegOut<Logic, C>) {
    let mut ff1 = Logic::Zero;
    let mut ff2 = Logic::Zero;
    loop { ff2 = ff1; ff1 = d.read(); q.write(ff2); clk.tick().await; }
}";
    let obs = run(src, "sync_2ff", &[&[("d", ONE)], &[("d", ZERO)], &[("d", ZERO)], &[("d", ZERO)]]);
    // Edge 1: ff1 = 1, q = old ff1 = 0. Edge 2: q = 1. Edge 3: q = 0.
    assert_eq!(port(&obs[0], "q"), 0);
    assert_eq!(port(&obs[1], "q"), 1);
    assert_eq!(port(&obs[2], "q"), 0);
    // The poison proposition: ff2 is dead at the tick (negative), ff1 is live (positive).
    let full = format!("{PRELUDE}{src}");
    let stim: Vec<Vec<(&str, &[Logic])>> = vec![vec![("d", ONE)], vec![("d", ZERO)], vec![("d", ZERO)]];
    let mut neg = Module::new(&full, "sync_2ff", &[]).unwrap().with_poison(HashSet::from(["ff2".to_string()]));
    for s in &stim {
        neg.resume_bits(s).expect("ff2 poisoned: no read before reassignment");
        neg.edge();
    }
    let mut pos = Module::new(&full, "sync_2ff", &[]).unwrap().with_poison(HashSet::from(["ff1".to_string()]));
    pos.resume_bits(&stim[0]).unwrap();
    pos.edge();
    let e = pos.resume_bits(&stim[1]).unwrap_err();
    assert_eq!(e, RefError::Poisoned("ff1".into()));
}

/// I.8.5 / I.8.4(b): a counted `for` with a tick; zero iterations when empty at
/// run time; the loop variable is a register across its body's tick.
#[test]
fn counted_for_with_a_tick_and_a_run_time_empty_range() {
    let src = "
#[hardware(sequential)]
async fn ser(clk: Clock<C>, w: In<Bits<4>, C>, cnt: In<Bits<4>, C>, tx: RegOut<Logic, C>, done: RegOut<Logic, C>) {
    loop {
        let word = w.read();
        let n: usize = cnt.read().as_usize();
        done.write(Logic::Zero);
        for i in 0..n { tx.write(word[i]); clk.tick().await; }
        done.write(Logic::One);
        clk.tick().await;
    }
}";
    let (w, n3, n0) = (bits(0b0101, 4), bits(3, 4), bits(0, 4));
    let obs = run(src, "ser", &[
        &[("w", &w), ("cnt", &n3)], // i=0: tx=1
        &[("w", &w), ("cnt", &n3)], // i=1: tx=0
        &[("w", &w), ("cnt", &n3)], // i=2: tx=1
        &[("w", &w), ("cnt", &n3)], // done=1
        &[("w", &w), ("cnt", &n0)], // zero iterations: done=1 straight away (no extra cycle)
        &[("w", &w), ("cnt", &n3)], // i=0 again
    ]);
    let tx: Vec<u128> = obs.iter().map(|o| port(o, "tx")).collect();
    let done: Vec<u128> = obs.iter().map(|o| port(o, "done")).collect();
    assert_eq!(tx, vec![1, 0, 1, 1, 1, 1]);
    assert_eq!(done, vec![0, 0, 0, 1, 1, 0]);
}

/// I.8.3 / I.8.4(a): `while` waits, `break` out of a nested loop, `continue` of
/// the module loop — det_010_awaits's shapes.
#[test]
fn wait_loops_break_and_continue() {
    let src = "
#[hardware(sequential)]
async fn hs(clk: Clock<C>, req: In<Logic, C>, ack: In<Logic, C>, done: RegOut<Bits<8>, C>) {
    let mut n: Bits<8> = Bits::zero();
    loop {
        done.write(n);
        while req.read() == Logic::Zero { clk.tick().await; }
        loop { if ack.read() == Logic::One { break; } clk.tick().await; }
        n = n + Bits::from_lit::<1>();
        if n == Bits::from_u8(2) { n = Bits::zero(); clk.tick().await; continue; }
        clk.tick().await;
    }
}";
    // From simple_handshake's coverage stream, model: Top(done=n) → WaitReq/WaitAck.
    let stim: &[&[(&str, &[Logic])]] = &[
        &[("req", ZERO), ("ack", ZERO)], // wait req
        &[("req", ONE), ("ack", ZERO)],  // req seen, wait ack
        &[("req", ZERO), ("ack", ONE)],  // ack: n=1, tick
        &[("req", ONE), ("ack", ONE)],   // top: done=1; both high: n=2 → reset to 0, continue
        &[("req", ZERO), ("ack", ZERO)], // top: done=0
    ];
    let obs = run(src, "hs", stim);
    let done: Vec<u128> = obs.iter().map(|o| port(o, "done")).collect();
    assert_eq!(done, vec![0, 0, 0, 1, 0]);
}

/// I.2.4: tuple patterns with bindings and guards, enum state, tuple assignment —
/// traffic_light's `(phase, timer) = match (phase, timer, req) { … }` shape.
#[test]
fn match_with_tuple_patterns_guards_and_enum_state() {
    let src = "
#[derive(Clone, Copy)]
enum Phase { Green, Yellow, Red }
#[hardware(sequential)]
async fn tl(clk: Clock<C>, request: In<Logic, C>, red: RegOut<Logic, C>) {
    let mut phase = Phase::Green;
    let mut timer: u8 = 0;
    loop {
        (phase, timer) = match (phase, timer, request.read()) {
            (Phase::Green, _, Logic::One) => (Phase::Yellow, 0),
            (Phase::Green, _, _) => (Phase::Green, 0),
            (Phase::Yellow, t, _) if t < 1 => (Phase::Yellow, t + 1),
            (Phase::Yellow, _, _) => (Phase::Red, 0),
            (Phase::Red, t, _) if t < 2 => (Phase::Red, t + 1),
            (Phase::Red, _, _) => (Phase::Green, 0),
        };
        red.write(if matches!(phase, Phase::Red) { Logic::One } else { Logic::Zero });
        clk.tick().await;
    }
}";
    let obs = run(src, "tl", &[&[("request", ONE)], &[("request", ZERO)], &[("request", ZERO)], &[("request", ZERO)], &[("request", ZERO)], &[("request", ZERO)], &[("request", ZERO)]]);
    let red: Vec<u128> = obs.iter().map(|o| port(o, "red")).collect();
    // Green→Yellow (t0), Yellow t1, Yellow→Red, Red t1, Red t2, Red→Green, Green.
    assert_eq!(red, vec![0, 0, 1, 1, 1, 0, 0]);
}

/// I.7.2(h): an array local written by element, read by element.
#[test]
fn array_register_file() {
    let src = "
#[hardware(sequential)]
async fn rf(clk: Clock<C>, we: In<Logic, C>, wa: In<Bits<2>, C>, wd: In<Bits<8>, C>, ra: In<Bits<2>, C>, o: RegOut<Bits<8>, C>) {
    let mut regs = [Bits::<8>::zero(); 4];
    loop {
        if we.read() == Logic::One { regs[wa.read().as_usize()] = wd.read(); }
        o.write(regs[ra.read().as_usize()]);
        clk.tick().await;
    }
}";
    let (a1, a2, d9, dz) = (bits(1, 2), bits(2, 2), bits(9, 8), bits(0, 8));
    let obs = run(src, "rf", &[
        &[("we", ONE), ("wa", &a1), ("wd", &d9), ("ra", &a1)],
        &[("we", ZERO), ("wa", &a2), ("wd", &dz), ("ra", &a1)],
        &[("we", ZERO), ("wa", &a2), ("wd", &dz), ("ra", &a2)],
    ]);
    let o: Vec<u128> = obs.iter().map(|x| port(x, "o")).collect();
    assert_eq!(o, vec![9, 9, 0]);
}

/// I.9.3: a pre-loop expression statement runs once, in the first resumption —
/// the §B probe. The write to 7 takes effect at edge 1 and is never re-run.
#[test]
fn preloop_statement_runs_once() {
    let src = "
#[hardware(sequential)]
async fn preloop_probe(clk: Clock<C>, go: In<Logic, C>, q: RegOut<Bits<8>, C>) {
    let mut cnt: Bits<8> = Bits::zero();
    q.write(Bits::from_u8(7));
    loop {
        cnt = cnt + Bits::from_u8(1);
        if go.read() == Logic::One {
            clk.tick().await;
            q.write(cnt);
        }
        clk.tick().await;
    }
}";
    let obs = run(src, "preloop_probe", &[&[("go", ZERO)], &[("go", ONE)], &[("go", ZERO)], &[("go", ZERO)], &[("go", ZERO)]]);
    let q: Vec<u128> = obs.iter().map(|o| port(o, "q")).collect();
    // obs_1 = 7 (pre-loop write); cycle 2 go=1: tick (q holds 7); cycle 3: q.write(cnt=2) → obs_3 = 2;
    // after that q holds 2 whatever the head does (no re-run of the 7).
    assert_eq!(q, vec![7, 7, 2, 2, 2]);
}

/// The refusals name their rows.
#[test]
fn refusals_name_their_spec_rows() {
    let plain_out = "
#[hardware(sequential)]
async fn po(clk: Clock<C>, x: In<Logic, C>, o: Out<Logic, C>) { loop { o.write(x.read()); clk.tick().await; } }";
    let e = Module::new(&format!("{PRELUDE}{plain_out}"), "po", &[]).unwrap_err();
    assert!(matches!(e, RefError::Deferred(ref s) if s.contains("D-8")), "{e}");

    let stmt_macro = "
#[hardware(sequential)]
async fn sm(clk: Clock<C>, x: In<Logic, C>, q: RegOut<Logic, C>) { loop { println!(\"{}\", 1); q.write(x.read()); clk.tick().await; } }";
    let e = Module::new(&format!("{PRELUDE}{stmt_macro}"), "sm", &[]).unwrap_err();
    assert!(matches!(e, RefError::Unsupported(ref s) if s.contains("R13")), "{e}");

    let div = "
#[hardware(sequential)]
async fn dv(clk: Clock<C>, x: In<Bits<8>, C>, q: RegOut<Bits<8>, C>) { loop { q.write(x.read() / Bits::from_u8(2)); clk.tick().await; } }";
    let mut m = Module::new(&format!("{PRELUDE}{div}"), "dv", &[]).unwrap();
    let e = m.resume(&[("x", Value::Lit(8))]).unwrap_err();
    assert!(matches!(e, RefError::Unsupported(ref s) if s.contains("R15")), "{e}");

    let comb = "
#[hardware(combinational)]
fn cb(x: In<Logic, C>, o: Out<Logic, C>) { o.write(x.read()); }";
    let e = Module::new(&format!("{PRELUDE}{comb}"), "cb", &[]).unwrap_err();
    assert!(matches!(e, RefError::Deferred(ref s) if s.contains("I.11")), "{e}");
}

/// The static tick-site set contains every reached site (the path conventions of
/// the walker and the collector agree).
#[test]
fn reached_sites_are_static_sites() {
    let src = "
#[hardware(sequential)]
async fn it(clk: Clock<C>, sel: In<Logic, C>, q: RegOut<Logic, C>) {
    loop {
        match sel.read() {
            Logic::One => { q.write(Logic::One); clk.tick().await; }
            _ => clk.tick().await,
        }
        if sel.read() == Logic::Zero { clk.tick().await; } else if sel.read() == Logic::One { clk.tick().await; }
        for _ in 0..2 { clk.tick().await; }
    }
}";
    let full = format!("{PRELUDE}{src}");
    let mut m = Module::new(&full, "it", &[]).unwrap();
    let sites = m.tick_sites();
    assert_eq!(sites.len(), 5, "{sites:?}");
    for i in 0..40 {
        let s = if i % 3 == 0 { ONE } else { ZERO };
        m.resume_bits(&[("sel", s)]).unwrap();
        m.edge();
    }
    assert!(m.reached_sites().is_subset(&sites), "{:?} ⊄ {:?}", m.reached_sites(), sites);
    assert_eq!(m.reached_sites().len(), 5, "{:?}", m.reached_sites());
}

/// A const generic as a width and a value; a file const folded through `/`.
#[test]
fn const_generics_and_file_consts() {
    let src = "
const CLKS: usize = 4;
const HALF: usize = CLKS / 2;
#[hardware(sequential)]
async fn g<const N: usize>(clk: Clock<C>, x: In<Bits<N>, C>, q: RegOut<Bits<N>, C>) {
    loop {
        let v = x.read();
        for _ in 0..HALF { clk.tick().await; }
        let mut acc: Bits<N> = Bits::zero();
        for i in 0..N { acc[i] = v[N - 1 - i]; }
        q.write(acc);
        clk.tick().await;
    }
}";
    let full = format!("{PRELUDE}{src}");
    let mut m = Module::new(&full, "g", &[("N", 4)]).unwrap();
    let x = bits(0b0001, 4);
    for _ in 0..3 {
        m.resume_bits(&[("x", &x)]).unwrap();
        m.edge();
    }
    assert_eq!(port(&m.observe_bits(), "q"), 0b1000);
}

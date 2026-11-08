//! The interpreter's memory model (SPEC §I.10, D-16) pinned by hand-derived traces:
//! each expected vector is worked from the clause it names, not from a run of the
//! simulator. Harness convention as `interp_smoke.rs`: cycle N drives `x_{N+1}`,
//! then `resume`, `edge`, `observe` = `obs_{N+1}`.

use copper_core::Logic;
use copper_ref::{Module, RefError};

const PRELUDE: &str = "
struct C; impl ClockDomain for C {}
";

fn bits(v: u128, w: usize) -> Vec<Logic> {
    (0..w).map(|i| if (v >> i) & 1 == 1 { Logic::One } else { Logic::Zero }).collect()
}

fn val(v: &[Logic]) -> u128 {
    v.iter().enumerate().fold(0u128, |acc, (i, b)| acc | ((*b == Logic::One) as u128) << i)
}

/// Run `module` with one input port per stimulus column, returning `q`'s value
/// after each edge.
fn run_q(src: &str, module: &str, cols: &[(&str, usize, &[u128])]) -> Result<Vec<u128>, RefError> {
    let full = format!("{PRELUDE}{src}");
    let mut m = Module::new(&full, module, &[]).unwrap_or_else(|e| panic!("{module}: {e}"));
    let n = cols[0].2.len();
    let mut out = Vec::new();
    for i in 0..n {
        let owned: Vec<(&str, Vec<Logic>)> = cols.iter().map(|(p, w, v)| (*p, bits(v[i], *w))).collect();
        let ins: Vec<(&str, &[Logic])> = owned.iter().map(|(p, b)| (*p, &b[..])).collect();
        m.resume_bits(&ins)?;
        m.edge();
        let obs = m.observe_bits();
        out.push(val(&obs.iter().find(|(n, _)| n == "q").unwrap().1));
    }
    Ok(out)
}

fn rom(lat: usize) -> String {
    format!(
        "
#[hardware(sequential)]
async fn rom_l{lat}(clk: Clock<C>, a: In<Bits<2>, C>, q: RegOut<Bits<8>, C>) {{
    let rom = Memory::<Bits<8>, 1, 0, C, {lat}, 1>::from_fn(clk.clone(), 4, |i| Bits::from_usize(i * 10 + 1));
    loop {{
        rom.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        if rom.read_port::<0>().is_ready() {{
            q.write(rom.read_port::<0>().data());
        }}
    }}
}}"
    )
}

/// I.10.1(d), I.10.3: the word staged in the resumption ending at edge N is
/// observed in the resumption ending at edge N+1 and, through the `RegOut`, shown
/// from obs_{N+1}: a = [2, 3, 1, 0] → obs = [init, rom[2], rom[3], rom[1]].
#[test]
fn latency_one_rom_shows_the_word_two_observations_after_its_staging() {
    let q = run_q(&rom(1), "rom_l1", &[("a", 2, &[2, 3, 1, 0])]).unwrap();
    assert_eq!(q, vec![0, 21, 31, 11]);
}

/// I.10.1(c): at L = 2 the word reaches the output stage at edge N+L; until the
/// pipeline fills `is_ready()` is false and the `RegOut` holds (I.6.3).
#[test]
fn latency_two_rom_is_one_observation_later() {
    let q = run_q(&rom(2), "rom_l2", &[("a", 2, &[2, 3, 1, 0])]).unwrap();
    assert_eq!(q, vec![0, 0, 21, 31]);
}

/// I.10.1(d), D-16 1A: `data()` with the output stage empty is a run-time failure.
#[test]
fn unready_data_is_a_runtime_failure() {
    let src = "
#[hardware(sequential)]
async fn unready(clk: Clock<C>, a: In<Bits<2>, C>, q: RegOut<Bits<8>, C>) {
    let rom = Memory::<Bits<8>, 1, 0, C, 1, 1>::new(clk.clone(), 4);
    loop {
        clk.tick().await;
        q.write(rom.read_port::<0>().data());
    }
}";
    let e = run_q(src, "unready", &[("a", 2, &[0, 0])]).unwrap_err();
    assert!(matches!(e, RefError::Runtime(ref s) if s.contains("undelivered")), "{e}");
}

/// I.10.1(b), I.14.6(b): an out-of-range address is a run-time failure at the staging.
#[test]
fn out_of_range_address_is_a_runtime_failure() {
    let src = "
#[hardware(sequential)]
async fn oor(clk: Clock<C>, a: In<Bits<3>, C>, q: RegOut<Bits<8>, C>) {
    let rom = Memory::<Bits<8>, 1, 0, C, 1, 1>::new(clk.clone(), 4);
    loop {
        rom.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        q.write(rom.read_port::<0>().data());
    }
}";
    let e = run_q(src, "oor", &[("a", 3, &[1, 5])]).unwrap_err();
    assert!(matches!(e, RefError::Runtime(ref s) if s.contains("out of range")), "{e}");
}

/// I.10.1(b), D-16 (b) B: of two stagings of one port in one resumption the last wins.
#[test]
fn last_staging_of_a_port_wins() {
    let src = "
#[hardware(sequential)]
async fn two_stagings(clk: Clock<C>, a: In<Bits<2>, C>, q: RegOut<Bits<8>, C>) {
    let rom = Memory::<Bits<8>, 1, 0, C, 1, 1>::from_contents(clk.clone(), vec![Bits::from_u8(7), Bits::from_u8(8), Bits::from_u8(9), Bits::from_u8(10)]);
    loop {
        rom.read_port::<0>().read(0);
        rom.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        if rom.read_port::<0>().is_ready() {
            q.write(rom.read_port::<0>().data());
        }
    }
}";
    let q = run_q(src, "two_stagings", &[("a", 2, &[3, 2, 1])]).unwrap();
    assert_eq!(q, vec![0, 10, 9]);
}

fn rdw(mode: &str, write_lat: usize) -> String {
    format!(
        "
#[hardware(sequential)]
async fn rdw(clk: Clock<C>, d: In<Bits<8>, C>, q: RegOut<Bits<8>, C>) {{
    let mem = Memory::<Bits<8>, 1, 1, C, 1, {write_lat}>::new(clk.clone(), 4){mode};
    loop {{
        mem.write_port::<0>().write(0, d.read());
        mem.read_port::<0>().read(0);
        clk.tick().await;
        if mem.read_port::<0>().is_ready() {{
            q.write(mem.read_port::<0>().data());
        }}
    }}
}}"
    )
}

/// I.10.2: one address written and read in one resumption. ReadFirst delivers the
/// word `A` held before the edge, WriteFirst the committed one. d = [5, 6, 7, 8]:
/// ReadFirst delivers 0, 5, 6, … at edges 1, 2, 3; WriteFirst 5, 6, 7.
#[test]
fn read_first_and_write_first_differ_at_a_collision() {
    let d: &[u128] = &[5, 6, 7, 8];
    assert_eq!(run_q(&rdw("", 1), "rdw", &[("d", 8, d)]).unwrap(), vec![0, 0, 5, 6]);
    assert_eq!(run_q(&rdw(".read_first()", 1), "rdw", &[("d", 8, d)]).unwrap(), vec![0, 0, 5, 6]);
    assert_eq!(run_q(&rdw(".write_first()", 1), "rdw", &[("d", 8, d)]).unwrap(), vec![0, 5, 6, 7]);
}

/// I.10.1(c): a write staged in the resumption ending at edge N+1 is in `A` from
/// edge N+M. At M = 2 (ReadFirst) the word written at resumption 1 is first read
/// at edge 3: delivered 0, 0, 5, 6 → obs = [0, 0, 0, 5, 6].
#[test]
fn write_latency_two_commits_at_edge_n_plus_m() {
    let q = run_q(&rdw("", 2), "rdw", &[("d", 8, &[5, 6, 7, 8, 9])]).unwrap();
    assert_eq!(q, vec![0, 0, 0, 5, 6]);
}

/// I.10.1(c): two write ports committing one address at one edge — the higher
/// index wins.
#[test]
fn higher_write_port_wins() {
    let src = "
#[hardware(sequential)]
async fn prio(clk: Clock<C>, d: In<Bits<8>, C>, q: RegOut<Bits<8>, C>) {
    let mem = Memory::<Bits<8>, 1, 2, C, 1, 1>::new(clk.clone(), 4);
    loop {
        mem.write_port::<1>().write(2, d.read() + Bits::from_u8(100));
        mem.write_port::<0>().write(2, d.read());
        mem.read_port::<0>().read(2);
        clk.tick().await;
        if mem.read_port::<0>().is_ready() {
            q.write(mem.read_port::<0>().data());
        }
    }
}";
    let q = run_q(src, "prio", &[("d", 8, &[1, 2, 3, 4])]).unwrap();
    assert_eq!(q, vec![0, 0, 101, 102]);
}

/// I.10.1(a): a memory bound inside the loop is refused, and a `Memory` parameter
/// stays deferred (I.10.5: its storage is the owner's).
#[test]
fn memory_binding_outside_s_pre_and_memory_parameter_are_refused() {
    let in_loop = "
#[hardware(sequential)]
async fn inl(clk: Clock<C>, a: In<Bits<2>, C>, q: RegOut<Bits<8>, C>) {
    loop {
        let rom = Memory::<Bits<8>, 1, 0, C, 1, 1>::new(clk.clone(), 4);
        rom.read_port::<0>().read(a.read().as_usize());
        clk.tick().await;
        q.write(Bits::zero());
    }
}";
    let e = Module::new(&format!("{PRELUDE}{in_loop}"), "inl", &[]).unwrap_err();
    assert!(matches!(e, RefError::Unsupported(ref s) if s.contains("I.10.1(a)")), "{e}");
    let param = "
#[hardware(sequential)]
async fn par(clk: Clock<C>, m: Memory<Bits<8>, 1, 0, C, 1, 1>, q: RegOut<Bits<8>, C>) {
    loop { clk.tick().await; q.write(Bits::zero()); }
}";
    let e = Module::new(&format!("{PRELUDE}{param}"), "par", &[]).unwrap_err();
    assert!(matches!(e, RefError::Deferred(ref s) if s.contains("I.10.5")), "{e}");
}

/// I.10.6 (D-16 A8, D-25): a `combinational` module receives no `Memory` — the
/// interpreter refuses the header as the two front-ends do (`ui:fail/memory_combinational_*`).
/// Both shapes: an observe and a staging.
#[test]
fn a_memory_on_a_combinational_module_is_refused() {
    for (name, src) in [
        ("c_obs", "
#[hardware(combinational)]
fn c_obs(m: Memory<Bits<8>, 1, 0, C, 1, 1>, o: Out<Bits<8>, C>) { o.write(m.read_port::<0>().data()); }"),
        ("c_stage", "
#[hardware(combinational)]
fn c_stage(x: In<Bits<4>, C>, m: Memory<Bits<8>, 1, 0, C, 1, 1>, o: Out<Bits<4>, C>) {
    m.read_port::<0>().read(x.read().as_usize());
    o.write(x.read());
}"),
    ] {
        let e = Module::new(&format!("{PRELUDE}{src}"), name, &[]).unwrap_err();
        assert!(matches!(e, RefError::Unsupported(ref s) if s.contains("I.10.6")), "{name}: {e}");
    }
}

//! **The reference interpreter's four measurements** (SPEC Part III's first
//! evidence; `design_docs/review/MEASUREMENTS.md` §D), each predicted before it was
//! run and pinned here so it cannot drift:
//!
//! (i)   the pre-loop re-execution probe of §B and the two linear-route defects of
//!       SPEC I.9.4: interpreter ≡ simulator ≠ SV — "sim vs transpiler" becomes
//!       "spec vs transpiler" (the pre-loop pair, still pinned: D-20 step 5); the
//!       trailing fold at edge 1 and the one corpus disagreement the leg found,
//!       `regout_trailing_single_tick` — both AGREE on all three legs since Phase C
//!       step 4 removed the fold (2026-11-06, MEASUREMENTS §H);
//! (ii)  `pc` states of the extraction route vs the interpreter's tick sites, per
//!       corpus module — the many-to-one ratio of I.0.4;
//! (iii) the poison run for the register proposition (I.0.5(a), I.7.7): every local
//!       NOT in `infer_registers`' set overwritten at every tick; a read before
//!       reassignment is an under-inference;
//! (iv)  rustc's layout as a superset of I.7.1: `size_of_val` of the future.
//!
//! Reading (SPEC_OVERVIEW.md §7): the interpreter is the spec made executable.
//! sim ≠ ref with SV = ref → simulator/macro bug; sim = ref ≠ SV → transpiler bug or
//! a II.3 row; all differ → spec gap or interpreter bug.

mod common;

use common::{verilator_available, EquivalenceTest, Pattern, Rng};
use copper_core::port::{registered_wire, wire, In, RegOut};
use copper_core::{Bits, Clock, ClockDomain, Logic};
use copper_macros::hardware;
use copper_ref::{Module, RefError};
use copper_sim::HardwareExecutor;
use std::collections::{BTreeMap, BTreeSet, HashSet};

struct MainClk;
impl ClockDomain for MainClk {}

include!("ref_probes/startup_probes.rs");
const PROBES_SRC: &str = include_str!("ref_probes/startup_probes.rs");
include!("fixtures/out_phase_dut.rs");
const OUT_PHASE_SRC: &str = include_str!("fixtures/out_phase_dut.rs");
const SYNC_TICK_LAST_SRC: &str = include_str!("ref_probes/sync_2ff_tick_last.rs");

// Fixtures whose futures (iv) measures. One include per file; names must not
// collide across files.
include!("fixtures/counted_repetition_dut.rs");
include!("fixtures/cross_state_local_dut.rs");
include!("fixtures/mac_fsm_dut.rs");
include!("fixtures/uart_rx_dut.rs");
include!("fixtures/regout_multiphase_dut.rs");
include!("fixtures/handshake_dut.rs");

// ── (i) startup probes and the corpus disagreement ─────────────────────────

/// Drive a one-input (`Logic`), one-output (`Bits<8>`) probe for `$cycles` cycles
/// of seeded random stimulus through every leg; the verdicts.
macro_rules! run_go_q {
    ($name:literal, $src:expr, $ctor:path, $cycles:expr) => {{
        let mut eq = EquivalenceTest::differential_only($name, $src, Some($name)).with_reference_interpreter();
        let mut clk = Clock::<MainClk>::new();
        let mut exec = HardwareExecutor::new();
        let (go_drv, go_in) = wire::<Logic, MainClk>(Logic::Zero);
        let (q_out, q_obs) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
        let dh = q_out.dirty_handle();
        let reads = vec![go_in.wire_id()];
        exec.spawn_wired($ctor(clk.clone(), go_in, q_out), vec![dh], reads);
        let mut rng = Rng::new(0x5eed_2026_11_05);
        for i in 0..$cycles {
            // A quiet start: a first-cycle `go = 1` would mask the pre-loop write
            // (D-11's probe (ii): a head write on every path hides it).
            let go = if i < 4 { Logic::Zero } else { rng.logic() };
            go_drv.write(go);
            if i == 0 {
                // Observation 0 (SPEC §III.2.1): after the first resumption's settle,
                // before edge 1 — init(q) on every leg (I.9.4), PREDICTED to agree.
                exec.pre_edge_settle::<MainClk>();
                let q0 = q_obs.read();
                eq.record_observation_zero(&[("q", &q0[..])]);
                exec.post_edge_settle(&mut clk);
            } else {
                exec.tick_clock(&mut clk);
            }
            let q = q_obs.read();
            eq.record_differential(&[("go", std::slice::from_ref(&go))], &[("q", &q[..])]);
        }
        eq.finish_report()
    }};
}

/// §B probe (i) and D-11's (i-b): PREDICTED sim = ref ≠ SV for both. The
/// simulator and the interpreter both run the pre-loop write once; the extraction
/// route re-runs it, the linear route drops it.
#[test]
fn preloop_statement_is_spec_vs_transpiler_on_both_routes() {
    if !verilator_available() {
        panic!("Verilator is absent: this measurement needs the SV leg and cannot be skipped silently");
    }
    let runs = [
        ("preloop_probe", run_go_q!("preloop_probe", PROBES_SRC, preloop_probe, 64)),
        ("preloop_linear", run_go_q!("preloop_linear", PROBES_SRC, preloop_linear, 64)),
    ];
    for (name, v) in runs {
        println!("{name}: transpiled {} interpreter {:?}", v.transpiled, v.interpreter);
        assert!(
            matches!(v.interpreter, Some(Ok(64))),
            "{name}: the interpreter was predicted to agree with the simulator: {:?}",
            v.interpreter
        );
        assert!(!v.transpiled, "{name}: the transpiled SV was predicted to diverge (II.3 rows (f2)/(f3))");
        // SPEC §III.2.8: sim = ref ≠ SV — a transpiler bug or a II.3 row (here (f2)/(f3)).
        assert_eq!(v.pattern, Some(Pattern::SimEqRefNeSv), "{name}: {}", v.report);
        v.record_pin(&format!("{name}: II.3 row {} (P15), the Phase C entry fixes it", if name == "preloop_probe" { "(f2)" } else { "(f3)" }));
    }
}

/// D-11's (iv), II.3 row (f4): `let r = 5; loop { tick; q.write(r) }`. Pinned
/// sim = ref ≠ SV at obs_1 only while the linear route folded the trailing region
/// into phase 0 at edge 1 (SV `obs_1 = 5`); since Phase C step 4 the trailing region
/// is the next resumption's head, guarded off at edge 1, and all three legs agree —
/// PREDICTED and measured (MEASUREMENTS §H).
#[test]
fn trailing_fold_at_edge_1_agrees_on_three_legs() {
    if !verilator_available() {
        panic!("Verilator is absent: this measurement needs the SV leg and cannot be skipped silently");
    }
    let mut eq = EquivalenceTest::differential_only("trailing_fold", PROBES_SRC, Some("trailing_fold"))
        .with_reference_interpreter();
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let (q_out, q_obs) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
    let dh = q_out.dirty_handle();
    exec.spawn_wired(trailing_fold(clk.clone(), q_out), vec![dh], vec![]);
    let mut first = None;
    for i in 0..16 {
        if i == 0 {
            exec.pre_edge_settle::<MainClk>();
            let q0 = q_obs.read();
            eq.record_observation_zero(&[("q", &q0[..])]);
            exec.post_edge_settle(&mut clk);
        } else {
            exec.tick_clock(&mut clk);
        }
        let q = q_obs.read();
        if i == 0 {
            first = Some(q.as_u8());
        }
        eq.record_differential(&[], &[("q", &q[..])]);
    }
    assert_eq!(first, Some(0), "the coroutine's obs_1 is init(q): nothing was written before edge 1");
    let v = eq.finish_report();
    println!("trailing_fold: transpiled {} interpreter {:?}", v.transpiled, v.interpreter);
    assert!(matches!(v.interpreter, Some(Ok(16))), "{:?}", v.interpreter);
    assert!(v.transpiled, "the SV shows init(q) at obs_1, as the coroutine does:\n{}", v.report);
    assert_eq!(v.pattern, None, "no disagreement:\n{}", v.report);
}

/// `regout_trailing_single_tick` (`loop { tick; q.write(acc + step.read()) … }`)
/// under the one-resumption executor: the simulator's trace, and the relation's
/// verdicts against the transpiled SV and the reference interpreter.
fn regout_trailing_single_tick_run() -> (Vec<u8>, Vec<u8>, common::Verdicts) {
    let mut eq = EquivalenceTest::differential_only(
        "regout_trailing_single_tick",
        OUT_PHASE_SRC,
        Some("regout_trailing_single_tick"),
    )
    .with_reference_interpreter();
    let mut clk = Clock::<MainClk>::new();
    let mut exec = HardwareExecutor::new();
    let (s_drv, s_in) = wire::<Bits<8>, MainClk>(Bits::zero());
    let (q_out, q_obs) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
    let dh = q_out.dirty_handle();
    let reads = vec![s_in.wire_id()];
    exec.spawn_wired(regout_trailing_single_tick(clk.clone(), s_in, q_out), vec![dh], reads);
    let mut rng = Rng::new(7);
    let (mut xs, mut qs) = (Vec::new(), Vec::new());
    for _ in 0..48 {
        let x = rng.u8();
        s_drv.write(Bits::from_u8(x));
        exec.tick_clock(&mut clk);
        let q = q_obs.read();
        let xb = Bits::<8>::from_u8(x);
        eq.record_differential(&[("step", &xb[..])], &[("q", &q[..])]);
        xs.push(x);
        qs.push(q.as_u8());
    }
    (xs, qs, eq.finish_report())
}

/// For each recorded cycle n (observation n+1), Σ of `xs[from..through(n)]`, wrapping.
fn prefix_sums(xs: &[u8], from: usize, through: impl Fn(usize) -> usize) -> Vec<u8> {
    (0..xs.len())
        .map(|n| xs[from.min(through(n))..through(n)].iter().fold(0u8, |a, &x| a.wrapping_add(x)))
        .collect()
}

/// P12: the simulator reads the trailing `step.read()` at t_{N+1}⁻
/// (obs_{N+1} = Σ x_2..x_{N+1}) and agrees with the reference interpreter. The
/// transpiled SV realized the two-job reading (the linear route's trailing fold)
/// until Phase C step 4 — pinned sim = ref ≠ SV from the flip (step 3), a `SKIP`
/// row in the sweep — and agrees since the trailing region lowers as the next
/// resumption's head (2026-11-06, MEASUREMENTS §H; the module now sweeps).
#[test]
fn regout_trailing_single_tick_agrees_on_three_legs() {
    if !verilator_available() {
        panic!("Verilator is absent: this measurement needs the SV leg and cannot be skipped silently");
    }
    let (xs, qs, v) = regout_trailing_single_tick_run();
    // The first resumption runs to the head tick and writes nothing (cycle 0's
    // input is read by no one); the resumption ending at edge N+1 adds x_{N+1}.
    assert_eq!(qs, prefix_sums(&xs, 1, |n| n + 1), "the one-job simulator shows Σ x_2..x_{{N+1}} at obs_{{N+1}}");
    println!("regout_trailing_single_tick (One): transpiled {} interpreter {:?}", v.transpiled, v.interpreter);
    assert!(matches!(v.interpreter, Some(Ok(48))), "sim ≡ ref under One: {:?}", v.interpreter);
    assert!(v.transpiled, "sim ≡ SV: the trailing region is the next resumption's head:\n{}", v.report);
    assert_eq!(v.pattern, None, "no disagreement:\n{}", v.report);
}

// ── corpus walk shared by (ii) and (iii) ───────────────────────────────────

fn corpus_files() -> Vec<std::path::PathBuf> {
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(d) else { return };
        for e in entries.filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "old") {
                    continue;
                }
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    walk(&root.join("tests/fixtures"), &mut files);
    walk(&root.join("examples"), &mut files);
    files.push(root.join("src/sync.rs"));
    files.sort();
    files
}

fn is_hardware(f: &syn::ItemFn) -> bool {
    copper_ref::module::hardware_mode(f).is_some()
}

/// The sweep's monomorphizations (build.rs `PARAMS`), mirrored for the generic
/// modules the interpreter can run.
fn params_for(name: &str) -> Vec<(&'static str, u64)> {
    match name {
        "det_for" => vec![("N", 3)],
        "shift_register" => vec![("N", 8), ("N_1", 7)],
        "wide_alu" => vec![("N", 32)],
        "rotate_right" | "priority_encode" => vec![("N", 8), ("N_LOG", 3)],
        "ripple_carry_adder" => vec![("N", 8)],
        "mux" => vec![("WIDTH_P", 8), ("ELS_P", 4), ("LG_ELS_LP", 2)],
        _ => vec![],
    }
}

fn random_inputs(m: &Module, rng: &mut Rng) -> Vec<(String, Vec<Logic>)> {
    m.input_ports()
        .into_iter()
        .map(|p| {
            let w = p.ty.bit_width().unwrap();
            (p.name, (0..w).map(|_| rng.logic()).collect())
        })
        .collect()
}

/// Drive `m` for `cycles` cycles of seeded stimulus; the trace of observations,
/// or the first error.
fn drive(m: &mut Module, seed: u64, cycles: usize) -> Result<Vec<Vec<(String, Vec<Logic>)>>, RefError> {
    let mut rng = Rng::new(seed);
    let mut trace = Vec::with_capacity(cycles);
    for _ in 0..cycles {
        let ins = random_inputs(m, &mut rng);
        let refs: Vec<(&str, &[Logic])> = ins.iter().map(|(n, v)| (n.as_str(), &v[..])).collect();
        m.resume_bits(&refs)?;
        m.edge();
        trace.push(m.observe_bits());
    }
    Ok(trace)
}

/// `pc` states of the extraction route for `f`, if the route fires (the same
/// pipeline `transpile_fir` runs up to `extract_control`): the arms of the
/// `match pc` less the `_` default.
fn pc_states(src: &str, f: &syn::ItemFn, hardware_fns: &HashSet<String>) -> Option<usize> {
    let _ = src;
    let mut fir = copper_codegen::capture_frontend_ir(f, hardware_fns).ok()?;
    copper_codegen::control_extract::desugar_tick_waits(&mut fir);
    copper_codegen::control_extract::desugar_counted_loops_in(&mut fir);
    copper_codegen::control_extract::extract_control(&mut fir);
    use copper_core::frontend_ir::{ExprType, RawStmtKind};
    let body = fir.raw_statements.iter().find_map(|s| match &s.kind {
        RawStmtKind::Expr(es) => match &es.expr {
            ExprType::Loop(l) => Some(&l.body),
            _ => None,
        },
        _ => None,
    })?;
    if body.len() != 2 {
        return None;
    }
    let RawStmtKind::Expr(es) = &body[0].kind else { return None };
    let ExprType::Match(m) = &es.expr else { return None };
    let is_pc = matches!(&*m.scrutinee, ExprType::Path(p) if p.path_text.starts_with("pc"));
    if !is_pc || m.arms.last().map(|a| a.pattern_text.as_str()) != Some("_") {
        return None;
    }
    Some(m.arms.len() - 1)
}

/// (ii) `pc` states vs tick sites, per corpus module. PREDICTED: on every
/// extracted module `pc` states ≥ tick sites + 1 (state 0 is the loop head, not a
/// site; a ticking branch's continuation is lowered once per arm), so the ratio is
/// ≥ 1 and many-to-one where branches tick; linear modules have no `pc`.
#[test]
fn pc_states_vs_tick_sites() {
    let mut rows: Vec<(String, usize, usize, Option<usize>, Option<usize>)> = Vec::new();
    for path in corpus_files() {
        let src = std::fs::read_to_string(&path).unwrap();
        let Ok(file) = syn::parse_file(&src) else { continue };
        let hardware_fns: HashSet<String> = file
            .items
            .iter()
            .filter_map(|i| match i {
                syn::Item::Fn(f) if is_hardware(f) => Some(f.sig.ident.to_string()),
                _ => None,
            })
            .collect();
        for item in &file.items {
            let syn::Item::Fn(f) = item else { continue };
            if !is_hardware(f) {
                continue;
            }
            let name = f.sig.ident.to_string();
            let sites = copper_ref::static_tick_sites(f).len();
            if sites == 0 {
                continue; // combinational
            }
            let states = pc_states(&src, f, &hardware_fns);
            let ps = params_for(&name);
            let reached = match Module::new(&src, &name, &ps) {
                Ok(_) => {
                    let mut union = BTreeSet::new();
                    let mut ok = true;
                    for seed in 0..4u64 {
                        let mut mm = Module::new(&src, &name, &ps).unwrap();
                        if drive(&mut mm, 0x1234 + seed, 200).is_err() {
                            ok = false;
                            break;
                        }
                        union.extend(mm.reached_sites().iter().cloned());
                    }
                    ok.then_some(union.len())
                }
                Err(_) => None,
            };
            let file_tag = path.file_stem().unwrap().to_string_lossy().to_string();
            rows.push((format!("{file_tag}::{name}"), sites, 0, states, reached));
        }
    }
    println!("\n(ii) pc states (extraction route) vs tick sites (interpreter), per module");
    println!("{:<48} {:>5} {:>9} {:>7} {:>6}", "module", "sites", "pc states", "ratio", "reached");
    let mut extracted = 0;
    let mut ratios: BTreeMap<String, usize> = BTreeMap::new();
    for (name, sites, _, states, reached) in &rows {
        let (st, ratio) = match states {
            Some(s) => (s.to_string(), format!("{:.2}", *s as f64 / *sites as f64)),
            None => ("linear".into(), "-".into()),
        };
        let r = match reached {
            Some(r) => r.to_string(),
            None => "-".into(),
        };
        println!("{name:<48} {sites:>5} {st:>9} {ratio:>7} {r:>6}");
        if let Some(s) = states {
            extracted += 1;
            assert!(*s >= 1, "{name}: an extracted module with no states");
            *ratios.entry(format!("{s}/{sites}")).or_insert(0) += 1;
        }
    }
    println!("{} modules with a tick; {extracted} extracted; ratio histogram (states/sites: modules): {ratios:?}", rows.len());
    assert!(rows.len() > 100, "the corpus walk stopped reaching the corpus: {}", rows.len());
    assert!(extracted > 20, "fewer extracted modules than the corpus has: {extracted}");
}

// ── (iii) the poison run ────────────────────────────────────────────────────

/// One poison run: the trace with every local in `targets` poisoned at every tick
/// against the clean trace, over 4 seeds × 200 cycles. `Ok(())` if identical;
/// `Err(local)` naming the first poisoned read or the first trace difference.
fn poison_run(src: &str, name: &str, ps: &[(&str, u64)], targets: &HashSet<String>) -> Result<(), String> {
    for seed in 0..4u64 {
        let mut clean = Module::new(src, name, ps).map_err(|e| e.to_string())?;
        let clean_trace = drive(&mut clean, 0x9e37 + seed, 200).map_err(|e| format!("clean run: {e}"))?;
        let mut poisoned = Module::new(src, name, ps).unwrap().with_poison(targets.clone());
        let got = drive(&mut poisoned, 0x9e37 + seed, 200);
        match got {
            Err(RefError::Poisoned(l)) => return Err(l),
            Err(e) => return Err(format!("poisoned run stopped: {e}")),
            Ok(t) => {
                if t != clean_trace {
                    return Err(format!("trace changed under poison (seed {seed}) — a poisoned value reached an output without a direct read"));
                }
            }
        }
    }
    Ok(())
}

/// (iii) The poison run over every leg-eligible module. PREDICTED flagged (the
/// nested-loop fold defect of I.7.5, "a local assigned before a ticking `for` and
/// read inside it after an interior tick is MISSED"): serializer/`word`,
/// uart_tx/`byte`, det_for/`ok` (both copies); nothing else. MEASURED: none —
/// see the note at the pinned set. The tick-last `sync_2ff` probe: `ff2` the first
/// negative (dead at the tick), `ff1` the first positive (forcing it into the
/// poison set changes the trace); `fold_defect` the first corpus-shaped positive.
#[test]
fn poison_run_for_the_register_proposition() {
    // The two first cases, by construction.
    {
        let f = syn::parse_file(SYNC_TICK_LAST_SRC).unwrap();
        let item = f.items.iter().find_map(|i| match i { syn::Item::Fn(f) => Some(f.clone()), _ => None }).unwrap();
        let regs: BTreeSet<String> = copper_analysis::infer_registers(&item).into_iter().collect();
        assert_eq!(regs, BTreeSet::from(["ff1".to_string()]), "I.7.5: the one-job sync_2ff infers {{ff1}}");
        let m = Module::new(SYNC_TICK_LAST_SRC, "sync_2ff_tick_last", &[]).unwrap();
        let assigned = m.assigned_in_loop();
        let targets: HashSet<String> = assigned.difference(&regs).cloned().collect();
        assert_eq!(targets, HashSet::from(["ff2".to_string()]));
        assert_eq!(poison_run(SYNC_TICK_LAST_SRC, "sync_2ff_tick_last", &[], &targets), Ok(()), "ff2 is dead at the tick: the first NEGATIVE");
        let forced: HashSet<String> = HashSet::from(["ff1".to_string(), "ff2".to_string()]);
        assert_eq!(poison_run(SYNC_TICK_LAST_SRC, "sync_2ff_tick_last", &[], &forced), Err("ff1".to_string()), "ff1 is live at the tick: the first POSITIVE");
    }

    // The fold defect of I.7.5, by construction: the inference misses `v`, the
    // poison run flags it (PREDICTED), so the corpus walk below has teeth.
    {
        let f = syn::parse_file(SYNC_TICK_LAST_SRC).unwrap();
        let item = f.items.iter().find_map(|i| match i { syn::Item::Fn(f) if f.sig.ident == "fold_defect" => Some(f.clone()), _ => None }).unwrap();
        let regs: BTreeSet<String> = copper_analysis::infer_registers(&item).into_iter().collect();
        println!("fold_defect: infer_registers = {regs:?}");
        assert!(!regs.contains("v"), "I.7.5: the fold misses `v` today (if this fails, the defect was fixed — update the row)");
        let m = Module::new(SYNC_TICK_LAST_SRC, "fold_defect", &[]).unwrap();
        let targets: HashSet<String> = m.assigned_in_loop().difference(&regs).cloned().collect();
        assert_eq!(poison_run(SYNC_TICK_LAST_SRC, "fold_defect", &[], &targets), Err("v".to_string()), "the poison run catches the fold's under-inference");
    }

    let mut flagged: BTreeMap<String, String> = BTreeMap::new();
    let mut clean: Vec<String> = Vec::new();
    let mut skipped = 0;
    for path in corpus_files() {
        let src = std::fs::read_to_string(&path).unwrap();
        let Ok(file) = syn::parse_file(&src) else { continue };
        for item in &file.items {
            let syn::Item::Fn(f) = item else { continue };
            if !is_hardware(f) {
                continue;
            }
            let name = f.sig.ident.to_string();
            let ps = params_for(&name);
            let m = match Module::new(&src, &name, &ps) {
                Ok(m) => m,
                Err(_) => {
                    skipped += 1;
                    continue;
                }
            };
            let regs: BTreeSet<String> = copper_analysis::infer_registers(f).into_iter().collect();
            let targets: HashSet<String> = m.assigned_in_loop().difference(&regs).cloned().collect();
            let tag = format!("{}::{name}", path.file_stem().unwrap().to_string_lossy());
            match poison_run(&src, &name, &ps, &targets) {
                Ok(()) => clean.push(tag),
                // The interpreter cannot run the module at all (a front-end gap
                // with its own REF_SKIP row, e.g. `Self { .. }`): not eligible,
                // not a register-inference finding.
                Err(l) if l.starts_with("clean run:") => skipped += 1,
                Err(l) => {
                    flagged.insert(tag, l);
                }
            }
        }
    }
    println!("\n(iii) poison run: {} modules clean, {} flagged, {skipped} not eligible", clean.len(), flagged.len());
    for (m, l) in &flagged {
        println!("  FLAGGED {m}: `{l}` read after a tick before reassignment, not in infer_registers");
    }
    // The measured set, pinned: EMPTY. The prediction (MEASUREMENTS.md §D) named
    // serializer/`word`, uart_tx/`byte` and det_for/`ok` as I.7.5's fold defect and
    // was wrong: each of those is live out of a tick node that precedes its `for`
    // (serializer's start-bit tick, uart_tx's start-bit `for`, det_for's own fold
    // node whose out-edge is a tick and whose uses include `ok`), so backward
    // liveness carries it. The defect needs the read to be reachable ONLY through
    // the fold node's entry with no tick in between — `fold_defect` above — and no
    // corpus module has that shape (I.7.5: "no fixture has the shape").
    let expected: BTreeMap<String, String> = BTreeMap::new();
    assert_eq!(flagged, expected, "the poison run's flagged set moved");
    assert!(clean.len() >= 40, "{} clean modules — the walk stopped reaching the corpus", clean.len());
}

// ── (iv) rustc's layout as a superset ───────────────────────────────────────

/// (iv) `size_of_val` of each module's future, beside its inferred register set.
/// The future holds every local live across an await (rustc's lowering keeps a
/// superset of I.7.1's registers, I.0.5(c)); its size is a bound the registers'
/// storage cannot exceed. `-Zprint-type-sizes` gives the per-variant layout — see
/// MEASUREMENTS.md §D for the one-off run.
#[test]
fn future_size_bounds_the_register_storage() {
    let clk = Clock::<MainClk>::new();
    let mut rows: Vec<(&str, usize, Vec<String>, &str)> = Vec::new();
    macro_rules! row {
        ($name:literal, $src:literal, $fut:expr) => {{
            let f = syn::parse_file(include_str!($src)).unwrap();
            let item = f
                .items
                .iter()
                .find_map(|i| match i {
                    syn::Item::Fn(f) if f.sig.ident == $name => Some(f.clone()),
                    _ => None,
                })
                .unwrap();
            let regs = copper_analysis::infer_registers(&item);
            rows.push(($name, std::mem::size_of_val(&$fut), regs, $src));
        }};
    }
    row!("counted_repetition", "fixtures/counted_repetition_dut.rs", {
        let (_, d) = wire::<Bits<8>, MainClk>(Bits::zero());
        let (s, _) = registered_wire::<Logic, MainClk>(&clk, Logic::Zero);
        counted_repetition(clk.clone(), d, s)
    });
    row!("capture_after_wait", "fixtures/cross_state_local_dut.rs", {
        let (_, go) = wire::<Logic, MainClk>(Logic::Zero);
        let (_, d) = wire::<Bits<8>, MainClk>(Bits::zero());
        let (o, _) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
        capture_after_wait(clk.clone(), go, d, o)
    });
    row!("mac_fsm", "fixtures/mac_fsm_dut.rs", {
        let (_, a) = wire::<Bits<8>, MainClk>(Bits::zero());
        let (_, b) = wire::<Bits<8>, MainClk>(Bits::zero());
        let (_, c) = wire::<Bits<8>, MainClk>(Bits::zero());
        let (o, _) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
        mac_fsm(clk.clone(), a, b, c, o)
    });
    row!("uart_rx_dut", "fixtures/uart_rx_dut.rs", {
        let (_, s) = wire::<Logic, MainClk>(Logic::One);
        let (dv, _) = registered_wire::<Logic, MainClk>(&clk, Logic::Zero);
        let (b, _) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
        uart_rx_dut(clk.clone(), s, dv, b)
    });
    row!("regout_multiphase", "fixtures/regout_multiphase_dut.rs", {
        let (_, d) = wire::<Bits<4>, MainClk>(Bits::zero());
        let (w, _) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
        regout_multiphase(clk.clone(), d, w)
    });
    row!("handshake", "fixtures/handshake_dut.rs", {
        let (_, r) = wire::<Logic, MainClk>(Logic::Zero);
        let (_, a) = wire::<Logic, MainClk>(Logic::Zero);
        let (d, _) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
        handshake(clk.clone(), r, a, d)
    });
    row!("regout_trailing_single_tick", "fixtures/out_phase_dut.rs", {
        let (_, s) = wire::<Bits<8>, MainClk>(Bits::zero());
        let (q, _) = registered_wire::<Bits<8>, MainClk>(&clk, Bits::zero());
        regout_trailing_single_tick(clk.clone(), s, q)
    });
    println!("\n(iv) size_of_val of the module future vs the inferred registers");
    println!("{:<30} {:>6}  registers", "module", "bytes");
    for (name, bytes, regs, _) in &rows {
        println!("{name:<30} {bytes:>6}  {}", regs.join(", "));
        // Every future carries at least its captured ports (Arc handles, 8 bytes
        // each) plus a discriminant: a bound that is never zero.
        assert!(*bytes >= 16, "{name}: {bytes} bytes");
    }
}

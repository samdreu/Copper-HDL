//! Front-end parity (2026-10-23): the `#[hardware]` macro and the transpiler
//! enforce the SAME rule set, so a module that simulates also transpiles, or is
//! refused by both.
//!
//! Until this landed the two front-ends refused different things. The pre-tick
//! alignment family ran in the macro only; the memory-staging rules, the
//! plain-`Out`-from-a-read-result rule, the nested-loop ordering and the foreign
//! `await` rejection ran in the transpiler only. A module could therefore compile,
//! simulate, and be refused at transpile time — the transpiler's refusal set was
//! the de-facto language, and two fixtures (`tick_first_waiter`, `mp_rom`) lived in
//! the corpus as sim-only modules with `SKIP` rows saying "refused by design".
//!
//! This test is the standing audit, in both directions:
//!
//! * **transpiler-only rules over the corpus.** Every clocked `#[hardware]` fn in
//!   the tree already compiles through the macro, so every one must pass the rules
//!   the macro now runs (the former transpiler-only set). A module here is one the
//!   build would refuse — the expected set is empty by construction, and the test
//!   names the offender rather than letting `cargo build` do it somewhere else.
//! * **macro rules over the corpus.** Every module a macro-side rule flags must
//!   carry `allow_pretick_alignment` (the opt-out both front-ends honour), or the
//!   transpiler would refuse a module the macro accepted. `multi_write_collapse`
//!   has no opt-out in either front-end, so its flagged set must be empty.
//!
//! The witnesses for the refused shapes are `copper-macros/tests/ui/fail/` cases
//! (`tick_first_wait.rs`, `memory_result_drives_plain_out.rs`, `foreign_await.rs`),
//! not corpus modules: a module both front-ends refuse cannot be a swept module.
//!
//! The rule list below is the one both front-ends run; its seventh entry, the
//! linear-route anchor rule, was restated by form and freed of its register
//! requirement on 2026-11-02 (`CYCLE_DATAFLOW_SEMANTICS.md` §10.8) and the
//! trailing-`Out` rule gained a guard clause the same day — both in
//! `copper-analysis` alone, so the audit's list did not change.
//!
//! The audit scans clocked modules. The combinational rule (definite assignment)
//! became shared on 2026-11-02 (§10.12): the transpiler refused a default-then-
//! override the macro admitted. Its two-direction pin is
//! `copper-codegen/tests/comb_definite_assignment_parity.rs`, and the sweep
//! transpiles every combinational module the macro compiles.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use syn::{Item, ItemFn};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target" || n == "obj_dir") {
                continue;
            }
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// `(mode, opted_out)` from `#[hardware(<mode>, <flags>…)]`, reading the first token
/// rather than `parse_args::<Ident>()`, which fails once a flag is present.
fn hardware_attr(f: &ItemFn) -> Option<(String, bool)> {
    f.attrs.iter().find_map(|a| {
        if !a.path().segments.last().is_some_and(|s| s.ident == "hardware") {
            return None;
        }
        let text = a.meta.require_list().ok()?.tokens.to_string();
        let mode = text.split(',').next()?.trim().to_string();
        Some((mode, text.contains("allow_pretick_alignment")))
    })
}

fn clocked_hardware_fns(items: &[Item], out: &mut Vec<(ItemFn, bool)>) {
    for item in items {
        match item {
            Item::Fn(f) => {
                if let Some((mode, opted_out)) = hardware_attr(f) {
                    if mode == "sequential" || mode == "synchronizer" {
                        out.push((f.clone(), opted_out));
                    }
                }
            }
            Item::Mod(m) => {
                if let Some((_, items)) = &m.content {
                    clocked_hardware_fns(items, out);
                }
            }
            _ => {}
        }
    }
}

fn corpus() -> Vec<(String, ItemFn, bool)> {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["examples", "src", "tests"] {
        rs_files(&root.join(dir), &mut files);
    }
    files.sort();
    let mut out = Vec::new();
    for path in &files {
        let Ok(src) = fs::read_to_string(path) else { continue };
        let Ok(file) = syn::parse_file(&src) else { continue };
        let mut fns = Vec::new();
        clocked_hardware_fns(&file.items, &mut fns);
        let rel = path.strip_prefix(&root).unwrap_or(path).display().to_string();
        for (f, opted_out) in fns {
            out.push((format!("{rel}::{}", f.sig.ident), f, opted_out));
        }
    }
    out
}

#[test]
fn every_corpus_module_passes_the_rules_the_transpiler_alone_used_to_enforce() {
    let mods = corpus();
    assert!(mods.len() >= 40, "expected to scan the corpus, only saw {} clocked modules", mods.len());

    let mut failing: BTreeSet<String> = BTreeSet::new();
    for (name, f, _) in &mods {
        let mut why = Vec::new();
        if let Err(e) = copper_analysis::check_nested_loop_shape(f) {
            why.push(format!("nested-loop shape: {e}"));
        }
        if let Err(e) = copper_analysis::check_awaits(f) {
            why.push(format!("await: {e}"));
        }
        if let Err(e) = copper_analysis::check_no_preloop_tick(f) {
            why.push(format!("R21 (tick before the module loop): {e}"));
        }
        if let Err(e) = copper_analysis::check_counted_for_nesting(f) {
            why.push(format!("run-time-empty counted `for` ending an enclosing one: {e}"));
        }
        if let Err(e) = copper_analysis::check_register_initializers(f) {
            why.push(format!("register power-on value: {e}"));
        }
        if let Err(e) = copper_analysis::check_memory_staging(f) {
            why.push(format!("memory staging: {e}"));
        }
        if let Err(e) = copper_analysis::check_clocked_outputs(f) {
            why.push(format!("R8 (plain Out on a clocked module): {e}"));
        }
        if !why.is_empty() {
            failing.insert(format!("{name} — {}", why.join("; ")));
        }
    }
    assert!(
        failing.is_empty(),
        "these corpus modules are refused by a rule the macro now enforces, so they \
         cannot compile; either the module is a refused shape (move it to a \
         copper-macros/tests/ui/fail case) or a rule regressed into a false positive: \
         {failing:#?}"
    );
}

/// Phase C step 3 (D-8, 2026-11-06): the pre-tick alignment family and
/// `multi_write_collapse` are retired (SPEC II.3.2), and with them the
/// `allow_pretick_alignment` opt-out this test used to audit. What replaces it is
/// R8, which test 1 runs over the corpus; this test pins that the opt-out is gone
/// from every module in it.
#[test]
fn no_corpus_module_carries_the_retired_opt_out() {
    let mods = corpus();
    let opted: Vec<&String> = mods.iter().filter(|(_, _, o)| *o).map(|(n, _, _)| n).collect();
    assert!(
        opted.is_empty(),
        "`allow_pretick_alignment` was retired with the pre-tick alignment rules (D-8); \
         the macro now refuses it, so these modules cannot compile: {opted:?}"
    );
}

/// Every `#[hardware]` fn of the corpus, in every mode (the step-8 data rules apply
/// to combinational modules too).
fn all_hardware_fns() -> Vec<(String, ItemFn)> {
    fn walk(items: &[Item], out: &mut Vec<ItemFn>) {
        for item in items {
            match item {
                Item::Fn(f) if hardware_attr(f).is_some() => out.push(f.clone()),
                Item::Mod(m) => {
                    if let Some((_, items)) = &m.content {
                        walk(items, out);
                    }
                }
                _ => {}
            }
        }
    }
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["examples", "src", "tests"] {
        rs_files(&root.join(dir), &mut files);
    }
    files.sort();
    let mut out = Vec::new();
    for path in &files {
        let Ok(src) = fs::read_to_string(path) else { continue };
        let Ok(file) = syn::parse_file(&src) else { continue };
        let mut fns = Vec::new();
        walk(&file.items, &mut fns);
        let rel = path.strip_prefix(&root).unwrap_or(path).display().to_string();
        for f in fns {
            out.push((format!("{rel}::{}", f.sig.ident), f));
        }
    }
    out
}

/// Phase C step 8 (SPEC I.14.7 (7), (9); MEASUREMENTS §M): `checked_*` / `is_valid()`
/// OUTSIDE and the memory port index below the port count, both front-ends, one
/// function each (`check_outside_methods`, `check_memory_port_index`). The corpus
/// passes both (it compiles through the macro, which runs them); the refused shapes'
/// witnesses are `copper-macros/tests/ui/fail/{outside_checked_add,outside_is_valid,
/// memory_port_index}.rs` and, for the transpiler, `copper-codegen/tests/
/// unsupported_constructs.rs`.
#[test]
fn every_corpus_module_passes_the_step_8_data_rules() {
    let mods = all_hardware_fns();
    assert!(mods.len() >= 200, "expected every #[hardware] fn, saw {}", mods.len());
    let mut failing = BTreeSet::new();
    for (name, f) in &mods {
        if let Err(e) = copper_analysis::check_outside_methods(f) {
            failing.insert(format!("{name} — OUTSIDE method: {e}"));
        }
        if let Err(e) = copper_analysis::check_memory_port_index(f) {
            failing.insert(format!("{name} — memory port index: {e}"));
        }
    }
    assert!(failing.is_empty(), "{failing:#?}");
}

#[test]
fn the_step_8_data_rules_refuse_their_witnesses_and_admit_their_twins() {
    let parse = |s: &str| syn::parse_str::<ItemFn>(s).expect("witness parses");
    let refused = [
        ("checked_add", "async fn m(clk: Clock<C>, a: In<u8, C>, q: RegOut<u8, C>) { loop { q.write(a.read().checked_add(1).unwrap_or(0)); clk.tick().await; } }"),
        ("is_valid", "async fn m(clk: Clock<C>, a: In<Bits<8>, C>, q: RegOut<Logic, C>) { loop { q.write(if a.read().is_valid() { Logic::One } else { Logic::Zero }); clk.tick().await; } }"),
        ("read_port index", "async fn m(clk: Clock<C>, q: RegOut<Bits<8>, C>) { let mem = Memory::<Bits<8>, 1, 1, C, 1, 1>::new(clk.clone(), 4); loop { mem.read_port::<1>().read(0); q.write(mem.read_port::<0>().data()); clk.tick().await; } }"),
        ("write_port index", "async fn m(clk: Clock<C>, mem: Memory<Bits<8>, 2, 1, C, 1, 1>) { loop { mem.write_port::<1>().write(0, Bits::zero()); clk.tick().await; } }"),
    ];
    for (what, src) in refused {
        let f = parse(src);
        let r = copper_analysis::check_outside_methods(&f).and(copper_analysis::check_memory_port_index(&f));
        assert!(r.is_err(), "{what}: expected a refusal");
    }
    let admitted = [
        "async fn m(clk: Clock<C>, a: In<u8, C>, q: RegOut<u8, C>) { loop { q.write(a.read().saturating_add(1)); clk.tick().await; } }",
        "async fn m(clk: Clock<C>, q: RegOut<Bits<8>, C>) { let mem = Memory::<Bits<8>, 2, 1, C, 1, 1>::new(clk.clone(), 4); loop { mem.read_port::<1>().read(0); q.write(mem.read_port::<0>().data()); clk.tick().await; } }",
        "async fn m<const K: usize>(clk: Clock<C>, mem: Memory<Bits<8>, K, 1, C, 1, 1>) { loop { mem.read_port::<3>().read(0); clk.tick().await; } }",
    ];
    for src in admitted {
        let f = parse(src);
        copper_analysis::check_outside_methods(&f).expect("admitted");
        copper_analysis::check_memory_port_index(&f).expect("admitted");
    }
}

/// R11 under D-21 A3 (2026-11-07, MEASUREMENTS §N). The macro has rustc judge a
/// register initializer (a `const { }` block); the transpiler runs the same syntactic
/// half (`check_register_initializers`: no run-time read) and a call half
/// (`check_register_initializer_calls_in`, rustc's E0015 — given the file's `const fn`s)
/// before its fold. Both halves are pinned here on witnesses and twins; the
/// end-to-end parity (the macro's ui cases transpiled) is
/// `copper-codegen/tests/register_init_parity.rs`.
#[test]
fn r11_halves_refuse_their_witnesses_and_admit_their_twins() {
    let parse = |body: &str| {
        syn::parse_str::<ItemFn>(&format!(
            "async fn m(clk: Clock<C>, a: In<Bits<8>, C>, o: RegOut<Bits<8>, C>) {{ {body} }}"
        ))
        .expect("witness parses")
    };
    let reg = |init: &str| format!("let mut r = {init}; loop {{ clk.tick().await; r = r + a.read(); o.write(r); }}");
    let file_const_fns = |p: &str| p == "seed_of" || p == "Cfg::twice" || p == "Latch::bubble";
    // (initializer, syntactic half refuses, call half refuses)
    let rows = [
        ("a.read()", true, false),
        ("Bits::from_u8(a.read().as_u8())", true, false),
        ("seed()", false, true),
        ("Other::bubble()", false, true),
        ("Bits::from_u8(seed_of(3))", false, false),
        ("Bits::from_u8(Cfg::twice(Cfg::K))", false, false),
        ("Latch::bubble()", false, false),
        ("Bits::zero()", false, false),
        ("SEED", false, false),
    ];
    for (init, syn_refuses, call_refuses) in rows {
        let f = parse(&reg(init));
        assert_eq!(copper_analysis::check_register_initializers(&f).is_err(), syn_refuses, "syntactic half, {init}");
        assert_eq!(
            copper_analysis::check_register_initializer_calls_in(&f, &file_const_fns).is_err(),
            call_refuses,
            "call half, {init}"
        );
    }
    // A wire is asked neither (C2): `w` is rewritten before every later read.
    let f = parse("let mut w = a.read(); loop { o.write(w); clk.tick().await; w = a.read(); }");
    assert!(copper_analysis::check_register_initializers(&f).is_ok());
    assert!(copper_analysis::check_register_initializer_calls_in(&f, &|_| false).is_ok());
}

/// D-21 H1 (decided 2026-11-07): no corpus module has a held pre-loop value (the rule
/// refuses it in both front-ends); the witness is ui/fail `held_preloop_read.rs`.
#[test]
fn no_corpus_module_holds_a_preloop_read() {
    let mods = all_hardware_fns();
    let mut held = BTreeSet::new();
    let mut looked = 0;
    for (name, f) in &mods {
        looked += 1;
        for (v, _) in copper_analysis::held_preloop_values(f) {
            held.insert(format!("{name}::{v}"));
        }
    }
    println!("held pre-loop values: {} modules scanned, {} flagged", looked, held.len());
    assert!(held.is_empty(), "{held:#?}");
}

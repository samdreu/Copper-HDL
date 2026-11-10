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
        ("checked_sub", "async fn m(clk: Clock<C>, a: In<u8, C>, q: RegOut<u8, C>) { loop { q.write(a.read().checked_sub(1).unwrap_or(0)); clk.tick().await; } }"),
        ("is_valid", "async fn m(clk: Clock<C>, a: In<Bits<8>, C>, q: RegOut<Logic, C>) { loop { q.write(if a.read().is_valid() { Logic::One } else { Logic::Zero }); clk.tick().await; } }"),
        ("read_port index", "async fn m(clk: Clock<C>, q: RegOut<Bits<8>, C>) { let mem = Memory::<Bits<8>, 1, 1, C, 1, 1>::new(clk.clone(), 4); loop { mem.read_port::<1>().read(0); q.write(mem.read_port::<0>().data()); clk.tick().await; } }"),
        ("write_port index", "async fn m(clk: Clock<C>, mem: Memory<Bits<8>, 2, 1, C, 1, 1>) { loop { mem.write_port::<1>().write(0, Bits::zero()); clk.tick().await; } }"),
        ("== X", "fn m(a: In<Logic, C>, q: Out<Logic, C>) { q.write(if a.read() == Logic::X { Logic::One } else { Logic::Zero }); }"),
        ("X pattern", "fn m(a: In<Logic, C>, q: Out<Logic, C>) { q.write(match a.read() { Logic::X => Logic::One, _ => Logic::Zero }); }"),
        ("X const pattern", "fn m(a: In<Logic, C>, q: Out<Logic, C>) { const U: Logic = Logic::X; q.write(match a.read() { U => Logic::One, _ => Logic::Zero }); }"),
        ("X in matches!", "fn m(a: In<Logic, C>, q: Out<Logic, C>) { q.write(if matches!(a.read(), Logic::X | Logic::Zero) { Logic::One } else { Logic::Zero }); }"),
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
        "fn m(a: In<Logic, C>, q: Out<Logic, C>) { q.write(match a.read() { Logic::One => Logic::Zero, _ => Logic::One }); }",
        "fn m(a: In<Logic, C>, q: Out<Logic, C>) { const U: Logic = Logic::One; q.write(match a.read() { U => Logic::Zero, u => u }); }",
        "fn m(a: In<Logic, C>, q: Out<Logic, C>) { q.write(a.read().eq_logic(Logic::X)); }",
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

/// SPEC II.1.3 / P10 (Phase C step 10, MEASUREMENTS §P): the audit's blind spots
/// closed in the corpus direction. Every `#[hardware]` fn of the tree, in every
/// mode, passes the shared syntactic rules both front-ends now run first
/// (`check_front_end_rules`: the header R20 with R9's flag and R10, R17, I.14.3's
/// OUTSIDE methods, the memory port index, D3, R16, R18, R7, R12, R13/R14, R5, the
/// CDC port rule, R19), R2/R3 (`check_reachability`) on a clocked body and D2
/// (`check_definite_assignment`) on a combinational one. The corpus compiles
/// through the macro, so a failure here is a rule that regressed into a false
/// positive, or a module the macro somehow admitted around it. The
/// transpiler-admits direction is `copper-codegen/tests/front_end_parity_ui.rs`.
#[test]
fn every_corpus_module_passes_every_shared_rule_in_its_mode() {
    let mods = every_hardware_fn_including_fn_bodies();
    assert!(mods.len() >= 250, "expected every #[hardware] fn, saw {}", mods.len());
    let mut failing = BTreeSet::new();
    let mut by_mode = std::collections::BTreeMap::<String, usize>::new();
    for (name, f) in &mods {
        let mode = match copper_analysis::hardware_mode(f) {
            Some(Ok(m)) => m,
            Some(Err(e)) => {
                failing.insert(format!("{name} — attribute: {e}"));
                continue;
            }
            None => unreachable!("all_hardware_fns returns attributed fns"),
        };
        *by_mode.entry(format!("{mode:?}")).or_default() += 1;
        if let Err(e) = copper_analysis::check_front_end_rules(f, mode) {
            failing.insert(format!("{name} — {e}"));
        }
        if mode.is_sequential_like() {
            if let Err(e) = copper_analysis::check_reachability(f) {
                failing.insert(format!("{name} — R2/R3: {e}"));
            }
        }
        if let Err(e) = copper_analysis::check_definite_assignment(f) {
            failing.insert(format!("{name} — D2: {e}"));
        }
    }
    println!("shared-rule audit: {} fns by mode {by_mode:?}", mods.len());
    assert!(failing.is_empty(), "{failing:#?}");
}

/// [`all_hardware_fns`] plus the `#[hardware]` fns declared inside a fn body (a
/// test that defines its module inline): the macro compiles those too.
fn every_hardware_fn_including_fn_bodies() -> Vec<(String, ItemFn)> {
    fn walk(items: &[Item], out: &mut Vec<ItemFn>) {
        for item in items {
            match item {
                Item::Fn(f) => {
                    if hardware_attr(f).is_some() {
                        out.push(f.clone());
                    }
                    let inner: Vec<Item> = f
                        .block
                        .stmts
                        .iter()
                        .filter_map(|s| match s {
                            syn::Stmt::Item(i) => Some(i.clone()),
                            _ => None,
                        })
                        .collect();
                    walk(&inner, out);
                }
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

/// Free `fn`s of one file (any `mod` depth) by name, and the free-fn names each calls.
fn helper_call_graph(items: &[Item]) -> (std::collections::BTreeMap<String, BTreeSet<String>>, Vec<ItemFn>) {
    use syn::visit::Visit;
    struct Calls(BTreeSet<String>);
    impl<'a> Visit<'a> for Calls {
        fn visit_expr_call(&mut self, c: &'a syn::ExprCall) {
            if let syn::Expr::Path(p) = &*c.func {
                if let Some(seg) = p.path.segments.last() {
                    self.0.insert(seg.ident.to_string());
                }
            }
            syn::visit::visit_expr_call(self, c);
        }
    }
    fn walk(items: &[Item], fns: &mut Vec<ItemFn>) {
        for item in items {
            match item {
                Item::Fn(f) => fns.push(f.clone()),
                Item::Mod(m) => {
                    if let Some((_, items)) = &m.content {
                        walk(items, fns);
                    }
                }
                _ => {}
            }
        }
    }
    let mut fns = Vec::new();
    walk(items, &mut fns);
    let names: BTreeSet<String> = fns.iter().map(|f| f.sig.ident.to_string()).collect();
    let mut graph = std::collections::BTreeMap::new();
    for f in &fns {
        let mut c = Calls(BTreeSet::new());
        c.visit_block(&f.block);
        graph.insert(f.sig.ident.to_string(), c.0.intersection(&names).cloned().collect());
    }
    let hardware = fns.into_iter().filter(|f| hardware_attr(f).is_some()).collect();
    (graph, hardware)
}

/// The `#[hardware]` fns of `items` that reach a helper `fn` lying on a call cycle.
fn modules_reaching_a_recursive_helper(items: &[Item]) -> Vec<String> {
    let (graph, hardware) = helper_call_graph(items);
    let reach = |from: &str| -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack: Vec<String> = graph.get(from).into_iter().flatten().cloned().collect();
        while let Some(n) = stack.pop() {
            if seen.insert(n.clone()) {
                stack.extend(graph.get(&n).into_iter().flatten().cloned());
            }
        }
        seen
    };
    hardware
        .iter()
        .filter(|m| reach(&m.sig.ident.to_string()).iter().any(|h| reach(h).contains(h)))
        .map(|m| m.sig.ident.to_string())
        .collect()
}

/// The behavioural `#[hardware]` fns of `items` whose body binds a call of a
/// `#[hardware]` fn of the same file in a `let` (`let _ = child(..)`, `let m =
/// child(..)`): II.1.3's second named exception (D-24 (1)). The file's `#[hardware]`
/// names (any depth, a test's fn-body modules too) stand for the transpiler's registry
/// (`check_no_behavioural_instance`).
fn modules_binding_a_hardware_call(items: &[Item]) -> Vec<String> {
    use syn::visit::Visit;
    struct All(Vec<ItemFn>);
    impl<'ast> Visit<'ast> for All {
        fn visit_item_fn(&mut self, f: &'ast ItemFn) {
            if hardware_attr(f).is_some() {
                self.0.push(f.clone());
            }
            syn::visit::visit_item_fn(self, f);
        }
    }
    let mut all = All(Vec::new());
    for i in items {
        all.visit_item(i);
    }
    let hardware = all.0;
    let names: BTreeSet<String> = hardware.iter().map(|f| f.sig.ident.to_string()).collect();
    struct Lets<'a>(&'a BTreeSet<String>, bool);
    impl<'ast> Visit<'ast> for Lets<'_> {
        fn visit_local(&mut self, l: &'ast syn::Local) {
            if let Some(init) = &l.init {
                if let syn::Expr::Call(c) = &*init.expr {
                    if let syn::Expr::Path(p) = &*c.func {
                        if p.path.segments.last().is_some_and(|s| self.0.contains(&s.ident.to_string())) {
                            self.1 = true;
                        }
                    }
                }
            }
            syn::visit::visit_local(self, l);
        }
        fn visit_item(&mut self, _: &'ast Item) {}
    }
    hardware
        .iter()
        .filter(|f| hardware_attr(f).is_some_and(|(mode, _)| mode != "structural"))
        .filter(|f| {
            let mut v = Lets(&names, false);
            v.visit_block(&f.block);
            v.1
        })
        .map(|f| f.sig.ident.to_string())
        .collect()
}

/// The `#[hardware]` modules whose body has a pattern naming a FILE-level `const` whose value
/// is X (`Logic::X`, `Bits::x()`, `Bits::<N>::x()`): an identifier pattern (rustc reads a const's
/// name in a pattern as the const) anywhere in a `match`, `if let`, `let` or closure parameter.
fn modules_matching_a_file_x_const(items: &[syn::Item]) -> Vec<String> {
    use syn::visit::Visit;
    fn is_x(e: &syn::Expr) -> bool {
        match e {
            syn::Expr::Path(p) => {
                let segs: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
                segs == ["Logic", "X"]
            }
            syn::Expr::Call(c) if c.args.is_empty() => match &*c.func {
                syn::Expr::Path(p) => {
                    let segs: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
                    segs.len() == 2 && segs[0] == "Bits" && segs[1] == "x"
                }
                _ => false,
            },
            _ => false,
        }
    }
    let xconsts: BTreeSet<String> = items
        .iter()
        .filter_map(|it| match it {
            syn::Item::Const(c) if is_x(&c.expr) => Some(c.ident.to_string()),
            _ => None,
        })
        .collect();
    if xconsts.is_empty() {
        return Vec::new();
    }
    struct P<'a>(&'a BTreeSet<String>, bool);
    impl<'ast> Visit<'ast> for P<'_> {
        fn visit_pat_ident(&mut self, p: &'ast syn::PatIdent) {
            if p.subpat.is_none() && self.0.contains(&p.ident.to_string()) {
                self.1 = true;
            }
            syn::visit::visit_pat_ident(self, p);
        }
        fn visit_pat(&mut self, p: &'ast syn::Pat) {
            if let syn::Pat::Path(pp) = p {
                if pp.path.get_ident().is_some_and(|i| self.0.contains(&i.to_string())) {
                    self.1 = true;
                }
            }
            syn::visit::visit_pat(self, p);
        }
    }
    items
        .iter()
        .filter_map(|it| match it {
            syn::Item::Fn(f) if hardware_attr(f).is_some() => Some(f),
            _ => None,
        })
        .filter(|f| {
            let mut v = P(&xconsts, false);
            v.visit_block(&f.block);
            v.1
        })
        .map(|f| f.sig.ident.to_string())
        .collect()
}

/// The integer-method probes of MEASUREMENTS §AI.3 (44 methods, `rotate_left` at a constant and
/// a run-time amount) and §AJ's additions: `(method, receiver type, arguments)`. In the probe
/// body `v` and `w` have the receiver type and `s` is a `u32` below 32.
const INT_METHOD_PROBES: &[(&str, &str, &str)] = &[
    ("count_ones", "u16", ""), ("count_zeros", "u16", ""), ("leading_zeros", "u16", ""),
    ("trailing_zeros", "u16", ""), ("count_ones", "i16", ""), ("leading_zeros", "i8", ""),
    ("leading_ones", "u16", ""), ("trailing_ones", "u16", ""), ("rotate_left", "u16", "3"),
    ("rotate_right", "u16", "3"), ("rotate_left", "u16", "s"), ("swap_bytes", "u16", ""),
    ("reverse_bits", "u16", ""), ("to_be", "u16", ""), ("to_le", "u16", ""), ("pow", "u16", "2"),
    ("wrapping_pow", "u16", "3"), ("saturating_pow", "u16", "3"), ("abs", "i16", ""),
    ("wrapping_abs", "i16", ""), ("unsigned_abs", "i16", ""), ("saturating_abs", "i16", ""),
    ("signum", "i16", ""), ("is_positive", "i16", ""), ("is_negative", "i16", ""),
    ("wrapping_neg", "u16", ""), ("saturating_neg", "i16", ""), ("is_power_of_two", "u16", ""),
    ("next_power_of_two", "u8", ""), ("wrapping_shl", "u16", "s"), ("wrapping_shr", "u16", "s"),
    ("wrapping_div", "u16", "3"), ("wrapping_rem", "u16", "3"), ("div_euclid", "u16", "3"),
    ("rem_euclid", "u16", "3"), ("abs_diff", "u16", "w"), ("min", "u16", "w"), ("max", "u16", "w"),
    ("clamp", "u16", "3, 9"), ("ilog2", "u16", ""), ("isqrt", "u16", ""), ("midpoint", "u16", "w"),
    ("wrapping_add", "u16", "w"), ("saturating_add", "u16", "w"),
    // §AJ: the rest of what `LOWERED_INT_METHODS` holds, and a few more of std's API.
    ("wrapping_sub", "u16", "w"), ("wrapping_mul", "u16", "w"), ("saturating_sub", "u16", "w"),
    ("saturating_mul", "u16", "w"), ("ilog10", "u16", ""), ("ilog", "u16", "3"),
    ("saturating_div", "u16", "3"),
    ("is_multiple_of", "u16", "3"), ("unbounded_shl", "u16", "s"), ("cast_signed", "u16", ""),
    ("min", "i16", "w"), ("max", "i16", "w"), ("abs_diff", "i16", "w"), ("rotate_right", "i16", "s"),
    ("wrapping_neg", "i16", ""), ("wrapping_shr", "i16", "s"), ("swap_bytes", "i32", ""),
    ("reverse_bits", "i8", ""),
];

/// One probe module: `let n = <call>;` written to a 16-bit port, sequential or combinational.
fn int_method_probe(method: &str, ty: &str, args: &str, sequential: bool, path_form: bool) -> String {
    let call = match (path_form, args.is_empty()) {
        (false, _) => format!("v.{method}({args})"),
        (true, true) => format!("{ty}::{method}(v)"),
        (true, false) => format!("{ty}::{method}(v, {args})"),
    };
    let body = format!(
        "let v: {ty} = (a.read().as_u16() | 1) as {ty}; let w: {ty} = b.read().as_u16() as {ty}; \
         let s: u32 = (b.read().as_u16() & 31) as u32; let n = {call}; \
         q.write(Bits::from_u16(n as u16)); let _ = (w, s);"
    );
    let head = "use copper_core::{Bits, Clock, ClockDomain}; use copper_core::port::{In, Out, RegOut}; \
                use copper_macros::hardware; struct A; impl ClockDomain for A {}";
    if sequential {
        format!("{head} #[hardware(sequential)] async fn m(clk: Clock<A>, a: In<Bits<16>, A>, b: In<Bits<16>, A>, q: RegOut<Bits<16>, A>) {{ loop {{ {body} clk.tick().await; }} }}")
    } else {
        format!("{head} #[hardware(combinational)] fn m<D: ClockDomain>(a: In<Bits<16>, D>, b: In<Bits<16>, D>, q: Out<Bits<16>, D>) {{ {body} }}")
    }
}

/// D-32: the transpiler admits every probe of a method on `copper_codegen::LOWERED_INT_METHODS`
/// (read from the code), refuses `to_be`/`to_le` with D-32 (2)'s message, and refuses every other
/// probe with the fourth exception's one message — `.`-form and path-call form, sequential and
/// combinational. Every listed method is probed, so the list cannot claim a lowering no probe ran.
fn int_methods_follow_the_lowered_list() {
    let lowered = copper_codegen::LOWERED_INT_METHODS;
    for m in lowered {
        assert!(INT_METHOD_PROBES.iter().any(|(p, _, _)| p == m), "lowered method `{m}` has no probe");
    }
    let cfg = copper_codegen::EmitConfig::default();
    let mut wrong = Vec::new();
    for &(m, ty, args) in INT_METHOD_PROBES {
        for sequential in [true, false] {
            for path_form in [false, true] {
                let src = int_method_probe(m, ty, args, sequential, path_form);
                let r = copper_codegen::transpile_source(&src, Some("m"), &cfg);
                let what = format!("{ty}::{m}({args}) seq={sequential} path={path_form}");
                match r {
                    Ok(_) if lowered.contains(&m) => {}
                    Err(e) if m == "to_be" || m == "to_le" => {
                        if !e.contains(copper_analysis::TARGET_BYTE_ORDER_MESSAGE) {
                            wrong.push(format!("{what}: not D-32 (2)'s message: {e}"));
                        }
                    }
                    Err(e) if !lowered.contains(&m) => {
                        let msg = format!("method `{m}` {}", copper_analysis::UNLOWERED_METHOD_MESSAGE);
                        if !e.contains(&msg) {
                            wrong.push(format!("{what}: not the fourth exception's message: {e}"));
                        }
                    }
                    Ok(_) => wrong.push(format!("{what}: transpiles, but `{m}` is not on LOWERED_INT_METHODS")),
                    Err(e) => wrong.push(format!("{what}: on LOWERED_INT_METHODS, refused: {e}")),
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// `#[hardware]` fns of one file (any `mod` depth) calling an integer method of
/// [`INT_METHOD_PROBES`] the transpiler does not lower — `.m(..)` or `<int>::m(..)`, type-blind
/// (the scan's false hits would show as unexpected witnesses). `to_be`/`to_le` are refused by
/// both front-ends and are not the exception's.
fn modules_calling_an_unlowered_int_method(items: &[Item]) -> Vec<String> {
    const PRIMS: [&str; 12] = ["u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize"];
    fn unlowered(m: &str) -> bool {
        INT_METHOD_PROBES.iter().any(|(p, _, _)| *p == m)
            && !copper_codegen::LOWERED_INT_METHODS.contains(&m)
            && m != "to_be"
            && m != "to_le"
    }
    struct V(bool);
    impl<'ast> syn::visit::Visit<'ast> for V {
        fn visit_expr_method_call(&mut self, mc: &'ast syn::ExprMethodCall) {
            self.0 |= unlowered(&mc.method.to_string());
            syn::visit::visit_expr_method_call(self, mc);
        }
        fn visit_expr_call(&mut self, c: &'ast syn::ExprCall) {
            if let syn::Expr::Path(p) = &*c.func {
                let segs: Vec<String> = p.path.segments.iter().map(|s| s.ident.to_string()).collect();
                if let [.., ty, m] = segs.as_slice() {
                    self.0 |= PRIMS.contains(&ty.as_str()) && unlowered(m);
                }
            }
            syn::visit::visit_expr_call(self, c);
        }
    }
    let mut fns = Vec::new();
    fn walk(items: &[Item], out: &mut Vec<ItemFn>) {
        for it in items {
            match it {
                Item::Fn(f) if f.attrs.iter().any(|a| a.path().is_ident("hardware")) => out.push(f.clone()),
                Item::Mod(m) => {
                    if let Some((_, inner)) = &m.content {
                        walk(inner, out);
                    }
                }
                _ => {}
            }
        }
    }
    walk(items, &mut fns);
    fns.into_iter()
        .filter(|f| {
            let mut v = V(false);
            syn::visit::Visit::visit_block(&mut v, &f.block);
            v.0
        })
        .map(|f| f.sig.ident.to_string())
        .collect()
}

/// SPEC II.1.3's four named exceptions, each found by a scan that sees what the macro
/// cannot, and each required to have exactly its witnesses, so none can widen unseen.
///
/// **R6** (D-22; MEASUREMENTS §Q): recursion through a helper `fn` compiles — the macro
/// holds the module's `ItemFn` only, never a helper's body (I.2.1), and rustc admits a
/// non-`async` recursion — and the transpiler refuses it when it inlines
/// (`RECURSIVE_HELPER_MESSAGE`). The scan reads each file's helper call graph.
///
/// **The let-bound instance** (D-24 (1); MEASUREMENTS §T): `let _ = child(..)` / `let m
/// = child(..)` in a behavioural body compiles — a binding is a use, so the denied
/// `#[must_use]` of D-23 (1) does not fire — and the transpiler refuses it with I.13.5's
/// message; the simulator's drop guard panics when the child is dropped unstarted. The
/// scan reads each behavioural body's `let`s against the file's `#[hardware]` names.
///
/// **The file-level X const in a pattern** (D-26 (2); MEASUREMENTS §U.11): `const UX: Logic =
/// Logic::X; … match v { UX => … }` compiles — the macro holds the `ItemFn` and cannot see a
/// file const's value — and the transpiler folds the value and refuses it with D-24's
/// `X_PATTERN_MESSAGE`; the simulator matches X structurally. The scan reads each file's X-valued
/// consts against the patterns of its `#[hardware]` bodies.
///
/// **An integer method the transpiler does not lower** (D-32 (1); MEASUREMENTS §AJ): rustc
/// type-checks `v.rotate_left(3)` and the macro sees no types, so it compiles and simulates with
/// Rust's meaning; the transpiler refuses it with ``method `<m>` `` and `UNLOWERED_METHOD_MESSAGE`,
/// never with text. Its extent is the complement of `copper_codegen::LOWERED_INT_METHODS`, read
/// from the code: [`int_methods_follow_the_lowered_list`] transpiles every probe of
/// [`INT_METHOD_PROBES`] in both spellings and both body kinds, and the scan requires
/// `tests/int_method_unlowered_d32.rs` to hold the only modules calling an unlowered one.
///
/// Any other module a scan finds simulates (or panics) and does not transpile — a parity break
/// to fix, not a fifth exception.
#[test]
fn the_four_named_parity_exceptions_have_exactly_their_witnesses() {
    int_methods_follow_the_lowered_list();
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["examples", "src", "tests"] {
        rs_files(&root.join(dir), &mut files);
    }
    files.sort();
    let mut found = BTreeSet::new();
    let mut bound = BTreeSet::new();
    let mut xmatch = BTreeSet::new();
    let mut unlowered = BTreeSet::new();
    for path in &files {
        let Ok(src) = fs::read_to_string(path) else { continue };
        let Ok(file) = syn::parse_file(&src) else { continue };
        let rel = path.strip_prefix(&root).unwrap_or(path).display().to_string();
        for m in modules_reaching_a_recursive_helper(&file.items) {
            found.insert(format!("{rel}::{m}"));
        }
        for m in modules_binding_a_hardware_call(&file.items) {
            bound.insert(format!("{rel}::{m}"));
        }
        for m in modules_matching_a_file_x_const(&file.items) {
            xmatch.insert(format!("{rel}::{m}"));
        }
        for m in modules_calling_an_unlowered_int_method(&file.items) {
            unlowered.insert(format!("{rel}::{m}"));
        }
    }
    let expected: BTreeSet<String> = [
        "tests/int_method_unlowered_d32.rs::um_comb_path".to_string(),
        "tests/int_method_unlowered_d32.rs::um_seq".to_string(),
    ]
    .into();
    assert_eq!(
        unlowered, expected,
        "the D-32 witnesses are the only modules calling an integer method the transpiler does not lower"
    );
    let ui = fs::read_to_string(root.join("copper-macros/tests/ui/pass/int_method_unlowered_d32.rs")).unwrap();
    assert_eq!(modules_calling_an_unlowered_int_method(&syn::parse_file(&ui).unwrap().items), vec!["int_method_unlowered"]);
    let expected: BTreeSet<String> = [
        "tests/file_x_const_pattern_d26.rs::fx_bits".to_string(),
        "tests/file_x_const_pattern_d26.rs::fx_logic".to_string(),
    ]
    .into();
    assert_eq!(xmatch, expected, "the file-X-const witnesses are the only modules matching an X-valued file const");
    let ui = fs::read_to_string(root.join("copper-macros/tests/ui/pass/file_x_const_pattern_d26.rs")).unwrap();
    assert_eq!(modules_matching_a_file_x_const(&syn::parse_file(&ui).unwrap().items), vec!["m"]);
    let xp = |src: &str| modules_matching_a_file_x_const(&syn::parse_file(src).unwrap().items);
    let body = "#[hardware(combinational)] fn m<D: ClockDomain>(a: In<Logic, D>, o: Out<Logic, D>) { match a.read() { K => o.write(Logic::One), _ => o.write(Logic::Zero) } }";
    assert_eq!(xp(&format!("const K: Logic = Logic::X; {body}")), vec!["m"]);
    assert_eq!(xp(&format!("const K: Bits<2> = Bits::<2>::x(); {body}")), vec!["m"]);
    assert!(xp(&format!("const K: Logic = Logic::One; {body}")).is_empty(), "a 2-state const is lowered");
    let expected: BTreeSet<String> = ["tests/recursive_helper_r6.rs::parity8".to_string()].into();
    assert_eq!(found, expected, "R6's witness is the one module reaching a recursive helper");
    let expected: BTreeSet<String> = [
        "tests/let_instance_d24.rs::parent_named".to_string(),
        "tests/let_instance_d24.rs::parent_wildcard".to_string(),
    ]
    .into();
    assert_eq!(bound, expected, "the let-instance witnesses are the only behavioural bodies binding a module call");
    let ui = fs::read_to_string(root.join("copper-macros/tests/ui/pass/let_instance_d24.rs")).unwrap();
    assert_eq!(modules_binding_a_hardware_call(&syn::parse_file(&ui).unwrap().items), vec!["parent"]);
    let lp = |src: &str| modules_binding_a_hardware_call(&syn::parse_file(src).unwrap().items);
    let c = "#[hardware(sequential)] async fn c(clk: Clock<A>) { loop { clk.tick().await; } }";
    assert_eq!(lp(&format!("{c} #[hardware(sequential)] async fn p(clk: Clock<A>) {{ let m = c(clk.clone()); loop {{ clk.tick().await; }} }}")), vec!["p"]);
    assert!(lp(&format!("fn h(x: u8) -> u8 {{ x }} {c} #[hardware(sequential)] async fn p(clk: Clock<A>) {{ let m = h(1); loop {{ clk.tick().await; }} }}")).is_empty());
    assert!(lp(&format!("{c} #[hardware(structural)] fn p(clk: Clock<A>) {{ c(clk.clone()); }}")).is_empty());

    // The scan sees the ui row too, and a direct, a mutual and an absent recursion.
    let ui = fs::read_to_string(root.join("copper-macros/tests/ui/pass/recursive_helper_r6.rs")).unwrap();
    assert_eq!(modules_reaching_a_recursive_helper(&syn::parse_file(&ui).unwrap().items), vec!["parity8"]);
    let probe = |src: &str| modules_reaching_a_recursive_helper(&syn::parse_file(src).unwrap().items);
    let m = "#[hardware(combinational)] fn m<D: ClockDomain>(a: In<u8, D>, o: Out<u8, D>) { o.write(f(a.read())); }";
    assert_eq!(probe(&format!("fn f(x: u8) -> u8 {{ g(x) }} fn g(x: u8) -> u8 {{ if x == 0 {{ 0 }} else {{ f(x - 1) }} }} {m}")), vec!["m"]);
    assert!(probe(&format!("fn f(x: u8) -> u8 {{ g(x) }} fn g(x: u8) -> u8 {{ x }} {m}")).is_empty());
}

/// The three rules of 2026-11-08 (MEASUREMENTS §V), each run by both front-ends from
/// `check_front_end_rules` / the shared CFG: a net in a behavioural body (I.13.5,
/// `check_no_body_nets`), `Bits<N>`'s width bound (R16, `check_admissible`), and a
/// ticking `for` over a constant-empty range as a boundary (R3 / I.8.4(b),
/// `empty_range_dependencies` — the macro has rustc check what the transpiler folds).
/// Witnesses refused, twins admitted; the ui/fail cases `behavioural_net`,
/// `bits_width_zero`, `bits_width_over_128`, `constant_empty_for_boundary` are the
/// macro's side, and `front_end_parity_ui.rs` transpiles each.
#[test]
fn the_2026_11_08_rules_refuse_their_witnesses_and_admit_their_twins() {
    let parse = |s: &str| syn::parse_str::<ItemFn>(s).unwrap();
    for (what, src) in [
        ("wire", "async fn m(clk: Clock<A>, q: RegOut<Logic, A>) { let (_d, _r) = wire::<Logic, A>(Logic::Zero); loop { q.write(Logic::One); clk.tick().await; } }"),
        ("registered_wire", "async fn m(clk: Clock<A>, q: RegOut<Logic, A>) { loop { let w = copper_sim::registered_wire::<Logic, A>(Logic::Zero); q.write(Logic::One); clk.tick().await; } }"),
        ("net", "fn m(a: In<Logic, A>, o: Out<Logic, A>) { let n = net::<Logic, A>(); o.write(a.read()); }"),
    ] {
        let e = copper_analysis::front_end::check_no_body_nets(&parse(src)).expect_err(what);
        assert_eq!(e.to_string(), copper_analysis::front_end::BODY_NET_MESSAGE, "{what}");
    }
    copper_analysis::front_end::check_no_body_nets(&parse(
        "async fn m(clk: Clock<A>, q: RegOut<Logic, A>) { let w = wire_count(3); loop { q.write(Logic::One); clk.tick().await; } }",
    ))
    .expect("a helper whose name only starts with `wire` is not a net");

    // D-26 (1): an array-register element write in a tick-free `for` bounded by a const
    // generic; its twins — a literal bound, a packed `[Logic; 4]` vector, a ticking `for`.
    let afg = |bound: &str, init: &str, ty: &str, tick: &str| {
        format!(
            "async fn m<const N: usize>(clk: Clock<A>, a: In<{ty}, A>, q: RegOut<{ty}, A>) {{ \
             let mut arr = [{init}; 4]; loop {{ q.write(arr[0]); \
             for j in 0..{bound} {{ arr[j] = a.read(); {tick} }} clk.tick().await; }} }}"
        )
    };
    let e = copper_analysis::front_end::check_array_write_in_generic_for(&parse(&afg("N", "Bits::<8>::zero()", "Bits<8>", "")))
        .expect_err("the generic-bound array write");
    assert_eq!(e.to_string(), copper_analysis::front_end::ARRAY_WRITE_GENERIC_FOR_MESSAGE);
    for (what, src) in [
        ("a literal bound", afg("4", "Bits::<8>::zero()", "Bits<8>", "")),
        ("a packed vector", afg("N", "Logic::Zero", "Logic", "")),
        ("a ticking for", afg("N", "Bits::<8>::zero()", "Bits<8>", "clk.tick().await;")),
    ] {
        copper_analysis::front_end::check_array_write_in_generic_for(&parse(&src)).expect(what);
    }

    for (src, n) in [
        ("fn m(a: In<Bits<0>, A>, o: Out<Bits<0>, A>) { o.write(a.read()); }", 0),
        ("fn m(a: In<Bits<8>, A>, o: Out<Bits<8>, A>) { let w: Bits<129> = Bits::zero(); o.write(a.read()); }", 129),
        ("fn m(a: In<Bits<8>, A>, o: Out<Bits<8>, A>) { let w = Bits::<{ 2 * 128 }>::zero(); o.write(a.read()); }", 256),
    ] {
        let e = copper_analysis::check_admissible(&parse(src)).expect_err(src);
        assert_eq!(e.to_string(), copper_analysis::bits_width_message(n), "{src}");
    }
    for src in [
        "fn m(a: In<Bits<1>, A>, o: Out<Bits<128>, A>) { o.write(Bits::zero()); }",
        "fn m<const N: usize>(a: In<Bits<N>, A>, o: Out<Bits<N>, A>) { o.write(a.read()); }",
        "fn m(a: In<Bits<W>, A>, o: Out<Bits<W>, A>) { o.write(a.read()); }",
    ] {
        copper_analysis::check_admissible(&parse(src)).expect(src);
    }

    // R3 over a constant range: the dependencies name the range whose emptiness the
    // verdict needs, none when the loop has a boundary of its own, and a binder is
    // never taken for a constant (a run-time bound stays the I.8.4 snapshot).
    let only = "async fn m(clk: Clock<A>, q: RegOut<u8, A>) { loop { q.write(1); for _ in 0..EMPTY { clk.tick().await; } } }";
    let deps = copper_analysis::empty_range_dependencies(&parse(only));
    assert_eq!(deps.iter().map(|s| s.iter().map(copper_analysis::range_key).collect::<Vec<_>>()).collect::<Vec<_>>(), vec![vec!["0 .. EMPTY".to_string()]]);
    let own = "async fn m(clk: Clock<A>, q: RegOut<u8, A>) { loop { q.write(1); for _ in 0..EMPTY { clk.tick().await; } clk.tick().await; } }";
    assert!(copper_analysis::empty_range_dependencies(&parse(own)).is_empty());
    let two = "async fn m(clk: Clock<A>, q: RegOut<u8, A>) { loop { for _ in 0..P { clk.tick().await; } for _ in 0..Q { clk.tick().await; } } }";
    let deps = copper_analysis::empty_range_dependencies(&parse(two));
    assert_eq!(deps.len(), 1, "one minimal set: both empty");
    assert_eq!(deps[0].len(), 2);
    let runtime = "async fn m(clk: Clock<A>, a: In<u8, A>, q: RegOut<u8, A>) { loop { let n = a.read(); for _ in 0..n { clk.tick().await; } } }";
    assert!(copper_analysis::constant_ticking_for_ranges(&parse(runtime)).is_empty());
    let generic = "async fn m<const N: usize>(clk: Clock<A>, q: RegOut<u8, A>) { loop { for _ in 0..N { clk.tick().await; } } }";
    assert!(copper_analysis::constant_ticking_for_ranges(&parse(generic)).is_empty());
    // The oracle: told the range is empty, R3 refuses with its message; non-empty, admits.
    let f = parse(only);
    let r = |empty: bool| {
        copper_analysis::with_empty_range_oracle(Box::new(move |_| Some(empty)), || copper_analysis::check_reachability(&f))
    };
    assert_eq!(r(true).unwrap_err().to_string(), copper_analysis::NESTED_EXIT_MESSAGE);
    r(false).expect("a non-empty constant range is a boundary");
}

/// The corpus side of the 2026-11-08 rules (MEASUREMENTS §V): no behavioural body
/// calls a net constructor, no `Bits` width is written outside 1..=128 (both
/// through `every_corpus_module_passes_every_shared_rule_in_its_mode`, which runs
/// `check_front_end_rules`), and here: how many clocked modules have a constant
/// ticking-`for` range, and how many depend on one being non-empty (each of those gets
/// the macro's rustc-checked assertion; each must hold, or the corpus would not compile).
#[test]
fn constant_range_boundaries_in_the_corpus() {
    let mods = every_hardware_fn_including_fn_bodies();
    let (mut with_ranges, mut ranges, mut dependent, mut sets, mut multi) = (0, 0, 0, 0, 0);
    let mut names = Vec::new();
    for (name, f) in &mods {
        let r = copper_analysis::constant_ticking_for_ranges(f);
        if r.is_empty() {
            continue;
        }
        with_ranges += 1;
        ranges += r.len();
        let d = copper_analysis::empty_range_dependencies(f);
        if !d.is_empty() {
            dependent += 1;
            sets += d.len();
            multi += d.iter().filter(|s| s.len() > 1).count();
            names.push(name.clone());
        }
    }
    println!(
        "constant ticking-for ranges: {ranges} in {with_ranges} modules; {dependent} modules depend on one \
         being non-empty ({sets} minimal sets, {multi} of more than one range): {names:?}"
    );
}

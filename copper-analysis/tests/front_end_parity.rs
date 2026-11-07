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

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
        let mem = copper_analysis::memory_result_drives_plain_out(f);
        if !mem.is_empty() {
            why.push(format!("memory result drives plain Out: {mem:?}"));
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

#[test]
fn every_module_a_macro_rule_flags_carries_the_opt_out_the_transpiler_honours() {
    let mods = corpus();

    let mut collapsing: BTreeSet<String> = BTreeSet::new();
    let mut flagged_without_opt_out: BTreeSet<String> = BTreeSet::new();
    for (name, f, opted_out) in &mods {
        if !copper_analysis::multi_write_collapse(f).is_empty() {
            collapsing.insert(name.clone());
        }
        let hits: Vec<&str> = [
            ("D1", !copper_analysis::unprotected_pretick_out_write(f).is_empty()),
            ("write-before-update", !copper_analysis::pretick_out_write_before_update(f).is_empty()),
            ("multi-phase", !copper_analysis::multi_phase_out_write(f).is_empty()),
            ("trailing RegOut from input", !copper_analysis::trailing_regout_reads_input(f).is_empty()),
            ("trailing Out from input", !copper_analysis::trailing_out_reads_input(f).is_empty()),
            ("late constant", !copper_analysis::out_first_driven_late(f).is_empty()),
        ]
        .into_iter()
        .filter(|(_, hit)| *hit)
        .map(|(n, _)| n)
        .collect();
        if !hits.is_empty() && !opted_out {
            flagged_without_opt_out.insert(format!("{name} — {}", hits.join(", ")));
        }
    }
    assert!(
        collapsing.is_empty(),
        "multi_write_collapse has no opt-out in either front-end, yet these corpus \
         modules trip it — they could not have compiled through the macro: {collapsing:?}"
    );
    assert!(
        flagged_without_opt_out.is_empty(),
        "these corpus modules trip a pre-tick alignment rule without \
         `allow_pretick_alignment`, so the macro would refuse them and so would the \
         transpiler — they cannot be in the tree as written: {flagged_without_opt_out:#?}"
    );
}

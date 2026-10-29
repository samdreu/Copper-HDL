//! Exact-set corpus pin for the register power-on rule (`copper_analysis::init`,
//! 2026-10-29), in the style of `pretick_alignment_corpus.rs`.
//!
//! Every pre-loop `let mut` the liveness inference calls a register must be
//! initialized from a constant or from `Bits::x()`; both front-ends refuse anything
//! else. The rule has no opt-out, so a corpus module it flags could not have
//! compiled — the expected set is **empty**, and this test exists so that a new
//! shape of initializer shows up as a diff here (and a false positive shows up as a
//! module name) rather than as a build failure somewhere in the tree. It also counts
//! how many pre-loop registers the corpus initializes, so the rule is known to be
//! looking at something.

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

/// The mode named by `#[hardware(<mode>, <flags>…)]`, from the first token (see
/// `pretick_alignment_corpus.rs` for why not `parse_args::<syn::Ident>()`).
fn hardware_mode_of(f: &ItemFn) -> Option<String> {
    f.attrs.iter().find_map(|a| {
        if !a.path().segments.last().is_some_and(|s| s.ident == "hardware") {
            return None;
        }
        let text = a.meta.require_list().ok()?.tokens.to_string();
        Some(text.split(',').next()?.trim().to_string())
    })
}

fn clocked_hardware_fns(items: &[Item], out: &mut Vec<ItemFn>) {
    for item in items {
        match item {
            Item::Fn(f) => {
                let mode = hardware_mode_of(f);
                if matches!(mode.as_deref(), Some("sequential") | Some("synchronizer")) {
                    out.push(f.clone());
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

/// The modules the rule must flag: none. A module here would be refused by the
/// macro, so it could not be in the tree as a swept module.
const EXPECTED_FLAGGED: &[&str] = &[];

#[test]
fn register_power_on_rule_flags_exactly_the_expected_modules() {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["examples", "tests", "src"] {
        rs_files(&root.join(dir), &mut files);
    }
    files.sort();

    let mut scanned = 0usize;
    let mut initialized_registers = 0usize;
    let mut flagged: BTreeSet<String> = BTreeSet::new();
    for path in &files {
        let Ok(src) = fs::read_to_string(path) else { continue };
        let Ok(file) = syn::parse_file(&src) else { continue };
        let mut fns = Vec::new();
        clocked_hardware_fns(&file.items, &mut fns);
        let rel = path.strip_prefix(&root).unwrap_or(path).display().to_string();
        for f in fns {
            scanned += 1;
            let registers = copper_analysis::infer_registers(&f);
            // Pre-loop `let mut` bindings in the register set — the ones the rule
            // looks at (the count is informational, the set is the pin).
            initialized_registers += f
                .block
                .stmts
                .iter()
                .take_while(|s| !matches!(s, syn::Stmt::Expr(syn::Expr::Loop(_), _)))
                .filter(|s| match s {
                    syn::Stmt::Local(l) => {
                        let name = match &l.pat {
                            syn::Pat::Ident(pi) => Some(pi.ident.to_string()),
                            syn::Pat::Type(pt) => match &*pt.pat {
                                syn::Pat::Ident(pi) => Some(pi.ident.to_string()),
                                _ => None,
                            },
                            _ => None,
                        };
                        l.init.is_some() && name.is_some_and(|n| registers.contains(&n))
                    }
                    _ => false,
                })
                .count();
            for (reg, _) in copper_analysis::non_constant_register_initializers(&f, &registers) {
                flagged.insert(format!("{rel}::{}::{reg}", f.sig.ident));
            }
        }
    }
    assert!(scanned >= 40, "expected to scan the corpus, only saw {scanned} clocked modules");
    assert!(
        initialized_registers >= 100,
        "expected the corpus to initialize well over a hundred pre-loop registers, saw \
         {initialized_registers} — is the scan or the register inference broken?"
    );

    let expected: BTreeSet<String> = EXPECTED_FLAGGED.iter().map(|s| s.to_string()).collect();
    let unexpected: Vec<_> = flagged.difference(&expected).cloned().collect();
    let missing: Vec<_> = expected.difference(&flagged).cloned().collect();
    assert!(
        unexpected.is_empty(),
        "NEW register(s) initialized from a non-constant: {unexpected:#?}\n\
         Both front-ends refuse this, so the module cannot compile as written. Initialize \
         the register with a constant, or with `Bits::x()` and a reset; if the initializer \
         IS a constant the predicate does not recognize, extend `is_constant_expr` (and the \
         codegen evaluator `lower_register_init`) in the same change."
    );
    assert!(
        missing.is_empty(),
        "expected flagged module(s) no longer flagged: {missing:?}"
    );
    println!(
        "register power-on: {scanned} clocked modules scanned, {initialized_registers} pre-loop \
         registers initialized, {} flagged",
        flagged.len()
    );
}

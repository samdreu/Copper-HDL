//! Corpus-wide inferred-register baseline — the instrument of Phase C step 6
//! (`design_docs/review/MEASUREMENTS.md` §K).
//!
//! `sv-baseline`'s twin for the analysis side: it records, for every clocked
//! `#[hardware]` module in `examples/`, `tests/`, `src/` (the same walk), the set
//! `copper_analysis::infer_registers` returns, so a change to the inference is a
//! printed per-module diff rather than something read off by eye:
//!
//! ```text
//! cargo run -q -p copper-codegen --bin register-baseline -- snapshot   # before
//! cargo run -q -p copper-codegen --bin register-baseline -- diff       # after
//! cargo run -q -p copper-codegen --bin register-baseline -- print      # the table
//! ```
//!
//! The baseline is one text file, `target/register-baseline.txt`, one line per
//! module: `<key>\t<reg>,<reg>,…`. `diff` prints every module whose set changed
//! as `- removed` / `+ added` names and exits non-zero if any did.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "old" || n == "target" || n == "obj_dir") {
                continue;
            }
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// module key -> inferred register set, for every module with a top-level loop.
fn infer_all(root: &Path) -> BTreeMap<String, BTreeSet<String>> {
    let mut files = Vec::new();
    for d in ["examples", "tests", "src"] {
        collect(&root.join(d), &mut files);
    }
    files.sort();
    let mut out = BTreeMap::new();
    for path in &files {
        let Ok(src) = std::fs::read_to_string(path) else { continue };
        let Ok(file) = syn::parse_file(&src) else { continue };
        for it in &file.items {
            let syn::Item::Fn(f) = it else { continue };
            if !f.attrs.iter().any(|a| a.path().segments.last().is_some_and(|s| s.ident == "hardware")) {
                continue;
            }
            let Some(cfg) = copper_analysis::Cfg::build(f) else { continue };
            let key = format!("{}::{}", path.strip_prefix(root).unwrap_or(path).display(), f.sig.ident);
            out.insert(key, cfg.registers().into_iter().collect());
        }
    }
    out
}

fn render(all: &BTreeMap<String, BTreeSet<String>>) -> String {
    let mut s = String::new();
    for (k, v) in all {
        s.push_str(k);
        s.push('\t');
        s.push_str(&v.iter().cloned().collect::<Vec<_>>().join(","));
        s.push('\n');
    }
    s
}

fn parse(text: &str) -> BTreeMap<String, BTreeSet<String>> {
    text.lines()
        .filter_map(|l| {
            let (k, v) = l.split_once('\t')?;
            Some((k.to_string(), v.split(',').filter(|x| !x.is_empty()).map(String::from).collect()))
        })
        .collect()
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root");
    let base = root.join("target/register-baseline.txt");
    let all = infer_all(root);
    match mode.as_str() {
        "snapshot" => {
            std::fs::create_dir_all(base.parent().unwrap()).unwrap();
            std::fs::write(&base, render(&all)).unwrap();
            println!("snapshot: {} clocked modules -> {}", all.len(), base.display());
        }
        "print" => print!("{}", render(&all)),
        "diff" => {
            let old = parse(&std::fs::read_to_string(&base).expect("run `register-baseline snapshot` first"));
            let mut changed = 0usize;
            let keys: BTreeSet<&String> = old.keys().chain(all.keys()).collect();
            for k in keys {
                let empty = BTreeSet::new();
                let (o, n) = (old.get(k).unwrap_or(&empty), all.get(k).unwrap_or(&empty));
                if o == n && old.contains_key(k) == all.contains_key(k) {
                    continue;
                }
                changed += 1;
                let tag = match (old.contains_key(k), all.contains_key(k)) {
                    (false, true) => "NEW     ",
                    (true, false) => "REMOVED ",
                    _ => "CHANGED ",
                };
                let minus: Vec<_> = o.difference(n).cloned().collect();
                let plus: Vec<_> = n.difference(o).cloned().collect();
                println!("  {tag}{k}: -{minus:?} +{plus:?}");
            }
            if changed == 0 {
                println!("register-baseline: {} clocked modules, no changes", all.len());
            } else {
                println!("register-baseline: {changed} of {} clocked modules differ", all.len());
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("usage: register-baseline <snapshot|diff|print>");
            std::process::exit(2);
        }
    }
}

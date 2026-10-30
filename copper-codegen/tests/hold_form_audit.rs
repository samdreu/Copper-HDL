//! The corpus audit of the mux + `<port>_hold` emission form — the DECLARED set
//! an `sv-baseline diff` is checked against when that form's reach changes.
//!
//! `vlir_lower::opening_hold_outputs` decides, per conditionally driven plain
//! `Out` port of a phase, whether the port takes phase C's forwarded mux + hold
//! form (at least one opening drive) or the edge-sampled register (none). Since
//! 2026-10-29 the decision is DRIVE-LEVEL: a port with both an opening and a
//! closing drive — a MIXED-anchor port — takes the form, its comb copy muxing
//! the opening drives only while every drive loads the hold register
//! (`design_docs/CYCLE_DATAFLOW_SEMANTICS.md` §9.4). The lowering records each
//! such port in `VLIRSeqBody::hold_form_ports`, which `copper_codegen::hold_form_ports`
//! exposes, and this test walks the same corpus `sv-baseline` walks and pins the
//! EXACT set of mixed-anchor ports. When the form's reach changes again, the
//! sv-baseline delta must equal the delta of this list — read off a list, not
//! off the diff.
//!
//! Enumerated 2026-10-29, when the drive-level form landed: the sv-baseline delta
//! of that change was exactly the seven modules below (two copies of
//! `branch_merge`: the fixture and the probe file's verbatim twin), no referenced
//! module among them. `e4i` is in the set on the CODEGEN side only — its
//! register-free body makes both of its writes opening in the simulator, so the
//! form does not make it agree (`multi_phase_out_write` clause 3 refuses it, and
//! its probe stays an `assert_ne!`); it is listed because the emission reaches it.

use std::collections::BTreeSet;
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

/// `<file>::<module>::<port> (opening/closing)` for every hold-form port in the
/// corpus, split into the all-opening and the mixed-anchor sets.
fn audit() -> (BTreeSet<String>, BTreeSet<String>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("workspace root");
    let mut files = Vec::new();
    for d in ["examples", "tests", "src"] {
        collect(&root.join(d), &mut files);
    }
    files.sort();
    let mut all_opening = BTreeSet::new();
    let mut mixed = BTreeSet::new();
    for path in &files {
        let Ok(src) = std::fs::read_to_string(path) else { continue };
        let Ok(file) = syn::parse_file(&src) else { continue };
        let names: Vec<String> = file
            .items
            .iter()
            .filter_map(|it| match it {
                syn::Item::Fn(f) if copper_codegen::is_hardware_fn(f) => Some(f.sig.ident.to_string()),
                _ => None,
            })
            .collect();
        let multi = names.len() > 1;
        let file_key = path.strip_prefix(root).unwrap_or(path).display().to_string();
        for name in names {
            let Ok(ports) = copper_codegen::hold_form_ports(&src, if multi { Some(name.as_str()) } else { None })
            else {
                continue; // refused modules have no emission to audit
            };
            for p in ports {
                let key = format!("{file_key}::{name}::{} ({}/{})", p.port, p.opening, p.closing);
                if p.closing > 0 { mixed.insert(key); } else { all_opening.insert(key); }
            }
        }
    }
    (all_opening, mixed)
}

#[test]
fn the_mixed_anchor_hold_form_set_is_exactly_the_declared_one() {
    let (all_opening, mixed) = audit();
    eprintln!("all-opening hold-form ports ({}):", all_opening.len());
    for k in &all_opening {
        eprintln!("  {k}");
    }
    eprintln!("mixed-anchor hold-form ports ({}):", mixed.len());
    for k in &mixed {
        eprintln!("  {k}");
    }
    let expected: BTreeSet<String> = [
        "tests/fixtures/control_extraction_dut.rs::branch_merge::tail_o (1/1)",
        "tests/fixtures/control_extraction_dut.rs::branch_merge_explicit::tail_o (1/1)",
        "tests/sequential_forwarding_divergence.rs::branch_merge::tail_o (1/1)",
        "tests/sequential_forwarding_divergence.rs::d2a_mixed_two_drives_hold::o (1/1)",
        "tests/sequential_forwarding_divergence.rs::e4_closing_then_opening::o (1/1)",
        "tests/sequential_forwarding_divergence.rs::e4i_register_free_passthrough_then_opening::o (1/1)",
        "tests/sequential_forwarding_divergence.rs::e6_closing_via_wire_then_opening::o (1/1)",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(
        mixed, expected,
        "the set of mixed-anchor ports taking the drive-level mux + hold form changed — \
         the sv-baseline delta of whatever changed it must equal this delta, and a new \
         member needs its own measured verdict in tests/sequential_forwarding_divergence.rs"
    );
    assert!(!all_opening.is_empty(), "the all-opening hold-form set is empty — phase C's form is not reaching anything");
}

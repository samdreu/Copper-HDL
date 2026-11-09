//! D-30 (2026-11-09, SPEC I.8.4(b), MEASUREMENTS §AF): a ticking `for` over a range bounded by a
//! const generic, where R3's verdict depends on it, is taken as non-empty by both front-ends and
//! checked per instance. The Rust side (an inline `const { assert!(..) }` the macro places, E0080 at
//! monomorphization) is `src/lib.rs`'s `compile_fail,E0080` doctest. This file pins the transpiler's
//! side — the elaboration-time check it emits, refused by Verilator at an empty instance — and the
//! set of ranges both front-ends choose (`copper_analysis::assumed_nonempty_generic_ranges`).

mod common;
use common::{verilator_available, verilator_command};

const DUT_SRC: &str = include_str!("fixtures/generic_range_dut.rs");

static TMP_NONCE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn module(src: &str, name: &str) -> syn::ItemFn {
    let file: syn::File = syn::parse_str(src).expect("parse");
    file.items
        .into_iter()
        .find_map(|i| match i {
            syn::Item::Fn(f) if f.sig.ident == name => Some(f),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no fn {name}"))
}

fn assumed(src: &str, name: &str) -> Vec<String> {
    copper_analysis::assumed_nonempty_generic_ranges(&module(src, name))
        .iter()
        .map(copper_analysis::range_key)
        .collect()
}

#[test]
fn the_assumed_ranges_are_exactly_those_r3_depends_on() {
    // Each fixture module: the generic ranges R3's verdict depends on.
    for (m, want) in [
        ("grn_only_boundary", vec!["0 .. W"]),
        ("grn_then_wait", vec!["0 .. W"]),
        ("grn_handwritten", vec!["0 .. W"]),
        // Both inner `for`s together are the iteration's boundary: a set of two, each checked.
        ("grn_inner_ends_wait", vec!["0 .. W", "GRN_ONE ..= (W + GRN_ONE)"]),
        ("grn_outer_ends_wait", vec!["0 .. W"]),
        ("grn_two_generic", vec!["0 .. W", "0 .. V"]),
    ] {
        assert_eq!(assumed(DUT_SRC, m), want, "{m}");
    }
    // A generic range followed by a tick is no boundary R3 depends on: nothing assumed, the
    // lowering keeps its empty-range exit (det_for's `0..N`, MEASUREMENTS §AE.5's `zzi_33`).
    let det_for = include_str!("fixtures/det_for_dut.rs");
    assert!(assumed(det_for, "det_for").is_empty());
    let followed = "async fn f<const W: usize>(clk: Clock<A>, q: RegOut<Bits<8>, A>) { loop { for i in 0u8..2 { \
        for _ in 0..W { clk.tick().await; } q.write(Bits::from_u8(i)); clk.tick().await; } } }";
    assert!(assumed(followed, "f").is_empty());
}

fn lint(sv: &str, top: &str, params: &[(&str, i64)]) -> (bool, String) {
    let dir = std::env::temp_dir().join(format!(
        "copper_d30_{}_{}",
        std::process::id(),
        TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{top}.sv"));
    std::fs::write(&file, sv).unwrap();
    let mut cmd = verilator_command();
    cmd.args(["--lint-only", "-Wall", "-Wno-DECLFILENAME", "--top-module", top]);
    for (k, v) in params {
        cmd.arg(format!("-G{k}={v}"));
    }
    cmd.arg(&file);
    let out = cmd.output().expect("run verilator");
    let _ = std::fs::remove_dir_all(&dir);
    (out.status.success(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn an_empty_instance_does_not_elaborate_and_a_non_empty_one_does() {
    let cfg = copper_codegen::EmitConfig::default();
    for (m, n) in [("grn_only_boundary", 1), ("grn_inner_ends_wait", 2), ("grn_two_generic", 2)] {
        let sv = copper_codegen::transpile_source(DUT_SRC, Some(m), &cfg).expect(m);
        assert_eq!(sv.matches("D-30").count(), n, "{m}: one check per assumed range\n{sv}");
    }
    if !verilator_available() {
        return;
    }
    let sv = copper_codegen::transpile_source(DUT_SRC, Some("grn_only_boundary"), &cfg).unwrap();
    let (ok, err) = lint(&sv, "grn_only_boundary", &[("W", 0)]);
    assert!(!ok && err.contains("USERERROR") && err.contains("D-30"), "W = 0 must not elaborate:\n{err}");
    for w in [1, 3] {
        let (ok, err) = lint(&sv, "grn_only_boundary", &[("W", w)]);
        assert!(ok, "W = {w} must elaborate:\n{err}");
    }
    // The set of two is checked range by range (recorded over-check): W = 0 with V = 3 is refused
    // though Rust's R3 would hold through `0..V`.
    let sv = copper_codegen::transpile_source(DUT_SRC, Some("grn_two_generic"), &cfg).unwrap();
    let (ok, _) = lint(&sv, "grn_two_generic", &[("W", 0), ("V", 3)]);
    assert!(!ok, "W = 0, V = 3: refused, range by range");
    let (ok, err) = lint(&sv, "grn_two_generic", &[("W", 2), ("V", 3)]);
    assert!(ok, "W = 2, V = 3 must elaborate:\n{err}");
}

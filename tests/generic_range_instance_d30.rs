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

/// Whether a tool is present and runnable: absent = skip, installed-but-broken = fail (the
/// harness's split, CLAUDE.md).
fn tool_available(bin: &str, version_flag: &str) -> Option<String> {
    match std::process::Command::new(bin).arg(version_flag).output() {
        Ok(o) if o.status.success() => Some(String::from_utf8_lossy(&o.stdout).into_owned()),
        Ok(o) => panic!(
            "{bin} is installed but `{bin} {version_flag}` failed — a broken environment, not a missing tool:\n{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        Err(_) => None,
    }
}

/// Elaborate `sv` under Icarus (`iverilog -g2012`, then `vvp`): whether both exit 0.
fn icarus_accepts(sv: &str, top: &str, params: &[(&str, i64)]) -> bool {
    let dir = std::env::temp_dir().join(format!(
        "copper_d30_icarus_{}_{}",
        std::process::id(),
        TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("m.sv");
    let out = dir.join("m.vvp");
    std::fs::write(&file, sv).unwrap();
    let mut cmd = std::process::Command::new("iverilog");
    cmd.args(["-g2012", "-s", top, "-o"]).arg(&out);
    for (k, v) in params {
        cmd.arg(format!("-P{top}.{k}={v}"));
    }
    let ok = cmd.arg(&file).output().expect("run iverilog").status.success()
        && std::process::Command::new("vvp").arg(&out).output().expect("run vvp").status.success();
    let _ = std::fs::remove_dir_all(&dir);
    ok
}

/// The toolchain limit of SPEC III.3.4 `II.3 found icarus-elab-error` (MEASUREMENTS §AG): Icarus 12
/// runs an elaboration system task (IEEE 1800-2017 §20.11) in a module's own scope but silently skips
/// one inside a generate scope — where D-30's and I.14.5(d)'s checks are — so it elaborates an
/// instance Verilator and Yosys refuse. Icarus is no correctness leg (`tests/sim_throughput.rs` only).
/// Pinned on Icarus 12 so a release that changes it fails here and the line is revisited.
#[test]
fn icarus_12_skips_an_elaboration_task_inside_a_generate_scope() {
    let cfg = copper_codegen::EmitConfig::default();
    let sv = copper_codegen::transpile_source(DUT_SRC, Some("grn_only_boundary"), &cfg).unwrap();
    if let Some(v) = tool_available("yosys", "-V") {
        let dir = std::env::temp_dir().join(format!(
            "copper_d30_yosys_{}_{}",
            std::process::id(),
            TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("m.sv");
        std::fs::write(&file, &sv).unwrap();
        let yosys = |w: i64| {
            std::process::Command::new("yosys")
                .arg("-q")
                .arg("-p")
                .arg(format!(
                    "read_verilog -sv {}; chparam -set W {w} grn_only_boundary; hierarchy -top grn_only_boundary",
                    file.display()
                ))
                .output()
                .expect("run yosys")
        };
        let refused = yosys(0);
        assert!(
            !refused.status.success() && String::from_utf8_lossy(&refused.stderr).contains("D-30"),
            "Yosys ({}) must refuse W = 0 with D-30's message",
            v.trim()
        );
        assert!(yosys(3).status.success(), "Yosys must elaborate W = 3");
        let _ = std::fs::remove_dir_all(&dir);
    }
    let Some(version) = tool_available("iverilog", "-V") else { return };
    let first = version.lines().next().unwrap_or("").to_string();
    // The cause, three ways: a module-scope `$error` is run, the same task in a generate `if` is not,
    // and the emitted D-30 check (a generate `if` around `$error`) is therefore skipped.
    let module_scope = icarus_accepts("module t; $error(\"x\"); endmodule\n", "t", &[]);
    let generate_scope = icarus_accepts("module t; if (1) begin : b $error(\"x\"); end endmodule\n", "t", &[]);
    let empty_instance = icarus_accepts(&sv, "grn_only_boundary", &[("W", 0)]);
    assert!(icarus_accepts(&sv, "grn_only_boundary", &[("W", 3)]), "{first}: W = 3 must elaborate");
    assert!(!module_scope, "{first}: a module-scope elaboration `$error` must fail the elaboration");
    if first.contains("version 12.") {
        assert!(
            generate_scope && empty_instance,
            "{first}: Icarus 12 was measured to skip an elaboration task in a generate scope \
             (generate-scope accepted: {generate_scope}, W = 0 accepted: {empty_instance}); \
             the limit changed — revisit SPEC III.3.4 `II.3 found icarus-elab-error`"
        );
    } else {
        println!(
            "{first}: not the measured Icarus 12 — generate-scope `$error` accepted: {generate_scope}, \
             D-30 W = 0 accepted: {empty_instance} (the pin covers Icarus 12 only)"
        );
    }
}

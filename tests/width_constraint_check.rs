//! **Width constraints as elaboration-time checks** (SPEC I.14.5(d), D-19 5A;
//! I.14.7 (8), Phase C step 8, `design_docs/review/MEASUREMENTS.md` §M).
//!
//! A generic module's `const { assert!(N_LOG == safe_clog2(N)) }` is a shape rule at
//! instantiation: rustc refuses a violating instance at monomorphization. Until step
//! 8 the transpiler's one parametric SV module stated nothing, so `rotate_right` at
//! `N = 8, N_LOG = 2` elaborated silently. Now each assertion is a generate-time
//! `if (!(cond)) $error(…)`, and Verilator stops elaborating that instance.
//!
//! Pinned here: (1) `rotate_right`'s SV at a violating parameter is refused by
//! Verilator with the assertion's message, and at the sweep's parameter it builds;
//! (2) every `const { assert!(..) }` in the corpus's generic modules becomes a check
//! (none is left untranslated). Icarus 12 does not run elaboration-time `$error` in
//! a generate block (measured 2026-11-07: exit 0, no message), so the check binds
//! the Verilator profile the sweep uses.

mod common;

use common::{verilator_available, verilator_command};
use std::sync::atomic::{AtomicUsize, Ordering};

static TMP_NONCE: AtomicUsize = AtomicUsize::new(0);

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rotate_right_sv() -> String {
    let src = std::fs::read_to_string(root().join("examples/combinational/rotate_right.rs")).unwrap();
    copper_codegen::transpile_source(&src, Some("rotate_right"), &copper_codegen::EmitConfig::default())
        .expect("rotate_right transpiles")
}

/// `verilator --lint-only -Wall` of `sv` at the given `-G` parameters.
fn lint(sv: &str, params: &[(&str, usize)]) -> std::process::Output {
    let dir = std::env::temp_dir().join(format!(
        "copper_width_check_{}_{}",
        std::process::id(),
        TMP_NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rotate_right.sv");
    std::fs::write(&path, sv).unwrap();
    let mut cmd = verilator_command();
    cmd.current_dir(&dir).args(["--lint-only", "-Wall", "-Wno-DECLFILENAME"]);
    for (k, v) in params {
        cmd.arg(format!("-G{k}={v}"));
    }
    cmd.arg(&path);
    let out = cmd.output().expect("verilator runs");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

#[test]
fn rotate_right_states_its_width_constraint() {
    let sv = rotate_right_sv();
    assert!(
        sv.contains("if (!((N_LOG == $clog2(N)))) begin : __copper_width_check_0")
            && sv.contains("$error(\"N_LOG must equal safe_clog2(N)\");"),
        "the width constraint is not emitted:\n{sv}"
    );
}

#[test]
fn rotate_right_at_a_violating_parameter_is_refused_at_elaboration() {
    if !verilator_available() {
        return;
    }
    let sv = rotate_right_sv();
    let bad = lint(&sv, &[("N", 8), ("N_LOG", 2)]);
    let text = format!("{}{}", String::from_utf8_lossy(&bad.stdout), String::from_utf8_lossy(&bad.stderr));
    assert!(!bad.status.success(), "N = 8, N_LOG = 2 elaborated:\n{text}");
    assert!(text.contains("N_LOG must equal safe_clog2(N)"), "not the constraint's message:\n{text}");
    // The sweep's instance (build.rs PARAMS: N = 8, N_LOG = 3) builds.
    let good = lint(&sv, &[("N", 8), ("N_LOG", 3)]);
    assert!(
        good.status.success(),
        "N = 8, N_LOG = 3 refused:\n{}",
        String::from_utf8_lossy(&good.stderr)
    );
}

#[test]
fn every_corpus_width_constraint_becomes_a_check() {
    let files = [
        ("examples/sequential/shift_register.rs", None),
        ("examples/combinational/mux.rs", None),
        ("examples/combinational/rotate_right.rs", None),
        ("examples/combinational/priority_encode.rs", None),
        ("tests/fixtures/rotate_right_dut.rs", None),
        ("tests/fixtures/shift_register_dut.rs", None),
        ("tests/fixtures/priority_encode_dut.rs", None::<&str>),
    ];
    let mut total = 0;
    for (f, _) in files {
        let src = std::fs::read_to_string(root().join(f)).unwrap();
        let asserts = src.matches("const { assert!(").count();
        assert!(asserts > 0, "{f}: expected a width constraint");
        let sv = copper_codegen::transpile_source(&src, None, &copper_codegen::EmitConfig::default())
            .unwrap_or_else(|e| panic!("{f}: {e}"));
        let checks = sv.matches("__copper_width_check_").count();
        assert_eq!(checks, asserts, "{f}: {asserts} assertion(s), {checks} check(s):\n{sv}");
        total += checks;
    }
    eprintln!("width constraints emitted as checks: {total} in {} files", files.len());
}

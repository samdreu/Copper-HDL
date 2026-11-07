//! R11 front-end parity under D-21 A3 (2026-11-07, MEASUREMENTS §N).
//!
//! The macro has rustc judge a register's power-on value: every pre-loop register
//! initializer is evaluated in an inline `const { }` block, so "constant" is Rust's
//! constant. The transpiler cannot see that verdict; it runs R11's syntactic half
//! (no run-time read — the macro runs the same function first), refuses a call to a
//! fn that is not a `const fn` it knows (rustc's E0015), and folds the rest through
//! `copper_analysis::consts::ConstEnv`.
//!
//! The two are checked on the **same inputs**: this test transpiles the macro's own
//! trybuild cases. `copper-macros/tests/ui/pass/register_init_rust_constants_ok.rs`
//! compiles through rustc, so the transpiler must admit it (the older
//! `register_init_constants_ok.rs` is NOT transpilable, for two reasons unrelated to
//! R11 — a match-as-expression with no wildcard arm, and a `[Logic; 4]` register's
//! element writes refused as a latch; MEASUREMENTS §N); every `ui/fail/register_init_*.rs`
//! is refused by rustc, so the transpiler must refuse it, and with the R11 reason
//! (not some unrelated construct). The admitted spellings are also swept on all
//! three legs as `tests/fixtures/const_init_dut.rs`.

use std::path::{Path, PathBuf};

fn ui_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("copper-macros/tests/ui")
}

fn cases(sub: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(ui_dir().join(sub))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().is_some_and(|x| x == "rs")
                && p.file_name().unwrap().to_string_lossy().starts_with("register_init_")
        })
        .collect();
    v.sort();
    v
}

fn transpile(path: &Path) -> Result<String, String> {
    let src = std::fs::read_to_string(path).unwrap();
    copper_codegen::transpile_source(&src, None, &copper_codegen::EmitConfig::default())
}

#[test]
fn the_transpiler_admits_every_initializer_rustc_admits() {
    let pass = vec![ui_dir().join("pass/register_init_rust_constants_ok.rs")];
    assert!(pass[0].exists(), "the D-21 ui/pass case is missing — the parity check would be vacuous");
    for p in &pass {
        let sv = transpile(p).unwrap_or_else(|e| panic!("{}: rustc admits it, the transpiler refuses: {e}", p.display()));
        // Each admitted spelling reaches the SV as a stated power-on value.
        for (reg, value) in [
            ("via_fn", "8'd10"),       // seed_of(3) = 3 * 3 + 1
            ("assoc", "8'd11"),        // Cfg::K
            ("assoc_fn", "8'd22"),     // Cfg::twice(Cfg::K)
            ("from_const", "8'd7"),    // SEED
        ] {
            let line = format!("{reg} = {value};");
            assert!(sv.contains(&line), "{}: expected `{line}` in the initial block:\n{sv}", p.display());
        }
    }
}

#[test]
fn the_transpiler_refuses_every_initializer_rustc_refuses() {
    let fail = cases("fail");
    assert!(fail.len() >= 3, "expected the three ui/fail/register_init_*.rs cases, found {}", fail.len());
    for p in &fail {
        let err = match transpile(p) {
            Ok(_) => panic!("{}: rustc refuses it, the transpiler admits it", p.display()),
            Err(e) => e,
        };
        assert!(
            err.contains("has no power-on value for it"),
            "{}: refused, but not by R11: {err}",
            p.display()
        );
    }
}

/// The two halves name the right reason: a run-time read gets R11's syntactic message
/// (the macro prints the same), a non-`const` callee the call message (the macro's is
/// rustc's E0015).
#[test]
fn each_refusal_names_its_reason() {
    let dir = ui_dir().join("fail");
    let e = transpile(&dir.join("register_init_reads_input.rs")).unwrap_err();
    assert!(e.contains("register `acc` is initialized from a method call that reads a run-time value"), "{e}");
    let e = transpile(&dir.join("register_init_not_constant.rs")).unwrap_err();
    assert!(e.contains("initialized by a call to `seed`, which is not a `const fn`"), "{e}");
    let e = transpile(&dir.join("register_init_non_const_ctor.rs")).unwrap_err();
    assert!(e.contains("initialized by a call to `Latch::bubble`, which is not a `const fn`"), "{e}");
}

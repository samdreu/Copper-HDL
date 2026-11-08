//! Front-end parity in the transpiler-admits direction (SPEC II.1.3, P10; Phase C
//! step 10, `review/MEASUREMENTS.md` §P).
//!
//! The macro's trybuild cases are the battery: every `copper-macros/tests/ui/fail/*.rs`
//! is a shape rustc refuses (through the macro, or through rustc itself where the
//! macro wraps a const), so the transpiler must refuse it; every `ui/pass/*.rs`
//! compiles, so the transpiler must admit it. Before step 10 the transpiler admitted
//! ten of the then-30 fail cases (`cdc_crossing`, `cdc_memory_parameter`, the three
//! `function_typed_*` header cases, `multiple_top_level_loops`, `param_shadowing`,
//! `retired_pretick_opt_out`, `sequential_unprotectable_read`, and refused
//! `clock_construction` only by the accident of a width message). Every rule a fail
//! case pins is now one function of `copper-analysis` with one message, so the test
//! also requires the transpiler's message to be the macro's — the first line of the
//! case's `.stderr`.
//!
//! A new ui/fail case is in the battery the moment it exists, as a sweep module is.

use std::path::{Path, PathBuf};

fn ui_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().join("copper-macros/tests/ui")
}

fn cases(sub: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(ui_dir().join(sub))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    v.sort();
    v
}

/// Transpile every `#[hardware]` fn of the case: `Ok` iff all of them transpile,
/// else the errors (a case refused by the macro is refused at one of its modules).
fn transpile(path: &Path) -> Result<String, String> {
    let src = std::fs::read_to_string(path).unwrap();
    let file = syn::parse_file(&src).unwrap();
    let names: Vec<String> = file
        .items
        .iter()
        .filter_map(|i| match i {
            syn::Item::Fn(f) if copper_codegen::is_hardware_fn(f) => Some(f.sig.ident.to_string()),
            _ => None,
        })
        .collect();
    let mut errs = Vec::new();
    for n in &names {
        if let Err(e) = copper_codegen::transpile_source(&src, Some(n), &copper_codegen::EmitConfig::default()) {
            errs.push(e);
        }
    }
    if errs.is_empty() { Ok(format!("{} modules", names.len())) } else { Err(errs.join("\n")) }
}

/// ui/pass cases the transpiler refuses, each with its reviewed reason. A case
/// here is a recorded parity break, not a silent exclusion.
const PASS_NOT_TRANSPILED: &[(&str, &str)] = &[(
    "register_init_constants_ok.rs",
    "a match-as-expression with no wildcard arm and a `[Logic; 4]` register's element writes \
     refused as a latch — two lowering limits unrelated to the rules this case pins (R11's \
     constants); the transpilable twin is register_init_rust_constants_ok.rs (MEASUREMENTS §N)",
)];

/// ui/fail cases whose transpiler message is not the macro's, with the reason.
/// rustc, not the macro, refuses these — E0015 for a non-`const` callee inside the
/// macro's `const { }` wrap (R11), E0308 for a plain `fn` called where the macro
/// wraps an instantiation in `copper_sim::__instance` (I.13.4(d)) — so there is no
/// macro message to equal; the transpiler refuses from its own knowledge (the
/// fold, the registry).
const RUSTC_JUDGED: &[&str] = &[
    "register_init_non_const_ctor.rs",
    "register_init_not_constant.rs",
    "structural_plain_fn_call.rs",
];

#[test]
fn the_transpiler_refuses_every_shape_the_macro_refuses_with_the_same_message() {
    let fail = cases("fail");
    assert!(fail.len() >= 37, "expected the ui/fail battery, found {}", fail.len());
    let mut admitted = Vec::new();
    let mut other_message = Vec::new();
    for p in &fail {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let err = match transpile(p) {
            Ok(_) => {
                admitted.push(name);
                continue;
            }
            Err(e) => e,
        };
        if RUSTC_JUDGED.contains(&name.as_str()) {
            continue;
        }
        let stderr = std::fs::read_to_string(p.with_extension("stderr")).unwrap();
        let first = stderr.lines().next().unwrap_or("");
        let Some(macro_msg) = first.strip_prefix("error: ") else {
            other_message.push(format!("{name}: macro's first line is not a macro error: {first}"));
            continue;
        };
        if !err.contains(macro_msg) {
            other_message.push(format!("{name}:\n    macro:      {macro_msg}\n    transpiler: {err}"));
        }
    }
    assert!(admitted.is_empty(), "the macro refuses these, the transpiler admits them: {admitted:#?}");
    assert!(other_message.is_empty(), "refused by both, with different messages:\n{}", other_message.join("\n"));
}

#[test]
fn the_transpiler_admits_every_shape_the_macro_admits() {
    let pass = cases("pass");
    assert!(pass.len() >= 8, "expected the ui/pass battery, found {}", pass.len());
    let mut refused = Vec::new();
    for p in &pass {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let r = transpile(p);
        let excused = PASS_NOT_TRANSPILED.iter().any(|(n, _)| *n == name);
        match (r, excused) {
            (Ok(_), false) | (Err(_), true) => {}
            (Err(e), false) => refused.push(format!("{name}: {e}")),
            (Ok(_), true) => refused.push(format!("{name}: transpiles now — delete its PASS_NOT_TRANSPILED row")),
        }
    }
    assert!(refused.is_empty(), "{refused:#?}");
}

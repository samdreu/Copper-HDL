//! The integer bit-count methods' emitted form under three tools (SPEC III.3.4 `II.3 found
//! int-bit-count` and its family, `II.3 found yosys-unary-cast`; MEASUREMENTS §AI). The sweep checks
//! the values (`tests/fixtures/int_bit_count_dut.rs`, simulator vs interpreter vs Verilator); this file
//! checks that the text is accepted by Verilator (`--lint-only -Wall`), Yosys (`proc; opt`) and Icarus
//! (`-g2012`), and pins the Yosys limit the emitter works around: Yosys 0.68 refuses a size cast
//! directly under a prefix operator (`~16'(x)`), so the emitter writes `~(16'(x))`. An absent Yosys or
//! Icarus skips its leg; an installed-but-broken one fails (CLAUDE.md's split).

mod common;
use common::{verilator_available, verilator_command};

const IBC_SRC: &str = include_str!("fixtures/int_bit_count_dut.rs");
const WINT_SRC: &str = include_str!("fixtures/wide_int_dut.rs");
const IBC_MODULES: [&str; 6] =
    ["ibc_seq_unsigned", "ibc_seq_signed", "ibc_comb_unsigned", "ibc_comb_signed", "ibc_u32_result", "ibc_path_call"];

static TMP_NONCE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

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

/// `sv` written to a fresh per-invocation directory (removed by the caller via the returned path).
fn write_sv(sv: &str, top: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "copper_ibc_{}_{}",
        std::process::id(),
        TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{top}.sv"));
    std::fs::write(&file, sv).unwrap();
    file
}

/// Run each available tool on `sv`; the failures, by tool, with their output.
fn three_tools(sv: &str, top: &str) -> Vec<String> {
    let file = write_sv(sv, top);
    let mut failures = Vec::new();
    let text = |o: &std::process::Output| {
        format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
    };
    if verilator_available() {
        let o = verilator_command()
            .args(["--lint-only", "-Wall", "-Wno-DECLFILENAME", "--top-module", top])
            .arg(&file)
            .output()
            .expect("run verilator");
        if !o.status.success() {
            failures.push(format!("verilator on {top}:\n{}", text(&o)));
        }
    }
    if tool_available("yosys", "-V").is_some() {
        let script = format!("read_verilog -sv {}; hierarchy -top {top}; proc; opt", file.display());
        let o = std::process::Command::new("yosys").args(["-q", "-p", &script]).output().expect("run yosys");
        if !o.status.success() {
            failures.push(format!("yosys on {top}:\n{}", text(&o)));
        }
    }
    if tool_available("iverilog", "-V").is_some() {
        let o = std::process::Command::new("iverilog")
            .args(["-g2012", "-s", top, "-o"])
            .arg(file.with_extension("vvp"))
            .arg(&file)
            .output()
            .expect("run iverilog");
        if !o.status.success() {
            failures.push(format!("iverilog on {top}:\n{}", text(&o)));
        }
    }
    let _ = std::fs::remove_dir_all(file.parent().unwrap());
    failures
}

/// Every bit-count module, and `wint_not` (the corpus module whose `~128'(x)` Yosys refused before
/// §AI), is accepted by every installed tool of the three.
#[test]
fn the_bit_count_forms_are_accepted_by_verilator_yosys_and_icarus() {
    let cfg = copper_codegen::EmitConfig::default();
    let mut failures = Vec::new();
    for m in IBC_MODULES {
        let sv = copper_codegen::transpile_source(IBC_SRC, Some(m), &cfg).unwrap_or_else(|e| panic!("{m}: {e:?}"));
        assert!(m == "ibc_path_call" || sv.contains("$countones("), "{m}: no `$countones` in\n{sv}");
        failures.extend(three_tools(&sv, m));
    }
    let sv = copper_codegen::transpile_source(WINT_SRC, Some("wint_not"), &cfg).expect("wint_not");
    failures.extend(three_tools(&sv, "wint_not"));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The emitter parenthesizes a size cast under a prefix operator: no `~N'(`, `-N'(` or `!N'(` in any
/// of these modules' text, and `wint_not`'s complement is `~(128'(x))`.
#[test]
fn a_size_cast_under_a_prefix_operator_is_parenthesized() {
    let cfg = copper_codegen::EmitConfig::default();
    let bare = |sv: &str| {
        let b = sv.as_bytes();
        (0..b.len()).any(|i| {
            matches!(b[i], b'~' | b'!' | b'-') && {
                let d = b[i + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
                d > 0 && b[i + 1 + d..].starts_with(b"'(")
            }
        })
    };
    let wint = copper_codegen::transpile_source(WINT_SRC, Some("wint_not"), &cfg).expect("wint_not");
    assert!(wint.contains("(~(128'(x)))"), "wint_not:\n{wint}");
    assert!(!bare(&wint), "a bare cast under a prefix operator in wint_not:\n{wint}");
    for m in IBC_MODULES {
        let sv = copper_codegen::transpile_source(IBC_SRC, Some(m), &cfg).unwrap();
        assert!(!bare(&sv), "a bare cast under a prefix operator in {m}:\n{sv}");
    }
}

/// The Yosys limit itself, pinned on Yosys 0.68 so a release that changes it fails here and the
/// emitter's workaround is revisited: `(~16'(x))` is refused, `(~(16'(x)))` accepted.
#[test]
fn yosys_0_68_refuses_a_bare_cast_under_a_prefix_operator() {
    let Some(version) = tool_available("yosys", "-V") else { return };
    if !version.contains("Yosys 0.68") {
        return;
    }
    let run = |body: &str| {
        let sv = format!(
            "module t(input logic [127:0] x, output logic [15:0] w);\n  always_comb w = {body};\nendmodule\n"
        );
        let file = write_sv(&sv, "t");
        let script = format!("read_verilog -sv {}; hierarchy -top t; proc; opt", file.display());
        let o = std::process::Command::new("yosys").args(["-q", "-p", &script]).output().expect("run yosys");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
        (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
    };
    let (ok, out) = run("(~16'(x))");
    assert!(!ok && out.contains("Static cast with zero or negative size"), "Yosys 0.68 accepted `(~16'(x))`:\n{out}");
    let (ok, out) = run("(~(16'(x)))");
    assert!(ok, "Yosys 0.68 refused `(~(16'(x)))`:\n{out}");
}

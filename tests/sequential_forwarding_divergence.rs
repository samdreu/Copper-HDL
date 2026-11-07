//! The forwarding-divergence battery, after Phase C step 3 (D-8, 2026-11-06).
//!
//! This file held the measured two-job divergence probes — 167 tests over 160
//! modules, each a plain `Out` on a clocked body (the D1/D2 pair, the V8 family,
//! the `d*`/`e*` batteries) — the evidence behind the pre-tick alignment rules.
//! Under the one-job body (D-8, SPEC II.1 R8) a clocked module has no plain `Out`:
//! every one of those modules is unexpressible, the rules they measured are retired
//! (SPEC II.3.2) and the tests went with them (named in the Phase C step 3 commit;
//! the record of what they measured is `design_docs/CYCLE_DATAFLOW_SEMANTICS.md`
//! §9–§10 and `design_docs/PRETICK_ALIGNMENT_GUARDRAIL.md`).
//!
//! What stays is the battery's one agreeing `RegOut` row (SPEC II.3.2: "the
//! agreeing `RegOut` rows kept where they compile"): the pulse driven in two
//! phases, which agrees with its own transpiled SystemVerilog.

mod common;
use common::{verilator_available, verilator_command};
use copper_core::port::RegOut;
use copper_core::{Clock, ClockDomain, Logic};
use copper_macros::hardware;
use copper_sim::HardwareExecutor;
use std::path::Path;

/// Per-invocation nonce for temporary directories (CLAUDE.md: never key a work
/// dir on anything less unique than one invocation).
static TMP_NONCE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

struct C;
impl ClockDomain for C {}

const CYCLES: usize = 13;

// ── Verilator plumbing ────────────────────────────────────────────────────────

/// Verilate `sv` (already written to `sv_path`) as `top`, tick `clk_port`, and
/// collect the whitespace-separated values each `probe` line prints per cycle.
fn run_sv(sv_path: &Path, top: &str, clk_port: &str, probes: &[&str], extra_init: &str) -> Vec<Vec<u32>> {
    // Unique PER INVOCATION, not per top module: two tests in this binary
    // Verilate the same top in parallel (`trailing_update` is built by both the
    // d1-trailing gap test and the m2 model-lowering test), and a shared
    // directory lets them clobber each other's build — the false-PASS/false-FAIL
    // mechanism the repo's obj_dir convention exists to prevent (see CLAUDE.md).
    static NONCE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let work =
        std::env::temp_dir().join(format!("copper_fwd_{top}_{}_{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();

    let mut tb = format!(
        "#include \"V{top}.h\"\n#include \"verilated.h\"\n#include <iostream>\n\
         int main(int c, char** v) {{ Verilated::commandArgs(c, v);\n\
         V{top}* t = new V{top}(); t->{clk_port} = 0; {extra_init} t->eval();\n"
    );
    let body: String = probes.iter().map(|p| format!("std::cout << (int)t->{p} << \" \";")).collect();
    for _ in 0..CYCLES {
        tb.push_str(&format!(
            "t->{clk_port}=0; t->eval(); t->{clk_port}=1; t->eval(); {body} std::cout << std::endl;\n"
        ));
    }
    tb.push_str("return 0; }\n");
    let tb_path = work.join("tb.cpp");
    std::fs::write(&tb_path, tb).unwrap();

    let out = verilator_command()
        .current_dir(&work)
        .args([
            "--cc", "--exe", "--build", "--top-module", top,
            "-Wno-DECLFILENAME", "-Wno-WIDTHEXPAND", "-CFLAGS", "-std=c++14",
        ])
        .arg(std::fs::canonicalize(sv_path).unwrap())
        .arg(&tb_path)
        .output()
        .unwrap();
    assert!(out.status.success(), "verilator build failed:\n{}", String::from_utf8_lossy(&out.stderr));

    let run = std::process::Command::new(work.join(format!("obj_dir/V{top}"))).output().unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout).into_owned();
    let _ = std::fs::remove_dir_all(&work);
    stdout
        .lines()
        .map(|l| l.split_whitespace().filter_map(|x| x.parse().ok()).collect())
        .collect()
}

/// Transpile `src`'s `top` to a temp `.sv` and run it, returning one column.
fn transpile_and_run(src: &str, top: &str, clk: &str, probe: &str, init: &str) -> Vec<u8> {
    let sv = copper_codegen::transpile_source(src, Some(top), &copper_codegen::EmitConfig::default())
        .expect("transpile");
    let dir = std::env::temp_dir().join(format!("copper_fwd_src_{top}_{}_{}", std::process::id(), TMP_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{top}.sv"));
    std::fs::write(&p, &sv).unwrap();
    let rows = run_sv(&p, top, clk, &[probe], init);
    let _ = std::fs::remove_dir_all(&dir);
    rows.into_iter().map(|r| r[0] as u8).collect()
}

const PULSE_REG_SRC: &str = r#"
#[hardware(sequential)]
async fn pulse_registered(clk: Clock<C>, dv: RegOut<Logic, C>) {
    loop {
        for _ in 0..3 { clk.tick().await; }
        dv.write(Logic::One);
        clk.tick().await;
        dv.write(Logic::Zero);
    }
}
"#;
#[hardware(sequential)]
async fn pulse_registered(clk: Clock<C>, dv: RegOut<Logic, C>) {
    loop {
        for _ in 0..3 { clk.tick().await; }
        dv.write(Logic::One);
        clk.tick().await;
        dv.write(Logic::Zero);
    }
}

fn sim_pulse_registered() -> Vec<u8> {
    let mut clk = Clock::<C>::new();
    let mut exec = HardwareExecutor::new();
    let (o, obs) = copper_core::port::registered_wire::<Logic, C>(&clk, Logic::Zero);
    let dh = o.dirty_handle();
    exec.spawn_wired(pulse_registered(clk.clone(), o), vec![dh], vec![]);
    (0..CYCLES)
        .map(|_| {
            exec.tick_clock(&mut clk);
            u8::from(obs.read() == Logic::One)
        })
        .collect()
}

/// The discriminator, isolated: identical design, `RegOut` instead of `Out`, and
/// the divergence disappears. This is what makes the diagnostic's advice correct.
#[test]
fn the_registered_output_form_agrees() {
    let sim = sim_pulse_registered();
    if !verilator_available() {
        return;
    }
    let sv = transpile_and_run(PULSE_REG_SRC, "pulse_registered", "clk", "dv", "");
    assert_eq!(
        sim, sv,
        "the `RegOut` form must stay sim ≡ SV — it is the remedy the guard points at"
    );
}


//! Generates one differential equivalence test per `#[hardware]` module in
//! `tests/fixtures/` **and** `examples/` — phases 2 and 3 of
//! `design_docs/CORPUS_DIFFERENTIAL_SWEEP.md`.
//!
//! Each generated case runs the module in the simulator under seeded random
//! stimulus and Verilates the SystemVerilog it transpiles to against that trace,
//! cycle by cycle. No reference model is involved: the simulator and the emitted SV
//! are two independent implementations of one source, so comparing them is already
//! an oracle. That is what makes the case *generatable*, and generatable is what
//! makes the coverage complete instead of "whatever anybody got round to".
//!
//! The output is `$OUT_DIR/corpus_generated.rs`, `include!`d by
//! `tests/corpus_generated.rs`. **A skipped module still gets a test**, marked
//! `#[ignore]` with its reason, because a check that silently does not run is this
//! repo's most-repeated bug class (see the G-A/G-B/G-C guards in
//! `tools/regression.sh`) and a sweep is exactly the wrong place to reintroduce it.
//!
//! # Why a build script and not a proc macro
//!
//! A proc macro reading files off disk has no rebuild tracking: edit a fixture and
//! its generated test silently stays stale. `cargo:rerun-if-changed` below is the
//! whole reason this is a build script.
//!
//! # Two kinds of file
//!
//! A **fixture** declares no imports and no clock domains — deliberately, so the
//! including test picks them — so the wrapper supplies both. An **example** is a
//! standalone program and brings its own, so the wrapper supplies neither and would
//! collide if it did. The predicate is the file's own content (`use` items,
//! `impl ClockDomain for`), not which directory it came from.
//!
//! Everything the generated *body* needs is imported under `__`-prefixed aliases, so
//! it cannot collide with whatever the included file already imports.
//!
//! # Port names
//!
//! The generated testbench addresses the Verilated model by the **emitted** name, and
//! a name colliding with a SystemVerilog or C++ keyword is legalized (`event` →
//! `event_sig`). The generated code calls `copper_codegen::legalized_port_name` at
//! run time rather than reimplementing that rule — two copies of a naming rule that
//! must agree is the drift bug this repo keeps recording.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use quote::ToTokens;

/// Modules the sweep must not run, each with the reason it cannot. Kept here, in
/// version control and in review, rather than inferred: "this one is expected to
/// fail" is a claim that should have an author and a sentence.
const SKIP: &[(&str, &str)] = &[
    (
        "probe",
        "an `allow_pretick_alignment` witness: the module exists to DEMONSTRATE the pre-tick \
         divergence, so sim ≠ SV is its purpose. probe_timing_investigation.rs measures it",
    ),
    (
        "rv32i_cpu_pipelined",
        "ANCHORED OUTSIDE THIS SWEEP as of 2026-08-27: tests/rv32i_pipelined_verilator.rs \
         runs all 13 architectural programs on the simulator AND on the transpiled core \
         Verilated under a hand-written owner (the received-memory ABI's parent, WriteFirst \
         collision policy), comparing (program_counter, halted, a0) cycle-for-cycle through \
         the halt. THIS sweep still cannot cover it — its `Memory` PARAMETER cannot be \
         supplied by the harness (the Kind::Memory rule below) — so the dedicated lane is \
         the behavioural gate. Every transpile cause is closed; see the lane's header for \
         the timing rules it pinned (edge-form staging, ff-position array reads, the \
         RegOut halt outputs)",
    ),
    // ── The claim ledger: tests/fixtures/{bits_ops,signedness,aggregate_locals,
    // match_selector,out_phase,mem_address_width}_dut.rs. Each module below is a
    // KNOWN-WRONG lowering with a working twin in the same file that DOES sweep, so
    // a fix shows up as an entry that can be deleted. The failure mode named in each
    // reason is the one actually observed, not the one predicted.
    //
    // A row here CANNOT tell you when it has gone stale: the generated case's body is
    // replaced by a `panic!`, so `--ignored` proves nothing. Two rows sat here after
    // their lowerings were fixed (`bit_not_bits` — `!` on `Bits<N>` now emits `~` —
    // and `lit_width_in_ternary` — the literal now takes the port's width), found only
    // by a 2026-09-01 audit that re-transpiled them; the `bit_not_bits` REFERENCE
    // anchor had been inert the whole time. Re-transpile the module before trusting
    // a reason that names emitted text.
    (
        "out_from_reg_before_commit",
        "ONE-CYCLE DIVERGENCE, GUARDED as of 2026-08-27: the trailing clause of \
         `pretick_out_write_before_update` refuses the shape (a trailing plain-`Out` write \
         before the update of a register it reads publishes the previous generation for the \
         whole cycle); this fixture keeps `allow_pretick_alignment` as the demonstration. \
         Measured minimal as `v8t_stage_publish_then_load`; the landing-day catch was \
         module_composition_hybrid's three pipeline stages (sim-only tests), migrated to the \
         canonical write/read/tick/update spelling. `out_from_reg_after_commit` is the \
         agreeing flip and sweeps",
    ),
    (
        "vr_sink_mealy",
        "ONE-CYCLE DIVERGENCE, GUARDED as of 2026-10-20: a Mealy `ready = !full || drain` \
         computed into a `let can_take: Logic` temporary, written, and only then `full` \
         updated in the same segment — V8a with one rename, which \
         `pretick_out_write_before_update` did not see through until it learned to follow \
         same-segment temporaries (reaching definitions). MEASURED before the fix: `ready` \
         the only diverging port, the simulator at 1 against the SV at 0 on 15 of 200 \
         cycles (hardware leads by one — the emitted `assign ready = can_take` is a wire of \
         `full`'s Q, the simulator reads `full` pre-update). The macro now refuses the shape \
         (ui/fail/write_before_update_via_temp.rs); this fixture keeps \
         `allow_pretick_alignment` as the demonstration. `vr_sink_moore` is the registered \
         spelling and sweeps",
    ),
    (
        "vr_skid",
        "ONE-CYCLE DIVERGENCE, GUARDED as of 2026-10-20: the same temporary-routed Mealy \
         ready as `vr_sink_mealy` (`let ir: Logic = …; in_ready.write(ir); … full = …`) in a \
         one-entry skid buffer whose `out_valid`/`out_data` are written before any input \
         read (the agreeing V8c position) and sweep clean. MEASURED before the fix: \
         `in_ready` the only diverging port, simulator 1 against SV 0 on 20 of 200 cycles. \
         Kept with `allow_pretick_alignment` as the demonstration; the guard refuses it \
         without the opt-out",
    ),
    (
        "two_domain_top",
        "a `#[hardware(structural)]` parent: transpile-only by design, with no simulatable body \
         to drive (item 4 — the sim wires the hierarchy by hand)",
    ),
    (
        "mod8",
        "a precondition the sweep cannot keep, found by the multi-sequence stimulus policy on \
         2026-10-28: `a % b` with `b == 0` is X in hardware (its header says the divisor is kept \
         non-zero BY THE STIMULUS), and the sweep does no stimulus masking by design (design doc \
         §3.4). The single 64-vector seed never drew a zero divisor; the policy's all-zeros and \
         `only a ones` patterns always do, and two of its four seeds did — measured `o expected \
         255 got 0` (the simulator's all-X read back as 255 against Verilator's 2-state 0) at \
         `(sequence 4 \"pattern list\", cycle 0)`, `(sequence 1, cycle 59)`, `(sequence 2, cycle \
         27)`; every vector with a non-zero divisor agrees. Not a divergence of the lowering: \
         comparing an undefined value against an undefined value is a test of nothing (the RESET \
         table's rationale), and the remainder operator stays covered by the hand-written vectors \
         of tests/wide_and_mod_datapath_equivalence.rs. An X-aware comparison (the simulator's X as don't-care) \
         would restore this module to the sweep and is the alternative recorded, not taken: it \
         would turn every X the simulator ever emits into a silent pass",
    ),
    (
        "branch_merge_explicit",
        "a demonstration of the pre-tick hazard since 2026-08-25: its conditionally-written \
         constant `tail_o` leads the identical emitted SV by one cycle when the FIRST cycle takes \
         the `sel == 1` path (measured 2026-10-26: sim tail_o = 1 at cycle 0, SV = 0). `tail_o` is \
         written both behind the deferred `sel` read (closing) and in the `pc == 1` arm (opening), \
         a mixed-anchor port that stays edge-sampled, so the arm-granular opening walk of \
         2026-10-26 — which fixed the minimal pair \
         sequential_forwarding_divergence.rs::a_write_in_a_state_arm_agrees_under_arm_granular_opening \
         and this module's `head_o` — does not reach it. Until 2026-10-28 the sweep's single seed \
         took `sel == 0` first, under which the lag is invisible; under the multi-sequence stimulus \
         policy the sweep exposes it directly (the `first sel=1` sequence fails, exactly as it does \
         for the live twin `branch_merge` — its own row below), so this row is the record of a \
         measured divergence, not a guess. The re-derived D1 rule (2026-10-28, clause A: mixed \
         anchors) REJECTS `tail_o`; the module keeps `allow_pretick_alignment` because it exists \
         to demonstrate it",
    ),
    (
        "branch_merge",
        "ONE-CYCLE DIVERGENCE, MEASURED BY THIS SWEEP 2026-10-28 under the multi-sequence stimulus \
         policy (design doc §3.2): the live, UNFLAGGED async twin of `branch_merge_explicit` \
         (byte-identical SystemVerilog). Its `tail_o` leads the SV by one cycle, once, whenever \
         the FIRST cycle takes `sel == 1` — sim `tail_o = 1` at that cycle, SV `0`, agreement from \
         the next cycle on; `head_o` and `mid_o` agree throughout. Exposed by the first-cycle \
         sequence `first sel=1` (sequence 5 of 6 — the historical seed, sequence 0, drives \
         `sel == 0` first and had masked it since 2026-08-25, guardrail §5.5) and by two of the \
         four seeds (1 and 2, which also open with `sel == 1`): \
         `FAIL: Cycle 832 tail_o expected 1 got 0 (sequence 5 \"first sel=1\", cycle 0)`. Pinned \
         with its vectors in \
         sequential_forwarding_divergence.rs::branch_merge_tail_leads_its_sv_when_the_first_cycle_takes_sel_one. \
         No D1 clause reaches the async spelling (its one `tail_o` write sits after the branch \
         merge, behind a read on every path), so the module compiles without an opt-out; the \
         mechanism is the explicit twin's mixed-anchor `tail_o` under control extraction's \
         duplication. A FINDING, not a fix: no rule and no emission changed for it",
    ),
];

/// The monomorphization to sweep a **generic** module at, in signature order.
///
/// A generic module transpiles to a *parametric* SystemVerilog module, so it can be
/// swept — Verilated with `-G` at the same widths the simulator runs, exactly as the
/// hand-written tests do. What cannot be inferred is *which* widths: the parameters
/// are often constrained (`N_LOG == clog2(N)`, asserted in the module itself), so a
/// guess is a compile error at best. The values here are the ones the corresponding
/// hand-written equivalence tests use, so the sweep and the vector test exercise the
/// same shape.
///
/// A generic module missing from this table is ignored with a reason that says so.
const PARAMS: &[(&str, &[(&str, i64)])] = &[
    ("det_for", &[("N", 3)]),
    ("mux", &[("WIDTH_P", 8), ("ELS_P", 4), ("LG_ELS_LP", 2)]),
    ("priority_encode", &[("N", 8), ("N_LOG", 3)]),
    ("ripple_carry_adder", &[("N", 8)]),
    ("rotate_right", &[("N", 8), ("N_LOG", 3)]),
    ("shift_register", &[("N", 8), ("N_1", 7)]),
    ("wide_alu", &[("N", 32)]),
];

/// Modules whose state is **undefined until reset**, and the port that resets them:
/// `(module, port, active_low)`. Cycle 0 drives the port to its asserted value; every
/// cycle after that is random, so the reset itself keeps getting exercised.
///
/// This is not a workaround. `shift_register` initialises its register to `Bits::x()`
/// — deliberately, because an unreset flip-flop is X in hardware — and the two
/// implementations then legitimately DISAGREE about what X reads as (the simulator
/// carries X, Verilator's 2-state model reads 0). Sweeping the pre-reset window
/// compares undefined behaviour against undefined behaviour, which is a test of
/// nothing. A design that needs a reset gets one.
/// An **independent hand-written Verilog reference** for a module, by name.
///
/// The sweep's ordinary check is simulator vs the SystemVerilog the transpiler
/// emitted: two implementations of ONE source. That is an oracle for "the two
/// agree" and nothing more — a misconception shared between the executor and the
/// lowering is invisible to it. A reference nobody derived from either closes that
/// gap, and a module listed here is checked against all three.
///
/// **Provenance is part of the claim and belongs in the file's header.** A
/// reference written by whoever wrote the Copper module catches lowering and
/// transcription errors but shares that person's model of the design; only a
/// genuinely third-party source (BaseJump STL and the like) buys independence.
/// Third-party is preferred wherever one exists — see `examples/basejump/sv/` for
/// the header format, including what was adapted and why.
///
/// The referenced file's SystemVerilog module must be NAMED FOR THE COPPER MODULE:
/// it is Verilated as the top exactly as the transpiler's output is. Vendored
/// third-party code is therefore adapted (renamed ports, concrete parameter
/// defaults), which the vendored BaseJump files already do and record.
///
/// A module NOT listed here is unanchored, and `tools/regression.sh` prints how
/// many there are on every run — the gap is tracked rather than rediscovered.
const REFERENCE: &[(&str, &str)] = &[
    ("ram_read_first", "tests/fixtures/reference_sv/ram_read_first.sv"),
    // This reference is what proved the original `!`-on-`Bits<N>` bug was the
    // TRANSPILER's and not the simulator's (it agreed with the sim and disagreed with
    // the emitted SV). Live since the module's SKIP row was deleted on 2026-09-01.
    ("bit_not_bits", "tests/fixtures/reference_sv/bit_not_bits.sv"),
    // The paper's §2.1 listing. Hand-written as the explicit three-state machine a
    // Verilog designer writes; anchors BOTH `handshake` spellings (while_wait's and
    // wait_loop's), which tests/handshake_equivalence.rs shows are cycle-identical.
    ("handshake", "tests/fixtures/reference_sv/handshake.sv"),
];

const RESET: &[(&str, &str, bool)] = &[
    ("shift_register", "rstn", true),
    // Both found by the multi-sequence stimulus policy on 2026-10-28, on its first
    // run: the single seed each had before happened to assert `reset_i` on cycle 0,
    // so the pre-reset state was never observed. `simple_counter` starts from
    // `Bits::<8>::x()` — the shift_register case exactly (sim X, read back as 255,
    // against Verilator's 0: `count_o expected 255 got 0` on every sequence whose
    // first cycle had `reset_i == 0`). `lfsr` (both copies, same source) starts from
    // a RUST initial value, `let mut state = Bits::from_u32(1)`, which the emitted
    // SystemVerilog does not carry — its flop has no initializer and no reset value
    // outside the `reset_i` branch, so the Verilated model starts at 0 and, being an
    // LFSR, stays there until reset: measured `o expected 1 got 0` with
    // `reset_i == 0, yumi_i == 0` first, and `o expected 2785017856 got 0` (one shift
    // of 1 under the taps) with `yumi_i == 1` first — pinned in
    // tests/lfsr_equivalence.rs. A pre-reset flop is X in hardware, so the row
    // records the measured difference and starts the comparison at the reset, as
    // the table has always done; the Rust initializer's reach into the SV is a
    // transpiler question recorded here, not resolved here.
    ("lfsr", "reset_i", false),
    ("simple_counter", "reset_i", false),
    // Held low the core is idle and its boot port owns the memory's write bus, so the
    // sweep's random stimulus reaches a defined state instead of X. This is the first
    // CPU-scale design in the sweep; it got here when `vlir_lower` started casting
    // memory addresses to the address-net width (TODO cause Q).
    ("rv32i_cpu_transpilable", "rstn", true),
];

// ── The multi-sequence stimulus policy (2026-10-28) ────────────────────────────
//
// `design_docs/CORPUS_DIFFERENTIAL_SWEEP.md` §3.2. Until 2026-10-28 every module was
// driven by ONE seeded sequence, so its verdict rested on one stimulus and a
// divergence that depends on the initial input pattern was invisible —
// `branch_merge` passed by its seed alone (its SKIP row). Each module is now driven
// by several independent sequences, back to back in one testbench run of ONE
// Verilated model (a fresh model instance and a fresh executor per sequence), so the
// cost stays one Verilator build per module. The constants are here; the plan is
// built at run time by `tests/common::StimulusPlan` (a width written as a file-scope
// `const` is not known to this script), and pinned by
// `tests/corpus_generated.rs::the_stimulus_policy_is_what_the_design_doc_says`.

/// Independent random sequences per module. Sequence 0 uses the historical
/// per-module seed (`seed_of`), so the pre-policy stimulus is sequence 0 verbatim.
const SEEDS: usize = 4;
/// Cycles per seed sequence (a clocked module).
const SEED_CYCLES: usize = 200;
/// Cycles per first-cycle sequence: the chosen initial vector on the first live
/// cycle, random stimulus after.
const FIRST_CYCLES: usize = 32;
/// A clocked module with at most this many input bits (its RESET port excluded)
/// starts one sequence from EVERY initial input vector (≤ 16 sequences); a wider
/// one from the fixed pattern list (all zeros, all ones, each port alone ones,
/// each port alone zeros).
const EXHAUSTIVE_BITS: usize = 4;
/// A combinational module with at most this many input bits is driven by its whole
/// truth table (≤ 256 vectors) — strictly stronger than random vectors.
const COMB_EXHAUSTIVE_BITS: usize = 8;
/// Random input vectors per seed for a wider combinational module (no clock, so
/// `poll_tasks`); the pattern list is appended as further vectors.
const COMB_VECTORS: usize = 64;

fn main() {
    println!("cargo:rerun-if-changed=tests/fixtures");
    println!("cargo:rerun-if-changed=examples");
    println!("cargo:rerun-if-changed=build.rs");

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));

    let mut files: Vec<PathBuf> = Vec::new();
    collect_rs(&manifest.join("tests/fixtures"), &mut files);
    collect_rs(&manifest.join("examples"), &mut files);
    files.sort();

    let mut code = String::from(
        "// @generated by build.rs — phase 2 of design_docs/CORPUS_DIFFERENTIAL_SWEEP.md.\n\
         // Edit build.rs, not this file.\n\n",
    );
    let (mut emitted, mut ignored) = (0usize, 0usize);
    let mut covered: Vec<String> = Vec::new();

    for path in &files {
        let src = std::fs::read_to_string(path).expect("fixture is readable");
        let file = match syn::parse_file(&src) {
            Ok(f) => f,
            // A fixture that does not parse standalone is not this script's problem
            // to report — the test that includes it will say so far more clearly.
            Err(_) => continue,
        };
        let modules: Vec<Module> = file.items.iter().filter_map(Module::from_item).collect();
        if modules.is_empty() {
            continue;
        }
        let key = wrapper_name(&manifest, path);
        let (m, i) = emit_file(&mut code, path, &src, &modules, &key);
        emitted += m;
        ignored += i;
        covered.extend(modules.iter().map(|m| format!("{key}::{}", m.name)));
    }

    // The manifest the coverage guard checks against: every module this script
    // emitted a case for, ignored ones included. A generator that quietly stops
    // covering something is the failure mode this whole file exists to prevent, so
    // it is asserted in the test binary rather than trusted.
    let _ = writeln!(
        code,
        "/// Every fixture module `build.rs` emitted a case for (ignored ones too).\n\
         pub const COVERED: &[&str] = &[{}];",
        covered.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(", ")
    );

    // Validate the REFERENCE table before anything uses it. Every failure mode here
    // is silent otherwise: a mistyped module name anchors NOTHING and the sweep goes
    // on passing, which is this repo's signature bug in a new place. Checked at build
    // time so the mistake is reported where it was made rather than as a confusing
    // Verilator error twenty seconds later.
    for (module, path) in REFERENCE {
        let leaves: Vec<&str> =
            covered.iter().map(|n| n.rsplit("::").next().unwrap_or(n)).collect();
        if !leaves.contains(module) {
            // Near-misses only. Printing all ~130 module names buries the one thing
            // the reader needs, which is "did I typo it".
            let pre = &module[..module.len().min(4)];
            let mut near: Vec<&str> =
                leaves.iter().copied().filter(|l| l.starts_with(pre)).collect();
            near.sort_unstable();
            near.dedup();
            let hint = if near.is_empty() {
                format!("no module starts with `{pre}`; {} are covered", leaves.len())
            } else {
                format!("did you mean: {}", near.join(", "))
            };
            panic!(
                "REFERENCE names `{module}`, which is not a module the sweep covers. \
                 A typo here anchors nothing and every test still passes. {hint}"
            );
        }
        let full = manifest.join(path);
        assert!(
            full.is_file(),
            "REFERENCE for `{module}` points at `{path}`, which does not exist \
             (looked in {})",
            full.display()
        );
        let sv = std::fs::read_to_string(&full)
            .unwrap_or_else(|e| panic!("REFERENCE for `{module}`: cannot read {path}: {e}"));
        assert!(
            sv.contains(&format!("module {module}")),
            "REFERENCE for `{module}` is `{path}`, which does not declare `module {module}`. \
             Verilator is run with `--top-module {module}`, so the reference's module must be \
             NAMED FOR THE COPPER MODULE — vendored third-party code is renamed for this, which \
             the files in examples/basejump/sv/ do and record in their headers"
        );
        assert!(
            !sv.contains("// @generated"),
            "REFERENCE for `{module}` points at generated output. A reference derived from the \
             transpiler cannot anchor the transpiler"
        );
        println!("cargo:rerun-if-changed={path}");
    }

    // Validate the SKIP table the same way. A row whose module no longer exists is
    // not a skip, it is a sentence nobody reads — `rv32i_cpu` sat here for days after
    // the file it named was deleted (2026-08-26 → 2026-09-01), with its reason still
    // describing a `Vec<Bits<32>>` port that was long gone.
    {
        let leaves: Vec<&str> =
            covered.iter().map(|n| n.rsplit("::").next().unwrap_or(n)).collect();
        for (module, _) in SKIP {
            assert!(
                leaves.contains(module),
                "SKIP names `{module}`, which is not a module the sweep covers — the row is \
                 dead. Delete it (or fix the name) so the table stays a list of reviewed \
                 exclusions rather than a place stale reasons go to be believed"
            );
        }
    }

    // The anchoring ledger. A module cross-checked only against the transpiler's own
    // output is verified for CONSISTENCY; one checked against an independent
    // reference is verified for SEMANTICS. Keeping the two counts apart is the point
    // — see REFERENCE above — and G-E in tools/regression.sh prints the remainder so
    // it stays visible instead of being rediscovered by an audit.
    //
    // A module that is both REFERENCE'd and SKIP'd is NOT counted as anchored: its
    // reference leg never runs, so the row is inert. (`bit_not_bits` was exactly that
    // for a week, and G-E over-reported by one the whole time.)
    let anchored = covered
        .iter()
        .filter(|n| {
            let leaf = n.rsplit("::").next().unwrap_or(n);
            REFERENCE.iter().any(|(m, _)| *m == leaf) && !SKIP.iter().any(|(m, _)| *m == leaf)
        })
        .count();
    let _ = writeln!(
        code,
        "/// Modules checked against an INDEPENDENT hand-written Verilog reference.\n\
         pub const ANCHORED: usize = {anchored};\n\
         /// Swept modules with no independent reference — cross-checked against the\n\
         /// transpiler's own output only, so consistent but not externally anchored.\n\
         pub const UNANCHORED: usize = {};",
        covered.len() - anchored
    );

    // The policy's constants, for the generated cases and for the pin — one
    // definition, so the generator and the plan cannot disagree about them.
    let _ = writeln!(
        code,
        "/// The stimulus policy's constants — design_docs/CORPUS_DIFFERENTIAL_SWEEP.md §3.2.\n\
         pub const BUDGET: crate::common::StimulusBudget = crate::common::StimulusBudget {{\n    \
         seeds: {SEEDS}, seed_cycles: {SEED_CYCLES}, first_cycles: {FIRST_CYCLES},\n    \
         exhaustive_bits: {EXHAUSTIVE_BITS}, comb_exhaustive_bits: {COMB_EXHAUSTIVE_BITS},\n    \
         comb_vectors: {COMB_VECTORS},\n}};"
    );

    // Surfaced in the build log too, so the counts are visible without running.
    println!("cargo:warning=corpus sweep: {emitted} generated, {ignored} ignored-with-reason");
    println!(
        "cargo:warning=stimulus policy: {SEEDS} seeds x {SEED_CYCLES} cycles per module, plus a \
         {FIRST_CYCLES}-cycle sequence from every initial input vector up to {EXHAUSTIVE_BITS} input \
         bits (a first-cycle pattern list above); combinational: the whole truth table up to \
         {COMB_EXHAUSTIVE_BITS} bits, else {SEEDS} x {COMB_VECTORS} vectors + the pattern list"
    );
    let unanchored_names: Vec<&str> = covered
        .iter()
        .filter(|n| {
            let leaf = n.rsplit("::").next().unwrap_or(n);
            !REFERENCE.iter().any(|(m, _)| *m == leaf)
        })
        .map(|n| n.as_str())
        .collect();
    println!(
        "cargo:warning=anchoring: {anchored} module(s) checked against an independent reference, \
         {} cross-checked against the transpiler only",
        unanchored_names.len()
    );
    // Named once the list is short enough to read. While it is the whole corpus the
    // count is the useful number; the names become the useful thing as the remainder
    // shrinks, which is the direction this is meant to move.
    if !unanchored_names.is_empty() && unanchored_names.len() <= 25 {
        println!("cargo:warning=unanchored: {}", unanchored_names.join(", "));
    }
    std::fs::write(out_dir.join("corpus_generated.rs"), code).expect("write generated tests");
}

/// Every `.rs` under `dir`, recursively (`examples/` has subdirectories).
///
/// `old/` directories are excluded — the convention for set-aside scratch whose
/// modules must not enter the sweep (the deleted `examples/cpu/old/` pre-subset
/// copies, with `Vec` ports the admissible grammar rejects, were the original
/// case; sweeping them generated corpus cases that could not compile).
/// `tools/regression.sh`'s G-A guard prunes the same way.
fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.filter_map(Result::ok) {
        let p = entry.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "old") {
                continue;
            }
            collect_rs(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

/// A unique Rust identifier for one source file's wrapper module, from its path
/// relative to the manifest — `examples/cdc/flag_crossing.rs` → `ex_cdc_flag_crossing`.
/// Path-derived rather than stem-derived because two files legitimately share a stem,
/// and because two different modules can share a NAME (`fast_counter` exists in both
/// `two_domain_counter.rs` and `two_domain_hierarchy.rs`).
fn wrapper_name(manifest: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(manifest).unwrap_or(path).with_extension("");
    let mut s: String = rel
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    s = s
        .trim_start_matches("tests_fixtures_")
        .trim_start_matches("examples_")
        .to_string();
    let prefix = if path.components().any(|c| c.as_os_str() == "examples") { "ex_" } else { "fx_" };
    format!("{prefix}{s}")
}

/// One `#[hardware]` module, as much of it as generation needs.
struct Module {
    name: String,
    /// Const-generic parameter names in signature order, empty for a concrete module.
    /// A generic module emits a PARAMETRIC SystemVerilog module, so it can be swept —
    /// at the widths `PARAMS` records for it.
    generics: Vec<String>,
    ports: Vec<Port>,
}

struct Port {
    name: String,
    /// The payload type as written (`Bits<8>`, `Logic`), or the domain for a clock.
    ty: String,
    domain: String,
    kind: Kind,
}

#[derive(PartialEq)]
enum Kind {
    Clock,
    In,
    Out,
    RegOut,
    /// A `Memory<T, R, W, D, RL, WL>` the module RECEIVES rather than declares.
    /// Not a port: no wire is made for it and no stimulus is generated. It exists
    /// as a variant so `classify` can return `Some` for it — see the note there.
    Memory,
}

impl Module {
    fn from_item(item: &syn::Item) -> Option<Module> {
        let syn::Item::Fn(f) = item else { return None };
        if !f.attrs.iter().any(|a| {
            a.path().segments.last().is_some_and(|s| s.ident == "hardware")
        }) {
            return None;
        }
        let mut ports = Vec::new();
        for arg in &f.sig.inputs {
            let syn::FnArg::Typed(pt) = arg else { return None };
            let name = pt.pat.to_token_stream().to_string();
            let (kind, ty, domain) = classify(&pt.ty)?;
            ports.push(Port { name, ty, domain, kind });
        }
        let generics = f
            .sig
            .generics
            .params
            .iter()
            .filter_map(|p| match p {
                syn::GenericParam::Const(c) => Some(c.ident.to_string()),
                _ => None,
            })
            .collect();
        Some(Module { name: f.sig.ident.to_string(), generics, ports })
    }

    fn clock(&self) -> Option<&Port> {
        self.ports.iter().find(|p| p.kind == Kind::Clock)
    }

    /// The recorded monomorphization, if this module needs one and has one. Every
    /// declared parameter must be covered — a partial entry would monomorphize to
    /// something the author did not choose.
    fn params(&self) -> Option<&'static [(&'static str, i64)]> {
        if self.generics.is_empty() {
            return None;
        }
        let entry = PARAMS.iter().find(|(n, _)| *n == self.name)?.1;
        self.generics
            .iter()
            .all(|g| entry.iter().any(|(n, _)| n == g))
            .then_some(entry)
    }

    /// The reason this module cannot be swept, if any.
    fn skip_reason(&self, qualified: &str) -> Option<String> {
        // Matched on the bare name, or on `<wrapper>::<name>` when only one COPY of a
        // module is affected — `ripple_carry_adder` exists twice and only the example
        // copy uses the construct that refuses.
        if let Some((_, why)) = SKIP
            .iter()
            .find(|(n, _)| *n == self.name || n.rsplit("::").next() == Some(self.name.as_str()) && *n == qualified)
        {
            return Some((*why).to_string());
        }
        if !self.generics.is_empty() && self.params().is_none() {
            return Some(format!(
                "generic module with no monomorphization recorded: add `(\"{}\", &[{}])` to \
                 build.rs's PARAMS table. The parameters are usually constrained (a module can \
                 `const assert!` a relation between them), so the widths are a decision, not \
                 something to infer",
                self.name,
                self.generics
                    .iter()
                    .map(|g| format!("(\"{g}\", ?)"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if self.ports.iter().any(|p| p.kind == Kind::Memory) {
            return Some(
                "receives a `Memory<…>` parameter: the sweep would have to invent its size and \
                 its contents, and both are design decisions (a ROM's contents ARE the design). \
                 Same disposition as PARAMS — a memory-taking module is swept by a hand-written \
                 test that states them, not by generated stimulus"
                    .to_string(),
            );
        }
        if self.ports.iter().filter(|p| p.kind == Kind::Clock).count() > 1 {
            return Some(
                "more than one clock port: the generated harness drives a single clock, and the \
                 ratio between two domains is a design decision the sweep must not invent — \
                 phase 4"
                    .to_string(),
            );
        }
        if self.ports.iter().any(|p| p.kind != Kind::Clock && !is_stimulable(&p.ty)) {
            return Some(format!(
                "unsupported port payload: the generator can only randomise `Logic` and \
                 `Bits<N>` ({})",
                self.ports
                    .iter()
                    .filter(|p| p.kind != Kind::Clock && !is_stimulable(&p.ty))
                    .map(|p| format!("{}: {}", p.name, p.ty))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        None
    }
}

/// `In<Bits<8>, MainClk>` → `(Kind::In, "Bits<8>", "MainClk")`; `Clock<D>` → the domain.
///
/// **Returning `None` here deletes a module from the sweep.** `Module::from_item`
/// propagates it with `?`, so an unclassifiable parameter does not make the module
/// un-sweepable-with-a-reason — it makes the module invisible, which is the one
/// outcome the SKIP table exists to prevent. That is what happened when `Memory`
/// first became a legal parameter type: the pipelined CPU dropped out of the
/// corpus silently and only `every_corpus_module_has_a_generated_case` (which
/// scans for `#[hardware]` by attribute, NOT through this function) noticed.
/// So a new parameter kind belongs here first, and in `skip_reason` second.
fn classify(ty: &syn::Type) -> Option<(Kind, String, String)> {
    let syn::Type::Path(tp) = ty else { return None };
    let seg = tp.path.segments.last()?;
    let kind = match seg.ident.to_string().as_str() {
        "Clock" => Kind::Clock,
        "In" => Kind::In,
        "Out" => Kind::Out,
        "RegOut" => Kind::RegOut,
        "Memory" => Kind::Memory,
        _ => return None,
    };
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else { return None };
    let text: Vec<String> = args
        .args
        .iter()
        .map(|a| a.to_token_stream().to_string().replace(' ', ""))
        .collect();
    match kind {
        Kind::Clock => Some((kind, text.first()?.clone(), text.first()?.clone())),
        // `Memory<T, R, W, D, RL, WL>` — the domain is the 4th argument, not the
        // 2nd, because R and W sit between the element type and it.
        Kind::Memory => Some((kind, text.first()?.clone(), text.get(3)?.clone())),
        _ => Some((kind, text.first()?.clone(), text.get(1)?.clone())),
    }
}

/// Can seeded random stimulus be generated for this payload? `RandStim` covers
/// `Logic`, `Bits<N>` for any `N` — including a width written as a file-scope `const`
/// or a const-generic parameter, both of which are in scope in the generated body —
/// and arrays of either.
fn is_stimulable(ty: &str) -> bool {
    if ty == "Logic" {
        return true;
    }
    if let Some(inner) = ty.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        // `[Bits<W>;N]` — the element must be stimulable; the length is a width-like
        // identifier or a literal, and is checked by the compiler, not here.
        return inner.rsplit_once(';').is_some_and(|(elem, _)| is_stimulable(elem.trim()));
    }
    ty.strip_prefix("Bits<")
        .and_then(|r| r.strip_suffix('>'))
        .is_some_and(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

/// A zero of the payload type, for the wire's initial value.
fn zero_of(ty: &str) -> String {
    if ty == "Logic" {
        "__Logic::Zero".to_string()
    } else if ty.starts_with('[') {
        // `[T; N]`: `from_fn` rather than `[zero; N]`, which would need `Copy` in a
        // const context the element type does not necessarily satisfy.
        let elem = ty[1..ty.len() - 1].rsplit_once(';').map(|(e, _)| e.trim()).unwrap_or("Logic");
        format!("std::array::from_fn(|_| {})", zero_of(elem))
    } else {
        "__Bits::zero()".to_string()
    }
}

/// A stable per-module seed, so two modules do not walk the same bit pattern and a
/// failure is reproducible from the name alone (FNV-1a).
fn seed_of(name: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in name.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x1000_0000_01b3);
    }
    h | 1
}

/// Emit the wrapper module for one source file plus a test per hardware module.
/// Returns `(generated, ignored)`.
fn emit_file(
    code: &mut String,
    path: &Path,
    src: &str,
    modules: &[Module],
    wrapper: &str,
) -> (usize, usize) {
    let file_path = path.to_string_lossy();

    // An `include!` cannot produce inner doc comments (`//!` may not follow an item,
    // and a macro expansion cannot emit one at all), so such a file will not compile
    // included. Every fixture and every example uses `//` headers today; if one
    // regresses, say so in the ignore reason rather than emitting a case that breaks
    // the build.
    let inner_doc = src.trim_start().starts_with("//!");

    // A file that brings its own imports and clock domains is an EXAMPLE — a
    // standalone program. Supplying them again would collide. A fixture brings
    // neither, deliberately, so the wrapper supplies both. The predicate is content,
    // not directory.
    let has_own_imports = src.lines().any(|l| l.trim_start().starts_with("use "));
    // Spelled three ways in the corpus — `impl ClockDomain for X {}`, the same with
    // no space before the braces, and `impl copper_core::ClockDomain for X {}` — so
    // match on the phrase and take the identifier that follows, rather than on a
    // prefix.
    let declared_domains: BTreeSet<String> = src
        .lines()
        .filter_map(|l| l.split_once("ClockDomain for "))
        .map(|(_, rest)| {
            rest.trim()
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect::<String>()
        })
        .filter(|d| !d.is_empty())
        .collect();

    let needed_domains: BTreeSet<&str> = modules
        .iter()
        .flat_map(|m| m.ports.iter())
        .map(|p| p.domain.as_str())
        .filter(|d| *d != "()" && !declared_domains.contains(*d))
        .collect();

    let _ = writeln!(code, "#[allow(unused_imports, dead_code)]");
    let _ = writeln!(code, "mod {wrapper} {{");
    // Aliased, so nothing here can clash with what the included file imports.
    let _ = writeln!(
        code,
        "    use copper_core::port::{{registered_wire as __rw, wire as __w}};\n    \
         use copper_core::types::Bits as __Bits;\n    \
         use copper_core::{{Clock as __Clock, Logic as __Logic}};\n    \
         use copper_sim::HardwareExecutor as __Exec;\n    \
         use crate::common::{{EquivalenceTest as __Eq, RandStim as __RandStim, Rng as __Rng, \
         StimulusPlan as __Plan}};"
    );
    if !has_own_imports {
        // A fixture: it needs the hardware vocabulary to compile at all.
        let _ = writeln!(
            code,
            "    use copper_core::port::{{registered_wire, wire, In, Out, RegOut}};\n    \
             use copper_core::types::Bits;\n    \
             use copper_core::{{Clock, ClockDomain, Logic, Memory}};\n    \
             use copper_macros::hardware;\n    \
             use copper_sim::HardwareExecutor;"
        );
    }
    for d in &needed_domains {
        // Fully qualified: a file that needs a domain declared is one that did not
        // import `ClockDomain` either.
        let _ = writeln!(
            code,
            "    struct {d};\n    impl copper_core::ClockDomain for {d} {{}}"
        );
    }
    if !inner_doc {
        let _ = writeln!(code, "    include!(\"{file_path}\");");
    }
    let _ = writeln!(code, "    const SRC: &str = include_str!(\"{file_path}\");\n");

    let (mut emitted, mut ignored) = (0, 0);
    for m in modules {
        let reason = if inner_doc {
            Some(
                "the file opens with `//!` module docs, which `include!` cannot carry — an inner \
                 doc comment may not follow an item. Give it a `//` header, as every other \
                 fixture and example has"
                    .to_string(),
            )
        } else {
            m.skip_reason(&format!("{wrapper}::{}", m.name))
        };
        match reason {
            Some(reason) => {
                let _ = writeln!(code, "    #[ignore = \"{}\"]", escape(&reason));
                emit_test(code, m, /*body=*/ false);
                ignored += 1;
            }
            None => {
                emit_test(code, m, true);
                emitted += 1;
            }
        }
    }
    let _ = writeln!(code, "}}\n");
    (emitted, ignored)
}

/// The test itself. With `body == false` the case is `#[ignore]`d and the body is a
/// single `panic!` — it must still *exist* (so the skip is listed and counted), and
/// it must not be silently green if someone removes the `#[ignore]`.
fn emit_test(code: &mut String, m: &Module, body: bool) {
    let name = &m.name;
    let _ = writeln!(code, "    #[test]");
    let _ = writeln!(code, "    fn {name}_differential() {{");
    if !body {
        let _ = writeln!(
            code,
            "        panic!(\"skipped by build.rs's SKIP table — see the #[ignore] reason\");"
        );
        let _ = writeln!(code, "    }}\n");
        return;
    }

    let clk = m.clock();
    let params = m.params();

    // The monomorphization, as local bindings the port types and the call both use.
    for (n, v) in params.unwrap_or(&[]) {
        let _ = writeln!(code, "        const P_{n}: usize = {v};");
    }

    let with_reference = match REFERENCE.iter().find(|(n, _)| *n == m.name) {
        Some((_, path)) => format!("\n            .with_hand_written_reference(\"{path}\")"),
        None => String::new(),
    };
    let with_params = match params {
        Some(ps) => format!(
            "\n            .with_params(&[{}])",
            ps.iter().map(|(n, v)| format!("(\"{n}\", {v})")).collect::<Vec<_>>().join(", ")
        ),
        None => String::new(),
    };
    let _ = writeln!(
        code,
        "        let mut eq = __Eq::differential_only(\"{name}\", SRC, Some(\"{name}\")){with_params}{with_reference};"
    );
    // The stimulus plan: the `In` ports in signature order with their widths, which
    // are resolved by the compiler (`RandStim::WIDTH`) rather than parsed here, so
    // a width written as a file-scope `const` or a generic parameter is exact.
    let ins_decl: Vec<String> = m
        .ports
        .iter()
        .filter(|p| p.kind == Kind::In)
        .map(|p| format!("(\"{}\", <{} as __RandStim>::WIDTH)", p.name, alias_ty(&p.ty, &m.generics)))
        .collect();
    let reset = RESET.iter().find(|(n, _, _)| *n == m.name);
    let reset_arg = match reset {
        Some((_, port, active_low)) => {
            let idx = m
                .ports
                .iter()
                .filter(|p| p.kind == Kind::In)
                .position(|p| p.name == *port)
                .unwrap_or_else(|| panic!("RESET names port `{port}` of `{name}`, which is not an `In` port"));
            format!("Some(({idx}usize, {active_low}))")
        }
        None => "None".to_string(),
    };
    let _ = writeln!(
        code,
        "        let mut __plan = __Plan::new(\"{name}\", {seed}u64, &[{ins}], {clocked}, {reset_arg}, crate::BUDGET);\n        \
         println!(\"{{}}\", __plan.summary());",
        seed = seed_of(name),
        ins = ins_decl.join(", "),
        clocked = clk.is_some(),
    );

    // The names the testbench will address the Verilated model by — the EMITTED
    // ones. Resolved by the transpiler's own rule at run time, not guessed here.
    for p in m.ports.iter().filter(|p| p.kind != Kind::Clock) {
        let _ = writeln!(
            code,
            "        let {n}_sv = copper_codegen::legalized_port_name(\"{n}\");",
            n = p.name
        );
    }

    // One sequence per iteration: a fresh executor, clock and wires, so the
    // simulator starts from the design's initial state exactly as the Verilated
    // model does at the boundary `begin_sequence` records.
    let _ = writeln!(code, "        while __plan.next_sequence() {{");
    let _ = writeln!(code, "            eq.begin_sequence(&__plan.label());");
    // Unused (and immutable) in a module with no `In` ports; the lint attribute keeps
    // such a case warning-free without a second code shape.
    let _ = writeln!(
        code,
        "            #[allow(unused_variables, unused_mut)]\n            let mut rng = __Rng::new(__plan.seed());"
    );
    if let Some(c) = clk {
        let _ = writeln!(code, "            let mut clk = __Clock::<{}>::new();", c.domain);
    }
    let _ = writeln!(code, "            let mut exec = __Exec::new();");

    for p in &m.ports {
        let (ty, z, d) = (alias_ty(&p.ty, &m.generics), zero_of(&p.ty), &p.domain);
        match p.kind {
            Kind::Clock => {}
            // `skip_reason` returns early for any module with one, so a memory
            // never reaches code generation. Spelled out rather than caught by a
            // wildcard so a future parameter kind fails to compile here instead of
            // being silently dropped on the floor.
            Kind::Memory => unreachable!("a module with a Memory parameter is skipped"),
            Kind::In => {
                let _ = writeln!(
                    code,
                    "            let ({n}_drv, {n}_in) = __w::<{ty}, {d}>({z});",
                    n = p.name
                );
            }
            Kind::Out => {
                let _ = writeln!(
                    code,
                    "            let ({n}_out, {n}_obs) = __w::<{ty}, {d}>({z});",
                    n = p.name
                );
            }
            Kind::RegOut => {
                let _ = writeln!(
                    code,
                    "            let ({n}_out, {n}_obs) = __rw::<{ty}, {d}>(&clk, {z});",
                    n = p.name
                );
            }
        }
    }

    let handles: Vec<String> = m
        .ports
        .iter()
        .filter(|p| matches!(p.kind, Kind::Out | Kind::RegOut))
        .map(|p| format!("{}_out.dirty_handle()", p.name))
        .collect();
    let reads: Vec<String> = m
        .ports
        .iter()
        .filter(|p| p.kind == Kind::In)
        .map(|p| format!("{}_in.wire_id()", p.name))
        .collect();
    let _ = writeln!(code, "            let handles = vec![{}];", handles.join(", "));
    let _ = writeln!(code, "            let reads = vec![{}];", reads.join(", "));

    // Arguments in SIGNATURE order — not inputs-then-outputs, which is merely the
    // usual convention and not a rule any module is obliged to follow.
    let args: Vec<String> = m
        .ports
        .iter()
        .map(|p| match p.kind {
            Kind::Clock => "clk.clone()".to_string(),
            Kind::In => format!("{}_in", p.name),
            Kind::Out | Kind::RegOut => format!("{}_out", p.name),
            Kind::Memory => unreachable!("a module with a Memory parameter is skipped"),
        })
        .collect();
    let turbofish = match params {
        Some(_) if !m.generics.is_empty() => format!(
            "::<{}>",
            m.generics.iter().map(|g| format!("P_{g}")).collect::<Vec<_>>().join(", ")
        ),
        _ => String::new(),
    };
    let _ = writeln!(
        code,
        "            exec.spawn_wired({name}{turbofish}({}), handles, reads);\n",
        args.join(", ")
    );

    // The cycle loop. Every `In` port asks the plan for its value, once per cycle in
    // signature order — the plan's reset, first-cycle-vector and random rules are
    // all in `StimulusPlan::stim`, not here.
    let _ = writeln!(code, "            for __cycle in 0..__plan.cycles() {{");
    for (i, p) in m.ports.iter().filter(|p| p.kind == Kind::In).enumerate() {
        let ty = alias_ty(&p.ty, &m.generics);
        let _ = writeln!(
            code,
            "                let {n}_v = __plan.stim::<{ty}>(__cycle, {i}usize, &mut rng);\n                \
             {n}_drv.write({n}_v);",
            n = p.name
        );
    }
    let _ = writeln!(
        code,
        "                {}",
        if clk.is_some() { "exec.tick_clock(&mut clk);" } else { "exec.poll_tasks();" }
    );
    for p in m.ports.iter().filter(|p| matches!(p.kind, Kind::Out | Kind::RegOut)) {
        let _ = writeln!(code, "                let {n}_v = {n}_obs.read();", n = p.name);
    }
    let ins: Vec<String> = m
        .ports
        .iter()
        .filter(|p| p.kind == Kind::In)
        .map(|p| format!("(&{n}_sv[..], &{n}_v.as_bits()[..])", n = p.name))
        .collect();
    let outs: Vec<String> = m
        .ports
        .iter()
        .filter(|p| matches!(p.kind, Kind::Out | Kind::RegOut))
        .map(|p| format!("(&{n}_sv[..], &{n}_v.as_bits()[..])", n = p.name))
        .collect();
    let _ = writeln!(
        code,
        "                eq.record_differential(&[{}], &[{}]);",
        ins.join(", "),
        outs.join(", ")
    );
    let _ = writeln!(code, "            }}");
    let _ = writeln!(code, "        }}");
    let _ = writeln!(code, "        eq.finish();");
    let _ = writeln!(code, "    }}\n");
}

/// The payload type spelled for the generated body: `Bits` and `Logic` under the
/// wrapper's aliases (so nothing depends on what the included file imported), and any
/// const-generic parameter under its local `P_` binding.
///
/// Token-wise, not by string replacement: `Bits<N_LOG>` must not be rewritten by a
/// rule for `N`, and a substring replace does exactly that.
fn alias_ty(ty: &str, generics: &[String]) -> String {
    let mut out = String::new();
    let mut ident = String::new();
    let flush = |ident: &mut String, out: &mut String| {
        if ident.is_empty() {
            return;
        }
        let mapped = match ident.as_str() {
            "Bits" => "__Bits".to_string(),
            "Logic" => "__Logic".to_string(),
            other if generics.iter().any(|g| g == other) => format!("P_{other}"),
            other => other.to_string(),
        };
        out.push_str(&mapped);
        ident.clear();
    };
    for c in ty.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            ident.push(c);
        } else {
            flush(&mut ident, &mut out);
            out.push(c);
        }
    }
    flush(&mut ident, &mut out);
    out
}

/// Escape a reason string for a `#[ignore = "…"]` attribute.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

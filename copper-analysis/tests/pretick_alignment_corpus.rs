//! Corpus regression for the **pre-tick alignment hazard** detector
//! (`copper_analysis::unprotected_pretick_out_write`;
//! `design_docs/PRETICK_ALIGNMENT_GUARDRAIL.md`).
//!
//! This is gate **G2** of that plan, and the point is the second half: the rule must
//! catch every module measured to diverge **without flagging a single one that
//! doesn't**. A prior candidate rule matched all seven synthetic variants and still
//! rejected `mac_fsm` — the project's G2 name-exact register reference — which is why
//! the corpus verdict is pinned here as a test rather than checked by eye.
//!
//! The expectation is an **exact set**, so this fails in both directions: a newly
//! flagged module is a regression (or a real bug in that module), and a
//! no-longer-flagged one means the hazard was fixed and the list needs updating.
//!
//! Scope note: the detector examines only the pre-tick segment. The multi-tick
//! `accum_2` shape was once cited as a false negative of that scoping (plan Q5); it
//! was then measured to AGREE with its transpiled SV and un-ignored on 2026-08-21
//! (`tests/read_timing_equivalence.rs::accum_2_sim_matches_verilog`), so there is no
//! known middle-segment instance — the gap is theoretical until one is measured.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use syn::{Item, ItemFn};

/// Repo root, from this crate's manifest dir (`<root>/copper-analysis`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target" || n == "obj_dir") {
                continue;
            }
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// The mode named by `#[hardware(<mode>, <flags>…)]`, ignoring any trailing flags.
///
/// Parsing with `parse_args::<syn::Ident>()` fails outright once a flag is present
/// (e.g. `allow_pretick_alignment`), which would make such modules vanish from this
/// scan — a check that silently stops running. Take the first token instead.
fn hardware_mode_of(f: &ItemFn) -> Option<String> {
    f.attrs.iter().find_map(|a| {
        if !a.path().segments.last().is_some_and(|s| s.ident == "hardware") {
            return None;
        }
        let text = a.meta.require_list().ok()?.tokens.to_string();
        Some(text.split(',').next()?.trim().to_string())
    })
}

/// Every `fn` carrying `#[hardware(...)]` in a clocked mode, including inside `mod`s.
/// `combinational` has no tick and therefore no pre-tick segment.
fn clocked_hardware_fns(items: &[Item], out: &mut Vec<ItemFn>) {
    for item in items {
        match item {
            Item::Fn(f) => {
                let mode = hardware_mode_of(f);
                if matches!(mode.as_deref(), Some("sequential") | Some("synchronizer")) {
                    out.push(f.clone());
                }
            }
            Item::Mod(m) => {
                if let Some((_, items)) = &m.content {
                    clocked_hardware_fns(items, out);
                }
            }
            _ => {}
        }
    }
}

/// The modules the detector must flag, as `file::module`.
///
/// After the phase-3 migration these are **exactly** the fixtures that exist to
/// *demonstrate* the divergence. Each carries
/// `#[hardware(sequential, allow_pretick_alignment)]`, so the macro permits it while
/// this analysis still reports it — deliberately: the opt-out silences the *error*,
/// not the *detection*, so an opted-out module cannot quietly disappear from the
/// corpus view.
///
/// The set today (after the 2026-08-26 narrowing to READ-PRECEDED register-reading
/// writes and the constant-write clause — see the entries and their dated notes):
///
/// * `probe_fsm` / `w4_mixed_alignment` — the retained class: a read reaches the
///   write on one path only (the path-dependent region boundary).
/// * `pc_arm_write` / `pc_arm_toggle` / `branch_merge_explicit` — the constant-write
///   clause: a constant written on *some* paths, where the alternative is the held
///   value, so the phase shift is observable.
///
/// `add_then_write` / `fast_counter` (the original D1 witnesses) and `ram_prewrite`
/// were in this set until 2026-08-26 and left it — the history is recorded inline
/// below. The real designs that used to appear here — the three `fast_counter`
/// copies in `examples/cdc/` and `two_domain_hierarchy_cdc.rs` — were migrated to
/// update the sticky flag *after* the tick, the form measured to match the
/// independent hand-written SV reference.
const EXPECTED_FLAGGED: &[&str] = &[
    "tests/fixtures/probe_timing_dut.rs::probe_fsm",
    // `ram_prewrite` also left the set in the phase-D narrowing: its write is not
    // read-preceded, so it falls on the dissolved side of the discriminator. It
    // never had a behavioral verdict (it does not transpile; its only test is
    // `#[ignore]`d), so nothing measured contradicts the narrowing — if it ever
    // transpiles, the corpus sweep covers it the moment it does.
    // REMOVED 2026-08-26 (cycle-dataflow phase D): `add_then_write` and the
    // `fast_counter` witness were DISSOLVED by phase B's forwarded emission —
    // measured agreeing (`pre_tick_update_forwarding_agrees_end_to_end`,
    // `independent_hardware_anchors_the_corrected_spelling`) — and the rule was
    // narrowed to READ-PRECEDED register-reading writes (the path-dependent
    // region boundary), so the no-read opening shapes are legal now.
    //
    // ADDED the same day: `w4_mixed_alignment` — the retained class's own
    // demonstration witness (a read reaches the write on one path only),
    // measured DIVERGENT under forwarded emission (sim holds, SV alternates;
    // pinned in `d_narrowing_battery_verdicts`).
    "tests/sequential_forwarding_divergence.rs::w4_mixed_alignment",
    // Added 2026-08-25 when the CONSTANT-WRITE exemption was narrowed to
    // unconditional writes (guardrail 5.5). All three are measured divergences, not
    // rule regressions: each leads its own emitted SystemVerilog by exactly one
    // cycle, with both traces pinned in sequential_forwarding_divergence.rs. The
    // hold clause is untouched by forwarded emission (they write constants), so
    // the phase-D narrowing keeps all three.
    "tests/fixtures/control_extraction_dut.rs::branch_merge_explicit",
    "tests/sequential_forwarding_divergence.rs::pc_arm_toggle",
    "tests/sequential_forwarding_divergence.rs::pc_arm_write",
];

#[test]
fn pretick_alignment_hazard_flags_exactly_the_known_divergent_modules() {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["examples", "src", "tests"] {
        rs_files(&root.join(dir), &mut files);
    }
    files.sort();

    let mut scanned = 0usize;
    let mut flagged: BTreeSet<String> = BTreeSet::new();
    for path in &files {
        let Ok(src) = fs::read_to_string(path) else { continue };
        let Ok(file) = syn::parse_file(&src) else { continue };
        let mut fns = Vec::new();
        clocked_hardware_fns(&file.items, &mut fns);
        for f in &fns {
            scanned += 1;
            let ports = copper_analysis::unprotected_pretick_out_write(f);
            if !ports.is_empty() {
                let rel = path.strip_prefix(&root).unwrap_or(path).display();
                flagged.insert(format!("{rel}::{}", f.sig.ident));
            }
        }
    }

    assert!(scanned >= 40, "expected to scan the corpus, only saw {scanned} clocked modules");

    let expected: BTreeSet<String> = EXPECTED_FLAGGED.iter().map(|s| s.to_string()).collect();
    let unexpected: Vec<_> = flagged.difference(&expected).cloned().collect();
    let missing: Vec<_> = expected.difference(&flagged).cloned().collect();

    assert!(
        unexpected.is_empty(),
        "NEW module(s) flagged by the pre-tick alignment rule: {unexpected:#?}\n\
         Either that module has the divergence (check it against its transpiled SV), \
         or the rule has regressed into a false positive. Do NOT silence this by \
         adding to EXPECTED_FLAGGED without a measured verdict — a rule that rejects \
         a correct design is the failure mode this test exists to prevent."
    );
    assert!(
        missing.is_empty(),
        "module(s) no longer flagged: {missing:#?}\n\
         If the underlying divergence was fixed, drop them from EXPECTED_FLAGGED and \
         re-check design_docs/PRETICK_ALIGNMENT_GUARDRAIL.md — several tests are \
         pinned to the current (divergent) behaviour and will need re-blessing."
    );

    eprintln!(
        "pre-tick alignment: {scanned} clocked modules scanned, {} flagged (all measured-divergent)",
        flagged.len()
    );
}


// ── The multi-phase output rule ───────────────────────────────────────────────

/// Plain combinational `Out` ports driven in **more than one clock phase** whose
/// emitted form cannot show the simulator's trace
/// (`copper_analysis::multi_phase_out_write`, narrowed 2026-10-23, phase E).
///
/// The sibling of the rule above, and the one that covers the case it cannot see:
/// `unprotected_pretick_out_write` examines only head → first tick, and widening
/// it to every post-tick segment was measured and REJECTED (36 of 120 corpus
/// modules, ~30 with passing equivalence tests). Writing a plain `Out` after a
/// tick is the ordinary multi-phase pattern; writing it in *two* phases is what
/// this rule keys on — and since phase E it refuses only the measured divergent
/// classes (mixed anchors, register-free passthrough reads, the linear route),
/// admitting the input-free extracted class (`pulse_plain`, `e2`, `e3`) and the
/// all-closing class (`e5d`) that phase C's emission shows exactly. Each listed
/// module is one row of the verdict table in
/// `tests/sequential_forwarding_divergence.rs` (the `e*` tests pin its traces).
///
/// Like the rule above, the expectation is an **exact set** so it fails in both
/// directions. Every entry here must be a module that exists to DEMONSTRATE the
/// divergence and carries `#[hardware(sequential, allow_pretick_alignment)]` — the
/// opt-out silences the error, not the detection, so an opted-out module cannot
/// quietly disappear from this view. A real design appearing here is a bug in that
/// design, and the fix is `RegOut`.
const EXPECTED_MULTI_PHASE: &[&str] = &[
    // the linear route: refused by `vlir_lower` as multiply driven (e7) or
    // edge-sampled through the phase-guarded hold path (e7w)
    "tests/sequential_forwarding_divergence.rs::e7_linear_two_registers",
    "tests/sequential_forwarding_divergence.rs::e7w_linear_phase_local_wires",
    // an input read anywhere demotes every drive to edge-sampled (clause i)
    "tests/sequential_forwarding_divergence.rs::e2x_two_registers_plus_unrelated_read",
    // mixed anchors: one closing drive, one opening (clause ii)
    "tests/sequential_forwarding_divergence.rs::e4_closing_then_opening",
    "tests/sequential_forwarding_divergence.rs::e5_two_closing",
    "tests/sequential_forwarding_divergence.rs::e6_closing_via_wire_then_opening",
    // register-free: the reads feeding the writes are immediate passthroughs
    "tests/sequential_forwarding_divergence.rs::e4i_register_free_passthrough_then_opening",
    "tests/sequential_forwarding_divergence.rs::e5f_two_closing_register_free",
];

#[test]
fn multi_phase_out_write_flags_exactly_the_demonstration_modules() {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["examples", "src", "tests"] {
        rs_files(&root.join(dir), &mut files);
    }
    files.sort();

    let mut scanned = 0usize;
    let mut flagged: BTreeSet<String> = BTreeSet::new();
    for path in &files {
        let Ok(src) = fs::read_to_string(path) else { continue };
        let Ok(file) = syn::parse_file(&src) else { continue };
        let mut fns = Vec::new();
        clocked_hardware_fns(&file.items, &mut fns);
        for f in &fns {
            scanned += 1;
            if !copper_analysis::multi_phase_out_write(f).is_empty() {
                let rel = path.strip_prefix(&root).unwrap_or(path).display();
                flagged.insert(format!("{rel}::{}", f.sig.ident));
            }
        }
    }

    assert!(scanned >= 40, "expected to scan the corpus, only saw {scanned} clocked modules");

    let expected: BTreeSet<String> = EXPECTED_MULTI_PHASE.iter().map(|s| s.to_string()).collect();
    let unexpected: Vec<_> = flagged.difference(&expected).cloned().collect();
    let missing: Vec<_> = expected.difference(&flagged).cloned().collect();
    assert!(
        unexpected.is_empty(),
        "newly flagged (a plain `Out` driven in two clock phases in the mixed, \
         register-free or linear class — declare it `RegOut`, make every write of it \
         closing, or if the module exists to demonstrate the hazard add \
         `allow_pretick_alignment` and list it here): {unexpected:?}"
    );
    assert!(
        missing.is_empty(),
        "no longer flagged — the hazard may be fixed, or the module changed: {missing:?}"
    );
}

// ── The late-constant startup rule ────────────────────────────────────────────

/// Plain `Out` ports whose every write is a constant and whose first write executes
/// only after the second clock edge of a multi-tick loop
/// (`copper_analysis::out_first_driven_late`, 2026-10-23).
///
/// Not a steady-state divergence but a **startup transient**: the simulator shows
/// the port's initial value at the first observation (the writing segment has not
/// run yet) while the emitted `assign` drives the constant from time 0. Its
/// instance, `trailing_constant`, lived as a `SKIP` row ("STARTUP TRANSIENT") in
/// `build.rs` until the shape was refused; it is now
/// `copper-macros/tests/ui/fail/out_first_driven_late.rs`, and the agreeing half of
/// its flipping pair — the same constant one edge earlier,
/// `late_constant_one_edge` — sweeps as a positive case. Register-valued late writes
/// are deliberately outside the rule (`mac_pipeline`'s `out`, `trailing_out`'s `mid`
/// agree today because the flop's zero initial equals the wire's).
///
/// Same exact-set discipline as every rule above: an entry must be a module that
/// exists to DEMONSTRATE the hazard and carries `allow_pretick_alignment` (the
/// opt-out silences the error, not the detection). A real design appearing here is
/// a false positive to measure, not a line to add.
const EXPECTED_LATE_CONSTANT: &[&str] = &[
    // Both writes constant, both past a folded tick-bearing `for`. Since phase E
    // (2026-10-23) this is the ONLY rule that flags it — `multi_phase_out_write`
    // admits the input-free class — and the module agrees with its SV under
    // phase C's emission (the folded loop is this rule's conservative direction).
    "tests/sequential_forwarding_divergence.rs::pulse_plain",
];

#[test]
fn out_first_driven_late_flags_exactly_the_demonstration_modules() {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["examples", "src", "tests"] {
        rs_files(&root.join(dir), &mut files);
    }
    files.sort();

    let mut scanned = 0usize;
    let mut flagged: BTreeSet<String> = BTreeSet::new();
    for path in &files {
        let Ok(src) = fs::read_to_string(path) else { continue };
        let Ok(file) = syn::parse_file(&src) else { continue };
        let mut fns = Vec::new();
        clocked_hardware_fns(&file.items, &mut fns);
        for f in &fns {
            scanned += 1;
            if !copper_analysis::out_first_driven_late(f).is_empty() {
                let rel = path.strip_prefix(&root).unwrap_or(path).display();
                flagged.insert(format!("{rel}::{}", f.sig.ident));
            }
        }
    }

    assert!(scanned >= 40, "expected to scan the corpus, only saw {scanned} clocked modules");

    let expected: BTreeSet<String> = EXPECTED_LATE_CONSTANT.iter().map(|s| s.to_string()).collect();
    let unexpected: Vec<_> = flagged.difference(&expected).cloned().collect();
    let missing: Vec<_> = expected.difference(&flagged).cloned().collect();
    assert!(
        unexpected.is_empty(),
        "newly flagged by the late-constant startup rule (a plain `Out` whose every write \
         is a constant and whose first write comes after the second clock edge). Either \
         the module HAS the transient — sweep it: the signature is a mismatch at cycle 0 \
         only, and the remedy is a head-segment write or `RegOut` — or the rule has \
         regressed into a false positive, which is the failure mode this pin exists to \
         catch: {unexpected:?}"
    );
    assert!(
        missing.is_empty(),
        "no longer flagged — the transient may be fixed, or the module changed: {missing:?}"
    );
}

// ── The trailing-`RegOut`-from-an-input rule (2026-10-23) ──────────────────────

/// `RegOut` ports written after the last top-level tick from an `In` port read
/// directly, at an immediate read site (`copper_analysis::trailing_regout_reads_input`).
///
/// Born with the tick-dialect unification: a `RegOut` write takes effect at the
/// first clock edge after it executes, so the linear single-tick lowering now emits
/// a trailing `RegOut` from the committed register at the next edge (which made
/// `regout_trailing_single_tick` agree and sweep), and the one shape that emission
/// cannot realize is a value read straight from an input — the flip-flop samples it
/// one drive later than the simulator's immediate read. No corpus module has the
/// shape; the ui/fail case `trailing_regout_reads_input.rs` is the witness, and a
/// module joining this set is either a real divergence to measure or a rule
/// regression to fix.
const EXPECTED_TRAILING_REGOUT_INPUT: &[&str] = &[];

#[test]
fn trailing_regout_reads_input_flags_exactly_the_demonstration_modules() {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["examples", "src", "tests"] {
        rs_files(&root.join(dir), &mut files);
    }
    files.sort();

    let mut scanned = 0usize;
    let mut flagged: BTreeSet<String> = BTreeSet::new();
    for path in &files {
        let Ok(src) = fs::read_to_string(path) else { continue };
        let Ok(file) = syn::parse_file(&src) else { continue };
        let mut fns = Vec::new();
        clocked_hardware_fns(&file.items, &mut fns);
        for f in &fns {
            scanned += 1;
            if !copper_analysis::trailing_regout_reads_input(f).is_empty() {
                let rel = path.strip_prefix(&root).unwrap_or(path).display();
                flagged.insert(format!("{rel}::{}", f.sig.ident));
            }
        }
    }

    assert!(scanned >= 40, "expected to scan the corpus, only saw {scanned} clocked modules");

    let expected: BTreeSet<String> =
        EXPECTED_TRAILING_REGOUT_INPUT.iter().map(|s| s.to_string()).collect();
    let unexpected: Vec<_> = flagged.difference(&expected).cloned().collect();
    let missing: Vec<_> = expected.difference(&flagged).cloned().collect();
    assert!(
        unexpected.is_empty(),
        "newly flagged by the trailing-RegOut-from-input rule. Either the module HAS \
         the divergence — check it against its transpiled SV; the remedy is to read \
         the input before the tick or through a register — or the rule has regressed \
         into a false positive: {unexpected:?}"
    );
    assert!(missing.is_empty(), "no longer flagged — the module changed: {missing:?}");
}

// ── The trailing-plain-`Out`-from-an-input rule (2026-10-26, the e8 class) ──────────────────────

/// Plain `Out` ports written after the last top-level tick from an `In` port read
/// directly at an immediate read site, in a body that crosses more than one clock
/// edge (`copper_analysis::trailing_out_reads_input`).
///
/// The e8 class phase E found unguarded and the user chose to refuse (2026-10-26):
/// the simulator shows the immediate read's result at the opening observation, the
/// extraction route edge-samples the drive one observation late, and the linear
/// multi-tick route emits an ungated continuous assign. The single-tick body agrees
/// and is exempt. The witnesses are the measured probes in
/// (`e5_two_closing` is phase E's mixed-anchor probe, whose second write is exactly
/// this shape: a trailing immediate read, measured diverging)
/// `tests/sequential_forwarding_divergence.rs` (all opted out); the ui/fail case
/// `trailing_out_reads_input.rs` is the compile-time witness. A module joining this
/// set is either a real divergence to measure or a rule regression to fix.
const EXPECTED_TRAILING_OUT_INPUT: &[&str] = &[
    "tests/sequential_forwarding_divergence.rs::e5_two_closing",
    "tests/sequential_forwarding_divergence.rs::e8_single_phase_trailing_immediate_read",
    "tests/sequential_forwarding_divergence.rs::e8m_linear_two_ticks",
    "tests/sequential_forwarding_divergence.rs::e8p_extracted_passthrough",
];

#[test]
fn trailing_out_reads_input_flags_exactly_the_demonstration_modules() {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["examples", "src", "tests"] {
        rs_files(&root.join(dir), &mut files);
    }
    files.sort();

    let mut scanned = 0usize;
    let mut flagged: BTreeSet<String> = BTreeSet::new();
    for path in &files {
        let Ok(src) = fs::read_to_string(path) else { continue };
        let Ok(file) = syn::parse_file(&src) else { continue };
        let mut fns = Vec::new();
        clocked_hardware_fns(&file.items, &mut fns);
        for f in &fns {
            scanned += 1;
            if !copper_analysis::trailing_out_reads_input(f).is_empty() {
                let rel = path.strip_prefix(&root).unwrap_or(path).display();
                flagged.insert(format!("{rel}::{}", f.sig.ident));
            }
        }
    }

    assert!(scanned >= 40, "expected to scan the corpus, only saw {scanned} clocked modules");

    let expected: BTreeSet<String> =
        EXPECTED_TRAILING_OUT_INPUT.iter().map(|s| s.to_string()).collect();
    let unexpected: Vec<_> = flagged.difference(&expected).cloned().collect();
    let missing: Vec<_> = expected.difference(&flagged).cloned().collect();
    assert!(
        unexpected.is_empty(),
        "newly flagged by the trailing-Out-from-input rule. Either the module HAS \
         the divergence — check it against its transpiled SV; the remedy is to read \
         the input before the tick or through a register — or the rule has regressed \
         into a false positive: {unexpected:?}"
    );
    assert!(missing.is_empty(), "no longer flagged — the module changed: {missing:?}");
}

// ── The write-between-read-and-update rule (V8, m1) ──────────────────────────

/// Plain `Out` ports written between a leading `In` read and the update of a
/// register the write reads (`copper_analysis::pretick_out_write_before_update`;
/// `design_docs/DERIVATION_TABLE.md` F2).
///
/// The fifth member of the family, and the first DERIVED before it was measured:
/// the cycle-dataflow model predicted the divergence, the exact traces, and both
/// flipping controls (the V8 battery in `sequential_forwarding_divergence.rs`)
/// before the controlled measurement existed. The in-vivo instance was
/// `rv32i_cpu_transpilable`'s `program_counter` (TODO cause Q, divergence #1),
/// since rewritten to the post-commit form — which is why no real design appears
/// here. Same exact-set discipline as the rules above: the opt-out silences the
/// error, not the detection.
const EXPECTED_WRITE_BEFORE_UPDATE: &[&str] = &[
    "tests/sequential_forwarding_divergence.rs::v8a_read_write_update",
    // V8a with the update routed through a same-cycle temp — the shape the rule
    // caught in ui/pass/single_loop_local_ok.rs THE DAY IT LANDED (a compile-only
    // fixture nobody had measured; it diverges exactly like V8a and is pinned as
    // this witness; the fixture itself was reordered to the legal V8c form).
    "tests/sequential_forwarding_divergence.rs::v8d_temp_renamed_update",
    // The TRAILING clause (added 2026-08-27): a plain `Out` written before the
    // update of a register it reads in the trailing segment publishes the
    // previous generation for the whole cycle. Its landing-day catch was three
    // REAL modules — module_composition_hybrid's pipeline stages, whose tests
    // were sim-only and whose shape was measured divergent as
    // `v8t_stage_publish_then_load`; they were migrated to the canonical
    // write/read/tick/update spelling. The two below are the demonstration
    // witnesses that remain: the claim-ledger pair's divergent half, and the
    // minimal stage shape.
    "tests/fixtures/out_phase_dut.rs::out_from_reg_before_commit",
    "tests/sequential_forwarding_divergence.rs::v8t_stage_publish_then_load",
    // Seen THROUGH a `let` temporary (2026-10-20): a Mealy `ready = !full ||
    // drain` computed into `can_take` and written, with `full` updated later in
    // the same segment. V8a with one rename — the rule's syntactic `uses ∩
    // registers` saw only the temporary's name and let it through while
    // refusing the inline spelling. Both were MEASURED in the corpus sweep
    // before the rule learned the dataflow: `ready`/`in_ready` the only
    // diverging port, every mismatch the simulator at 1 against the SV at 0
    // (the SV drops `ready` the cycle `full` commits; the simulator a cycle
    // later — hardware leads by one), on 15 of 200 cycles for `vr_sink_mealy`
    // and 20 of 200 for `vr_skid` under the seeded stimulus (the lead is only
    // visible on cycles where `full` changes). They keep
    // `allow_pretick_alignment` as demonstrations; the
    // refused spellings are the `write_before_update_via_temp` /
    // `write_before_update_via_bool_temp` ui/fail cases. No pre-existing
    // corpus module was newly flagged — the extension's catch is exactly the
    // two probes that motivated it.
    "tests/fixtures/valid_ready_dut.rs::vr_sink_mealy",
    "tests/fixtures/valid_ready_dut.rs::vr_skid",
];

#[test]
fn write_before_update_flags_exactly_the_demonstration_modules() {
    let root = repo_root();
    let mut files = Vec::new();
    for dir in ["examples", "src", "tests"] {
        rs_files(&root.join(dir), &mut files);
    }
    files.sort();

    let mut scanned = 0usize;
    let mut flagged: BTreeSet<String> = BTreeSet::new();
    for path in &files {
        let Ok(src) = fs::read_to_string(path) else { continue };
        let Ok(file) = syn::parse_file(&src) else { continue };
        let mut fns = Vec::new();
        clocked_hardware_fns(&file.items, &mut fns);
        for f in &fns {
            scanned += 1;
            if !copper_analysis::pretick_out_write_before_update(f).is_empty() {
                let rel = path.strip_prefix(&root).unwrap_or(path).display();
                flagged.insert(format!("{rel}::{}", f.sig.ident));
            }
        }
    }

    assert!(scanned >= 40, "expected to scan the corpus, only saw {scanned} clocked modules");

    let expected: BTreeSet<String> =
        EXPECTED_WRITE_BEFORE_UPDATE.iter().map(|s| s.to_string()).collect();
    let unexpected: Vec<_> = flagged.difference(&expected).cloned().collect();
    let missing: Vec<_> = expected.difference(&flagged).cloned().collect();

    assert!(
        unexpected.is_empty(),
        "NEW module(s) flagged by the write-before-update rule: {unexpected:#?}\n\
         Either that module has the divergence (check it against its transpiled SV — \
         the V8a trace pair is the signature: SV leads the sim by exactly one), or \
         the rule has regressed into a false positive. Do NOT add to \
         EXPECTED_WRITE_BEFORE_UPDATE without a measured verdict."
    );
    assert!(
        missing.is_empty(),
        "module(s) no longer flagged: {missing:#?}\n\
         If the divergence was fixed (the paired cycle-dataflow migration), drop \
         them here and re-bless the V8 tests in sequential_forwarding_divergence.rs, \
         which pin today's divergent traces."
    );

    eprintln!(
        "write-before-update: {scanned} clocked modules scanned, {} flagged",
        flagged.len()
    );
}

//! **The two derivations of "which cycle does this statement run in", measured.**
//!
//! Copper decides that question twice. The simulator decides it by *suspension*: a
//! statement runs in whatever cycle the coroutine is in when it is polled. The
//! transpiler decides it *structurally*, in `shir_lower`, by splitting the lowered
//! loop body at `clk.tick().await` (`split_at_ticks`) and calling each piece a phase.
//! Two implementations of one semantic rule is the shape every silent sim ≠ synth
//! divergence in this project has had, so how far apart they are is worth knowing as
//! a number rather than an argument.
//!
//! `copper_analysis::clock_phase_count` computes the phase count from the SOURCE CFG
//! (Comb-connected components of the control-flow graph); this test compares it with
//! the phase count `shir_lower` arrives at, for every module in the corpus.
//!
//! # The result
//!
//! They agree everywhere **except after `control_extract`**, and there the lowered
//! count is always exactly 1. That pass rewrites a body whose ticks live inside
//! branches or loops into a single-tick `match pc` FSM, so the phase structure is not
//! *lost* — the `pc` states are the phases — it is simply no longer *represented* as
//! phases. Every downstream check that counts ticks then sees one.
//!
//! That is a much narrower defect than "two derivations that disagree", and it is why
//! the fix in `design_docs/TIMING_MODEL_UNIFICATION.md` is a phase tag rather than a
//! rewrite of the lowering. The set below is pinned so it cannot drift quietly: a
//! module joining it means a new pass has started hiding phases, and a module leaving
//! it means the tag (or something like it) has started working.

use std::collections::{BTreeSet, HashSet};

/// The modules whose LOWERED phase count differs from their SOURCE phase count.
/// Every one is control-extracted; every one lowers to exactly one phase.
const EXPECTED_DISAGREEMENTS: &[&str] = &[
    // Added 2026-09-02 with the module: a `for` over the pattern bits with a `break`
    // before its tick, plus a conditional trailing tick — every tick sits inside a
    // loop or a branch, so the whole body is extracted into one `match pc`. Not a
    // pass hiding phases from a module that used to be visible; a new module of the
    // shape this set exists to name. tests/det_for_probe.rs pins its timing.
    "det_for",
    // Added 2026-11-06 with the modules (Phase C step 5, II.3 (a)): a nested loop whose
    // `continue` follows a tick inside a branch — every tick in a loop or a branch, so
    // the body is extracted into one `match pc`; the sweep pins their timing
    // (tests/fixtures/nested_continue_dut.rs).
    "nested_continue",
    "nested_continue_match",
    // Added 2026-11-06 with the modules (Phase C step 5, II.3 (b)): nested loops whose
    // ticks lie only inside arms — extracted whole; the sweep pins their timing
    // (tests/fixtures/arms_only_loop_dut.rs).
    "arms_only_b1",
    "arms_only_b2",
    "arms_only_match",
    // Added 2026-11-07 (Phase C step 6, MEASUREMENTS §K.11). `for_empty` and
    // `for_empty_start` (step 5, II.3 (f1): a counted `for` over a run-time range) are
    // extracted into one `match pc` like every module here, but were never compared:
    // this test replayed the passes by hand without the register append, the
    // hoisted counter failed to lower, and the `Err` arm skipped them. Measured once
    // the test went through `lower_fir_to_shir`. `nested_loop_added` (step 6's
    // fixture) is a wait loop with its tick inside, extracted the same way.
    "for_empty",
    "for_empty_start",
    "nested_loop_added",
    // Added 2026-11-09 with D-27 (MEASUREMENTS §AA.3): the inclusive twins of `det_for` (an
    // author's `break` guards the hoisted tick) and `for_empty` (a run-time end guards it on the
    // counter having advanced) — the guarded tick takes the extraction route, as theirs does.
    "ifr_ticking_break",
    "ifr_ticking_runtime",
    // Added 2026-11-09 with the fixtures of D-28 and line `II.3 found loop-var-type`
    // (MEASUREMENTS §AB): ticking counted `for`s over a run-time end, with an author's
    // `break`, or two in one module loop (a nested loop each) — the extraction route, as
    // `det_for`'s and `for_empty`'s.
    "imx_i8_runtime",
    "imx_u16_break",
    "imx_u32_runtime",
    "imx_u64_runtime",
    "imx_u8_const",
    "imx_u8_runtime",
    "lvt_shadow",
    "lvt_signed",
    "lvt_use_ticking_break",
    // Added 2026-11-09 with line `II.3 found scoped-shadow` (MEASUREMENTS §AC): a ticking
    // counted `for` and a tick after it in one module loop, as `lvt_shadow`'s.
    "ssh_for_shadows_local",
    // Added 2026-10-20 with the modules: the paper's serializer (a `while` wait, a
    // counted `for` over the data bits, each with its tick inside) and the one-tick
    // handshake (two `while` waits). Every tick sits inside a loop, so both extract
    // into one `match pc` — new modules of the shape this set names, not a pass
    // that started hiding phases.
    "serializer",
    "simple_handshake",
    "capture_after_wait",
    "det_010_awaits",
    "handshake",
    "if_tick",
    "rom_gated",
    "rom_paced",
    // The valid/ready probes (tests/fixtures/valid_ready_dut.rs, 2026-10-20) with
    // a `while … { clk.tick().await; }` wait — desugared and extracted the same
    // way `waiter` / `while_waiter` are, so each lowers to one phase. The two
    // Mealy spellings in that file (`vr_sink_mealy`, `vr_skid`) are single-tick
    // and agree.
    "vr_read_after_delay",
    "vr_sink_moore",
    "vr_src_regout",
    "vr_src_same_cycle",
    "waiter",
    "while_waiter",
];

/// Run the transpiler's own pipeline as far as SHIR and report how many phases it
/// arrived at. `Err` for anything that does not lower (a combinational module has no
/// phases; a module blocked on a recorded cause never gets here).
fn lowered_phase_count(src: &str, module: &str) -> Result<usize, String> {
    let file = syn::parse_file(src).map_err(|e| e.to_string())?;
    let hardware_fns: HashSet<String> = file
        .items
        .iter()
        .filter_map(|i| match i {
            syn::Item::Fn(f) if is_hardware(f) => Some(f.sig.ident.to_string()),
            _ => None,
        })
        .collect();
    let f = file
        .items
        .iter()
        .find_map(|i| match i {
            syn::Item::Fn(f) if f.sig.ident == module => Some(f),
            _ => None,
        })
        .ok_or("no such module")?;

    // The transpiler's own pipeline to SHIR (`lower_fir_to_shir`): desugar, extract,
    // the register authority's append, then lower. Until Phase C step 6 this test
    // replayed the passes by hand without the append, so a module whose hoisted
    // counter then failed to lower was silently not compared (`for_empty`,
    // `for_empty_start`; MEASUREMENTS §K.11).
    let fir = copper_codegen::capture_frontend_ir(f, &hardware_fns).map_err(|e| format!("{e:?}"))?;
    let shir = copper_codegen::lower_fir_to_shir(&fir, &hardware_fns, &Default::default())?;
    match &shir.body {
        copper_core::shir::SHIRBody::Sequential(s) => Ok(s.phases.len()),
        _ => Err("not a sequential body".into()),
    }
}

fn is_hardware(f: &syn::ItemFn) -> bool {
    f.attrs
        .iter()
        .any(|a| a.path().segments.last().is_some_and(|s| s.ident == "hardware"))
}

fn corpus_files() -> Vec<std::path::PathBuf> {
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(d) else { return };
        for e in entries.filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    walk(&root.join("tests/fixtures"), &mut files);
    walk(&root.join("examples"), &mut files);
    files.sort();
    files
}

#[test]
fn the_two_phase_derivations_agree_except_where_extraction_hides_them() {
    let mut disagreements: BTreeSet<String> = BTreeSet::new();
    let mut agreed = 0usize;

    for path in corpus_files() {
        let src = std::fs::read_to_string(&path).expect("source is readable");
        let Ok(file) = syn::parse_file(&src) else { continue };
        for item in &file.items {
            let syn::Item::Fn(f) = item else { continue };
            if !is_hardware(f) {
                continue;
            }
            let name = f.sig.ident.to_string();
            let Some(source_phases) = copper_analysis::clock_phase_count(f) else { continue };
            match lowered_phase_count(&src, &name) {
                Ok(lowered) if lowered == source_phases => agreed += 1,
                Ok(lowered) => {
                    assert_eq!(
                        lowered, 1,
                        "{name}: the lowered phase count disagrees with the source ({source_phases}) \
                         and is not 1 — every disagreement so far has been control extraction \
                         collapsing the body to a single tick, so this is a NEW mechanism and \
                         wants investigating before it is added to the expected set"
                    );
                    disagreements.insert(name);
                }
                Err(_) => {}
            }
        }
    }

    assert!(agreed > 40, "only {agreed} modules compared — the probe stopped reaching the corpus");

    let expected: BTreeSet<String> =
        EXPECTED_DISAGREEMENTS.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        disagreements, expected,
        "the set of modules whose phase structure the lowering cannot see has CHANGED.\n\
         Gained → a pass has started hiding phases from a module that used to be visible.\n\
         Lost   → the phase tag (or equivalent) is working; move it out of the expected set \
         and record what fixed it in design_docs/TIMING_MODEL_UNIFICATION.md."
    );
}

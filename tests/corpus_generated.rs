//! Corpus differential sweep — one generated case per `#[hardware]` module in
//! `tests/fixtures/` and `examples/`.
//!
//! The cases are written by `build.rs`, not by hand: simulator vs Verilated
//! emitted-SystemVerilog needs no reference model, so there is nothing per-module
//! for a person to supply, and coverage stops depending on who got round to it.
//! (The hand-written phase-1 original, `tests/corpus_equivalence.rs`, was deleted
//! on 2026-08-25 once the generator reached `examples/` too.)
//!
//! A module the sweep cannot run still gets a test, `#[ignore]`d with the reason —
//! from `build.rs`'s `SKIP` table, or because it is generic. `tools/regression.sh`
//! prints every ignored test on every run, so a skip stays visible instead of
//! becoming a silent hole.
//!
//! **Reading a failure:** `verilator: FAIL` is a transpiler bug — the simulator is
//! the semantic source of truth. The trace comparison cannot fail here (with no
//! model, the simulator's own outputs are the expected trace), so any failure is
//! either a real divergence, invalid emitted SystemVerilog, or a generator bug that
//! shows up as a C++ compile error.

mod common;

include!(concat!(env!("OUT_DIR"), "/corpus_generated.rs"));

/// **The stimulus policy's pin** — `design_docs/CORPUS_DIFFERENTIAL_SWEEP.md` §3.2,
/// asserted against `StimulusPlan` with the generated `BUDGET`, so the sentence in
/// the doc and the sequences the sweep drives cannot drift apart silently.
/// `tools/regression.sh`'s G-D requires this test's `ok` line, the way it requires
/// the coverage guard's: a sweep that ran under some other policy is not the sweep
/// the doc describes.
#[test]
fn the_stimulus_policy_is_what_the_design_doc_says() {
    use common::{bits_key, RandStim, Rng, SequenceKind, StimulusPlan};
    use copper_core::{Bits, Logic};
    use std::collections::BTreeSet;

    let b = BUDGET;
    assert_eq!(
        (b.seeds, b.seed_cycles, b.first_cycles, b.exhaustive_bits, b.comb_exhaustive_bits, b.comb_vectors),
        (4, 200, 32, 4, 8, 64),
        "the constants §3.2 states"
    );
    let bits = |v: &[u8]| v.iter().map(|x| *x == 1).collect::<Vec<bool>>();
    let firsts = |p: &StimulusPlan| {
        p.sequences().iter().filter_map(|s| s.first.as_deref().map(bits_key)).collect::<BTreeSet<_>>()
    };

    // ── A one-input-bit clocked module (branch_merge's shape): four seeds, then
    //    one 32-cycle sequence from EACH initial vector. Sequence 0 is the
    //    historical seed, and every sequence has a seed of its own.
    let p = StimulusPlan::new("m", 0x1234, &[("sel", 1)], true, None, b).with_only(None);
    let seqs = p.sequences();
    assert_eq!(seqs.len(), 4 + 2);
    assert!(seqs[..4].iter().all(|s| s.kind == SequenceKind::Seed && s.cycles == 200 && s.first.is_none()));
    assert_eq!(seqs[0].seed, 0x1234, "sequence 0 is the historical per-module seed, verbatim");
    assert_eq!(seqs.iter().map(|s| s.seed).collect::<BTreeSet<_>>().len(), 6, "distinct seeds");
    assert!(seqs[4..].iter().all(|s| s.kind == SequenceKind::Exhaustive && s.cycles == 32));
    assert_eq!(firsts(&p), BTreeSet::from([bits(&[0]), bits(&[1])]));
    assert_eq!(p.first_live_cycle(), 0);

    // Driving it: sequence 0's stimulus IS the bare seeded walk the single-sequence
    // sweep made (so every verdict it ever gave is a subset of the new one); the
    // `sel = 1` sequence drives 1 on cycle 0 and random after.
    let mut p = p;
    assert!(p.next_sequence());
    assert_eq!(p.index(), 0);
    let mut rng = Rng::new(p.seed());
    let mut bare = Rng::new(0x1234);
    for c in 0..p.cycles() {
        assert_eq!(p.stim::<Logic>(c, 0, &mut rng), bare.logic(), "cycle {c}");
    }
    for _ in 0..5 {
        assert!(p.next_sequence());
    }
    assert_eq!(p.index(), 5);
    assert_eq!(p.label(), "first sel=1");
    let mut rng = Rng::new(p.seed());
    assert_eq!(p.stim::<Logic>(0, 0, &mut rng), Logic::One);
    for c in 1..p.cycles() {
        let _ = p.stim::<Logic>(c, 0, &mut rng);
    }
    assert!(!p.next_sequence());
    assert_eq!(p.covered().len(), 2, "both initial vectors were driven: {:?}", p.covered());

    // ── Four input bits: still exhaustive (16 sequences); five: the pattern list.
    let p = StimulusPlan::new("m", 1, &[("a", 1), ("b", 3)], true, None, b);
    assert_eq!(p.sequences().len(), 4 + 16);
    assert_eq!(firsts(&p).len(), 16);
    let p = StimulusPlan::new("m", 1, &[("a", 1), ("b", 4)], true, None, b);
    assert!(p.sequences()[4..].iter().all(|s| s.kind == SequenceKind::Pattern && s.cycles == 32));
    assert_eq!(
        firsts(&p),
        BTreeSet::from([
            bits(&[0, 0, 0, 0, 0]), // all zeros
            bits(&[1, 1, 1, 1, 1]), // all ones
            bits(&[1, 0, 0, 0, 0]), // only a ones  (= only b zeros)
            bits(&[0, 1, 1, 1, 1]), // only a zeros (= only b ones)
        ]),
        "the pattern list, deduplicated"
    );
    assert_eq!(p.sequences().len(), 4 + 4);

    // ── No inputs: seeds only.
    assert_eq!(StimulusPlan::new("m", 1, &[], true, None, b).sequences().len(), 4);

    // ── A RESET module (shift_register's ports): the reset bit is excluded from W,
    //    asserted on cycle 0 and DEASSERTED on cycle 1, the first live cycle, where
    //    the initial vector is driven. In a seed sequence cycle 1's reset is random,
    //    as it was before the policy.
    let mut p = StimulusPlan::new(
        "m", 7, &[("d", 1), ("en", 1), ("dir", 1), ("rstn", 1)], true, Some((3, true)), b,
    ).with_only(None);
    assert_eq!(p.width(), 3);
    assert_eq!(p.sequences().len(), 4 + 8);
    assert_eq!(p.first_live_cycle(), 1);
    assert!(p.next_sequence()); // sequence 0, a seed
    let mut rng = Rng::new(p.seed());
    let mut bare = Rng::new(7);
    for c in 0..3 {
        for port in 0..4 {
            let v = p.stim::<Logic>(c, port, &mut rng);
            let expect = if port == 3 && c == 0 { Logic::Zero } else { bare.logic() };
            assert_eq!(v, expect, "seed sequence, cycle {c} port {port}");
        }
    }
    for _ in 0..4 {
        assert!(p.next_sequence());
    }
    let want = p.sequences()[4].first.clone().unwrap();
    let mut rng = Rng::new(p.seed());
    for port in 0..4 {
        let _ = p.stim::<Logic>(0, port, &mut rng);
    }
    let cycle1: Vec<Logic> = (0..4).map(|port| p.stim::<Logic>(1, port, &mut rng)).collect();
    assert_eq!(cycle1[3], Logic::One, "reset deasserted on the first live cycle");
    assert_eq!(&cycle1[..3], &want[..], "the initial vector lands on cycle 1");

    // ── Combinational, W ≤ 8: the whole truth table, vector c on cycle c, and
    //    nothing else; wider: four seeds of 64 vectors plus the pattern list.
    let mut p = StimulusPlan::new("c", 1, &[("a", 3), ("b", 2)], false, None, b).with_only(None);
    assert_eq!(p.sequences().len(), 1);
    assert_eq!(p.sequences()[0].kind, SequenceKind::Vectors);
    assert_eq!(p.cycles_total(), 32);
    assert!(p.next_sequence());
    let mut rng = Rng::new(p.seed());
    for c in 0..32usize {
        let a = p.stim::<Bits<3>>(c, 0, &mut rng);
        let bb = p.stim::<Bits<2>>(c, 1, &mut rng);
        let got: Vec<Logic> = a.as_bits().into_iter().chain(bb.as_bits()).collect();
        let want: Vec<Logic> = (0..5).map(|j| if (c >> j) & 1 == 1 { Logic::One } else { Logic::Zero }).collect();
        assert_eq!(got, want, "vector {c}");
    }
    let p = StimulusPlan::new("c", 1, &[("a", 8), ("b", 1)], false, None, b);
    assert_eq!(p.sequences().len(), 5);
    assert!(p.sequences()[..4].iter().all(|s| s.kind == SequenceKind::Seed && s.cycles == 64));
    assert_eq!(p.sequences()[4].kind, SequenceKind::Vectors);
    assert_eq!(p.sequences()[4].cycles, 4, "the pattern list for (8, 1): zeros, ones, a-only, b-only");

    // ── Run-time dedup: a pattern whose vector a SEED sequence already drove on its
    //    first live cycle is skipped. Which seed does that is data, so find one — the
    //    plan for ("a", 1), ("b", 4) has 32 initial vectors and four of them are
    //    patterns, so some base seed below a few hundred collides.
    let drive = |p: &mut StimulusPlan| {
        let mut ran = Vec::new();
        while p.next_sequence() {
            ran.push(p.index());
            let mut rng = Rng::new(p.seed());
            for c in 0..p.cycles() {
                let _ = p.stim::<Logic>(c, 0, &mut rng);
                let _ = p.stim::<Bits<4>>(c, 1, &mut rng);
            }
        }
        ran
    };
    let mut found = None;
    for base in 1..2000u64 {
        let mut p = StimulusPlan::new("d", base, &[("a", 1), ("b", 4)], true, None, b).with_only(None);
        let ran = drive(&mut p);
        if ran.len() < p.sequences().len() {
            let skipped: Vec<usize> = (0..p.sequences().len()).filter(|i| !ran.contains(i)).collect();
            for i in &skipped {
                let s = &p.sequences()[*i];
                assert_eq!(s.kind, SequenceKind::Pattern, "only a pattern sequence may be dropped");
                let seed_firsts: BTreeSet<Vec<bool>> = {
                    // The vectors the four seed sequences drove on cycle 0.
                    let mut q = StimulusPlan::new("d", base, &[("a", 1), ("b", 4)], true, None, b).with_only(None);
                    let mut v = BTreeSet::new();
                    for _ in 0..4 {
                        assert!(q.next_sequence());
                        let mut rng = Rng::new(q.seed());
                        let a = q.stim::<Logic>(0, 0, &mut rng);
                        let bb = q.stim::<Bits<4>>(0, 1, &mut rng);
                        v.insert(bits_key(&a.as_bits().into_iter().chain(bb.as_bits()).collect::<Vec<_>>()));
                    }
                    v
                };
                assert!(
                    seed_firsts.contains(&bits_key(s.first.as_ref().unwrap())),
                    "sequence {i} was dropped but no seed drove its vector"
                );
            }
            found = Some((base, skipped));
            break;
        }
    }
    assert!(found.is_some(), "no base seed below 2000 made a seed sequence cover a pattern — the dedup never fired");

    // ── `COPPER_SWEEP_SEQUENCE=<i>` (here `with_only`): that one sequence runs, alone.
    let mut p = StimulusPlan::new("m", 1, &[("sel", 1)], true, None, b).with_only(Some(5));
    assert!(p.next_sequence());
    assert_eq!(p.index(), 5);
    assert!(!p.next_sequence());
}

/// **The guard.** Every `#[hardware]` module in `tests/fixtures/` and `examples/`
/// must have a case — running, or `#[ignore]`d with a reason. Asserted against a
/// fresh scan rather than against the generator's own bookkeeping, so a `build.rs`
/// that silently stops covering something (a parse it declines, a filter that grows
/// too wide, a directory it no longer walks) fails here instead of quietly shrinking
/// the sweep.
///
/// Same idea as `tools/regression.sh`'s G-A/G-B/G-C: in this repo "the check did not
/// run" has been a more expensive bug than "the check failed".
///
/// Keys are `<wrapper>::<module>`, not bare module names: two different modules
/// legitimately share a name (`fast_counter` exists in both `two_domain_counter.rs`
/// and `two_domain_hierarchy.rs`), and a set of bare names would silently merge them.
#[test]
fn every_corpus_module_has_a_generated_case() {
    use std::collections::BTreeSet;

    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.filter_map(Result::ok) {
            let p = e.path();
            if p.is_dir() {
                // `old/` is excluded here BECAUSE it is excluded from the sweep
                // (build.rs `collect_rs`, 2026-08-26): examples/cpu/old/ is
                // untracked scratch whose pre-subset spellings (`Vec` ports) the
                // admissible grammar rejects, so its cases could never compile.
                // The two scans must prune identically or this guard reports the
                // deliberate exclusion as silent coverage loss — which is exactly
                // what it did when only build.rs was changed, working as designed.
                if p.file_name().is_some_and(|n| n == "old") {
                    continue;
                }
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

    let mut found: BTreeSet<String> = BTreeSet::new();
    for path in files {
        let src = std::fs::read_to_string(&path).expect("source is readable");
        let file = syn::parse_file(&src)
            .unwrap_or_else(|e| panic!("{} does not parse standalone: {e}", path.display()));
        for item in &file.items {
            if let syn::Item::Fn(f) = item {
                if f.attrs.iter().any(|a| {
                    a.path().segments.last().is_some_and(|s| s.ident == "hardware")
                }) {
                    // Mirror build.rs's wrapper naming, so the two sets are comparable.
                    let rel = path.strip_prefix(root).unwrap_or(&path).with_extension("");
                    let mut w: String = rel
                        .to_string_lossy()
                        .chars()
                        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                        .collect();
                    let is_example = path.components().any(|c| c.as_os_str() == "examples");
                    w = w
                        .trim_start_matches("tests_fixtures_")
                        .trim_start_matches("examples_")
                        .to_string();
                    let prefix = if is_example { "ex_" } else { "fx_" };
                    found.insert(format!("{prefix}{w}::{}", f.sig.ident));
                }
            }
        }
    }

    let covered: BTreeSet<String> = COVERED.iter().map(|s| s.to_string()).collect();
    let missing: Vec<&String> = found.difference(&covered).collect();
    let extra: Vec<&String> = covered.difference(&found).collect();
    assert!(
        missing.is_empty(),
        "corpus modules with no generated case — the sweep silently stopped covering \
         them: {missing:?}"
    );
    assert!(
        extra.is_empty(),
        "generated cases for modules that no longer exist (stale OUT_DIR?): {extra:?}"
    );
}

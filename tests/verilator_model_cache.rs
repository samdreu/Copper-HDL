//! The built-model cache in `copper-sim/src/verification.rs`.
//!
//! A full regression Verilates a few hundred models; the cache lets a repeat run with
//! unchanged emitted SystemVerilog rebuild none of them. These tests pin the three
//! properties that make that safe rather than merely fast:
//!
//!   * an identical model is a **hit** (no Verilator run) with the same key;
//!   * a model that differs by one character is a **miss** with a different key and
//!     — against the same stimulus — a different verdict, so a stale model can
//!     never be served for a changed one;
//!   * two invocations of one model at the same time **both pass** and leave exactly
//!     one complete entry — the `det_010_independent_golden` pattern that once made
//!     a shared work directory clobber itself.
//!
//! Every model here carries a per-run nonce in a comment, so the first call of each
//! test is a miss even when the cache already holds the previous run's entries.

mod common;

use common::verilator_available;
use copper_core::Logic;
use copper_sim::{
    verify_with_verilator_report, verilator_cache_root, CacheOutcome, SimulationTrace,
    VerilatorCachePolicy, VerilatorRun,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const MODULE: &str = "cache_probe";

/// Unique per call across runs and threads: the nonce that forces a first miss.
fn nonce() -> String {
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    format!("{}_{}_{}", std::process::id(), t, SEQ.fetch_add(1, Ordering::Relaxed))
}

/// A one-bit combinational module: `y = a` when `inverted` is false, `y = ~a`
/// when true. No clock, so no `clk` anywhere in the text (the harness sniffs for it).
fn write_probe(tag: &str, inverted: bool) -> String {
    let dir = PathBuf::from("target/verilator");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{MODULE}_{tag}.sv"));
    let body = if inverted { "~a" } else { "a" };
    fs::write(
        &path,
        format!(
            "// nonce {tag}\nmodule {MODULE}(input logic a, output logic y);\n  assign y = {body};\nendmodule\n"
        ),
    )
    .unwrap();
    path.to_string_lossy().into_owned()
}

/// The stimulus `a = 0,1,0,1,1,0` with the passthrough's expected `y = a`.
fn passthrough_trace() -> SimulationTrace {
    let mut trace = SimulationTrace::new();
    for (cycle, bit) in [0u8, 1, 0, 1, 1, 0].into_iter().enumerate() {
        let l = if bit == 1 { Logic::One } else { Logic::Zero };
        trace.add_cycle(cycle, vec![("a".to_string(), vec![l])], vec![("y".to_string(), vec![l])]);
    }
    trace
}

fn verify(sv: &str, vcd: Option<&str>, policy: VerilatorCachePolicy) -> VerilatorRun {
    verify_with_verilator_report(sv, MODULE, &passthrough_trace(), vcd, &[], policy)
        .unwrap_or_else(|e| panic!("harness failure (not a mismatch): {e}"))
}

/// What a complete entry holds, in `fs::read_dir`-sorted (byte) order.
fn expected_entry_files() -> Vec<String> {
    let mut v = vec!["COMPLETE".to_string(), "manifest.txt".to_string(), format!("V{MODULE}")];
    v.sort();
    v
}

fn entry_files(key: &str) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(verilator_cache_root().join(key))
        .unwrap_or_else(|e| panic!("cache entry {key} missing: {e}"))
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn an_identical_rebuild_is_a_hit_with_the_same_key() {
    if !verilator_available() {
        return;
    }
    let sv = write_probe(&nonce(), false);

    let first = verify(&sv, None, VerilatorCachePolicy::Enabled);
    assert!(first.passed);
    let key = match &first.cache {
        CacheOutcome::Miss { key } => key.clone(),
        other => panic!("a fresh model must miss, got {other:?}"),
    };
    assert_eq!(key.len(), 64, "key is a SHA-256 hex digest");
    assert_eq!(
        entry_files(&key),
        expected_entry_files(),
        "the entry holds the executable, the key material and the completion marker only"
    );

    let second = verify(&sv, None, VerilatorCachePolicy::Enabled);
    assert!(second.passed);
    assert_eq!(second.cache, CacheOutcome::Hit { key: key.clone() });

    // The manifest is the exact key material: its hash IS the directory name.
    let manifest = fs::read(verilator_cache_root().join(&key).join("manifest.txt")).unwrap();
    assert_eq!(copper_sim::sha256_hex(&manifest), key);
}

/// Stale-model protection. The passthrough passes; flip one character of the SV
/// (`a` → `~a`) and the same stimulus must now be a *miss* under a *different* key
/// and must *fail* — if the cache had served the passthrough's executable for the
/// inverter's source, this would pass, which is the one outcome worse than no cache.
#[test]
fn a_one_character_change_misses_and_changes_the_verdict() {
    if !verilator_available() {
        return;
    }
    let tag = nonce();
    let good = write_probe(&tag, false);
    let first = verify(&good, None, VerilatorCachePolicy::Enabled);
    assert!(first.passed);
    let good_key = first.cache.key().unwrap().to_string();

    // Same path, same nonce, one character different.
    let bad = write_probe(&tag, true);
    assert_eq!(good, bad, "the edit is in place, at the same path");
    let changed = verify(&bad, None, VerilatorCachePolicy::Enabled);
    let bad_key = match &changed.cache {
        CacheOutcome::Miss { key } => key.clone(),
        other => panic!("a changed model must miss, got {other:?}"),
    };
    assert_ne!(bad_key, good_key);
    assert!(!changed.passed, "the inverter must NOT pass the passthrough's trace");
    assert!(changed.output.contains("FAIL: Cycle 0 y expected 0 got 1"), "{}", changed.output);

    // And the failing model is itself cached, correctly: a repeat is a hit and still fails.
    let again = verify(&bad, None, VerilatorCachePolicy::Enabled);
    assert_eq!(again.cache, CacheOutcome::Hit { key: bad_key });
    assert!(!again.passed);

    // Restoring the source restores the hit on the original entry, which the bad
    // build neither overwrote nor disturbed.
    let restored = write_probe(&tag, false);
    let back = verify(&restored, None, VerilatorCachePolicy::Enabled);
    assert_eq!(back.cache, CacheOutcome::Hit { key: good_key });
    assert!(back.passed);
}

/// The `det_010_independent_golden` shape: several threads verify the same model
/// at the same moment. All must pass, every miss must have built in its own
/// directory, and the race to publish must leave exactly one complete entry that a
/// later call then hits.
#[test]
fn parallel_verifications_of_one_model_all_pass_and_publish_once() {
    if !verilator_available() {
        return;
    }
    let sv = write_probe(&nonce(), false);
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let sv = sv.clone();
            std::thread::spawn(move || verify(&sv, None, VerilatorCachePolicy::Enabled))
        })
        .collect();
    let runs: Vec<VerilatorRun> = threads.into_iter().map(|t| t.join().unwrap()).collect();

    let mut keys: Vec<&str> = runs.iter().map(|r| r.cache.key().expect("cache consulted")).collect();
    keys.dedup();
    assert_eq!(keys.len(), 1, "one model, one key: {keys:?}");
    assert!(runs.iter().all(|r| r.passed), "{runs:#?}");
    assert!(
        runs.iter().any(|r| matches!(r.cache, CacheOutcome::Miss { .. })),
        "a fresh model: at least one thread built it"
    );

    let key = keys[0].to_string();
    assert_eq!(entry_files(&key), expected_entry_files());
    // No staging directory survived the race, won or lost.
    let strays: Vec<String> = fs::read_dir(verilator_cache_root())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(".staging_"))
        .collect();
    assert!(strays.is_empty(), "staging directories left behind: {strays:?}");

    let later = verify(&sv, None, VerilatorCachePolicy::Enabled);
    assert_eq!(later.cache, CacheOutcome::Hit { key });
    assert!(later.passed);
}

/// A traced run writes its VCD to the per-invocation path it was given, never into
/// the cache: two traced runs of one model, different paths, both files written,
/// and the second is a hit (the path is not in the key).
#[test]
fn traced_runs_share_the_model_but_not_the_waveform() {
    if !verilator_available() {
        return;
    }
    let tag = nonce();
    let sv = write_probe(&tag, false);
    let vcd_a = format!("target/verilator/{MODULE}_{tag}_a.vcd");
    let vcd_b = format!("target/verilator/{MODULE}_{tag}_b.vcd");
    let untraced = verify(&sv, None, VerilatorCachePolicy::Enabled);
    let a = verify(&sv, Some(&vcd_a), VerilatorCachePolicy::Enabled);
    let b = verify(&sv, Some(&vcd_b), VerilatorCachePolicy::Enabled);
    assert!(untraced.passed && a.passed && b.passed);

    let key_a = match &a.cache {
        CacheOutcome::Miss { key } => key.clone(),
        other => panic!("first traced build must miss, got {other:?}"),
    };
    assert_ne!(Some(key_a.as_str()), untraced.cache.key(), "--trace is part of the key");
    assert_eq!(b.cache, CacheOutcome::Hit { key: key_a.clone() }, "the VCD path is not");

    for vcd in [&vcd_a, &vcd_b] {
        let text = fs::read_to_string(vcd).unwrap_or_else(|e| panic!("{vcd} not written: {e}"));
        assert!(text.contains("$enddefinitions"), "{vcd} is not a VCD");
    }
    let in_cache: Vec<String> = fs::read_dir(verilator_cache_root().join(&key_a))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".vcd"))
        .collect();
    assert!(in_cache.is_empty(), "a run wrote into the cache entry: {in_cache:?}");
}

/// `COPPER_VERILATOR_CACHE=0`: the cache is neither read nor written, and the
/// verdict is unchanged.
#[test]
fn a_disabled_cache_builds_every_time_and_publishes_nothing() {
    if !verilator_available() {
        return;
    }
    let sv = write_probe(&nonce(), false);
    let cached = verify(&sv, None, VerilatorCachePolicy::Enabled);
    let key = cached.cache.key().unwrap().to_string();

    let off = verify(&sv, None, VerilatorCachePolicy::Disabled);
    assert!(off.passed);
    assert!(
        matches!(&off.cache, CacheOutcome::Bypassed { reason } if reason.contains("COPPER_VERILATOR_CACHE=0")),
        "{:?}",
        off.cache
    );

    // A model the cache never saw stays unseen when disabled.
    let fresh = write_probe(&nonce(), false);
    let off_fresh = verify(&fresh, None, VerilatorCachePolicy::Disabled);
    assert!(off_fresh.passed);
    let entries_with_fresh: Vec<_> = fs::read_dir(verilator_cache_root())
        .unwrap()
        .filter_map(|e| {
            let p = e.unwrap().path();
            let m = fs::read_to_string(p.join("manifest.txt")).ok()?;
            m.contains(&fs::read_to_string(&fresh).unwrap()).then_some(p)
        })
        .collect();
    assert!(entries_with_fresh.is_empty(), "disabled cache published {entries_with_fresh:?}");

    // The environment parse is what `from_env` applies.
    assert_eq!(VerilatorCachePolicy::from_setting(Some("0")), VerilatorCachePolicy::Disabled);
    let _ = key;
}

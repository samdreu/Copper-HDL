//! The agreement relation's verdicts (SPEC §III.2): what the differential harness
//! reports for one stimulus sequence beyond "the legs agreed", and the ledger that
//! `tools/regression.sh` counts them from (guard G-G).
//!
//! A sequence is one environment (an input stream from the design's initial state,
//! III.2.0). Its verdict is one of:
//!
//! * **PASS** — every pair the preconditions admit agreed at every observation
//!   recorded, observation 0 included (III.2.1).
//! * **OUTSIDE** — the simulator showed an X bit at an observation the relation
//!   compares against the SystemVerilog (III.2.2): from that observation on the SV
//!   pair is not compared. Not a pass and not a failure; `sim ≡ ref` still compares
//!   it exactly (an X equals only an X).
//! * **TRUNCATED** — a run-time failure (III.2.4: an undelivered `data()`, an
//!   out-of-range address, an X fed to a condition or an `as_*` view, integer
//!   overflow) ended the coroutine's trace at a resumption; the simulator and the
//!   reference failed at the same one (or the reference did not run), and the SV is
//!   not compared from there.
//! * **DISAGREE** — some pair differs where the preconditions admit the
//!   observation, read by its pattern (III.2.8, [`Pattern`]).
//!
//! Every verdict is printed with its module, sequence, cycle and port, and written
//! to the ledger (`COPPER_VERDICT_LEDGER`, a directory) when one is set, so the
//! regression can count each and refuse to pass with one unprinted.

use std::any::Any;

use copper_core::Logic;

/// How a disagreement reads (SPEC §III.2.8, `copper-ref/README.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Pattern {
    /// sim ≠ ref, SV = ref: a simulator or macro bug, or a Phase C obligation not
    /// yet landed (P12).
    SimNeRefSvEqRef,
    /// sim = ref ≠ SV: a transpiler bug, or an ADMITTED-AND-DIVERGENT row of II.3.3.
    SimEqRefNeSv,
    /// sim = SV ≠ ref: both implementations share a reading Part I does not give —
    /// the reference, or a two-job residue (the one found, `regout_trailing_single_tick`,
    /// agrees on all three legs since Phase C step 4).
    SimEqSvNeRef,
    /// All three differ: a spec gap or an interpreter bug.
    AllDiffer,
    /// sim ≠ SV where the reference leg does not run (a `REF_SKIP` row or class):
    /// unread until the reference covers the module — a transpiler bug, a II.3 row,
    /// or a simulator bug.
    SimNeSvNoRef,
    /// sim ≠ ref where the SV is not compared (it left the relation, or the
    /// disagreement is where a run-time failure falls, III.2.4): a simulator/macro
    /// bug or an interpreter bug.
    SimNeRefSvOutside,
    /// sim ≠ ref in a reference-leg case, which runs no SV at all.
    SimNeRefNoSv,
}

impl Pattern {
    /// The short form the ledger and the guard key on.
    pub fn label(self) -> &'static str {
        match self {
            Pattern::SimNeRefSvEqRef => "sim≠ref,SV=ref",
            Pattern::SimEqRefNeSv => "sim=ref≠SV",
            Pattern::SimEqSvNeRef => "sim=SV≠ref",
            Pattern::AllDiffer => "all-differ",
            Pattern::SimNeSvNoRef => "sim≠SV,no-ref",
            Pattern::SimNeRefSvOutside => "sim≠ref,SV-outside",
            Pattern::SimNeRefNoSv => "sim≠ref,no-SV",
        }
    }

    /// The reading SPEC §III.2.8 gives the pattern.
    pub fn reading(self) -> &'static str {
        match self {
            Pattern::SimNeRefSvEqRef => {
                "sim≠ref, SV=ref → a simulator or macro bug, or a Phase C obligation not yet landed"
            }
            Pattern::SimEqRefNeSv => "sim=ref≠SV → a transpiler bug, or an ADMITTED-AND-DIVERGENT II.3 row",
            Pattern::SimEqSvNeRef => {
                "sim=SV≠ref → the reference, or the two-job residue both implementations share"
            }
            Pattern::AllDiffer => "all three differ → a spec gap or an interpreter bug (read the interpreter first)",
            Pattern::SimNeSvNoRef => {
                "sim≠SV, no reference leg → unread (transpiler, II.3 row or simulator) until the \
                 reference covers the module"
            }
            Pattern::SimNeRefSvOutside => {
                "sim≠ref with the SV outside the relation → a simulator/macro bug or an interpreter bug"
            }
            Pattern::SimNeRefNoSv => "sim≠ref (reference-leg case, no SV) → a simulator/macro bug or an interpreter bug",
        }
    }

    /// Classify one observation's three values. `sv` / `rf` are `None` where that
    /// leg does not compare the observation; `sv_outside` says the SV leg exists but
    /// the observation is outside its pair. Returns `None` if every compared pair
    /// agrees.
    pub fn of(sim: &[Logic], rf: Option<&[Logic]>, sv: Option<u128>, sv_outside: bool, sv_runs: bool) -> Option<Pattern> {
        let sim_int = to_int(sim);
        let sim_ne_sv = matches!((sv, sim_int), (Some(v), Some(s)) if v != s);
        let sim_ne_ref = rf.is_some_and(|r| r != sim);
        match (rf, sv) {
            (Some(r), Some(v)) => {
                let ref_eq_sv = to_int(r) == Some(v);
                match (sim_ne_ref, sim_ne_sv) {
                    (false, false) => None,
                    (false, true) => Some(Pattern::SimEqRefNeSv),
                    (true, false) => Some(Pattern::SimEqSvNeRef),
                    (true, true) if ref_eq_sv => Some(Pattern::SimNeRefSvEqRef),
                    (true, true) => Some(Pattern::AllDiffer),
                }
            }
            (None, Some(_)) => sim_ne_sv.then_some(Pattern::SimNeSvNoRef),
            (Some(_), None) if !sv_runs => sim_ne_ref.then_some(Pattern::SimNeRefNoSv),
            (Some(_), None) => {
                let _ = sv_outside;
                sim_ne_ref.then_some(Pattern::SimNeRefSvOutside)
            }
            (None, None) => None,
        }
    }
}

/// A bit vector as the integer the testbench compares (`None` if any bit is X).
pub fn to_int(v: &[Logic]) -> Option<u128> {
    let mut r = 0u128;
    for (i, b) in v.iter().enumerate() {
        match b {
            Logic::One => r |= 1u128 << i,
            Logic::Zero => {}
            Logic::X => return None,
        }
    }
    Some(r)
}

/// Run one simulator step, catching a panic as its message. The simulator has no
/// error channel: a run-time failure of the coroutine is a Rust panic inside the
/// executor's poll.
pub fn run_step<F: FnOnce()>(f: F) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|p| panic_text(&p))
}

fn panic_text(p: &Box<dyn Any + Send>) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = p.downcast_ref::<String>() {
        s.clone()
    } else {
        "<a panic whose payload is not a string>".to_string()
    }
}

/// The run-time failures of SPEC §III.2.4 (§I.14.6(b)), recognized by the
/// simulator's panic text. Any OTHER panic is not a run-time failure of the
/// coroutine — a combinational loop, an executor invariant, a harness bug — and
/// fails the case instead of truncating it; truncating on an unrecognized panic
/// would turn an executor bug into a counted verdict.
pub fn runtime_failure_kind(msg: &str) -> Option<&'static str> {
    if msg.contains("read pipeline output is not yet valid") {
        Some("an undelivered data() (I.10)")
    } else if msg.contains("is out of range — this memory has") {
        Some("an out-of-range memory address (I.14.6(b))")
    } else if msg.contains("Cannot convert X to") {
        Some("an X fed to a condition or an as_* view (I.14.2(c))")
    } else if msg.contains("with overflow") {
        Some("Rust integer overflow (I.14.3)")
    } else if msg.contains("index out of bounds") {
        Some("an out-of-range index (I.14.6(b))")
    } else {
        None
    }
}

/// The interpreter's error text for the same failure kinds, so a `Runtime` error
/// that is an interpreter invariant is not mistaken for the coroutine's failure.
pub fn ref_runtime_failure_kind(msg: &str) -> Option<&'static str> {
    let m = msg.to_ascii_lowercase();
    // `interp.rs`'s `x_err`: "<view> of an X value (the simulator panics: …)".
    if m.contains("of an x value") {
        Some("an X fed to a condition or an as_* view (I.14.2(c))")
    } else if m.contains("out of bounds") || m.contains("out of range") {
        Some("an out-of-range index or address (I.14.6(b))")
    } else if m.contains("overflow") {
        Some("integer overflow (I.14.3)")
    } else if m.contains("data()") || m.contains("undelivered") {
        Some("an undelivered data() (I.10)")
    } else {
        None
    }
}

/// One sequence's outcome under the relation.
#[derive(Clone, Debug, Default)]
pub struct SeqOutcome {
    pub index: usize,
    pub label: String,
    /// `(global cycle, local cycle or None for obs_0, port)` of the first X
    /// observation that took the SV pair out of the relation.
    pub outside: Option<(usize, Option<usize>, String)>,
    /// `(resumption, message)` of an agreed run-time failure.
    pub truncated: Option<(usize, String)>,
    /// The first disagreement: its pattern and a line naming where.
    pub disagree: Option<(Pattern, String)>,
    /// A disagreement excused by a table row (its sentence), with its pattern.
    pub excused: Option<(Pattern, String)>,
    /// Observation 0 of a `RESET` module is declared outside the SV pair by its row
    /// (III.2.2, "from reset"): recorded so the report can say so.
    pub obs0_declared_out: bool,
}

impl SeqOutcome {
    /// The one word the ledger counts. A sequence is counted once, by the most
    /// severe thing it shows: DISAGREE, EXCUSED, OUTSIDE, TRUNCATED, PASS.
    pub fn verdict(&self) -> &'static str {
        if self.disagree.is_some() {
            "DISAGREE"
        } else if self.excused.is_some() {
            "EXCUSED"
        } else if self.outside.is_some() {
            "OUTSIDE"
        } else if self.truncated.is_some() {
            "TRUNCATED"
        } else {
            "PASS"
        }
    }

    /// The ledger/report line for the sequence.
    pub fn line(&self, case: &str) -> String {
        let mut detail = Vec::new();
        if let Some((p, l)) = &self.disagree {
            detail.push(format!("pattern={} {l} — {}", p.label(), p.reading()));
        }
        if let Some((p, why)) = &self.excused {
            detail.push(format!("excused pattern={} — {why}", p.label()));
        }
        if let Some((c, local, port)) = &self.outside {
            let at = match local {
                None => "observation 0".to_string(),
                Some(l) => format!("cycle {l}"),
            };
            detail.push(format!(
                "OUTSIDE (X vs SV, III.2.2) from {at} (recorded cycle {c}), port {port}"
            ));
        }
        if let Some((r, m)) = &self.truncated {
            detail.push(format!("TRUNCATED (III.2.4) at resumption {r}: {}", one_line(m)));
        }
        if self.obs0_declared_out {
            detail.push("obs_0 declared outside the SV pair by its RESET row".to_string());
        }
        format!(
            "{}\t{case}\tsequence {} \"{}\"\t{}",
            self.verdict(),
            self.index,
            self.label,
            detail.join("; ")
        )
    }
}

pub fn one_line(s: &str) -> String {
    let l = s.lines().next().unwrap_or("").trim();
    if l.len() > 200 {
        format!("{}…", &l[..l.char_indices().take_while(|(i, _)| *i < 200).last().map_or(0, |(i, c)| i + c.len_utf8())])
    } else {
        l.to_string()
    }
}

/// The ledger directory, if the run asked for one (`tools/regression.sh` sets it).
pub const LEDGER_ENV: &str = "COPPER_VERDICT_LEDGER";

/// Append `lines` to a fresh file in the ledger (one file per call: no two writers
/// ever share one). `origin` is `sweep-differential`, `sweep-reference` or `pin`.
pub fn ledger_write(origin: &str, case: &str, lines: &[String]) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let Ok(dir) = std::env::var(LEDGER_ENV) else { return };
    if dir.is_empty() {
        return;
    }
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).expect("create the verdict ledger directory");
    let safe: String = case.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let path = dir.join(format!(
        "{origin}__{safe}__{}_{}.tsv",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let mut text = String::new();
    for l in lines {
        text.push_str(origin);
        text.push('\t');
        text.push_str(l);
        text.push('\n');
    }
    std::fs::write(&path, text).unwrap_or_else(|e| panic!("write the verdict ledger {}: {e}", path.display()));
}

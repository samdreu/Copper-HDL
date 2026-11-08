//! **The conformance ledger cannot go stale silently** (SPEC §III.3; D-20 Phase C
//! step 11; `design_docs/review/MEASUREMENTS.md` §R).
//!
//! SPEC §III.3 records, one line per normative clause of Parts I and II, per row of
//! II.3.3 (per method for row (e)) and per retired rule (II.3.2 rules 7–15), whether
//! the code conforms: **CLOSED** with the fixture that shows it, or **OPEN** with an
//! owner and a reason. This test reads that table out of `design_docs/SPEC.md` and
//! checks, statically:
//!
//! 1. **Completeness** — every numbered clause of Parts I and II (bar the
//!    "Implementation map" and "Carried" clauses, which name the evidence rather than
//!    state a requirement), every rule row of II.1.2's table, every row of II.3.3,
//!    every method named in row (e)'s shape, and every retired rule 7–15 has exactly
//!    one ledger line; and the ledger names no clause the SPEC does not have. A new
//!    clause without a line fails here.
//! 2. **Well-formedness** — each line's status is CLOSED or OPEN; a CLOSED line names
//!    at least one fixture token and the guard that runs it; an OPEN line names an
//!    owner and a reason.
//! 3. **Every CLOSED fixture exists and is reachable** — `sweep:<file>::<fn>`: the file
//!    has `fn <fn>`, the sweep emitted a case for it (`corpus_manifest.rs`), and
//!    `build.rs` does not `SKIP` it; `ui:{fail,pass}/<case>` and `coreui:fail/<case>`:
//!    the trybuild case exists (a fail case with its `.stderr`); `test:<file>::<fn>`:
//!    the test file has `fn <fn>` under `#[test]`, not `#[ignore]`d;
//!    `example:<name>`: a registered `[[example]]`. A row-(e) line additionally needs
//!    its method's name in one of its fixtures' source.
//!
//! What a static test cannot see — that each fixture actually RAN and PASSED in this
//! run, under the sweep's full stimulus policy — is checked by `tools/regression.sh`'s
//! G-K, which reads the `REQUIRE` lines this test writes to `COPPER_LEDGER_REPORT` and
//! finds each one's `... ok` line in the run's own test log (a swept module needs both
//! its `_differential` and its `_reference_leg` case; an example its PASS status).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

include!(concat!(env!("OUT_DIR"), "/corpus_manifest.rs"));

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
}

/// One line of the ledger table.
#[derive(Debug)]
struct Line {
    id: String,
    fixtures: Vec<String>,
    guards: String,
    status: String,
    owner: String,
}

/// The section between `### III.3` and the next `## ` / `### ` heading at its level or
/// above that is not one of its own subsections.
fn ledger_section(spec: &str) -> &str {
    let start = spec.find("\n### III.3 ").expect("SPEC.md has a §III.3 heading");
    let rest = &spec[start + 1..];
    let end = rest[4..].find("\n## ").map(|i| i + 4).unwrap_or(rest.len());
    &rest[..end]
}

fn cells(row: &str) -> Vec<String> {
    let t = row.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    t.split('|').map(|c| c.trim().to_string()).collect()
}

/// Backticked spans of a cell.
fn ticks(cell: &str) -> Vec<String> {
    cell.split('`').skip(1).step_by(2).map(str::to_string).collect()
}

/// The ledger's tables: every row whose first cell is a ledger id (the header row
/// `| Line | Clause | …` and separator rows are skipped). Columns: id, clause title,
/// fixture tokens, guards, status, owner / reason.
fn parse_ledger(sec: &str) -> Vec<Line> {
    let mut out = Vec::new();
    for row in sec.lines().filter(|l| l.trim_start().starts_with('|')) {
        let c = cells(row);
        if c.len() < 6 || c[0] == "Line" || c[0].starts_with("---") {
            continue;
        }
        let id = c[0].trim_matches('*').to_string();
        out.push(Line {
            id,
            fixtures: ticks(&c[2]).into_iter().filter(|t| is_token(t)).collect(),
            guards: c[3].clone(),
            status: c[4].trim_matches('*').to_string(),
            owner: c[5].clone(),
        });
    }
    out
}

fn is_token(t: &str) -> bool {
    ["sweep:", "ui:", "coreui:", "test:", "example:", "gone:"].iter().any(|p| t.starts_with(p))
}

/// The ids SPEC Parts I and II require a ledger line for.
fn required_ids(spec: &str) -> BTreeSet<String> {
    let part3 = spec.find("\n## Part III").expect("SPEC.md has a Part III");
    let body = &spec[..part3];
    let mut ids = BTreeSet::new();
    // 1. Numbered clauses: `**I.3.4 Title.**` at the start of a line.
    for l in body.lines() {
        let Some(rest) = l.strip_prefix("**") else { continue };
        let Some((num, after)) = rest.split_once(' ') else { continue };
        let is_num = (num.starts_with("I.") || num.starts_with("II."))
            && num.split('.').skip(1).all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
            && num.split('.').count() == 3;
        if !is_num {
            continue;
        }
        let title = after.trim_start();
        if title.starts_with("Implementation map") || title.starts_with("Carried") {
            continue;
        }
        ids.insert(num.to_string());
    }
    // 2. II.1.2's rule rows, II.2.1's derived rules, II.3.3's rows, row (e)'s methods,
    //    II.3.2's retired rules 7–15.
    let sub = |from: &str, to: &str| -> &str {
        let a = body.find(from).unwrap_or_else(|| panic!("SPEC.md has {from}"));
        let b = body[a..].find(to).map(|i| a + i).unwrap_or(body.len());
        &body[a..b]
    };
    for row in sub("**II.1.2 ", "**II.1.3 ").lines().filter(|l| l.starts_with("| R")) {
        let r = cells(row)[0].clone();
        if r.len() > 1 && r[1..].chars().all(|c| c.is_ascii_digit() || c == '/' || c == 'R') {
            ids.insert(format!("II.1.2 {r}"));
        }
    }
    for l in sub("**II.2.1 ", "**II.2.2 ").lines() {
        if let Some(rest) = l.strip_prefix("- **D") {
            let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !n.is_empty() {
                ids.insert(format!("II.2.1 D{n}"));
            }
        }
    }
    for row in sub("**II.3.3 ", "**II.3.4 ").lines().filter(|l| l.starts_with("| (")) {
        let c = cells(row);
        let r = c[0].clone();
        if r == "(e)" {
            for m in ticks(&c[1]) {
                // Methods only: a lowercase identifier (`Bits`, a type, is not a line).
                if m.starts_with(|ch: char| ch.is_ascii_lowercase()) && m.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
                    ids.insert(format!("II.3.3 (e) {m}"));
                }
            }
        } else {
            ids.insert(format!("II.3.3 {r}"));
        }
    }
    for row in sub("**II.3.2 ", "**II.3.3 ").lines().filter(|l| l.starts_with("| ")) {
        let c = cells(row);
        if let Ok(n) = c[0].parse::<u32>() {
            if (7..=15).contains(&n) {
                ids.insert(format!("retired {n}"));
            }
        }
    }
    ids
}

/// `build.rs`'s `SKIP` names (the first string of each tuple in the table).
fn skip_names(build_rs: &str) -> BTreeSet<String> {
    let a = build_rs.find("const SKIP: &[(&str, &str)] = &[").expect("build.rs SKIP");
    let b = a + build_rs[a..].find("\n];").expect("end of SKIP");
    let mut names = BTreeSet::new();
    let mut prev_open = false;
    for l in build_rs[a..b].lines() {
        let t = l.trim();
        if prev_open && t.starts_with('"') && t.ends_with("\",") {
            names.insert(t.trim_end_matches(',').trim_matches('"').to_string());
        }
        prev_open = t == "(";
    }
    names
}

/// `build.rs`'s `wrapper_name`, restated (two copies of the rule; the manifest check
/// below fails if they ever disagree, since the computed name would be absent).
fn wrapper_name(rel: &str) -> String {
    let stem = rel.strip_suffix(".rs").unwrap_or(rel);
    let mut s: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    s = s.trim_start_matches("tests_fixtures_").trim_start_matches("examples_").to_string();
    let prefix = if rel.starts_with("examples/") { "ex_" } else { "fx_" };
    format!("{prefix}{s}")
}

fn has_fn(src: &str, name: &str) -> bool {
    src.lines().any(|l| {
        let t = l.trim_start();
        [format!("fn {name}("), format!("fn {name}<"), format!("fn {name} (")].iter().any(|p| {
            t.starts_with(p.as_str())
                || t.starts_with(&format!("pub {p}"))
                || t.starts_with(&format!("async {p}"))
                || t.starts_with(&format!("pub async {p}"))
        })
    })
}

/// `fn name` is a `#[test]` (attribute among the lines just above it) and not ignored.
fn is_live_test(src: &str, name: &str) -> Result<(), String> {
    let lines: Vec<&str> = src.lines().collect();
    let Some(i) = lines.iter().position(|l| {
        let t = l.trim_start();
        t.starts_with(&format!("fn {name}(")) || t.starts_with(&format!("async fn {name}("))
    }) else {
        return Err(format!("no `fn {name}(`"));
    };
    let mut attrs = Vec::new();
    for l in lines[..i].iter().rev() {
        let t = l.trim();
        if t.starts_with("#[") || t.starts_with("///") || t.starts_with("//") {
            attrs.push(t);
        } else {
            break;
        }
    }
    if !attrs.iter().any(|a| a.starts_with("#[test]") || a.starts_with("#[tokio::test")) {
        return Err(format!("`fn {name}` is not a #[test]"));
    }
    if attrs.iter().any(|a| a.starts_with("#[ignore")) {
        return Err(format!("`fn {name}` is #[ignore]d"));
    }
    Ok(())
}

/// Where `fn name` is defined in a crate's `src/` (the root crate's included), if anywhere.
fn defined_in(root: &Path, name: &str) -> Option<String> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    for e in std::fs::read_dir(root).expect("repo root").flatten() {
        let p = e.path();
        let n = p.file_name().unwrap().to_string_lossy().to_string();
        if p.is_dir() && (n.starts_with("copper-") || n == "src") {
            walk(&if n == "src" { p.clone() } else { p.join("src") }, &mut files);
        }
    }
    files.sort();
    for f in files {
        let src = read(&f);
        if has_fn(&src, name) || src.lines().any(|l| l.trim_start().starts_with(&format!("pub(crate) fn {name}("))) {
            return Some(f.strip_prefix(root).unwrap_or(&f).display().to_string());
        }
    }
    None
}

fn registered_examples(cargo_toml: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut in_ex = false;
    for l in cargo_toml.lines() {
        let t = l.trim();
        if t.starts_with('[') {
            in_ex = t == "[[example]]";
        } else if in_ex && t.starts_with("name") {
            if let Some(v) = t.split('"').nth(1) {
                out.insert(v.to_string());
            }
        }
    }
    out
}

const MACRO_UI_TEST: (&str, &str) = ("tests/macro_tests.rs", "ui_tests");
const PARITY_UI_FAIL: (&str, &str) =
    ("tests/front_end_parity_ui.rs", "the_transpiler_refuses_every_shape_the_macro_refuses_with_the_same_message");
const PARITY_UI_PASS: (&str, &str) = ("tests/front_end_parity_ui.rs", "the_transpiler_admits_every_shape_the_macro_admits");
const CORE_UI_TEST: (&str, &str) = ("tests/cdc_ui.rs", "cdc_rules_are_rejected_at_compile_time");

#[test]
fn the_conformance_ledger_names_live_fixtures() {
    let r = root();
    let spec = read(&r.join("design_docs/SPEC.md"));
    assert!(!spec.contains("### III.3 Conformance ledger — TBD"), "§III.3 is still TBD");
    let sec = ledger_section(&spec);
    let lines = parse_ledger(sec);
    assert!(lines.len() > 100, "the ledger table parsed to {} lines — the parser lost it", lines.len());

    let build_rs = read(&r.join("build.rs"));
    let skip = skip_names(&build_rs);
    assert!(skip.contains("mod8"), "the SKIP parser lost build.rs's table: {skip:?}");
    let examples = registered_examples(&read(&r.join("Cargo.toml")));
    let manifest: BTreeSet<&str> = SWEEP_MODULES.iter().copied().collect();

    let mut failures = Vec::new();
    // 1. Completeness, both ways; ids unique.
    let required = required_ids(&spec);
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for l in &lines {
        *seen.entry(l.id.as_str()).or_default() += 1;
    }
    for (id, n) in &seen {
        if *n > 1 {
            failures.push(format!("ledger line `{id}` appears {n} times"));
        }
    }
    for id in &required {
        if !seen.contains_key(id.as_str()) {
            failures.push(format!("SPEC requires a ledger line for `{id}`; there is none"));
        }
    }
    // Lines beyond the required set are allowed only for II.3 rows found in Phase C
    // that II.3.3's table does not carry yet (`II.3 found …`) and for row-(e) methods
    // the row owes (`II.3.3 (e) …`).
    for id in seen.keys() {
        let extra_ok = id.starts_with("II.3 found ") || id.starts_with("II.3.3 (e) ");
        if !required.contains(*id) && !extra_ok {
            failures.push(format!("ledger line `{id}` names no clause, rule or row of the SPEC"));
        }
    }

    // 2–3. Each line.
    let mut require: BTreeSet<String> = BTreeSet::new();
    let (mut closed, mut open) = (0usize, 0usize);
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for l in &lines {
        match l.status.as_str() {
            "CLOSED" => closed += 1,
            "OPEN" => {
                open += 1;
                if l.owner.chars().filter(|c| c.is_alphanumeric()).count() < 10 {
                    failures.push(format!("`{}` is OPEN with no owner and reason", l.id));
                }
                continue;
            }
            s => {
                failures.push(format!("`{}` has status `{s}` (CLOSED or OPEN)", l.id));
                continue;
            }
        }
        if l.fixtures.iter().all(|t| t.starts_with("gone:")) {
            failures.push(format!("`{}` is CLOSED with no fixture token that runs", l.id));
        }
        if l.guards.chars().filter(|c| c.is_alphanumeric()).count() < 2 {
            failures.push(format!("`{}` is CLOSED with no guard named", l.id));
        }
        let mut sources = String::new();
        for t in &l.fixtures {
            let (kind, arg) = t.split_once(':').unwrap();
            *kinds.entry(kind).or_default() += 1;
            let mut fail = |m: String| failures.push(format!("`{}`: `{t}`: {m}", l.id));
            match kind {
                "sweep" => {
                    let Some((file, f)) = arg.rsplit_once("::") else { fail("not <file>::<fn>".into()); continue };
                    let p = r.join(file);
                    if !(file.starts_with("tests/fixtures/") || file.starts_with("examples/")) {
                        fail("a swept module lives in tests/fixtures/ or examples/".into());
                    }
                    if !p.is_file() {
                        fail("no such file".into());
                        continue;
                    }
                    let src = read(&p);
                    if !has_fn(&src, f) {
                        fail(format!("the file has no `fn {f}`"));
                    }
                    let w = format!("{}::{f}", wrapper_name(file));
                    if !manifest.contains(w.as_str()) {
                        fail(format!("the sweep emitted no case `{w}`"));
                    }
                    if skip.contains(f) {
                        fail("build.rs SKIPs it — a skipped module closes nothing".into());
                    }
                    if !l.guards.contains("G-D") {
                        fail("a swept fixture is run by G-D; the guard cell does not name it".into());
                    }
                    sources += &src;
                    require.insert(format!("REQUIRE sweep {w}"));
                }
                "ui" | "coreui" => {
                    let Some((dir, case)) = arg.split_once('/') else { fail("not <fail|pass>/<case>".into()); continue };
                    let base = if kind == "ui" { "copper-macros/tests/ui" } else { "copper-core/tests/ui" };
                    let p = r.join(base).join(dir).join(format!("{case}.rs"));
                    if !(dir == "fail" || (dir == "pass" && kind == "ui")) {
                        fail("unknown ui directory".into());
                    }
                    if !p.is_file() {
                        fail(format!("no {}", p.display()));
                        continue;
                    }
                    if dir == "fail" && !p.with_extension("stderr").is_file() {
                        fail("a compile-fail case without its .stderr pins nothing".into());
                    }
                    sources += &read(&p);
                    if kind == "coreui" {
                        require.insert(format!("REQUIRE test {} {}", CORE_UI_TEST.0, CORE_UI_TEST.1));
                    } else {
                        require.insert(format!("REQUIRE test {} {}", MACRO_UI_TEST.0, MACRO_UI_TEST.1));
                        let parity = if dir == "fail" { PARITY_UI_FAIL } else { PARITY_UI_PASS };
                        require.insert(format!("REQUIRE test {} {}", parity.0, parity.1));
                    }
                }
                "test" => {
                    let Some((file, f)) = arg.rsplit_once("::") else { fail("not <file>::<fn>".into()); continue };
                    let p = r.join(file);
                    if !p.is_file() {
                        fail("no such file".into());
                        continue;
                    }
                    let src = read(&p);
                    if let Err(e) = is_live_test(&src, f) {
                        fail(e);
                    }
                    let stem = Path::new(file).file_name().unwrap().to_string_lossy();
                    sources += &src;
                    require.insert(format!("REQUIRE test tests/{stem} {f}"));
                }
                "gone" => {
                    // A retired rule: no definition of the function survives in any crate.
                    if let Some(at) = defined_in(&r, arg) {
                        fail(format!("`fn {arg}` is still defined at {at}"));
                    }
                }
                "example" => {
                    if !examples.contains(arg) {
                        fail("not a registered [[example]]".into());
                    }
                    require.insert(format!("REQUIRE example {arg}"));
                }
                _ => fail("unknown token kind".into()),
            }
        }
        // A row-(e) method line must name a fixture that uses the method.
        if let Some(m) = l.id.strip_prefix("II.3.3 (e) ") {
            let m = m.trim();
            if !sources.contains(m) {
                failures.push(format!("`{}`: no fixture of the line mentions `{m}`", l.id));
            }
        }
    }

    let summary = format!(
        "conformance ledger: {} lines — {closed} CLOSED, {open} OPEN; {} required by the SPEC; fixtures: {}; {} run checks",
        lines.len(),
        required.len(),
        kinds.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", "),
        require.len()
    );
    println!("{summary}");
    for l in lines.iter().filter(|l| l.status == "OPEN") {
        println!("OPEN {}: {}", l.id, l.owner.chars().take(140).collect::<String>());
    }
    if let Ok(p) = std::env::var("COPPER_LEDGER_REPORT") {
        if failures.is_empty() {
            let mut body = format!("{summary}\n");
            for q in &require {
                body += &format!("{q}\n");
            }
            std::fs::write(&p, body).expect("write the ledger report");
        }
    }
    assert!(failures.is_empty(), "{} ledger failure(s):\n  {}", failures.len(), failures.join("\n  "));
}

/// The parsers above against a hand-written ledger, so a parser that silently reads
/// nothing cannot make the real check vacuous.
#[test]
fn the_ledger_parsers_read_what_they_are_given() {
    let sec = "### III.3 X\n\n| Line | Clause | Fixture | Guard | Status | Owner / reason |\n|---|---|---|---|---|---|\n\
               | I.0.1 | the coroutine | `sweep:tests/fixtures/a.rs::m`, `test:tests/x.rs::t` | G-D, G-F | CLOSED | — |\n\
               | II.3.3 (e) concat | concat | — | — | OPEN | the transpiler: a lowering owed |\n";
    let ls = parse_ledger(sec);
    assert_eq!(ls.len(), 2);
    assert_eq!(ls[0].id, "I.0.1");
    assert_eq!(ls[0].fixtures, vec!["sweep:tests/fixtures/a.rs::m", "test:tests/x.rs::t"]);
    assert_eq!(ls[1].status, "OPEN");
    assert_eq!(wrapper_name("tests/fixtures/bits_methods_dut.rs"), "fx_bits_methods_dut");
    assert_eq!(wrapper_name("examples/cdc/flag_crossing.rs"), "ex_cdc_flag_crossing");
    let spec = "## Part I\n**I.0.1 The coroutine.** x\n**I.0.7 Implementation map.** y\n**I.0.8 Carried NOT-YET list.**\n\
                **II.1.2 The shape rules.**\n| R1 | a |\n| R13/R14 | b |\n**II.1.3 Parity.**\n\
                **II.2.1 Derived.**\n- **D1 — x** y\n- **D4** — retired.\n**II.2.2 Directions.**\n\
                **II.3.2 Sort.**\n| 7 | x |\n| 16 | y |\n**II.3.3 Rows.**\n| (a) | s |\n| (e) | `concat`, `get` | m |\n**II.3.4 X.**\n## Part III\n";
    let ids = required_ids(spec);
    let want: BTreeSet<String> = [
        "I.0.1", "II.1.2", "II.1.2 R1", "II.1.2 R13/R14", "II.1.3", "II.2.1", "II.2.1 D1", "II.2.1 D4", "II.2.2",
        "II.3.2", "II.3.3", "II.3.3 (a)",
        "II.3.3 (e) concat", "II.3.3 (e) get", "II.3.4", "retired 7",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    assert_eq!(ids, want);
}

#!/usr/bin/env bash
# tools/regression.sh — the Copper regression runner.
#
# Copper is a Rust-embedded HDL that (a) simulates hardware in Rust and
# (b) transpiles `#[hardware]` modules to SystemVerilog, then proves the two agree by
# compiling the SV with Verilator and comparing cycle-by-cycle. "Does it work?" means
# that agreement holds — so this script drives all four surfaces that can break it:
#
#   1. cargo build --workspace     the crates compile
#   2. copper-transpile <file>     the standalone SV emitter / CLI
#   3. cargo test --workspace      every test target
#   4. the example binaries, in parallel   Copper-sim vs Verilator equivalence
#
# THE DEFAULT IS THE FULL REGRESSION — bare, it runs everything. That is deliberate:
# an example's `main()` is a real self-check (several assert against independent
# BaseJump Verilog and `exit(1)` on mismatch), and `cargo test` only *builds*
# examples, it never runs them. A driver that quietly covered a subset would be the
# same failure mode this project keeps hitting.
#
# Usage:
#   tools/regression.sh                 FULL regression (default)
#   tools/regression.sh --quick         fast inner loop: build + CLI + a few examples
#   tools/regression.sh --no-examples   build + CLI + tests (no Verilator needed)
#   tools/regression.sh --no-test       skip `cargo test --workspace`
#   tools/regression.sh --example NAME  run only the named example(s)
#
# Anything less than the full run prints "PARTIAL", so a subset cannot be mistaken
# for a clean regression. Exits non-zero on the first failure.
#
# ── Why the guards exist ─────────────────────────────────────────────────────
# Repeatedly, the bug here has been *a check that silently did not run*: examples
# never executed, Verilator failures swallowed as "not installed", an `#[ignore]`
# whose stated reason had stopped being true, a test file with no binary. Each looked
# green. So this script does not just run things — it asserts that it ran them:
#
#   G-A  every examples/**.rs is registered as a [[example]] in Cargo.toml
#   G-B  every registered example actually ran
#   G-C  every tests/*.rs (root and per-crate) produced a test binary that ran
#   G-D  the corpus differential sweep covered every #[hardware] module and ran
#   G-E  how much of that sweep is anchored to an INDEPENDENT hand-written Verilog
#        reference rather than only cross-checked against the transpiler (printed,
#        not enforced)
#   G-F  the REFERENCE-INTERPRETER leg (copper-ref, the spec made executable —
#        design_docs/SPEC_OVERVIEW.md §7) ran on every module it covers — as many
#        `ok` cases as build.rs generated to run, the clocked, memory (I.10) and
#        combinational (I.11) legs each non-empty, and the design leg (I.13) — and
#        the modules it skips are skipped with a sentence (build.rs REF_SKIP)
#   G-G  the AGREEMENT RELATION's verdicts (SPEC §III.2): every swept case wrote its
#        per-sequence verdicts to the ledger, the counts of PASS, OUTSIDE (an X vs the
#        SV), TRUNCATED (a run-time failure), EXCUSED (a signature class — none since
#        the observation-0 plain-`Out` class emptied with D-8) and each disagreement pattern are printed, no sweep disagreement survives,
#        and every REF_SKIP row excusing a measured disagreement is matched by a pin
#        that measured the same pattern this run
#        (G-H, the sweep re-run under the one-resumption executor while it was behind
#        `COPPER_RESUMPTION`, merged into G-G when it became the only executor —
#        Phase C step 3, 2026-11-06)
#   G-I  the PROJECTION (SPEC §III.1 P1/P15, tests/projection_relation.rs): the
#        relation R between the emitted control state and the coroutine's
#        continuations was built and every transition checked on every clocked module
#        the sweep covers, the initial state included; its counts are printed
#        (G-H is retired, see above)
#   G-J  the two CONSTANT EVALUATORS agree (D-21 B2, SPEC I.14.4,
#        tests/const_evaluators_agree.rs): the interpreter's and copper_analysis's
#        evaluated every corpus const item, pre-loop initializer, counted-`for` bound
#        and index both ways (initializers also against the transpiler's stated
#        power-on value), 0 DISAGREE; its counts are printed
#   G-K  the CONFORMANCE LEDGER (SPEC §III.3, tests/conformance_ledger.rs): every
#        clause, rule and II.3 row has a CLOSED line naming fixtures that exist, or
#        an OPEN line with an owner; and every CLOSED line's fixture RAN AND PASSED in
#        this run — a swept module's `_differential` and `_reference_leg` cases, a
#        named test's `ok` in its own binary, an example's PASS; its counts are printed
#
# and it prints the `#[ignore]`d tests every run, because a skipped check that prints
# nothing is indistinguishable from a passing one.
set -uo pipefail

# A stale VERILATOR_ROOT makes `verilator` refuse to run. The Rust harness clears it
# internally (see copper-sim `verilator_status`), but this script also probes the
# binary directly, so clear it here too.
unset VERILATOR_ROOT

# Every Verilator model recompiles the Verilator runtime C++ from scratch, and a full
# run builds a few hundred models. Verilator's generated makefiles honour OBJCACHE
# (`verilated.mk`), so when ccache is installed hand it over: the runtime objects
# then compile once, and a module whose emitted SystemVerilog did not change is a
# cache hit end to end. Purely a build-speed knob — the models are identical.
if [ -z "${OBJCACHE:-}" ] && command -v ccache >/dev/null 2>&1; then
  export OBJCACHE=ccache
fi
# Built models themselves are cached under target/verilator/cache, keyed on the
# full emitted SystemVerilog + testbench + flags + Verilator version (see
# copper-sim/src/verification.rs). `COPPER_VERILATOR_CACHE=0 tools/regression.sh`
# forces every model to be rebuilt; `cargo clean` drops the cache.

cd "$(git rev-parse --show-toplevel 2>/dev/null || echo .)" || exit 2

# Representative fast set for --quick: combinational, generic monomorphization,
# sequential/FSM, and memory.
REP_EXAMPLES=(one_bit_comparator mux shift_register traffic_light_fsm dual_port_ram)

QUICK=0
RUN_TEST=1
NO_EXAMPLES=0
CUSTOM_EXAMPLES=()

while [ $# -gt 0 ]; do
  case "$1" in
    --quick)        QUICK=1; RUN_TEST=0 ;;
    --no-test)      RUN_TEST=0 ;;
    --no-examples)  NO_EXAMPLES=1 ;;
    --example)      shift; [ $# -gt 0 ] || { echo "--example needs a name" >&2; exit 2; }
                    CUSTOM_EXAMPLES+=("$1"); RUN_TEST=0 ;;
    -h|--help)      sed -n '2,32p' "$0"; exit 0 ;;
    *)              echo "unknown arg: $1 (try --help)" >&2; exit 2 ;;
  esac
  shift
done

PARTIAL=0
[ "$QUICK" -eq 1 ] && PARTIAL=1
[ "$NO_EXAMPLES" -eq 1 ] && PARTIAL=1
[ "$RUN_TEST" -eq 0 ] && PARTIAL=1
[ "${#CUSTOM_EXAMPLES[@]}" -gt 0 ] && PARTIAL=1

fail() { echo; echo "REGRESSION FAIL: $*" >&2; exit 1; }
step() { echo; echo "==== $* ===="; }

START=$(date +%s)
elapsed() { echo "$(( $(date +%s) - START ))s"; }

TMP="$(mktemp -d)"
FAILURE_LOG="./regression-failure.log"
# Keep the log when something failed. A regression that deletes its own evidence
# forces you to reproduce before you can even read it — and an intermittent failure
# may not reproduce. (This is how the det_010 work-dir race was finally diagnosed.)
KEEP_LOG=0
cleanup() {
  if [ "$KEEP_LOG" -eq 1 ] && [ -f "$TMP/test.log" ]; then
    cp -f "$TMP/test.log" "$FAILURE_LOG" 2>/dev/null &&
      echo "failing log preserved at $FAILURE_LOG" >&2
  fi
  rm -rf "$TMP"
}
trap cleanup EXIT

# Every [[example]] name in Cargo.toml, in order.
# (macOS ships bash 3.2 — no `mapfile`, so read line by line.)
registered_examples() {
  grep -A1 '^\[\[example\]\]' Cargo.toml | grep 'name =' | sed 's/.*"\(.*\)".*/\1/'
}

# G-G: read the verdict ledger. $1 the ledger dir, $2 the differential cases that
# ran (0: none expected), $3 the reference-leg cases that ran, $4 whether the
# pin/table checks apply (1 for a full `cargo test`, 0 for --quick's leg-only run).
gg_relation() {
  local L="$1" want_d="$2" want_r="$3" full="$4"
  [ -d "$L" ] || fail "G-G: no verdict ledger at $L"
  local all; all="$(cat "$L"/*.tsv 2>/dev/null)"
  [ -n "$all" ] || fail "G-G: the verdict ledger is empty — no case reported a verdict"
  local d_cases r_cases
  d_cases=$(printf '%s\n' "$all" | awk -F'\t' '$1=="sweep-differential" && $2=="CASE"' | wc -l | tr -d ' ')
  r_cases=$(printf '%s\n' "$all" | awk -F'\t' '$1=="sweep-reference" && $2=="CASE"' | wc -l | tr -d ' ')
  [ "$d_cases" -eq "$want_d" ] || fail "G-G: $want_d differential case(s) ran but $d_cases wrote a verdict"
  [ "$r_cases" -eq "$want_r" ] || fail "G-G: $want_r reference-leg case(s) ran but $r_cases wrote a verdict"
  local origin v n line
  for origin in sweep-differential sweep-reference; do
    line=""
    for v in PASS OUTSIDE TRUNCATED EXCUSED DISAGREE; do
      n=$(printf '%s\n' "$all" | awk -F'\t' -v o="$origin" -v v="$v" '$1==o && $2==v' | wc -l | tr -d ' ')
      line="$line $n $v,"
    done
    echo "  G-G ${origin#sweep-}: sequences —${line%,}"
  done
  local declared
  declared=$(printf '%s\n' "$all" | awk -F'\t' '$1=="sweep-differential"' | grep -c "declared outside the SV pair" || true)
  echo "       observation 0 declared outside the SV pair by a RESET row: $declared sequence(s)"
  # Patterns: of the excused sequences, and of every disagreement (pinned or not).
  local pats
  pats=$(printf '%s\n' "$all" | awk -F'\t' '$1=="sweep-differential" && $2=="EXCUSED"' | grep -o 'excused pattern=[^ ]*' | sed 's/excused pattern=//' | sort | uniq -c | awk '{printf "%s%s %s", (NR>1?", ":""), $1, $2}')
  echo "       excused by pattern: ${pats:-none}"
  local nd
  nd=$(printf '%s\n' "$all" | awk -F'\t' '($1=="sweep-differential" || $1=="sweep-reference") && $2=="DISAGREE"' | wc -l | tr -d ' ')
  if [ "$nd" -gt 0 ]; then
    printf '%s\n' "$all" | awk -F'\t' '($1=="sweep-differential" || $1=="sweep-reference") && $2=="DISAGREE"' | cut -c1-200 | head -10
    fail "G-G: $nd swept sequence(s) DISAGREE — neither pinned nor excused"
  fi
  [ "$full" -eq 1 ] || return 0
  grep -q "^test excused_disagreements_are_written_to_the_ledger \.\.\. ok" "$TMP/test.log" \
    || fail "G-G: the excused-disagreement table was not written to the ledger"
  local pins
  pins=$(printf '%s\n' "$all" | awk -F'\t' '$1=="pin" && $2=="PINNED"{print $3" "$5}')
  local npins; npins=$(printf '%s\n' "$pins" | grep -c . || true)
  echo "       pinned disagreements: $npins"
  printf '%s\n' "$pins" | grep . | sed 's/^/         /'
  local row m p
  while IFS= read -r row; do
    [ -n "$row" ] || continue
    m=$(echo "$row" | cut -f3); p=$(echo "$row" | cut -f5)
    printf '%s\n' "$pins" | grep -qx "$m $p" \
      || fail "G-G: REF_SKIP excuses '$m' as $p, but no pin measured that pattern this run"
    echo "       REF_SKIP '$m' ($p) — matched by its pin"
  done <<EOF_ROWS
$(printf '%s\n' "$all" | awk -F'\t' '$1=="table" && $2=="ROW"')
EOF_ROWS
  echo "  G-G ok: every case's verdicts are counted; no unexcused, unpinned disagreement"
}

# ── 1. build ─────────────────────────────────────────────────────────────────
step "1/4  build workspace"
cargo build --workspace 2>&1 | tail -3
[ "${PIPESTATUS[0]}" -eq 0 ] || fail "cargo build --workspace"

# ── 2. the standalone CLI ────────────────────────────────────────────────────
step "2/4  copper-transpile CLI"
BIN=target/debug/copper-transpile
[ -x "$BIN" ] || cargo build -q -p copper-codegen || fail "building copper-transpile"

# Capture into variables, then grep. Piping the binary straight into `grep -q` trips
# `set -o pipefail`: grep closes the pipe on the first match, the transpiler dies with
# SIGPIPE, and pipefail then fails the whole pipeline.
DUT=examples/combinational/one_bit_comparator.rs
LIST=$("$BIN" "$DUT" --list) || fail "--list exited non-zero"
echo "$LIST" | grep -q one_bit_comparator || fail "--list did not report the module"

SV=$("$BIN" "$DUT") || fail "transpile exited non-zero"
echo "$SV" | grep -q "^module one_bit_comparator" || fail "emitted SV missing the module"
echo "$SV" | grep -q "^endmodule" || fail "emitted SV missing 'endmodule'"
echo "  transpiled $(echo "$SV" | wc -l | tr -d ' ') lines of SystemVerilog — OK"

# ── 3. every test target ─────────────────────────────────────────────────────
if [ "$RUN_TEST" -eq 1 ]; then
  step "3/4  cargo test --workspace"
  # The verdict ledger G-G reads (tests/common/relation.rs): one file per case.
  LEDGER="$TMP/ledger"
  rm -rf "$LEDGER"; mkdir -p "$LEDGER"
  # G-I's coverage line (tests/projection_relation.rs writes it here; a passing
  # test's stdout is captured).
  PROJ_REPORT="$TMP/projection.txt"
  rm -f "$PROJ_REPORT"
  # G-J's coverage line (tests/const_evaluators_agree.rs writes it on a pass).
  CONST_REPORT="$TMP/const_eval.txt"
  rm -f "$CONST_REPORT"
  REFLEG_REPORT="$TMP/ref_leg.txt"
  rm -f "$REFLEG_REPORT"
  # G-K's report (tests/conformance_ledger.rs writes it on a pass).
  LEDGER_REPORT="$TMP/conformance_ledger.txt"
  rm -f "$LEDGER_REPORT"
  COPPER_VERDICT_LEDGER="$LEDGER" COPPER_PROJECTION_REPORT="$PROJ_REPORT" COPPER_CONST_EVAL_REPORT="$CONST_REPORT" \
    COPPER_REF_LEG_REPORT="$REFLEG_REPORT" COPPER_LEDGER_REPORT="$LEDGER_REPORT" \
    cargo test --workspace > "$TMP/test.log" 2>&1
  TEST_RC=$?

  grep -E "^test result:" "$TMP/test.log" | awk '
    {for (i = 1; i <= NF; i++) {
       if ($i == "passed;")  p += $(i-1)
       if ($i == "failed;")  f += $(i-1)
       if ($i == "ignored;") g += $(i-1) }}
    END { printf "  %d passed, %d failed, %d ignored across %d test binaries\n", p, f, g, NR }'

  if [ "$TEST_RC" -ne 0 ]; then
    KEEP_LOG=1
    grep -E "^(test .* FAILED|error(\[|:))" "$TMP/test.log" | head -20
    fail "cargo test --workspace"
  fi

  # G-C — no orphaned test file. A tests/*.rs that never produced a binary is a
  # check sitting on disk doing nothing.
  MISSING=""
  for f in tests/*.rs */tests/*.rs; do
    [ -e "$f" ] || continue
    stem="$(basename "$f" .rs)"
    grep -q "Running tests/${stem}\.rs" "$TMP/test.log" || MISSING="$MISSING $f"
  done
  [ -z "$MISSING" ] || { echo "  never ran:$MISSING" >&2; fail "G-C: orphaned test file(s)"; }
  echo "  G-C ok: all $(grep -c 'Running tests/' "$TMP/test.log") test files ran"

  # G-D — the corpus differential sweep actually covered the corpus. Every
  # `#[hardware]` module in tests/fixtures/ and examples/ must have a generated case
  # that ran, or an `#[ignore]` with a reason; `every_corpus_module_has_a_generated_case`
  # asserts that against a fresh scan. The sweep is the only check that says the
  # SystemVerilog a module transpiles to agrees with the simulator, so it silently
  # covering nothing is the expensive failure — the same reason G-A/G-B/G-C exist.
  grep -q "^test every_corpus_module_has_a_generated_case \.\.\. ok" "$TMP/test.log" \
    || fail "G-D: the corpus sweep's coverage guard did not run"
  # ...and under the stimulus policy the design doc states (several sequences per
  # module, every initial vector for a narrow one — CORPUS_DIFFERENTIAL_SWEEP.md
  # §3.2). The policy is pinned by one test; a sweep that ran without it ran under
  # some other policy, so its `ok` line is required too.
  grep -q "^test the_stimulus_policy_is_what_the_design_doc_says \.\.\. ok" "$TMP/test.log" \
    || fail "G-D: the corpus sweep's stimulus-policy pin did not run or did not pass"
  SWEPT=$(grep -cE "^test (fx|ex)_[a-z0-9_]+::[a-z0-9_]+_differential \.\.\. ok" "$TMP/test.log")
  SWEPT_SKIP=$(grep -cE "^test (fx|ex)_[a-z0-9_]+::[a-z0-9_]+_differential \.\.\. ignored" "$TMP/test.log")
  [ "$SWEPT" -gt 0 ] || fail "G-D: no differential cases ran"
  echo "  G-D ok: corpus sweep ran $SWEPT differential cases ($SWEPT_SKIP ignored-with-reason), each under the pinned multi-sequence stimulus policy"
  POLICY_LINE=$(grep -h "stimulus policy:" "$TMP"/*.log 2>/dev/null | tail -1 | sed 's/.*stimulus policy: //')
  [ -n "$POLICY_LINE" ] && echo "       policy: $POLICY_LINE"

  # G-I — the PROJECTION (SPEC III.1 P1, P15). A static test, no Verilator: for every
  # clocked module the sweep covers it builds the relation R between the emitted
  # machine's control state (`pc`, or the phase) and the coroutine's continuations,
  # checks every transition of it path by path, and checks the initial state encodes
  # the entry. It fails itself on a module it cannot relate without a listed reason,
  # and on a sweep module it never examined; here it must have RUN, passed, and
  # related a non-zero number of modules.
  grep -q "^test the_projection_relation_holds_on_the_corpus \.\.\. ok" "$TMP/test.log" \
    || fail "G-I: the projection test (tests/projection_relation.rs) did not run or did not pass"
  [ -s "$PROJ_REPORT" ] || fail "G-I: the projection test wrote no coverage report"
  PROJ_LINE=$(grep -h "^projection relation:" "$PROJ_REPORT" | tail -1 | sed 's/^projection relation: //')
  [ -n "$PROJ_LINE" ] || fail "G-I: the projection report has no coverage line"
  PROJ_N=$(echo "$PROJ_LINE" | sed -E 's/^([0-9]+) clocked.*/\1/')
  [ "${PROJ_N:-0}" -gt 0 ] 2>/dev/null || fail "G-I: the projection relation covered zero modules"
  echo "  G-I ok: projection relation — $PROJ_LINE"

  # G-J — the two CONSTANT EVALUATORS (D-21 B2, SPEC I.14.4). The interpreter's
  # evaluator stays independent of the front-ends' so the reference leg can attribute
  # a disagreement; this test pins the two against each other on every corpus
  # constant. It fails itself on a disagreement, an unpinned asymmetry, or a class
  # with nothing compared; here it must have RUN, passed and written its counts.
  grep -q "^test the_two_constant_evaluators_agree_on_every_corpus_constant \.\.\. ok" "$TMP/test.log" \
    || fail "G-J: the constant-evaluator test (tests/const_evaluators_agree.rs) did not run or did not pass"
  [ -s "$CONST_REPORT" ] || fail "G-J: the constant-evaluator test wrote no report"
  CONST_LINE=$(grep -h "^constant evaluators:" "$CONST_REPORT" | tail -1 | sed 's/^constant evaluators: //')
  [ -n "$CONST_LINE" ] || fail "G-J: the constant-evaluator report has no coverage line"
  echo "$CONST_LINE" | grep -q " 0 DISAGREE$" || fail "G-J: the constant evaluators disagree: $CONST_LINE"
  echo "  G-J ok: constant evaluators — $CONST_LINE"

  # G-K — the CONFORMANCE LEDGER (SPEC §III.3). The test checks statically that every
  # clause has a line and every CLOSED line's fixture exists and is swept; what it
  # cannot see is that the fixture ran and passed in THIS run, so it writes one
  # REQUIRE line per fixture and each is found here, in the run's own log. A test is
  # matched inside the section of its own binary (`Running tests/<stem>.rs`), so a
  # same-named test in another file cannot stand in for it. Examples are checked
  # after step 4, against their PASS status.
  grep -q "^test the_conformance_ledger_names_live_fixtures \.\.\. ok" "$TMP/test.log" \
    || fail "G-K: the conformance-ledger test (tests/conformance_ledger.rs) did not run or did not pass"
  [ -s "$LEDGER_REPORT" ] || fail "G-K: the conformance-ledger test wrote no report"
  LEDGER_LINE=$(grep -h "^conformance ledger:" "$LEDGER_REPORT" | tail -1 | sed 's/^conformance ledger: //')
  [ -n "$LEDGER_LINE" ] || fail "G-K: the conformance-ledger report has no summary line"
  # One pass over the log: "<binary stem> <test name>" for every `ok` test.
  awk '/^ *Running /{ f=$2; sub(/.*\//, "", f); sub(/\.rs$/, "", f) }
       /^test .* \.\.\. ok$/ { n=$2; sub(/.*::/, "", n); print f " " n; print f " " $2 }' \
    "$TMP/test.log" | sort -u > "$TMP/ok_tests.txt"
  GK_MISSING=""; GK_N=0; GK_EXAMPLES=""
  while read -r _ kind a b; do
    case "$kind" in
      sweep)
        GK_N=$((GK_N + 1))
        grep -qx "corpus_generated ${a}_differential" "$TMP/ok_tests.txt" \
          || GK_MISSING="$GK_MISSING ${a}_differential"
        grep -qx "corpus_generated ${a}_reference_leg" "$TMP/ok_tests.txt" \
          || GK_MISSING="$GK_MISSING ${a}_reference_leg" ;;
      test)
        GK_N=$((GK_N + 1))
        stem=$(basename "$a" .rs)
        grep -qx "$stem $b" "$TMP/ok_tests.txt" || GK_MISSING="$GK_MISSING $a::$b" ;;
      example)
        GK_EXAMPLES="$GK_EXAMPLES $a" ;;
      *) fail "G-K: unknown REQUIRE kind '$kind' in the ledger report" ;;
    esac
  done < <(grep '^REQUIRE ' "$LEDGER_REPORT")
  [ -z "$GK_MISSING" ] || { echo "  did not pass in this run:$GK_MISSING" >&2; fail "G-K: a CLOSED ledger line's fixture did not run and pass"; }
  echo "  G-K ok: conformance ledger — $LEDGER_LINE"
  echo "       $GK_N swept/test fixture checks passed in this run; examples checked after step 4:${GK_EXAMPLES:- none}"

  # G-F — the REFERENCE-INTERPRETER leg. copper-ref runs the kernel under SPEC
  # §I.0's one-resumption protocol with no rustc lowering and no executor, so it is
  # the one leg that can say the simulator is wrong (SPEC_OVERVIEW.md §7: sim ≠ ref
  # with SV = ref is a simulator/macro bug). build.rs generates one
  # `<name>_reference_leg` case per module — `#[ignore]`d with a sentence where a
  # REF_SKIP row names the module or the interpreter defers the class (a `Memory`
  # parameter, a second clock in one kernel, a structural parent — none swept today).
  # The leg needs no Verilator. Since Phase C step 9 it has four legs, and each must
  # have RUN: the clocked kernel (I.0), the memory model (I.10), the combinational
  # evaluation (I.11) — counted by build.rs and written by
  # corpus_generated.rs::the_reference_leg_covers_every_leg — and the design leg
  # (I.13), tests/two_domain_hierarchy_cdc.rs::reference_interpreter_design_matches_sim.
  # The `ok` count must EQUAL the generated run count: a case that should run and did
  # not is the bug class every other guard exists for.
  REF_RAN=$(grep -cE "^test (fx|ex)_[a-z0-9_]+::[a-z0-9_]+_reference_leg \.\.\. ok" "$TMP/test.log")
  REF_SKIPPED=$(grep -cE "^test (fx|ex)_[a-z0-9_]+::[a-z0-9_]+_reference_leg \.\.\. ignored" "$TMP/test.log")
  [ "$REF_RAN" -gt 0 ] || fail "G-F: the reference-interpreter leg ran on zero modules"
  grep -q "^test the_reference_leg_covers_every_leg \.\.\. ok" "$TMP/test.log" \
    || fail "G-F: the reference-leg coverage test did not run or did not pass (a leg is empty)"
  [ -s "$REFLEG_REPORT" ] || fail "G-F: the reference-leg coverage test wrote no report"
  REF_LINE=$(grep -h "^reference leg: run=" "$REFLEG_REPORT" | tail -1 | sed 's/^reference leg: //')
  REF_WANT=$(echo "$REF_LINE" | sed -E 's/^run=([0-9]+) .*/\1/')
  [ "$REF_RAN" -eq "${REF_WANT:-0}" ] 2>/dev/null \
    || fail "G-F: $REF_RAN reference-leg case(s) passed, build.rs generated ${REF_WANT:-?} to run"
  grep -q "^test reference_interpreter_design_matches_sim \.\.\. ok" "$TMP/test.log" \
    || fail "G-F: the design leg (tests/two_domain_hierarchy_cdc.rs::reference_interpreter_design_matches_sim) did not run or did not pass"
  echo "  G-F ok: reference-interpreter leg ran on $REF_RAN module(s) ($REF_SKIPPED skipped with a reason)"
  echo "       coverage: $REF_LINE; design leg: two_domain_top (I.13) ran"

  # G-G — the AGREEMENT RELATION (SPEC §III.2). A differential case no longer has
  # two outcomes: a sequence can PASS, leave the relation OUTSIDE (an X observation
  # against the 2-state SV, III.2.2 — never compared as 1), be TRUNCATED at a
  # run-time failure both the simulator and the reference fail at (III.2.4), be
  # EXCUSED by a signature class (none today: the observation-0 plain-`Out` class
  # emptied with the D-8 rewrites and was deleted at the flip, III.2.1), or
  # DISAGREE by a pattern (III.2.8). Each verdict that is not a PASS is exactly the
  # kind of thing that hides if nobody prints it, so every count is printed, zeros
  # included, and the ledger must account for every case that ran.
  gg_relation "$LEDGER" "$SWEPT" "$REF_RAN" 1

  # G-E — ANCHORING. The sweep checks the simulator against the SystemVerilog the
  # transpiler emitted: two implementations of one source, which proves CONSISTENCY.
  # A module additionally checked against an independent hand-written Verilog
  # reference has its SEMANTICS anchored to something outside Copper, and a
  # misconception shared between the executor and the lowering is only visible there.
  # Not a hard failure — most of the corpus is unanchored today and blocking on that
  # would help nobody. It is printed every run for the reason the ignored-test list
  # is: a gap nobody prints is a gap somebody rediscovers in an audit.
  ANCHOR_LINE=$(grep -h "anchoring:" "$TMP"/*.log 2>/dev/null | tail -1 | sed 's/.*anchoring: //')
  if [ -n "$ANCHOR_LINE" ]; then
    echo "  G-E: $ANCHOR_LINE"
    UNANCH=$(grep -h "unanchored:" "$TMP"/*.log 2>/dev/null | tail -1 | sed 's/.*unanchored: //')
    [ -n "$UNANCH" ] && echo "       unanchored: $UNANCH"
  else
    echo "  G-E: anchoring ledger not found in the build log (build.rs emits it)"
  fi

  # Ignored tests stay VISIBLE. These are deliberate, but an ignore whose stated
  # reason has gone stale is invisible otherwise — exactly what happened to accum_2,
  # which sat disabled long after the divergence it described had been fixed.
  NIGN=$(grep -c "\.\.\. ignored" "$TMP/test.log")
  if [ "$NIGN" -gt 0 ]; then
    echo "  $NIGN ignored test(s) — deliberately not run, re-read these periodically:"
    grep "\.\.\. ignored" "$TMP/test.log" | sed 's/^test /    /' | cut -c1-140
  fi
else
  echo; echo "  (cargo test skipped)"
fi

# ── 3q. --quick: the reference-interpreter leg alone (no Verilator needed) ─────
# The leg is cheap (the simulator plus an interpreter, no Verilator build), so the
# fast loop runs it even though it skips `cargo test`: a spec disagreement shows up
# in the inner loop, not only in the full regression.
if [ "$QUICK" -eq 1 ]; then
  step "3q   reference-interpreter leg (copper-ref vs simulator, no Verilator)"
  # Not `-q`: quiet mode drops the per-test lines the count below reads.
  LEDGER="$TMP/ledger"
  rm -rf "$LEDGER"; mkdir -p "$LEDGER"
  REFLEG_REPORT="$TMP/ref_leg.txt"
  rm -f "$REFLEG_REPORT"
  COPPER_VERDICT_LEDGER="$LEDGER" COPPER_REF_LEG_REPORT="$REFLEG_REPORT" \
    cargo test --test corpus_generated -- reference_leg > "$TMP/refleg.log" 2>&1
  REF_RC=$?
  REF_RAN=$(grep -cE "^test (fx|ex)_[a-z0-9_]+::[a-z0-9_]+_reference_leg \.\.\. ok" "$TMP/refleg.log")
  REF_SKIPPED=$(grep -cE "^test (fx|ex)_[a-z0-9_]+::[a-z0-9_]+_reference_leg \.\.\. ignored" "$TMP/refleg.log")
  if [ "$REF_RC" -ne 0 ]; then
    grep -E "^(test .* FAILED|error(\[|:)|reference leg: FAIL)" "$TMP/refleg.log" | head -20
    fail "reference-interpreter leg"
  fi
  [ "$REF_RAN" -gt 0 ] || fail "G-F: the reference-interpreter leg ran on zero modules"
  REF_WANT=$(grep -h "^reference leg: run=" "$REFLEG_REPORT" 2>/dev/null | sed -E 's/^reference leg: run=([0-9]+) .*/\1/')
  [ "$REF_RAN" -eq "${REF_WANT:-0}" ] 2>/dev/null \
    || fail "G-F: $REF_RAN reference-leg case(s) passed, build.rs generated ${REF_WANT:-?} to run"
  echo "  G-F ok: reference-interpreter leg ran on $REF_RAN module(s) ($REF_SKIPPED skipped with a reason)"
  gg_relation "$LEDGER" 0 "$REF_RAN" 0
fi

# ── 4. examples: sim vs Verilator ────────────────────────────────────────────
if [ "$NO_EXAMPLES" -eq 1 ]; then
  echo; echo "  (examples skipped)"
else
  step "4/4  examples (Copper sim vs Verilator equivalence)"

  if [ "${#CUSTOM_EXAMPLES[@]}" -gt 0 ]; then
    EXAMPLES=("${CUSTOM_EXAMPLES[@]}")
  elif [ "$QUICK" -eq 1 ]; then
    EXAMPLES=("${REP_EXAMPLES[@]}")
    echo "  (--quick: representative subset only)"
  else
    EXAMPLES=()
    while IFS= read -r line; do EXAMPLES+=("$line"); done < <(registered_examples)

    # G-A — an examples/**.rs with no [[example]] entry can never be run by anything.
    # `old/` directories are exempt — the convention for set-aside scratch that
    # must not count as an example (the deleted examples/cpu/old/ pre-subset
    # copies were the original case); build.rs's corpus sweep prunes the same
    # way, so the two scans stay consistent.
    UNREG=""
    for f in $(find examples -path '*/old/*' -prune -o -name '*.rs' -print | sort); do
      grep -q "path = \"$f\"" Cargo.toml || UNREG="$UNREG $f"
    done
    [ -z "$UNREG" ] || { echo "  unregistered:$UNREG" >&2; fail "G-A: example(s) that can never run"; }
    echo "  G-A ok: all $(find examples -path '*/old/*' -prune -o -name '*.rs' -print | wc -l | tr -d ' ') example files are registered"
  fi

  command -v verilator >/dev/null || fail "verilator not on PATH (brew install verilator)"

  # Build every example once, then run the binaries in parallel. A `cargo run` per
  # example serialises on cargo's lock and pays cargo's start-up each time, and the
  # examples are independent of each other: each writes its own
  # `waveforms/<name>.vcd` and its own per-invocation Verilator work directory, and
  # none reads the environment at run time. The results are printed in registration
  # order afterwards, so the output reads the same as the sequential loop did.
  cargo build -q --examples || fail "building the examples"
  EXDIR="${CARGO_TARGET_DIR:-target}/debug/examples"
  JOBS="${COPPER_REGRESSION_JOBS:-$(sysctl -n hw.ncpu 2>/dev/null || nproc 2>/dev/null || echo 4)}"
  mkdir -p "$TMP/ex"
  printf '%s\n' "${EXAMPLES[@]}" | xargs -P "$JOBS" -I{} sh -c \
    'if "$3/$1" >"$2/ex/$1.log" 2>&1; then echo PASS >"$2/ex/$1.status"; else echo FAIL >"$2/ex/$1.status"; fi' \
    _ {} "$TMP" "$EXDIR"
  PASS=0; TOTAL=0
  for ex in "${EXAMPLES[@]}"; do
    TOTAL=$((TOTAL + 1))
    printf '  %-22s ' "$ex"
    if [ "$(cat "$TMP/ex/$ex.status" 2>/dev/null)" = PASS ]; then
      echo "PASS"; PASS=$((PASS + 1))
    else
      echo "FAIL"
      sed 's/^/      /' "$TMP/ex/$ex.log" 2>/dev/null | tail -8
      echo "      (rerun: cargo run --example $ex)"
    fi
  done
  echo "  examples: $PASS/$TOTAL passed"
  [ "$PASS" -eq "$TOTAL" ] || fail "$((TOTAL - PASS)) example(s) failed"

  # G-B — every registered example actually ran.
  if [ "$QUICK" -eq 0 ] && [ "${#CUSTOM_EXAMPLES[@]}" -eq 0 ]; then
    NREG=$(registered_examples | wc -l | tr -d ' ')
    [ "$TOTAL" -eq "$NREG" ] || fail "G-B: $NREG registered but only $TOTAL ran"
    echo "  G-B ok: all $NREG registered examples ran"
    # G-K's example fixtures: each CLOSED line's example passed in this run.
    if [ "$RUN_TEST" -eq 1 ]; then
      for ex in ${GK_EXAMPLES:-}; do
        [ "$(cat "$TMP/ex/$ex.status" 2>/dev/null)" = PASS ] \
          || fail "G-K: the ledger's example fixture '$ex' did not pass in this run"
      done
      echo "  G-K ok: the ledger's example fixtures passed:${GK_EXAMPLES:- none}"
    fi
  fi
fi

echo
if [ "$PARTIAL" -eq 1 ]; then
  echo "PARTIAL OK ($(elapsed)) — a subset only; run tools/regression.sh bare for the full regression"
else
  echo "REGRESSION OK ($(elapsed))"
fi

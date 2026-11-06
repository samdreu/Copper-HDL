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
#        design_docs/SPEC_OVERVIEW.md §7) ran on every module it covers, and the
#        modules it skips are skipped with a sentence (build.rs REF_SKIP)
#   G-G  the AGREEMENT RELATION's verdicts (SPEC §III.2): every swept case wrote its
#        per-sequence verdicts to the ledger, the counts of PASS, OUTSIDE (an X vs the
#        SV), TRUNCATED (a run-time failure), EXCUSED (observation 0 of a plain `Out`)
#        and each disagreement pattern are printed, no sweep disagreement survives,
#        and every REF_SKIP row excusing a measured disagreement is matched by a pin
#        that measured the same pattern this run
#   G-H  the ONE-RESUMPTION executor (Phase C step 2, `COPPER_RESUMPTION=one`, not yet
#        the default): the sweep is run a second time under it and its verdict counts
#        are printed, so the flip step 3 owes is measured on every run; it fails only
#        where the new executor breaks sim ≡ ref (a reference-leg disagreement, or a
#        differential pattern with sim ≠ ref), never on the sim ≠ SV of the two-job
#        spellings step 3 rewrites
#
# and it prints the `#[ignore]`d tests every run, because a skipped check that prints
# nothing is indistinguishable from a passing one.
set -uo pipefail

# A stale VERILATOR_ROOT makes `verilator` refuse to run. The Rust harness clears it
# internally (see copper-sim `verilator_status`), but this script also probes the
# binary directly, so clear it here too.
unset VERILATOR_ROOT
# The run is a verdict on the compiled default executor; G-H runs the other mode itself.
unset COPPER_RESUMPTION

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
  COPPER_VERDICT_LEDGER="$LEDGER" cargo test --workspace > "$TMP/test.log" 2>&1
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

  # G-F — the REFERENCE-INTERPRETER leg. copper-ref runs the kernel under SPEC
  # §I.0's one-resumption protocol with no rustc lowering and no executor, so it is
  # the one leg that can say the simulator is wrong (SPEC_OVERVIEW.md §7: sim ≠ ref
  # with SV = ref is a simulator/macro bug). build.rs generates one
  # `<name>_reference_leg` case per module — `#[ignore]`d with a sentence where the
  # interpreter defers the class (plain `Out` awaiting Phase C, memories, a second
  # clock, combinational, structural) or a REF_SKIP row names the module. The leg
  # needs no Verilator. Zero cases run means the leg silently stopped covering the
  # corpus, which is the bug class every other guard exists for.
  REF_RAN=$(grep -cE "^test (fx|ex)_[a-z0-9_]+::[a-z0-9_]+_reference_leg \.\.\. ok" "$TMP/test.log")
  REF_SKIPPED=$(grep -cE "^test (fx|ex)_[a-z0-9_]+::[a-z0-9_]+_reference_leg \.\.\. ignored" "$TMP/test.log")
  [ "$REF_RAN" -gt 0 ] || fail "G-F: the reference-interpreter leg ran on zero modules"
  echo "  G-F ok: reference-interpreter leg ran on $REF_RAN module(s) ($REF_SKIPPED skipped with a reason)"
  REF_LINE=$(grep -h "reference leg:" "$TMP"/*.log 2>/dev/null | grep "REF_SKIP" | tail -1 | sed 's/.*reference leg: //')
  [ -n "$REF_LINE" ] && echo "       coverage: $REF_LINE"

  # G-G — the AGREEMENT RELATION (SPEC §III.2). A differential case no longer has
  # two outcomes: a sequence can PASS, leave the relation OUTSIDE (an X observation
  # against the 2-state SV, III.2.2 — never compared as 1), be TRUNCATED at a
  # run-time failure both the simulator and the reference fail at (III.2.4), be
  # EXCUSED by a signature class (a plain `Out` at observation 0, III.2.1), or
  # DISAGREE by a pattern (III.2.8). Each verdict that is not a PASS is exactly the
  # kind of thing that hides if nobody prints it, so every count is printed, zeros
  # included, and the ledger must account for every case that ran.
  gg_relation "$LEDGER" "$SWEPT" "$REF_RAN" 1

  # G-H — the one-resumption executor (SPEC §I.0.2, D-8; Phase C step 2), behind
  # `COPPER_RESUMPTION=one` until step 3 rewrites the two-job spellings and flips the
  # default. The sweep runs again under it and every count is printed. Today 66 cases
  # change (the plain-`Out` modules and P12, MEASUREMENTS §F.1), and they are counted,
  # not failed. A disagreement with the reference interpreter does fail, because the
  # new executor exists to BE the coroutine the interpreter runs (P12).
  LEDGER_ONE="$TMP/ledger_one"
  rm -rf "$LEDGER_ONE"; mkdir -p "$LEDGER_ONE"
  COPPER_RESUMPTION=one COPPER_VERDICT_LEDGER="$LEDGER_ONE" \
    cargo test --test corpus_generated > "$TMP/sweep_one.log" 2>&1 || true
  ALL_ONE="$(cat "$LEDGER_ONE"/*.tsv 2>/dev/null)"
  [ -n "$ALL_ONE" ] || fail "G-H: the one-resumption sweep wrote no verdicts"
  D_ONE=$(printf '%s\n' "$ALL_ONE" | awk -F'\t' '$1=="sweep-differential" && $2=="CASE"' | wc -l | tr -d ' ')
  R_ONE=$(printf '%s\n' "$ALL_ONE" | awk -F'\t' '$1=="sweep-reference" && $2=="CASE"' | wc -l | tr -d ' ')
  [ "$D_ONE" -eq "$SWEPT" ] || fail "G-H: $SWEPT differential case(s) under the default, $D_ONE under one resumption"
  [ "$R_ONE" -eq "$REF_RAN" ] || fail "G-H: $REF_RAN reference-leg case(s) under the default, $R_ONE under one resumption"
  for origin in sweep-differential sweep-reference; do
    line=""
    for v in PASS OUTSIDE TRUNCATED EXCUSED DISAGREE; do
      n=$(printf '%s\n' "$ALL_ONE" | awk -F'\t' -v o="$origin" -v v="$v" '$1==o && $2==v' | wc -l | tr -d ' ')
      line="$line $n $v,"
    done
    echo "  G-H ${origin#sweep-} (one resumption): sequences —${line%,}"
  done
  CHANGED=$(printf '%s\n' "$ALL_ONE" | awk -F'\t' '$1=="sweep-differential" && $2=="DISAGREE"{print $3}' | sort -u | wc -l | tr -d ' ')
  PATS_ONE=$(printf '%s\n' "$ALL_ONE" | awk -F'\t' '$1=="sweep-differential" && $2=="DISAGREE"' | grep -o 'pattern=[^ ]*' | sed 's/pattern=//' | sort | uniq -c | awk '{printf "%s%s %s", (NR>1?", ":""), $1, $2}')
  echo "       cases that disagree under one resumption: $CHANGED of $D_ONE (the flip step 3 owes); by pattern: ${PATS_ONE:-none}"
  BAD_ONE=$(printf '%s\n' "$ALL_ONE" | awk -F'\t' '($1=="sweep-reference" && $2=="DISAGREE") || ($1=="sweep-differential" && $2=="DISAGREE" && $0 ~ /pattern=(sim≠ref|sim=SV≠ref|all-differ)/)' | wc -l | tr -d ' ')
  if [ "$BAD_ONE" -gt 0 ]; then
    printf '%s\n' "$ALL_ONE" | awk -F'\t' '($1=="sweep-reference" && $2=="DISAGREE") || ($1=="sweep-differential" && $2=="DISAGREE" && $0 ~ /pattern=(sim≠ref|sim=SV≠ref|all-differ)/)' | cut -c1-200 | head -10
    KEEP_LOG=1
    fail "G-H: $BAD_ONE sequence(s) where the one-resumption simulator disagrees with the reference interpreter"
  fi
  echo "  G-H ok: under one resumption the simulator agrees with the reference interpreter on every sequence it runs"

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
  COPPER_VERDICT_LEDGER="$LEDGER" cargo test --test corpus_generated -- reference_leg > "$TMP/refleg.log" 2>&1
  REF_RC=$?
  REF_RAN=$(grep -cE "^test (fx|ex)_[a-z0-9_]+::[a-z0-9_]+_reference_leg \.\.\. ok" "$TMP/refleg.log")
  REF_SKIPPED=$(grep -cE "^test (fx|ex)_[a-z0-9_]+::[a-z0-9_]+_reference_leg \.\.\. ignored" "$TMP/refleg.log")
  if [ "$REF_RC" -ne 0 ]; then
    grep -E "^(test .* FAILED|error(\[|:)|reference leg: FAIL)" "$TMP/refleg.log" | head -20
    fail "reference-interpreter leg"
  fi
  [ "$REF_RAN" -gt 0 ] || fail "G-F: the reference-interpreter leg ran on zero modules"
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
  fi
fi

echo
if [ "$PARTIAL" -eq 1 ]; then
  echo "PARTIAL OK ($(elapsed)) — a subset only; run tools/regression.sh bare for the full regression"
else
  echo "REGRESSION OK ($(elapsed))"
fi

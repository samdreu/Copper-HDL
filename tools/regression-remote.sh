#!/usr/bin/env bash
# Run the full regression on a remote Linux machine and bring the log back, so
# the number it prints is a REGRESSION number and not a laptop number.
#
#   tools/regression-remote.sh [--setup] [--ref REV | --sync] [--host H] [--dir D]
#                              [--max-load X] [--force] [-- <regression.sh args>]
#
# Why remote: on macOS every freshly built executable is assessed by Gatekeeper
# at launch (an online notarization check), a full run launches a few hundred
# of them, and a laptop sleeps. Measured 2026-10-21: the same tree, same warm
# model cache, took 304 s or 2598 s for the test phase depending on nothing in
# the tree. A 96-core Linux box has none of that and parallelises the Verilator
# builds the cold part of a run is made of.
#
# Modes:
#   --ref REV    (default, REV defaults to HEAD) The remote clone fetches origin
#                and checks out that exact commit, detached, with a clean tree.
#                The commit must already be on origin — this script never pushes.
#                This is the mode for a number you will quote: the log header
#                records the commit and the tree is exactly that commit.
#   --sync       rsync this working tree (uncommitted changes included) onto the
#                remote clone instead. For "does my change pass", not for numbers;
#                the header says SYNCED WORKING TREE and records `git describe
#                --dirty` from here. Changed files are written with the remote's
#                current mtime (see the rsync flags below) so cargo rebuilds them;
#                a preserved laptop mtime older than the last remote build would
#                make cargo keep the stale artifact.
#   --setup      One-time: create the scratch target directory, point the clone's
#                `target/` at it with a symlink (the harness writes
#                `target/verilator/…` relative to the repo, so CARGO_TARGET_DIR
#                alone would leave the models on NFS), check every tool, fetch.
#
# Everything after `--` is passed to tools/regression.sh (e.g. `-- --quick`), so
# a subset still prints PARTIAL OK and is still not a regression run.
#
# The remote side: `$DIR` is the clone (default ~/Copper-HDL), tools come from
# ~/oss-cad-suite/bin and ~/.cargo/bin, and the run refuses to start when the
# 1-minute load average is above --max-load (default: a sixth of the cores) —
# a number from a loaded box is the thing this script exists to avoid.
# Logs land in `target/remote-regression/<stamp>-<commit>.log` on both sides.
set -uo pipefail
cd "$(git rev-parse --show-toplevel 2>/dev/null || echo .)" || exit 2

HOST=recycle; DIR='~/Copper-HDL'; SCRATCH=/scratch/$USER/copper-target
MODE=ref; REV=HEAD; SETUP=0; MAX_LOAD=""; FORCE=0; EXTRA=()
while [ $# -gt 0 ]; do
  case "$1" in
    --host) HOST=$2; shift 2;;
    --dir) DIR=$2; shift 2;;
    --scratch) SCRATCH=$2; shift 2;;
    --ref) MODE=ref; REV=$2; shift 2;;
    --sync) MODE=sync; shift;;
    --setup) SETUP=1; shift;;
    --max-load) MAX_LOAD=$2; shift 2;;
    --force) FORCE=1; shift;;
    --) shift; EXTRA=("$@"); break;;
    *) echo "unknown argument: $1 (see the header of $0)" >&2; exit 2;;
  esac
done

STAMP=$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p target/remote-regression

# ── What the remote runs. Quoted heredoc: nothing here is expanded locally. ──
read -r -d '' REMOTE <<'EOF'
set -uo pipefail
# DIR SCRATCH MODE REV MAX_LOAD FORCE STAMP SETUP LOCAL_DESC and the EXTRA array
# arrive as shell assignments prepended to this script (ssh flattens its argument
# list into one string, so empty or space-containing arguments would not survive
# the trip as positional parameters).
export PATH="$HOME/oss-cad-suite/bin:$HOME/.cargo/bin:$PATH"
unset VERILATOR_ROOT
eval "DIR=$DIR"   # expand a leading ~ sent as text

if [ "$SETUP" -eq 1 ]; then
  echo "== setup on $(hostname)"
  [ -d "$DIR/.git" ] || { echo "no clone at $DIR — clone the repository there first" >&2; exit 2; }
  mkdir -p "$SCRATCH"
  cd "$DIR"
  if [ -L target ]; then
    echo "  target -> $(readlink target) (already a symlink)"
  else
    # Never `rm -rf` on the NFS home: a still-open file leaves a placeholder and the
    # removal fails half way. Rename the old directory aside instead.
    [ -e target ] && mv target "target.nfs-$STAMP" && echo "  moved the NFS target/ aside as target.nfs-$STAMP (delete it when convenient)"
    ln -s "$SCRATCH" target && echo "  target -> $SCRATCH"
  fi
  for t in cargo rustc verilator iverilog yosys git rsync python3; do
    printf '  %-10s %s\n' "$t" "$(command -v "$t" || echo MISSING)"
  done
  command -v ccache >/dev/null || echo "  ccache     absent (optional; only cold Verilator builds are slower)"
  git fetch -q origin && echo "  fetched origin"
  exit 0
fi

cd "$DIR" || exit 2
[ -L target ] || echo "WARNING: $DIR/target is not a symlink onto local disk — run with --setup first; this run will build on NFS" >&2

NCORES=$(nproc)
[ -n "$MAX_LOAD" ] || MAX_LOAD=$(( NCORES / 6 ))
LOAD1=$(cut -d' ' -f1 /proc/loadavg)
if [ "$FORCE" -eq 0 ] && [ "$(echo "$LOAD1 > $MAX_LOAD" | bc)" = 1 ]; then
  echo "refusing: 1-minute load average is $LOAD1 on $NCORES cores (limit $MAX_LOAD); --force overrides" >&2
  exit 3
fi

case "$MODE" in
  ref)
    git fetch -q origin || { echo "git fetch failed" >&2; exit 2; }
    SHA=$(git rev-parse --verify --quiet "$REV^{commit}") || { echo "commit $REV is not on origin — push it first (this script never pushes)" >&2; exit 2; }
    git checkout -q --detach "$SHA" && git reset -q --hard && git clean -qfdx -e target -e 'target.nfs-*' || exit 2
    DESC="$(git rev-parse --short HEAD) (exact commit, clean tree)"
    ;;
  sync)
    DESC="SYNCED WORKING TREE from the laptop: $LOCAL_DESC"
    ;;
esac

LOG="target/remote-regression/$STAMP-$(git rev-parse --short HEAD).log"
mkdir -p target/remote-regression
{
  echo "host:      $(hostname) ($NCORES cores, $(grep -m1 'model name' /proc/cpuinfo | sed 's/.*: //'))"
  echo "os:        $(uname -sr)"
  echo "load:      $(cut -d' ' -f1-3 /proc/loadavg) at start (limit $MAX_LOAD)"
  echo "tree:      $DESC"
  echo "target:    $(readlink -f target)"
  echo "rustc:     $(rustc --version)"
  echo "verilator: $(verilator --version)"
  echo "models cached before: $(ls target/verilator/cache 2>/dev/null | wc -l)"
  echo "started:   $(date -u +%FT%TZ)"
  echo "args:      tools/regression.sh ${EXTRA[*]-}"
  echo
} | tee "$LOG"
SECONDS=0
tools/regression.sh ${EXTRA[@]+"${EXTRA[@]}"} 2>&1 | tee -a "$LOG"
RC=${PIPESTATUS[0]}
{
  echo
  echo "finished:  $(date -u +%FT%TZ)  wall ${SECONDS}s  exit $RC"
  echo "models cached after:  $(ls target/verilator/cache 2>/dev/null | wc -l)"
  echo "load:      $(cut -d' ' -f1-3 /proc/loadavg) at end"
} | tee -a "$LOG"
echo "REMOTE_LOG=$LOG"
exit "$RC"
EOF

LOCAL_DESC=$(git describe --always --dirty 2>/dev/null)

if [ "$MODE" = sync ]; then
  echo "== syncing the working tree to $HOST:$DIR"
  # --checksum --no-times, NOT -a's preserved mtimes: cargo decides "fresh" by
  # comparing a source file's mtime with the previous build's fingerprint, so a
  # file changed on the laptop BEFORE the last remote build but synced AFTER it
  # arrives with an mtime older than that fingerprint and is never recompiled —
  # measured 2026-10-28: a merged proc-macro source synced with its merge-time
  # mtime was treated as unchanged and the stale .so failed a trybuild case that
  # passed locally. With --checksum unchanged files are skipped by content, and
  # every file that does change is written with the remote's current time.
  rsync -rlpgoDz --checksum --no-times --delete \
    --exclude target --exclude 'target.nfs-*' --exclude .git --exclude .claude \
    --exclude paper --exclude waveforms --exclude '*.log' \
    ./ "$HOST:$DIR/" || { echo "rsync failed" >&2; exit 2; }
fi

if [ "$MODE" = ref ]; then
  SHA=$(git rev-parse --verify --quiet "$REV^{commit}") || { echo "unknown revision $REV" >&2; exit 2; }
  REV=$SHA
fi

# The parameters travel as shell assignments in front of the script, each
# quoted with printf %q, so empty values and spaces survive ssh's flattening.
PREAMBLE=$(printf 'DIR=%q; SCRATCH=%q; MODE=%q; REV=%q; MAX_LOAD=%q; FORCE=%q; STAMP=%q; SETUP=%q; LOCAL_DESC=%q\n' \
  "$DIR" "$SCRATCH" "$MODE" "$REV" "${MAX_LOAD:-}" "$FORCE" "$STAMP" "$SETUP" "$LOCAL_DESC")
PREAMBLE="$PREAMBLE"$'\n'"$(printf 'EXTRA=('; for a in ${EXTRA[@]+"${EXTRA[@]}"}; do printf '%q ' "$a"; done; printf ')')"
ssh -o BatchMode=yes "$HOST" bash -s <<<"$PREAMBLE"$'\n'"$REMOTE" \
  2>&1 | tee "target/remote-regression/$STAMP.stdout"
RC=${PIPESTATUS[0]}
[ "$SETUP" -eq 1 ] && exit "$RC"

REMOTE_LOG=$(grep -o 'REMOTE_LOG=.*' "target/remote-regression/$STAMP.stdout" | tail -1 | cut -d= -f2-)
if [ -n "$REMOTE_LOG" ]; then
  LOCAL_LOG="target/remote-regression/$(basename "$REMOTE_LOG")"
  scp -q "$HOST:$DIR/$REMOTE_LOG" "$LOCAL_LOG" 2>/dev/null && echo "log: $LOCAL_LOG"
fi
rm -f "target/remote-regression/$STAMP.stdout"
exit "$RC"

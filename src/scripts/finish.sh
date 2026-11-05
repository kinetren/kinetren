#!/usr/bin/env bash
# Wrap-up of work on the changes (rule, item 7 of round 12).
#
# `cargo test` builds a SEPARATE test binary and does NOT touch
# target/debug/kinetren — the one a person runs. Because of this
# the user twice ran an outdated program and considered the defects
# unfixed. So every agent finishes work with this script:
# it rebuilds what gets run, names the version and cleans up the litter.
#
# From R.68 the script is the ONLY check point: running the workshop tests
# became its stage (previously it was a separate agent command, and the budget with the
# verdict fell into two unrelated halves). Each stage
# names its cost, and at the end a table and the budget are printed — five minutes,
# set by the owner. An overrun is MARKED, but does not make the run
# red: the rule “tests from the specs are not weakened” outranks the budget.
#
#   scripts/finish.sh            # debug build + checks + version + cleanup (stale artifacts too)
#   scripts/finish.sh --release  # plus the release tarball in a container
#   scripts/finish.sh --deep     # plus cleanup of intermediate objects
#   scripts/finish.sh --rebuild-check  # plus t977: an edit of a plugin kernel rebuilds no core crate
set -euo pipefail
cd "$(dirname "$0")/.." # src/

RELEASE=0
DEEP=0
REBUILD=0
for a in "$@"; do
    case "$a" in
        --release) RELEASE=1 ;;
        --deep) DEEP=1 ;;
        --rebuild-check) REBUILD=1 ;;
        *) echo "unknown key: $a" >&2; exit 2 ;;
    esac
done

# One finish.sh at a time on the machine (the owner, 2026-11-01: "only one at a time, the others
# wait, whoever takes it first runs it"): three parallel runs of the full test set and the live
# scenarios made each of them several times slower and collided on the live display. The lock is
# machine-wide (one file for every worktree) and is held by a `flock -o` process around the whole
# run: `-o` keeps the lock out of the children, so a leaked Xvfb or kt-app cannot hold it after the
# run ends, and a killed run releases it together with its flock. The wait is not a stage and is
# not counted in the budget. KT_FINISH_NO_LOCK=1 skips it (a CI runner has one job anyway).
if [ -z "${KT_FINISH_NO_LOCK:-}${KT_FINISH_LOCKED:-}" ] && command -v flock > /dev/null; then
    FINISH_LOCK="${KT_FINISH_LOCK:-${XDG_RUNTIME_DIR:-/tmp}/kinetren-finish.lock}"
    if ! flock -n "$FINISH_LOCK" true; then
        echo "finish.sh: another run holds the machine lock — $(cat "$FINISH_LOCK.who" 2>/dev/null || echo unknown); waiting"
    fi
    exec env KT_FINISH_LOCKED="$FINISH_LOCK" KT_FINISH_WAIT_FROM="$(date +%s)" \
        flock -o "$FINISH_LOCK" "$(pwd)/scripts/finish.sh" "$@"
fi
if [ -n "${KT_FINISH_LOCKED:-}" ]; then
    echo "$(pwd) pid $$ since $(date '+%H:%M:%S')" > "$KT_FINISH_LOCKED.who"
    WAITED=$(($(date +%s) - ${KT_FINISH_WAIT_FROM:-$(date +%s)}))
    [ "$WAITED" -gt 1 ] && echo "finish.sh: machine lock taken after $WAITED s"
fi

VER=$(git log -1 --format=%cd --date=format:%y.%m.%d 2>/dev/null || echo 00.00.00)
HASH=$(git rev-parse --short=8 HEAD 2>/dev/null || echo nogit)
FULL="kt$VER-$HASH"
DIRTY=""
[ -n "$(git status --porcelain 2>/dev/null)" ] && DIRTY=" (there are uncommitted changes)"

# Log of one stage: the verdict is read from cargo’s exit code, and the short
# tail is printed for the person (R.58).
STAGE_LOG="$(mktemp -t kt-finish-stage.XXXXXX)"
trap 'rm -f "$STAGE_LOG"' EXIT

# Stage timing (R.68). The measurements file is “name<TAB>seconds<TAB>code”;
# the table and budget are computed by scripts/finish-budget.py. The STAGE HEADER
# IS PRINTED ONLY FROM HERE: otherwise a stage could be quietly moved out of the budget —
# the same class as “red under someone else’s name” (R.55) and “verdict from tail”
# (R.58). The guard `finish-budget.py --check-script` checks this.
mkdir -p target
TIMES="target/finish-times.txt"
: > "$TIMES"

stage() { # stage <name> <command...> — the command’s output goes to the person as is
    local name="$1"; shift
    printf '== %s ==\n' "$name"
    local s rc=0
    s=$(date +%s)
    "$@" || rc=$?
    printf '%s\t%s\t%s\n' "$name" "$(($(date +%s) - s))" "$rc" >> "$TIMES"
    return $rc
}

stage_log() { # stage_log <name> <command...> — output goes to a file, the verdict comes from the command
    local name="$1"; shift
    printf '== %s ==\n' "$name"
    local s rc=0
    s=$(date +%s)
    "$@" > "$STAGE_LOG" 2>&1 || rc=$?
    printf '%s\t%s\t%s\n' "$name" "$(($(date +%s) - s))" "$rc" >> "$TIMES"
    tail -4 "$STAGE_LOG"
    if [ "$rc" != 0 ]; then
        grep -E "^(test .* FAILED|failures:|error)" "$STAGE_LOG" | head -20 >&2
    fi
    return $rc
}

stage "build of what the user runs" cargo build -p kt-app -p kt-cli

if [ "$RELEASE" = 1 ]; then
    stage "release tarball" scripts/release-build.sh
fi

# t977 (plugin-api-stage1): an edit of one plugin kernel recompiles only the family, the assembler,
# kt-app and kt-cli (and test targets). Optional: it rebuilds the test binaries twice (~2 min).
if [ "$REBUILD" = 1 ]; then
    stage "rebuild check (t977)" python3 scripts/plugin-rebuild-check.py check || REBUILD_FAILED=1
fi

# Workshop test run — a stage of THIS script (R.68). The kt-gpu GPU conformance
# on the machine’s real adapter also lives here: a separate stage for the
# kt-gpu crate was a duplicate of this run and was dropped (its ten seconds went
# to what the workshop had already run). Without an adapter the kt-gpu tests skip
# themselves and print the reason.
#
# THERE IS NEXTEST — WE RUN WITH IT (measurement R.68): 216 s against 310 s for `cargo test`,
# and — the main thing — GREEN. `cargo test` runs 96 test binaries one by one,
# and inside a binary all tests at once: t663 waits for the background redraw of the reel and under
# such a crowd does not wait (60 s timeout). nextest distributes tests across
# processes, scheduling by cores. The workshop has no doctests (checked:
# `cargo test --doc` — zero), so nothing is lost. No nextest —
# we go the old way; it stays the reference.
if command -v cargo-nextest > /dev/null; then
    WS_CMD=(cargo nextest run --workspace --no-fail-fast)
else
    WS_CMD=(cargo test --workspace -q)
fi
if stage_log "workshop tests" "${WS_CMD[@]}"; then
    echo "workshop tests: ok"
else
    cp "$STAGE_LOG" target/finish-workspace-fail.log 2>/dev/null || true
    echo "WORKSHOP TESTS FAILED (full log: target/finish-workspace-fail.log)" >&2
    WS_FAILED=1
fi

# Live check in a virtual screen (R.27): real binary, real X,
# real XTest gestures. Without Xvfb it simply skips, breaking nothing;
# NO_LIVE=1 turns it off explicitly (for example when you run finish.sh in a loop).
if [ -z "${NO_LIVE:-}" ] && command -v Xvfb > /dev/null; then
    if stage "live scenarios" ./scripts/live-check.sh; then
        echo "live scenarios: ok"
    else
        echo "LIVE SCENARIOS FAILED (screenshots: target/live-out)" >&2
        LIVE_FAILED=1
    fi
elif [ -z "${NO_LIVE:-}" ]; then
    echo "live scenarios skipped: no Xvfb (sudo apt install xvfb openbox)"
fi

clean_targets() {
    # old tarballs and unpacked directories: we keep only the current one
    if [ -d target/dist-out ]; then
        find target/dist-out -maxdepth 1 -name 'kinetren-*' \
            ! -name "kinetren-${FULL#kt}-*" -exec rm -rf {} + 2>/dev/null || true
    fi
    if [ "$DEEP" = 1 ]; then
        # release/ and dist-container/ are fully regenerable and weigh gigabytes;
        # debug/ is not touched — otherwise the next edit waits for a full rebuild
        rm -rf target/release target/dist-container
    fi
    # stale artifacts of earlier states of the tree (2026-10-30: one agent day grew a worktree's
    # target to 142 GB against ~22 GB of a fresh build) — cargo names what this tree needs,
    # the rest of debug/deps and debug/incremental goes; nothing the next run needs is touched
    python3 scripts/target-prune.py || true
    du -sh target/debug target/release target/dist-container target/dist-out 2>/dev/null || true
}
stage "cleanup" clean_targets

mkdir -p target/dist-out
printf '%s\n%s\n' "$FULL" "$(pwd)/target/debug/kinetren" > target/dist-out/LATEST
echo

# Price of the check: the stage table, the total and the budget of five minutes (R.68).
python3 scripts/finish-budget.py --report "$TIMES"
echo

# Red is named BY ITS OWN NAME (R.55): one flag stood on three stages,
# and a fall of the GPU was printed as “live scenarios red” — the report lied
# about WHAT broke, and sent people to look in the wrong place.
[ -n "${LIVE_FAILED:-}" ] && echo "WARNING: live scenarios red — see target/live-out"
[ -n "${WS_FAILED:-}" ] && echo "WARNING: workshop tests red — see target/finish-workspace-fail.log"
[ -n "${REBUILD_FAILED:-}" ] && echo "WARNING: rebuild check red (t977) — a plugin edit recompiled a core crate"
echo "VERSION: $FULL$DIRTY"
echo "run: $(pwd)/target/debug/kinetren"

# R.67: USER PATH STEPS from the live scenarios go into the report. The rule “think
# about the user” holds on two mechanisms: the line PATH in the spec (the guard
# below) and PICTURES of what the person will see. Without a list they stay
# lying in target/live-out, and the report again turns into “tests green”.
STEPS=$(ls -1 target/live-out/*/[0-9]*-step-*.png 2>/dev/null | sort)
if [ -n "$STEPS" ]; then
    echo "live scenario steps (for the report):"
    echo "$STEPS" | sed 's|^|  |'
else
    echo "no live scenario steps — run: ./scripts/live-check.sh"
fi

# guard of the “user path” for new UI scenarios (R.67)
python3 "$(dirname "$0")/specs-user-path.py" --check || exit 1
# the owner's claims registry (R.71, t823): every numbered item of docs/ktprompts/*.txt has a
# row with a state from the closed set, a `done` names a live test, a `question` asks
python3 "$(dirname "$0")/ktprompts-index.py" --check || exit 1
# guards of the live check (R.68): waiting instead of sleep, and live coverage —
# how many places in the program the person has NOT touched with the mouse ONCE
python3 "$(dirname "$0")/live-sleeps.py" --check || exit 1
python3 "$(dirname "$0")/live-coverage.py" --check --brief || exit 1
# the repository's public face (readme-showcase, t826): English README and
# guide, links and images resolve, screenshots only from readme_shots.py
python3 "$(dirname "$0")/readme-check.py" --check || exit 1
# how Kinetren reaches people (github-ci-docker-build, t834): one build
# container and toolchain shared with CI, make targets, workflows, the name
python3 "$(dirname "$0")/dist-check.py" --check || exit 1
# R.43 item 301: the debug build counts honestly, but is 9 times slower. On a dense
# edit the audio render there goes past 100 % of real time, the ring empties
# and callback underruns grow — they are heard as silence. This is the price of the profile, not a
# defect, and speed cannot be judged from debug.
if [ ! -x target/release/kinetren ]; then
    echo "judge SPEED only by the release build: cargo build --release -p kt-app"
else
    echo "for speed measurements: $(pwd)/target/release/kinetren"
fi
[ "$RELEASE" = 1 ] && echo "tarball:   $(ls -1 target/dist-out/*.tar.gz 2>/dev/null | tail -1)"
exit 0

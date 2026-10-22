#!/usr/bin/env bash
# Live check in a VIRTUAL screen (R.27, parallel run and stand — R.68).
#
# Previously the “live check” was manual: launch the window, move the mouse around, look at
# the screenshot. It was not reproducible, required a free screen (if the session was locked,
# there was nothing to check with) and was not fit for regression. Here it is the same thing, but
# in Xvfb: the real kt-app binary, a real X server, real XTest events
# and real wgpu (software rendering) — unlike the harness tests, which run
# egui without a window.
#
# From R.68 scenarios are handed out to WORKERS (owner’s decision: “Yes, with a load limit
# and auto-restart of the red one”). Each worker has its own Xvfb, its own
# user storage and one live kt-app — a stand in which the projects of
# scenarios are opened one after another (scripts/live/worker.py). The ban of R.59 §468
# (“live scenarios are not run in two streams”) is lifted together with its reason:
# a red scenario is automatically re-run ALONE, and only
# a second failure counts as red.
#
#     ./scripts/live-check.sh                # all scenarios
#     ./scripts/live-check.sh handles fade   # only the named ones
#     KT_LIVE_WORKERS=1 ./scripts/live-check.sh   # strictly one by one, no re-runs
#
# Screenshots and logs — target/live-out/<scenario>/.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
# RUN DIRECTORY — overridable (R.68): the guards t801/t802 run
# live-check during a workshop run, and it cleans the verdict directory —
# so a foreign RUNNING run lost its verdicts, declaring everything red.
out="${KT_LIVE_OUT:-$root/target/live-out}"
scenarios_dir="$root/scripts/live"
display_base="${KT_LIVE_DISPLAY:-:99}"
display_base="${display_base#:}"

if ! command -v Xvfb > /dev/null; then
    echo "no Xvfb: sudo apt install xvfb openbox" >&2
    exit 2
fi
if [ ! -x "$root/target/debug/kinetren" ]; then
    echo "no target/debug/kinetren: cargo build -p kt-app" >&2
    exit 2
fi

mkdir -p "$out"
rm -rf "$out/verdicts"

want=("$@")
if [ ${#want[@]} -eq 0 ]; then
    # kt_live.py — the frame, worker.py — the worker, balance.py — the distribution by
    # cost, `_*.py` — stand-in scenarios of the guards (t801/t802): none of these are
    # scenarios, and they are not part of the common run
    mapfile -t want < <(cd "$scenarios_dir" && ls *.py \
        | grep -v -E '^(kt_live|worker|balance|_.*)\.py$' | sed 's/\.py$//')
fi

# How many workers. One kt-app in software rendering holds a load of about seven
# (measurement R.68), so we divide the cores by eight: exactly a load of ~14 on two
# copies is what painted the green scenarios red in §468. The ceiling is six: beyond that we hit
# the disk and the X servers, and the gain turns into noise.
cores=$(nproc 2> /dev/null || echo 4)
auto=$((cores / 8))
[ "$auto" -lt 1 ] && auto=1
[ "$auto" -gt 6 ] && auto=6
workers="${KT_LIVE_WORKERS:-$auto}"
[ "${#want[@]}" -lt "$workers" ] && workers="${#want[@]}"
[ "$workers" -lt 1 ] && workers=1

run_worker() { # run_worker <number> <scenarios...>
    local n="$1"; shift
    [ $# -eq 0 ] && return 0
    local disp=":$((display_base + n))"
    python3 "$scenarios_dir/worker.py" "$disp" "$out" "$@" 2>&1 \
        | sed "s/^/[$n] /"
}

echo "live scenarios: ${#want[@]} pcs., workers $workers (cores $cores)"

# DISTRIBUTION BY COST, not by order (R.68): workers write the time of each
# scenario into `times.txt`, and the next run hands them out as “the longest one —
# to the least busy” (greedy LPT). Otherwise one worker gets `nestparam`
# (136 s) and holds the whole run while the other three have already finished.
mapfile -t order < <(python3 "$scenarios_dir/balance.py" "$out/times.txt" "$workers" "${want[@]}")

pids=()
for ((w = 0; w < workers; w++)); do
    read -r -a bucket <<< "${order[w]:-}"
    run_worker "$w" "${bucket[@]}" &
    pids+=($!)
done

for p in "${pids[@]}"; do wait "$p" || true; done

verdicts="$out/verdicts"
reds=()
for name in "${want[@]}"; do
    if [ "$(cat "$verdicts/$name" 2> /dev/null || echo fail)" != ok ]; then
        reds+=("$name")
    fi
done

# COST OF THE RUN’S SLEEP (R.69): the sum of the actual fixed sleep of all
# scenarios against their total time. A number for the report and the next
# measurement, NOT a verdict: under workers it is not sleep that grows but waiting, and a threshold
# on it would catch load instead of regression. The verdict is held by the static
# limit of declared seconds (scripts/live-sleeps.py).
slept_files=()
for name in "${want[@]}"; do
    [ -f "$out/$name/slept.txt" ] && slept_files+=("$out/$name/slept.txt")
done
if [ ${#slept_files[@]} -gt 0 ]; then
    awk -F'\t' '{s+=$1; t+=$2} END {if (t>0) printf "run sleep: %.0f s of %.0f s sequential time (%.0f %%)\n", s, t, 100*s/t}' \
        "${slept_files[@]}"
fi

if [ ${#reds[@]} -eq 0 ]; then
    echo "live scenarios: ${#want[@]} of ${#want[@]} green"
    exit 0
fi

# RE-RUN OF RED ALONE (R.68). Going red under load is not
# a verdict: §468 showed that a second run next to it paints green scenarios red, and
# exactly this rule is already written for “fell during a pause or in the order of a background
# stream”. Now the machine enforces it, not the agent’s memory. A silent
# re-run has no right to exist — otherwise flakiness becomes invisible.
if [ "$workers" = 1 ]; then
    echo "RED (the run is already single-process): ${reds[*]}" >&2
    exit 1
fi
echo "rerun alone: ${reds[*]}"
again=()
for name in "${reds[@]}"; do
    # `|| true`: the worker returns the scenario’s exit code, and `set -e` on it silently
    # aborted the WHOLE re-run — the report lost both “passed the second time” and
    # “red twice” (caught by test t801)
    run_worker 0 "$name" || true
    if [ "$(cat "$verdicts/$name" 2> /dev/null || echo fail)" != ok ]; then
        again+=("$name")
    fi
done

for name in "${reds[@]}"; do
    case " ${again[*]} " in
        *" $name "*) ;;
        *) echo "PASSED ON THE SECOND TRY (alone): $name — analyze the scenario" ;;
    esac
done

if [ ${#again[@]} -eq 0 ]; then
    echo "live scenarios: green (some on the second try)"
    exit 0
fi
echo "RED TWICE: ${again[*]} (screenshots: target/live-out/<scenario>)" >&2
exit 1

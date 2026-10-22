#!/usr/bin/env python3
"""The guard “waiting, not sleeping” for live scenarios (R.68).

R.68 measurement: 96 % of the live run time went to unconditional `time.sleep`
(`nestparam` — 130.5 s of 135.9 in 591 calls; `curvesrc` — 41.9 of 43.2).
Pauses were tuned to software rendering (“a frame takes ~0.2 s”), that is, the framework
waited for HOURS for what the program can say itself: it reports the number
of the drawn frame and the count of input events read.

The rule this guard enforces:

* in a SCENARIO (`scripts/live/*.py`) there is no raw `time.sleep` at all —
  waiting goes through the framework (`wait_frame`, `wait_seen`, `settle`,
  `wait_playhead`, `pixel_settled`), and a polling loop uses `poll_step()`;
* in the FRAMEWORK the raw `time.sleep` lives in exactly three places: `pause()` —
  a pause WITH A REASON, `_poll()` — a waiting step inside the framework, and
  `poll_step()` — a polling step for scenario loops;
* each call of `pause(...)` has a reason in words of at least forty characters;
* EVERY GESTURE of the framework waits until the program reads the input (`_after_gesture`),
  and keeps no fixed pauses inside a gesture — the legitimate places are listed
  by name (`PAUSE_OK_IN`). This is the R.69 rule: waiting on an event was
  built in R.68 but did not reach the gestures themselves.

The “forty characters” measure is the same as for the PATH guard (R.67) and the window registry
(R.66): an exception must be MEASURABLE, otherwise an excuse becomes
cheaper than the description.

    python3 scripts/live-sleeps.py --check
    python3 scripts/live-sleeps.py --check --scripts <copy of the scripts directory>
"""
import ast
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
# ROOT OF THE CHECKED SOURCES — OVERRIDABLE (`--scripts <directory>`).
# The guard’s tests (t803, t808) check it with VIOLATIONS: they substitute a scenario
# with a raw pause, break the driver, raise a constant. To do this in the working
# tree is impossible — the tests run at the same time (threads in `cargo test`, processes in
# nextest), and corrupting one becomes a red verdict for another. Exactly this
# class (“flakiness by construction”) the round removes, so the rule
# touches its own tests too: each works on its OWN copy.
SCRIPTS = ROOT / "src" / "scripts"
if "--scripts" in sys.argv:
    SCRIPTS = Path(sys.argv[sys.argv.index("--scripts") + 1]).resolve()
LIVE = SCRIPTS / "live"
FRAMEWORK = "kt_live.py"
# MOUSE AND KEYBOARD DRIVER (R.69). Before this round the guard looked only at
# the frame and the scenarios, while the driver slept inside itself — 123 s of the 387 s of sleep
# of the whole live run lived OUTSIDE the scope of the rule “a pause only with a reason”.
# Sleep hidden in the driver is no different from sleep in the frame.
DRIVER = SCRIPTS / "ui-probe.py"
DRIVER_SLEEP_HOME = "_nap"
# the worker frame raises Xvfb and processes — there the wait is system-level, not
# window-level: there is no “frame” to wait for yet, the program is not running
RUNNER = "worker.py"
SLEEP_HOMES = {"pause", "_poll", "poll_step"}
MIN_REASON = 40

# GESTURE FRAME and the rule of R.69: each waits until the PROGRAM has read the input
# (`_after_gesture`), not as long as a frame takes “usually”. The rule
# is held by the guard, because in prose it already failed to hold: the wait on an
# event was built in R.68 and did not reach the gestures.
GESTURES = {
    "click",
    "right_click",
    "double_click",
    "drag",
    "key",
    "wheel",
    "hover",
    "close_window",
    "type_text",
}
# Where a fixed pause inside a gesture is LAWFUL, and why exactly there:
#   _after_gesture — the fallback, when an egui event legitimately will not come at all;
#   focus          — the window manager raises the window, and the program has no
#                    input event of its own here;
#   double_click   — the gap between clicks is a TEMPORARY egui threshold (R.34);
#   type_text      — the gap between characters, when the scenario asks for it.
PAUSE_OK_IN = {"_after_gesture", "focus", "double_click", "type_text"}

# SLEEP BUDGET — RATCHET (R.69, stage 1). The reasons in words turned out not enough:
# all 265 s of sleep in the frame were “lawful”, each pause carried
# an explanation longer than forty characters. So the SUM of
# DECLARED SECONDS is also counted: each constant in the frame’s `pause(...)` and in the
# driver’s sleep defaults. The measure is static — it depends neither on the machine nor
# on how many times the gesture was called: a verdict that depends on load is
# flakiness by construction (the actual sleep of the run is printed by live-check.sh).
#
# The limit is moved DELIBERATELY, in one line and with a reason — like
# `UNTOUCHED_LIMIT` of the live coverage report (R.68). R.69: 14.9 → 11.2
# after the gestures moved to waiting for the read input; the actual
# sleep of the run at that moment went 539 → 41 s.
SLEEP_BUDGET = 11.2


def raw_sleeps(tree):
    """(line, name of the enclosing function) for each `time.sleep(...)`."""
    out = []

    def walk(node, fn):
        for child in ast.iter_child_nodes(node):
            name = fn
            if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef)):
                name = child.name
            if (
                isinstance(child, ast.Call)
                and isinstance(child.func, ast.Attribute)
                and child.func.attr == "sleep"
                and isinstance(child.func.value, ast.Name)
                and child.func.value.id == "time"
            ):
                out.append((child.lineno, fn))
            walk(child, name)

    walk(tree, None)
    return out


def pause_calls(tree):
    """(line, reason) for each call of `pause(...)`."""
    out = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == "pause":
            reason = None
            if len(node.args) >= 2:
                arg = node.args[1]
                if isinstance(arg, ast.Constant) and isinstance(arg.value, str):
                    reason = arg.value
                elif isinstance(arg, ast.JoinedStr):
                    reason = "".join(
                        v.value for v in arg.values if isinstance(v, ast.Constant)
                    )
            out.append((node.lineno, reason))
    return out


def param_defaults(tree, call):
    """Numeric default values of parameters of the function in whose body `call` stands."""
    for fn in ast.walk(tree):
        if not isinstance(fn, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        if not any(n is call for n in ast.walk(fn)):
            continue
        args = fn.args
        pairs = zip(args.args[len(args.args) - len(args.defaults):], args.defaults)
        return {
            a.arg: float(d.value)
            for a, d in pairs
            if isinstance(d, ast.Constant) and isinstance(d.value, (int, float))
        }
    return {}


def declared_pauses(tree, rel):
    """[(seconds, where)] for each call of `pause(<number>, …)` in the tree.

    In the FRAMEWORK the duration must be a NUMBER AT THE CALL SITE: a pause
    hidden behind a constant name never reached the budget at all — a gap
    was found as soon as it was used (double-click spacing,
    R.69). In a SCENARIO the duration may legitimately be a parameter: for `rate`
    the pause is the check itself (the program must play that long), and for
    the typing of a line the gap comes from the scenario. Such ones do not count toward the budget and
    are named in the report on a separate line. The second return value is the list
    of violations, the third is the computed pauses.
    """
    out, bad, computed = [], [], []
    for node in ast.walk(tree):
        if not (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id == "pause"
            and node.args
        ):
            continue
        arg = node.args[0]
        if isinstance(arg, ast.Constant) and isinstance(arg.value, (int, float)):
            out.append((float(arg.value), f"{rel}:{node.lineno}"))
        elif isinstance(arg, ast.Name) and arg.id in param_defaults(tree, node):
            # the duration came in as a PARAMETER with a numeric default: the budget counts the default —
            # how long the frame sleeps when the scenario did not ask for
            # anything else (`type_text(gap=0.0)`)
            out.append((param_defaults(tree, node)[arg.id], f"{rel}:{node.lineno}"))
        elif rel.endswith(FRAMEWORK):
            bad.append(
                f"{rel}:{node.lineno}: the duration of a FRAMEWORK pause is not a number at the "
                "call site — such a pause does not reach the sleep budget and becomes invisible"
            )
        else:
            computed.append(f"{rel}:{node.lineno}")
    return out, bad, computed


def driver_sleeps(tree):
    """Sleep of driver X: raw `time.sleep` outside `_nap` and sleepy DEFAULTS.

    A default is a pause too, and it is more dangerous than one written in the call: it is not
    visible at the gesture site. Before R.69 `move(pause=0.05)`, `button(pause=0.09)` and
    `key(pause=0.05, after=0.12)` slept behind the framework exactly that way.
    """
    raw = [(line, fn) for line, fn in raw_sleeps(tree) if fn != DRIVER_SLEEP_HOME]
    seconds = 0.0
    defaults = []
    for node in ast.walk(tree):
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        args = node.args
        pairs = list(zip(args.args[len(args.args) - len(args.defaults):], args.defaults))
        for arg, default in pairs:
            if arg.arg not in ("pause", "after"):
                continue
            if isinstance(default, ast.Constant) and isinstance(default.value, (int, float)):
                seconds += float(default.value)
                defaults.append((node.name, arg.arg, float(default.value)))
    return raw, seconds, defaults


def gesture_faults(tree):
    """A gesture that does not wait for the program, and a pause inside a gesture not on the list."""
    bad = []
    for node in ast.walk(tree):
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        if node.name not in GESTURES:
            continue
        src = ast.dump(node)
        if "'_after_gesture'" not in src:
            bad.append(
                f"gesture `{node.name}` does not wait for the READ input (`_after_gesture`): "
                "a fixed pause does not stretch under load — this is the very "
                "class of false reds (§468) that R.69 removes"
            )
        if node.name in PAUSE_OK_IN:
            continue
        for line, _reason in pause_calls(node):
            bad.append(
                f"kt_live.py:{line}: a fixed pause inside the gesture `{node.name}`. "
                "Legitimate places are listed (`PAUSE_OK_IN`): the waiting fallback, raising "
                "the window, the double-click time threshold, the typing gap requested by the scenario"
            )
    return bad


def check():
    bad = []
    declared = []
    computed = []
    for path in sorted(LIVE.glob("*.py")):
        if path.name == RUNNER:
            continue
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        rel = f"{SCRIPTS.name}/live/{path.name}"
        for line, fn in raw_sleeps(tree):
            if path.name == FRAMEWORK and fn in SLEEP_HOMES:
                continue
            if path.name == FRAMEWORK:
                bad.append(
                    f"{rel}:{line}: raw `time.sleep` in the framework outside `pause`/`_poll` "
                    f"(function {fn}) — waiting must wait for a frame or the input of the program"
                )
            else:
                bad.append(
                    f"{rel}:{line}: raw `time.sleep` in a scenario — it must wait for "
                    "the program (`wait_frame`/`wait_seen`/`settle`), not the clock"
                )
        for line, reason in pause_calls(tree):
            if reason is None:
                bad.append(f"{rel}:{line}: `pause(...)` without a reason in words")
            elif len(reason) < MIN_REASON:
                bad.append(
                    f"{rel}:{line}: the reason for the pause is shorter than {MIN_REASON} characters: {reason!r}"
                )
        seen_pauses, bad_pauses, computed_pauses = declared_pauses(tree, rel)
        declared.extend(seen_pauses)
        bad.extend(bad_pauses)
        computed.extend(computed_pauses)
        if path.name == FRAMEWORK:
            bad.extend(gesture_faults(tree))

    # X DRIVER — the same area (R.69)
    drv = ast.parse(DRIVER.read_text(encoding="utf-8"), filename=str(DRIVER))
    raw, drv_seconds, defaults = driver_sleeps(drv)
    for line, fn in raw:
        bad.append(
            f"{DRIVER.name}:{line}: raw `time.sleep` in the driver outside "
            f"`{DRIVER_SLEEP_HOME}` (function {fn}) — the sleep of the driver must be "
            "visible and counted on a par with the sleep of the framework"
        )
    declared.extend(
        (v, f"{DRIVER.name} ({f}.{a})") for f, a, v in defaults
    )
    budget = sum(v for v, _ in declared)

    if computed:
        print(
            "pauses with a computed duration (not in the budget; the duration "
            f"comes from the scenario): {', '.join(computed)}"
        )
    print(
        f"declared sleep of the live check: {budget:.2f} s against a limit of "
        f"{SLEEP_BUDGET:.2f} s (in the driver {drv_seconds:.2f} s: "
        + ", ".join(f"{f}.{a}={v}" for f, a, v in defaults)
        + ")"
    )
    if budget > SLEEP_BUDGET + 1e-9:
        worst = max(declared, default=(0.0, "—"))
        bad.append(
            f"declared sleep {budget:.2f} s exceeded the limit {SLEEP_BUDGET:.2f} s; "
            f"the most expensive pause is {worst[1]} ({worst[0]} s). The limit is moved "
            "deliberately, in one line and with a reason (SLEEP_BUDGET)"
        )
    if bad:
        print("WAITING REPLACED BY SLEEP:", file=sys.stderr)
        for b in bad:
            print("  " + b, file=sys.stderr)
        return 1
    print("live scenarios wait for the program, not the clock (no raw pauses)")
    return 0


if __name__ == "__main__":
    if "--check" not in sys.argv[1:]:
        print(__doc__)
        sys.exit(2)
    sys.exit(check())

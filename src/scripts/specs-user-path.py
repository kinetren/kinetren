#!/usr/bin/env python3
"""Guard “USER PATH” for UI scenarios (R.67).

The owner: “HOW MANY TIMES DO I HAVE TO REPEAT SO THAT ALL ___ALL___ AGENTS THINK ABOUT
THE USER FIRST? … THIS IS ABSURD, IT NEEDS CHANGING!”. The rule
in prose (the kt-app checklist, R.40) held for seven rounds and did not work:
the check “what the person sees” must be part of the CONTRACT TEXT.

Each scenario of a UI capability whose test lives in `src/apps/kt-app/` and has
number tNNN ≥ 783 must carry a line `- **PATH**`: where to click, what to drag, and what is
visible after each step. The threshold is by number, not “for everyone”:
numbering is continuous and monotonic, and rewriting two hundred earlier
scenarios for the sake of form is pointless — the rule applies from the round in which it was
introduced.

Both contracts (`openspec/specs`) and active changes
(`openspec/changes/*/specs`) are checked: otherwise the guard would be silent
exactly where a new scenario is written.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CAPS = ("ui-timeline", "ui-reach")
FIRST = 783  # first test number from which the rule applies

SCEN = re.compile(r"^#### Scenario: (.+)$")
TEST = re.compile(r"^- \*\*TEST\*\* (.+)$")
PATH = re.compile(r"^- \*\*PATH\*\*\s*(.*)$")
TNUM = re.compile(r"\bt(\d+)[a-z]?\b")


def spec_files():
    for cap in CAPS:
        p = ROOT / "openspec" / "specs" / cap / "spec.md"
        if p.exists():
            yield p
    for d in sorted((ROOT / "openspec" / "changes").glob("*/specs")):
        if "archive" in d.parts:
            continue
        for cap in CAPS:
            p = d / cap / "spec.md"
            if p.exists():
                yield p


def check(path):
    bad = []
    name, test_line, path_line = None, None, None

    def close():
        if name is None or test_line is None:
            return
        in_app = "src/apps/kt-app/" in test_line
        nums = [int(n) for n in TNUM.findall(test_line)]
        if not in_app or not nums or max(nums) < FIRST:
            return
        if path_line is None:
            bad.append((name, "no PATH line"))
        elif len(path_line.strip()) < 40:
            bad.append((name, f"PATH shorter than 40 characters: {path_line.strip()!r}"))

    for line in path.read_text(encoding="utf-8").splitlines():
        m = SCEN.match(line)
        if m:
            close()
            name, test_line, path_line = m.group(1), None, None
            continue
        if name is None:
            continue
        m = TEST.match(line)
        if m:
            test_line = m.group(1)
            continue
        m = PATH.match(line)
        if m:
            path_line = m.group(1)
    close()
    return bad


def main():
    problems = []
    for f in spec_files():
        for name, why in check(f):
            problems.append(f"{f.relative_to(ROOT)}: scenario “{name}” — {why}")
    if problems:
        print("UI scenario without a “user path” (what the person sees is part of the contract):")
        for p in problems:
            print("  " + p)
        return 1
    print("UI scenarios with kt-app tests from t783: the user path is described in all of them")
    return 0


if __name__ == "__main__":
    sys.exit(main())

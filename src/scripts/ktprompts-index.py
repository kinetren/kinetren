#!/usr/bin/env python3
"""Guard of the owner's claims registry (R.71, change r71-ktprompts-index).

    python3 src/scripts/ktprompts-index.py --check              the registries in docs/ktprompts/
    python3 src/scripts/ktprompts-index.py --check --dir <dir>  another directory (test t823)

The owner: "make important notes for the agents, tests, etc., so that we do not return to the
same kinds of problems". Without a registry the next round takes from his file what is already
done, or does the opposite of what was decided — and a rule in prose already failed here twice
("do not make obvious labels" — item 17 of R.7, and then "Add morpher here").

Every claims file `docs/ktprompts/<name>.txt` (verbatim) has a registry `<name>-index.md`. A
registry row is a Markdown table row `| # | Line | Claim | State | Address | Stage |` above the
section "What OTIO export loses". The guard is red when:

- a source has no registry (the file name is printed);
- a numbered line of the source (`1.`, `5.1.`, `a.`, `TODO:`) falls into no row's `Line`
  (the address `<file>:<line>` and the start of the text are printed);
- a `Line` points past the end of the source, or two rows share a `#`;
- the state is outside the closed set `done | partial | no | question | future`;
- `done` names no round (`R.NN` or a change) or no test, or a named test `tNNN` has no
  `fn tNNN_` in the sources of `src/` (the same rule as the MIGRATED registry of live-coverage.py);
- `no: … defect` names no capability of `openspec/specs/` and no change;
  `no: … feature` names no note `F-…`, no "no note" and no change;
- `question` carries no question text (a `?`);
- the address or the stage is empty.

The guard keeps the FORM and the existence of tests, not the truth of "done": the truth is the
live check in the round that takes the item (design.md, "Risks").

There are no dependencies; the line numbers of a source are the numbers `cat -n` prints.
"""

import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_DIR = ROOT / "docs" / "ktprompts"
SRC = ROOT / "src"
SPECS = ROOT / "openspec" / "specs"
CHANGES = ROOT / "openspec" / "changes"

STATES = ("done", "partial", "no", "question", "future")
LOSES = "## What OTIO export loses"

# An item of a source: a number with dots ("1.", "5.1.", "11."), a letter sub-item ("a."),
# a TODO line. A plain sentence that merely starts with a digit has no dot right after it.
ITEM = re.compile(r"^\s*(?:\d+(?:\.\d+)*\.|[a-z]\.|TODO:)\s*\S")
LINES = re.compile(r"^\d+(?:\s*[–-]\s*\d+)?(?:\s*,\s*\d+(?:\s*[–-]\s*\d+)?)*$")
TEST = re.compile(r"\bt(\d{1,4}[a-z]?)\b")
ROUND = re.compile(r"\bR\.\d+\b")
FNAME = re.compile(r"\bfn t(\d{1,4}[a-z]?)_")


def parse_lines(spec):
    out = set()
    for part in spec.split(","):
        a, _, b = part.replace("–", "-").partition("-")
        lo = int(a)
        hi = int(b) if b.strip() else lo
        out.update(range(lo, hi + 1))
    return out


def registry_rows(path):
    """Rows of the registry: (md line, #, lines spec, claim, state, address, stage)."""
    rows = []
    for n, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if line.startswith(LOSES):
            break
        if not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if len(cells) < 6 or not LINES.match(cells[1]):
            continue  # a header, a separator, or another table
        rows.append((n, *cells[:6]))
    return rows


def known_tests():
    # os.walk with pruning: rglob would descend into src/target (tens of GB of build output)
    found = set()
    for top, dirs, files in os.walk(SRC):
        dirs[:] = [d for d in dirs if d != "target" and not d.startswith(".")]
        for f in files:
            if f.endswith(".rs"):
                try:
                    text = Path(top, f).read_text(encoding="utf-8", errors="replace")
                except OSError:
                    continue
                found.update(FNAME.findall(text))
    return found


def known_changes():
    names = set()
    if CHANGES.is_dir():
        for d in CHANGES.iterdir():
            if d.is_dir() and d.name != "archive":
                names.add(d.name)
        arch = CHANGES / "archive"
        if arch.is_dir():
            for d in arch.iterdir():
                # archived as <date>-<name>
                names.add(re.sub(r"^\d{4}-\d{2}-\d{2}-", "", d.name))
    return names


def check_state(state, address, caps, changes, tests):
    """The error text of one row's state, or None."""
    text = f"{state} {address}"
    word = re.sub(r"[*`]", "", state).strip().split(":")[0].split()[0].lower() if state.strip() else ""
    if word not in STATES:
        return f"state «{state}» is outside the set {' | '.join(STATES)}"
    named_change = any(re.search(rf"\b{re.escape(c)}\b", text) for c in changes)
    if word == "done":
        if not (ROUND.search(state) or named_change):
            return "`done` names no round (R.NN or a change)"
        nums = TEST.findall(state)
        if not nums:
            return "`done` names no test tNNN"
        missing = [f"t{t}" for t in nums if t not in tests]
        if missing:
            return f"`done` refers to a test that does not exist in src/: {', '.join(missing)}"
    if word == "no":
        low = text.lower()
        if "defect" in low and not (named_change or any(re.search(rf"\b{re.escape(c)}\b", text) for c in caps)):
            return "a defect names no capability of openspec/specs and no change"
        if "feature" in low and not (named_change or re.search(r"\bF-[A-Z]+-\d+", text) or "no note" in low):
            return "a feature names no note F-…, no \"no note\" and no change"
    if word == "question":
        tail = re.sub(r"^\W*question\b[^:]*:?", "", re.sub(r"[*`]", "", state), flags=re.I).strip()
        if "?" not in tail or len(tail) < 10:
            return "`question` without the text of the question"
    return None


def check_source(txt, caps, changes, tests):
    """(errors, counts by state) of one source and its registry."""
    errors = []
    reg = txt.with_name(txt.stem + "-index.md")
    if not reg.exists():
        return [f"{txt.name}: the source has no registry {reg.name}"], {}
    src_lines = txt.read_text(encoding="utf-8").splitlines()
    covered = set()
    seen = {}
    counts = {s: 0 for s in STATES}
    rows = registry_rows(reg)
    if not rows:
        errors.append(f"{reg.name}: no registry rows")
    for md_line, num, spec, claim, state, address, stage in rows:
        where = f"{reg.name}:{md_line} [{num}]"
        if num in seen:
            errors.append(f"{where}: the # repeats the row of line {seen[num]}")
        seen[num] = md_line
        lines = parse_lines(spec)
        past = sorted(n for n in lines if n < 1 or n > len(src_lines))
        if past:
            errors.append(f"{where}: Line {spec} is past the end of {txt.name} ({len(src_lines)} lines)")
        covered |= lines
        bad = check_state(state, address, caps, changes, tests)
        if bad:
            errors.append(f"{where}: {bad}")
        else:
            counts[re.sub(r"[*`]", "", state).strip().split(":")[0].split()[0].lower()] += 1
        if not claim:
            errors.append(f"{where}: empty claim")
        if not address:
            errors.append(f"{where}: empty address")
        if not stage:
            errors.append(f"{where}: empty stage (a round, a change or \"later\")")
    for n, line in enumerate(src_lines, 1):
        if ITEM.match(line) and n not in covered:
            errors.append(f"{txt.name}:{n}: the item has no registry row — «{line.strip()[:70]}»")
    return errors, counts


def main(argv):
    if "--check" not in argv:
        print(__doc__)
        return 2
    d = DEFAULT_DIR
    if "--dir" in argv:
        d = Path(argv[argv.index("--dir") + 1])
    sources = sorted(d.glob("*.txt"))
    if not sources:
        print(f"ktprompts: no claims files in {d}")
        return 1
    caps = sorted(p.name for p in SPECS.iterdir() if (p / "spec.md").exists()) if SPECS.is_dir() else []
    changes = known_changes()
    tests = known_tests()
    all_errors = []
    for txt in sources:
        errors, counts = check_source(txt, caps, changes, tests)
        all_errors += errors
        if counts:
            summary = ", ".join(f"{s} {counts[s]}" for s in STATES if counts[s])
            print(f"ktprompts {txt.stem}: {sum(counts.values())} rows — {summary}")
    if all_errors:
        print(f"ktprompts: {len(all_errors)} error(s):")
        for e in all_errors:
            print(f"  {e}")
        return 1
    print("ktprompts: every claim has a row")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

#!/usr/bin/env python3
"""Cost of verification: a table “stage — seconds”, total and budget (R.68).

The occasion verbatim (owner): “tests in finish.sh or something like that - very
slow to run, we need to think of something”. The measurement showed that the verification round
takes 21 minutes, but NO stage named its own cost — there was nothing to say
about what to speed up, and the degradation was visible to no one.

The budget is 300 s (five minutes, set by the owner). An overrun IS PRINTED and
is marked, but does NOT make the run red: otherwise the budget would become a knife against
the set of checks, and the rule “tests from the specs are not weakened” outweighs the budget
(see openspec/specs/durability/spec.md).

    finish-budget.py --report target/finish-times.txt    # table and verdict
    finish-budget.py --check-script scripts/finish.sh    # all stages in the measurement
    finish-budget.py --selftest                          # self-check (t800)

Format of the measurement file — one line per stage: “name<TAB>seconds<TAB>code”.
"""
import re
import sys
from pathlib import Path

BUDGET = 300  # s; five minutes — the owner’s decision (R.68)


class Bad(Exception):
    """The accounting itself is broken: this is red, not an overrun."""


def read_times(path):
    p = Path(path)
    if not p.exists():
        raise Bad(f"no measurement file {path} — the stages were not timed")
    rows = []
    for n, line in enumerate(p.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        parts = line.split("\t")
        if len(parts) != 3:
            raise Bad(f"{path}:{n}: the line is not “name<TAB>seconds<TAB>code”: {line!r}")
        name, secs, rc = parts
        try:
            rows.append((name, int(secs), int(rc)))
        except ValueError as e:
            raise Bad(f"{path}:{n}: not a number: {line!r}") from e
    if not rows:
        raise Bad(f"{path}: not a single measurement — the stages were not timed")
    return rows


def report(rows, budget=BUDGET, out=sys.stdout):
    total = sum(s for _, s, _ in rows)
    width = max(len(n) for n, _, _ in rows)
    print("== cost of verification ==", file=out)
    for name, secs, rc in sorted(rows, key=lambda r: -r[1]):
        mark = "" if rc == 0 else "  ← RED"
        share = 100.0 * secs / total if total else 0.0
        print(f"  {secs:6d} s  {name:<{width}}  {share:4.0f} %{mark}", file=out)
    print(f"  {total:6d} s  {'TOTAL':<{width}}  budget {budget} s", file=out)
    if total > budget:
        worst, wsecs, _ = max(rows, key=lambda r: r[1])
        print(
            f"WARNING: verification exceeded the budget {budget} s by {total - budget} s; "
            f"more expensive than all “{worst}” ({wsecs} s, {100.0 * wsecs / total:.0f} %)",
            file=out,
        )
    else:
        print(f"verification within budget: {total} s of {budget} s", file=out)
    return total


# ---- guard of the script itself ------------------------------------------------
#
# A stage could quietly be taken out of the budget — its header printed outside
# the wrapper, and its seconds vanish from the table. This is the same class as “red under
# someone else’s name” (R.55) and “verdict from tail” (R.58): the report lies about what
# happened. So the only lawful way to print a stage header is the wrapper,
# and the guard checks that.
# A stage header is a line PRINTED that STARTS with “== ”. It must not be wider:
# “simd: ok (AVX2 == scalar)” is an ordinary verdict line, and a guard that failed on it
# would teach people to get around it by rewording.
HEADER = re.compile(r"""(?:echo|printf)\s+["']==\s""")


def check_script(path):
    p = Path(path)
    if not p.exists():
        raise Bad(f"no script {path}")
    src = p.read_text(encoding="utf-8")
    bad = []
    in_stage = False
    depth = 0
    for n, line in enumerate(src.splitlines(), 1):
        stripped = line.strip()
        if re.match(r"^stage(_log)?\s*\(\)\s*\{", stripped):
            in_stage, depth = True, 1
            continue
        if in_stage:
            depth += line.count("{") - line.count("}")
            if depth <= 0:
                in_stage = False
            continue
        if stripped.startswith("#"):
            continue
        if HEADER.search(stripped):
            bad.append((n, stripped))
    if bad:
        raise Bad(
            "the stage heading is printed outside the stage wrapper — its seconds will not reach "
            "the budget:\n"
            + "\n".join(f"  {path}:{n}: {t}" for n, t in bad)
        )
    for need, why in (
        ("finish-budget.py", "the script does not call the cost accounting — there will be no stage table"),
        ("finish-times.txt", "the script does not write the measurement file"),
    ):
        if need not in src:
            raise Bad(f"{why} (no mention of {need})")
    return True


def selftest():
    import io

    # 1) overrun: named the most expensive stage and the size of the search
    buf = io.StringIO()
    report([("live scenarios", 1012, 0), ("workshop", 259, 0)], out=buf)
    said = buf.getvalue()
    assert "WARNING" in said and "live scenarios" in said, said
    assert "by 971 s" in said, said  # 1012 + 259 - 300

    # 2) within budget: no mark
    buf = io.StringIO()
    report([("build", 43, 0), ("workshop", 90, 0)], out=buf)
    said = buf.getvalue()
    assert "WARNING" not in said and "within budget" in said, said

    # 3) a red stage is named red directly in the table
    buf = io.StringIO()
    report([("live scenarios", 120, 1)], out=buf)
    assert "RED" in buf.getvalue(), buf.getvalue()

    # 4) broken accounting is red, not “zero stages”
    for rows, what in ((None, "no file"), ("", "empty file")):
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "times.txt"
            if rows is not None:
                f.write_text(rows, encoding="utf-8")
            try:
                read_times(f)
            except Bad:
                pass
            else:
                raise AssertionError(f"{what}: the accounting stayed silent")

    # 5) the guard sees a stage header outside the wrapper
    import tempfile

    with tempfile.TemporaryDirectory() as d:
        f = Path(d) / "finish.sh"
        f.write_text(
            'stage() {\n  printf "== %s ==\\n" "$1"\n}\n'
            "# finish-budget.py finish-times.txt\n"
            'stage "build" cargo build\n',
            encoding="utf-8",
        )
        assert check_script(f) is True
        f.write_text(
            'stage() {\n  printf "== %s ==\\n" "$1"\n}\n'
            "# finish-budget.py finish-times.txt\n"
            'echo "== cleanup =="\n',
            encoding="utf-8",
        )
        try:
            check_script(f)
        except Bad as e:
            assert "outside the stage wrapper" in str(e), e
        else:
            raise AssertionError("the guard did not notice a stage outside the wrapper")

    print("finish-budget: self-check green (overrun, budget, red stage, "
          "broken accounting, stage outside the wrapper)")


def main(argv):
    if len(argv) == 2 and argv[1] == "--selftest":
        selftest()
        return 0
    if len(argv) == 3 and argv[1] == "--report":
        report(read_times(argv[2]))
        return 0
    if len(argv) == 3 and argv[1] == "--check-script":
        check_script(argv[2])
        print(f"{argv[2]}: all stages go through the timed wrapper")
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv))
    except Bad as e:
        print(f"COST ACCOUNTING BROKEN: {e}", file=sys.stderr)
        sys.exit(1)

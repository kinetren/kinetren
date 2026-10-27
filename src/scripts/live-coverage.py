#!/usr/bin/env python3
"""Live coverage: which places a person has NEVER reached with the mouse (R.68).

Decision of the owner: “part of the tests should move to non-gui, since tests partly repeat
each other, so a large simple part can calmly go to headless, while the complex moments
and those never clicked through must be clicked by hand. So that “never clicked through”
is not a matter of taste,
the machine counts it.”

What is checked against what:

* the PROGRAM publishes rectangles of named widgets
  (`widgets::field_probe::note("…")`) — these are the places a live
  scenario can aim at;
* SCENARIOS click them by name (`click_probe`, `right_click_probe`,
  `double_click_probe`, `hover_probe`, `has_probe`,
  `scroll_probe_into_view`) — OR take a probe rectangle from the dump
  (`f["param"] == "bin.reel"`) and aim by coordinate. The second path
  is counted THE SAME WAY: otherwise the report lies in its own favor — `bin.reel` and `speed`
  were clicked by hand in `nestparam` and `rate`, yet counted as gaps.

The report: names that the program publishes and that nobody touched with the mouse are
candidates for a live check. What turns RED (`--check`) are things about FALSEHOOD,
not about completeness, plus the DECLARED LIMIT of gaps:

1. a scenario aims at a name that the program no longer publishes — such a
   scenario checks thin air;
2. a spec refers to a live scenario that is not on disk — a “move”
   of a check to headless must leave the scenario of the spec with the new address, and
   not drop it;
3. a scenario has NO ENTRY POINT (`if __name__ == "__main__"`). Such a file
   runs and does NOTHING, and the run counts it as green: `select.py`
   lived that way since R.33 and was caught only by timing (0.0 s per scenario that
   must run for fifteen seconds);
4. a live scenario from the move registry (`MIGRATED`) remained on disk or its
   new address does not exist. Moving a check to headless is a CHANGE
   OF ADDRESS, not a loss: the registry names where each check went, and
   the guard checks that the named test exists in the sources;
5. untouched places became MORE than the declared limit (`UNTOUCHED_LIMIT`).
   The limit is not a target but a ratchet: a new place in the program is either clicked
   live, or the limit is moved deliberately, in one line and with an explanation.

    python3 scripts/live-coverage.py            # report
    python3 scripts/live-coverage.py --check    # report and verdict
"""
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src" / "apps" / "kt-app" / "src"
# KT_LIVE_DIR: a COPY of the scenario folder for the guard's own test (t804 adds a "returned"
# scenario there) — the real folder is shared with tests running in parallel (t808 globs it)
LIVE = Path(os.environ["KT_LIVE_DIR"]) if os.environ.get("KT_LIVE_DIR") else ROOT / "src" / "scripts" / "live"
SPECS = ROOT / "openspec"

NOTE = re.compile(r'field_probe::note\(\s*&?(?:format!\()?\s*"([^"]+)"')
# The probe name is often built by `format!` into a variable, and by a call to `note(...)`
# it cannot be seen. So the second source is taken from all the literals of the FORM
# OF THE PROBE NAME: “word.word[.word]”, where the insertions are written as “{}”/“{x}”.
SHAPE = re.compile(r'"([a-z][a-z0-9_]*(?:\.[a-z0-9_]*(?:\{[^"}]*\})?[a-z0-9_]*)+)"')
USE = re.compile(
    r'(?:click_probe|right_click_probe|double_click_probe|hover_probe|has_probe'
    r'|scroll_probe_into_view)\(\s*f?"([^"]+)"'
    # THE SECOND LAWFUL WAY OF AIMING: the scenario takes the probe’s rectangle from the
    # dump itself and aims by coordinate — so `nestparam` presses the composition card
    # in the bin, and `rate` drags the speed bar. Not counting it
    # means lying in its own favour: the place was clicked by hand, and the report calls
    # it a hole and sends people to write a second, identical scenario.
    r'|\[\s*"param"\s*\]\s*==\s*"([^"]+)"'
)
# THE THIRD WAY: the scenario does NOT know the probe name in advance and takes it from the
# dump by its prefix (`f["param"].startswith("auto.add.")` in `formula`) — the parameter
# for which a curve is created depends on the project. Such an aim covers
# the WHOLE family of names with this prefix.
USE_PREFIX = re.compile(r'\.startswith\(\s*"([^"]+)"')
# A click is an aim; `has_probe` can also be NEGATIVE (“the widget must not be
# here”), and in that case the name is legitimately absent from the program. So “aims at
# something nonexistent” is counted only for gestures.
USE_CLICK = re.compile(
    r'(?:click_probe|right_click_probe|double_click_probe|hover_probe'
    r'|scroll_probe_into_view)\(\s*f?"([^"]+)"'
)
SPEC_LIVE = re.compile(r"src/scripts/live/([a-z_0-9]+)\.py")
FILE_EXT = {"otio", "kt", "png", "json", "db", "rs", "py", "toml", "wav", "mp4", "jpg", "ktproj", "dll"}

# REGISTRY OF THE MOVE (R.68, task 5.2). The criterion is in the spec `ui-reach`: live
# what remains is a place never touched ONCE, a gesture hit, and a complex path from
# the owner’s claim; a check that AFTER a gesture already clicked in another
# place asserts only the state of the model or file, moves to
# headless. The registry is the proof that the CHECK moved, not that it vanished:
# the guard requires that the scenario file no longer exists AND that each
# named test exists in the sources.
MIGRATED = {
    "nest": (
        "roll up → enter → move → exit → save: all assertions "
        "about the model and .otio; gestures clicked in nestparam/nestshare/nestdead",
        ["t427", "t456", "t457", "t431"],
    ),
    "nestenter": (
        "Enter/Esc cascade and the strip inside an open reel: assertions about "
        "depth and rows; keyboard gesture clicked in nestdead/nestshare",
        ["t597", "t634", "t636"],
    ),
    "nestopen": (
        "expansion in place, edit from the strip, Esc: assertions about the document "
        "and what plays; double click on an event clicked in nestdead/audiofx",
        ["t570", "t571", "t572", "t573"],
    ),
    "nestzero": (
        "an event at zero inside the reel and a cursor that does not go there: "
        "assertions about the model; event drag clicked in select/level/corner",
        ["t637", "t637b"],
    ),
    "fxfield": (
        "the typed key value reaches the file: typing in the parameter field "
        "clicked live in nestparam (instance handles) and formula",
        ["t281"],
    ),
    "curvesrc": (
        "broken formula text does not spoil the model, the valid one is applied: gesture "
        "stayed live in formula (the “unlink” click moved there too)",
        ["t709", "t749"],
    ),
}

# LIMIT ON HOLES — a ratchet, not a target (R.68, task 5.3). Places that a
# person never reached with the mouse ONCE were 108 of 174 at the start of stage 5
# (after the report learned to count, as an aim, also the parsing of the dump and
# the prefix — 105). Task 5.3 closed twenty-three: track verbs from the
# menu of the strip and from its window (`trackverbs.py`), the raster, the frame rate and all
# four conform rules in the settings window (`conform.py`), the form of the
# “Geometry” node (`geomctl.py`), “Delete curve” and “Done” in the effects window
# (`formula.py`), the solo warning in the render window (`track.py`).
# The limit holds what has been achieved: a new place of the program either gets a live
# step, or the number is moved deliberately — in one line and with a reason in the report.
# 82 → 88 (landing screenshots, 2026-10-21): six probes added for
# site/screenshots.py — window frames `win.fx`, `win.morpher`, `win.render`,
# `win.render_progress`, the picker popup `fxreg.popup` and `render.start`.
# That script drives them with the mouse but is not a live-check scenario;
# `live/readme_shots.py` (readme-showcase, task 3.1) is to touch them and
# bring the limit back down.
# 88 → 84 (readme-showcase, 2026-10-21): `live/readme_shots.py` reaches the
# window frames, the picker and four probes it added itself (`tb.diag`,
# `win.diag`, `win.transition`, `fxreg.add.video`); `win.render_progress`
# and `render.start` stay untouched — the README does not show a render.
UNTOUCHED_LIMIT = 84


def as_pattern(name):
    """“titlew.preset.{i}” → a regex that matches “titlew.preset.2”."""
    return re.compile("^" + re.sub(r"\{[^}]*\}", ".*", re.escape(name)
                                   .replace(r"\{", "{").replace(r"\}", "}")) + "$")


def published():
    out = {}
    for p in sorted(SRC.glob("*.rs")):
        if p.name in ("reach_tests.rs", "layout_tests.rs"):
            continue
        text = p.read_text(encoding="utf-8")
        for m in NOTE.finditer(text):
            out.setdefault(m.group(1), p.name)
        for m in SHAPE.finditer(text):
            # file names and extensions also get into the form of the probe name
            if m.group(1).rsplit(".", 1)[-1] in FILE_EXT:
                continue
            out.setdefault(m.group(1), p.name)
    return out


def used(rx=USE):
    out = {}
    for p in sorted(LIVE.glob("*.py")):
        if p.name in ("kt_live.py", "worker.py"):
            continue
        text = p.read_text(encoding="utf-8")
        for m in rx.finditer(text):
            # an aim has two lawful kinds (the probe call and the parsing of the dump), and
            # which group fired is known only to the regex
            name = next((g for g in m.groups() if g), None)
            if name:
                out.setdefault(name, set()).add(p.stem)
        if rx is USE:
            # the prefix covers the whole family: “auto.add.” → “auto.add.{}”
            for m in USE_PREFIX.finditer(text):
                out.setdefault(m.group(1) + "{}", set()).add(p.stem)
    return out


def migration_faults():
    """Violations of the move registry: the scenario remained or the address is empty."""
    sources = [
        p
        for d in (ROOT / "src" / "apps", ROOT / "src" / "libs")
        for p in d.rglob("*.rs")
    ]
    text = "\n".join(p.read_text(encoding="utf-8", errors="ignore") for p in sources)
    bad = []
    for name, (_why, tests) in sorted(MIGRATED.items()):
        if (LIVE / f"{name}.py").exists():
            bad.append(
                f"scenario {name}.py is declared as moved to headless, but it is on disk: "
                "either remove it from the MIGRATED registry or delete the file"
            )
        for t in tests:
            # the address is matched EXACTLY (`fn t456_…`, not “starts with t456”):
            # otherwise deleting t456 would pass unnoticed while t456b is alive
            if not re.search(rf"\bfn {re.escape(t)}_", text):
                bad.append(
                    f"move {name}.py gave the test as its address {t}, which does not exist in the sources"
                )
    return bad


def report(brief=False):
    pub, use = published(), used()
    pats = {name: as_pattern(name) for name in pub}
    use_pats = {name: as_pattern(name) for name in use}

    untouched = sorted(
        name for name, pat in pats.items()
        if not any(pat.match(u) or use_pats[u].match(name) for u in use)
    )
    clicked = {n: as_pattern(n) for n in used(USE_CLICK)}
    phantom = sorted(
        name for name, pat in clicked.items()
        if not any(pat.match(p) or pats[p].match(name) for p in pub)
    )

    print(f"live coverage: the program publishes {len(pub)} places, "
          f"scenarios touch {len(pub) - len(untouched)}, "
          f"with the mouse never touched {len(untouched)} "
          f"(declared limit {UNTOUCHED_LIMIT})")
    if MIGRATED and not brief:
        print(f"moved to headless ({len(MIGRATED)} scenarios):")
        for name, (why, tests) in sorted(MIGRATED.items()):
            print(f"  {name}.py → {', '.join(tests)}: {why}")
    if untouched and not brief:
        print(f"NEVER touched with the mouse ({len(untouched)}):")
        by_place = {}
        for n in untouched:
            by_place.setdefault(n.split(".")[0], []).append(n)
        for place, names in sorted(by_place.items()):
            print(f"  {place}: {', '.join(names)}")
    no_entry = sorted(
        p.stem
        for p in LIVE.glob("*.py")
        if p.name not in ("kt_live.py", "worker.py", "balance.py")
        and not p.name.startswith("_")
        and "__main__" not in p.read_text(encoding="utf-8")
    )
    dead_specs = sorted(
        {n for f in SPECS.rglob("spec.md") if "archive" not in f.parts
         for n in SPEC_LIVE.findall(f.read_text(encoding="utf-8"))
         if not (LIVE / f"{n}.py").exists()}
    )
    return untouched, phantom, dead_specs, no_entry


def main(argv):
    untouched, phantom, dead_specs, no_entry = report(brief="--brief" in argv)
    if "--check" not in argv:
        return 0
    bad = []
    for n in phantom:
        bad.append(f"scenario aims at “{n}” which the program does not publish")
    for n in dead_specs:
        bad.append(f"spec refers to live scenario {n}.py which is not on disk")
    for n in no_entry:
        bad.append(
            f"scenario {n}.py has no entry point (`if __name__ == \"__main__\"`) — "
            "it runs, does nothing and counts as green"
        )
    bad.extend(migration_faults())
    if len(untouched) > UNTOUCHED_LIMIT:
        bad.append(
            f"places never touched with the mouse have become {len(untouched)} with "
            f"the declared limit {UNTOUCHED_LIMIT}: a new place needs a live "
            "step, or the limit is moved deliberately (UNTOUCHED_LIMIT)"
        )
    if bad:
        print("LIVE COVERAGE — RED:", file=sys.stderr)
        for b in bad:
            print("  " + b, file=sys.stderr)
        return 1
    print("live coverage: scenarios aim at existing places, "
          "specs — at existing scenarios")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

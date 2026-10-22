#!/usr/bin/env python3
"""Check and summary of the docs/features/ catalog (base of future features, R.36).

    python3 src/scripts/features-index.py --check    validation, non-zero exit on errors
    python3 src/scripts/features-index.py --table    markdown table of priorities to stdout

The script rewrites NOTHING: the table is pasted into the README by hand (rule 20
of the checklist — nothing grows by itself). The single source of truth about the list
of categories is the table “## Categories” in docs/features/README.md; here it is read,
not duplicated.

There are no dependencies: our frontmatter is a fixed subset of YAML
(a scalar or a flat list in square brackets), parsed in place.
"""

import datetime
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FEATURES = ROOT / "docs" / "features"
README = FEATURES / "README.md"
EDITORS = FEATURES / "_editors.md"

STATUSES = ["researched", "candidate", "accepted", "in-progress", "done",
            "rejected", "already-have"]
PRIORITIES = ["A", "B", "C"]
COSTS = ["S", "M", "L", "XL"]
WE_HAVE = ["yes", "no", "partial"]

REQUIRED_SECTIONS = ["Essence", "Who has it and how", "The trick behind it", "Pros", "Cons",
                     "What users say", "How it fits Kinotrain",
                     "Estimate", "Verdict", "Sources"]

REQUIRED_FIELDS = ["id", "title", "status", "priority", "cost", "category",
                   "editors", "we_have", "crates", "invariants", "ui_rules",
                   "round", "point", "tests", "depends_on", "checked"]

# “The trick behind it” shorter than this is almost certainly a retelling of a marketing list.
GIMMICK_MIN = 200
STALE_DAYS = 548  # 18 months

# The second wave (R.39, category help/). The conclusion “we need a reference” is known BEFORE
# the reconnaissance, so the verdict takes a form, and its promise — an address. Without this
# 33 notes would have written the same thing, and the wave would not have been worth reading.
HELP_CAT = "help"
VERDICT_SHAPES = ("TEST:", "CHECK:", "REJECTED:", "ALREADY-HAVE:", "DEFERRED:")
VERDICT_TAIL_MIN = 60
TEMPLATE = FEATURES / "_template.md"

# Third wave (R.41): the tracker is a source, not an axis. A note that declares trackers:
# must carry a COUNT (the share comparison of two trackers) and an ADDRESS (our code). “N bugs about proxies in Kdenlive”
# is a statistic; a finding begins where the number is pinned to src/.
TRACKERS = FEATURES / "_trackers.md"
SIGNAL_CAT = "resilience"

# Fourth wave (R.42, category collab/). The topic is the cloud features of other editors,
# and we have no network in any form: 0 matches for
# std::net|TcpStream|reqwest|hyper|tokio across all of src/ and not a single HTTP/TLS crate
# among the 505 packages in Cargo.lock. So the conclusion “there is no network, so we do not take it” is known
# BEFORE the reconnaissance and would have written itself forty-five times. The note must split
# the feature in two: what stays on ONE machine — and what exactly is paid for the second half.
SERVICES = FEATURES / "_services.md"
OFFLINE_CAT = "collab"
OFFLINE_SHAPES = ("REMAINS", "DEGENERATES", "DOES NOT REMAIN")
OFFLINE_TAIL_MIN = 80  # is counted WITHOUT an anchor: otherwise a long path picks up the minimum by itself
# Closed vocabulary of “what we lack beyond the socket”. The word “network” is not here
# DELIBERATELY: its absence is not a finding.
NET_COSTS = ("user account", "shared clock", "edit log", "access rights",
             "project-as-database", "cross-project storage", "external executor",
             "media address")
ANCHOR_RE = re.compile(r"^(src/[\w./-]+?)(?::\d+(?:-\d+)?|::[\w:]+)?$")

# Fifth wave (change bootstrap-specs-from-chronicles, 2026-09-21): project
# decisions moved into OpenSpec. “How it fits” must name the capability
# (`CAPABILITY: openspec/specs/<cap>/`, the directory exists and carries spec.md), and for
# FROZEN statuses the section is a link plus one line of substance (≤ 5 non-empty lines,
# no src/ paths required). For live statuses src/ paths are still mandatory.
SPECS = ROOT / "openspec" / "specs"
FROZEN_STATUSES = ("done", "accepted", "already-have", "rejected")
FROZEN_FITS_MAX_LINES = 5
CAPABILITY_RE = re.compile(r"^CAPABILITY:\s*(.+)$", re.M)


def parse_front(text):
    """Frontmatter → dict. Value: list [a, b], null, or a string."""
    if not text.startswith("---\n"):
        return None, text
    end = text.find("\n---\n", 4)
    if end < 0:
        return None, text
    front, body = text[4:end], text[end + 5:]
    out = {}
    for line in front.splitlines():
        line = line.split("  #")[0].rstrip()
        if not line or line.lstrip().startswith("#"):
            continue
        if ":" not in line:
            continue
        k, v = line.split(":", 1)
        k, v = k.strip(), v.strip()
        if v.startswith("[") and v.endswith("]"):
            inner = v[1:-1].strip()
            out[k] = [x.strip() for x in inner.split(",") if x.strip()] if inner else []
        elif v in ("null", "~", ""):
            out[k] = None
        else:
            out[k] = v
    return out, body


def sections(body):
    """{H2 heading: text under it}."""
    out, cur, buf = {}, None, []
    for line in body.splitlines():
        m = re.match(r"^##\s+(.+?)\s*$", line)
        if m:
            if cur is not None:
                out[cur] = "\n".join(buf).strip()
            cur, buf = m.group(1), []
        elif cur is not None:
            buf.append(line)
    if cur is not None:
        out[cur] = "\n".join(buf).strip()
    return out


def read_categories():
    """Categories from the README table: {directory: (code, plan)}."""
    text = README.read_text(encoding="utf-8")
    out = {}
    for m in re.finditer(r"^\|\s*`(\w+)/`\s*\|\s*([A-Z]+)\s*\|[^|]*\|\s*(\d+)\s*/\s*(\d+)\s*\|",
                         text, re.M):
        out[m.group(1)] = (m.group(2), int(m.group(3)), int(m.group(4)))
    return out


def read_editor_keys():
    # The point is legitimate: the key carries the version (blender-5.2, vscode-1.136, jetbrains-2026.2).
    return set(re.findall(r"^\|\s*`([a-z0-9.-]+)`\s*\|", EDITORS.read_text(encoding="utf-8"), re.M))


def read_tracker_keys():
    """Keys from the “## Registry” table in _trackers.md — one source of truth."""
    if not TRACKERS.exists():
        return set()
    return set(re.findall(r"^\|\s*`([a-z0-9-]+)`\s*\|\s*\w+\s*\|\s*`",
                          TRACKERS.read_text(encoding="utf-8"), re.M))


def load_notes():
    return sorted(p for p in FEATURES.rglob("*.md")
                  if p.name != "README.md" and not p.name.startswith("_"))


def read_service_keys():
    """Keys from the “## Registry” table in _services.md.

    A SEPARATE file, not a third table in _editors.md: read_editor_keys() takes any
    line of the file with a slug in the first cell, and `frame-io` would silently become a valid
    value of the editors: field — i.e. a service would land in the table where the verdict lives.
    """
    if not SERVICES.exists():
        return set()
    m = re.search(r"## Registry(.*?)(?=\n## |\Z)", SERVICES.read_text(encoding="utf-8"), re.S)
    return set(re.findall(r"^\|\s*`([a-z0-9.-]+)`\s*\|", m.group(1), re.M)) if m else set()


def anchor_ok(anchor):
    """Rule 8 mechanically: an address is code that exists TODAY.

    The first address check in the database is BY TREE, not by form: an anchor on
    src/libs/kt-collab/src/session.rs — is already an implementation scheme, and it will not build.
    """
    m = ANCHOR_RE.match(anchor.strip())
    return bool(m) and (ROOT / m.group(1)).exists()


def check():
    errors, notes = [], []
    cats = read_categories()
    editor_keys = read_editor_keys()
    seen_ids = {}
    help_promises = {}
    signal_filters = {}
    service_keys = read_service_keys()
    offline_anchors = {}
    offline_shapes_seen = {}
    tracker_keys = read_tracker_keys()
    today = datetime.date.today()

    if not cats:
        errors.append("README.md: the “## Categories” table was not parsed")

    for path in load_notes():
        rel = path.relative_to(ROOT)
        def bad(msg):
            errors.append(f"{rel}: {msg}")

        front, body = parse_front(path.read_text(encoding="utf-8"))
        if front is None:
            bad("no YAML frontmatter")
            continue

        for f in REQUIRED_FIELDS:
            if f not in front:
                bad(f"no field {f}")

        fid = front.get("id", "")
        # path: docs/features/<category>/<subcategory>/<file>.md
        parts = path.relative_to(FEATURES).parts
        if len(parts) != 3:
            bad("file is not in the category/subcategory/file.md layout")
        else:
            cat = parts[0]
            if cat not in cats:
                bad(f"category “{cat}” is not declared in README")
            else:
                code = cats[cat][0]
                if not fid.startswith(f"F-{code}-"):
                    bad(f"id “{fid}” does not start with F-{code}- (category {cat})")
            if front.get("category") != f"{parts[0]}/{parts[1]}":
                bad(f"category field “{front.get('category')}” ≠ path {parts[0]}/{parts[1]}")

        if not re.match(r"^F-[A-Z]+-\d{3}$", fid or ""):
            bad(f"id “{fid}” is not in the F-CODE-NNN format")
        elif fid in seen_ids:
            bad(f"id {fid} is already taken: {seen_ids[fid]}")
        else:
            seen_ids[fid] = str(rel)

        m = re.search(r"^#\s+(\S+)\s+—\s+(.+?)\s*$", body, re.M)
        if not m:
            bad("no heading “# F-CODE-NNN — Title”")
        else:
            if m.group(1) != fid:
                bad(f"id in the heading ({m.group(1)}) ≠ id in the header ({fid})")
            if front.get("title") and m.group(2).strip() != str(front["title"]).strip():
                bad("title in the header ≠ name in the heading")

        for field, allowed in (("status", STATUSES), ("priority", PRIORITIES),
                               ("cost", COSTS), ("we_have", WE_HAVE)):
            v = front.get(field)
            if v is not None and v not in allowed:
                bad(f"{field}: “{v}” outside the vocabulary {allowed}")

        for e in front.get("editors") or []:
            if e not in editor_keys:
                bad(f"editor “{e}” is not declared in _editors.md")

        sec = sections(body)
        for s in REQUIRED_SECTIONS:
            if s not in sec:
                bad(f"no section “{s}”")
            elif not sec[s].strip():
                bad(f"section “{s}” is empty")

        # Three checks against the database turning into a wikipedia.
        gimmick = sec.get("The trick behind it", "")
        if len(gimmick) < GIMMICK_MIN:
            bad(f"“The trick behind it” is shorter than {GIMMICK_MIN} characters ({len(gimmick)}) — "
                "this is a retelling, not a finding")
        feedback = sec.get("What users say", "")
        has_quote = re.search(r"^>", feedback, re.M) and "http" in feedback
        # An honest gap is better than an invented quote: the site may be unavailable.
        gap = re.search(r"NO QUOTES:\s*(.{80,})", feedback, re.S)
        if not (has_quote or gap):
            bad("“What users say”: a verbatim quote is needed (a line with >) "
                "and a link, or a note “NO QUOTES:” with an explanation of what was searched for")
        fits = sec.get("How it fits Kinotrain", "")
        status = front.get("status")
        cap_lines = CAPABILITY_RE.findall(fits)
        for cl in cap_lines:
            for cap in re.findall(r"openspec/specs/([\w-]+)/", cl):
                if not (SPECS / cap / "spec.md").is_file():
                    bad(f"“CAPABILITY:” points to openspec/specs/{cap}/ — "
                        "no such capability (or it has no spec.md)")
            if "openspec/specs/" not in cl:
                bad("“CAPABILITY:” without a path openspec/specs/<cap>/")
        if status in FROZEN_STATUSES:
            if len(cap_lines) != 1:
                bad("frozen status: “How it fits” must carry exactly one "
                    "line “CAPABILITY: openspec/specs/<cap>/” — the decision lives there")
            n_lines = sum(1 for l in fits.splitlines() if l.strip())
            if n_lines > FROZEN_FITS_MAX_LINES:
                bad(f"frozen status: “How it fits” — {n_lines} lines at "
                    f"a limit of {FROZEN_FITS_MAX_LINES}; the design is in "
                    "openspec/specs/<cap>/design.md, not here")
        elif "src/" not in fits:
            bad("“How it fits Kinotrain”: there is not a single path of the form src/… — "
                "the section was written blind")

        if status == "rejected" and len(sec.get("Verdict", "")) < 40:
            bad("status “rejected” without a clear reason in “Verdict”")
        if status == "done":
            if not front.get("point"):
                bad("status “done” without point")
            if not front.get("tests"):
                bad("status “done” without tests")
        if status in ("candidate", "accepted"):
            try:
                d = datetime.date.fromisoformat(str(front.get("checked")))
                if (today - d).days > STALE_DAYS:
                    bad(f"checked={d} older than 18 months with status “{status}” — recheck")
            except ValueError:
                bad(f"checked=“{front.get('checked')}” is not a date in the YYYY-MM-DD format")

        # --- second wave: category help/ (R.39) --------------------------
        if len(parts) == 3 and parts[0] == HELP_CAT:
            verdict = sec.get("Verdict", "")
            shaped = [l.strip() for l in verdict.splitlines()
                      if l.strip().startswith(VERDICT_SHAPES)]
            if not shaped:
                bad("“Verdict”: a line of the form TEST:/CHECK:/REJECTED:/"
                    "ALREADY-HAVE:/DEFERRED: — “we need a reference” does not count as a verdict")
            else:
                head = shaped[0]
                kind, tail = head.split(":", 1)
                if len(tail.strip()) < VERDICT_TAIL_MIN:
                    bad(f"“Verdict”: after “{kind}:” less than {VERDICT_TAIL_MIN} characters")
                m = re.search(r"\bt\d+[a-z]?\b"
                              r"|(?:src|site|docs)/[\w./-]+"
                              r"|--check\b[\w -]*"
                              r"|\b\w+::\w+\b", head)
                if not m:
                    bad("“Verdict”: the promise is not addressable — name a test, a path "
                        "or a --check clause")
                else:
                    help_promises.setdefault(m.group(0), []).append(str(rel))

            # The dispute about pop-up hints is settled (R.39), but only by looking
            # the guard in the eye: t83 holds the ban, rule 10 is its wording.
            if "tooltip_delay" in body or "Order::Tooltip" in body:
                if "t83" not in fits:
                    bad("a dispute about pop-up hints without analysis of t83 in “How it fits” "
                        "— the regression guard is not named")
                if 10 not in [int(x) for x in (front.get("ui_rules") or [])
                              if str(x).isdigit()]:
                    bad("a dispute about pop-up hints without ui_rules: [10] in the header")

        # --- third wave: trackers as a source (R.41) ------------------
        trk = front.get("trackers") or []
        if len(parts) == 3 and parts[0] == SIGNAL_CAT and not trk:
            bad("category resilience/ was created for countable findings: "
                "trackers: field must not be empty")
        if trk:
            for t in trk:
                if t not in tracker_keys:
                    bad(f"tracker “{t}” is not declared in _trackers.md")

            sig = [l.strip() for l in feedback.splitlines()
                   if l.strip().startswith("SIGNAL:")]
            if not sig:
                bad("trackers: is declared, but “What users say” has no line "
                    "“SIGNAL:” — statistics without a comparison do not count as a finding")
            else:
                head = sig[0]
                if len(re.findall(r"\d", head)) < 2 or "%" not in head:
                    bad("“SIGNAL:” must carry two numbers and a share in percent "
                        "(a count of one tracker against the share of another)")
                f = re.search(r"`([^`]+)`", head)
                if not f:
                    bad("“SIGNAL:” without an exact filter in backticks — "
                        "the number cannot be reproduced")
                else:
                    signal_filters.setdefault(f.group(1), []).append(str(rel))

            addr = [l.strip() for l in fits.splitlines()
                    if l.strip().startswith("ADDRESS:")]
            if not addr:
                bad("trackers: is declared, but “How it fits” has no line “ADDRESS:” — "
                    "a foreign pain is not tied to our code")
            elif "src/" not in addr[0]:
                bad("“ADDRESS:” without a src/… path — the tie to our code is not named")

        # --- fourth wave: category collab/ (R.42) ------------------
        if len(parts) == 3 and parts[0] == OFFLINE_CAT:
            svc = front.get("services") or []
            if not svc:
                bad("category collab/ was created for cloud capabilities: the field "
                    "services: must not be empty — the note must name "
                    "what exactly stands on the other side of the network")
            for sv in svc:
                if sv not in service_keys:
                    bad(f"service “{sv}” is not declared in _services.md")

            off = [l.strip() for l in fits.splitlines()
                   if l.strip().startswith("OFFLINE:")]
            if not off:
                bad("“How it fits”: there is no line “OFFLINE:” — “we have no network, "
                    "so we do not take it” does not count as the conclusion of the wave")
            else:
                tail = off[0].split(":", 1)[1].strip()
                shape = next((x for x in OFFLINE_SHAPES if tail.startswith(x)), None)
                if shape is None:
                    bad("“OFFLINE:” without a marker — needs REMAINS / DEGENERATES / "
                        "DOES NOT REMAIN as the first word")
                else:
                    offline_shapes_seen.setdefault(shape, []).append(str(rel))
                    a = re.search(r"`([^`]+)`", tail)
                    if not a:
                        bad("“OFFLINE:” without an anchor in backticks — the remainder "
                            "is not addressable (the anchor is mandatory also in the DOES NOT REMAIN branch: "
                            "there it points to the place where the absence is visible)")
                    elif not anchor_ok(a.group(1)):
                        bad(f"“OFFLINE:” anchor “{a.group(1)}” does not point to "
                            "an existing file src/… — the note designs "
                            "the implementation (rule 8) instead of describing our code")
                    else:
                        offline_anchors.setdefault(a.group(1), []).append(str(rel))
                    prose = re.sub(r"`[^`]*`", "", tail[len(shape):]).strip(" —–-:")
                    if len(prose) < OFFLINE_TAIL_MIN:
                        bad(f"“OFFLINE: {shape}” — prose {len(prose)} characters at "
                            f"a minimum of {OFFLINE_TAIL_MIN}: it is not said WHAT exactly "
                            "remains (or where it is visible that nothing remains)")

            price = [l.strip() for l in fits.splitlines()
                     if l.strip().startswith("NETWORK COST:")]
            if not price:
                bad("“How it fits”: there is no line “NETWORK COST:” — it is not named what "
                    "exactly is paid on that side")
            else:
                head = price[0]
                inv = [int(x) for x in re.findall(r"invariants?\s+(\d{1,2})", head)]
                named = [c for c in NET_COSTS if c in head]
                if not inv and not named:
                    bad("“NETWORK COST:” must name invariant N or a mechanism from "
                        f"the vocabulary: {', '.join(NET_COSTS)}. The word “network” is not in the vocabulary "
                        "no: its absence is known without a note")
                declared = [int(x) for x in (front.get("invariants") or [])
                            if str(x).isdigit()]
                for n in inv:
                    if not 1 <= n <= 12:
                        bad(f"“NETWORK COST:”: invariant {n} does not exist; there are twelve of them")
                    elif n not in declared:
                        bad(f"“NETWORK COST:” names invariant {n}, but the header "
                            f"invariants: {declared} — the prose and the header disagree")

        notes.append((front, rel))

    # A promise repeated twice is a duplicated conclusion.
    for token, where in sorted(help_promises.items()):
        if len(where) > 1:
            errors.append(f"help/: the promise “{token}” stands FIRST in {len(where)} "
                          f"notes ({', '.join(where)}) — the conclusion is duplicated")

    # The same filter in two notes is one finding counted twice.
    # A remainder named twice is the same remainder.
    for a, where in sorted(offline_anchors.items()):
        if len(where) > 1:
            errors.append(f"OFFLINE: anchor “{a}” stands in {len(where)} notes "
                          f"({', '.join(where)}) — the remainder is doubled")

    # Category-level balancer. Where everything STAYS — an invented local core;
    # where nothing stays — exactly the defect the rule was made for.
    n_collab = sum(1 for f, _ in notes
                   if str(f.get("category", "")).startswith(OFFLINE_CAT + "/"))
    if n_collab >= 10:
        for sh in OFFLINE_SHAPES:
            if sh not in offline_shapes_seen:
                errors.append(f"collab/: {n_collab} notes and not one with a marker "
                              f"“{sh}” — the classifier has degenerated, check the axis")

    for f, where in sorted(signal_filters.items()):
        if len(where) > 1:
            errors.append(f"SIGNAL: filter “{f}” is repeated in {len(where)} notes "
                          f"({', '.join(where)}) — the finding is doubled")

    # _template.md is excluded from load_notes() (name starts with “_”), so its slugs
    # once went stale silently and would break every copied note.
    tfront, _ = parse_front(TEMPLATE.read_text(encoding="utf-8"))
    for e in (tfront or {}).get("editors") or []:
        if e not in editor_keys:
            errors.append(f"_template.md: editor “{e}” is not declared in _editors.md")
    for sv in (tfront or {}).get("services") or []:
        if sv not in service_keys:
            errors.append(f"_template.md: service “{sv}” is not declared in _services.md")

    # The coverage table in the README must not lie.
    actual = {}
    for front, rel in notes:
        actual[Path(rel).parts[2]] = actual.get(Path(rel).parts[2], 0) + 1
    for cat, (code, written, planned) in cats.items():
        got = actual.get(cat, 0)
        if got != written:
            errors.append(f"README: at {cat}/ written “{written} / {planned}”, "
                          f"but there are files {got}")

    print(f"{len(notes)} notes, {len(cats)} categories, {len(errors)} errors")
    for e in errors:
        print(f"  ERROR  {e}")
    return 1 if errors else 0


def table():
    rows = []
    for path in load_notes():
        front, _ = parse_front(path.read_text(encoding="utf-8"))
        if not front:
            continue
        rows.append((front.get("priority", "C"), front.get("cost", "M"),
                     front.get("id", "?"), front.get("title", "?"),
                     front.get("status", "?"), front.get("we_have", "?"),
                     str(path.relative_to(FEATURES))))
    rows.sort(key=lambda r: (PRIORITIES.index(r[0]) if r[0] in PRIORITIES else 9,
                             COSTS.index(r[1]) if r[1] in COSTS else 9, r[2]))
    print("| ID | Feature | Status | Prio | Cost | We have | Note |")
    print("|---|---|---|---|---|---|---|")
    for pr, cost, fid, title, status, we, rel in rows:
        print(f"| {fid} | {title} | {status} | {pr} | {cost} | {we} | `{rel}` |")
    return 0


if __name__ == "__main__":
    arg = sys.argv[1] if len(sys.argv) > 1 else "--check"
    if arg == "--check":
        sys.exit(check())
    if arg == "--table":
        sys.exit(table())
    print(__doc__)
    sys.exit(2)

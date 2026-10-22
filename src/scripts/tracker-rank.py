#!/usr/bin/env python3
"""Analysis of exported bug trackers (R.41, third reconnaissance wave).

    python3 src/scripts/tracker-rank.py                    rank the topics
    python3 src/scripts/tracker-rank.py --theme proxy      top issues of the topic
    python3 src/scripts/tracker-rank.py --wishes           requests only (wishlist)
    python3 src/scripts/tracker-rank.py --rejected         what was declined to do
    python3 src/scripts/tracker-rank.py --neglect          open for years and significant
    python3 src/scripts/tracker-rank.py --unmatched        what the lexicon did not cover
    python3 src/scripts/tracker-rank.py --query 'regex' [--tracker K]
    python3 src/scripts/tracker-rank.py --table            markdown for _trackers.md
    python3 src/scripts/tracker-rank.py --check            dumps in place and intact

The topic lexicon and the registry are read from `docs/features/_trackers.md` and are NOT duplicated here —
the same technique as reading categories from README in features-index.py.

Why it is counted this way:

* **Shares, not counts.** Trackers differ in size and culture: Blender comments three times as
  actively as Flowblade. An issue’s weight is divided by the MEDIAN of its own tracker (not the mean —
  the distributions are heavy-tailed), and topics are compared by share within their own tracker.
* **A duplicate weighs the most** (×5): a person hit it independently, and the triager confirmed
  it is the same thing. A comment is cheap and often triage noise, so ×1.
* **“Closed” ≠ “fixed”.** In Kdenlive `WORKSFORME` is 24.4 % of all issues. Therefore everywhere
  `fixed` is computed as the share of REAL fixes among closed issues, not the share of closed ones.
* **FFmpeg trac is outside the shared ranking**: Trac has no votes and no comment counter, and the
  sample is open only — the denominator is not comparable.

Exit codes: 0 — success, 1 — check failed, 2 — unknown argument.
"""

import hashlib
import json
import os
import re
import statistics
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TRACKERS = ROOT / "docs" / "features" / "_trackers.md"
CACHE = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "kinetren" / "trackers"

# The corpus is read from the “Corpus” column of the registry, NOT from the code: before, this held
# OUT_OF_RANK = {"ffmpeg-trac"}, which is exactly the duplication of the registry in the script
# that _trackers.md forbids itself. The corpora are not compared with each other —
# they have different denominators (R.42).
DEFAULT_CORPUS = "nle"

WISH_LABELS = re.compile(r"enhancement|feature|idea|rfe|type/design|type/to do", re.I)
BUG_LABELS = re.compile(r"\bbug\b|crash|regression|type/bug|defect", re.I)
WISH_TITLE = re.compile(r"^\s*(add|support|allow|please|would be nice|feature request|\[feature\])", re.I)
BUG_TITLE = re.compile(r"crash|freeze|hang|broken|not work|corrupt|error|fail", re.I)


def read_lexicon():
    """[(topic, compiled regexp)] from the “## Lexicon of topics” table."""
    text = TRACKERS.read_text(encoding="utf-8")
    m = re.search(r"## Lexicon of topics(.*?)(?=\n## )", text, re.S)
    if not m:
        sys.exit("_trackers.md has no “## Lexicon of topics” section")
    out = []
    for row in re.finditer(r"^\|\s*([^|`]+?)\s*\|\s*`([^`]+)`\s*\|", m.group(1), re.M):
        name = row.group(1).strip()
        if name.lower() in ("topic",) or set(name) <= set("-: "):
            continue
        # In the markdown table the pipe is escaped (\|), while in the regex it is a LITERAL
        # pipe, not an alternative. We remove the escaping when reading.
        out.append((name, re.compile(row.group(2).replace(r"\|", "|"), re.I)))
    return out


def read_corpora():
    """{key: corpus} from the “Corpus” column of the registry.

    Parsed by the HEADER, not by position: the column was appended last, and
    read_registry()/read_tracker_keys() anchor on the first three — their order
    must not be changed.
    """
    out, idx = {}, None
    for line in TRACKERS.read_text(encoding="utf-8").splitlines():
        if not line.startswith("|"):
            idx = None
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if "Key" in cells and "Corpus" in cells:
            idx = cells.index("Corpus")
            continue
        if idx is None or len(cells) <= idx:
            continue
        k = re.match(r"^`([a-z0-9-]+)`$", cells[0])
        if k:
            out[k.group(1)] = cells[idx].strip("` ")
    return out


def load(corpus=None):
    """{key: [records]} from the cache; corpus=None — all dumps without parsing."""
    data = {}
    if not CACHE.exists():
        return data
    cor = read_corpora()
    for p in sorted(CACHE.glob("*.jsonl")):
        if corpus is not None and cor.get(p.stem, DEFAULT_CORPUS) != corpus:
            continue
        rows = []
        with open(p, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line:
                    rows.append(json.loads(line))
        if rows:
            data[p.stem] = rows
    return data


def kind(r):
    """'wish' | 'bug' | '?' — by the declared field; the heuristic only as a last resort."""
    sev = (r.get("severity_raw") or "").lower()
    if sev == "wishlist":
        return "wish"
    if sev == "enhancement":
        return "wish"
    if sev == "defect":
        return "bug"
    labs = " ".join(r.get("labels") or [])
    if WISH_LABELS.search(labs):
        return "wish"
    if BUG_LABELS.search(labs):
        return "bug"
    t = r.get("title") or ""
    if WISH_TITLE.search(t):
        return "wish?"
    if BUG_TITLE.search(t):
        return "bug?"
    return "?"


def is_open(r):
    st = (r.get("state") or "").lower()
    return st in ("open", "opened", "unconfirmed", "confirmed", "assigned", "reopened", "new")


def is_fixed(r):
    """Closed by a REAL fix (not WORKSFORME/not_planned/duplicate)."""
    if is_open(r):
        return False
    cr = (r.get("closed_reason") or "").upper()
    if cr in ("FIXED", "COMPLETED"):
        return True
    if cr in ("WORKSFORME", "WONTFIX", "INVALID", "DUPLICATE", "UPSTREAM",
              "MOVED", "NOT_PLANNED", "UNMAINTAINED", "REPORTED_UPSTREAM"):
        return False
    return cr == ""  # github/gitea/gitlab: closed without a “won't do” mark → counted as a fix


def rejected(r):
    cr = (r.get("closed_reason") or "").upper()
    return cr in ("WONTFIX", "INVALID", "NOT_PLANNED", "UNMAINTAINED")


def weights(data):
    """Weight normalization by the median of its own tracker + count of incoming duplicates."""
    dupes = {}
    for k, rows in data.items():
        cnt = {}
        for r in rows:
            d = r.get("dupe_of")
            if d:
                cnt[d] = cnt.get(d, 0) + 1
        dupes[k] = cnt
    med = {}
    for k, rows in data.items():
        raw = [_raw(r, dupes[k]) for r in rows]
        med[k] = statistics.median(raw) or 1.0
    for k, rows in data.items():
        for r in rows:
            r["_e"] = _raw(r, dupes[k]) / med[k]
            r["_dupes"] = dupes[k].get(r.get("id"), 0)
    return data


def _raw(r, dupes):
    return (1 + (r.get("comments") or 0) + 2 * (r.get("reactions") or 0)
            + 3 * (r.get("upvotes") or 0) + (r.get("cc") or 0)
            + 5 * dupes.get(r.get("id"), 0))


def years_open(r):
    if not is_open(r) or not r.get("created"):
        return 0.0
    try:
        c = datetime.fromisoformat(str(r["created"]).replace("Z", "+00:00"))
        return max(0.0, (datetime.now(timezone.utc) - c).days / 365.25)
    except ValueError:
        return 0.0


def match(r, rx):
    hay = " ".join([r.get("title") or "", " ".join(r.get("labels") or []),
                    r.get("component") or ""])
    return bool(rx.search(hay))


def themes_table(data, lex, only_wish=False):
    rank = list(data)  # the corpus is already filtered in load()
    rows = []
    for name, rx in lex:
        per, tot, wish, bug, opn, fixed, closed, dup, w = {}, 0, 0, 0, 0, 0, 0, 0, 0.0
        for k in rank:
            hit = [r for r in data[k] if match(r, rx)]
            if only_wish:
                hit = [r for r in hit if kind(r).startswith("wish")]
            per[k] = len(hit) / max(1, len(data[k]))
            tot += len(hit)
            for r in hit:
                kd = kind(r)
                wish += kd.startswith("wish")
                bug += kd.startswith("bug")
                opn += is_open(r)
                dup += r["_dupes"]
                w += r["_e"]
                if not is_open(r):
                    closed += 1
                    fixed += is_fixed(r)
        if not tot:
            continue
        shares = [v for v in per.values() if v > 0]
        rows.append(dict(
            theme=name, n=tot, wish=wish, bug=bug, open=opn,
            fixed=(fixed / closed if closed else 0.0), dup=dup, w=w,
            universal=min(shares) if len(shares) >= 4 else 0.0,
            gap=(max(shares) / statistics.median(shares)) if len(shares) >= 3 else 0.0,
            per=per))
    rows.sort(key=lambda d: -d["w"])
    return rows, rank


def coverage(data, lex):
    rank = list(data)  # the corpus is already filtered in load()
    tot = sum(len(data[k]) for k in rank)
    hit = 0
    for k in rank:
        for r in data[k]:
            if any(match(r, rx) for _, rx in lex):
                hit += 1
    return hit, tot


def main():
    args = sys.argv[1:]
    # --corpus is taken out BEFORE the mode is chosen: otherwise, put first, it would itself
    # be taken for a mode and the help would be printed (R.42).
    corpus = DEFAULT_CORPUS
    if "--corpus" in args:
        i = args.index("--corpus")
        corpus = args[i + 1] if i + 1 < len(args) else DEFAULT_CORPUS
        del args[i:i + 2]
    mode = args[0] if args else "--themes"
    lex = read_lexicon()
    raw_all = load(None)
    data = weights(load(None if mode == "--check" else corpus))
    if not data:
        known = sorted(set(read_corpora().values())) or ["(there is no “Corpus” column in the registry)"]
        sys.exit(f"in corpus “{corpus}” has no dumps; corpora present: {', '.join(known)}")

    if mode == "--check":
        cor = read_corpora()
        print(f"{len(data)} dumps, {sum(len(v) for v in data.values())} records, "
              f"{len(lex)} topics in the lexicon")
        bad = 0
        for k in sorted(data):
            p = CACHE / f"{k}.jsonl"
            h = hashlib.sha256(p.read_bytes()).hexdigest()[:16]
            rows = raw_all.get(k, [])
            empty = sum(1 for r in rows if not (r.get("title") or "").strip())
            dup = len(rows) - len({r.get("id") for r in rows})
            flag = ""
            # The ffmpeg-trac dump was 3095 records with id=None and an empty title and passed
            # silently. A check that does not exist is not a check (R.42).
            if rows and empty / len(rows) > 0.05:
                flag += f"  EMPTY {100*empty/len(rows):.0f}%"; bad += 1
            if dup:
                flag += f"  DUPLICATES {dup}"; bad += 1
            print(f"  {k:<14} {len(rows):>6} records  {cor.get(k,'?'):<8}"
                  f"sha256:{h}{flag}")
        if bad:
            print(f"\ndumps with defects: {bad} — re-export "
                  "(tracker-fetch.py will delete state and start over)")
        return 1 if bad else 0

    if mode in ("--themes", "--wishes", "--table"):
        rows, rank = themes_table(data, lex, only_wish=(mode == "--wishes"))
        hit, tot = coverage(data, lex)
        print(f"# corpus “{corpus}”: {len(rank)} trackers ({', '.join(rank)})")
        print(f"# lexicon coverage: {hit}/{tot} = {100*hit/max(1,tot):.1f}%"
              + ("  (requests only)" if mode == "--wishes" else ""))
        print()
        print("| Topic | n | requests | complaints | open | fixes | dups | weight | universal | gap |")
        print("|---|---|---|---|---|---|---|---|---|---|")
        for d in rows:
            print(f"| {d['theme']} | {d['n']} | {d['wish']} | {d['bug']} | {d['open']} "
                  f"| {d['fixed']:.2f} | {d['dup']} | {d['w']:.0f} "
                  f"| {100*d['universal']:.1f}% | {d['gap']:.1f} |")
        return 0

    if mode == "--theme":
        rx = dict(lex).get(args[1]) if len(args) > 1 else None
        if rx is None:
            sys.exit(f"no topic “{args[1] if len(args)>1 else ''}”; present: "
                     + ", ".join(n for n, _ in lex))
        hit = [r for rows in data.values() for r in rows if match(r, rx)]
        hit.sort(key=lambda r: -r["_e"])
        for r in hit[:40]:
            print(f"{r['_e']:6.1f}  {r['tracker']:<13} {kind(r):<6} "
                  f"{'open' if is_open(r) else 'closed':<5} {(r.get('created') or '')[:10]}  "
                  f"{(r.get('title') or '')[:80]}")
            print(f"        {r.get('url')}")
        return 0

    if mode == "--rejected":
        hit = [r for rows in data.values() for r in rows if rejected(r)]
        hit.sort(key=lambda r: -r["_e"])
        print(f"rejected by maintainers: {len(hit)}")
        for r in hit[:40]:
            print(f"{r['_e']:6.1f}  {r['tracker']:<13} {(r.get('closed_reason') or ''):<12} "
                  f"{(r.get('title') or '')[:70]}\n        {r.get('url')}")
        return 0

    if mode == "--neglect":
        hit = [r for rows in data.values() for r in rows if is_open(r)]
        for r in hit:
            r["_n"] = years_open(r) * r["_e"]
        hit.sort(key=lambda r: -r["_n"])
        for r in hit[:40]:
            print(f"{r['_n']:7.1f}  {years_open(r):4.1f} years  {r['tracker']:<13} "
                  f"{kind(r):<6} {(r.get('title') or '')[:66]}\n        {r.get('url')}")
        return 0

    if mode == "--unmatched":
        rank = list(data)
        un = [r for k in rank for r in data[k]
              if not any(match(r, rx) for _, rx in lex)]
        hit, tot = coverage(data, lex)
        print(f"coverage {100*hit/max(1,tot):.1f}%, uncovered {len(un)}")
        big = {}
        for r in un:
            ws = re.findall(r"[a-z]{4,}", (r.get("title") or "").lower())
            for a, b in zip(ws, ws[1:]):
                big[f"{a} {b}"] = big.get(f"{a} {b}", 0) + 1
        for k, v in sorted(big.items(), key=lambda x: -x[1])[:50]:
            print(f"  {v:>4}  {k}")
        return 0

    if mode == "--query":
        rx = re.compile(args[1], re.I)
        only = args[args.index("--tracker") + 1] if "--tracker" in args else None
        for k in sorted(data):
            if only and k != only:
                continue
            hit = [r for r in data[k] if match(r, rx)]
            if hit:
                print(f"{k:<14} {len(hit):>5} of {len(data[k]):>5} = "
                      f"{100*len(hit)/len(data[k]):.1f}%")
        allhit = [r for k, rows in data.items() if not only or k == only
                  for r in rows if match(r, rx)]
        allhit.sort(key=lambda r: -r["_e"])
        print("  examples:")
        for r in allhit[:5]:
            print(f"    {r['tracker']} #{r['id']} {(r.get('title') or '')[:66]}\n"
                  f"      {r.get('url')}")
        return 0

    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())

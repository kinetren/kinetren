#!/usr/bin/env python3
"""Dump of the bug trackers of open editors (R.41, third reconnaissance wave).

    python3 src/scripts/tracker-fetch.py                 what is declared and what is already there
    python3 src/scripts/tracker-fetch.py --fetch mlt      resume one tracker
    python3 src/scripts/tracker-fetch.py --all            resume all of them in turn
    python3 src/scripts/tracker-fetch.py --all --budget 300
    python3 src/scripts/tracker-fetch.py --pin            registry rows for _trackers.md

The tracker registry is read from `docs/features/_trackers.md` and is NOT duplicated here —
the same trick as reading the categories from the README in features-index.py.

Why it is done this way and not otherwise:

* **Resumption by CURSOR, not by page number.** GitHub does not return `rel="last"`,
  the total number of pages is not known in advance. Plus, with the default sort (created desc)
  a new issue shifts the pagination and the dump loses records. So wherever possible —
  `sort=created&direction=asc`: new items are appended to the tail, and the state keeps the URL
  of the next page.
* **ETag.** A 304 response from GitHub does not spend the limit, so a repeated run is almost
  free. The ETag lies in the state file next to the cursor.
* **The limit is read from the response**, not guessed: at `x-ratelimit-remaining <= 1` we sleep until
  `x-ratelimit-reset`. Without a token GitHub gives 60 requests per hour.
* **Dumps outside the repository** — ~150 MB, and there is no license for mass distribution of issue texts
  from any source, and no test needs them offline. See the “Licenses” section
  in `_trackers.md`.

TOKEN. It is read from the `GITHUB_TOKEN` environment variable or from `.env` in the project root (the file is covered by
`.gitignore`). The script NEVER prints, logs or saves it: the output shows only
“token: present/absent”. Without a token everything works, just more slowly.

Return codes: 0 — success, 1 — something not fully downloaded, 2 — unknown argument.
"""

import csv
import hashlib
import io
import json
import os
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REGISTRY = ROOT / "docs" / "features" / "_trackers.md"
CACHE = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "kinetren" / "trackers"

UA = "kinetren-features-research (+docs/features/_trackers.md)"
BODY_MAX = 4000
DEFAULT_BUDGET = 55  # below the hourly limit of 60 for GitHub without a token


def token():
    """GITHUB_TOKEN from the environment or from .env. The value is not given out."""
    t = os.environ.get("GITHUB_TOKEN", "").strip()
    if t:
        return t
    env = ROOT / ".env"
    if env.exists():
        for line in env.read_text(encoding="utf-8", errors="replace").splitlines():
            line = line.strip()
            if line.startswith("GITHUB_TOKEN"):
                _, _, v = line.partition("=")
                return v.strip().strip('"').strip("'")
    return ""


def read_registry():
    """{key: {engine, url}} from the table “## Registry” in _trackers.md."""
    text = REGISTRY.read_text(encoding="utf-8")
    out = {}
    for m in re.finditer(
        r"^\|\s*`([a-z0-9-]+)`\s*\|\s*(\w+)\s*\|\s*`([^`]+)`\s*\|", text, re.M
    ):
        out[m.group(1)] = {"engine": m.group(2), "url": m.group(3)}
    return out


class Budget:
    def __init__(self, n):
        self.left = n
        self.used = 0

    def take(self):
        if self.left <= 0:
            return False
        self.left -= 1
        self.used += 1
        return True


def get(url, headers=None, budget=None, tries=4):
    """GET respecting the limits. Returns (code, body, headers).

    A network failure is not a reason to lose the dump: a timeout and a broken connection are
    retried with a growing pause. A single unhandled TimeoutError already
    brought the run down midway through pitivi (R.41).
    """
    if budget is not None and not budget.take():
        return None, None, None
    req = urllib.request.Request(url, headers={"User-Agent": UA, **(headers or {})})
    for attempt in range(tries):
        try:
            return _once(req)
        except urllib.error.HTTPError:
            raise
        except (TimeoutError, OSError) as e:
            if attempt == tries - 1:
                print(f"    the network does not respond after {tries} attempts: {e}", flush=True)
                return None, None, None
            wait = 5 * (attempt + 1)
            print(f"    network failure ({e}), retry after {wait} s", flush=True)
            time.sleep(wait)


def _once(req):
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            body = r.read()
            hdrs = {k.lower(): v for k, v in r.headers.items()}
            _respect_limits(hdrs)
            return r.status, body, hdrs
    except urllib.error.HTTPError as e:
        hdrs = {k.lower(): v for k, v in (e.headers or {}).items()}
        if e.code == 304:
            return 304, b"", hdrs
        if e.code in (403, 429):
            _respect_limits(hdrs, forced=True)
            return e.code, e.read(), hdrs
        return e.code, e.read(), hdrs


def _respect_limits(hdrs, forced=False):
    rem = hdrs.get("x-ratelimit-remaining")
    reset = hdrs.get("x-ratelimit-reset")
    if rem is not None and reset is not None:
        try:
            if int(rem) <= 1 or forced:
                wait = max(0, int(reset) - int(time.time())) + 3
                if wait > 0:
                    print(f"    limit exhausted, sleep {wait} s", flush=True)
                    time.sleep(min(wait, 3700))
                    return
        except ValueError:
            pass
    time.sleep(1.2)


def set_param(url, key, value):
    """Replace/add a parameter with an honest parse of the query.

    Naive substitution via re has already cost a loop here: `per_page=100`
    contains the substring `page=`, so the check “the parameter is already there” fired
    falsely, the page did not change, and the dump piled up 8991 records with only 100
    unique ids (R.41).
    """
    parts = urllib.parse.urlsplit(url)
    q = urllib.parse.parse_qsl(parts.query, keep_blank_values=True)
    q = [(k, v) for k, v in q if k != key] + [(key, str(value))]
    return urllib.parse.urlunsplit(parts._replace(query=urllib.parse.urlencode(q)))


def norm(tracker, engine, raw):
    """One schema for five engines. None — the record is discarded."""
    g = lambda k, d=None: raw.get(k, d)
    if engine == "github":
        if "pull_request" in raw:
            return None  # /issues also returns pull requests
        labels = [l["name"] for l in g("labels", [])]
        return dict(
            tracker=tracker, id=g("number"), url=g("html_url"), title=g("title") or "",
            body=(g("body") or "")[:BODY_MAX], state=g("state"),
            closed_reason=g("state_reason"), created=g("created_at"),
            updated=g("updated_at"), closed=g("closed_at"), labels=labels,
            severity_raw="", component="",
            comments=g("comments", 0) or 0,
            reactions=(g("reactions") or {}).get("total_count", 0),
            upvotes=0, cc=0, dupe_of=None)
    if engine == "gitlab":
        return dict(
            tracker=tracker, id=g("iid"), url=g("web_url"), title=g("title") or "",
            body=(g("description") or "")[:BODY_MAX], state=g("state"),
            closed_reason=None, created=g("created_at"), updated=g("updated_at"),
            closed=g("closed_at"), labels=g("labels", []),
            severity_raw=g("severity", "") or "", component="",
            comments=g("user_notes_count", 0) or 0, reactions=0,
            upvotes=g("upvotes", 0) or 0, cc=0, dupe_of=None)
    if engine == "bugzilla":
        return dict(
            tracker=tracker, id=g("id"),
            url=f"https://bugs.kde.org/show_bug.cgi?id={g('id')}",
            title=g("summary") or "", body="", state=g("status"),
            closed_reason=g("resolution") or None, created=g("creation_time"),
            updated=g("last_change_time"), closed=None,
            labels=g("keywords", []) or [],
            severity_raw=g("severity", "") or "", component=g("component", "") or "",
            comments=0, reactions=0, upvotes=0,
            cc=len(g("cc", []) or []), dupe_of=g("dupe_of"))
    if engine == "gitea":
        # Gitea, like GitHub, puts pull requests into /issues. The filter was written
        # only for github, and 100 PRs got into the blender-vse dump (R.42).
        # WARNING: check the VALUE, not the presence of the key. GitHub does not return the
        # pull_request field for issues at all, while Gitea returns it as null —
        # a check “in raw” would filter the whole dump down to the last record.
        if g("pull_request") or "/pulls/" in (g("html_url") or ""):
            return None
        labels = [l["name"] for l in g("labels", [])]
        return dict(
            tracker=tracker, id=g("number"), url=g("html_url"), title=g("title") or "",
            body=(g("body") or "")[:BODY_MAX], state=g("state"), closed_reason=None,
            created=g("created_at"), updated=g("updated_at"), closed=g("closed_at"),
            labels=labels, severity_raw="", component="",
            comments=g("comments", 0) or 0, reactions=0, upvotes=0, cc=0, dupe_of=None)
    if engine == "trac":
        # csv.DictReader silently parses ANY text: if the answer is not the CSV we
        # expect, we get rows with id=None and an empty summary. The ffmpeg-trac dump had
        # 3095 such blanks, and nobody noticed (R.42).
        # Trac returns columns with a CAPITAL letter (Summary, Created, Modified), although
        # the query uses lowercase, and puts a BOM before the file — because of which the first column
        # was called “﻿id”. Both misses produced empty records (R.42).
        raw = {k.lstrip("\ufeff").strip().lower(): v for k, v in raw.items() if k}
        rid = raw.get("id")
        if not rid or not (raw.get("summary") or "").strip():
            return None
        return dict(
            tracker=tracker, id=rid,
            url=f"https://trac.ffmpeg.org/ticket/{rid}",
            title=raw.get("summary", ""), body="", state="open", closed_reason=None,
            created=raw.get("created") or raw.get("time"),
            updated=raw.get("modified") or raw.get("changetime"), closed=None,
            labels=[raw.get("priority", "")], severity_raw=raw.get("type", "") or "",
            component=raw.get("component", "") or "",
            comments=0, reactions=0, upvotes=0, cc=0, dupe_of=None)
    return None


def out_path(key):
    return CACHE / f"{key}.jsonl"


def state_path(key):
    return CACHE / f"{key}.state.json"


def fetch(key, info, budget):
    """Returns (records, whether downloaded to the end)."""
    engine, base = info["engine"], info["url"]
    CACHE.mkdir(parents=True, exist_ok=True)
    st = {}
    if state_path(key).exists():
        st = json.loads(state_path(key).read_text(encoding="utf-8"))
    if st.get("done"):
        n = sum(1 for _ in open(out_path(key), encoding="utf-8")) if out_path(key).exists() else 0
        return n, True

    mode = "a" if st.get("next") else "w"
    # Resuming APPENDS to the file, and the counter is taken from state — without deduplication
    # a second pass puts the whole dump in again. That is how blender-vse became 3992 rows
    # for 1996 tasks, two copies of each (R.42).
    seen = set()
    if mode == "a" and out_path(key).exists():
        with open(out_path(key), encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    seen.add(json.loads(line).get("id"))
                except json.JSONDecodeError:
                    pass
    fh = open(out_path(key), mode, encoding="utf-8")
    total = st.get("count", 0)

    def emit(r):
        """The single place of writing: discards None and duplicates."""
        nonlocal total
        if not r or r.get("id") in seen:
            return
        seen.add(r.get("id"))
        fh.write(json.dumps(r, ensure_ascii=False) + "\n")
        total += 1
    hdrs = {}
    tok = token()

    try:
        if engine == "bugzilla":
            url = base + ("&" if "?" in base else "?") + (
                "include_fields=id,summary,status,resolution,severity,component,"
                "creation_time,last_change_time,cc,dupe_of,keywords")
            code, body, _ = get(url, budget=budget)
            if code != 200:
                print(f"    HTTP {code}"); return total, False
            for b in json.loads(body).get("bugs", []):
                emit(norm(key, engine, b))
            st = {"done": True, "count": total}

        elif engine == "trac":
            code, body, _ = get(base, budget=budget)
            if code != 200:
                print(f"    HTTP {code}"); return total, False
            rd = csv.DictReader(io.StringIO(body.decode("utf-8-sig", "replace")))
            cols = {(c or "").strip().lower() for c in (rd.fieldnames or [])}
            if not {"id", "summary"} <= cols:
                print(f"    THE RESPONSE IS NOT THE EXPECTED CSV: the columns {sorted(cols)[:6]} — "
                      "not a single record was taken")
                return total, False
            for row in rd:
                emit(norm(key, engine, row))
            st = {"done": True, "count": total}

        else:
            url = st.get("next")
            if not url:
                sep = "&" if "?" in base else "?"
                if engine == "github":
                    url = f"{base}{sep}state=all&per_page=100&sort=created&direction=asc"
                    hdrs["Accept"] = "application/vnd.github+json"
                    if tok:
                        hdrs["Authorization"] = f"Bearer {tok}"
                elif engine == "gitlab":
                    url = f"{base}{sep}per_page=100&order_by=created_at&sort=asc&scope=all"
                else:  # gitea
                    url = f"{base}{sep}state=all&limit=50&page=1"
            elif engine == "github":
                hdrs["Accept"] = "application/vnd.github+json"
                if tok:
                    hdrs["Authorization"] = f"Bearer {tok}"

            page = st.get("page", 1)
            while url:
                h = dict(hdrs)
                code, body, rh = get(url, headers=h, budget=budget)
                if code is None:
                    print(f"    budget exhausted on page {page}")
                    st = {"next": url, "count": total, "page": page}
                    return total, False
                if code != 200:
                    print(f"    HTTP {code} on page {page}")
                    st = {"next": url, "count": total, "page": page}
                    return total, False
                items = json.loads(body)
                if not items:
                    break
                for it in items:
                    emit(norm(key, engine, it))
                page += 1
                nxt = None
                if engine == "github":
                    link = rh.get("link", "")
                    m = re.search(r'<([^>]+)>;\s*rel="next"', link)
                    nxt = m.group(1) if m else None
                elif engine == "gitlab":
                    np = rh.get("x-next-page", "").strip()
                    if np:
                        nxt = set_param(url, "page", np)
                else:  # gitea
                    nxt = set_param(url, "page", str(page)) if len(items) == 50 else None
                if nxt == url:
                    print("    the next page matched the current one — stopping, "
                          "so as not to loop")
                    nxt = None
                url = nxt
                if total and total % 500 == 0:
                    print(f"    {total} records…", flush=True)
            st = {"done": True, "count": total}
    finally:
        fh.close()
        state_path(key).write_text(json.dumps(st), encoding="utf-8")

    return total, bool(st.get("done"))


def sha_of(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def cmd_list(reg):
    print(f"registry: {len(reg)} trackers; cache: {CACHE}")
    print(f"GitHub token: {'present' if token() else 'absent (60 requests/hour)'}")
    done = 0
    for k, v in reg.items():
        p, s = out_path(k), state_path(k)
        n = sum(1 for _ in open(p, encoding="utf-8")) if p.exists() else 0
        st = json.loads(s.read_text()) if s.exists() else {}
        mark = "ready" if st.get("done") else ("partially" if n else "no")
        if st.get("done"):
            done += 1
        print(f"  {k:<14} {v['engine']:<9} {n:>6} records  {mark}")
    print(f"done: {done}/{len(reg)}")
    return 0


def main():
    args = sys.argv[1:]
    reg = read_registry()
    if not reg:
        sys.exit("registry empty: the “## Registry” table in _trackers.md was not parsed")

    if not args or args[0] == "--list":
        return cmd_list(reg)

    if args[0] == "--pin":
        for k in reg:
            p = out_path(k)
            if p.exists():
                n = sum(1 for _ in open(p, encoding="utf-8"))
                print(f"| `{k}` | … | … | {n} | {time.strftime('%Y-%m-%d')} | `{sha_of(p)[:16]}` |")
        return 0

    budget = Budget(DEFAULT_BUDGET)
    if "--budget" in args:
        budget = Budget(int(args[args.index("--budget") + 1]))

    if args[0] == "--all":
        keys = list(reg)
    elif args[0] == "--fetch":
        keys = [args[1]]
        if keys[0] not in reg:
            sys.exit(f"no such tracker: {keys[0]}")
    else:
        print(__doc__)
        return 2

    print(f"GitHub token: {'present' if token() else 'absent (60 requests/hour)'}; "
          f"request budget: {budget.left}")
    incomplete = 0
    for k in keys:
        print(f"[{k}] {reg[k]['engine']}", flush=True)
        n, done = fetch(k, reg[k], budget)
        print(f"    {n} records, {'ready' if done else 'NOT FULLY DOWNLOADED'}", flush=True)
        # A dump finished with zero is almost always a broken parse, not an empty
        # tracker. A pull-request filter written as “the key is present” once cut off
        # the whole gitea down to the last record, and this passed silently (R.42).
        if done and n == 0:
            print("    WARNING: zero records on a successful response — almost certainly the parse is "
                  "broken, not an empty tracker", flush=True)
        if not done:
            incomplete += 1
    print(f"requests spent: {budget.used}")
    return 1 if incomplete else 0


if __name__ == "__main__":
    sys.exit(main())

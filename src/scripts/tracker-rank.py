#!/usr/bin/env python3
"""Анализ выгруженных багтрекеров (Р.41, третья волна разведки).

    python3 src/scripts/tracker-rank.py                    ранжирование тем
    python3 src/scripts/tracker-rank.py --theme прокси     топ задач темы
    python3 src/scripts/tracker-rank.py --wishes           только запросы (wishlist)
    python3 src/scripts/tracker-rank.py --rejected         что отказались делать
    python3 src/scripts/tracker-rank.py --neglect          открыто годами и весомо
    python3 src/scripts/tracker-rank.py --unmatched        чего лексикон не покрыл
    python3 src/scripts/tracker-rank.py --query 'regex' [--tracker K]
    python3 src/scripts/tracker-rank.py --table            markdown для _trackers.md
    python3 src/scripts/tracker-rank.py --check            дампы на месте и целы

Лексикон тем и реестр читаются из `docs/features/_trackers.md` и НЕ дублируются здесь —
тот же приём, что чтение категорий из README в features-index.py.

Почему считается именно так:

* **Доли, а не штуки.** Трекеры разного размера и культуры: у Blender комментируют втрое
  активнее, чем у Flowblade. Вес задачи делится на МЕДИАНУ своего трекера (не на среднее —
  распределения тяжелохвостые), а темы сравниваются долей внутри своего трекера.
* **Дубль весит больше всего** (×5): человек напоролся независимо, а триажёр подтвердил,
  что это то же самое. Комментарий дёшев и часто триажный шум, поэтому ×1.
* **«Закрыто» ≠ «починено».** У Kdenlive `WORKSFORME` — 24.4 % всех задач. Поэтому везде
  считается `fixed` = доля НАСТОЯЩИХ фиксов среди закрытых, а не доля закрытых.
* **FFmpeg trac вне общего рейтинга**: у Trac нет ни голосов, ни счётчика комментариев, и
  выборка только открытая — знаменатель несравним.

Коды возврата: 0 — успех, 1 — проверка не прошла, 2 — неизвестный аргумент.
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
CACHE = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "kinotrain" / "trackers"

# Корпус читается из колонки «Корпус» реестра, а НЕ из кода: раньше здесь стоял
# OUT_OF_RANK = {"ffmpeg-trac"}, и это ровно то дублирование реестра в скрипте,
# которое _trackers.md сам себе запрещает. Корпуса между собой не сравниваются —
# у них разные знаменатели (Р.42).
DEFAULT_CORPUS = "nle"

WISH_LABELS = re.compile(r"enhancement|feature|idea|rfe|type/design|type/to do", re.I)
BUG_LABELS = re.compile(r"\bbug\b|crash|regression|type/bug|defect", re.I)
WISH_TITLE = re.compile(r"^\s*(add|support|allow|please|would be nice|feature request|\[feature\])", re.I)
BUG_TITLE = re.compile(r"crash|freeze|hang|broken|not work|corrupt|error|fail", re.I)


def read_lexicon():
    """[(тема, скомпилированный регексп)] из таблицы «## Лексикон тем»."""
    text = TRACKERS.read_text(encoding="utf-8")
    m = re.search(r"## Лексикон тем(.*?)(?=\n## )", text, re.S)
    if not m:
        sys.exit("в _trackers.md нет раздела «## Лексикон тем»")
    out = []
    for row in re.finditer(r"^\|\s*([^|`]+?)\s*\|\s*`([^`]+)`\s*\|", m.group(1), re.M):
        name = row.group(1).strip()
        if name.lower() in ("тема",) or set(name) <= set("-: "):
            continue
        # В markdown-таблице пайп экранирован (\|), а в регэкспе это ЛИТЕРАЛЬНЫЙ
        # пайп, а не альтернатива. Снимаем экранирование при чтении.
        out.append((name, re.compile(row.group(2).replace(r"\|", "|"), re.I)))
    return out


def read_corpora():
    """{ключ: корпус} из колонки «Корпус» реестра.

    Разбор по ЗАГОЛОВКУ, а не по позиции: колонка дописана последней, и
    read_registry()/read_tracker_keys() якорятся на первые три — их порядок
    трогать нельзя.
    """
    out, idx = {}, None
    for line in TRACKERS.read_text(encoding="utf-8").splitlines():
        if not line.startswith("|"):
            idx = None
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if "Ключ" in cells and "Корпус" in cells:
            idx = cells.index("Корпус")
            continue
        if idx is None or len(cells) <= idx:
            continue
        k = re.match(r"^`([a-z0-9-]+)`$", cells[0])
        if k:
            out[k.group(1)] = cells[idx].strip("` ")
    return out


def load(corpus=None):
    """{ключ: [записи]} из кэша; corpus=None — все дампы без разбора."""
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
    """'wish' | 'bug' | '?' — по объявленному полю, эвристика только как крайний случай."""
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
    """Закрыта НАСТОЯЩИМ фиксом (а не WORKSFORME/not_planned/дублем)."""
    if is_open(r):
        return False
    cr = (r.get("closed_reason") or "").upper()
    if cr in ("FIXED", "COMPLETED"):
        return True
    if cr in ("WORKSFORME", "WONTFIX", "INVALID", "DUPLICATE", "UPSTREAM",
              "MOVED", "NOT_PLANNED", "UNMAINTAINED", "REPORTED_UPSTREAM"):
        return False
    return cr == ""  # github/gitea/gitlab: закрыта без пометки «не будем» → считаем фиксом


def rejected(r):
    cr = (r.get("closed_reason") or "").upper()
    return cr in ("WONTFIX", "INVALID", "NOT_PLANNED", "UNMAINTAINED")


def weights(data):
    """Нормировка веса на медиану своего трекера + подсчёт входящих дублей."""
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
    rank = list(data)  # корпус уже отфильтрован в load()
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
    rank = list(data)  # корпус уже отфильтрован в load()
    tot = sum(len(data[k]) for k in rank)
    hit = 0
    for k in rank:
        for r in data[k]:
            if any(match(r, rx) for _, rx in lex):
                hit += 1
    return hit, tot


def main():
    args = sys.argv[1:]
    mode = args[0] if args else "--themes"
    lex = read_lexicon()
    corpus = args[args.index("--corpus") + 1] if "--corpus" in args else DEFAULT_CORPUS
    raw_all = load(None)
    data = weights(load(None if mode == "--check" else corpus))
    if not data:
        known = sorted(set(read_corpora().values())) or ["(колонки «Корпус» в реестре нет)"]
        sys.exit(f"в корпусе «{corpus}» дампов нет; есть корпуса: {', '.join(known)}")

    if mode == "--check":
        cor = read_corpora()
        print(f"{len(data)} дампов, {sum(len(v) for v in data.values())} записей, "
              f"{len(lex)} тем в лексиконе")
        bad = 0
        for k in sorted(data):
            p = CACHE / f"{k}.jsonl"
            h = hashlib.sha256(p.read_bytes()).hexdigest()[:16]
            rows = raw_all.get(k, [])
            empty = sum(1 for r in rows if not (r.get("title") or "").strip())
            dup = len(rows) - len({r.get("id") for r in rows})
            flag = ""
            # Дамп ffmpeg-trac был 3095 записями с id=None и пустым title и прошёл
            # молча. Проверка, которой нет, — не проверка (Р.42).
            if rows and empty / len(rows) > 0.05:
                flag += f"  ПУСТЫХ {100*empty/len(rows):.0f}%"; bad += 1
            if dup:
                flag += f"  ПОВТОРОВ {dup}"; bad += 1
            print(f"  {k:<14} {len(rows):>6} записей  {cor.get(k,'?'):<8}"
                  f"sha256:{h}{flag}")
        if bad:
            print(f"\nдампов с браком: {bad} — перевыгрузить "
                  "(tracker-fetch.py удалит state и начнёт заново)")
        return 1 if bad else 0

    if mode in ("--themes", "--wishes", "--table"):
        rows, rank = themes_table(data, lex, only_wish=(mode == "--wishes"))
        hit, tot = coverage(data, lex)
        print(f"# корпус «{corpus}»: {len(rank)} трекеров ({', '.join(rank)})")
        print(f"# покрытие лексикона: {hit}/{tot} = {100*hit/max(1,tot):.1f}%"
              + ("  (только запросы)" if mode == "--wishes" else ""))
        print()
        print("| Тема | n | запросы | жалобы | откр. | фиксы | дубли | вес | universal | gap |")
        print("|---|---|---|---|---|---|---|---|---|---|")
        for d in rows:
            print(f"| {d['theme']} | {d['n']} | {d['wish']} | {d['bug']} | {d['open']} "
                  f"| {d['fixed']:.2f} | {d['dup']} | {d['w']:.0f} "
                  f"| {100*d['universal']:.1f}% | {d['gap']:.1f} |")
        return 0

    if mode == "--theme":
        rx = dict(lex).get(args[1]) if len(args) > 1 else None
        if rx is None:
            sys.exit(f"нет темы «{args[1] if len(args)>1 else ''}»; есть: "
                     + ", ".join(n for n, _ in lex))
        hit = [r for rows in data.values() for r in rows if match(r, rx)]
        hit.sort(key=lambda r: -r["_e"])
        for r in hit[:40]:
            print(f"{r['_e']:6.1f}  {r['tracker']:<13} {kind(r):<6} "
                  f"{'откр' if is_open(r) else 'закр':<5} {(r.get('created') or '')[:10]}  "
                  f"{(r.get('title') or '')[:80]}")
            print(f"        {r.get('url')}")
        return 0

    if mode == "--rejected":
        hit = [r for rows in data.values() for r in rows if rejected(r)]
        hit.sort(key=lambda r: -r["_e"])
        print(f"отклонено сопровождающими: {len(hit)}")
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
            print(f"{r['_n']:7.1f}  {years_open(r):4.1f} лет  {r['tracker']:<13} "
                  f"{kind(r):<6} {(r.get('title') or '')[:66]}\n        {r.get('url')}")
        return 0

    if mode == "--unmatched":
        rank = list(data)
        un = [r for k in rank for r in data[k]
              if not any(match(r, rx) for _, rx in lex)]
        hit, tot = coverage(data, lex)
        print(f"покрытие {100*hit/max(1,tot):.1f}%, непокрыто {len(un)}")
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
                print(f"{k:<14} {len(hit):>5} из {len(data[k]):>5} = "
                      f"{100*len(hit)/len(data[k]):.1f}%")
        allhit = [r for k, rows in data.items() if not only or k == only
                  for r in rows if match(r, rx)]
        allhit.sort(key=lambda r: -r["_e"])
        print("  примеры:")
        for r in allhit[:5]:
            print(f"    {r['tracker']} #{r['id']} {(r.get('title') or '')[:66]}\n"
                  f"      {r.get('url')}")
        return 0

    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())

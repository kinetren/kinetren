#!/usr/bin/env python3
"""Выгрузка багтрекеров открытых редакторов (Р.41, третья волна разведки).

    python3 src/scripts/tracker-fetch.py                 что объявлено и что уже лежит
    python3 src/scripts/tracker-fetch.py --fetch mlt      докачать один трекер
    python3 src/scripts/tracker-fetch.py --all            докачать все по очереди
    python3 src/scripts/tracker-fetch.py --all --budget 300
    python3 src/scripts/tracker-fetch.py --pin            строки реестра для _trackers.md

Реестр трекеров читается из `docs/features/_trackers.md` и НЕ дублируется здесь —
тот же приём, что чтение категорий из README в features-index.py.

Почему так, а не иначе:

* **Возобновление по КУРСОРУ, а не по номеру страницы.** GitHub не отдаёт `rel="last"`,
  общее число страниц заранее неизвестно. Плюс при сортировке по умолчанию (created desc)
  новая задача сдвигает пагинацию и выгрузка теряет записи. Поэтому везде, где можно, —
  `sort=created&direction=asc`: новое дописывается в хвост, а состояние хранит URL
  следующей страницы.
* **ETag.** Ответ 304 у GitHub не расходует лимит, поэтому повторный прогон почти
  бесплатен. ETag лежит в state-файле рядом с курсором.
* **Лимит читается из ответа**, а не угадывается: при `x-ratelimit-remaining <= 1` спим до
  `x-ratelimit-reset`. Без токена GitHub даёт 60 запросов в час.
* **Дампы вне репозитория** — ~150 МБ, лицензии на массовое распространение текстов задач
  нет ни у одного источника, и ни одному тесту они офлайн не нужны. См. раздел «Лицензии»
  в `_trackers.md`.

ТОКЕН. Читается из окружения `GITHUB_TOKEN` либо из `.env` в корне проекта (файл закрыт
`.gitignore`). Скрипт его НИКОГДА не печатает, не логирует и не сохраняет: в выводе видно
только «токен: есть/нет». Без токена всё работает, просто медленнее.

Коды возврата: 0 — успех, 1 — что-то не докачано, 2 — неизвестный аргумент.
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
CACHE = Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "kinotrain" / "trackers"

UA = "kinotrain-features-research (+docs/features/_trackers.md)"
BODY_MAX = 4000
DEFAULT_BUDGET = 55  # ниже часовых 60 у GitHub без токена


def token():
    """GITHUB_TOKEN из окружения либо из .env. Значение наружу не отдаётся."""
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
    """{ключ: {engine, url}} из таблицы «## Реестр» в _trackers.md."""
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
    """GET с уважением к лимитам. Возвращает (код, тело, заголовки).

    Сетевой сбой — не повод терять выгрузку: таймаут и обрыв соединения
    повторяются с нарастающей паузой. Один необработанный TimeoutError уже
    уронил прогон на середине pitivi (Р.41).
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
                print(f"    сеть не отвечает после {tries} попыток: {e}", flush=True)
                return None, None, None
            wait = 5 * (attempt + 1)
            print(f"    сетевой сбой ({e}), повтор через {wait} с", flush=True)
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
                    print(f"    лимит исчерпан, сон {wait} с", flush=True)
                    time.sleep(min(wait, 3700))
                    return
        except ValueError:
            pass
    time.sleep(1.2)


def set_param(url, key, value):
    """Заменить/добавить параметр честным разбором query.

    Наивная подстановка через re здесь уже стоила зацикливания: `per_page=100`
    содержит подстроку `page=`, поэтому проверка «параметр уже есть» срабатывала
    ложно, страница не менялась, и выгрузка намотала 8991 запись при 100
    уникальных id (Р.41).
    """
    parts = urllib.parse.urlsplit(url)
    q = urllib.parse.parse_qsl(parts.query, keep_blank_values=True)
    q = [(k, v) for k, v in q if k != key] + [(key, str(value))]
    return urllib.parse.urlunsplit(parts._replace(query=urllib.parse.urlencode(q)))


def norm(tracker, engine, raw):
    """Одна схема на пять движков. None — запись выбрасывается."""
    g = lambda k, d=None: raw.get(k, d)
    if engine == "github":
        if "pull_request" in raw:
            return None  # /issues отдаёт и пул-реквесты
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
        # Gitea, как и GitHub, кладёт пул-реквесты в /issues. Фильтр был написан
        # только для github, и в дамп blender-vse попало 100 PR (Р.42).
        # ВНИМАНИЕ: проверять ЗНАЧЕНИЕ, а не наличие ключа. GitHub поле
        # pull_request у задач не отдаёт вовсе, а Gitea отдаёт его равным null —
        # проверка "in raw" отсеяла бы весь дамп до последней записи.
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
        # csv.DictReader молча разбирает ЛЮБОЙ текст: если ответ не тот CSV, что мы
        # ждём, получаются строки с id=None и пустым summary. Дамп ffmpeg-trac был
        # 3095 таких пустышек, и никто этого не заметил (Р.42).
        # Trac отдаёт колонки с ЗАГЛАВНОЙ буквы (Summary, Created, Modified), хотя
        # в запросе они строчные, и предваряет файл BOM — из-за чего первая колонка
        # звалась "\ufeffid". Оба промаха давали пустые записи (Р.42).
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
    """Возвращает (записей, докачано_ли_до_конца)."""
    engine, base = info["engine"], info["url"]
    CACHE.mkdir(parents=True, exist_ok=True)
    st = {}
    if state_path(key).exists():
        st = json.loads(state_path(key).read_text(encoding="utf-8"))
    if st.get("done"):
        n = sum(1 for _ in open(out_path(key), encoding="utf-8")) if out_path(key).exists() else 0
        return n, True

    mode = "a" if st.get("next") else "w"
    # Возобновление ДОПИСЫВАЕТ файл, а счётчик берётся из state — без дедупликации
    # второй проход кладёт весь дамп повторно. Так blender-vse стал 3992 строками
    # на 1996 задач, по две копии каждой (Р.42).
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
        """Единственное место записи: отбрасывает None и повторы."""
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
                print(f"    ОТВЕТ НЕ ТОТ CSV: колонки {sorted(cols)[:6]} — "
                      "ни одной записи не взято")
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
                    print(f"    бюджет исчерпан на странице {page}")
                    st = {"next": url, "count": total, "page": page}
                    return total, False
                if code != 200:
                    print(f"    HTTP {code} на странице {page}")
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
                    print("    следующая страница совпала с текущей — обрываю, "
                          "чтобы не зациклиться")
                    nxt = None
                url = nxt
                if total and total % 500 == 0:
                    print(f"    {total} записей…", flush=True)
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
    print(f"реестр: {len(reg)} трекеров; кэш: {CACHE}")
    print(f"токен GitHub: {'есть' if token() else 'нет (60 запросов/час)'}")
    done = 0
    for k, v in reg.items():
        p, s = out_path(k), state_path(k)
        n = sum(1 for _ in open(p, encoding="utf-8")) if p.exists() else 0
        st = json.loads(s.read_text()) if s.exists() else {}
        mark = "готов" if st.get("done") else ("частично" if n else "нет")
        if st.get("done"):
            done += 1
        print(f"  {k:<14} {v['engine']:<9} {n:>6} записей  {mark}")
    print(f"готово: {done}/{len(reg)}")
    return 0


def main():
    args = sys.argv[1:]
    reg = read_registry()
    if not reg:
        sys.exit("реестр пуст: не разобралась таблица «## Реестр» в _trackers.md")

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
            sys.exit(f"нет такого трекера: {keys[0]}")
    else:
        print(__doc__)
        return 2

    print(f"токен GitHub: {'есть' if token() else 'нет (60 запросов/час)'}; "
          f"бюджет запросов: {budget.left}")
    incomplete = 0
    for k in keys:
        print(f"[{k}] {reg[k]['engine']}", flush=True)
        n, done = fetch(k, reg[k], budget)
        print(f"    {n} записей, {'готов' if done else 'НЕ ДОКАЧАН'}", flush=True)
        # Дамп, законченный нулём, — почти всегда сломанный разбор, а не пустой
        # трекер. Фильтр пул-реквестов, написанный как «ключ есть», однажды отсёк
        # весь gitea до последней записи, и это прошло молча (Р.42).
        if done and n == 0:
            print("    ВНИМАНИЕ: ноль записей при успешном ответе — почти наверняка "
                  "сломан разбор, а не пустой трекер", flush=True)
        if not done:
            incomplete += 1
    print(f"запросов израсходовано: {budget.used}")
    return 1 if incomplete else 0


if __name__ == "__main__":
    sys.exit(main())

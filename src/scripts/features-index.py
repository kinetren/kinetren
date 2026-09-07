#!/usr/bin/env python3
"""Проверка и сводка каталога docs/features/ (база будущих фич, Р.36).

    python3 src/scripts/features-index.py --check    валидация, ненулевой выход на ошибках
    python3 src/scripts/features-index.py --table    markdown-таблица приоритетов в stdout

Скрипт НИЧЕГО не переписывает: таблица вставляется в README руками (правило 20
чек-листа — ничего не отрастает само). Единственный источник правды о списке
категорий — таблица «## Категории» в docs/features/README.md; здесь она читается,
а не дублируется.

Зависимостей нет: frontmatter у нас — фиксированное подмножество YAML
(скаляр либо плоский список в квадратных скобках), парсится на месте.
"""

import datetime
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FEATURES = ROOT / "docs" / "features"
README = FEATURES / "README.md"
EDITORS = FEATURES / "_editors.md"

STATUSES = ["исследовано", "кандидат", "принято", "в-работе", "сделано",
            "отвергнуто", "уже-есть"]
PRIORITIES = ["A", "B", "C"]
COSTS = ["S", "M", "L", "XL"]
WE_HAVE = ["да", "нет", "частично"]

REQUIRED_SECTIONS = ["Суть", "У кого и как", "Фишка фишки", "Плюсы", "Минусы",
                     "Что говорят пользователи", "Как ложится на Kinotrain",
                     "Оценка", "Вердикт", "Источники"]

REQUIRED_FIELDS = ["id", "title", "status", "priority", "cost", "category",
                   "editors", "we_have", "crates", "invariants", "ui_rules",
                   "round", "point", "tests", "depends_on", "checked"]

# «Фишка фишки» короче этого — почти наверняка пересказ маркетингового списка.
GIMMICK_MIN = 200
STALE_DAYS = 548  # 18 месяцев


def parse_front(text):
    """Frontmatter → dict. Значение: список [a, b], null, либо строка."""
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
    """{заголовок H2: текст под ним}."""
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
    """Категории из таблицы README: {каталог: (код, план)}."""
    text = README.read_text(encoding="utf-8")
    out = {}
    for m in re.finditer(r"^\|\s*`(\w+)/`\s*\|\s*([A-Z]+)\s*\|[^|]*\|\s*(\d+)\s*/\s*(\d+)\s*\|",
                         text, re.M):
        out[m.group(1)] = (m.group(2), int(m.group(3)), int(m.group(4)))
    return out


def read_editor_keys():
    return set(re.findall(r"^\|\s*`([a-z0-9-]+)`\s*\|", EDITORS.read_text(encoding="utf-8"), re.M))


def load_notes():
    return sorted(p for p in FEATURES.rglob("*.md")
                  if p.name != "README.md" and not p.name.startswith("_"))


def check():
    errors, notes = [], []
    cats = read_categories()
    editor_keys = read_editor_keys()
    seen_ids = {}
    today = datetime.date.today()

    if not cats:
        errors.append("README.md: не разобралась таблица «## Категории»")

    for path in load_notes():
        rel = path.relative_to(ROOT)
        def bad(msg):
            errors.append(f"{rel}: {msg}")

        front, body = parse_front(path.read_text(encoding="utf-8"))
        if front is None:
            bad("нет YAML-frontmatter")
            continue

        for f in REQUIRED_FIELDS:
            if f not in front:
                bad(f"нет поля {f}")

        fid = front.get("id", "")
        # путь: docs/features/<категория>/<подкатегория>/<файл>.md
        parts = path.relative_to(FEATURES).parts
        if len(parts) != 3:
            bad("файл лежит не по схеме категория/подкатегория/файл.md")
        else:
            cat = parts[0]
            if cat not in cats:
                bad(f"категория «{cat}» не объявлена в README")
            else:
                code = cats[cat][0]
                if not fid.startswith(f"F-{code}-"):
                    bad(f"id «{fid}» не начинается с F-{code}- (категория {cat})")
            if front.get("category") != f"{parts[0]}/{parts[1]}":
                bad(f"поле category «{front.get('category')}» ≠ пути {parts[0]}/{parts[1]}")

        if not re.match(r"^F-[A-Z]+-\d{3}$", fid or ""):
            bad(f"id «{fid}» не в формате F-КОД-NNN")
        elif fid in seen_ids:
            bad(f"id {fid} уже занят: {seen_ids[fid]}")
        else:
            seen_ids[fid] = str(rel)

        m = re.search(r"^#\s+(\S+)\s+—\s+(.+?)\s*$", body, re.M)
        if not m:
            bad("нет заголовка «# F-КОД-NNN — Название»")
        else:
            if m.group(1) != fid:
                bad(f"id в заголовке ({m.group(1)}) ≠ id в шапке ({fid})")
            if front.get("title") and m.group(2).strip() != str(front["title"]).strip():
                bad("title в шапке ≠ названию в заголовке")

        for field, allowed in (("status", STATUSES), ("priority", PRIORITIES),
                               ("cost", COSTS), ("we_have", WE_HAVE)):
            v = front.get(field)
            if v is not None and v not in allowed:
                bad(f"{field}: «{v}» вне словаря {allowed}")

        for e in front.get("editors") or []:
            if e not in editor_keys:
                bad(f"редактор «{e}» не объявлен в _editors.md")

        sec = sections(body)
        for s in REQUIRED_SECTIONS:
            if s not in sec:
                bad(f"нет раздела «{s}»")
            elif not sec[s].strip():
                bad(f"раздел «{s}» пуст")

        # Три проверки против превращения базы в википедию.
        gimmick = sec.get("Фишка фишки", "")
        if len(gimmick) < GIMMICK_MIN:
            bad(f"«Фишка фишки» короче {GIMMICK_MIN} символов ({len(gimmick)}) — "
                "это пересказ, а не находка")
        feedback = sec.get("Что говорят пользователи", "")
        has_quote = re.search(r"^>", feedback, re.M) and "http" in feedback
        # Честный пробел лучше выдуманной цитаты: площадка бывает недоступна.
        gap = re.search(r"НЕТ ЦИТАТ:\s*(.{80,})", feedback, re.S)
        if not (has_quote or gap):
            bad("«Что говорят пользователи»: нужна дословная цитата (строка с >) "
                "и ссылка, либо пометка «НЕТ ЦИТАТ:» с объяснением, что искали")
        fits = sec.get("Как ложится на Kinotrain", "")
        if "src/" not in fits:
            bad("«Как ложится на Kinotrain»: нет ни одного пути вида src/… — "
                "раздел написан вслепую")

        status = front.get("status")
        if status == "отвергнуто" and len(sec.get("Вердикт", "")) < 40:
            bad("статус «отвергнуто» без внятной причины в «Вердикт»")
        if status == "сделано":
            if not front.get("point"):
                bad("статус «сделано» без point")
            if not front.get("tests"):
                bad("статус «сделано» без tests")
        if status in ("кандидат", "принято"):
            try:
                d = datetime.date.fromisoformat(str(front.get("checked")))
                if (today - d).days > STALE_DAYS:
                    bad(f"checked={d} старше 18 месяцев при статусе «{status}» — перепроверить")
            except ValueError:
                bad(f"checked=«{front.get('checked')}» не дата в формате ГГГГ-ММ-ДД")

        notes.append((front, rel))

    # Таблица покрытия в README не должна врать.
    actual = {}
    for front, rel in notes:
        actual[Path(rel).parts[2]] = actual.get(Path(rel).parts[2], 0) + 1
    for cat, (code, written, planned) in cats.items():
        got = actual.get(cat, 0)
        if got != written:
            errors.append(f"README: у {cat}/ написано «{written} / {planned}», "
                          f"а файлов {got}")

    print(f"{len(notes)} заметок, {len(cats)} категорий, {len(errors)} ошибок")
    for e in errors:
        print(f"  ОШИБКА  {e}")
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
    print("| ID | Фича | Статус | Приор. | Цена | У нас | Заметка |")
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

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

# Вторая волна (Р.39, категория help/). Вывод «нам нужна справка» известен ДО
# разведки, поэтому вердикт получает форму, а его обещание — адрес. Без этого
# 33 заметки написали бы одно и то же, и волна не стоила бы прочтения.
HELP_CAT = "help"
VERDICT_SHAPES = ("ТЕСТ:", "ПРОВЕРКА:", "ОТВЕРГНУТО:", "УЖЕ-ЕСТЬ:", "ОТЛОЖЕНО:")
VERDICT_TAIL_MIN = 60
TEMPLATE = FEATURES / "_template.md"

# Третья волна (Р.41): трекер — источник, а не ось. Заметка, объявившая trackers:,
# обязана нести СЧЁТ (сравнение долей двух трекеров) и АДРЕС (наш код). «В Kdenlive
# N багов про прокси» — статистика; находка начинается там, где число упёрто в src/.
TRACKERS = FEATURES / "_trackers.md"
SIGNAL_CAT = "resilience"

# Четвёртая волна (Р.42, категория collab/). Тема — облачные возможности чужих
# редакторов, а сети у нас нет ни в одном виде: 0 совпадений по
# std::net|TcpStream|reqwest|hyper|tokio во всём src/ и ни одного HTTP/TLS-крейта
# среди 505 пакетов Cargo.lock. Значит вывод «сети нет, поэтому не берём» известен
# ДО разведки и написался бы сам собой сорок пять раз. Заметка обязана разделить
# фишку надвое: что остаётся на ОДНОЙ машине — и чем именно платят за вторую половину.
SERVICES = FEATURES / "_services.md"
OFFLINE_CAT = "collab"
OFFLINE_SHAPES = ("ОСТАЁТСЯ", "ВЫРОЖДАЕТСЯ", "НЕ ОСТАЁТСЯ")
OFFLINE_TAIL_MIN = 80  # считается БЕЗ якоря: иначе длинный путь сам добирает минимум
# Закрытый словарь «чего у нас нет сверх сокета». Слова «сеть» здесь нет
# СОЗНАТЕЛЬНО: её отсутствие — не находка.
NET_COSTS = ("учётная запись", "общие часы", "журнал правок", "права доступа",
             "проект-как-база", "хранилище между проектами", "внешний исполнитель",
             "адрес материала")
ANCHOR_RE = re.compile(r"^(src/[\w./-]+?)(?::\d+(?:-\d+)?|::[\w:]+)?$")


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
    # Точка законна: ключ несёт версию (blender-5.2, vscode-1.136, jetbrains-2026.2).
    return set(re.findall(r"^\|\s*`([a-z0-9.-]+)`\s*\|", EDITORS.read_text(encoding="utf-8"), re.M))


def read_tracker_keys():
    """Ключи из таблицы «## Реестр» в _trackers.md — источник правды один."""
    if not TRACKERS.exists():
        return set()
    return set(re.findall(r"^\|\s*`([a-z0-9-]+)`\s*\|\s*\w+\s*\|\s*`",
                          TRACKERS.read_text(encoding="utf-8"), re.M))


def load_notes():
    return sorted(p for p in FEATURES.rglob("*.md")
                  if p.name != "README.md" and not p.name.startswith("_"))


def read_service_keys():
    """Ключи из таблицы «## Реестр» в _services.md.

    ОТДЕЛЬНЫЙ файл, а не третья таблица _editors.md: read_editor_keys() берёт любую
    строку файла со слагом в первой клетке, и `frame-io` молча стал бы законным
    значением поля editors: — то есть сервис попал бы в таблицу, где живёт вердикт.
    """
    if not SERVICES.exists():
        return set()
    m = re.search(r"## Реестр(.*?)(?=\n## |\Z)", SERVICES.read_text(encoding="utf-8"), re.S)
    return set(re.findall(r"^\|\s*`([a-z0-9.-]+)`\s*\|", m.group(1), re.M)) if m else set()


def anchor_ok(anchor):
    """Правило 8 механически: адрес — это код, который есть СЕГОДНЯ.

    Первая в базе проверка адреса ПО ДЕРЕВУ, а не по форме: якорь на
    src/libs/kt-collab/src/session.rs — уже схема реализации, и она не соберётся.
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

        # --- вторая волна: категория help/ (Р.39) --------------------------
        if len(parts) == 3 and parts[0] == HELP_CAT:
            verdict = sec.get("Вердикт", "")
            shaped = [l.strip() for l in verdict.splitlines()
                      if l.strip().startswith(VERDICT_SHAPES)]
            if not shaped:
                bad("«Вердикт»: нужна строка вида ТЕСТ:/ПРОВЕРКА:/ОТВЕРГНУТО:/"
                    "УЖЕ-ЕСТЬ:/ОТЛОЖЕНО: — «нам нужна справка» выводом не считается")
            else:
                head = shaped[0]
                kind, tail = head.split(":", 1)
                if len(tail.strip()) < VERDICT_TAIL_MIN:
                    bad(f"«Вердикт»: после «{kind}:» меньше {VERDICT_TAIL_MIN} символов")
                m = re.search(r"\bt\d+[a-z]?\b"
                              r"|(?:src|site|docs)/[\w./-]+"
                              r"|--check\b[\w -]*"
                              r"|\b\w+::\w+\b", head)
                if not m:
                    bad("«Вердикт»: обещание не адресуемо — назови тест, путь "
                        "или клаузулу --check")
                else:
                    help_promises.setdefault(m.group(0), []).append(str(rel))

            # Спор про всплывающие подсказки разрешён (Р.39), но только глядя
            # в глаза стражу: t83 держит запрет, правило 10 — его формулировка.
            if "tooltip_delay" in body or "Order::Tooltip" in body:
                if "t83" not in fits:
                    bad("спор про всплывающие подсказки без разбора t83 в «Как ложится» "
                        "— регресс-страж не назван")
                if 10 not in [int(x) for x in (front.get("ui_rules") or [])
                              if str(x).isdigit()]:
                    bad("спор про всплывающие подсказки без ui_rules: [10] в шапке")

        # --- третья волна: трекеры как источник (Р.41) ------------------
        trk = front.get("trackers") or []
        if len(parts) == 3 and parts[0] == SIGNAL_CAT and not trk:
            bad("категория resilience/ заведена под счётные находки: "
                "поле trackers: обязано быть непустым")
        if trk:
            for t in trk:
                if t not in tracker_keys:
                    bad(f"трекер «{t}» не объявлен в _trackers.md")

            sig = [l.strip() for l in feedback.splitlines()
                   if l.strip().startswith("СИГНАЛ:")]
            if not sig:
                bad("объявлен trackers:, но в «Что говорят пользователи» нет строки "
                    "«СИГНАЛ:» — статистика без сравнения находкой не считается")
            else:
                head = sig[0]
                if len(re.findall(r"\d", head)) < 2 or "%" not in head:
                    bad("«СИГНАЛ:» обязан нести два числа и долю в процентах "
                        "(число одного трекера против доли другого)")
                f = re.search(r"`([^`]+)`", head)
                if not f:
                    bad("«СИГНАЛ:» без точного фильтра в обратных кавычках — "
                        "число невоспроизводимо")
                else:
                    signal_filters.setdefault(f.group(1), []).append(str(rel))

            addr = [l.strip() for l in fits.splitlines()
                    if l.strip().startswith("АДРЕС:")]
            if not addr:
                bad("объявлен trackers:, но в «Как ложится» нет строки «АДРЕС:» — "
                    "чужая боль не привязана к нашему коду")
            elif "src/" not in addr[0]:
                bad("«АДРЕС:» без пути src/… — привязка к нашему коду не названа")

        # --- четвёртая волна: категория collab/ (Р.42) ------------------
        if len(parts) == 3 and parts[0] == OFFLINE_CAT:
            svc = front.get("services") or []
            if not svc:
                bad("категория collab/ заведена под облачные возможности: поле "
                    "services: обязано быть непустым — заметка обязана назвать, "
                    "что именно стоит по ту сторону сети")
            for sv in svc:
                if sv not in service_keys:
                    bad(f"сервис «{sv}» не объявлен в _services.md")

            off = [l.strip() for l in fits.splitlines()
                   if l.strip().startswith("БЕЗ СЕТИ:")]
            if not off:
                bad("«Как ложится»: нет строки «БЕЗ СЕТИ:» — «у нас нет сети, "
                    "поэтому не берём» выводом волны не считается")
            else:
                tail = off[0].split(":", 1)[1].strip()
                shape = next((x for x in OFFLINE_SHAPES if tail.startswith(x)), None)
                if shape is None:
                    bad("«БЕЗ СЕТИ:» без маркера — нужен ОСТАЁТСЯ / ВЫРОЖДАЕТСЯ / "
                        "НЕ ОСТАЁТСЯ первым словом")
                else:
                    offline_shapes_seen.setdefault(shape, []).append(str(rel))
                    a = re.search(r"`([^`]+)`", tail)
                    if not a:
                        bad("«БЕЗ СЕТИ:» без якоря в обратных кавычках — остаток "
                            "не адресуем (якорь обязателен и в ветке НЕ ОСТАЁТСЯ: "
                            "там он указывает на место, где отсутствие видно)")
                    elif not anchor_ok(a.group(1)):
                        bad(f"«БЕЗ СЕТИ:» якорь «{a.group(1)}» не указывает на "
                            "существующий файл src/… — заметка проектирует "
                            "реализацию (правило 8), а не описывает наш код")
                    else:
                        offline_anchors.setdefault(a.group(1), []).append(str(rel))
                    prose = re.sub(r"`[^`]*`", "", tail[len(shape):]).strip(" —–-:")
                    if len(prose) < OFFLINE_TAIL_MIN:
                        bad(f"«БЕЗ СЕТИ: {shape}» — прозы {len(prose)} символов при "
                            f"минимуме {OFFLINE_TAIL_MIN}: не сказано, ЧТО именно "
                            "остаётся (или где видно, что не остаётся ничего)")

            price = [l.strip() for l in fits.splitlines()
                     if l.strip().startswith("ЦЕНА СЕТИ:")]
            if not price:
                bad("«Как ложится»: нет строки «ЦЕНА СЕТИ:» — не названо, за что "
                    "именно платят той стороной")
            else:
                head = price[0]
                inv = [int(x) for x in re.findall(r"инвариант\w*\s+(\d{1,2})", head)]
                named = [c for c in NET_COSTS if c in head]
                if not inv and not named:
                    bad("«ЦЕНА СЕТИ:» обязана назвать инвариант N либо механизм из "
                        f"словаря: {', '.join(NET_COSTS)}. Слова «сеть» в словаре "
                        "нет: её отсутствие известно и без заметки")
                declared = [int(x) for x in (front.get("invariants") or [])
                            if str(x).isdigit()]
                for n in inv:
                    if not 1 <= n <= 12:
                        bad(f"«ЦЕНА СЕТИ:»: инварианта {n} не существует, их двенадцать")
                    elif n not in declared:
                        bad(f"«ЦЕНА СЕТИ:» называет инвариант {n}, а в шапке "
                            f"invariants: {declared} — проза и шапка разошлись")

        notes.append((front, rel))

    # Обещание, повторённое дважды, — это дублированный вывод.
    for token, where in sorted(help_promises.items()):
        if len(where) > 1:
            errors.append(f"help/: обещание «{token}» стоит ПЕРВЫМ в {len(where)} "
                          f"заметках ({', '.join(where)}) — вывод дублируется")

    # Один и тот же фильтр в двух заметках — это одна находка, посчитанная дважды.
    # Остаток, названный дважды, — это один и тот же остаток.
    for a, where in sorted(offline_anchors.items()):
        if len(where) > 1:
            errors.append(f"БЕЗ СЕТИ: якорь «{a}» стоит в {len(where)} заметках "
                          f"({', '.join(where)}) — остаток задвоен")

    # Балансир уровня КАТЕГОРИИ. Где всё ОСТАЁТСЯ — выдуманное локальное ядро;
    # где ничего не остаётся — тот самый брак, ради которого правило заведено.
    n_collab = sum(1 for f, _ in notes
                   if str(f.get("category", "")).startswith(OFFLINE_CAT + "/"))
    if n_collab >= 10:
        for sh in OFFLINE_SHAPES:
            if sh not in offline_shapes_seen:
                errors.append(f"collab/: {n_collab} заметок и ни одной с маркером "
                              f"«{sh}» — классификатор выродился, проверь ось")

    for f, where in sorted(signal_filters.items()):
        if len(where) > 1:
            errors.append(f"СИГНАЛ: фильтр «{f}» повторяется в {len(where)} заметках "
                          f"({', '.join(where)}) — находка задвоена")

    # _template.md исключён из load_notes() (имя на «_»), поэтому его слаги
    # однажды протухли молча и сломали бы каждую скопированную заметку.
    tfront, _ = parse_front(TEMPLATE.read_text(encoding="utf-8"))
    for e in (tfront or {}).get("editors") or []:
        if e not in editor_keys:
            errors.append(f"_template.md: редактор «{e}» не объявлен в _editors.md")
    for sv in (tfront or {}).get("services") or []:
        if sv not in service_keys:
            errors.append(f"_template.md: сервис «{sv}» не объявлен в _services.md")

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

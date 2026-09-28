#!/usr/bin/env python3
"""Живое покрытие: до каких мест человек НИ РАЗУ не дошёл мышью (Р.68).

Решение владельца: «часть тестов вынести в non-gui, ведь тесты отчасти
повторяют действия между собой, можно большую простую часть вынести в
headless спокойно, и прокликивать сложные моменты или те которые не
прокликали ни разу». Чтобы «не прокликали ни разу» не было вкусовщиной,
его считает машина.

Что с чем сверяется:

* ПРОГРАММА публикует прямоугольники именованных виджетов
  (`widgets::field_probe::note("…")`) — это и есть места, куда живой
  сценарий может прицелиться;
* СЦЕНАРИИ кликают их по имени (`click_probe`, `right_click_probe`,
  `double_click_probe`, `hover_probe`, `has_probe`,
  `scroll_probe_into_view`) — ИЛИ берут прямоугольник щупа из дампа
  (`f["param"] == "bin.reel"`) и целятся по координате. Второй путь
  считается ТАК ЖЕ: иначе отчёт врёт в свою пользу — `bin.reel` и `speed`
  прокликаны руками в `nestparam` и `rate`, а числились дырой.

Отчёт: имена, которые программа публикует, а мышью не трогал никто, — это
кандидаты на живую проверку. Красным (`--check`) становятся вещи про ЛОЖЬ,
а не про полноту, плюс ОБЪЯВЛЕННЫЙ ПРЕДЕЛ дыр:

1. сценарий целится в имя, которого программа больше НЕ публикует — такой
   сценарий проверяет воздух;
2. спека ссылается на живой сценарий, которого нет на диске — «переезд»
   проверки в headless обязан оставлять сценарий спеки с новым адресом, а
   не выкидывать его;
3. у сценария НЕТ ТОЧКИ ВХОДА (`if __name__ == "__main__"`). Такой файл
   запускается и не делает НИЧЕГО, а прогон считает его зелёным: `select.py`
   прожил так с Р.33 и был пойман только хронометражем (0.0 с на сценарий,
   который должен идти пятнадцать секунд);
4. живой сценарий из реестра переезда (`MIGRATED`) остался на диске или его
   новый адрес не существует. Переезд проверки в headless — это СМЕНА
   АДРЕСА, а не потеря: реестр называет, куда уехала каждая проверка, и
   сторож проверяет, что названный тест есть в исходниках;
5. нетронутых мест стало БОЛЬШЕ объявленного предела (`UNTOUCHED_LIMIT`).
   Предел — не цель, а храповик: новое место программы либо кликается
   живьём, либо предел двигают осознанно, одной строкой и с объяснением.

    python3 scripts/live-coverage.py            # отчёт
    python3 scripts/live-coverage.py --check    # отчёт и вердикт
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src" / "apps" / "kt-app" / "src"
LIVE = ROOT / "src" / "scripts" / "live"
SPECS = ROOT / "openspec"

NOTE = re.compile(r'field_probe::note\(\s*&?(?:format!\()?\s*"([^"]+)"')
# Имя щупа часто собирается `format!` в переменную, и по вызову `note(...)`
# его не увидеть. Поэтому вторым источником берутся все литералы ФОРМЫ
# ИМЕНИ ЩУПА: «слово.слово[.слово]», где вставки записаны как «{}»/«{x}».
SHAPE = re.compile(r'"([a-z][a-z0-9_]*(?:\.[a-z0-9_]*(?:\{[^"}]*\})?[a-z0-9_]*)+)"')
USE = re.compile(
    r'(?:click_probe|right_click_probe|double_click_probe|hover_probe|has_probe'
    r'|scroll_probe_into_view)\(\s*f?"([^"]+)"'
    # ВТОРОЙ ЗАКОННЫЙ ПУТЬ ПРИЦЕЛА: сценарий достаёт прямоугольник щупа из
    # дампа сам и целится по координате — так `nestparam` жмёт карточку
    # композиции в бине, а `rate` тянет полосу скорости. Не считать его
    # значит врать в свою пользу: место прокликано руками, а отчёт зовёт
    # его дырой и посылает заводить второй такой же сценарий.
    r'|\[\s*"param"\s*\]\s*==\s*"([^"]+)"'
)
# Клик — это прицел; `has_probe` бывает и ОТРИЦАТЕЛЬНЫМ («виджета тут быть не
# должно»), и имя в нём законно отсутствует у программы. Поэтому «целится в
# несуществующее» считается только по жестам.
USE_CLICK = re.compile(
    r'(?:click_probe|right_click_probe|double_click_probe|hover_probe'
    r'|scroll_probe_into_view)\(\s*f?"([^"]+)"'
)
SPEC_LIVE = re.compile(r"src/scripts/live/([a-z_0-9]+)\.py")
FILE_EXT = {"otio", "png", "json", "db", "rs", "py", "toml", "wav", "mp4", "jpg", "ktproj"}

# РЕЕСТР ПЕРЕЕЗДА (Р.68, задача 5.2). Критерий — в спеке `ui-reach`: живьём
# остаётся место, не тронутое НИ РАЗУ, попадание жеста и сложный путь из
# претензии владельца; проверка, которая ПОСЛЕ уже прокликанного в другом
# месте жеста утверждает только состояние модели или файла, уезжает в
# headless. Реестр — доказательство, что уехала ПРОВЕРКА, а не исчезла:
# сторож требует, чтобы файла сценария больше не было И чтобы каждый
# названный тест существовал в исходниках.
MIGRATED = {
    "nest": (
        "свернуть → войти → подвинуть → выйти → сохранить: все утверждения "
        "про модель и .otio, жесты прокликаны в nestparam/nestshare/nestdead",
        ["t427", "t456", "t457", "t431"],
    ),
    "nestenter": (
        "каскад Enter/Esc и полоса внутри открытой ленты: утверждения про "
        "глубину и строки, клавиатурный жест прокликан в nestdead/nestshare",
        ["t597", "t634", "t636"],
    ),
    "nestopen": (
        "раскрытие на месте, правка из полосы, Esc: утверждения про документ "
        "и то, что играет; двойной клик по событию прокликан в nestdead/audiofx",
        ["t570", "t571", "t572", "t573"],
    ),
    "nestzero": (
        "событие за ноль внутри ленты и курсор, который туда не идёт: "
        "утверждения про модель, драг события прокликан в select/level/corner",
        ["t637", "t637b"],
    ),
    "fxfield": (
        "напечатанное значение ключа доезжает до файла: набор в поле параметра "
        "прокликан живьём в nestparam (ручки экземпляра) и formula",
        ["t281"],
    ),
    "curvesrc": (
        "битый текст формулы не портит модель, верный применяется: жест "
        "остался живым в formula (туда же переехал клик «отвязать»)",
        ["t709", "t749"],
    ),
}

# ПРЕДЕЛ ДЫР — храповик, а не цель (Р.68, задача 5.3). Мест, до которых
# человек не доходил мышью НИ РАЗУ, было 108 из 174 на начало стадии 5
# (после того как отчёт научился считать вторым прицелом разбор дампа —
# 105). Предел держит достигнутое: новое место программы либо получает
# живой шаг, либо число двигают осознанно — одной строкой и с причиной в
# отчёте.
UNTOUCHED_LIMIT = 105


def as_pattern(name):
    """«titlew.preset.{i}» → регулярка, которой соответствует «titlew.preset.2»."""
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
            # имена файлов и расширения в форму имени щупа тоже попадают
            if m.group(1).rsplit(".", 1)[-1] in FILE_EXT:
                continue
            out.setdefault(m.group(1), p.name)
    return out


def used(rx=USE):
    out = {}
    for p in sorted(LIVE.glob("*.py")):
        if p.name in ("kt_live.py", "worker.py"):
            continue
        for m in rx.finditer(p.read_text(encoding="utf-8")):
            # у прицела два законных вида (вызов щупа и разбор дампа), и
            # какая группа сработала — знает только регулярка
            name = next((g for g in m.groups() if g), None)
            if name:
                out.setdefault(name, set()).add(p.stem)
    return out


def migration_faults():
    """Нарушения реестра переезда: сценарий остался или адрес пустой."""
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
                f"сценарий {name}.py объявлен переехавшим в headless, а он на диске: "
                "либо уберите его из реестра MIGRATED, либо удалите файл"
            )
        for t in tests:
            # адрес сверяется ТОЧНО (`fn t456_…`, а не «начинается на t456»):
            # иначе удаление t456 прошло бы незамеченным, пока жив t456b
            if not re.search(rf"\bfn {re.escape(t)}_", text):
                bad.append(
                    f"переезд {name}.py назвал адресом тест {t}, которого нет в исходниках"
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

    print(f"живое покрытие: программа публикует {len(pub)} мест, "
          f"сценарии трогают {len(pub) - len(untouched)}, "
          f"мышью ни разу не трогали {len(untouched)} "
          f"(объявленный предел {UNTOUCHED_LIMIT})")
    if MIGRATED and not brief:
        print(f"переехало в headless ({len(MIGRATED)} сценариев):")
        for name, (why, tests) in sorted(MIGRATED.items()):
            print(f"  {name}.py → {', '.join(tests)}: {why}")
    if untouched and not brief:
        print(f"мышью НИ РАЗУ не трогали ({len(untouched)}):")
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
        bad.append(f"сценарий целится в «{n}», которого программа не публикует")
    for n in dead_specs:
        bad.append(f"спека ссылается на живой сценарий {n}.py, которого нет на диске")
    for n in no_entry:
        bad.append(
            f"у сценария {n}.py нет точки входа (`if __name__ == \"__main__\"`) — "
            "он запускается, не делает ничего и считается зелёным"
        )
    bad.extend(migration_faults())
    if len(untouched) > UNTOUCHED_LIMIT:
        bad.append(
            f"мест, не тронутых мышью НИ РАЗУ, стало {len(untouched)} при "
            f"объявленном пределе {UNTOUCHED_LIMIT}: новому месту нужен живой "
            "шаг, либо предел двигается осознанно (UNTOUCHED_LIMIT)"
        )
    if bad:
        print("ЖИВОЕ ПОКРЫТИЕ — КРАСНОЕ:", file=sys.stderr)
        for b in bad:
            print("  " + b, file=sys.stderr)
        return 1
    print("живое покрытие: сценарии целятся в существующие места, "
          "спеки — в существующие сценарии")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

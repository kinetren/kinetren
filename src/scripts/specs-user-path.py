#!/usr/bin/env python3
"""Сторож «ПУТЬ ПОЛЬЗОВАТЕЛЯ» у UI-сценариев (Р.67).

Владелец: «СКОЛЬКО РАЗ МНЕ ПОВТОРЯТЬ ЧТОБЫ ВСЕ ___ВСЕ___ АГЕНТЫ ДУМАЛИ О
ПОЛЬЗОВАТЕЛЕ В ПЕРВУЮ ОЧЕРЕДЬ? … ЭТО КООРДИНАЛЬНО НАДО МЕНЯТЬ!». Правило
прозой (чек-лист kt-app, Р.40) держалось семь раундов и не сработало:
проверка «что видит человек» обязана быть частью ТЕКСТА КОНТРАКТА.

Каждый сценарий UI-capability, чей тест живёт в `src/apps/kt-app/` и имеет
номер tNNN ≥ 783, обязан нести строку `- **ПУТЬ**`: куда кликнуть, что
потянуть и что видно после каждого шага. Порог по номеру, а не «у всех»:
нумерация сквозная и монотонная, и переписывать две сотни прежних
сценариев ради формы незачем — правило действует с раунда, в котором оно
заведено.

Проверяются и контракты (`openspec/specs`), и активные изменения
(`openspec/changes/*/specs`): иначе сторож молчал бы ровно там, где
пишется новый сценарий.
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CAPS = ("ui-timeline", "ui-reach")
FIRST = 783  # первый номер теста, с которого действует правило

SCEN = re.compile(r"^#### Scenario: (.+)$")
TEST = re.compile(r"^- \*\*TEST\*\* (.+)$")
PATH = re.compile(r"^- \*\*ПУТЬ\*\*\s*(.*)$")
TNUM = re.compile(r"\bt(\d+)[a-z]?\b")


def spec_files():
    for cap in CAPS:
        p = ROOT / "openspec" / "specs" / cap / "spec.md"
        if p.exists():
            yield p
    for d in sorted((ROOT / "openspec" / "changes").glob("*/specs")):
        if "archive" in d.parts:
            continue
        for cap in CAPS:
            p = d / cap / "spec.md"
            if p.exists():
                yield p


def check(path):
    bad = []
    name, test_line, path_line = None, None, None

    def close():
        if name is None or test_line is None:
            return
        in_app = "src/apps/kt-app/" in test_line
        nums = [int(n) for n in TNUM.findall(test_line)]
        if not in_app or not nums or max(nums) < FIRST:
            return
        if path_line is None:
            bad.append((name, "нет строки ПУТЬ"))
        elif len(path_line.strip()) < 40:
            bad.append((name, f"ПУТЬ короче 40 символов: {path_line.strip()!r}"))

    for line in path.read_text(encoding="utf-8").splitlines():
        m = SCEN.match(line)
        if m:
            close()
            name, test_line, path_line = m.group(1), None, None
            continue
        if name is None:
            continue
        m = TEST.match(line)
        if m:
            test_line = m.group(1)
            continue
        m = PATH.match(line)
        if m:
            path_line = m.group(1)
    close()
    return bad


def main():
    problems = []
    for f in spec_files():
        for name, why in check(f):
            problems.append(f"{f.relative_to(ROOT)}: сценарий «{name}» — {why}")
    if problems:
        print("UI-сценарий без «пути пользователя» (что видит человек — часть контракта):")
        for p in problems:
            print("  " + p)
        return 1
    print("сценарии UI с тестами kt-app от t783: путь пользователя описан у всех")
    return 0


if __name__ == "__main__":
    sys.exit(main())

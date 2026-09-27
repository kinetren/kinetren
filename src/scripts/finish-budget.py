#!/usr/bin/env python3
"""Цена проверки: таблица «стадия — секунды», итог и бюджет (Р.68).

Повод дословно (владелец): «тесты в finish.sh или что-то типо того - очень
долго выполняются, нужно что-то придумать». Замер показал, что круг проверки
стоит 21 минуту, но НИ ОДНА стадия не называла своей цены — говорить о том,
что ускорять, было нечем, а деградация никому не видна.

Бюджет — 300 с (пять минут, поставлено владельцем). Перерасход ПЕЧАТАЕТСЯ и
помечается, но НЕ делает прогон красным: иначе бюджет стал бы ножом по
набору проверок, а правило «тесты из спек не ослабляются» сильнее бюджета
(см. openspec/specs/durability/spec.md).

    finish-budget.py --report target/finish-times.txt    # таблица и вердикт
    finish-budget.py --check-script scripts/finish.sh    # все стадии в замере
    finish-budget.py --selftest                          # проверка себя (t800)

Формат файла замеров — по строке на стадию: «имя<TAB>секунды<TAB>код».
"""
import re
import sys
from pathlib import Path

BUDGET = 300  # с; пять минут — решение владельца (Р.68)


class Bad(Exception):
    """Сам учёт сломан: это красное, а не перерасход."""


def read_times(path):
    p = Path(path)
    if not p.exists():
        raise Bad(f"нет файла замеров {path} — стадии не хронометрировались")
    rows = []
    for n, line in enumerate(p.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        parts = line.split("\t")
        if len(parts) != 3:
            raise Bad(f"{path}:{n}: строка не «имя<TAB>секунды<TAB>код»: {line!r}")
        name, secs, rc = parts
        try:
            rows.append((name, int(secs), int(rc)))
        except ValueError as e:
            raise Bad(f"{path}:{n}: не число: {line!r}") from e
    if not rows:
        raise Bad(f"{path}: ни одного замера — стадии не хронометрировались")
    return rows


def report(rows, budget=BUDGET, out=sys.stdout):
    total = sum(s for _, s, _ in rows)
    width = max(len(n) for n, _, _ in rows)
    print("== цена проверки ==", file=out)
    for name, secs, rc in sorted(rows, key=lambda r: -r[1]):
        mark = "" if rc == 0 else "  ← КРАСНАЯ"
        share = 100.0 * secs / total if total else 0.0
        print(f"  {secs:6d} c  {name:<{width}}  {share:4.0f} %{mark}", file=out)
    print(f"  {total:6d} c  {'ИТОГО':<{width}}  бюджет {budget} c", file=out)
    if total > budget:
        worst, wsecs, _ = max(rows, key=lambda r: r[1])
        print(
            f"ВНИМАНИЕ: проверка вышла за бюджет {budget} c на {total - budget} c; "
            f"дороже всех «{worst}» ({wsecs} c, {100.0 * wsecs / total:.0f} %)",
            file=out,
        )
    else:
        print(f"проверка в бюджете: {total} c из {budget} c", file=out)
    return total


# ---- сторож самого скрипта ------------------------------------------------
#
# Стадию можно было бы тихо вынести из бюджета — напечатать заголовок мимо
# обёртки, и её секунды исчезнут из таблицы. Это тот же класс, что «красное
# чужим именем» (Р.55) и «вердикт у tail» (Р.58): отчёт врёт о том, что
# происходило. Поэтому единственный законный способ напечатать заголовок
# стадии — обёртка, а сторож это проверяет.
# Заголовок стадии — это печать строки, НАЧИНАЮЩЕЙСЯ с «== ». Шире брать
# нельзя: «simd: ok (AVX2 == скаляр)» — обычная строка вердикта, и сторож,
# который падал бы на ней, научил бы обходить себя переформулировкой.
HEADER = re.compile(r"""(?:echo|printf)\s+["']==\s""")


def check_script(path):
    p = Path(path)
    if not p.exists():
        raise Bad(f"нет скрипта {path}")
    src = p.read_text(encoding="utf-8")
    bad = []
    in_stage = False
    depth = 0
    for n, line in enumerate(src.splitlines(), 1):
        stripped = line.strip()
        if re.match(r"^stage(_log)?\s*\(\)\s*\{", stripped):
            in_stage, depth = True, 1
            continue
        if in_stage:
            depth += line.count("{") - line.count("}")
            if depth <= 0:
                in_stage = False
            continue
        if stripped.startswith("#"):
            continue
        if HEADER.search(stripped):
            bad.append((n, stripped))
    if bad:
        raise Bad(
            "заголовок стадии печатается мимо обёртки stage — её секунды не попадут "
            "в бюджет:\n"
            + "\n".join(f"  {path}:{n}: {t}" for n, t in bad)
        )
    for need, why in (
        ("finish-budget.py", "скрипт не зовёт учёт цены — таблицы стадий не будет"),
        ("finish-times.txt", "скрипт не пишет файл замеров"),
    ):
        if need not in src:
            raise Bad(f"{why} (нет упоминания {need})")
    return True


def selftest():
    import io

    # 1) перерасход: назван самой дорогой стадией и размером перебора
    buf = io.StringIO()
    report([("живые сценарии", 1012, 0), ("мастерская", 259, 0)], out=buf)
    said = buf.getvalue()
    assert "ВНИМАНИЕ" in said and "живые сценарии" in said, said
    assert "на 971 c" in said, said  # 1012 + 259 - 300

    # 2) в бюджете: пометки нет
    buf = io.StringIO()
    report([("сборка", 43, 0), ("мастерская", 90, 0)], out=buf)
    said = buf.getvalue()
    assert "ВНИМАНИЕ" not in said and "в бюджете" in said, said

    # 3) красная стадия названа красной прямо в таблице
    buf = io.StringIO()
    report([("живые сценарии", 120, 1)], out=buf)
    assert "КРАСНАЯ" in buf.getvalue(), buf.getvalue()

    # 4) сломанный учёт — это красное, а не «ноль стадий»
    for rows, what in ((None, "нет файла"), ("", "пустой файл")):
        import tempfile

        with tempfile.TemporaryDirectory() as d:
            f = Path(d) / "times.txt"
            if rows is not None:
                f.write_text(rows, encoding="utf-8")
            try:
                read_times(f)
            except Bad:
                pass
            else:
                raise AssertionError(f"{what}: учёт промолчал")

    # 5) сторож видит заголовок стадии мимо обёртки
    import tempfile

    with tempfile.TemporaryDirectory() as d:
        f = Path(d) / "finish.sh"
        f.write_text(
            'stage() {\n  printf "== %s ==\\n" "$1"\n}\n'
            "# finish-budget.py finish-times.txt\n"
            'stage "сборка" cargo build\n',
            encoding="utf-8",
        )
        assert check_script(f) is True
        f.write_text(
            'stage() {\n  printf "== %s ==\\n" "$1"\n}\n'
            "# finish-budget.py finish-times.txt\n"
            'echo "== чистка =="\n',
            encoding="utf-8",
        )
        try:
            check_script(f)
        except Bad as e:
            assert "мимо обёртки" in str(e), e
        else:
            raise AssertionError("сторож не заметил стадии вне обёртки")

    print("finish-budget: самопроверка зелёная (перерасход, бюджет, красная стадия, "
          "сломанный учёт, стадия мимо обёртки)")


def main(argv):
    if len(argv) == 2 and argv[1] == "--selftest":
        selftest()
        return 0
    if len(argv) == 3 and argv[1] == "--report":
        report(read_times(argv[2]))
        return 0
    if len(argv) == 3 and argv[1] == "--check-script":
        check_script(argv[2])
        print(f"{argv[2]}: все стадии идут через обёртку с замером")
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    try:
        sys.exit(main(sys.argv))
    except Bad as e:
        print(f"УЧЁТ ЦЕНЫ СЛОМАН: {e}", file=sys.stderr)
        sys.exit(1)

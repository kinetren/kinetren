#!/usr/bin/env python3
"""Сторож «ожидание, а не сон» у живых сценариев (Р.68).

Замер Р.68: 96 % времени живого прогона уходило в безусловный `time.sleep`
(`nestparam` — 130.5 с из 135.9 в 591 вызове; `curvesrc` — 41.9 из 43.2).
Паузы подбирались под софт-рендер («кадр идёт ~0.2 с»), то есть каркас
ЧАСАМИ ждал того, о чём программа может сказать сама: она сообщает номер
нарисованного кадра и число прочитанных событий ввода.

Правило, которое этот сторож держит:

* в СЦЕНАРИИ (`scripts/live/*.py`) сырого `time.sleep` не бывает вовсе —
  ожидание идёт через каркас (`wait_frame`, `wait_seen`, `settle`,
  `wait_playhead`, `pixel_settled`), а цикл опроса берёт `poll_step()`;
* в КАРКАСЕ сырой `time.sleep` живёт ровно в трёх местах: `pause()` —
  пауза С ПРИЧИНОЙ, `_poll()` — шаг ожидания внутри каркаса и
  `poll_step()` — шаг опроса для циклов сценария;
* у каждого вызова `pause(...)` причина словами не короче сорока символов.

Мера «сорок символов» — та же, что у сторожа ПУТЬ (Р.67) и у реестра окон
(Р.66): исключение обязано быть ИЗМЕРИМЫМ, иначе отговорка становится
дешевле описания.

    python3 scripts/live-sleeps.py --check
"""
import ast
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LIVE = ROOT / "src" / "scripts" / "live"
FRAMEWORK = "kt_live.py"
# каркас работника поднимает Xvfb и процессы — там ожидание системное, а не
# оконное: ждать «кадра» ещё некому, программа не запущена
RUNNER = "worker.py"
SLEEP_HOMES = {"pause", "_poll", "poll_step"}
MIN_REASON = 40


def raw_sleeps(tree):
    """(строка, имя объемлющей функции) для каждого `time.sleep(...)`."""
    out = []

    def walk(node, fn):
        for child in ast.iter_child_nodes(node):
            name = fn
            if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef)):
                name = child.name
            if (
                isinstance(child, ast.Call)
                and isinstance(child.func, ast.Attribute)
                and child.func.attr == "sleep"
                and isinstance(child.func.value, ast.Name)
                and child.func.value.id == "time"
            ):
                out.append((child.lineno, fn))
            walk(child, name)

    walk(tree, None)
    return out


def pause_calls(tree):
    """(строка, причина) для каждого вызова `pause(...)`."""
    out = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == "pause":
            reason = None
            if len(node.args) >= 2:
                arg = node.args[1]
                if isinstance(arg, ast.Constant) and isinstance(arg.value, str):
                    reason = arg.value
                elif isinstance(arg, ast.JoinedStr):
                    reason = "".join(
                        v.value for v in arg.values if isinstance(v, ast.Constant)
                    )
            out.append((node.lineno, reason))
    return out


def check():
    bad = []
    for path in sorted(LIVE.glob("*.py")):
        if path.name == RUNNER:
            continue
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        rel = f"src/scripts/live/{path.name}"
        for line, fn in raw_sleeps(tree):
            if path.name == FRAMEWORK and fn in SLEEP_HOMES:
                continue
            if path.name == FRAMEWORK:
                bad.append(
                    f"{rel}:{line}: сырой `time.sleep` в каркасе вне `pause`/`_poll` "
                    f"(функция {fn}) — ожидание обязано ждать кадр или ввод программы"
                )
            else:
                bad.append(
                    f"{rel}:{line}: сырой `time.sleep` в сценарии — ждать надо "
                    "программу (`wait_frame`/`wait_seen`/`settle`), а не часы"
                )
        for line, reason in pause_calls(tree):
            if reason is None:
                bad.append(f"{rel}:{line}: `pause(...)` без причины словами")
            elif len(reason) < MIN_REASON:
                bad.append(
                    f"{rel}:{line}: причина паузы короче {MIN_REASON} символов: {reason!r}"
                )
    if bad:
        print("ОЖИДАНИЕ ПОДМЕНЕНО СНОМ:", file=sys.stderr)
        for b in bad:
            print("  " + b, file=sys.stderr)
        return 1
    print("живые сценарии ждут программу, а не часы (сырых пауз нет)")
    return 0


if __name__ == "__main__":
    if "--check" not in sys.argv[1:]:
        print(__doc__)
        sys.exit(2)
    sys.exit(check())

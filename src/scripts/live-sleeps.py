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
* у каждого вызова `pause(...)` причина словами не короче сорока символов;
* КАЖДЫЙ ЖЕСТ каркаса ждёт, пока программа прочитает ввод (`_after_gesture`),
  и фиксированных пауз внутри жеста не держит — законные места перечислены
  поимённо (`PAUSE_OK_IN`). Это правило Р.69: ожидание по событию было
  построено в Р.68 и до самих жестов не доехало.

Мера «сорок символов» — та же, что у сторожа ПУТЬ (Р.67) и у реестра окон
(Р.66): исключение обязано быть ИЗМЕРИМЫМ, иначе отговорка становится
дешевле описания.

    python3 scripts/live-sleeps.py --check
    python3 scripts/live-sleeps.py --check --scripts <копия каталога scripts>
"""
import ast
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
# КОРЕНЬ ПРОВЕРЯЕМЫХ ИСХОДНИКОВ — ПОДМЕНЯЕМЫЙ (`--scripts <каталог>`).
# Тесты сторожа (t803, t808) проверяют его НАРУШЕНИЯМИ: подставляют сценарий
# с сырой паузой, ломают драйвер, поднимают константу. Делать это в рабочем
# дереве нельзя — тесты идут одновременно (нити у `cargo test`, процессы у
# nextest), и порча одного становится красным вердиктом другого. Ровно этот
# класс («флак по построению») раунд и убирает, так что своих тестов правило
# касается тоже: каждый работает на СВОЕЙ копии.
SCRIPTS = ROOT / "src" / "scripts"
if "--scripts" in sys.argv:
    SCRIPTS = Path(sys.argv[sys.argv.index("--scripts") + 1]).resolve()
LIVE = SCRIPTS / "live"
FRAMEWORK = "kt_live.py"
# ДРАЙВЕР МЫШИ И КЛАВИАТУРЫ (Р.69). До этого раунда сторож смотрел только на
# каркас и сценарии, а драйвер спал внутри себя — 123 с из 387 с сна всего
# живого прогона жили ВНЕ области действия правила «пауза только с причиной».
# Сон, спрятанный в драйвере, ничем не отличается от сна в каркасе.
DRIVER = SCRIPTS / "ui-probe.py"
DRIVER_SLEEP_HOME = "_nap"
# каркас работника поднимает Xvfb и процессы — там ожидание системное, а не
# оконное: ждать «кадра» ещё некому, программа не запущена
RUNNER = "worker.py"
SLEEP_HOMES = {"pause", "_poll", "poll_step"}
MIN_REASON = 40

# ЖЕСТЫ КАРКАСА и правило Р.69: каждый ждёт, пока ПРОГРАММА прочитает ввод
# (`_after_gesture`), а не столько, сколько кадр идёт «обычно». Правило
# держится сторожем, потому что прозой оно уже не удержалось: ожидание по
# событию было построено в Р.68 и до жестов не доехало.
GESTURES = {
    "click",
    "right_click",
    "double_click",
    "drag",
    "key",
    "wheel",
    "hover",
    "close_window",
    "type_text",
}
# Где фиксированная пауза внутри жеста ЗАКОННА, и почему ровно там:
#   _after_gesture — фолбэк, когда события у egui законно не будет вовсе;
#   focus          — поднятие окна делает оконный менеджер, своего события
#                    ввода у программы тут нет;
#   double_click   — зазор между кликами это ВРЕМЕННОЙ порог egui (Р.34);
#   type_text      — зазор между символами, когда его просит сценарий.
PAUSE_OK_IN = {"_after_gesture", "focus", "double_click", "type_text"}

# БЮДЖЕТ СНА — ХРАПОВИК (Р.69, стадия 1). Причины словами оказалось мало:
# все 265 с сна в каркасе были «законными», у каждой паузы стояло
# объяснение длиннее сорока символов. Поэтому считается ещё и СУММА
# ОБЪЯВЛЕННЫХ СЕКУНД: каждая константа в `pause(...)` каркаса и в сонных
# умолчаниях драйвера. Мера статическая — она не зависит ни от машины, ни
# от того, сколько раз жест позвали: вердикт, зависящий от загрузки, это
# флак по построению (фактический сон прогона печатает live-check.sh).
#
# Предел двигают ОСОЗНАННО, одной строкой и с причиной — как
# `UNTOUCHED_LIMIT` у отчёта живого покрытия (Р.68).
SLEEP_BUDGET = 14.9


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


def declared_pauses(tree, rel):
    """[(секунды, где)] у каждого вызова `pause(<число>, …)` дерева."""
    out = []
    for node in ast.walk(tree):
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id == "pause"
            and node.args
            and isinstance(node.args[0], ast.Constant)
            and isinstance(node.args[0].value, (int, float))
        ):
            out.append((float(node.args[0].value), f"{rel}:{node.lineno}"))
    return out


def driver_sleeps(tree):
    """Сон драйвера X: сырые `time.sleep` вне `_nap` и сонные УМОЛЧАНИЯ.

    Умолчание — это тоже пауза, и она опаснее записанной в вызове: её не
    видно на месте жеста. До Р.69 `move(pause=0.05)`, `button(pause=0.09)` и
    `key(pause=0.05, after=0.12)` спали за каркас именно так.
    """
    raw = [(line, fn) for line, fn in raw_sleeps(tree) if fn != DRIVER_SLEEP_HOME]
    seconds = 0.0
    defaults = []
    for node in ast.walk(tree):
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        args = node.args
        pairs = list(zip(args.args[len(args.args) - len(args.defaults):], args.defaults))
        for arg, default in pairs:
            if arg.arg not in ("pause", "after"):
                continue
            if isinstance(default, ast.Constant) and isinstance(default.value, (int, float)):
                seconds += float(default.value)
                defaults.append((node.name, arg.arg, float(default.value)))
    return raw, seconds, defaults


def gesture_faults(tree):
    """Жест, который не ждёт программу, и пауза внутри жеста не по списку."""
    bad = []
    for node in ast.walk(tree):
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        if node.name not in GESTURES:
            continue
        src = ast.dump(node)
        if "'_after_gesture'" not in src:
            bad.append(
                f"жест `{node.name}` не ждёт ПРОЧИТАННЫЙ ввод (`_after_gesture`): "
                "фиксированная пауза не растягивается под нагрузкой — это тот самый "
                "класс ложных покраснений (§468), который Р.69 и убирает"
            )
        if node.name in PAUSE_OK_IN:
            continue
        for line, _reason in pause_calls(node):
            bad.append(
                f"kt_live.py:{line}: фиксированная пауза внутри жеста `{node.name}`. "
                "Законные места перечислены (`PAUSE_OK_IN`): фолбэк ожидания, поднятие "
                "окна, временной порог двойного клика, зазор набора по просьбе сценария"
            )
    return bad


def check():
    bad = []
    declared = []
    for path in sorted(LIVE.glob("*.py")):
        if path.name == RUNNER:
            continue
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        rel = f"{SCRIPTS.name}/live/{path.name}"
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
        declared.extend(declared_pauses(tree, rel))
        if path.name == FRAMEWORK:
            bad.extend(gesture_faults(tree))

    # ДРАЙВЕР X — та же область (Р.69)
    drv = ast.parse(DRIVER.read_text(encoding="utf-8"), filename=str(DRIVER))
    raw, drv_seconds, defaults = driver_sleeps(drv)
    for line, fn in raw:
        bad.append(
            f"{DRIVER.name}:{line}: сырой `time.sleep` в драйвере вне "
            f"`{DRIVER_SLEEP_HOME}` (функция {fn}) — сон драйвера обязан быть "
            "виден и посчитан наравне с сном каркаса"
        )
    declared.extend(
        (v, f"{DRIVER.name} ({f}.{a})") for f, a, v in defaults
    )
    budget = sum(v for v, _ in declared)

    print(
        f"объявленный сон живой проверки: {budget:.2f} c при пределе "
        f"{SLEEP_BUDGET:.2f} c (в драйвере {drv_seconds:.2f} c: "
        + ", ".join(f"{f}.{a}={v}" for f, a, v in defaults)
        + ")"
    )
    if budget > SLEEP_BUDGET + 1e-9:
        worst = max(declared, default=(0.0, "—"))
        bad.append(
            f"объявленный сон {budget:.2f} c превысил предел {SLEEP_BUDGET:.2f} c; "
            f"самая дорогая пауза — {worst[1]} ({worst[0]} c). Предел двигают "
            "осознанно, одной строкой и с причиной (SLEEP_BUDGET)"
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

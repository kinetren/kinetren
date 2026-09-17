#!/usr/bin/env python3
"""Живой сценарий Р.54 (п.431): галерея пикера эффектов в НАСТОЯЩЕЙ программе.

Харнесс-тесты (t547/t548) гоняют клик и вёрстку внутри одного процесса на
искусственном клипе. Здесь проверяется то, чего харнесс проверить не может:
миниатюры рисуются НА КАДРЕ настоящего медиа (фоновый поток успел посчитать
цепочку и залить текстуру), а клик по ячейке доезжает до файла проекта с
параметрами ИМЕННО этого варианта.

Попап в дереве доступности не ищется, поэтому координаты берутся из самой
программы: `KT_GEOM_DUMP` отдаёт прямоугольники `fxreg.add` и
`fxreg.var.<вид>.<номер>` (правило Р.27 — сценарий не угадывает смещения).
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, geometry, require_tools  # noqa: E402

SEC = 90000
# «Плотно» — второй вариант размытия (radius 24 против шаблонных 8)
VARIANT = "fxreg.var.blur.1"
WANT_RADIUS = 24


def radius_in(path):
    """Радиусы всех blur-эффектов в сохранённом проекте."""
    doc = json.load(open(path))
    found = []

    def walk(node):
        if isinstance(node, dict):
            if node.get("kind") == "blur":
                found.append(node.get("radius"))
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)

    walk(doc)
    return found


def main(out):
    require_tools()
    proj = os.path.join(out, "fxpicker.otio")
    subprocess.run([KT_CLI, "demo", proj, "--force"], check=True, capture_output=True)

    with Live("fxpicker", out, project=proj) as app:
        # 1) окно FX по двойному клику (зазор между кликами МАЛЕНЬКИЙ, п.232)
        y = app.lane_y("video", 0, frac=0.5)
        before = app.shot("before")
        app.double_click(app.x_at(3 * SEC), y)
        app.shot("fxwin")
        d = app.screen_diff(before)
        app.expect(d > 0.01, f"окно FX открылось (экран изменился на {d:.1%})")

        # 2) галерея: кнопка пикера отдаёт свой прямоугольник, клик по ней
        # раскрывает попап
        pre_picker = app.shot("pre-picker")
        if not app.click_probe("fxreg.add", "программа отдала прямоугольник кнопки пикера"):
            return app.done()
        app.shot("gallery")
        d = app.screen_diff(pre_picker)
        app.expect(d > 0.02, f"галерея раскрылась (экран изменился на {d:.1%})")

        # 2б) сужаем поиском, как человек: 27 строк в прокрутку целиком не
        # влезают, а ячейки НИЖЕ СГИБА не рисуются и не считаются вовсе.
        # Строка поиска получает фокус сама при открытии меню.
        for ch in "blur":
            app.key(ch)
        app.shot("filtered")

        # 3) МИНИАТЮРЫ ПОСЧИТАНЫ: ячейка варианта перестала быть пустой
        # плашкой. Ждём не «пока устоится», а СМЕНЫ (правило Р.37): фоновый
        # поток считает цепочку и заливает текстуру уже после раскрытия.
        cell = None
        for _ in range(40):
            f = next(
                (f for f in app.geom().get("fields", []) if f["param"] == VARIANT),
                None,
            )
            if f:
                cell = f
                break
            time.sleep(0.25)
        app.expect(cell is not None, f"программа отдала прямоугольник ячейки «{VARIANT}»")
        if cell is None:
            return app.done()
        win = geometry()
        # центр КАРТИНКИ ячейки (подпись идёт ниже картинки)
        px = (
            win["x"] + int(cell["x"] + cell["w"] / 2),
            win["y"] + int(cell["y"] + cell["h"] * 0.35),
        )
        drawn = app.pixel_settled(px, differs_from=None)
        app.expect(
            drawn is not None and sum(drawn) > 24,
            f"миниатюра нарисована, а не осталась чёрной плашкой (пиксель {drawn})",
        )

        # 4) ОСЬ СИЛЫ: проводка по горизонтали от ячейки «Плотно» к
        # соседней слева обязана дать ПРОМЕЖУТОЧНОЕ значение — то, что
        # человек видел под курсором, а не названный вариант.
        win = geometry()
        cx = win["x"] + int(cell["x"] + cell["w"] / 2)
        cy = win["y"] + int(cell["y"] + cell["h"] * 0.35)
        before_radii = radius_in(proj)
        app.drag(cx, cy, cx - int(cell["w"] / 2), cy)
        app.shot("axis")
        app.saved_project()
        axis_radii = [r for r in radius_in(proj) if r not in before_radii]
        app.expect(
            len(axis_radii) == 1,
            f"проводка добавила ровно один blur (было {before_radii}, стало {radius_in(proj)})",
        )
        if axis_radii:
            app.expect(
                3 < axis_radii[0] < WANT_RADIUS,
                f"ось дала ПРОМЕЖУТОЧНОЕ значение между 3 и {WANT_RADIUS}: {axis_radii[0]}",
            )
        # откат: ось добавила эффект одним шагом истории, и Ctrl+Z обязан
        # его снять — иначе следующая проверка мерила бы сумму двух жестов
        app.key("ctrl+z")
        app.saved_project()
        after_undo = radius_in(proj)
        app.expect(
            axis_radii and axis_radii[0] not in after_undo,
            f"Ctrl+Z снял добавленное осью (осталось {after_undo})",
        )

        # 5) клик по ячейке — и в ФАЙЛЕ ровно её параметры
        before_radii = after_undo
        app.click_probe("fxreg.add", "пикер открывается повторно")
        for ch in "blur":
            app.key(ch)
        if not app.click_probe(VARIANT, "ячейка варианта кликабельна"):
            return app.done()
        app.shot("after-pick")
        app.saved_project()
        after_radii = radius_in(proj)
        app.expect(
            len(after_radii) == len(before_radii) + 1,
            f"в проекте появился ровно один новый blur ({before_radii} → {after_radii})",
        )
        new = [r for r in after_radii if r not in before_radii] or after_radii
        app.expect(
            WANT_RADIUS in new,
            f"добавился ВАРИАНТ (radius {WANT_RADIUS}), а не шаблон: {after_radii}",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

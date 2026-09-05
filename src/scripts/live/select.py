#!/usr/bin/env python3
"""Живой сценарий Р.33: рамка выделения и перенос пачки.

Слова пользователя (п.7): «на основном таймлайне не хватает area-selection…
можно по зажатию ctrl выделять через area… ну и это естественно везде
учесть». Харнесс-тесты t202-t205 проверяют это в симуляции, а здесь —
НАСТОЯЩЕЕ окно: мышь водит XTest с зажатым Ctrl, а результат читается из
сохранённого программой .otio.

Проверяется цепочка, которой пользуется человек: рамкой взял событие →
Ctrl+кликом добавил второе → потянул одно, уехали ОБА, каждое на один и
тот же сдвиг, длительности целы.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "select.otio"),
        [(0.0, 4.0, "synth:Ramp"), (6.0, 4.0, "synth:Dc")],
    )

    with Live("select", out, project=proj) as app:
        y = app.lane_y("video", 0, frac=0.72)
        app.shot("open")

        p = app.project_now()
        c1, c2 = p.clip("c1"), p.clip("c2")
        app.expect(
            c1.start == 0 and c2.start == 6 * SEC, f"стартовый монтаж: {p.clips}"
        )

        # 1. Ctrl+драг с ПУСТОГО места (зазор 4..6 с) вправо за конец c2:
        # рамка обязана забрать c2 и НЕ создать регион
        app.drag(
            app.x_at(5 * SEC), y, app.x_at(11 * SEC), y, mods=("ctrl",)
        )
        app.shot("after-rubber")
        p = app.saved_project()
        region = (
            p.doc.get("metadata", {}).get("kt", {}).get("transport", {}).get("region")
        )
        app.expect(not region, f"Ctrl+драг создал регион вместо рамки: {region}")

        # 2. Ctrl+клик по первому событию добавляет его к выделению
        app.click(app.x_at(2 * SEC), y, mods=("ctrl",))
        app.shot("after-ctrl-click")

        # 3. тянем ПЕРВОЕ событие вправо на 2 с — уехать обязаны оба
        app.drag(app.x_at(2 * SEC), y, app.x_at(4 * SEC), y)
        app.shot("after-block-move")
        p = app.saved_project()
        c1, c2 = p.clip("c1"), p.clip("c2")
        app.near(c1.start, 2 * SEC, 27000, "первое событие уехало на 2 с")
        app.near(c2.start, 8 * SEC, 27000, "второе событие пачки уехало на столько же")
        app.near(c1.duration, 4 * SEC, 3000, "длительность c1 цела")
        app.near(c2.duration, 4 * SEC, 3000, "длительность c2 цела")
        app.expect(not p.transitions, f"перенос пачки не заводит переходов: {p.transitions}")

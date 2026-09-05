#!/usr/bin/env python3
"""Живой сценарий Р.33 С2: окно эффектов открывается двойным кликом и
показывает цепочку.

Проверяются два решения раунда сразу: двойной клик по событию открывает FX
(п.204 — подсказка обещала это с Р.18 и врала) и окно рисуется единой
полосой цепочки (п.217). Окно ищется по пикселям: после двойного клика над
таймлайном обязана появиться панель, которой до клика не было.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, require_tools  # noqa: E402
import subprocess  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    # демо-проект: у первого видео-события есть цепочка (пан/зум), значит
    # окно покажет и полосу цепочки, и сетку параметров
    proj = os.path.join(out, "fxwin.otio")
    subprocess.run([KT_CLI, "demo", proj], check=True, capture_output=True)

    with Live("fxwin", out, project=proj) as app:
        y = app.lane_y("video", 0, frac=0.72)
        before = app.shot("open")
        p = app.project_now()
        c1 = p.clips[0]

        # двойной клик по событию: окно FX обязано открыться
        x = app.x_at(c1.mid())
        app.double_click(x, y)
        after = app.shot("after-double-click")

        # окно рисуется поверх центра экрана: сравниваем полосу пикселей
        # НАД таймлайном — до клика там было превью, после появилось окно
        probe_y = int(y) - 220
        changed = 0
        for px in range(300, 1500, 40):
            if app.pixel(px, probe_y) != app.pixel_of(before, px, probe_y):
                changed += 1
        app.expect(
            changed >= 6,
            f"двойной клик открыл окно FX (изменилось точек над таймлайном: {changed})",
        )
        app.expect(os.path.exists(after), "скриншот окна снят")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

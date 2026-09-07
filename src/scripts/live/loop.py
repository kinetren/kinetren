#!/usr/bin/env python3
"""Живой сценарий Р.23: loop-регион и клик мимо него.

Дефект, который чинили в Р.23: при игре по loop-региону клик ПРАВЕЕ его
конца оставлял звук в петле, хотя курсор уходил линейно — «слева все ок, а
справа нет». Здесь это проверяется по-настоящему: курсор читается из
программы, а картинка — с экрана (клипы разного цвета, поэтому видно, что
именно играет).
"""
import json
import os
import time
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    red = color_media(out, "red", "C81E1E", seconds=10)
    blue = color_media(out, "blue", "1E3CC8", seconds=10)
    path = project_with_clips(
        os.path.join(out, "loop.otio"), [(0.0, 8.0, red), (8.0, 8.0, blue)]
    )
    set_region(path, start=2.0, end=6.0)

    with Live("loop", out, project=path) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 2, f"монтаж из двух клипов: {p.clips}")

        # 1. играем по региону: курсор обязан оставаться внутри и заворачиваться
        app.key("space")
        seen = app.wait_playhead(lambda t: t > 4 * SEC, "курсор пошёл", timeout=10)
        if not app.playing():
            app.expect(False, "воспроизведение не началось (нет звукового устройства?)")
            app.shot("no-playback")
            return app.done()
        inside = app.wait_playhead(lambda t: t < 3 * SEC, "курсор завернулся в начало региона")
        app.expect(
            all(t <= 6 * SEC + SEC // 2 for t in inside),
            f"курсор не убегал за конец региона: max {max(inside)}",
        )
        app.expect(app.brightness_red(app.preview_center()), "в регионе играет красный клип")
        app.shot("looping")

        # 2. клик ПРАВЕЕ региона во время игры — играем то, что под курсором
        before_right = app.pixel(*app.preview_center())
        app.click(app.x_at(11 * SEC), app.ruler_y())
        moved = app.wait_playhead(lambda t: t > 11 * SEC, "курсор пошёл от места клика")
        app.expect(
            all(t > 6 * SEC for t in moved[1:]),
            f"курсор НЕ вернулся в регион: {moved[:6]}",
        )
        r, g, b = app.pixel_settled(app.preview_center(), differs_from=before_right)
        app.expect(b > r + 40, f"справа от региона играет синий клип: RGB {r},{g},{b}")
        app.shot("after-click-right")

        # 3. клик ЛЕВЕЕ региона — то же самое зеркально.
        #
        # Вид СЛЕДУЕТ за курсором, поэтому к этому моменту начало проекта
        # уехало за левый край: `x_at(1 c)` отдавал x = −613, клик уходил
        # МИМО ОКНА, а проверка зеленела на устаревшем пикселе. Возвращаем
        # начало в вид (Home = nav_prev) и убеждаемся, что момент правда
        # виден — иначе проверка снова будет мерить не то.
        app.key("Home")
        time.sleep(0.6)
        geom = app.geom()
        target = 1 * SEC
        app.expect(
            geom["view_start"] <= target,
            f"начало проекта вернулось в вид: view_start={geom['view_start']}",
        )
        app.click(app.x_at(target), app.ruler_y())
        left = app.wait_playhead(lambda t: t < 3 * SEC, "курсор ушёл ЛЕВЕЕ региона")
        app.expect(
            all(t < 3 * SEC for t in left[1:]),
            f"курсор не остался слева от региона: {left[:6]}",
        )
        r, g, b = app.pixel_settled(app.preview_center(), differs_from=(r, g, b))
        app.expect(r > b + 40, f"слева от региона играет красный клип: RGB {r},{g},{b}")
        app.key("space")
        app.shot("after-click-left")
        return app.done()


def set_region(path, start, end):
    """Регион + loop в конверте kt.transport."""
    with open(path) as f:
        doc = json.load(f)
    doc["metadata"]["kt"]["transport"] = {
        "region": [round(start * SEC), round(end * SEC)],
        "loop_enabled": True,
    }
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

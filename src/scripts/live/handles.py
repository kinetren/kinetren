#!/usr/bin/env python3
"""Живой сценарий Р.26: наехал — растащил — материал вернулся.

Ровно то, на что жаловался пользователь: «в sony vegas если растащить
перекрывающиеся события — остаётся то, что перекрывалось, без обрезки».
Проверяется в НАСТОЯЩЕМ окне: мышь водит XTest, результат читается из
сохранённого программой .otio, а не из внутреннего состояния теста.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "handles.otio"), [(0.0, 6.0), (8.0, 6.0)])

    with Live("handles", out, project=proj) as app:
        y = app.lane_y("video", 0)
        sec = app.seconds
        app.shot("open")

        # 1. наезд: второй клип (8..14 с) тянем влево на 2 секунды
        app.drag(app.x_at(sec(11.0)), y, app.x_at(sec(9.0)), y)
        p = app.saved_project()
        app.expect(len(p.transitions) == 1, "наезд создал кроссфейд")
        app.expect(
            all(c.duration < sec(6.0) for c in p.clips),
            f"клипы подрезаны стыком: {p.clips}",
        )
        app.shot("after-overlap")

        # ручки видны: слева и справа от стыка в окне перехода — не пустота
        joint = sec(7.0)
        left = app.pixel(app.x_at(joint - sec(0.4)), y)
        right = app.pixel(app.x_at(joint + sec(0.4)), y)
        app.expect(
            left != (0, 0, 0) and right != (0, 0, 0),
            f"ручки нарисованы: слева {left}, справа {right}",
        )

        # 2. растаскиваем обратно — материал обязан вернуться
        app.drag(app.x_at(sec(9.0)), y, app.x_at(sec(13.0)), y)
        p = app.saved_project()
        app.expect(not p.transitions, "переход умер вместе со стыком")
        for c in p.clips:
            app.near(c.duration, sec(6.0), 3000, f"длительность {c.name} вернулась")
        app.expect(
            all(c.source_in == 0 for c in p.clips),
            f"точка входа вернулась к нулю: {p.clips}",
        )
        app.shot("after-pull-apart")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

#!/usr/bin/env python3
"""Живой сценарий Р.28 п.12: фейд за верхний уголок события.

Слова пользователя: «в sony vegas fade можно у событий сделать по краям без
еще одних событий — просто потянув за верхний уголок события». Проверяется
в НАСТОЯЩЕМ окне: мышь водит XTest, а результат читается из сохранённого
программой .otio — включая то, что само событие при этом НЕ тронуто
(«ничего нигде не должно отрастать/отрубаться», п.6).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "corner.otio"),
        [(2.0, 8.0, "synth:Ramp")],
    )

    with Live("corner", out, project=proj) as app:
        y_body = app.lane_y("video", 0, frac=0.72)
        y_strip = app.lane_y("video", 0, frac=0.06)  # верхняя полоса события
        app.shot("open")

        p = app.project_now()
        c1 = p.clip("c1")
        app.expect(c1.duration == 8 * SEC and not p.transitions, f"старт: {p.clips}")

        # выделяем событие — ручки фейда видны только у выделенного
        app.click(app.x_at(c1.mid()), y_body)
        app.shot("selected")

        # тянем ЛЕВЫЙ верхний уголок вправо на 2 с
        app.drag(app.x_at(c1.start + 200), y_strip, app.x_at(c1.start + 2 * SEC), y_strip)
        p = app.saved_project()
        app.expect(len(p.transitions) == 1, f"уголок завёл фейд: {p.transitions}")
        win = p.transitions[0]
        app.expect(win["in"] == 0, f"внешнего офсета нет — фейд внутри события: {win}")
        app.near(win["out"], 2 * SEC, SEC // 4, "длина фейда ≈ пройденному пути")
        c1 = p.clip("c1")
        app.expect(
            c1.duration == 8 * SEC and c1.source_in == 0,
            f"событие не тронуто: {p.clips}",
        )
        app.shot("after-fade")

        # задвигаем уголок обратно — фейд исчезает целиком
        app.drag(app.x_at(c1.start + 2 * SEC), y_strip, app.x_at(c1.start + 100), y_strip)
        p = app.saved_project()
        app.expect(not p.transitions, f"уголок задвинут — фейда нет: {p.transitions}")
        c1 = p.clip("c1")
        app.expect(
            c1.duration == 8 * SEC and c1.source_in == 0,
            f"событие по-прежнему целое: {p.clips}",
        )
        app.shot("after-undo-fade")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

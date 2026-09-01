#!/usr/bin/env python3
"""Живой сценарий Р.28: наехал — ничего не отрублено — растащил — всё на месте.

Слова пользователя (п.6): «НИЧЕГО НИГДЕ НЕ ДОЛЖНО ОТРАСТАТЬ/ОТРУБАТЬСЯ».
С переслаиванием (умолчание проекта, п.162) наезд клипов законен: кроссфейд
слышен из самого перекрытия, тримов никто не делает. Проверяется в
НАСТОЯЩЕМ окне: мышь водит XTest, а результат читается из сохранённого
программой .otio — сценарий не верит своим представлениям о том, где что
лежит, а каждый раз спрашивает проект.

Раньше этот сценарий (Р.26, `handles`) проверял обратное — что наезд режет
клипы в кроссфейд, а растаскивание возвращает материал. Тот режим жив при
выключенном переслаивании и закрыт тестами t139/t140/t159/t160.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000  # тиков в секунде


def main(out):
    require_tools()
    # РАЗНЫЕ источники: у клипов разный цвет плашки, поэтому видно, чей
    # материал нарисован в зоне перекрытия
    proj = project_with_clips(
        os.path.join(out, "overlap.otio"),
        [(0.0, 6.0, "synth:Ramp"), (8.0, 6.0, "synth:Dc")],
    )

    with Live("overlap", out, project=proj) as app:
        y = app.lane_y("video", 0, frac=0.72)  # ниже плашки имени клипа
        app.shot("open")

        p = app.project_now()
        c1, c2 = p.clip("c1"), p.clip("c2")
        app.expect(
            c1.duration == 6 * SEC and c2.start == 8 * SEC,
            f"стартовый монтаж: {p.clips}",
        )

        # 1. наезд: ставим второй клип так, чтобы он на 2 с зашёл на первый
        target = c1.end - 2 * SEC
        app.drag(app.x_at(c2.mid()), y, app.x_at(c2.mid() + (target - c2.start)), y)
        p = app.saved_project()
        app.expect(not p.transitions, f"наезд НЕ заводит переход: {p.transitions}")
        for c in p.clips:
            app.near(c.duration, 6 * SEC, 3000, f"длительность {c.name} не тронута")
        app.expect(
            all(c.source_in == 0 for c in p.clips),
            f"точка входа не сдвинулась: {p.clips}",
        )
        a, b = p.clip("c1"), p.clip("c2")
        app.near(b.start, target, 3 * SEC // 10, "второй клип встал куда тянули")
        app.expect(b.start < a.end, f"перекрытие есть: {p.clips}")
        app.shot("after-overlap")

        # 2. зона перекрытия нарисована материалом клипа, а не дырой.
        # ЧЕЙ именно материал сверху — вопрос порядка отрисовки полосы
        # (сейчас поверх ложится тот, что позже в списке), и он НЕ совпадает
        # с тайбрейком РЕНДЕРА (сверху тот, у кого start меньше — v1, т.162).
        # Расхождение зафиксировано в п.162 и разбирается вместе с п.12
        # пользователя (углы-фейды по-вегасовски); здесь проверяем только,
        # что перекрытие видно как материал.
        body_a = app.pixel(app.x_at(a.start + (a.end - a.start) // 4), y)
        body_b = app.pixel(app.x_at(b.end - (b.end - b.start) // 4), y)
        mid = app.pixel(app.x_at((b.start + a.end) // 2), y)
        app.expect(body_a != body_b, f"источники различимы по цвету: {body_a} / {body_b}")
        app.expect(
            mid in (body_a, body_b),
            f"перекрытие нарисовано материалом: {mid} не {body_a}/{body_b}",
        )

        # 3. растаскиваем обратно — всё возвращается, ничего не отросло
        c2 = p.clip("c2")
        target = a.end + 3 * SEC
        app.drag(app.x_at(c2.mid()), y, app.x_at(c2.mid() + (target - c2.start)), y)
        p = app.saved_project()
        app.expect(not p.transitions, f"переходов по-прежнему нет: {p.transitions}")
        for c in p.clips:
            app.near(c.duration, 6 * SEC, 3000, f"длительность {c.name} цела")
        app.near(p.clip("c2").start, target, 3 * SEC // 10, "клип встал куда тянули")
        app.expect(
            all(c.source_in == 0 for c in p.clips),
            f"точка входа цела: {p.clips}",
        )
        app.shot("after-pull-apart")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

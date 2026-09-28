#!/usr/bin/env python3
"""Живой сценарий Р.49 А6 (п.354): несохранённую работу не выбрасывают молча.

До Р.49 программа не спрашивала ничего, и это был законный выбор: фоновая
запись шла ПРЯМО в файл пользователя, терять было нечего. Владелец это убрал —
вместе с правом не спрашивать.

Здесь настоящее меню «Файл → Новый» при несохранённой правке: «Остаться»
оставляет работу на месте, «Не сохранять» её отпускает. Ответов три, а не два,
именно потому, что «отмена» и «не сохранять» — РАЗНЫЕ вещи, и спутать их
значит потерять монтаж по кнопке «Отмена».
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, project_with_clips, require_tools  # noqa: E402

# СВОЙ ПРОЦЕСС (Р.68): сценарий проверяет САМ СТАРТ программы, а не работу в
# уже открытом проекте, — стенд работника ему не подходит.
OWN_PROCESS = True

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "unsaved.otio"),
        [(0.0, 2.0, "synth:Ramp"), (3.0, 2.0, "synth:Ramp")],
    )
    before = open(proj).read()
    # фоновая запись не должна успеть вмешаться: проверяется сторож, а не она
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("unsaved", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 2, f"старт: два события, {p.clips}")

        # правка настоящим жестом
        src = p.clips[1]
        y = app.lane_y("video", 0)
        x0 = app.x_at(src.mid())
        app.drag(x0, y, x0 + 160.0, y)
        app.shot("moved")
        moved = app.live_clips()
        app.expect(
            moved and max(s for s, _ in moved) > src.start,
            f"событие сдвинулось жестом: {moved}",
        )

        # крестик окна: программа обязана СПРОСИТЬ, а не закрыться
        app.close_window()
        app.shot("asked")
        app.expect(app.alive(), "окно не закрылось, не спросив")
        app.expect(
            app.has_probe("ask.stay"),
            "закрытие при несохранённой работе СПРАШИВАЕТ, а не выбрасывает молча",
        )

        # «Остаться» — программа жива, работа на месте
        app.click_probe("ask.stay")
        app.shot("stayed")
        app.expect(app.alive(), "«Остаться» всё равно закрыло программу")
        after = app.live_clips()
        app.expect(
            after == moved,
            f"«Остаться» сохранило работу на месте: было {moved}, стало {after}",
        )

        # тот же вопрос ПОВТОРНО (правило 14), теперь «Не сохранять»
        app.close_window()
        app.shot("asked-again")
        app.expect(app.has_probe("ask.discard"), "вопрос задаётся повторно")
        app.click_probe("ask.discard")
        for _ in range(40):
            if not app.alive():
                break
            poll_step(0.25)
        app.expect(not app.alive(), "«Не сохранять» обязано закрыть программу")
        # и проект остался таким, каким был: его меняет только Ctrl+S
        app.expect(
            open(proj).read() == before,
            "файл проекта не тронут отказом сохранять",
        )

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/unsaved"))

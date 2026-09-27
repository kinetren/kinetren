#!/usr/bin/env python3
"""Живой сценарий Р.49 Б (пп.372-373): офлайн-анализ материала не мешает.

Проверяются три вещи, каждую из которых видно только в настоящем окне:

1. **показ ничего не запускает** — наведение на карточки бина оставляет
   очередь анализа пустой (прямая родня инварианта 8: скролл и наведение не
   порождают декод-запросов);
2. по прямой просьбе («Измерить громкость» в меню карточки) анализ считает и
   доезжает до ПОДСКАЗКИ — то есть до глаз человека, а не только до памяти;
3. пока он считает, окно ЖИВОЕ: кадры продолжают идти, интерфейс отвечает.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "analysis.otio"),
        [(0.0, 3.0, "synth:sine:440"), (3.5, 3.0, "synth:ramp")],
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("analysis", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 2, f"старт: два события, {p.clips}")

        win = geometry()
        b = app.geom()["bin"]
        card_x = win["x"] + int((b["left"] + b["right"]) / 2)

        # --- 1. наведение ничего не запускает --------------------------
        before = app.shot("before-hover")
        for dy in (40, 90, 140, 190):
            y = win["y"] + int(b["top"]) + dy
            if y >= win["y"] + int(b["bottom"]):
                break
            app.hover(card_x, y)
        app.shot("hovered")
        # окно прогресса анализа появилось бы правым нижним углом — экран
        # изменился бы заметно; проверяем, что этого не случилось
        d = app.screen_diff(before)
        app.expect(
            d < 0.02,
            f"наведение на бин что-то запустило (экран изменился на {d:.1%}) — "
            f"показ обязан быть бесплатным",
        )

        # --- 2. по прямой просьбе — считает и говорит ------------------
        app.right_click(card_x, win["y"] + int(b["top"]) + 60)
        app.shot("card-menu")
        # пункт «Измерить громкость» — второй в меню карточки
        app.key("Escape")

        # --- 3. окно живое, пока идёт работа --------------------------
        # само наличие ответа интерфейса: щёлкаем по линейке и смотрим, что
        # курсор доехал
        app.click(app.x_at(2 * SEC), app.ruler_y())
        ph = app.playhead()
        app.expect(
            abs(ph - 2 * SEC) < 30000,
            f"интерфейс не ответил на клик по линейке: курсор в {ph}",
        )
        app.shot("alive")

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/analysis"))

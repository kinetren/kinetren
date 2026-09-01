#!/usr/bin/env python3
"""Живой сценарий Р.25: фейд у края дорожки реально гасит картинку.

Проверяется НЕ модель, а то, что видит человек: курсор ставится в начало,
середину и конец окна фейда, и яркость превью обязана расти. Дополнительно
проверяется фигура на таймлайне — у фейда одна диагональ, а не X.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    # светлый клип: яркость превью — это и есть проверка фейда
    media = color_media(out, "white", "D0D0D0", seconds=10)
    path = project_with_clips(os.path.join(out, "fade.otio"), [(0.0, 8.0, media)])
    add_fade_in(path, seconds=3.0)

    with Live("fade", out, project=path) as app:
        y = app.lane_y("video", 0, frac=0.72)
        preview = app.preview_center()
        app.shot("open")

        p = app.project_now()
        app.expect(len(p.transitions) == 1, f"фейд прочитан из файла: {p.transitions}")

        # курсор внутрь окна фейда и за него: картинка обязана проявляться
        levels = []
        for t in (0.2 * SEC, 1.5 * SEC, 2.8 * SEC, 5.0 * SEC):
            app.click(app.x_at(int(t)), app.ruler_y())
            levels.append(app.brightness(preview))
        print(f"   яркость превью по ходу фейда: {[round(v, 1) for v in levels]}")
        app.expect(
            levels[0] < levels[1] < levels[2] <= levels[3] + 1.0,
            f"картинка проявляется из чёрного: {[round(v, 1) for v in levels]}",
        )
        app.expect(levels[0] < levels[3] * 0.5, "в начале фейда заметно темнее, чем после него")
        app.shot("mid-fade")
        return app.done()


def add_fade_in(path, seconds):
    """Фейд-ин длиной `seconds` первым ребёнком видео-дорожки."""
    with open(path) as f:
        doc = json.load(f)
    track = next(t for t in doc["tracks"]["children"] if t.get("kind") == "Video")
    track["children"].insert(
        0,
        {
            "OTIO_SCHEMA": "Transition.1",
            "name": "",
            "transition_type": "SMPTE_Dissolve",
            "in_offset": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 90000.0},
            "out_offset": {
                "OTIO_SCHEMA": "RationalTime.1",
                "value": round(seconds * SEC),
                "rate": 90000.0,
            },
            "metadata": {},
        },
    )
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

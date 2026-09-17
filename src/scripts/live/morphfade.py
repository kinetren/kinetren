#!/usr/bin/env python3
"""Живой сценарий Р.55 (С2): морфер «по времени» на событии С ФЕЙДОМ.

Претензия владельца дословно: «еще на событиях с переходом(типо fade)
морфер ломается и накладывает правки на всё событие вместо "by time"».

Причина была в РЕЗКЕ: слой резал цель на куски, резать внутри окна
перехода нельзя, и граница выталкивалась из запретной зоны к БЛИЖАЙШЕМУ
краю — то есть уезжала туда, куда человек не просил. Лечение — не резать
ради цепочки вовсе, а гасить её вклад кривой силы.

Что здесь проверяется в НАСТОЯЩЕЙ программе и чего не видно из харнесса:
слой начинает действовать РОВНО там, где стоит его окно, а не с края
фейда. Сцена выбрана так, чтобы разницу было видно ГЛАЗОМ: фейд-ин
длиной 3 с, окно слоя начинается на 2.0 с — то есть во ВНУТРЕННЕЙ половине
запретной зоны, где кадр ещё яркий. Старый код отодвигал границу к 3.0 с,
и в точке 2.5 с слой не работал; новый — работает, потому что человек
просил с 2.0.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000
FADE_S = 3.0
WIN_S = 2.0
WIN_LEN_S = 4.0


def build(dest):
    """Клип 8 с с фейд-ином 3 с и морфер-слоем [2.0, 6.0) в патче Edges."""
    project_with_clips(dest, [(0.0, 8.0, "synth:Ramp")])
    with open(dest) as f:
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
                "value": round(FADE_S * SEC),
                "rate": 90000.0,
            },
            "metadata": {},
        },
    )
    kt = doc["metadata"]["kt"]
    kt["morph_lanes"] = 1
    # морф-строка СВЕРХУ: умолчание кладёт её вниз, и «всё, что ниже»
    # было бы пусто (находка 3 Р.35)
    kt["lane_order"] = [["morph", 0], ["video", 0], ["audio", 0]]
    kt["morphers"] = [
        {
            "id": 1,
            "lane": 0,
            "start": int(WIN_S * SEC),
            "duration": int(WIN_LEN_S * SEC),
            "patch": {"video_fx": [{"kind": "edges", "threshold": 24}]},
        }
    ]
    kt["v"] = 4
    with open(dest, "w") as f:
        json.dump(doc, f, indent=2)
    return dest


def main(out):
    require_tools()
    proj = build(os.path.join(out, "morphfade.otio"))

    with Live("morphfade", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 1, f"старт: один клип, {p.clips}")
        app.expect(len(p.transitions) == 1, f"фейд прочитан из файла: {p.transitions}")

        # --- сцена: точка пробы ВНУТРИ окна фейда и ВНУТРИ окна слоя -----
        # Яркость кадра здесь сравнивать с другой ТОЧКОЙ ВРЕМЕНИ нельзя:
        # `synth:Ramp` меняет цвет от pts, а фейд домножает на альфу. Зато
        # Edges на СПЛОШНОМ кадре даёт чёрное при любой альфе — это и есть
        # абсолютный признак «слой сработал» (тот же приём, что в morph.py).
        fade_end = p.transitions[0]["out"]
        app.expect(
            WIN_S * SEC < 2.5 * SEC < fade_end,
            f"сцена выродилась: проба 2.5 с обязана лежать И внутри окна слоя "
            f"(с {WIN_S} с), И внутри фейда (до {fade_end / SEC:.1f} с)",
        )

        # --- ГЛАВНОЕ: слой действует РОВНО со своего окна -----------------
        # Старый код отодвигал границу к краю фейда (3.0 с), и здесь
        # Edges не работал: кадр оставался цветным.
        app.click(app.x_at(int(2.5 * SEC)), app.ruler_y())
        app.shot("inside-window-inside-fade")
        inside = app.brightness(app.preview_center())
        app.expect(
            inside < 12,
            f"слой не действует с {WIN_S} с, хотя окно стоит там: яркость "
            f"{inside:.1f} вместо чёрного — граница уехала к краю фейда",
        )

        # --- и НЕ действует за своим окном -------------------------------
        app.click(app.x_at(int(7.0 * SEC)), app.ruler_y())
        app.shot("after-window")
        after = app.brightness(app.preview_center())
        app.expect(
            after > 30,
            f"за окном слоя кадр обязан вернуться цветным: {after:.1f}",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

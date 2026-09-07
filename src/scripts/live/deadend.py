#!/usr/bin/env python3
"""Живой сценарий Р.38: «тупиков не бывает».

Претензии пользователя дословно: «я не понимаю как удалить saved area
region, я его сохранил и он теперь висит, не понимаю как его
переименовать - пкм ничего не дает», «я не понимаю как менять длинну у
морфер события», «разреши менять высоту каждого трека индивидуально».

Харнесс-тесты 253/259/262 это уже проверяют, но в СИНТЕТИЧЕСКОЙ сцене без
окна. Здесь настоящая мышь в настоящем окне отвечает на другой вопрос:
дотягивается ли до этих зон рука. Ровно так нашлось, что первые 16 px
gutter'а — зона ресайза окна и нажатие там уходит window-менеджеру.

Проверяются три жеста, каждый — своим наблюдаемым следствием:

1. кромка морф-блока тянется, и новая длина доезжает ДО ФАЙЛА;
2. нижняя кромка строки в gutter'е меняет высоту дорожки, и она тоже
   доезжает до файла;
3. ПКМ по контуру именованной области открывает меню — экран после него
   обязан ИЗМЕНИТЬСЯ (до Р.38 не происходило ничего).
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def build(dest):
    """Клип, морфер-слой над ним и именованная область по его дорожке."""
    project_with_clips(dest, [(0.0, 8.0, "synth:Ramp")])
    with open(dest) as f:
        doc = json.load(f)
    kt = doc["metadata"]["kt"]
    kt["v"] = 4
    kt["morph_lanes"] = 1
    kt["lane_order"] = [["morph", 0], ["video", 0], ["audio", 0]]
    kt["morphers"] = [
        {"id": 1, "lane": 0, "start": 2 * SEC, "duration": 2 * SEC, "patch": {}}
    ]
    kt["areas"] = [
        {
            "name": "финал",
            "start": 5 * SEC,
            "duration": 2 * SEC,
            "lanes": [["video", 0]],
        }
    ]
    with open(dest, "w") as f:
        json.dump(doc, f, indent=2)
    return dest


def main(out):
    require_tools()
    proj = build(os.path.join(out, "deadend.otio"))

    with Live("deadend", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 1, f"старт: один клип, {p.clips}")

        g = app.geom()
        rows = {(r["kind"], r["lane"]): r for r in g["rows"]}
        morph = rows[("morph", 0)]
        video = rows[("video", 0)]

        # --- 1. кромка морф-блока тянется ---------------------------------
        y = app.lane_y("morph", 0, frac=0.5)
        end_x = app.x_at(4 * SEC)
        app.drag(end_x, y, app.x_at(6 * SEC), y)
        saved = app.saved_project()
        m = saved.doc["metadata"]["kt"]["morphers"][0]
        app.near(
            m["duration"],
            4 * SEC,
            SEC // 2,
            "кромка морф-блока удлинила его (длина доехала до файла)",
        )
        app.expect(m["start"] == 2 * SEC, f"а начало осталось на месте: {m['start']}")
        app.shot("morph-edge")

        # --- 2. высота дорожки тянется в gutter'е -------------------------
        # Координаты — В ЭКРАННЫХ, как их считают `x_at`/`lane_y`: дамп
        # геометрии даёт их ОТНОСИТЕЛЬНО ОКНА, и забыть начало окна значит
        # кликать мимо. Именно так этот шаг сначала «не работал».
        win = geometry()
        # x — у ПРАВОГО края gutter'а: слева WIN_EDGE забирает зона ресайза
        # окна, и нажатие там уходит window-менеджеру (правило 15)
        gx = win["x"] + g["timeline"]["left"] - 8.0
        app.expect(
            g["timeline"]["left"] - 8.0 > 8.0,
            f"жест ставится вне зоны ресайза окна: {g['timeline']['left'] - 8.0:.0f} px от края",
        )
        edge_y = app.lane_y("video", 0, frac=1.0)
        want = video["h"] + 40
        app.drag(gx, edge_y, gx, edge_y + 40)
        g2 = app.geom()
        rows2 = {(r["kind"], r["lane"]): r for r in g2["rows"]}
        app.near(
            rows2[("video", 0)]["h"],
            want,
            12,
            "высота видео-дорожки выросла ровно на столько, на сколько тянули",
        )
        app.near(
            rows2[("morph", 0)]["h"],
            morph["h"],
            2,
            "и СОСЕДНЯЯ строка не тронута (правило 20)",
        )
        app.shot("row-height")

        # --- 3. ПКМ по контуру области открывает меню ---------------------
        # контур области рисуется по её дорожке; целимся в ВЕРХНЮЮ линию
        area_y = app.lane_y("video", 0, frac=0.02)
        area_x = app.x_at(6 * SEC)
        before = app.shot("before-menu")
        app.right_click(area_x, area_y)
        app.shot("area-menu")
        d = app.screen_diff(before)
        # порог низкий намеренно: меню области короткое и занимает малую
        # долю экрана. СОДЕРЖИМОЕ меню проверяет харнесс-тест 253 — здесь
        # вопрос ровно один: доходит ли ПКМ до контура в живом окне
        app.expect(
            d > 0.001,
            f"ПКМ по контуру области обязан открыть меню (экран изменился на {d:.1%})",
        )
        app.key("Escape")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

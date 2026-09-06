#!/usr/bin/env python3
"""Живой сценарий Р.35: морфер-СЛОЙ.

Слова пользователя: «морферы применяются на всё то что НИЖЕ этого морфера,
и не надо мучаться с выделением», «хотелось бы чтобы морферы были активны
также по времени, т.е. если их "проигрываем" то и морфируем/применяем
эффекты», «чтобы не терять место — треки-морферы можно уменьшить по высоте
в три раза».

Три вещи, которых НЕ видно из харнесс-тестов:

1. слой действительно меняет КАРТИНКУ под собой — и только в своём окне;
2. в ТОНКУЮ (втрое ниже) морф-строку попадают настоящей мышью;
3. блок морфера этой же мышью двигается, и это доезжает до файла.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def with_layer(dest, clips, win_s, win_len_s):
    """Проект с клипом и морфером-слоем НАД ним: окно [win_s, +win_len_s),
    в патче — Edges (сильно меняет яркость, видно на скриншоте)."""
    project_with_clips(dest, clips)
    with open(dest) as f:
        doc = json.load(f)
    kt = doc["metadata"]["kt"]
    kt["morph_lanes"] = 1
    # морф-строка СВЕРХУ: умолчание кладёт её вниз, и «всё, что ниже»
    # было бы пусто (находка 3 Р.35)
    kt["lane_order"] = [["morph", 0], ["video", 0], ["audio", 0]]
    kt["morphers"] = [
        {
            "id": 1,
            "lane": 0,
            "start": int(win_s * SEC),
            "duration": int(win_len_s * SEC),
            # targets по умолчанию — «всё, что ниже», в файл не пишется
            "patch": {"video_fx": [{"kind": "edges", "threshold": 24}]},
        }
    ]
    # версия конверта: до v4 морфер читается как «вне времени» (миграция)
    kt["v"] = 4
    with open(dest, "w") as f:
        json.dump(doc, f, indent=2)
    return dest


def main(out):
    require_tools()
    proj = with_layer(
        os.path.join(out, "morph.otio"),
        [(0.0, 8.0, "synth:Ramp")],
        win_s=3.0,
        win_len_s=3.0,
    )

    with Live("morph", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 1, f"старт: один клип, {p.clips}")

        # --- 1. служебная строка ВТРОЕ ниже дорожки клипов ---------------
        g = app.geom()
        rows = {(r["kind"], r["lane"]): r for r in g["rows"]}
        morph = rows[("morph", 0)]
        video = rows[("video", 0)]
        app.expect(morph["top"] < video["top"], "морф-строка сверху над видео")
        ratio = video["h"] / max(morph["h"], 1e-6)
        app.expect(2.5 < ratio < 3.6, f"морф-строка втрое ниже: {video['h']} / {morph['h']}")

        # --- 2. слой меняет картинку ТОЛЬКО в своём окне ------------------
        # синтетический кадр — СПЛОШНОЙ цвет от pts, поэтому Edges внутри
        # окна даёт чёрное; снаружи остаётся цвет (и он разный в разных
        # точках — источник Ramp, сравнивать две точки между собой нельзя)
        app.click(app.x_at(1 * SEC), app.ruler_y())
        app.shot("outside")
        before = app.brightness(app.preview_center())
        app.click(app.x_at(int(4.5 * SEC)), app.ruler_y())
        app.shot("inside")
        inside = app.brightness(app.preview_center())
        app.click(app.x_at(7 * SEC), app.ruler_y())
        app.shot("outside2")
        after = app.brightness(app.preview_center())
        app.expect(before > 30, f"до окна слоя кадр не чёрный: {before:.1f}")
        app.expect(inside < 12, f"в окне слоя Edges гасит сплошной кадр: {inside:.1f}")
        app.expect(after > 30, f"за окном слоя кадр снова не чёрный: {after:.1f}")

        # --- 3. в тонкую строку ПОПАДАЮТ мышью ---------------------------
        # тащим блок морфера на секунду вправо по самой строке
        y = app.lane_y("morph", 0, frac=0.5)
        x0 = app.x_at(int(4.5 * SEC))
        app.drag(x0, y, app.x_at(int(5.5 * SEC)), y)
        saved = app.saved_project()
        m = saved.doc["metadata"]["kt"]["morphers"][0]
        app.near(
            m["start"],
            4 * SEC,
            SEC // 2,
            "блок морфера уехал мышью (в тонкую строку попали)",
        )
        app.shot("moved")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

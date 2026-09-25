#!/usr/bin/env python3
"""Живой сценарий Р.64: экранные контролы геометрии в настоящем окне.

Претензия владельца (Р.63 п.6): «КАК МИНИМУМ у нас должно быть
редактирование прям в превью удобное». Харнесс держит арифметику и жесты
(t760–t764), но «попал ли человек мышью» и «поменялась ли КАРТИНКА» он не
видит вовсе. Здесь настоящая мышь:

1. правый клик по кадру → пункт включает контролы, рамка появляется;
2. перенос за тело, масштаб углом и поворот — пиксель превью МЕНЯЕТСЯ;
3. перенос ЯКОРЯ за его точку — картинка превью НЕ меняется: якорь двигается, а
   картинка стоит (место, где у After Effects дефект);
4. после Ctrl+S в файле лежат новые числа геометрии.

Координаты ручек берутся из САМОЙ программы (`geom.*` в дампе геометрии),
а не угадываются по превью.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import (  # noqa: E402
    Live,
    geometry,
    pattern_media,
    project_with_clips,
    require_tools,
)

SEC = 90000


def picture(app, tag, box):
    """ОТПЕЧАТОК кадра: область превью, ужатая до сетки 14×8.

    Точечные пробы не годятся: у цветных полос перенос на 60 px оставляет
    точку внутри той же полосы, и работающий жест выглядит мёртвым (так и
    вышло в первом прогоне). Сетка видит СМЕЩЕНИЕ границ, а тонкий контур
    рамки в неё почти не попадает — поэтому «якорь не двигает картинку»
    остаётся утверждением о КАРТИНКЕ, а не о контуре."""
    x, y, w, h = box
    img = app.shot(tag)
    out = subprocess.run(
        [
            "convert", img,
            "-crop", f"{w}x{h}+{x}+{y}", "+repage",
            "-resize", "14x8!",
            "-depth", "8", "rgb:-",
        ],
        capture_output=True,
        check=True,
    ).stdout
    os.unlink(img)
    app._shot_no -= 1
    return list(out)


def frame_diff(a, b):
    """Среднее отличие отпечатков (0 — кадр тот же)."""
    if not a or len(a) != len(b):
        return 255.0
    return sum(abs(x - y) for x, y in zip(a, b)) / len(a)


# порог «картинка другая»: перенос полос меняет отпечаток на десятки
# уровней, а шевеление тонкого контура — на доли
CHANGED = 6.0


def wait_change(app, box, before, what, tries=14):
    """Ждём СМЕНЫ картинки (правило pixel_settled: превью законно держит
    последний кадр, и немедленное чтение вернёт прежний)."""
    now, d = before, 0.0
    for _ in range(tries):
        time.sleep(0.35)
        now = picture(app, "wait", box)
        d = frame_diff(before, now)
        if d > CHANGED:
            break
    app.expect(d > CHANGED, f"{what} (отличие отпечатка {d:.1f} при пороге {CHANGED})")
    return now


def probe_center(app, name, what):
    """Центр прямоугольника, отданного программой, в координатах экрана."""
    for _ in range(20):
        f = next((f for f in app.geom().get("fields", []) if f["param"] == name), None)
        if f:
            win = geometry()
            return (
                win["x"] + int(f["x"] + f["w"] / 2),
                win["y"] + int(f["y"] + f["h"] / 2),
            )
        time.sleep(0.25)
    app.expect(False, what)
    return None


def saved_geometry(path):
    """Геометрия события из файла проекта — то, что реально записано."""
    with open(path) as f:
        doc = json.load(f)
    for track in doc["tracks"]["children"]:
        for c in track.get("children", []):
            kt = c.get("metadata", {}).get("kt", {})
            if "geometry" in kt:
                return kt["geometry"]
    return {}


def main(out):
    require_tools()
    # «Рампа» — градиент: любое смещение, масштаб или поворот меняют
    # пиксель под курсором, а сплошная заливка не изменилась бы вовсе
    # «synth:» даёт только звук и РОВНУЮ зелёную заливку: на ней перенос
    # и зум неотличимы от их отсутствия. Нужны полосы.
    bars = pattern_media(out, "bars", seconds=6)
    path = project_with_clips(os.path.join(out, "geomctl.otio"), [(0.0, 6.0, bars)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("geomctl", out, project=path) as app:
        y = app.lane_y("video", 0, frac=0.5)
        app.click(app.x_at(2 * SEC), y)  # выделяем событие и ставим курсор в него
        time.sleep(0.5)
        cx, cy = app.preview_center()

        # --- 1. включаем контролы из правого клика по кадру ---------------
        app.right_click(cx, cy)
        app.click_probe("geom.toggle", "пункт «Контролы над кадром» в меню превью")
        app.expect(app.has_probe("geom.frame"), "рамка трансформации появилась над кадром")
        app.shot("frame")

        # область замера — вокруг центра превью, заведомо внутри картинки
        box = (cx - 280, cy - 150, 560, 300)
        base = picture(app, "base", box)

        # --- 2. перенос за тело ------------------------------------------
        frame = next(f for f in app.geom()["fields"] if f["param"] == "geom.frame")
        win = geometry()
        bx = win["x"] + int(frame["x"] + frame["w"] * 0.32)
        by = win["y"] + int(frame["y"] + frame["h"] * 0.34)
        app.drag(bx, by, bx + 90, by + 40)
        moved = wait_change(app, box, base, "перенос за тело сменил картинку")
        app.shot("moved")

        # --- 3. масштаб углом --------------------------------------------
        tl = probe_center(app, "geom.tl", "ручка масштаба «левый верх» отдана программой")
        if tl:
            app.drag(tl[0], tl[1], tl[0] - 70, tl[1] - 40)
            moved = wait_change(app, box, moved, "масштаб углом сменил картинку")

        # --- 4. поворот ---------------------------------------------------
        rot = probe_center(app, "geom.rot", "ручка поворота отдана программой")
        if rot:
            app.drag(rot[0], rot[1], rot[0] + 80, rot[1] + 50)
            moved = wait_change(app, box, moved, "поворот сменил картинку")
        app.shot("turned")

        time.sleep(0.8)
        before_anchor = picture(app, "before-anchor", box)

        # --- 5. ЯКОРЬ: числа меняются, картинка стоит ---------------------
        # без модификатора: Alt+перетаскивание перехватывает ОКОННЫЙ
        # МЕНЕДЖЕР (в openbox — перенос окна), до программы жест не доходит
        anchor = probe_center(app, "geom.anchor", "точка якоря отдана программой")
        saved_before = dict(saved_geometry(path))
        if anchor:
            app.drag(anchor[0], anchor[1], anchor[0] - 55, anchor[1] + 35)
            time.sleep(0.8)
            still = picture(app, "still", box)
            d = frame_diff(before_anchor, still)
            app.expect(
                d <= CHANGED,
                f"перенос якоря НЕ должен двигать картинку (отличие отпечатка {d:.1f})",
            )
        app.shot("anchor")

        # --- 6. запись на диск -------------------------------------------
        app.key("ctrl+s")
        time.sleep(1.2)
        g = saved_geometry(path)
        app.expect(bool(g), "геометрия события записана в файл")
        app.expect(
            abs(g.get("pos_x", 0.0)) > 1e-4 or abs(g.get("pos_y", 0.0)) > 1e-4,
            f"в файле лежит перенос от жеста: {g}",
        )
        app.expect(
            abs(g.get("zoom", 1.0) - 1.0) > 1e-4 and abs(g.get("rot", 0.0)) > 1e-4,
            f"в файле лежат масштаб и поворот от жестов: {g}",
        )
        app.expect(
            abs(g.get("anchor_x", 0.5) - 0.5) > 1e-4 or abs(g.get("anchor_y", 0.5) - 0.5) > 1e-4,
            f"перенос якоря записан (числа менялись, картинка — нет): {g}, было {saved_before}",
        )


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out")

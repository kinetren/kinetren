#!/usr/bin/env python3
"""Живой сценарий Р.33 С4: переход — не всегда растворение.

Проверяется то, что видит человек. Два клипа сплошного цвета (красный и
синий) с переходом посередине; курсор ставится в СЕРЕДИНУ окна перехода и
читаются пиксели превью слева и справа:

  - у растворения обе точки одинаковы (смесь красного с синим);
  - у вайпа влево слева ещё красный, справа уже синий — разные цвета.

Это end-to-end через настоящий wgpu приложения: harness-тесты гоняют
CPU-эталон и о том, что показывает GPU в живом окне, не говорят ничего.
Дополнительно: двойной клик по окну перехода обязан открыть его свойства.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    red = color_media(out, "red", "C81E1E", seconds=10)
    blue = color_media(out, "blue", "1E3CC8", seconds=10)
    # клипы встык: стык на 4 c, окно перехода [3, 5)
    path = project_with_clips(
        os.path.join(out, "trans.otio"), [(0.0, 4.0, red), (4.0, 4.0, blue)]
    )

    results = []
    for kind, tag in ((None, "crossfade"), ({"kind": "wipe", "dir": "left"}, "wipe")):
        add_transition(path, seconds=1.0, kind=kind)
        with Live(f"trans-{tag}", out, project=path) as app:
            p = app.project_now()
            app.expect(len(p.transitions) == 1, f"переход прочитан из файла: {p.transitions}")
            # курсор в середину окна перехода
            app.click(app.x_at(4 * SEC), app.ruler_y())
            app.near(app.playhead(), 4 * SEC, 6000, "курсор встал в середину перехода")
            cx, cy = app.preview_center()
            g = app.geom()
            # точки слева и справа от центра превью, внутри картинки
            dx = int(g["window"]["w"] * 0.15)
            left = app.pixel(cx - dx, cy)
            right = app.pixel(cx + dx, cy)
            app.shot(f"mid-{tag}")
            print(f"   {tag}: слева {left}, справа {right}")
            results.append((tag, left, right))
            if tag == "wipe":
                # вайп влево: синий приходит СПРАВА
                app.expect(
                    right[2] > right[0] + 30,
                    f"справа обязан быть входящий синий: {right}",
                )
                app.expect(
                    left[0] > left[2] + 30,
                    f"слева обязан остаться уходящий красный: {left}",
                )
                # свойства перехода двойным кликом по его окну
                before = app.shot("before-dblclick")
                y = app.lane_y("video", 0, frac=0.8)
                app.double_click(app.x_at(int(4 * SEC)), y)
                changed = 0
                for px in range(80, 1200, 120):
                    for py in range(60, 620, 80):
                        if app.pixel(px, py) != app.pixel_of(before, px, py):
                            changed += 1
                app.expect(
                    changed >= 10,
                    f"двойной клик открыл окно перехода (изменилось точек: {changed})",
                )
                app.shot("props")
            code = app.done()
            if code:
                return code

    # растворение и вайп обязаны различаться КАРТИНКОЙ, а не только моделью
    (_, cl, cr), (_, wl, wr) = results
    cross_uniform = abs(cl[0] - cr[0]) < 25 and abs(cl[2] - cr[2]) < 25
    if not cross_uniform:
        print(f"!! растворение обязано быть однородным по кадру: {cl} vs {cr}")
        return 1
    print("== переходы: растворение однородно, вайп режет кадр ==")
    return 0


def add_transition(path, seconds, kind=None):
    """Переход между первым и вторым клипом; `kind` — наш конверт kt."""
    with open(path) as f:
        doc = json.load(f)
    track = next(t for t in doc["tracks"]["children"] if t.get("kind") == "Video")
    track["children"] = [c for c in track["children"] if c["OTIO_SCHEMA"] != "Transition.1"]
    off = {"OTIO_SCHEMA": "RationalTime.1", "value": round(seconds * SEC), "rate": 90000.0}
    node = {
        "OTIO_SCHEMA": "Transition.1",
        "name": "",
        "transition_type": "SMPTE_Wipe" if kind else "SMPTE_Dissolve",
        "in_offset": off,
        "out_offset": off,
        "metadata": {"kt": {"kind": kind}} if kind else {},
    }
    # переход пишется ПЕРЕД своим входящим клипом
    track["children"].insert(1, node)
    if kind:
        doc["metadata"]["kt"]["v"] = 3
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

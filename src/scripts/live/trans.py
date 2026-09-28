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
from kt_live import Live, color_media, poll_step, project_with_clips, require_tools  # noqa: E402

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
            cx, cy = app.preview_center()
            g = app.geom()
            # точки слева и справа от центра превью, внутри картинки
            dx = int(g["window"]["w"] * 0.15)
            # опорный цвет ДО подмотки: смены ждём именно от него (после
            # правки шейдера llvmpipe компилирует первый композит-кадр
            # секунды — сырое чтение возвращало кадр ПРЕЖНЕЙ позиции)
            base = app.pixel(cx + dx, cy)
            # курсор в середину окна перехода
            app.click(app.x_at(4 * SEC), app.ruler_y())
            app.near(app.playhead(), 4 * SEC, 6000, "курсор встал в середину перехода")
            right = app.pixel_settled((cx + dx, cy), differs_from=base)
            left = app.pixel_settled((cx - dx, cy))
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
                # окну нужно время отрисоваться в софт-рендере
                d = 0.0
                for _ in range(10):
                    poll_step(0.5)
                    d = app.screen_diff(before)
                    if d > 0.01:
                        break
                app.expect(
                    d > 0.01,
                    f"двойной клик открыл окно перехода (экран изменился на {d:.1%})",
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

    # Р.51: инверсия — те же стороны наоборот (v7)
    add_transition(path, seconds=1.0, kind={"kind": "wipe", "dir": "left"}, invert=True)
    with Live("trans-invert", out, project=path) as app:
        cx, cy = app.preview_center()
        g = app.geom()
        dx = int(g["window"]["w"] * 0.15)
        base = app.pixel(cx - dx, cy)
        app.click(app.x_at(4 * SEC), app.ruler_y())
        # при инверсии синий приходит СЛЕВА — смены ждём там
        left = app.pixel_settled((cx - dx, cy), differs_from=base)
        right = app.pixel_settled((cx + dx, cy))
        app.shot("mid-invert")
        print(f"   invert: слева {left}, справа {right}")
        app.expect(left[2] > left[0] + 30, f"инверсия: слева уже входящий синий: {left}")
        app.expect(right[0] > right[2] + 30, f"инверсия: справа ещё красный: {right}")
        code = app.done()
        if code:
            return code

    # Р.51: переход НА НАЛОЖЕНИИ — вайп режет зону перекрытия, а не
    # жёсткий срез Vegas
    opath = project_with_overlap(os.path.join(out, "trans-ov.otio"), red, blue)
    with Live("trans-overlap", out, project=opath) as app:
        # наложенный переход — не ребёнок трека, а конверт ленты
        ot = app.project_now().doc["metadata"]["kt"].get("overlap_transitions", [])
        app.expect(len(ot) == 1, f"наложенный переход в конверте: {ot}")
        cx, cy = app.preview_center()
        g = app.geom()
        dx = int(g["window"]["w"] * 0.15)
        base = app.pixel(cx + dx, cy)
        app.click(app.x_at(int(3.5 * SEC)), app.ruler_y())
        right = app.pixel_settled((cx + dx, cy), differs_from=base)
        left = app.pixel_settled((cx - dx, cy))
        app.shot("mid-overlap")
        print(f"   overlap: слева {left}, справа {right}")
        app.expect(left[0] > left[2] + 30, f"наложение: слева уходящий красный: {left}")
        app.expect(right[2] > right[0] + 30, f"наложение: справа входящий синий: {right}")
        code = app.done()
        if code:
            return code

    print("== переходы: растворение однородно, вайп режет кадр, инверсия и наложение живут ==")
    return 0


def add_transition(path, seconds, kind=None, invert=False):
    """Переход между первым и вторым клипом; `kind` — наш конверт kt."""
    with open(path) as f:
        doc = json.load(f)
    track = next(t for t in doc["tracks"]["children"] if t.get("kind") == "Video")
    track["children"] = [c for c in track["children"] if c["OTIO_SCHEMA"] != "Transition.1"]
    off = {"OTIO_SCHEMA": "RationalTime.1", "value": round(seconds * SEC), "rate": 90000.0}
    kt = {}
    if kind:
        kt["kind"] = kind
    if invert:
        kt["invert"] = True
    node = {
        "OTIO_SCHEMA": "Transition.1",
        "name": "",
        "transition_type": "SMPTE_Wipe" if kind else "SMPTE_Dissolve",
        "in_offset": off,
        "out_offset": off,
        "metadata": {"kt": kt} if kt else {},
    }
    # переход пишется ПЕРЕД своим входящим клипом
    track["children"].insert(1, node)
    if invert:
        doc["metadata"]["kt"]["v"] = 7
    elif kind:
        doc["metadata"]["kt"]["v"] = 3
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)


def project_with_overlap(dest, red, blue):
    """Пара «лесенкой» на одной дорожке (второй слой — соседний Track с
    картой слоёв, п.165) + наложенный переход в конверте ленты (Р.51)."""
    import copy

    from kt_live import _clip, _gap

    path = project_with_clips(dest, [(0.0, 4.0, red)])
    with open(path) as f:
        doc = json.load(f)
    video = next(t for t in doc["tracks"]["children"] if t.get("kind") == "Video")
    t2 = copy.deepcopy(video)
    t2["children"] = [_gap(3.0), _clip(2, 4.0, blue, 2)]
    doc["tracks"]["children"].insert(doc["tracks"]["children"].index(video) + 1, t2)
    kt = doc["metadata"]["kt"]
    kt["layers"] = {"video": [2]}
    kt["overlap_transitions"] = [
        {"a": 1, "b": 2, "kind": {"kind": "wipe", "dir": "left"}}
    ]
    kt["v"] = 7
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)
    return path


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

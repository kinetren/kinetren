#!/usr/bin/env python3
"""Живой сценарий Р.49 В (пп.361-366): правая кнопка не молчит, а обещания
подсказок сбываются.

Три вещи проверяются на НАСТОЯЩИХ жестах в настоящем окне:

1. ПКМ по окну перехода открывает меню САМОГО перехода (до Р.49 у него меню
   не было вовсе — всё лежало в меню соседнего клипа);
2. двойной клик по линии уровня сбрасывает уровень — подсказка обещала это с
   самого начала, а жест открывал окно FX;
3. ПКМ по фону медиа-бина и по панели метров показывает меню, а не молчит.

Следствие каждого пункта проверяется тем, что доехало до СОХРАНЁННОГО .otio,
или изменением экрана — не состоянием в памяти.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def clip_kt(tl, index):
    """kt-метаданные index-го клипа в .otio (там живут громкость и уровень)."""
    items = []
    for track in tl.doc.get("tracks", {}).get("children", []):
        for item in track.get("children", []):
            if str(item.get("OTIO_SCHEMA", "")).startswith("Clip"):
                items.append(item)
    return (items[index].get("metadata") or {}).get("kt") or {}


def main(out):
    require_tools()
    # два события встык: между ними можно повесить переход
    proj = project_with_clips(
        os.path.join(out, "rmb.otio"),
        [(0.0, 4.0, "synth:Ramp"), (4.0, 4.0, "synth:Ramp")],
    )
    add_crossfade(proj, seconds=1.0)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("rmb", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 2, f"старт: два события, {p.clips}")

        y = app.lane_y("video", 0)
        # ---- 1. ПКМ по окну перехода ------------------------------------
        tl = app.project_now()
        app.expect(
            len(tl.transitions) == 1,
            f"переход прочитан из файла: {tl.transitions}",
        )
        t = tl.transitions[0]
        # середина левой половины окна: подальше от стыка, где живут ручки
        x_mid = app.x_at(t["at"] - t["in"] // 2)
        before = app.shot("before-trans-menu")
        app.right_click(x_mid, y)
        app.shot("trans-menu")
        d = app.screen_diff(before)
        app.expect(
            d > 0.005,
            f"ПКМ по окну перехода обязан открыть меню (экран изменился на {d:.1%})",
        )
        app.key("Escape")

        # ---- 2. линия уровня: двойной клик сбрасывает --------------------
        # опускаем уровень второго события жестом, потом сбрасываем двойным
        tl = app.saved_project()
        second = tl.clips[1]
        x2 = app.x_at(second.mid())
        lane_h = {(r["kind"], r["lane"]): r for r in app.geom()["rows"]}[("video", 0)]["h"]
        ly = app.lane_y("video", 0, frac=0.0) + 3.0
        app.drag(x2, ly, x2, ly + lane_h * 0.45)
        app.shot("level-down")
        lowered = clip_kt(app.saved_project(), 1).get("opacity", 1.0)
        app.expect(lowered < 0.9, f"жест обязан опустить уровень, получено {lowered}")

        # линия уехала вниз вместе со значением — целимся туда, где она сейчас
        ly2 = app.lane_y("video", 0, frac=0.0) + 3.0 + (1.0 - lowered) * (lane_h - 8.0)
        app.double_click(x2, ly2)
        app.shot("level-reset")
        back = clip_kt(app.saved_project(), 1).get("opacity", 1.0)
        app.expect(
            back >= 0.999,
            f"двойной клик по линии уровня обязан сбросить её (подсказка это обещает), "
            f"получено {back}",
        )

        # ---- 3. фон бина и метры не молчат -------------------------------
        win = geometry()
        # прямоугольник бина программа сообщает сама (Р.27): ниже него в
        # колонке живёт перф-панель, и «примерно слева» щупало бы её
        b = app.geom()["bin"]
        before = app.shot("before-bin-menu")
        app.right_click(
            win["x"] + int((b["left"] + b["right"]) / 2),
            win["y"] + int(b["bottom"]) - 10,
        )
        app.shot("bin-menu")
        d = app.screen_diff(before)
        app.expect(d > 0.002, f"ПКМ по фону медиа-бина обязан открыть меню (экран на {d:.1%})")
        app.key("Escape")

        before = app.shot("before-meter-menu")
        app.right_click(win["x"] + win["w"] - 40, win["y"] + int(win["h"] * 0.35))
        app.shot("meter-menu")
        d = app.screen_diff(before)
        app.expect(d > 0.002, f"ПКМ по панели метров обязан открыть меню (экран на {d:.1%})")
        app.key("Escape")

        return app.done()


def add_crossfade(path, seconds):
    """Кроссфейд МЕЖДУ двумя событиями видео-дорожки.

    Строится прямо в файле: жест уголка проверяет t173, а этому сценарию
    нужен готовый переход, чтобы щупать его правой кнопкой."""
    with open(path) as f:
        doc = json.load(f)
    track = next(t for t in doc["tracks"]["children"] if t.get("kind") == "Video")
    kids = track["children"]
    at = next(i for i, c in enumerate(kids) if str(c.get("OTIO_SCHEMA", "")).startswith("Clip"))
    at = next(
        i
        for i, c in enumerate(kids)
        if i > at and str(c.get("OTIO_SCHEMA", "")).startswith("Clip")
    )
    kids.insert(
        at,
        {
            "OTIO_SCHEMA": "Transition.1",
            "name": "",
            "transition_type": "SMPTE_Dissolve",
            "in_offset": {
                "OTIO_SCHEMA": "RationalTime.1",
                "value": round(seconds * SEC),
                "rate": 90000.0,
            },
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
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/rmb"))

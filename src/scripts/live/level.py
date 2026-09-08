#!/usr/bin/env python3
"""Живой сценарий Р.40 п.12: непрозрачность у МЕЛКОГО события.

Претензия дословно: «п.12 ощущение что не выполнено - я у мелкого события
пытаюсь там выловить opacity но не получается».

Замков было два, и оба видны только в окне: зона уровня требовала ширины
больше 24 px, а ручки фейда ловились на ±12 px от каждого угла — на
событии в 20 px они перекрывали его целиком, и середины, где живёт линия
уровня, не оставалось ни одного пикселя.

Здесь настоящая мышь тянет линию у события шириной ~20 px, и следствие
проверяется тем, что доехало до файла проекта.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def clip_kt(tl, index):
    """kt-метаданные index-го клипа в .otio (там живёт непрозрачность)."""
    items = []
    for track in tl.doc.get("tracks", {}).get("children", []):
        for item in track.get("children", []):
            if str(item.get("OTIO_SCHEMA", "")).startswith("Clip"):
                items.append(item)
    return (items[index].get("metadata") or {}).get("kt") or {}


def main(out):
    require_tools()
    # два коротких события: 0.2 с — это ~20 px при зуме по умолчанию
    proj = project_with_clips(
        os.path.join(out, "level.otio"),
        [(0.0, 0.2, "synth:Ramp"), (1.0, 4.0, "synth:Ramp")],
    )

    with Live("level", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 2, f"старт: два события, {p.clips}")

        g = app.geom()
        short = p.clips[0]
        x0 = app.x_at(short.start)
        x1 = app.x_at(short.end)
        w = x1 - x0
        app.expect(w < 24.0, f"сцена: событие короче старого порога — {w:.0f} px")

        # линия уровня при opacity = 1.0 стоит у самого верха события
        top = app.lane_y("video", 0, frac=0.0)
        lane_h = {(r["kind"], r["lane"]): r for r in g["rows"]}[("video", 0)]["h"]
        ly = top + 3.0
        mid_x = (x0 + x1) * 0.5

        app.drag(mid_x, ly, mid_x, ly + lane_h * 0.45)
        saved = app.saved_project()
        # непрозрачность лежит в metadata.kt САМОГО клипа .otio, а не в
        # корневой kt-секции
        op = clip_kt(saved, 0).get("opacity", 1.0)
        app.expect(
            op < 0.95,
            f"линия уровня у короткого события обязана тянуться, а непрозрачность {op}",
        )
        app.shot("level-dragged")

        # соседнее ДЛИННОЕ событие не тронуто: жест правит ровно то, за что
        # тянули (правило 20 «ничего не отрастает и не отрубается само»)
        op2 = clip_kt(saved, 1).get("opacity", 1.0)
        app.expect(op2 == 1.0, f"соседнее событие не тронуто, а у него {op2}")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

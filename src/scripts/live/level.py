#!/usr/bin/env python3
"""Live scenario R.40 item 12: opacity on a SMALL event.

Complaint verbatim: “item 12, it feels not done - on a small event I
try to catch the opacity there but it does not work”.

There were two locks, and both are visible only in the window: the level zone required a width
greater than 24 px, and the fade handles were caught within ±12 px of each corner — on
an event of 20 px they covered all of it, and the middle, where the level line lives,
was left without a single pixel.

Here a real mouse drags the line on an event ~20 px wide, and the consequence
is checked by what reached the project file.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def clip_kt(tl, index):
    """kt metadata of the index-th clip in the .otio (where opacity lives)"""
    items = []
    for track in tl.doc.get("tracks", {}).get("children", []):
        for item in track.get("children", []):
            if str(item.get("OTIO_SCHEMA", "")).startswith("Clip"):
                items.append(item)
    return (items[index].get("metadata") or {}).get("kt") or {}


def main(out):
    require_tools()
    # two short events: 0.2 s is ~20 px at the default zoom
    proj = project_with_clips(
        os.path.join(out, "level.otio"),
        [(0.0, 0.2, "synth:Ramp"), (1.0, 4.0, "synth:Ramp")],
    )

    with Live("level", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 2, f"start: two events, {p.clips}")

        g = app.geom()
        short = p.clips[0]
        x0 = app.x_at(short.start)
        x1 = app.x_at(short.end)
        w = x1 - x0
        app.expect(w < 24.0, f"scene: an event shorter than the old threshold — {w:.0f} px")

        # the level line at opacity = 1.0 stands at the very top of the event
        top = app.lane_y("video", 0, frac=0.0)
        lane_h = {(r["kind"], r["lane"]): r for r in g["rows"]}[("video", 0)]["h"]
        ly = top + 3.0
        mid_x = (x0 + x1) * 0.5

        app.drag(mid_x, ly, mid_x, ly + lane_h * 0.45)
        saved = app.saved_project()
        # the opacity lies in metadata.kt of the clip ITSELF in the .otio, not in the
        # root kt section
        op = clip_kt(saved, 0).get("opacity", 1.0)
        app.expect(
            op < 0.95,
            f"the level line of a short event must stretch, and the opacity {op}",
        )
        app.shot("level-dragged")

        # the neighbouring LONG event is untouched: the gesture adjusts exactly what was
        # dragged (rule 20 “nothing grows or switches off by itself”)
        op2 = clip_kt(saved, 1).get("opacity", 1.0)
        app.expect(op2 == 1.0, f"the neighboring event is untouched, and its {op2}")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

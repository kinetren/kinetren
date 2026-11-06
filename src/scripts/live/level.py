#!/usr/bin/env python3
"""Live scenario R.40 item 12 and U5.3 of R.72: the level line and the size of an event.

R.40, verbatim: “on a small event I try to catch the opacity there but it does not work”.
R.72 U5.3, a later word: “at a small zoom do not offer changing opacity/gain at once, I wanted
to move events so many times, and the volume change keeps triggering”.

So on an event ~20 px wide the middle moves the event and the opacity stays, and the
menu's number field reaches it; on a wide event the real mouse drags the line. The
consequence is checked by what reached the project file.
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

        # the narrow event: a press on the line is not the level
        app.drag(mid_x, ly + 6.0, mid_x + 40.0, ly + 6.0)
        saved = app.saved_project()
        op = clip_kt(saved, 0).get("opacity", 1.0)
        app.expect(op == 1.0, f"a narrow event has no level gesture, and the opacity {op}")
        app.shot("narrow-moved")

        # the wide neighbour: the line is the level
        wide = app.project_now().clips[1]
        x = (app.x_at(wide.start) + app.x_at(wide.end)) * 0.5
        app.drag(x, ly, x, ly + lane_h * 0.45)
        saved = app.saved_project()
        op2 = clip_kt(saved, 1).get("opacity", 1.0)
        app.expect(op2 < 0.95, f"the level line of a wide event must stretch, and the opacity {op2}")
        app.shot("level-dragged")

        # rule 20 “nothing grows or switches off by itself”: the other event is untouched
        op = clip_kt(saved, 0).get("opacity", 1.0)
        app.expect(op == 1.0, f"the neighboring event is untouched, and its {op}")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

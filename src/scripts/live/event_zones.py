#!/usr/bin/env python3
"""Live scenario R.72, USABILITY 5-7: an event's zones, mute and its chip, the range marker's end.

The owner, verbatim: “the lower corner must be only a horizontal length drag”; “let the gain go down
to mute … show somewhere on the event what gain/opacity is set”; “something is wrong with the second
marker … the end of the range marker”. The zones are one function for both kinds, so a video
event carries the gestures here.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def clip_kt(tl, index):
    items = []
    for track in tl.doc.get("tracks", {}).get("children", []):
        for item in track.get("children", []):
            if str(item.get("OTIO_SCHEMA", "")).startswith("Clip"):
                items.append(item)
    return (items[index].get("metadata") or {}).get("kt") or {}


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "zones.otio"), [(2.0, 6.0, "synth:Ramp")])
    with Live("event_zones", out, project=proj) as app:
        g = app.geom()
        start, dur = app.live_clips()[0]
        x1 = app.x_at(start + dur)
        h = next(r for r in g["rows"] if r["kind"] == "video" and r["lane"] == 0)["h"]
        bottom = app.lane_y("video", 0, frac=0.0) + h - 7
        app.step("open", "one event, row %d px" % h)

        # the bottom corner is a length drag, with the line anywhere
        app.drag(x1 - 2, bottom, x1 - 80, bottom)
        app.settle()
        start2, dur2 = app.live_clips()[0]
        app.expect(dur2 < dur, f"the bottom corner must change the length: {dur} → {dur2}")
        app.step("corner", "the bottom right corner: the event got shorter")

        # the line in the middle, down to the half: the chip says it
        xm = (app.x_at(start2) + app.x_at(start2 + dur2)) // 2
        top = app.lane_y("video", 0, frac=0.0)
        app.drag(xm, top + 3, xm, top + h * 0.55)
        app.settle()
        op = clip_kt(app.saved_project(), 0).get("opacity", 1.0)
        app.expect(op < 0.8, f"the line must change the opacity: {op}")
        app.step("chip", "the level line down: the chip after the name shows the opacity")

        # the range marker's end
        ruler = app.ruler_y()
        app.drag(app.x_at(3 * SEC), ruler, app.x_at(5 * SEC), ruler)
        app.settle()
        app.click(app.x_at(4 * SEC), ruler)
        app.wait_playhead(lambda t: abs(t - 4 * SEC) < 3000, "the cursor stands inside the region")
        app.key("m")
        app.settle()
        app.key("Return")
        app.settle()
        app.step("marker", "a region and a range marker over it")
        y = geometry()["y"] + int(app.geom()["timeline"]["top"]) + 4
        end = app.x_at(5 * SEC)
        app.hover(end, y)
        app.step("marker-hover", "the end of the marker named in the status line")
        app.drag(end, y, app.x_at(6 * SEC), y)
        app.settle()
        text = app.saved_text()
        project = json.loads(text)
        marks = next(v["markers"] for v in project.values() if isinstance(v, dict) and v.get("markers"))
        app.expect(len(marks) == 1, f"one marker: {marks}")
        got = marks[0].get("t", 0) + marks[0].get("duration", 0)
        app.near(got, 6 * SEC, 3000, "the end of the range marker must reach 6 s")
        app.step("marker-moved", "the end of the marker moved, the region stayed")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

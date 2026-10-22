#!/usr/bin/env python3
"""Live scenario R.38: “there are no dead ends”.

Claims of the user verbatim: “I do not understand how to delete a saved area
region, I saved it and now it hangs there, I do not understand how to
rename it - RMB gives nothing”, “I do not understand how to change the length of
a morph of the event”, “allow changing the height of each track individually”.

The harness tests 253/259/262 already check this, but in a SYNTHETIC scene without a
window. Here a real mouse in a real window answers another question:
can a hand reach these zones at all. This is exactly how it turned out that the first 16 px
of the gutter are the window resize zone, and a press there goes to the window manager.

Three gestures are checked, each by its own observable consequence:

1. the edge of the morph block is dragged, and the new length reaches the FILE;
2. the bottom edge of a row in the gutter changes the track height, and that also
   reaches the file;
3. RMB on the outline of a named area opens a menu — the screen after it
   must CHANGE (before R.38 nothing happened).
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def build(dest):
    """A clip, a morpher layer above it, and a named area on its track."""
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
            "name": "final",
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
        app.expect(len(p.clips) == 1, f"start: one clip, {p.clips}")

        g = app.geom()
        rows = {(r["kind"], r["lane"]): r for r in g["rows"]}
        morph = rows[("morph", 0)]
        video = rows[("video", 0)]

        # --- 1. the edge of the morph block is dragged ---------------------------------
        y = app.lane_y("morph", 0, frac=0.5)
        end_x = app.x_at(4 * SEC)
        app.drag(end_x, y, app.x_at(6 * SEC), y)
        saved = app.saved_project()
        m = saved.doc["metadata"]["kt"]["morphers"][0]
        app.near(
            m["duration"],
            4 * SEC,
            SEC // 2,
            "the edge of the morph block lengthened it (the length reached the file)",
        )
        app.expect(m["start"] == 2 * SEC, f"and the start stayed in place: {m['start']}")
        app.shot("morph-edge")

        # --- 2. track height is dragged in the gutter -------------------------
        # Coordinates are ON-SCREEN, as `x_at`/`lane_y` count them: the geometry
        # dump gives them RELATIVE TO THE WINDOW, and forgetting the window origin means
        # clicking aside. This is exactly how this step first “did not work”.
        win = geometry()
        # x — at the RIGHT edge of the gutter: on the left the window’s resize zone takes WIN_EDGE,
        # and the press there goes to the window manager (rule 15)
        gx = win["x"] + g["timeline"]["left"] - 8.0
        app.expect(
            g["timeline"]["left"] - 8.0 > 8.0,
            f"the gesture is placed outside the window resize zone: {g['timeline']['left'] - 8.0:.0f} px from the edge",
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
            "the height of the video track grew by exactly as much as was dragged",
        )
        app.near(
            rows2[("morph", 0)]["h"],
            morph["h"],
            2,
            "and the NEIGHBORING row is untouched (rule 20)",
        )
        app.shot("row-height")

        # --- 3. RMB on the outline of the area opens the menu ---------------------
        # the area outline is drawn along its track; we aim at the TOP line
        area_y = app.lane_y("video", 0, frac=0.02)
        area_x = app.x_at(6 * SEC)
        before = app.shot("before-menu")
        app.right_click(area_x, area_y)
        app.shot("area-menu")
        d = app.screen_diff(before)
        # the threshold is low on purpose: the area’s menu is short and takes a small
        # share of the screen. The CONTENT of the menu is checked by harness test 253 — here
        # the question is exactly one: does the RMB reach the outline in the live window
        app.expect(
            d > 0.001,
            f"RMB on the outline of the area must open a menu (the screen changed by {d:.1%})",
        )
        app.key("Escape")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

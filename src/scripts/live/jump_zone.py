#!/usr/bin/env python3
"""Live scenario R.72, TIMELINE 2-4 and T6: the scrollbar, both jump zones, the header.

The owner, verbatim: “very hard to find the side scrollbar, either the window resize or the jump
zone triggers”; “the added track buttons broke half of the jump zone on the left”; “if an event is
in the jump zone, it opens for some reason, even at the strongest edge”; “I would like to drag the
order of tracks with the mouse at the side”.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def stack_tracks(dest, copies):
    """More video tracks than the window shows, each with its own event."""
    doc = json.load(open(dest))
    vid = [t for t in doc["tracks"]["children"] if t.get("kind") == "Video"]
    nid = 100
    for _ in range(copies):
        clone = json.loads(json.dumps(vid[0]))
        for c in clone["children"]:
            if c.get("OTIO_SCHEMA", "").startswith("Clip"):
                nid += 1
                c["metadata"]["kt"]["id"] = nid
        doc["tracks"]["children"].append(clone)
    json.dump(doc, open(dest, "w"), indent=4)
    return dest


def main(out):
    require_tools()
    proj = stack_tracks(
        project_with_clips(os.path.join(out, "jump.otio"), [(0.0, 120.0, "synth:Ramp")]), 9
    )
    with Live("jump_zone", out, project=proj) as app:
        g = app.geom()
        win = geometry()
        tl = g["timeline"]
        right = win["x"] + int(tl["right"])
        left = win["x"] + int(tl["left"])
        y = app.lane_y("video", 0, frac=0.6)
        app.step("open", "a long event under both zones")

        # the event in the right zone: two quick clicks, two jumps, no window
        v0 = app.geom()["view_start"]
        app.double_click(right - 30, y)
        app.until(lambda: app.geom()["view_start"] > v0)
        app.settle()
        v1 = app.geom()["view_start"]
        app.expect(v1 > v0, f"a click in the right zone over an event must jump: {v0} → {v1}")
        app.expect(not app.has_probe("fx.close", tries=2), "the double click opened the event")
        app.step("right-jump", "two clicks near the right edge: the view jumped, no window")

        # the left zone on a row with badges
        v2 = app.geom()["view_start"]
        app.click(left + 6, y)
        app.until(lambda: app.geom()["view_start"] < v2)
        app.settle()
        v3 = app.geom()["view_start"]
        app.expect(v3 < v2, f"the left zone must jump on a row with badges: {v2} → {v3}")
        app.step("left-jump", "a click by the track header: the view jumped left")

        # the hot end under the header
        row = next(r for r in g["rows"] if r["kind"] == "video" and r["lane"] == 0)
        app.click(left - 20, win["y"] + int(row["top"]) + 4)
        app.until(lambda: app.geom()["view_start"] < v3)
        app.settle()
        v4 = app.geom()["view_start"]
        app.expect(v4 < v3, f"the header's free part must jump: {v3} → {v4}")
        app.step("header-jump", "a click on the header's free part: the hottest jump")

        # the scrollbar flush with the window, taken at its edge
        app.expect(app.has_probe("tl.vscroll"), "the tracks do not fit: the scrollbar must be there")
        bar = next(f for f in app.geom()["fields"] if f["param"] == "tl.vscroll")
        x = win["x"] + int(bar["x"] + bar["w"]) - 6
        y0 = win["y"] + int(bar["y"]) + 20
        rows0 = app.lane_y("video", 0, frac=0.5)
        app.drag(x, y0, x, y0 + 90)
        app.settle()
        app.expect(app.lane_y("video", 0, frac=0.5) < rows0, "the scrollbar must scroll the tracks")
        app.expect(app.geom()["window"]["w"] == g["window"]["w"], "the window took the press")
        app.step("scrollbar", "the scrollbar dragged, the window kept its size")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

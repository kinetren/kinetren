#!/usr/bin/env python3
"""Live scenario R.49 B (items 361-366): the right button does not stay silent, and the promises
of the hints come true.

Three things are checked on REAL gestures in a real window:

1. RMB on the transition window opens the menu of the TRANSITION itself (before R.49 it had no
   menu at all — everything lay in the menu of the neighboring clip);
2. a double click on the level line resets the level — the hint promised this from the start,
   while the gesture was opening the FX window;
3. RMB on the background of the media bin and on the meter panel shows a menu, instead of staying silent.

The consequence of each item is checked by what reached the SAVED .otio,
or by a change of the screen — not by the state in memory.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def clip_kt(tl, index):
    """kt metadata of the index-th clip in the .otio (where volume and level live)"""
    items = []
    for track in tl.doc.get("tracks", {}).get("children", []):
        for item in track.get("children", []):
            if str(item.get("OTIO_SCHEMA", "")).startswith("Clip"):
                items.append(item)
    return (items[index].get("metadata") or {}).get("kt") or {}


def main(out):
    require_tools()
    # two events end to end: a transition can be hung between them
    proj = project_with_clips(
        os.path.join(out, "rmb.otio"),
        [(0.0, 4.0, "synth:Ramp"), (4.0, 4.0, "synth:Ramp")],
    )
    add_crossfade(proj, seconds=1.0)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("rmb", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 2, f"start: two events, {p.clips}")

        y = app.lane_y("video", 0)
        # ---- 1. RMB on the transition window ------------------------------------
        tl = app.project_now()
        app.expect(
            len(tl.transitions) == 1,
            f"transition read from the file: {tl.transitions}",
        )
        t = tl.transitions[0]
        # the middle of the left half of the window: away from the joint, where the handles live
        x_mid = app.x_at(t["at"] - t["in"] // 2)
        before = app.shot("before-trans-menu")
        app.right_click(x_mid, y)
        app.shot("trans-menu")
        d = app.screen_diff(before)
        app.expect(
            d > 0.005,
            f"RMB on the transition window must open the menu (the screen changed by {d:.1%})",
        )
        app.key("Escape")

        # ---- 2. level line: double click resets --------------------
        # we lower the level of the second event by a gesture, then reset it by a double click
        tl = app.saved_project()
        second = tl.clips[1]
        x2 = app.x_at(second.mid())
        lane_h = {(r["kind"], r["lane"]): r for r in app.geom()["rows"]}[("video", 0)]["h"]
        ly = app.lane_y("video", 0, frac=0.0) + 3.0
        app.drag(x2, ly, x2, ly + lane_h * 0.45)
        app.shot("level-down")
        lowered = clip_kt(app.saved_project(), 1).get("opacity", 1.0)
        app.expect(lowered < 0.9, f"the gesture must lower the level, got {lowered}")

        # the line has moved down together with the value — we aim where it is now
        ly2 = app.lane_y("video", 0, frac=0.0) + 3.0 + (1.0 - lowered) * (lane_h - 8.0)
        app.double_click(x2, ly2)
        app.shot("level-reset")
        back = clip_kt(app.saved_project(), 1).get("opacity", 1.0)
        app.expect(
            back >= 0.999,
            f"a double click on the level line must reset it (the hint promises this), "
            f"got {back}",
        )

        # ---- 3. the bin background and meters do not stay silent -------------------------------
        win = geometry()
        # the program reports the bin rectangle itself (R.27): below it in the
        # column lives the perf panel, and “roughly on the left” would probe it
        b = app.geom()["bin"]
        before = app.shot("before-bin-menu")
        app.right_click(
            win["x"] + int((b["left"] + b["right"]) / 2),
            win["y"] + int(b["bottom"]) - 10,
        )
        app.shot("bin-menu")
        d = app.screen_diff(before)
        app.expect(d > 0.002, f"RMB on the media bin background must open the menu (the screen on {d:.1%})")
        app.key("Escape")

        before = app.shot("before-meter-menu")
        app.right_click(win["x"] + win["w"] - 40, win["y"] + int(win["h"] * 0.35))
        app.shot("meter-menu")
        d = app.screen_diff(before)
        app.expect(d > 0.002, f"RMB on the meters panel must open the menu (the screen on {d:.1%})")
        app.key("Escape")

        return app.done()


def add_crossfade(path, seconds):
    """A crossfade BETWEEN two events of a video track.

    It is built directly in the file: the corner gesture is checked by t173, and this scenario
    needs a ready transition, so it can probe it with the right button."""
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

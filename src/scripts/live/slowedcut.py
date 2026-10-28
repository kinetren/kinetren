#!/usr/bin/env python3
"""Live scenario slowed-reverb-is-an-event: a slowed event with a speed CURVE is copied, pasted
and cut like any event.

The owner's words (ktprompts 26.10.18 item 3): "slowed does not let me copy-paste the event and
cut it into pieces — childish excuses". The cut WAS refused, silently in the picture: a speed curve
that starts below 1× moved the right half's keys with the anchor to a negative tick, and validation
rejected the whole batch. `slowed.py` cuts a CONSTANT 0.5× event, which always worked; here the
event carries a curve 0.5→1.5, as in the owner's projects.

One event 0–6 s on the video track with slowed reverb and the speed curve.
The synthetic frame is a solid colour from the pts, so "which frame is on screen" is read from the
preview pixel: a cut must not move the material under a point after it.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def build(dest):
    project_with_clips(dest, [(0.0, 6.0, "synth:Ramp")])
    with open(dest) as f:
        doc = json.load(f)
    doc["metadata"]["kt"]["v"] = 4
    node = {
        "OTIO_SCHEMA": "Effect.1",
        "name": "",
        "effect_name": "kt:slowed_reverb",
        "enabled": True,
        "metadata": {
            "kt": {
                "kind": "slowed_reverb",
                "rate": 1.0,
                "mix": 0.0,
                "reverb_delay_ms": 85.0,
                # the speed curve 0.5 → 1.5 over the event (keys in material ticks)
                "auto": [
                    {"param": "rate", "keys": [{"t": 0, "v": 0.5}, {"t": 6 * SEC, "v": 1.5}]}
                ],
            }
        },
    }
    for track in doc["tracks"]["children"]:
        for c in track.get("children", []):
            if str(c.get("OTIO_SCHEMA", "")).startswith("Clip"):
                c["effects"] = [node]
    with open(dest, "w") as f:
        json.dump(doc, f, indent=2)
    return dest


def frame_at(app, seconds, prev=None):
    was = prev if prev is not None else app.pixel(*app.preview_center())
    app.click(app.x_at(int(seconds * SEC)), app.ruler_y())
    return app.pixel_settled(app.preview_center(), differs_from=was)


def main(out):
    require_tools()
    proj = build(os.path.join(out, "slowedcut.otio"))
    with Live("slowedcut", out, project=proj) as app:
        n0 = len(app.live_clips())
        app.expect(n0 >= 1, f"the slowed event is in the project: {app.live_clips()}")
        app.step("open", "a slowed event with a speed curve 0.5→1.5")

        # 1. click the event → Copy → click the ruler at 7 s → Paste: a copy appears.
        # Through the "Edit" menu, as `nestshare` does: the virtual screen has no system
        # clipboard, and the menu items call the same body as Ctrl+C / Ctrl+V (rule 17)
        app.click(app.x_at(2 * SEC), app.lane_y("video", 0))
        app.click_probe("menu.edit", "“Edit” menu button")
        app.click_probe("edit.copy", "“Copy” item")
        app.expect(app.geom().get("clipboard", 0) >= 1, "Copy took the slowed event")
        app.click(app.x_at(7 * SEC), app.ruler_y())
        app.click_probe("menu.edit", "“Edit” menu button")
        app.click_probe("edit.paste", "“Paste” item")
        n1 = len(app.live_clips())
        app.expect(n1 == 2 * n0, f"Paste put a copy at the cursor: {n0} → {n1} events")
        app.step("pasted", "Edit → Paste: the copy of the slowed event stands at 7 s")

        # 2. the frame at 4.5 s of the original, then S at 3 s: the frame at 4.5 s is the same
        probe = 4.5
        before = frame_at(app, probe)
        app.click(app.x_at(2 * SEC), app.lane_y("video", 0))
        app.click(app.x_at(3 * SEC), app.ruler_y())
        app.key("s")
        n2 = len(app.live_clips())
        app.expect(n2 > n1, f"S cut the slowed event with a speed curve: {n1} → {n2} events")
        after = frame_at(app, probe, prev=before)
        same = all(abs(x - y) <= 10 for x, y in zip(after, before))
        app.expect(same, f"after the cut the frame at {probe} s is the same: {after} ≈ {before}")
        app.step("cut", "S: the slowed event is cut at 3 s, the picture after the cut is unchanged")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

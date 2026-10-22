#!/usr/bin/env python3
"""Live scenario R.35: morpher LAYER.

The user’s words: “morphers apply to everything that is BELOW this morpher,
and there is no need to struggle with selection”, “I would like morphers to be active
also in time, i.e. if they are “played”, then they also morph/apply the
effects”, “so as not to lose space — morph tracks can be reduced in height
threefold” (R.38 item 10 corrected: “increase the morph track height by default
by 2” — now the morph row is one and a half times lower, and the camera row is three times lower).

Three things that are NOT visible from the harness tests:

1. the layer really changes the PICTURE beneath it — and only in its own window;
2. into the THIN morph row they hit with a real mouse;
3. the morpher block is moved with the same mouse, and this reaches the file.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def with_layer(dest, clips, win_s, win_len_s, strength=None):
    """Project with a clip and a morpher-LAYER ABOVE it: window [win_s, +win_len_s),
    in the patch — Edges (strongly changes brightness, visible in the screenshot)."""
    project_with_clips(dest, clips)
    with open(dest) as f:
        doc = json.load(f)
    kt = doc["metadata"]["kt"]
    kt["morph_lanes"] = 1
    # the morph row ON TOP: the default puts it below, and “everything below it”
    # would be empty (finding 3 of R.35)
    kt["lane_order"] = [["morph", 0], ["video", 0], ["audio", 0]]
    m = {
        "id": 1,
        "lane": 0,
        "start": int(win_s * SEC),
        "duration": int(win_len_s * SEC),
        # the default targets — “everything below” — are not written to the file
        "patch": {"video_fx": [{"kind": "edges", "threshold": 24}]},
    }
    if strength is not None:
        m["strength"] = strength
    kt["morphers"] = [m]
    # envelope version: before v4 the morpher is read as “outside time” (migration)
    kt["v"] = 4
    with open(dest, "w") as f:
        json.dump(doc, f, indent=2)
    return dest


def main(out):
    require_tools()
    proj = with_layer(
        os.path.join(out, "morph.otio"),
        [(0.0, 8.0, "synth:Ramp")],
        win_s=3.0,
        win_len_s=3.0,
    )

    with Live("morph", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 1, f"start: one clip, {p.clips}")

        # --- 1. the service row is THREE TIMES lower than the clip tracks ---------------
        g = app.geom()
        rows = {(r["kind"], r["lane"]): r for r in g["rows"]}
        morph = rows[("morph", 0)]
        video = rows[("video", 0)]
        app.expect(morph["top"] < video["top"], "morph row above the video")
        # R.35 asked for “three times”, R.38 item 10 — “raise the morpher track’s default height
        # by 2”: now the morph row is lower by ONE AND A HALF, and
        # three times lower remained the CAMERA. We check the very thing the request was
        # for: the row is noticeably lower than the clip tracks and is hit by the mouse.
        ratio = video["h"] / max(morph["h"], 1e-6)
        app.expect(1.3 < ratio < 1.8, f"morph row lower by one and a half: {video['h']} / {morph['h']}")

        # --- 2. the layer changes the picture ONLY within its own window ------------------
        # the synthetic frame is a SOLID colour from pts, so Edges inside the
        # window gives black; outside the colour remains (and it differs at different
        # points — the Ramp source, two points cannot be compared with each other)
        app.click(app.x_at(1 * SEC), app.ruler_y())
        app.shot("outside")
        before = app.brightness(app.preview_center())
        app.click(app.x_at(int(4.5 * SEC)), app.ruler_y())
        app.shot("inside")
        inside = app.brightness(app.preview_center())
        app.click(app.x_at(7 * SEC), app.ruler_y())
        app.shot("outside2")
        after = app.brightness(app.preview_center())
        app.expect(before > 30, f"before the layer window the frame is not black: {before:.1f}")
        app.expect(inside < 12, f"in the layer window Edges blanks the solid frame: {inside:.1f}")
        app.expect(after > 30, f"after the layer window the frame is not black again: {after:.1f}")

        # --- 3. the thin row is HIT with the mouse ---------------------------
        # we drag the morpher block one second to the right along the row itself
        y = app.lane_y("morph", 0, frac=0.5)
        x0 = app.x_at(int(4.5 * SEC))
        app.drag(x0, y, app.x_at(int(5.5 * SEC)), y)
        saved = app.saved_project()
        m = saved.doc["metadata"]["kt"]["morphers"][0]
        app.near(
            m["start"],
            4 * SEC,
            SEC // 2,
            "the morpher block was moved by mouse (it hit the thin row)",
        )
        app.shot("moved")
        return app.done()


def strength_probe(out):
    """Layer strength (R.38 item 12) in a REAL window: half strength must give
    a picture BETWEEN “no layer” and “the full layer”.

    A separate session for each strength: the strength lives in the file, and the scenario does not know how
    to re-read the project on the fly — four Lives in one scenario once
    killed the display (R.37), so there is exactly one session per run."""
    vals = {}
    for tag, strength in (("off", 0.0), ("half", 0.5), ("full", 1.0)):
        proj = with_layer(
            os.path.join(out, f"morph-{tag}.otio"),
            [(0.0, 8.0, "synth:Ramp")],
            win_s=3.0,
            win_len_s=3.0,
            strength=strength,
        )
        with Live(f"morph-{tag}", out, project=proj) as app:
            app.click(app.x_at(int(4.5 * SEC)), app.ruler_y())
            app.shot(tag)
            vals[tag] = app.brightness(app.preview_center())
            app.expect(True, f"strength {strength}: brightness {vals[tag]:.1f}")
            rc = app.done()
        if rc:
            return rc
    print(f"   brightness: off={vals['off']:.1f} half={vals['half']:.1f} full={vals['full']:.1f}")
    if not (vals["full"] < vals["half"] < vals["off"]):
        print(
            "   FAILURE half strength must be BETWEEN “no layer” and “full layer”: "
            f"{vals}",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    d = sys.argv[1] if len(sys.argv) > 1 else "."
    rc = main(d)
    sys.exit(rc or strength_probe(d))

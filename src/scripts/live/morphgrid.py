#!/usr/bin/env python3
"""Live scenario morpher-event-grid-cost: the owner's event grid in a real window.

The owner (ktprompts 26.10.18 item 8): "an event grid in the morpher with automation
0 → 12×30 → 12×10 → 0 drops fps — it is basically a simple pixel shader". The scene is OURS
(R.67): a synthetic 960×540 source, a morpher block with a grid automated 1×1 → 12×30 → 12×10 →
1×1 and fades at both ends — the t946 / t947 case.

PATH (ui-timeline spec, "The probe and the panel name the cost and the backend"):
1. the project opens, the cursor stands at 2 s (12×30) → the Performance panel shows the line
   "Compose … ms" under the FX backend line; the program composed one cell, not the full pass;
2. Play from the start → the compose value follows the frames while the grid grows;
3. Stop → the last values stay and the window no longer repaints.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, require_tools  # noqa: E402

SEC = 90000


def owner_case(dest):
    keys = lambda vals: [{"t": s * 2 * SEC, "v": v} for s, v in enumerate(vals)]  # noqa: E731
    project = {
        "format_version": 1,
        "settings": {"fps": 30, "width": 960, "height": 540, "sample_rate": 48000},
        "media": [{"id": 1, "path": "synth:ramp"}],
        "clips": [{
            "id": 1, "link_group": None, "kind": "video", "lane": 0, "start": 0,
            "duration": 6 * SEC, "source_in": 0, "media": 1, "gain_db": 0.0, "fx": [],
        }],
        "tracks": [{"id": 1, "kind": "video"}, {"id": 2, "kind": "audio"}, {"id": 3, "kind": "morph"}],
        "morphers": [{
            "id": 1, "lane": 0, "start": 0, "duration": 6 * SEC,
            "targets": {"on": "clips", "ids": [1]},
            "patch": {
                "video_fx": [{"kind": "event_grid", "cols": 1, "rows": 1, "center": False}],
                "video_auto": [[
                    {"param": "cols", "ease": False, "keys": keys([1.0, 12.0, 12.0, 1.0])},
                    {"param": "rows", "ease": False, "keys": keys([1.0, 30.0, 10.0, 1.0])},
                ]],
            },
            "fade_in": SEC // 2,
            "fade_out": SEC // 2,
        }],
        "overlap_layers": True,
    }
    with open(dest, "w") as f:
        json.dump({"kinetren": 1, "project": project}, f, indent=2)
    return dest


def main(out):
    require_tools()
    path = owner_case(os.path.join(out, "morphgrid.kt"))
    full = 960 * 540 * 16  # the full 16-tap pass at the project raster, before the tile rule

    with Live("morphgrid", out, project=path) as app:
        # --- 1. paused at 12×30: the panel names the cost, and it is one cell ----------
        app.click(app.x_at(2 * SEC), app.ruler_y())
        app.settle()
        c = app.geom()["compose"]
        app.expect(c["ms"] > 0, f"the panel's compose time is measured: {c}")
        app.expect(
            0 < c["subsamples"] and c["subsamples"] * 100 <= full,
            f"at 12×30 the grid composes one cell, not the full pass: {c['subsamples']} of {full}",
        )
        app.step("paused-12x30", "paused at 12×30: the Performance panel shows the Compose line")

        # --- 2. play: the value follows the frames ------------------------------------------
        app.click(app.x_at(0), app.ruler_y())
        app.key("space")
        app.wait_playhead(lambda t: t > 1.5 * SEC, "playback reached 1.5 s", timeout=40)
        c1 = app.geom()["compose"]
        app.expect(c1["avg"] > 0, f"while playing the 1 s average is measured: {c1}")
        app.step("playing", "playing while the grid grows: the Compose line follows the frames")

        # --- 3. stop: the values stay, the window stops repainting ----------------------
        app.key("space")
        app.settle()
        # the frame on screen is recomposed until its layers have landed, and the panel's average
        # covers the last second: "stopped" is a pair of reads a second apart that agree, and
        # a program that kept composing never gives one
        a = app.geom()
        for _ in range(20):
            poll_step(1.0)
            b = app.geom()
            if a["compose"] == b["compose"]:
                break
            a = b
        app.expect(a["compose"] == b["compose"], f"the values changed after Stop: {a['compose']} → {b['compose']}")
        # the program's idle tick is 250 ms (4 frames a second); repainting for the panel would be
        # dozens a second — the Compose line itself must not keep the window animating
        app.expect(b["frame"] - a["frame"] <= 6, f"the window kept repainting after Stop: {a['frame']} → {b['frame']}")
        app.step("stopped", "stopped: the last Compose values stay, nothing repaints")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

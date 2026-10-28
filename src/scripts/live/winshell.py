#!/usr/bin/env python3
"""Live scenario shared-window-shell: a window does not grow by itself; Esc closes the top one.

The owner's words (kkt item 2, 2026-10-27): "on my other system the morpher window endlessly
tries to grow in width; move the mouse anywhere in the main window or the morpher window — it
widens fast; don't move it — slowly". His system (2026-10-28): Ubuntu, 1920×1080, the
program's own zoom raised twice with Ctrl+= (1.2). The harness test t923 reproduces it; here
the REAL program does the same gestures: open the morpher window, Ctrl+= twice, move the
mouse for thirty seconds, read the window's width from its probe.

Then ktprompts 26.10.18 item 6: the camera window did not close on Esc. With the FX window and
the camera window open, Esc closes the camera window (the top one) first, then the FX window.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def scene(dest):
    """A clip, a morpher layer over it with a twelve-slot chain (the strip wraps), and a
    camera event on a camera track."""
    project_with_clips(dest, [(0.0, 8.0, "synth:Ramp")])
    with open(dest) as f:
        doc = json.load(f)
    kt = doc["metadata"]["kt"]
    kt["morph_lanes"] = 1
    kt["camera_lanes"] = 1
    kt["lane_order"] = [["camera", 0], ["morph", 0], ["video", 0], ["audio", 0]]
    kt["morphers"] = [{
        "id": 1,
        "lane": 0,
        "start": 0,
        "duration": 6 * SEC,
        "patch": {
            "video_fx": [{"kind": "edges", "threshold": 24 + i} for i in range(12)],
            # the first slot carries a curve: its graph is open when the slot is selected
            "video_auto": [
                [{"param": "threshold", "keys": [{"t": 0, "v": 10.0}, {"t": 4 * SEC, "v": 60.0}]}]
            ] + [[] for _ in range(11)],
        },
    }]
    kt["cameras"] = [{"id": 1, "lane": 0, "start": 0, "duration": 6 * SEC, "cam": {}}]
    kt["v"] = 4
    with open(dest, "w") as f:
        json.dump(doc, f, indent=2)
    return dest


def width(app, probe):
    r = app._probe_rect(probe)
    app.expect(r is not None, f"the window probe {probe} is in the dump")
    return r["w"] if r else 0.0


def main(out):
    require_tools()
    proj = scene(os.path.join(out, "winshell.otio"))
    with Live("winshell", out, project=proj) as app:
        p = app.project_now()
        app.expect(len(p.clips) == 1, f"start: one clip, {p.clips}")

        # --- 1. the morpher window at the program's zoom 1.1 and 1.2 keeps its width -----
        app.double_click(app.x_at(3 * SEC), app.lane_y("morph", 0))
        app.expect(app.has_probe("win.morpher"), "double click on the morph block opened its window")
        # the owner's scene: an effect slot selected, its parameters and curve graph shown
        # (clicked BEFORE the zoom: the probe rectangles are in points, the click in pixels)
        app.click_probe("fx.slot.0", "the first effect slot of the strip")
        screen0 = app.geom()["window"]["w"]
        # the owner pressed Ctrl+= "a couple of times": each press is one zoom step (1.1, 1.2),
        # and the growth depends on the fractional width in physical pixels, so both are held
        for presses, zoom in ((1, 1.1), (2, 1.2)):
            app.key("ctrl+equal")
            app.settle()
            screen = app.geom()["window"]["w"]
            # the dump is in points: at zoom z the same screen is z times fewer points wide
            app.expect(
                abs(screen0 / max(screen, 1.0) - zoom) < 0.03,
                f"Ctrl+= ×{presses} set the program's zoom to {zoom}: {screen0:.1f} → {screen:.1f} points",
            )
            w0 = width(app, "win.morpher")
            app.step(f"zoom-{zoom}", f"the morpher window at zoom {zoom}, before the mouse moves")
            log = [round(w0, 2)]
            frame0 = app.geom()["frame"]
            x_hi = max(screen * 0.8, 400)
            t_end = time.time() + 15.0
            n = 0
            while time.time() < t_end:
                n += 1
                app.hover(60 + (n * 37) % int(x_hi), 120 + (n * 23) % 400)
                if n % 3 == 0:
                    log.append(round(width(app, "win.morpher"), 2))
            w1 = width(app, "win.morpher")
            log.append(round(w1, 2))
            frames = app.geom()["frame"] - frame0
            app.expect(frames >= 50, f"the program drew {frames} frames during the motion ({n} hovers)")
            app.step(
                f"zoom-{zoom}-after-15s",
                f"after fifteen seconds of mouse motion at zoom {zoom} the window has the same width",
            )
            app.expect(abs(w1 - w0) <= 1.0, f"the morpher window keeps its width at zoom {zoom}: {log}")
        app.key("ctrl+0")
        app.key("Escape")
        app.expect(app.wait_gone("win.morpher"), "Esc closed the morpher window")

        # --- 2. Esc closes the TOP window first --------------------------------------
        app.double_click(app.x_at(2 * SEC), app.lane_y("video", 0))
        app.expect(app.has_probe("win.fx"), "double click on the event opened its FX window")
        app.double_click(app.x_at(3 * SEC), app.lane_y("camera", 0))
        app.expect(app.has_probe("win.camera"), "double click on the camera block opened its window")
        app.step("fx-and-camera", "the FX window and the camera window over it")
        app.key("Escape")
        app.expect(app.wait_gone("win.camera"), "the first Esc closed the camera window (the top one)")
        app.expect(app.has_probe("win.fx"), "the FX window is still open after the first Esc")
        app.step("esc-camera-gone", "first Esc: the camera window is gone, the FX window stays")
        app.key("Escape")
        app.expect(app.wait_gone("win.fx"), "the second Esc closed the FX window")
        app.step("esc-fx-gone", "second Esc: no window is left")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "/tmp/kt-live-winshell"))

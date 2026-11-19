#!/usr/bin/env python3
"""Live scenario chroma-key-eyedropper (kkt-26.11.01b B2): the chroma key's colour is taken from the
preview's frame with an eyedropper.

His words (line 3, translated): the greenscreen has no eyedropper to poke the same preview frame or
take a colour from the screen anywhere else.

PATH (video-fx "A colour is taken from the screen with the eyedropper"):

1. a teal clip; a double click opens its effects window; the picker adds Greenscreen;
2. the pipette beside the key colour → the cursor is a crosshair, the status line says to click a
   colour, Esc puts it down;
3. a click on the preview's picture → the key colour becomes the teal of the frame, and the preview
   does not start playing; the saved project holds the teal key.
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000
TEAL = (0x1E, 0x8C, 0x8C)


def main(out):
    require_tools()
    teal = color_media(out, "teal", "1E8C8C", seconds=6)
    path = project_with_clips(os.path.join(out, "eyedropper.otio"), [(0.0, 6.0, teal)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("eyedropper", out, project=path) as app:
        # 1) the effects window, Greenscreen from the picker
        app.click(app.x_at(2 * SEC), app.ruler_y())
        app.double_click(app.x_at(3 * SEC), app.lane_y("video", 0, frac=0.5))
        app.expect(app.has_probe("fxreg.add"), "the effects window opened")
        if not app.click_probe("fxreg.add", "the picker button"):
            return app.done()
        for ch in "green":
            app.key(ch)
        cell = False
        for _ in range(40):
            if app.has_probe("fxreg.tpl.greenscreen", tries=1):
                cell = True
                break
            poll_step(0.25)
        app.expect(cell, "the gallery shows Greenscreen")
        if not cell or not app.click_probe("fxreg.tpl.greenscreen", "the Greenscreen cell"):
            return app.done()
        app.expect(app.has_probe("fx.key.pick"), "the key colour row has a pipette")
        app.step("greenscreen", "Greenscreen added: the key colour row with its pipette")

        # 2) arm, Esc, arm again
        app.click_probe("fx.key.pick", "the pipette")
        app.expect(app.has_probe("eyedrop.layer"), "the pick layer is up")
        app.step("armed", "the pipette is on: the status line says to click a colour")
        app.key("Escape")
        app.expect(app.wait_gone("eyedrop.layer"), "Esc put the pipette down")
        app.expect(app.has_probe("fx.key.pick"), "Esc kept the effects window")
        app.click_probe("fx.key.pick", "the pipette again")
        app.expect(app.has_probe("eyedrop.layer"), "the pick layer is up again")

        # 3) a click on the preview's picture takes its colour
        cx, cy = app.preview_center()
        app.click(cx, cy)
        app.expect(app.wait_gone("eyedrop.layer"), "the pick put the pipette down")
        app.step("picked", "the key colour is the teal of the frame; the preview did not start playing")
        app.expect(not app.geom().get("playing", False), "the pick's click did not start playback")
        text = app.saved_text()
        got = [re.search(rf'"{n}":\s*([0-9.]+)', text) for n in ("key_r", "key_g", "key_b")]
        vals = [float(m.group(1)) if m else -1 for m in got]
        app.expect(all(abs(v - t) <= 12 for v, t in zip(vals, TEAL)),
                   f"the saved key is the frame's teal: {vals} vs {TEAL}")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/eyedropper"))

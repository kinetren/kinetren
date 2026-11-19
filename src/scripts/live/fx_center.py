#!/usr/bin/env python3
"""Live scenario fx-center-on-preview (`v26.10` T10): an effect's centre is set by dragging it on
the preview, as in Vegas.

His words (line 52, translated): Radial Blur has a centre X Y … and I cannot, as in Vegas, point
the centre on the preview itself.

PATH (video-fx "An effect's centre is dragged on the preview"):

1. a striped clip; a double click opens its effects window; the picker adds Radial Blur: a crosshair
   stands at the middle of the preview's picture;
2. the crosshair is dragged up and right: it follows, the blur's rays now spread from there;
3. the saved project holds the new centre (right of the middle, above it).
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, pattern_media, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def field(app, name):
    return next((f for f in app.geom().get("fields", []) if f["param"] == name), None)


def main(out):
    require_tools()
    bars = pattern_media(out, "bars", 6)
    path = project_with_clips(os.path.join(out, "fx_center.otio"), [(0.0, 6.0, bars)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("fx_center", out, project=path) as app:
        app.click(app.x_at(2 * SEC), app.ruler_y())
        app.double_click(app.x_at(3 * SEC), app.lane_y("video", 0, frac=0.5))
        app.expect(app.has_probe("fxreg.add"), "the effects window opened")
        if not app.click_probe("fxreg.add", "the picker button"):
            return app.done()
        for ch in "radial":
            app.key(ch)
        cell = False
        for _ in range(40):
            if app.has_probe("fxreg.tpl.radial_blur", tries=1):
                cell = True
                break
            poll_step(0.25)
        app.expect(cell, "the gallery shows Radial Blur")
        if not cell or not app.click_probe("fxreg.tpl.radial_blur", "the Radial Blur cell"):
            return app.done()
        app.expect(app.has_probe("fx.center"), "a centre crosshair stands on the preview")
        frame = field(app, "preview.frame")
        cross = field(app, "fx.center")
        app.step("crosshair", "Radial Blur added: its centre crosshair at the middle of the picture")
        if not (frame and cross):
            return app.done()
        mid = (frame["x"] + frame["w"] / 2, frame["y"] + frame["h"] / 2)
        c0 = (cross["x"] + cross["w"] / 2, cross["y"] + cross["h"] / 2)
        app.expect(abs(c0[0] - mid[0]) < 3 and abs(c0[1] - mid[1]) < 3, f"the default centre is the middle: {c0} vs {mid}")

        win = geometry()
        x0, y0 = win["x"] + int(c0[0]), win["y"] + int(c0[1])
        dx, dy = int(frame["w"] * 0.25), -int(frame["h"] * 0.25)
        app.drag(x0, y0, x0 + dx, y0 + dy)
        moved = app.until(lambda: (lambda f: f is not None and f["x"] + f["w"] / 2 - c0[0] > dx * 0.8)(field(app, "fx.center")))
        app.step("dragged", "the crosshair dragged up and right: the rays spread from there")
        app.expect(moved, f"the crosshair followed the drag: {field(app, 'fx.center')}")
        text = app.saved_text()
        cx = re.search(r'"center_x":\s*(-?[0-9.]+)', text)
        cy = re.search(r'"center_y":\s*(-?[0-9.]+)', text)
        vals = (float(cx.group(1)) if cx else 0.0, float(cy.group(1)) if cy else 0.0)
        app.expect(abs(vals[0] - 0.5) < 0.05 and abs(vals[1] + 0.5) < 0.05,
                   f"the saved centre is right of and above the middle: {vals}")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/fx_center"))

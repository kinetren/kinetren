#!/usr/bin/env python3
"""Live scenario fx-form-width (kkt-26.11.07 line 13, test t3592): a form uses the width it is given.

The owner: the camera ISP form was "a sheet three kilometres long, and half of its width is empty",
"I asked so many times to remember it for ever". OUR fixture (a 3 s synthetic ramp), real gestures:

1. a double click on the event → the effects window; "+ effect…", "Camera ISP" → its pipeline,
   one column of blocks at the default width;
2. the window's corner dragged to the screen's width → the blocks stand side by side in columns,
   the sheet is shorter;
3. the corner dragged back narrow → one column again, nothing reaches past the window's edge.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def fields(app):
    return {f["param"]: f for f in app.geom().get("fields", [])}


def stages(app):
    """The pipeline's stage switches: where each block's stages stand."""
    return [f for n, f in fields(app).items() if n.startswith("isp.stage.")]


def columns(rects):
    """How many distinct columns the rectangles stand in (by their left edge)."""
    xs = sorted(r["x"] for r in rects)
    cols = []
    for x in xs:
        if not cols or x - cols[-1] > 60:
            cols.append(x)
    return len(cols)


def span(rects):
    return max(r["y"] + r["h"] for r in rects) - min(r["y"] for r in rects) if rects else 0


def drag_corner(app, r, dx, dy=0):
    """Drag a window's bottom-right corner by (dx, dy) as a person does."""
    win = geometry()
    x0 = win["x"] + int(r["x"] + r["w"]) - 3
    y0 = win["y"] + int(r["y"] + r["h"]) - 3
    app.drag(x0, y0, x0 + dx, y0 + dy)
    app.settle()


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "form_width.otio"), [(0.0, 3.0, "synth:Ramp")])

    with Live("form_width", out, project=proj) as app:
        app.resize(1900, 1000)
        app.double_click(app.x_at(1 * SEC), app.lane_y("video", 0, frac=0.5))
        app.expect(app.has_probe("win.fx"), "the effects window opened")
        if not app.click_probe("fxreg.add", "the picker button"):
            return app.done()
        poll_step(0.5)
        app.type_text("camera")
        if not app.has_probe("fxreg.tpl.camera_isp", tries=10):
            app.type_text("camera")
        app.click_probe("fxreg.tpl.camera_isp", "Camera ISP")
        app.expect(app.has_probe("isp.stage.wb", tries=20), "the window shows the camera's pipeline")
        app.settle()
        s0 = stages(app)
        app.expect(len(s0) >= 10, f"the pipeline's stages are drawn: {len(s0)}")
        c0, h0 = columns(s0), span(s0)
        app.step("default", f"the default width: the blocks in {c0} column(s), the sheet {h0:.0f} pt long")

        w0 = fields(app).get("win.fx")
        if not w0:
            return app.done()
        screen_w = app.geom()["window"]["w"]
        drag_corner(app, w0, int(screen_w - 24 - (w0["x"] + w0["w"])))
        w1 = fields(app).get("win.fx") or w0
        s1 = stages(app)
        c1, h1 = columns(s1), span(s1)
        app.expect(w1["w"] > w0["w"] + 600, f"the window followed its corner: {w0['w']:.0f} → {w1['w']:.0f} wide")
        app.expect(c1 >= 2 and c1 > c0, f"wide, the blocks stand side by side: {c0} → {c1} columns")
        app.expect(h1 < h0 * 0.7, f"wide, the sheet is shorter: {h0:.0f} → {h1:.0f} pt")
        app.step("wide", f"dragged to the screen's width: the blocks in {c1} columns, the sheet {h1:.0f} pt long")

        drag_corner(app, w1, int(460 - w1["w"]))
        w2 = fields(app).get("win.fx") or w1
        s2 = stages(app)
        c2 = columns(s2)
        app.expect(c2 == 1, f"narrow, one column again: {c2}")
        right = max((f["x"] + f["w"] for n, f in fields(app).items()
                     if n.startswith(("isp.", "lbl.")) and f["y"] >= w2["y"] and f["y"] <= w2["y"] + w2["h"]),
                    default=0)
        app.expect(right <= w2["x"] + w2["w"] + 1,
                   f"narrow, nothing reaches past the window's edge: {right:.0f} against {w2['x'] + w2['w']:.0f}")
        app.step("narrow", f"dragged narrow ({w2['w']:.0f} pt): one column, every row inside the window")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

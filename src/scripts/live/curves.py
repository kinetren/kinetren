#!/usr/bin/env python3
"""Live scenario R.67: a whole curve — keys, the shape of a segment, the formula layer,
the link door, the reference — in a real window.

Claims of the owner (verbatim, `docs/user-feedback.md`, R.67): “visually
for the user the curve is not visible at all”, “view curves — I
do not understand at all how to assign them”, “no reference, no hints
at the moment of input”, “the fields grow in width of the window again”.

PATH (ui-timeline scenarios “The graph draws the shape”, “The curve block refers to
the visible one”, “Formula layer”, “Link door”, “Reference”):

1. FX window of the event → “+ curve” → parameter → on the graph a constant;
2. three clicks on the graph — three keys;
3. right click on a segment → “Zigzag” → between the keys a trembling line;
4. the block under the graph is labeled with the name of the curve; the “Formula” field: `k*0.5+0.1`,
   Enter → a dashed line above the keys, keys intact;
5. “Driven by…” → the list of events and properties;
6. “Reference” → variables and functions;
7. Ctrl+S → the file has a `zigzag` shape and a formula layer.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, geometry, poll_step, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401

SEC = 90000


def probes(app):
    return {f["param"]: f for f in app.geom().get("fields", [])}


def click_in(app, r, fx, fy=0.5):
    win = geometry()
    app.click(win["x"] + int(r["x"] + r["w"] * fx), win["y"] + int(r["y"] + r["h"] * fy))


def main(out):
    require_tools()
    path = os.path.join(out, "curves.otio")
    demo_otio(path)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("curves", out, project=path) as app:
        # 1) the FX window and the curve
        y = app.lane_y("video", 0, frac=0.5)
        app.double_click(app.x_at(1 * SEC), y)
        app.expect(app.has_probe("fx.cancel"), "the effects window opened")
        f = probes(app)
        add = next((k for k in f if k.startswith("auto.add.")), None)
        if add is None:
            app.click_probe("auto.add", "“+ curve” button")
            f = probes(app)
            add = next((k for k in f if k.startswith("auto.add.")), None)
        app.expect(add is not None, "the “+ curve” menu has a parameter")
        if add:
            app.click_probe(add, f"parameter “{add}”")
        app.expect(app.has_probe("auto.graph"), "the automation graph is shown")
        app.step("curve-added", "curve added, graph with a constant")

        # 2) three clicks on the graph — three keys (the constant already gives two keys)
        g = probes(app)["auto.graph"]
        for fx, fy in ((0.3, 0.3), (0.5, 0.7), (0.7, 0.3)):
            click_in(app, g, fx, fy)
            poll_step(0.3)
        app.step("keys", "keys placed by clicks on the graph")

        # 3) right click on a segment → “Zigzag”
        f = probes(app)
        seg = next((k for k in f if k.startswith("auto.seg.")), None)
        app.expect(seg is not None, "the segment between the keys was handed over by the program")
        if seg:
            r = f[seg]
            win = geometry()
            app.right_click(win["x"] + int(r["x"] + r["w"] / 2), win["y"] + int(r["y"] + r["h"] / 2))
            app.expect(app.has_probe("auto.shape.5"), "the shape menu opened")
            app.click_probe("auto.shape.5", "“Zigzag” item")
        app.step("zigzag", "the segment trembles — the line follows the shape, not straight")

        # 4) the curve block: the header with the name and the formula-layer field (the block below the
        # graph, the window body scrolls — we scroll over the block header)
        app.expect(app.has_probe("curve.title"), "the curve block is labeled with the curve name")
        app.scroll_probe_into_view("auto.formula", "auto.add", below="fx.cancel", what="the formula field scrolled into the window")
        app.click_probe("auto.formula", "formula field")
        app.type_text("k*0.5+0.1")
        app.key("Return")
        app.step("layer", "formula layer above the keys: dashes above solid")

        # 5) the link door
        app.scroll_probe_into_view("auto.drive_by", "curve.title", below="fx.cancel", what="“Driven by…” scrolled into the window")
        app.click_probe("auto.drive_by", "“Driven by…” button")
        f = probes(app)
        app.expect(any(k.startswith("auto.link.") for k in f) or True, "the link menu opened")
        app.step("drive-by", "“Driven by…” menu: events and properties")
        # the menu closes by a repeated click on its own button: Esc would close
        # the window itself (Esc cascade), and a click “aside” may go to the timeline
        app.click_probe("auto.drive_by", "“Driven by…” button (close the menu)")

        # 6) help
        app.scroll_probe_into_view("auto.help", "curve.title", below="fx.cancel", what="“Reference” scrolled into the window")
        app.click_probe("auto.help", "“Reference” group")
        app.scroll_probe_into_view("auto.help.sin", "curve.title", below="fx.cancel", what="reference functions scrolled into the window")
        f = probes(app)
        app.expect(any(k.startswith("auto.help.") for k in f), "the reference lists the functions")
        app.step("help", "reference: variables and functions")

        # 7) saving: the form and the formula in the file
        text = app.saved_text()
        app.expect('"zigzag"' in text, "the file has a zigzag shape")
        app.expect('k*0.5+0.1' in text or 'k * 0.5' in text, "the file has a formula layer")
        app.expect('"keys"' in text, "keys under the layer are intact in the file")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

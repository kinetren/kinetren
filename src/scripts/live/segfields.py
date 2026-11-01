#!/usr/bin/env python3
"""Live scenario segment-shape-fields (kkt-26.11.01 item 10): the shape parameters of ONE
segment and of a SELECTION of segments, in a real window.

The owner's claim (verbatim, `docs/ktprompts/kkt-26.11.01.txt` line 19): “I want to set the
zigzag's parameters and there is nowhere to write them; if I set the zigzag for the whole
curve, then there is… and what if I have 100 segments with parameters?”

PATH (automation “The shape parameters of one segment and of a selection are edited by
fields”, ui-reach “Selecting segments is reachable by mouse and keyboard and is said”):

1. FX window of the event → “+ curve” → parameter; three clicks — five keys, four segments;
2. right click on the first segment → the menu header names the segment → “Zigzag”;
3. Ctrl+click on the second segment → both tinted; under “Shape of all segments” the row
   “Selected segments: 2” with “—” (a zigzag and a straight line);
4. the row's shape list → “Zigzag” → both segments tremble;
5. the row's Periods field: 12, Enter → both zigzags get denser, the rest untouched; the
   Amplitude field and the Profile list sit next to it; Ctrl+S → the file has two zigzags
   with 12 periods;
6. a drag over empty space across the whole graph → every segment tinted, the row says 4;
7. the row's shape list → “Cubic”, Tension 2, Enter → every segment becomes a smooth curve;
8. × at the end of the row → the row and the tint disappear.
"""
import os
import re
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, poll_step, require_tools  # noqa: E402
from kt_live import demo_otio  # noqa: E402

SEC = 90000


def probes(app):
    return {f["param"]: f for f in app.geom().get("fields", [])}


def at(r, fx, fy=0.5):
    win = geometry()
    return win["x"] + int(r["x"] + r["w"] * fx), win["y"] + int(r["y"] + r["h"] * fy)


def main(out):
    require_tools()
    path = os.path.join(out, "segfields.otio")
    demo_otio(path)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("segfields", out, project=path) as app:
        # 1) the FX window, a curve, five keys
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
        g = probes(app)["auto.graph"]
        for fx, fy in ((0.25, 0.3), (0.5, 0.7), (0.75, 0.3)):
            app.click(*at(g, fx, fy))
            poll_step(0.3)
        f = probes(app)
        app.expect("auto.seg.3" in f, "four segments between five keys")
        app.step("keys", "five keys, four segments")

        # 2) right click on the first segment → the header names it → Zigzag
        app.right_click(*at(f["auto.seg.0"], 0.5, 0.15))
        app.expect(app.has_probe("auto.shape.header"), "the segment menu names what it acts on")
        app.click_probe("auto.shape.5", "“Zigzag” item")
        app.step("zigzag-one", "the first segment trembles; the selection row shows its own values")

        # 3) Ctrl+click the second segment: a selection of two, the kinds are mixed
        f = probes(app)
        app.click(*at(f["auto.seg.1"], 0.5, 0.15), mods=("ctrl",))
        app.scroll_probe_into_view("shape_sel.kind", "auto.add", below="fx.cancel",
                                   what="the selection row scrolled into the window")
        app.step("selection-two", "two segments tinted, the row says 2 and shows “—”")

        # 4) the row's shape list → Zigzag
        app.click_probe("shape_sel.kind", "the selection row's shape list")
        app.click_probe("shape_sel.item.5", "“Zigzag” in the selection row")
        app.step("zigzag-both", "both selected segments tremble")

        # 5) Periods: 12, Enter
        app.click_probe("shape_sel.periods", "the selection row's Periods field")
        app.type_text("12")
        app.key("Return")
        app.step("periods-12", "both zigzags denser, the rest untouched")
        app.hover_probe("shape_sel.amp", "the selection row's Amplitude field")
        app.click_probe("shape_sel.profile", "the selection row's Profile list (open)")
        app.click_probe("shape_sel.profile", "the selection row's Profile list (close)")
        text = app.saved_text()
        zig12 = len(re.findall(r'"periods":\s*12(\.0)?\b', text))
        app.expect(zig12 >= 2, f"the file has two zigzags with 12 periods (found {zig12})")

        # 6) a sweep over empty space across the whole graph (the wheel turns over the curve
        # block's title, not over the graph: the graph takes the wheel for its own axis)
        app.scroll_probe_into_view("auto.graph", "curve.title", below="fx.cancel",
                                   what="the graph scrolled back into the window")
        g = probes(app)["auto.graph"]
        x0, yy = at(g, 0.02, 0.08)
        x1, _ = at(g, 0.98, 0.08)
        app.drag(x0, yy, x1, yy)
        app.scroll_probe_into_view("shape_sel.clear", "auto.add", below="fx.cancel",
                                   what="the selection row after the sweep")
        app.step("sweep", "every segment tinted, the row says 4")

        # 7) the whole selection becomes Cubic with tension 2
        app.click_probe("shape_sel.kind", "the selection row's shape list")
        app.click_probe("shape_sel.item.1", "“Cubic” in the selection row")
        app.click_probe("shape_sel.tension", "the selection row's Tension field")
        app.type_text("2")
        app.key("Return")
        app.step("cubic", "every selected segment is a smooth curve with tension 2")

        # 8) × clears the selection
        app.click_probe("shape_sel.clear", "× of the selection row")
        app.expect(app.wait_gone("shape_sel.kind"), "the selection row disappeared")
        app.step("cleared", "the row and the tint are gone")

        text = app.saved_text()
        app.expect(len(re.findall(r'"tension":\s*2(\.0)?\b', text)) >= 4,
                   "the file has four smooth segments with tension 2")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

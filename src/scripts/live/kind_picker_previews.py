#!/usr/bin/env python3
"""Live scenario kind-picker-previews (`v26.10` G6): the transition and generator kind pickers are
galleries of the frame after the click, as the effects picker.

The harness (t3110) drives the gallery in egui; here the real window and the real offline device:

1. a red clip and a blue clip with a crossfade between them; a double click on the transition opens
   its window; the Kind field opens a gallery of cells; after the misclick guard the cells show
   pictures (the cell areas are not one flat colour);
2. a click on the Iris cell: the transition is Iris, the gallery closes;
3. a generator (Solid) is created in the bin: its window opens; its Kind field opens a gallery of
   cells by group; a click on the Checker cell changes the kind.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, geometry, project_with_clips, require_tools  # noqa: E402
from trans import add_transition  # noqa: E402

SEC = 90000


def cell_spread(app, probe):
    """The largest per-channel spread inside a published cell: a picture of two colours or a pattern
    has one, a "Loading" placeholder is a dark plate with small text (a red and a blue have nearly
    the same brightness, so the channels are compared, not their sum)."""
    f = next((f for f in app.geom().get("fields", []) if f["param"] == probe), None)
    if not f:
        return 0
    win = geometry()
    x0, y0 = win["x"] + int(f["x"]), win["y"] + int(f["y"])
    w, h = int(f["w"]), int(f["h"] * 0.7)
    vals = []
    for i in range(1, 6):
        for j in range(1, 4):
            r, g, b = app.pixel(x0 + w * i // 6, y0 + h * j // 4)
            vals.append((r, g, b))
    return max(max(v[c] for v in vals) - min(v[c] for v in vals) for c in range(3))


def main(out):
    require_tools()
    red = color_media(out, "red", "C81E1E", seconds=10)
    blue = color_media(out, "blue", "1E3CC8", seconds=10)
    path = project_with_clips(os.path.join(out, "kind_picker.otio"), [(0.0, 4.0, red), (4.0, 4.0, blue)])
    add_transition(path, seconds=1.0)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("kind_picker_previews", out, project=path) as app:
        # --- 1. the transition's window, its kind gallery --------------------------------------
        app.double_click(app.x_at(4 * SEC), app.lane_y("video", 0, frac=0.8))
        app.expect(app.has_probe("win.transition"), "the double click opened the transition window")
        app.step("window", "the transition window, Kind: Crossfade")
        app.click_probe("tr.kind.pick", "the Kind field")
        app.expect(app.has_probe("trkind.iris"), "the gallery shows an Iris cell")
        app.expect(app.has_probe("trkind.crossfade"), "the current kind has a cell")
        f = next(f for f in app.geom()["fields"] if f["param"] == "trkind.iris")
        app.expect(f["w"] >= 79, f"a kind is a picture cell, not a row: {f}")
        drawn = app.until(lambda: cell_spread(app, "trkind.iris") > 60)
        app.step("gallery", "the kind gallery: each cell the cut at the window's middle with that kind")
        app.expect(drawn, f"the Iris cell shows a picture (spread {cell_spread(app, 'trkind.iris')})")

        # --- 2. a pick ------------------------------------------------------------------------------
        app.click_probe("trkind.iris", "the Iris cell")
        app.expect(app.wait_gone("trkind.iris"), "the gallery closed after the pick")
        text = app.saved_text()
        app.step("picked", "the window shows Iris")
        app.expect("iris" in text, "the transition in the saved project is Iris")
        app.key("Escape")
        app.expect(app.wait_gone("win.transition"), "Esc closed the transition window")

        # --- 3. a generator's kind gallery ------------------------------------------------------
        win = geometry()
        b = app.geom()["bin"]
        app.click(app.x_at(9 * SEC), app.ruler_y())
        app.right_click(win["x"] + int((b["left"] + b["right"]) / 2), win["y"] + int(b["bottom"]) - 10)
        app.click_probe("gen.new", "Create a generator")
        app.hover_probe("gen.new.group.bg", "the backgrounds group")
        app.click_probe("gen.new.solid", "Solid colour")
        app.expect(app.has_probe("genw.kind.pick"), "the generator's window opened with its Kind field")
        app.click_probe("genw.kind.pick", "the generator's Kind field")
        app.expect(app.has_probe("genw.kind.checker"), "the generator gallery shows a Checker cell")
        drawn = app.until(lambda: cell_spread(app, "genw.kind.checker") > 60)
        app.step("gen-gallery", "the generator kinds by group, each the event with that kind")
        app.expect(drawn, f"the Checker cell shows a picture (spread {cell_spread(app, 'genw.kind.checker')})")
        app.click_probe("genw.kind.checker", "the Checker cell")
        app.expect(app.wait_gone("genw.kind.checker"), "the gallery closed after the pick")
        text = app.saved_text()
        app.step("gen-picked", "the generator is a checker")
        app.expect("gen:checker" in text, "the generator in the saved project is a checker")


if __name__ == "__main__":
    main(sys.argv[1])

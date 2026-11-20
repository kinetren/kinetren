#!/usr/bin/env python3
"""Live scenario gen-backgrounds (`v26.10` G5, the backgrounds group): the moving backgrounds.

His words (2026-10-30, question 6, in the G5 row): all four groups — backgrounds: moving gradients,
particles, plasma, patterns; "not like in the 90s".

PATH (generators "The moving backgrounds"):

1. Edit → Create a generator… → Background → Plasma: an event at the cursor, its window open; the
   preview shows the plasma;
2. the playhead at 1 s → the picture changed by itself (no keyframes);
3. Edit → Create a generator… → Background → Pattern → its window shows the Stripes / Dots / Grid /
   Waves row; Dots → the picture changes.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def field(app, name):
    return next((f for f in app.geom().get("fields", []) if f["param"] == name), None)


def grid(app, frame):
    win = geometry()
    return [
        app.pixel(win["x"] + int(frame["x"] + frame["w"] * (i + 0.5) / 8), win["y"] + int(frame["y"] + frame["h"] * (j + 0.5) / 5))
        for j in range(5)
        for i in range(8)
    ]


def create(app, tag, what):
    app.click_probe("menu.edit", "the Edit menu")
    app.hover_probe("gen.new", "Create a generator…")
    app.hover_probe("gen.new.group.bg", "the Background group")
    app.click_probe(f"gen.new.{tag}", what)


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "gen_backgrounds.otio"), [(6.0, 2.0, "synth:Ramp")])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("gen_backgrounds", out, project=proj) as app:
        app.key("Home")
        create(app, "plasma", "Plasma")
        # the first frames after opening may not have drawn the picture yet: wait for its probe
        app.until(lambda: field(app, "preview.frame") is not None)
        frame = field(app, "preview.frame")
        app.expect(frame is not None, "the preview's picture is published")
        if not frame:
            return app.done()
        app.until(lambda: len(set(grid(app, frame))) > 6)
        first = grid(app, frame)
        app.step("plasma", "Plasma at 0 s: a palette of three colours flowing")
        app.expect(len(set(first)) > 6, f"the plasma has many colours: {len(set(first))}")
        app.click(app.x_at(SEC), app.ruler_y())
        moved = app.until(lambda: grid(app, frame) != first)
        app.step("plasma-1s", "at 1 s: the plasma moved by itself")
        app.expect(moved, "the plasma changed between 0 and 1 s with no keyframe")
        app.key("Escape")

        app.key("Home")
        create(app, "pattern", "Pattern")
        app.expect(app.has_probe("genc.pattern.dots"), "the pattern's window shows its choices")
        stripes = grid(app, frame)
        app.step("stripes", "Pattern: stripes at an angle; the window's Stripes / Dots / Grid / Waves row")
        app.click_probe("genc.pattern.dots", "Dots")
        dots = app.until(lambda: grid(app, frame) != stripes)
        app.step("dots", "Dots: the pattern became a field of dots")
        app.expect(dots, "choosing Dots changed the picture")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/gen_backgrounds"))

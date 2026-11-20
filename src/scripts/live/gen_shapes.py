#!/usr/bin/env python3
"""Live scenario gen-shapes (`v26.10` G5, the shapes and plates group).

His words (2026-10-30, question 6, in the G5 row): shapes and plates — star/polygon/arrow, lower thirds,
frames, progress bar.

PATH (generators "The shapes and plates"):

1. Edit → Create a generator… → Shapes → Star: a white-gold star over the clip under it (the clip shows
   round it — the star's frame is transparent);
2. Edit → Create a generator… → Shapes → Lower-third plate on the next track: a dark bar with an orange
   stripe at the lower third.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, geometry, project_with_clips, require_tools  # noqa: E402


def field(app, name):
    return next((f for f in app.geom().get("fields", []) if f["param"] == name), None)


def create(app, tag, what):
    app.click_probe("menu.edit", "the Edit menu")
    app.hover_probe("gen.new", "Create a generator…")
    app.hover_probe("gen.new.group.shapes", "the Shapes group")
    app.click_probe(f"gen.new.{tag}", what)
    app.key("Escape")


def main(out):
    require_tools()
    teal = color_media(out, "teal", "1E8C8C", seconds=6)
    proj = project_with_clips(os.path.join(out, "gen_shapes.otio"), [(0.0, 6.0, teal)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("gen_shapes", out, project=proj) as app:
        app.key("Home")
        frame = field(app, "preview.frame")
        app.expect(frame is not None, "the preview's picture is published")
        if not frame:
            return app.done()
        win = geometry()
        at = lambda fx, fy: (win["x"] + int(frame["x"] + frame["w"] * fx), win["y"] + int(frame["y"] + frame["h"] * fy))
        corner = app.pixel_settled(at(0.05, 0.1))
        create(app, "star", "Star")
        star = app.until(lambda: max(app.pixel(*at(0.5, 0.5))) > 200)
        app.step("star", "Star: a gold star over the teal clip, the clip round it")
        app.expect(star, f"the star's centre is lit: {app.pixel(*at(0.5, 0.5))}")
        app.expect(app.pixel(*at(0.05, 0.1)) == corner, "the star's frame is transparent: the clip shows in the corner")
        create(app, "plate", "Lower-third plate")
        bar = app.until(lambda: max(app.pixel(*at(0.3, 0.81))) < 80)
        app.step("plate", "Lower-third plate: a dark bar with an orange accent stripe at the lower third")
        app.expect(bar, f"the plate's bar is dark over the teal: {app.pixel(*at(0.3, 0.81))}")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/gen_shapes"))

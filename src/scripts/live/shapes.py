#!/usr/bin/env python3
"""Live scenario R.53 (items 417-424): a shape with alpha and a title animation.

Three things are checked that the harness does not see:

1. **The shape is placed from the grouped menu** (“Generator” → “Shapes” →
   “Rectangle”) and its ALPHA is honest: in the preview the rectangle is visible
   over the frame of the lower track, and next to it what is below shows through.
2. **The alpha rule (item 417)**: under the shape the picture remains, not a black
   slab — exactly what EventGrid broke by declaring the alpha lost.
3. **The title preset sets EVERYTHING in one step** (item 424): pressed “Typewriter” — the title
   started to type out; at ZERO the event has no letters, later they are there, and
   one Ctrl+Z returns the whole set of fields.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def new_generator(app, group, kind):
    """Create a generator via the bin background menu: group, then kind."""
    win = geometry()
    b = app.geom()["bin"]
    app.right_click(
        win["x"] + int((b["left"] + b["right"]) / 2),
        win["y"] + int(b["bottom"]) - 10,
    )
    app.click_probe("gen.new", "item “Create a generator”")
    app.click_probe(f"gen.new.group.{group}", f"group “{group}” in the submenu")
    app.click_probe(f"gen.new.{kind}", f"kind “{kind}” in the submenu")


def main(out):
    require_tools()
    # the bottom track is a light ramp: through the shape’s alpha it must be visible
    proj = project_with_clips(
        os.path.join(out, "shapes.otio"),
        [(0.0, 6.0, "synth:Ramp")],
    )

    with Live("shapes", out, project=proj) as app:
        app.shot("open")
        preview = app.preview_center()

        # cursor on the light frame of the bottom track
        app.click(app.x_at(int(2 * SEC)), app.ruler_y())
        ramp = app.pixel_settled(preview)
        app.expect(sum(ramp) > 30, f"the lower track frame is light: {ramp}")

        # 1. a rectangle from the “Shapes” group. We put it AFTER the end of the bottom
        # clip: an overlap on one track is a crossfade (R.28), and at the start of the overlap
        # the weight of the upper one is zero; the shape must be looked at
        # where it is alone.
        app.click(app.x_at(int(8 * SEC)), app.ruler_y())
        new_generator(app, "shapes", "rect")
        app.expect(app.has_probe("genw.color"), "shape parameters window opened")
        app.key("Escape")
        clips = app.live_clips()
        app.expect(len(clips) == 2, f"the shape was placed as a clip under the cursor: {clips}")
        app.shot("rect")

        # 2. the shape is DRAWN: a white rectangle in the centre of the frame, and
        # above — empty. The shape occupies 50×25 % of the frame, so the upper
        # quarter must stay empty: this is its ALPHA — it colours its own area, not the whole frame
        # (alpha rule, item 419).
        app.click(app.x_at(int(10 * SEC)), app.ruler_y())
        px = None
        for _ in range(30):
            px = app.pixel(*preview)
            if min(px) > 180:
                break
            poll_step(0.3)
        app.expect(
            px is not None and min(px) > 180,
            f"in the center of the preview a white shape: {px}",
        )
        g = app.geom()
        win = geometry()
        top_of_preview = (
            win["x"] + int(g["window"]["w"] * 0.5),
            win["y"] + int(g["timeline"]["top"] * 0.14),
        )
        edge = app.pixel(*top_of_preview)
        app.expect(
            max(edge) < 40,
            f"above the shape there must be emptiness, not a white slab across the whole frame: {edge}",
        )
        app.shot("alpha")

        # 3. the shape is removed: further on we need a clean stretch under the title, and
        # an overlap on one track is a crossfade
        app.click(app.x_at(int(10 * SEC)), app.lane_y("video", 0, 0.6))
        app.key("Delete")
        app.expect(len(app.live_clips()) == 1, f"shape deleted: {app.live_clips()}")

        # 4. the title with the “Typewriter” preset: it is printed by clusters
        app.click(app.x_at(int(8 * SEC)), app.ruler_y())
        new_generator(app, "titles", "text")
        app.expect(app.has_probe("titlew.text"), "title window opened")
        app.click_probe("titlew.text", "title text field")
        for ch in ["k", "t", "9"]:
            app.key(ch)
        app.click_probe("titlew.preset.2", "preset “Typewriter”")
        app.key("Escape")
        app.shot("typewriter")

        clips = app.live_clips()
        app.expect(len(clips) == 2, f"title placed as a clip: {clips}")
        t_start = clips[-1][0]

        # at the START of the event there are no letters (the typewriter has not yet printed one),
        # a second later there are. We take the brightness over the AREA, not the point:
        # a size 7 % of the frame gives a thin stroke, and hitting it with the centre
        # of the preview is a lottery.
        app.click(app.x_at(int(t_start + SEC / 10)), app.ruler_y())
        app.settle()
        zero = app.ink_box(preview, half=150)
        app.click(app.x_at(int(t_start + int(1.8 * SEC))), app.ruler_y())
        app.settle()
        later = app.ink_box(preview, half=150)
        app.expect(
            later > zero + 40,
            f"the typewriter types: at zero {zero:.0f}, later {later:.0f}",
        )
        app.shot("revealed")

        # 5. one Ctrl+Z returns the WHOLE preset set
        app.key("ctrl+z")
        app.saved_project()
        with open(app.project) as f:
            text = f.read()
        app.expect(
            '"typewriter"' not in text,
            "one Ctrl+Z must undo the preset entirely",
        )
        app.shot("undone")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

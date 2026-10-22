#!/usr/bin/env python3
"""Live scenario R.54 (item 431): the effects picker gallery in the REAL program.

The harness tests (t547/t548) run the click and the layout inside one process on an
artificial clip. Here we check what the harness cannot check:
thumbnails are drawn ON THE FRAME of real media (the background thread managed to compute
the chain and upload the texture), and a click on a cell reaches the project file with the
parameters of EXACTLY this variant.

A popup is not found in the accessibility tree, so the coordinates are taken from the
program itself: `KT_GEOM_DUMP` gives the rectangles `fxreg.add` and
`fxreg.var.<kind>.<number>` (rule R.27 — the scenario does not guess the offsets).
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, geometry, poll_step, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401

SEC = 90000
# “Dense” — the second blur variant (radius 24 against the template 8)
VARIANT = "fxreg.var.blur.1"
WANT_RADIUS = 24


def radius_in(text):
    """Radii of all blur effects in the saved project (the text of the file
    project: the chain of effects is part of the MODEL)."""
    doc = json.loads(text)
    found = []

    def walk(node):
        if isinstance(node, dict):
            if node.get("kind") == "blur":
                found.append(node.get("radius"))
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)

    walk(doc)
    return found


def main(out):
    require_tools()
    proj = os.path.join(out, "fxpicker.otio")
    demo_otio(proj)

    with Live("fxpicker", out, project=proj) as app:
        # 1) the FX window by a double click (the gap between clicks is SMALL, item 232)
        y = app.lane_y("video", 0, frac=0.5)
        before = app.shot("before")
        app.double_click(app.x_at(3 * SEC), y)
        app.shot("fxwin")
        d = app.screen_diff(before)
        app.expect(d > 0.01, f"FX window opened (the screen changed by {d:.1%})")

        # 2) the gallery: the picker button gives up its rectangle, a click on it
        # opens the popup
        pre_picker = app.shot("pre-picker")
        if not app.click_probe("fxreg.add", "the program gave the rectangle of the picker button"):
            return app.done()
        app.shot("gallery")
        d = app.screen_diff(pre_picker)
        app.expect(d > 0.02, f"gallery expanded (the screen changed by {d:.1%})")

        # 2b) we narrow by search, as a person does: 27 rows do not fit whole into the scroll,
        # and the cells BELOW THE FOLD are neither drawn nor counted at all.
        # The search line gets focus by itself when the menu opens.
        for ch in "blur":
            app.key(ch)
        app.shot("filtered")

        # 3) MINIATURES ARE COUNTED: the variant cell has stopped being an empty
        # tile. We wait not “until it settles”, but for a CHANGE (rule R.37): a background
        # thread computes the chain and fills the texture only after the expansion.
        cell = None
        for _ in range(40):
            f = next(
                (f for f in app.geom().get("fields", []) if f["param"] == VARIANT),
                None,
            )
            if f:
                cell = f
                break
            poll_step(0.25)
        app.expect(cell is not None, f"the program gave the rectangle of the cell “{VARIANT}”")
        if cell is None:
            return app.done()
        win = geometry()
        # the centre of the cell’s PICTURE (the caption goes below the picture)
        px = (
            win["x"] + int(cell["x"] + cell["w"] / 2),
            win["y"] + int(cell["y"] + cell["h"] * 0.35),
        )
        drawn = app.pixel_settled(px, differs_from=None)
        app.expect(
            drawn is not None and sum(drawn) > 24,
            f"thumbnail drawn, not left as a black plate (pixel {drawn})",
        )

        # 4) FORCE AXIS: the drag horizontally from the “Dense” cell to the
        # neighbouring one on the left must give an INTERMEDIATE value — what
        # the person saw under the cursor, not the named variant.
        win = geometry()
        cx = win["x"] + int(cell["x"] + cell["w"] / 2)
        cy = win["y"] + int(cell["y"] + cell["h"] * 0.35)
        before_radii = radius_in(app.project_text())
        app.drag(cx, cy, cx - int(cell["w"] / 2), cy)
        app.shot("axis")
        app.saved_project()
        axis_radii = [r for r in radius_in(app.project_text()) if r not in before_radii]
        app.expect(
            len(axis_radii) == 1,
            f"the wiring added exactly one blur (was {before_radii}, became {radius_in(app.project_text())})",
        )
        if axis_radii:
            app.expect(
                3 < axis_radii[0] < WANT_RADIUS,
                f"the axis gave an INTERMEDIATE value between 3 and {WANT_RADIUS}: {axis_radii[0]}",
            )
        # undo: the axis added the effect in one history step, and Ctrl+Z must
        # remove it — otherwise the next check would measure the sum of two gestures
        app.key("ctrl+z")
        app.saved_project()
        after_undo = radius_in(app.project_text())
        app.expect(
            axis_radii and axis_radii[0] not in after_undo,
            f"Ctrl+Z removed what the axis added (left {after_undo})",
        )

        # 5) a click on a cell — and in the FILE exactly its parameters
        before_radii = after_undo
        app.click_probe("fxreg.add", "the picker opens again")
        for ch in "blur":
            app.key(ch)
        if not app.click_probe(VARIANT, "the variant cell is clickable"):
            return app.done()
        app.shot("after-pick")
        app.saved_project()
        after_radii = radius_in(app.project_text())
        app.expect(
            len(after_radii) == len(before_radii) + 1,
            f"exactly one new blur appeared in the project ({before_radii} → {after_radii})",
        )
        new = [r for r in after_radii if r not in before_radii] or after_radii
        app.expect(
            WANT_RADIUS in new,
            f"a VARIANT was added (radius {WANT_RADIUS}), not a template: {after_radii}",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

#!/usr/bin/env python3
"""Live scenario R.63 (items 3-5): formula, focus and the shape menu in a real window.

Three claims of the owner, and all three were green in the harness:

3. “I do not understand how to enter formulas into automation curves — I write
   and it either erases the formula or nothing happens at all”;
4. “if you start typing something in the formula field, the input focus goes there
   and no matter how I poke the rest of the interface, pressing the
   space does not lead to a pause… until I close the effect window”;
5. “it is impossible to specify the shapes of curves… the context menu just disappears, I do not
   manage to reach it”.

The scenario acts by hand: it opens the effect window with a double click, types the
formula symbol by symbol, presses Enter, clicks on the timeline and checks the SPACE —
whether the program plays. The state is read from the geometry dump, not from the picture.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, geometry, pause, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401

SEC = 90000


def demo(dest):
    """Ordinary demo project: we will create the curve BY HAND, like a person."""
    demo_otio(dest)
    return dest


def main(out):
    require_tools()
    path = demo(os.path.join(out, "formula.otio"))
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("formula", out, project=path) as app:
        # 1) the effect window by a double click on the event
        y = app.lane_y("video", 0, frac=0.5)
        app.double_click(app.x_at(1 * SEC), y)
        app.expect(app.has_probe("auto.add"), "the effect window opened")
        app.shot("fxwin")

        # 1b) we set the curve BY HAND — by the same path as the person
        app.click_probe("auto.add", "“+ curve” button")
        # WE DO NOT TAKE WARP INTO THE LIST: the speed has only a straight form (t698), and
        # the segment menu would honestly show a refusal instead of the list of forms
        added = next(
            (
                f["param"]
                for f in app.geom().get("fields", [])
                if f["param"].startswith("auto.add.") and not f["param"].endswith(".rate")
            ),
            None,
        )
        app.expect(added is not None, "the “+ curve” menu showed the parameters")
        if added:
            app.click_probe(added, f"item {added}")
        app.expect(app.has_probe("auto.formula"), "the formula field is shown after the curve is created")

        # 2) FORM MENU: right click on the segment — and the menu MUST be reached
        # (“the context menu simply vanishes, I do not have time to reach
        # it”). The pointer leaves the click point for the item — the menu must
        # stay, and the form must land on the segment between the same two keys.
        app.right_click_probe("auto.seg.0", "the curve segment between the first keys")
        app.expect(app.has_probe("auto.shape.2"), "right click on the segment opened the list of shapes")
        app.shot("shapemenu")
        app.hover_probe("auto.shape.2", "“step” item")
        app.expect(
            app.has_probe("auto.shape.2"),
            "the menu is in place while the pointer reaches the item",
        )
        app.click_probe("auto.shape.2", "“step” item")
        saved = app.saved_project()  # .kt on disk, access to the envelope — through the export door
        app.expect(
            '"hold"' in open(saved.otio_path, encoding="utf-8").read(),
            "the segment shape reached the file",
        )

        # 3) TYPING BY CHARACTER: the text must not vanish, the keys must not die
        #
        # The field is scrolled into the window, not taken by old coordinates: in
        # R.67 the curve block grew (segment form, layer, help, link), the field
        # went under the bottom of the window body — a blind click hit ANOTHER control and
        # the scenario “did not see” the formula, although the program was sound.
        app.scroll_probe_into_view(
            "auto.formula", "auto.add", below="fx.cancel",
            what="the formula field scrolled into the window",
        )
        app.click_probe("auto.formula", "formula field")
        app.type_text("sin(t)*3+5")
        app.shot("typed")
        before = app.project_now().doc
        app.key("Return")
        saved = app.saved_project()
        doc = saved.doc
        formulas = []

        def walk(x):
            if isinstance(x, dict):
                src = x.get("source")
                if isinstance(src, dict) and src.get("kind") == "formula":
                    formulas.append(src.get("expr"))
                for v in x.values():
                    walk(v)
            elif isinstance(x, list):
                for v in x:
                    walk(v)

        walk(doc)
        app.expect(
            any("sin(t)*3+5" in str(f) for f in formulas),
            f"the formula reached the file in full: {formulas}",
        )
        _formula_walk = walk
        # R.67, item 6 of the owner: the formula is a LAYER over the keys, not a source
        # instead of them; “I may have changed my mind and would like to go back to the zigzag”
        keys_after = []

        def walk_keys(x):
            if isinstance(x, dict):
                if isinstance(x.get("keys"), list):
                    keys_after.append(len(x["keys"]))
                for v in x.values():
                    walk_keys(v)
            elif isinstance(x, list):
                for v in x:
                    walk_keys(v)

        walk_keys(doc)
        app.expect(
            any(n > 0 for n in keys_after),
            f"keys under the formula layer are intact: {keys_after}",
        )
        _ = before, saved

        # 3b) “UNBIND” returns the curve to ordinary keys.
        #
        # MOVED FROM `curvesrc.py` (R.68, task 5.2). That scenario
        # asserted about the model and the file exactly what t709 (broken text
        # does not touch the model) and t749 (a correct one is applied in full) hold, and cost
        # 57 s — of which 15 went on 24 BackSpace presses. The only thing
        # that lived only there was the GESTURE on the “unbind” button: it stays
        # alive — here, in the same window and on the same curve.
        app.scroll_probe_into_view(
            "auto.unlink", "auto.add", below="fx.cancel",
            what="“unlink” button scrolled into the window",
        )
        app.click_probe("auto.unlink", "“unlink” button on a formula curve")
        saved = app.saved_project()
        # the same walk, but into an EMPTY list: `walk` puts what it finds into
        # `formulas` from the enclosing area, so the list is substituted here
        formulas = []
        _formula_walk(saved.doc)
        app.expect(
            not formulas,
            f"“unlink” must return the ordinary keys, and the formula remained: {formulas}",
        )

        # 3c) “DELETE CURVE” — also a gesture that was never reached once
        # (R.68, task 5.3). The button stands at the named curve, next to
        # “unbind”; the consequence — the formula field disappears together with the curve.
        app.scroll_probe_into_view(
            "auto.remove", "auto.add", below="fx.cancel",
            what="“Delete curve” button scrolled into the window",
        )
        app.click_probe("auto.remove", "“Delete curve” button")
        app.settle()
        app.expect(
            not app.has_probe("auto.formula", tries=3),
            "the deleted curve took its formula field with it",
        )

        # 3d) the window closes by its OWN “Done” button — and opens again
        # by the same double click (R.68, task 5.3: before, the scenarios left
        # the effects window only with Esc, and the person never reached the button
        # once). The check stands HERE, before the play: afterwards the cursor will move, the view
        # will move with it, and `x_at` will give a coordinate past the window edge (“`x_at`
        # can only do what is ON SCREEN”, R.37).
        app.click_probe("fx.done", "“Done” button of the effects window")
        app.settle()
        app.expect(
            not app.has_probe("auto.add", tries=3),
            "“Done” closed the effects window",
        )
        # we aim at the MIDDLE of the edit, not the first second: the track label
        # lies ON TOP of the start of the row, and a double click there opens
        # the rename of V1 instead of the effects window (the same trap described
        # at `wait_loaded` in the frame)
        app.double_click(app.x_at(15 * SEC), y)
        app.expect(app.has_probe("auto.add"), "the effect window opened again with the same gesture")

        # 4) FOCUS: a click on the timeline — and the space bar PLAYS
        app.click(app.x_at(2 * SEC), y)
        was = app.playing()
        app.key("space")
        pause(
            1.0,
            "playback runs in REAL time: a second of play is itself "
            "the check, not waiting for a reaction of the interface",
        )
        now = app.playing()
        app.expect(
            now != was,
            f"after a click on the timeline the space plays with the effect window OPEN ({was} → {now})",
        )
        if now:
            app.key("space")

        # 5) the window’s BUTTON must not take the keyboard for good either
        app.click_probe("fxreg.add", "picker button in the effect window")
        app.key("Escape")  # close the picker popup
        was = app.playing()
        app.key("space")
        pause(
            1.0,
            "playback runs in REAL time: a second of play is itself "
            "the check, not waiting for a reaction of the interface",
        )
        now = app.playing()
        app.expect(
            now != was,
            f"after a click on the BUTTON of the window the space plays ({was} → {now})",
        )
        if now:
            app.key("space")
        app.shot("after-space")

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/formula"))

#!/usr/bin/env python3
"""Live scenario vhs-tape-simulation (kkt-26.11.01 V1): the VHS tape simulation in the REAL program.

The harness (t1193) clicks the deck face inside one process; here the person's path runs in the
program they launch: open an event's effects, add "VHS Tape" from the picker, see the preview
change (the GPU graph runs on the frame), click head B on the drawn drum, make it chipped, switch
to LP and to two decks — and the file holds exactly those physical quantities.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, require_tools  # noqa: E402
from kt_live import demo_otio  # noqa: E402

SEC = 90000


def tape_rows(text):
    """The vhs_tape slots of the saved project: their rows by name."""
    found = []

    def walk(node):
        if isinstance(node, dict):
            if node.get("kind") == "vhs_tape":
                found.append(node)
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)

    walk(json.loads(text))
    return found


def main(out):
    require_tools()
    proj = os.path.join(out, "vhs_tape.otio")
    demo_otio(proj)

    with Live("vhs_tape", out, project=proj) as app:
        y = app.lane_y("video", 0, frac=0.5)
        app.double_click(app.x_at(3 * SEC), y)
        app.step("fxwin", "double click on a video event → its effects window")

        if not app.click_probe("fxreg.add", "the program gave the rectangle of the picker button"):
            return app.done()
        for ch in "vhs tape":
            app.key("space" if ch == " " else ch)
        app.step("picker", "+ effect, typed “vhs tape” → the gallery shows VHS Tape")
        before = app.shot("before-tape")
        cell = None
        for _ in range(40):
            if any(f["param"] == "fxreg.tpl.vhs_tape" for f in app.geom().get("fields", [])):
                cell = True
                break
            poll_step(0.25)
        app.expect(cell is not None, "the picker shows the VHS Tape row")
        if not app.click_probe("fxreg.tpl.vhs_tape", "the VHS Tape row has a rectangle"):
            return app.done()
        # the face is drawn on the frames after the click: wait for its places, not a pause
        names = set()
        for _ in range(40):
            names = {f["param"] for f in app.geom().get("fields", [])}
            if "deck.head.0.1" in names:
                break
            app.wait_frame(1, what="the deck face after the pick", soft=True)
        app.wait_frame(2, what="the preview through the tape", soft=True)
        app.step("deck-face", "picked → a deck face: a drum with heads A and B, SP/LP/EP, the tracking")
        d = app.screen_diff(before)
        app.expect(d > 0.01, f"the window and the preview changed after adding the tape ({d:.1%})")
        for p in ("deck.head.0.0", "deck.head.0.1", "deck.speed.1", "tracking_um"):
            app.expect(p in names, f"the deck face shows “{p}”")

        if not app.click_probe("deck.head.0.1", "head B on the drum"):
            return app.done()
        app.wait_frame(2, what="the condition row", soft=True)
        app.step("head-b", "click head B on the drum → its condition row: Clean / Dirty / Clogged / Chipped")
        if not app.click_probe("deck.cond.Chipped", "the condition “Chipped”"):
            return app.done()
        app.wait_frame(2, what="the chipped head redrawn", soft=True)
        app.step("chipped", "Chipped → head B turns orange, its output −6 dB and gap 0.6 µm")
        app.click_probe("deck.speed.1", "the LP switch")
        app.wait_frame(2, what="LP redrawn", soft=True)
        app.step("lp", "LP → the narrower track: the picture is noisier, not softer")
        app.click_probe("deck.decks.1", "two decks")
        app.wait_frame(2, what="the second drum", soft=True)
        app.step("two-decks", "Two decks → a record drum and a play drum side by side")

        rows = tape_rows(app.saved_text())
        app.expect(len(rows) == 1, f"the file holds one VHS tape slot ({len(rows)})")
        if rows:
            r = rows[0]
            app.expect(abs(float(r.get("rec_b_output", 0)) + 6.0) < 1e-4, f"head B's output is −6 dB in the file ({r.get('rec_b_output')})")
            app.expect(abs(float(r.get("rec_b_gap", 0.3)) - 0.6) < 1e-4, f"head B's gap is 0.6 µm in the file ({r.get('rec_b_gap')})")
            app.expect(r.get("speed") == "lp", f"the speed is LP in the file ({r.get('speed')})")
            app.expect(r.get("decks") == "two", f"two decks in the file ({r.get('decks')})")
            app.expect("rec_a_output" not in r, "head A's untouched rows are not written")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

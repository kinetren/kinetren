#!/usr/bin/env python3
"""Live scenario R.66: the FORM CONTRACT in a real window.

The direct request of the owner (R.64): “and do not forget for the future to go through all the other
widgets and windows too”. The harness holds the contract
by machine (t779, t780), but it cannot show “whether the button is visible and whether it is clickable on a low
screen”: the egui window in the harness is squeezed by its own
rectangle, while here the window is real.

Scenario:

1. shrinks the program window to a low one (1100×640);
2. opens the effects window with a double click on an event;
3. makes sure the action buttons are VISIBLE inside the window and are pressed;
4. collapses the “Parameters” group and sees that the fields disappeared, and after a
   repeated click they came back — the collapse works and breaks nothing.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, geometry, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401

SEC = 90000


def probes(app):
    return {f["param"]: f for f in app.geom().get("fields", [])}


def main(out):
    require_tools()
    path = os.path.join(out, "forms.otio")
    demo_otio(path)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("forms", out, project=path) as app:
        # 1) a LOW window — where buttons are and slide past the edge
        app.resize(1100, 640)
        g = app.geom()
        print(f"   window {g['window']['w']:.0f}×{g['window']['h']:.0f}")

        # 2) effects window by a double click on the event
        #
        # we aim at the MIDDLE of the edit, not the first second: the track label
        # lies ON TOP of the start of the row, and in a narrow window (which we ourselves narrowed)
        # a double click there opens the rename of V1 instead of the effects window.
        # The same trap was in `formula` (R.68) and is described at `wait_loaded`.
        y = app.lane_y("video", 0, frac=0.5)
        app.double_click(app.x_at(15 * SEC), y)
        app.expect(app.has_probe("fx.cancel"), "the effects window opened")
        app.shot("fx-low")

        # 3) ACTION buttons are visible inside the window and are clicked
        f = probes(app)
        win = geometry()
        for name in ("fx.done", "fx.cancel"):
            r = f.get(name)
            app.expect(r is not None, f"button “{name}” handed over by the program")
            if r:
                inside = 0 <= r["y"] and r["y"] + r["h"] <= g["window"]["h"]
                app.expect(inside, f"button “{name}” is visible in the window (bottom {r['y'] + r['h']:.0f})")

        # 4) the group collapses and expands
        grp = next((k for k in f if k.startswith("grp.")), None)
        app.expect(grp is not None, "the form has a collapsible group")
        if grp:
            before = len(probes(app))
            app.click_probe(grp, "group heading")
            # THE FIELD COUNT IS READ AFTER SILENCE (R.69): the collapse in egui is
            # animated, and while it moves, the fields are still drawn and honestly
            # publish their rectangles. A fixed pause inside the click used to hide this;
            # now the click waits for the READ input, and “the picture has settled” is
            # a separate wait, requested by whoever reads the dump directly
            # (the same goes for `conform.py`).
            app.settle()
            folded = len(probes(app))
            app.expect(folded < before, f"the collapse hid the fields: was {before}, became {folded}")
            app.click_probe(grp, "group heading (back)")
            app.settle()
            back = len(probes(app))
            app.expect(back >= before - 1, f"expansion brought the fields back: {back} against {before}")
        app.shot("fx-folded")

        # the “Cancel” button really closes the window
        app.click_probe("fx.cancel", "“Cancel” button of the effects window")
        app.expect(not app.has_probe("fx.cancel", tries=4), "the window closed by the button")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out")

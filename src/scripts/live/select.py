#!/usr/bin/env python3
"""Live scenario R.33: the selection frame and moving a batch.

The user’s words (item 7): “the main timeline lacks area-selection…
one can select via area by holding ctrl… and this must naturally be
taken into account everywhere”. The harness tests t202-t205 check this in simulation, but here —
a REAL window: the mouse is driven by XTest with Ctrl held, and the result is read from the
.otio saved by the program.

The chain the person uses is checked: a marquee took an event →
a Ctrl+click added a second → one was dragged, BOTH moved, each by the same
shift, the durations intact.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "select.otio"),
        [(0.0, 4.0, "synth:Ramp"), (6.0, 4.0, "synth:Dc")],
    )

    with Live("select", out, project=proj) as app:
        y = app.lane_y("video", 0, frac=0.72)
        app.shot("open")

        p = app.project_now()
        c1, c2 = p.clip("c1"), p.clip("c2")
        app.expect(
            c1.start == 0 and c2.start == 6 * SEC, f"starting montage: {p.clips}"
        )

        # 1. Ctrl+drag from an EMPTY place (gap 4..6 s) to the right past the end of c2:
        # the frame must take c2 and NOT create a region
        app.drag(
            app.x_at(5 * SEC), y, app.x_at(11 * SEC), y, mods=("ctrl",)
        )
        app.shot("after-rubber")
        p = app.saved_project()
        region = (
            p.doc.get("metadata", {}).get("kt", {}).get("transport", {}).get("region")
        )
        app.expect(not region, f"Ctrl+drag created a region instead of a frame: {region}")

        # 2. Ctrl+click on the first event adds it to the selection
        app.click(app.x_at(2 * SEC), y, mods=("ctrl",))
        app.shot("after-ctrl-click")

        # 3. drag the FIRST event to the right by 2 s — both must move
        app.drag(app.x_at(2 * SEC), y, app.x_at(4 * SEC), y)
        app.shot("after-block-move")
        p = app.saved_project()
        c1, c2 = p.clip("c1"), p.clip("c2")
        app.near(c1.start, 2 * SEC, 27000, "the first event moved by 2 s")
        app.near(c2.start, 8 * SEC, 27000, "the second event of the batch moved by the same amount")
        app.near(c1.duration, 4 * SEC, 3000, "duration c1 intact")
        app.near(c2.duration, 4 * SEC, 3000, "duration c2 intact")
        app.expect(not p.transitions, f"moving the batch does not create transitions: {p.transitions}")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/select"))

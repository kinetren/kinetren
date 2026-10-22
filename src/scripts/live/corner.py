#!/usr/bin/env python3
"""Live scenario R.28 item 12: a fade by the upper corner of an event.

Words of the user: “in sony vegas fade can be made on events at the edges without
more events — just by dragging the upper corner of the event”. Checked
in a REAL window: the mouse is driven by XTest, and the result is read from the .otio saved
by the program — including the fact that the event itself is NOT touched in the process
(“nothing should grow or cut off anywhere”, item 6).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "corner.otio"),
        [(2.0, 8.0, "synth:Ramp")],
    )

    with Live("corner", out, project=proj) as app:
        y_body = app.lane_y("video", 0, frac=0.72)
        y_strip = app.lane_y("video", 0, frac=0.06)  # the top strip of the event
        app.shot("open")

        p = app.project_now()
        c1 = p.clip("c1")
        app.expect(c1.duration == 8 * SEC and not p.transitions, f"start: {p.clips}")

        # we select the event — the fade handles are visible only on the selected one
        app.click(app.x_at(c1.mid()), y_body)
        app.shot("selected")

        # we drag the LEFT top corner to the right by 2 s
        app.drag(app.x_at(c1.start + 200), y_strip, app.x_at(c1.start + 2 * SEC), y_strip)
        p = app.saved_project()
        app.expect(len(p.transitions) == 1, f"the corner started the fade: {p.transitions}")
        win = p.transitions[0]
        app.expect(win["in"] == 0, f"there is no external offset — the fade is inside the event: {win}")
        app.near(win["out"], 2 * SEC, SEC // 4, "fade length ≈ the distance traveled")
        c1 = p.clip("c1")
        app.expect(
            c1.duration == 8 * SEC and c1.source_in == 0,
            f"event untouched: {p.clips}",
        )
        app.shot("after-fade")

        # we push the corner back — the fade disappears entirely
        app.drag(app.x_at(c1.start + 2 * SEC), y_strip, app.x_at(c1.start + 100), y_strip)
        p = app.saved_project()
        app.expect(not p.transitions, f"corner pushed back — no fade: {p.transitions}")
        c1 = p.clip("c1")
        app.expect(
            c1.duration == 8 * SEC and c1.source_in == 0,
            f"event still whole: {p.clips}",
        )
        app.shot("after-undo-fade")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

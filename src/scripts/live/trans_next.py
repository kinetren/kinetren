#!/usr/bin/env python3
"""Live scenario generator-from-track, the transition half (`v26.10` G1): "Transition with next" is
one plain item that makes a dissolve with the next event and opens its window.

His words (line 12, translated): the transitions' right-click lists are the same cumbersome nonsense;
changing the kind in the window itself is enough.

PATH (ui-reach "A track's and an event's right-click menus create, the window picks the kind"):

1. two events end to end; a right click on the first one's body → "Transition with next" (no list of
   kinds);
2. a dissolve joins the two events and its window is open with the Kind field.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "trans_next.otio"),
        [(0.0, 4.0, "synth:Ramp"), (4.0, 4.0, "synth:Ramp")],
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("trans_next", out, project=proj) as app:
        p = app.project_now()
        app.expect(len(p.clips) == 2 and not p.transitions, f"start: two events, no transition: {p.clips}")
        app.right_click(app.x_at(2 * SEC), app.lane_y("video", 0, 0.7))
        app.expect(app.has_probe("tl.trans.next"), "the event's menu offers Transition with next")
        app.expect(not app.has_probe("tl.trans.crossfade", tries=2), "the menu lists no transition kinds")
        app.step("menu", "right click on the first event: one “Transition with next” item")
        app.click_probe("tl.trans.next", "Transition with next")
        app.expect(app.has_probe("win.transition"), "the transition window opened by itself")
        app.expect(app.has_probe("tr.kind.pick"), "the window shows its Kind field")
        app.step("window", "a dissolve between the events, its window open with the Kind field")
        tl = app.saved_project()
        app.expect(len(tl.transitions) == 1, f"one transition joins the events: {tl.transitions}")
        app.expect("crossfade" in app.saved_text(), "the transition is a dissolve")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/trans_next"))

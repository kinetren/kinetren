#!/usr/bin/env python3
"""Live scenario R.72, S1: “a convenient way to 'stop' an event: I want a freeze frame, and then
continue it”. The menu item is pressed for real; the pieces are read from the live model, the
played picture from the screen: it stands for 2 s and goes on from the same frame.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "freeze.otio"), [(0.0, 10.0, "synth:Ramp")])
    with Live("freeze_frame", out, project=proj) as app:
        app.click(app.x_at(4 * SEC), app.ruler_y())
        app.wait_playhead(lambda t: abs(t - 4 * SEC) < 3000, "the cursor stands at 4 s")
        app.step("cursor", "the cursor on the event at 4 s")

        y = app.lane_y("video", 0, frac=0.6)
        app.right_click(app.x_at(7 * SEC), y)
        app.step("menu", "the event's menu with “Insert frame hold (freeze frame)”")
        app.click_probe("clip.frame_hold", "the frame hold item")
        app.settle()
        clips = app.live_clips()
        app.expect(len(clips) == 3, f"three pieces: {clips}")
        app.expect(clips[1] == (4 * SEC, 2 * SEC), f"a hold of 2 s at 4 s: {clips}")
        app.expect(sum(d for _, d in clips) == 10 * SEC, f"the event keeps its length: {clips}")
        app.step("pieces", "three pieces: before, the 2 s hold, the rest")

        # the picture stands inside the hold and moves after it
        centre = app.preview_center()
        app.click(app.x_at(4 * SEC + SEC // 2), app.ruler_y())
        app.settle()
        a = app.pixel_settled(centre)
        app.click(app.x_at(5 * SEC + SEC // 2), app.ruler_y())
        app.settle()
        b = app.pixel_settled(centre)
        app.expect(a == b, f"the picture must stand in the hold: {a} vs {b}")
        app.click(app.x_at(8 * SEC), app.ruler_y())
        app.settle()
        app.step("after", "after the hold the picture goes on")

        app.key("ctrl+z")
        app.settle()
        clips = app.live_clips()
        app.expect(len(clips) == 1, f"one undo returns one event: {clips}")
        app.step("undone", "one undo: one event again")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

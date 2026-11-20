#!/usr/bin/env python3
"""Live scenario bin-drag-moment (F-MED-009, `v26.10` MW1, the owner's answer U9 of kkt-26.11.18b):
a bin card dragged to the timeline lands whole with the scrubbed moment under the mouse; Alt pressed
mid-drag puts its start there, Alt+Shift its end; the ghost shows the event before the drop.

PATH (ui-timeline "A bin card lands with the scrubbed moment under the mouse"):

1. a 12 s file in the bin, half a second of it on V1 from 0; its card is pressed at its middle (6 s)
   and carried to V1 at 12.5 s: before the release the ghost is 12 s long and starts at 6.5 s, a white
   tick at the mouse; released, the new event starts at 6.5 s;
2. the same drag with Alt pressed over the lane: the ghost jumps to start at the mouse; released, the
   event starts at 12.5 s;
3. with Alt+Shift: the ghost ends at the mouse; released, the event ends at 12.5 s (starts at 0.5 s).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000
DROP = 25 * SEC // 2  # 12.5 s: inside the view, and the end-landing starts at 0.5 s, after V1's event


def field(app, name):
    return next((f for f in app.geom().get("fields", []) if f["param"] == name), None)


def main(out):
    require_tools()
    teal = color_media(out, "teal", "1E8C8C", seconds=12)
    path = project_with_clips(os.path.join(out, "bin_drag_moment.otio"), [(0.0, 0.5, teal)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("bin_drag_moment", out, project=path) as app:
        app.until(lambda: field(app, "bin.file") is not None)
        card = field(app, "bin.file")
        app.expect(card is not None, "the file card is shown in the bin")
        if not card:
            return app.done()
        win = geometry()
        press = (win["x"] + int(card["x"] + card["w"] * 0.5), win["y"] + int(card["y"] + card["h"] / 2))
        drop = (app.x_at(DROP), app.lane_y("video", 0))


        def carry(mods, name, what, want_start):
            known = {c["id"] for c in app.geom().get("clips", [])}
            seen = {}

            def look():
                seen["ghost"] = field(app, "tl.drop_ghost") if app.has_probe("tl.drop_ghost", tries=1) else None
                app.step(name, what)

            app.hover(*press)
            # the card knows its file's length before the press (found-defects 952): its scrub timecode
            # is drawn only then, and a press before it carries the moment 0 — the ghost started at the
            # mouse under load
            app.expect(app.until(lambda: app.has_probe("bin.scrub.tc", tries=1)), f"{name}: the card knows its length")
            app.drag(press[0], press[1], drop[0], drop[1], steps=8, mods_mid=mods, before_release=look)
            ghost = seen.get("ghost")
            app.expect(ghost is not None, f"{name}: the drop ghost is drawn before the release")
            if ghost:
                gx = win["x"] + ghost["x"]
                app.expect(abs(gx - app.x_at(want_start)) <= 4,
                           f"{name}: the ghost starts where the event will: x {gx} vs {app.x_at(want_start)}")
            new = []
            app.until(lambda: new.extend(c for c in app.geom().get("clips", []) if c["id"] not in known) or new)
            app.expect(bool(new), f"{name}: the card was placed")
            if new:
                start = min(c["start"] for c in new)
                app.expect(abs(start - want_start) <= SEC // 3,
                           f"{name}: the event starts at {start / SEC:.2f} s, want {want_start / SEC:.2f} s")
                return new
            return []

        # --- 1. the scrubbed moment under the mouse -----------------------------------------------
        carry((), "moment", "the ghost: the whole 12 s, its middle at the mouse", DROP - 6 * SEC)
        app.key("ctrl+z")
        # --- 2. Alt: the start under the mouse ------------------------------------------------------
        carry(("alt",), "alt", "Alt pressed mid-drag: the ghost starts at the mouse", DROP)
        app.key("ctrl+z")
        # --- 3. Alt+Shift: the end under the mouse -------------------------------------------------
        carry(("alt", "shift"), "alt-shift", "Alt+Shift: the ghost ends at the mouse", DROP - 12 * SEC)
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/bin_drag_moment"))

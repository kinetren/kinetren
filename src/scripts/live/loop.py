#!/usr/bin/env python3
"""Live scenario R.23: loop region and a click beside it.

A defect fixed in R.23: when playing through a loop region, a click to the RIGHT of its
end left the sound in the loop, although the cursor moved on linearly — “left is fine, but
right is not”. Here this is checked for real: the cursor is read from the
program, and the picture from the screen (clips of different colors, so it is visible
exactly what is playing).
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    red = color_media(out, "red", "C81E1E", seconds=10)
    blue = color_media(out, "blue", "1E3CC8", seconds=10)
    path = project_with_clips(
        os.path.join(out, "loop.otio"), [(0.0, 8.0, red), (8.0, 8.0, blue)]
    )
    set_region(path, start=2.0, end=6.0)

    with Live("loop", out, project=path) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 2, f"montage of two clips: {p.clips}")

        # 1. we play the region: the cursor must stay inside and wrap around
        app.key("space")
        seen = app.wait_playhead(lambda t: t > 4 * SEC, "cursor moved", timeout=10)
        if not app.playing():
            app.expect(False, "playback did not start (no audio device?)")
            app.shot("no-playback")
            return app.done()
        inside = app.wait_playhead(lambda t: t < 3 * SEC, "cursor wrapped to the start of the region")
        app.expect(
            all(t <= 6 * SEC + SEC // 2 for t in inside),
            f"cursor did not escape past the end of the region: max {max(inside)}",
        )
        app.expect(app.brightness_red(app.preview_center()), "a red clip plays in the region")
        app.shot("looping")

        # 2. a click to the RIGHT of the region during play — we play what is under the cursor
        before_right = app.pixel(*app.preview_center())
        app.click(app.x_at(11 * SEC), app.ruler_y())
        moved = app.wait_playhead(lambda t: t > 11 * SEC, "cursor moved from the click point")
        app.expect(
            all(t > 6 * SEC for t in moved[1:]),
            f"cursor did NOT return into the region: {moved[:6]}",
        )
        r, g, b = app.pixel_settled(app.preview_center(), differs_from=before_right)
        app.expect(b > r + 40, f"a blue clip plays to the right of the region: RGB {r},{g},{b}")
        app.shot("after-click-right")

        # 3. a click LEFT of the region — the same, mirrored.
        #
        # The view FOLLOWS the cursor, so by this moment the start of the project
        # has slid past the left edge: `x_at(1 s)` gave x = −613, the click went
        # PAST THE WINDOW, and the check turned green on a stale pixel. We return the
        # start into the view (Home = nav_prev) and make sure the moment is really
        # visible — otherwise the check will again measure the wrong thing.
        app.key("Home")
        geom = app.geom()
        target = 1 * SEC
        app.expect(
            geom["view_start"] <= target,
            f"project start returned into view: view_start={geom['view_start']}",
        )
        app.click(app.x_at(target), app.ruler_y())
        left = app.wait_playhead(lambda t: t < 3 * SEC, "cursor went LEFT of the region")
        app.expect(
            all(t < 3 * SEC for t in left[1:]),
            f"cursor did not stay to the left of the region: {left[:6]}",
        )
        r, g, b = app.pixel_settled(app.preview_center(), differs_from=(r, g, b))
        app.expect(r > b + 40, f"a red clip plays to the left of the region: RGB {r},{g},{b}")
        app.key("space")
        app.shot("after-click-left")
        return app.done()


def set_region(path, start, end):
    """Region + loop in the kt.transport envelope."""
    with open(path) as f:
        doc = json.load(f)
    doc["metadata"]["kt"]["transport"] = {
        "region": [round(start * SEC), round(end * SEC)],
        "loop_enabled": True,
    }
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

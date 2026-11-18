#!/usr/bin/env python3
"""Live scenario region-crops-copy (kkt-26.11.01b B10): a region that crops the one selected event
copies that event's part under the region, nothing of the other event it crosses.

The harness (t3105) drives the copy in egui; here the real window:

1. two events, 0–2 s (red) and 3–6 s (teal); the teal one is clicked;
2. a region 1–4 s (I, O): it crosses the red event and crops the teal one;
3. Ctrl+C: the clipboard holds one event;
4. the playhead at 8 s, Edit › Paste: one new event of 1 s (the teal part 3–4 s) lands at 8 s.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    red = color_media(out, "red", "C81E1E", seconds=6)
    teal = color_media(out, "teal", "1E8C8C", seconds=6)
    path = project_with_clips(os.path.join(out, "region_crops_copy.otio"), [(0.0, 2.0, red), (3.0, 3.0, teal)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("region_crops_copy", out, project=path) as app:
        before = app.live_clips()
        app.expect(before == [(0, 2 * SEC), (3 * SEC, 3 * SEC)], f"two events: {before}")

        # --- 1–2. select the teal event, then the region 1–4 s ------------------------------
        app.click(app.x_at(5 * SEC), app.lane_y("video", 0))
        app.click(app.x_at(1 * SEC), app.ruler_y())
        app.key("i")
        app.click(app.x_at(4 * SEC), app.ruler_y())
        app.key("o")
        app.step("selected", "the teal event selected, a region 1–4 s over both events")

        # --- 3. copy ---------------------------------------------------------------------------
        app.key("ctrl+c")
        copied = app.until(lambda: app.geom().get("clipboard") == 1)
        app.step("copied", "Ctrl+C: the status line names the selected event's part")
        app.expect(copied, f"one event in the clipboard: {app.geom().get('clipboard')}")
        notes = app.geom().get("notices", [])
        app.expect(any("part under the region" in n for n in notes), f"the status names the part: {notes}")

        # --- 4. paste at 8 s ---------------------------------------------------------------------
        app.click(app.x_at(8 * SEC), app.ruler_y())
        app.click_probe("menu.edit", "the Edit menu")
        app.click_probe("edit.paste", "Edit › Paste")
        pasted = app.until(lambda: len(app.live_clips()) == 3)
        app.step("pasted", "Edit › Paste: only the teal part 3–4 s lands at 8 s")
        new = [c for c in app.live_clips() if c not in before]
        app.expect(pasted and new == [(8 * SEC, 1 * SEC)], f"the pasted piece is the crop: {new}")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/region_crops_copy"))

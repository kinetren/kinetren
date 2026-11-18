#!/usr/bin/env python3
"""Live scenario region-cut-key (F-TL-015, `v26.10` U9): the region's interior is cut out by `C` and
by Edit › Cut out the region interior.

The harness (t3102) drives the key and the menu item in egui; here the real window:

1. the playhead at 2 s → I, at 4 s → O: a region 2–4 s over one 6 s event;
2. C: the live model has two events, 0–2 s and 4–6 s;
3. Ctrl+Z: one event again;
4. Edit › Cut out the region interior: the same two events.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000
CUT = [(0, 2 * SEC), (4 * SEC, 2 * SEC)]


def main(out):
    require_tools()
    teal = color_media(out, "teal", "1E8C8C", seconds=6)
    path = project_with_clips(os.path.join(out, "region_cut_key.otio"), [(0.0, 6.0, teal)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("region_cut_key", out, project=path) as app:
        whole = app.live_clips()
        app.expect(whole == [(0, 6 * SEC)], f"one 6 s event: {whole}")

        # --- 1. the region with the keyboard -------------------------------------------
        app.click(app.x_at(2 * SEC), app.ruler_y())
        app.key("i")
        app.click(app.x_at(4 * SEC), app.ruler_y())
        app.key("o")
        app.step("region", "a region 2–4 s over the event")

        # --- 2. C -------------------------------------------------------------------
        app.key("c")
        cut = app.until(lambda: app.live_clips() == CUT)
        app.step("cut", "C: the region's interior is gone, the remainders stay in place")
        app.expect(cut, f"C cut out 2–4 s: {app.live_clips()}")

        # --- 3. undo -------------------------------------------------------------------
        app.key("ctrl+z")
        back = app.until(lambda: app.live_clips() == whole)
        app.step("undo", "Ctrl+Z: the event is whole again")
        app.expect(back, f"one undo restores the event: {app.live_clips()}")

        # --- 4. the Edit menu -----------------------------------------------------------
        app.click_probe("menu.edit", "the Edit menu")
        app.step("menu", "Edit menu: “Cut out the region interior” with C on the right")
        app.click_probe("menu.edit.punch_out", "the Edit item “Cut out the region interior”")
        by_menu = app.until(lambda: app.live_clips() == CUT)
        app.step("menu-cut", "the Edit item gave the same cut")
        app.expect(by_menu, f"the Edit item cut out 2–4 s: {app.live_clips()}")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/region_cut_key"))

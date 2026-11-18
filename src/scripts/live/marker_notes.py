#!/usr/bin/env python3
"""Live scenario marker-notes (`v26.10` U8): a marker carries a multi-line note.

The harness (t3113) drives the field in egui; here the real window:

1. a marker at 2 s (M), named "chorus";
2. right click on its flag → Note…: a multi-line field opens under the flag; two lines typed with
   Enter between them, a click elsewhere keeps them — the note glyph stands after the name;
3. hovering the flag, the status line reads both lines;
4. the saved project holds the two-line note.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    teal = color_media(out, "teal", "1E8C8C", seconds=6)
    path = project_with_clips(os.path.join(out, "marker_notes.otio"), [(0.0, 6.0, teal)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("marker_notes", out, project=path) as app:
        # --- 1. a named marker ---------------------------------------------------------------
        app.click(app.x_at(2 * SEC), app.ruler_y())
        app.key("m")
        app.type_text("chorus")
        app.key("Return")
        made = app.until(lambda: [m[0] for m in app.geom().get("markers", [])] == [2 * SEC])
        app.expect(made, f"M put a marker at 2 s: {app.geom().get('markers')}")
        app.step("marker", "a marker “chorus” at 2 s")

        # --- 2. Note… at the flag -----------------------------------------------------------
        flag = (app.x_at(2 * SEC), app.ruler_y())
        app.right_click(*flag)
        app.expect(app.has_probe("tl.marker_note"), "the marker's menu offers Note…")
        app.step("menu", "right click on the flag: Note… in the marker's menu")
        app.click_probe("tl.marker_note", "the item Note…")
        app.expect(app.has_probe("tl.marker_note.field"), "a note field opened at the flag")
        f = next(f for f in app.geom()["fields"] if f["param"] == "tl.marker_note.field")
        app.expect(f["h"] > 40, f"the note field is multi-line: {f}")
        app.type_text("take 3")
        app.key("Return")
        app.type_text("warmer")
        app.step("typing", "two lines typed into the note field")
        # an empty spot well clear of the field (it covers the ruler and the track under the flag)
        app.click(app.x_at(8 * SEC), app.lane_y("audio", 0))
        app.expect(app.wait_gone("tl.marker_note.field"), "a click elsewhere closed the field")
        app.expect(app.has_probe(f"tl.marker_note_mark.{2 * SEC}"), "the note glyph stands after the name")
        app.step("glyph", "the note glyph after the marker's name")

        # --- 3. hover reads the note ----------------------------------------------------------
        app.hover(*flag)
        read = app.until(lambda: app.geom().get("status", "") == "take 3 · warmer")
        app.step("hover", "the status line reads the note")
        app.expect(read, f"hovering the flag reads the note: “{app.geom().get('status', '')}”")

        # --- 4. on disk -------------------------------------------------------------------------
        text = app.saved_text()
        app.expect('"take 3\\nwarmer"' in text, "the saved project holds the two-line note")


if __name__ == "__main__":
    main(sys.argv[1])

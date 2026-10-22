#!/usr/bin/env python3
"""Live scenario R.52 C3: a title is typed in a real window.

Bin menu → “Create a generator” → “Text” opens the TITLE WINDOW (not the common
generator window — routing); a click into the field, keyboard input; a clip onto the
cursor — DRAWN LETTERS appear in the preview (brightness falls from the colored ramp frame
to “white letters on emptiness”, but not to zero); Ctrl+S —
the typed text lies in the gen:text path, escaped, and in
GeneratorReference.1.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "title.otio"),
        [(0.0, 3.0, "synth:Ramp")],
    )

    with Live("title", out, project=proj) as app:
        win = geometry()
        b = app.geom()["bin"]
        app.shot("open")

        # 1. create a TEXT from the generators submenu. The cursor is AFTER the end of the
        # bottom clip: creation puts the clip at once (item 416), and an overlap
        # on one track is a crossfade, and the title at its start is invisible.
        app.click(app.x_at(5 * SEC), app.ruler_y())
        app.right_click(
            win["x"] + int((b["left"] + b["right"]) / 2),
            win["y"] + int(b["bottom"]) - 10,
        )
        app.click_probe("gen.new", "item “Create a generator”")
        # R.53: the title lies in the group “Titles and counter”
        app.hover_probe("gen.new.group.titles", "group “Titles and countdown”")
        app.click_probe("gen.new.text", "kind “Text” in the submenu")
        app.expect(
            app.has_probe("titlew.text"),
            "the TITLE WINDOW opened (routing past the common window)",
        )
        app.shot("title-window")

        # 2. type the text into the window’s field
        app.click_probe("titlew.text", "title text field")
        for ch in ["k", "t"]:
            app.key(ch)
        # Esc closes the title window (cascade of item 11) — otherwise it OVERLAPS
        # the bin and the preview, and clicks go into it; at the same time the focus is handed back
        # (rule 14): the next hotkey works
        app.key("Escape")
        app.expect(not app.has_probe("titlew.text", tries=2), "Esc closed the title window")

        # 3. preview: on the synth clip the frame is coloured; we put the title on an empty
        # place — white letters arrive over the emptiness
        preview = app.preview_center()
        app.click(app.x_at(int(1.5 * SEC)), app.ruler_y())
        ramp_px = app.pixel_settled(preview)
        ramp_b = app.brightness(preview)
        app.expect(ramp_b > 20, f"frame of the synth clip is light: {ramp_b:.1f}")

        # R.53 (item 416): the title clip was placed by creation itself — there is no need to walk
        # to the bin behind the card. We will not move it by hand to an empty place:
        # it is enough to stand the cursor ON it.
        placed = len(app.live_clips()) == 2
        app.expect(placed, f"creating a title must place a clip: {app.live_clips()}")
        gen_start = app.live_clips()[-1][0] if placed else 0
        app.click(app.x_at(int(gen_start + 2 * SEC)), app.ruler_y())

        # the letters are there, but there is no background: the brightness of the AREA is high (white stroke), while
        # the frame’s central point is dark — the title is letters on emptiness,
        # not a filled rectangle
        drawn = None
        for _ in range(30):
            drawn = app.ink_box(preview, half=150)
            if drawn > 120:
                break
            poll_step(0.4)
        app.expect(
            drawn is not None and drawn > 120,
            f"in the preview the title letters over emptiness: {drawn} (ramp {ramp_b:.1f})",
        )
        app.shot("placed")

        # 4. TYPING TEXT WITH TRANSPORT PLAYING (checklist rule 2:
        # the gate “only on pause” is set only if, without it, an engine invariant breaks).
        # The edit goes through reconfiguring the source in
        # place (R.53, item 411), so the sound must not stutter, and the decoder
        # stream must not be recreated.
        # the window is opened FROM THE EVENT MENU while we stand: during play the view moves
        # with the cursor, and the clip’s coordinate goes stale before the click
        app.right_click(
            app.x_at(int(gen_start + 1 * SEC)),
            app.lane_y("video", 0, 0.6),
        )
        app.click_probe("gen.params", "“Generator parameters…” item")
        app.expect(app.has_probe("titlew.text"), "title window opened from the event menu")
        app.key("space")
        app.expect(app.geom().get("playing") is True, "transport plays")
        app.click_probe("titlew.text", "text field during playback")
        app.key("9")
        app.expect(
            app.geom().get("playing") is True,
            "a title edit during playback must not stop the transport",
        )
        # Esc FIRST: while the focus is in the field, the space would be typed into the text
        app.key("Escape")
        app.key("space")
        app.shot("typed-while-playing")

        # 5. the typed text in the file: the scheme — in the EXPORT (R.70), the text itself
        # — in the model of the project file
        app.expect('"kt9"' in app.saved_text(), "the typed text lies in the project file")
        with open(app.project_now().otio_path) as f:
            text = f.read()
        app.expect('"kt.text"' in text, "the title clip moved to GeneratorReference.1/kt.text")
        app.expect('"text": "kt9"' in text, "the text typed during playback lies in parameters")
        app.shot("saved")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

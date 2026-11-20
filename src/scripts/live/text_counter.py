#!/usr/bin/env python3
"""Live scenario text-counter-source (`v26.10` G5's timer/counter, G3): a title runs a stopwatch.

His words (2026-10-30, question 5): the timecode is text and is controlled as text — a mode of the Text
generator. The technical group of G5 names a timer/counter.

PATH (generators "A title runs a timecode, a stopwatch or a countdown"):

1. Edit → Create a generator… → Titles and countdown → Text: its window opens;
2. Text: Stopwatch → the Format row appears, the preview shows 0:00;
3. the playhead at 3 s → the preview's digits changed (0:03), the rest of the text's look the same.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def field(app, name):
    return next((f for f in app.geom().get("fields", []) if f["param"] == name), None)


def band(app, frame):
    """The text's band in the middle of the picture, sampled on a grid."""
    win = geometry()
    pts = []
    for j in range(5):
        for i in range(48):
            x = win["x"] + int(frame["x"] + frame["w"] * (0.3 + 0.4 * i / 47))
            y = win["y"] + int(frame["y"] + frame["h"] * (0.46 + 0.08 * j / 4))
            pts.append(app.pixel(x, y))
    return pts


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "text_counter.otio"), [(6.0, 2.0, "synth:Ramp")])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("text_counter", out, project=proj) as app:
        app.key("Home")
        app.click_probe("menu.edit", "the Edit menu")
        app.hover_probe("gen.new", "Create a generator…")
        app.hover_probe("gen.new.group.titles", "Titles and countdown")
        app.click_probe("gen.new.text", "Text")
        app.expect(app.has_probe("titlew.source.stopwatch"), "the title's window shows its source row")
        app.step("window", "a Text generator: its window, the Text row with Typed, Timecode, Stopwatch, Countdown")
        app.click_probe("titlew.source.stopwatch", "Stopwatch")
        frame = field(app, "preview.frame")
        app.expect(frame is not None, "the preview's picture is published")
        if not frame:
            return app.done()
        app.until(lambda: sum(max(p) > 150 for p in band(app, frame)) > 3)
        zero = band(app, frame)
        app.step("zero", "Stopwatch: the preview shows 0:00, the Format row under the source")
        app.expect(sum(max(p) > 150 for p in zero) > 3, "the stopwatch's digits are drawn")
        app.click(app.x_at(3 * SEC), app.ruler_y())
        moved = app.until(lambda: band(app, frame) != zero)
        three = band(app, frame)
        app.step("three", "at 3 s: the preview shows 0:03")
        app.expect(moved, "the digits changed with the time")
        app.expect(sum(max(p) > 150 for p in three) > 3, "the digits are still drawn at 3 s")
        text = app.saved_text()
        app.expect('"source": "stopwatch"' in text, "the saved title runs a stopwatch")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/text_counter"))

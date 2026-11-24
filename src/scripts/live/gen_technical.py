#!/usr/bin/env python3
"""Live scenario gen-technical (`v26.10` G5, the technical group): the sync flash and the safe areas.

His words (2026-10-30, question 6, in the G5 row): all four groups of generators, the technical one
among them — sync beep/flash, safe-area grids, timer/counter, audio tone/noise.

PATH (generators "The technical generators: a sync flash and the safe areas"):

1. Edit → Create a generator… → Signal check → Sync flash: an event at the cursor (0 s), its beep on A1;
   the preview at 0 s is a white field;
2. the playhead at 0.5 s: the preview is dark, the bar along the bottom half full;
3. Edit → Create a generator… → Signal check → Safe areas at 0.5 s on the next track: the guides stand
   over the dark field (a light line where the action-safe frame is, dark inside it);
4. the cursor at 7 s, SMPTE bars there: one silent picture; Tone ticked in its window → a linked sound
   event of the same span under it (found-defects 1000), off → gone, on → back; Space in a loop inside
   it → the meters move.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def field(app, name):
    return next((f for f in app.geom().get("fields", []) if f["param"] == name), None)


def create(app, tag, what):
    app.new_generator(tag, what=what)
    app.key("Escape")


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "gen_technical.otio"), [(4.0, 2.0, "synth:Ramp")])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("gen_technical", out, project=proj) as app:
        app.key("shift+Home")  # the project's start (Home is the previous edit point in the owner's keys, found-defects 951)
        create(app, "syncflash", "Sync flash")
        clips = app.until(lambda: len(app.live_clips()) >= 3)
        app.expect(clips, f"the flash and its beep were added: {app.live_clips()}")
        # the first frames after opening may not have drawn the picture yet: wait for its probe
        app.until(lambda: field(app, "preview.frame") is not None)
        frame = field(app, "preview.frame")
        app.expect(frame is not None, "the preview's picture is published")
        if not frame:
            return app.done()
        win = geometry()
        mid = (win["x"] + int(frame["x"] + frame["w"] / 2), win["y"] + int(frame["y"] + frame["h"] / 2))
        lit = app.pixel_settled(mid)
        app.step("flash", "Sync flash at 0 s: the preview is a white field, the beep on A1")
        app.expect(lit is not None and min(lit) > 200, f"the flash frame is white: {lit}")

        app.click(app.x_at(SEC // 2), app.ruler_y())
        dark = app.pixel_settled(mid, differs_from=lit)
        app.step("between", "at 0.5 s: a dark field, the bar along the bottom half full")
        app.expect(dark is not None and max(dark) < 60, f"between flashes the frame is dark: {dark}")

        create(app, "safearea", "Safe areas")
        app.until(lambda: len(app.live_clips()) >= 4)
        # the action-safe frame's left edge: 3.5 % in from the picture's left, at a third of its height
        edge = (win["x"] + int(frame["x"] + frame["w"] * 0.035) + 1, win["y"] + int(frame["y"] + frame["h"] / 3))
        line = app.pixel_settled(edge, differs_from=dark)
        app.step("guides", "Safe areas over the flash: the action- and title-safe frames and the centre cross")
        app.expect(line is not None and max(line) > max(dark) + 40, f"a guide line stands at the action-safe edge: {line} over {dark}")

        # 4) the tone of the bars brings its sound event (gen-sound-pair, ui-reach t3720)
        app.click(app.x_at(7 * SEC), app.ruler_y())
        before = len(app.live_clips())
        app.new_generator("smpte", what="SMPTE bars")
        app.expect(app.until(lambda: len(app.live_clips()) == before + 1), f"the bars are one silent event: {app.live_clips()}")
        app.step("bars", "SMPTE bars at 7 s, its window open: a picture with no sound event")
        app.click_probe("tone", "the Tone tick")
        app.expect(app.until(lambda: len(app.live_clips()) == before + 2), f"the tick brought the sound event: {app.live_clips()}")
        tpp = app.geom()["ticks_per_px"]
        at = [c for c in app.live_clips() if abs(c[0] - 7 * SEC) <= 2 * tpp]
        app.expect(len(at) == 2 and at[0][:2] == at[1][:2], f"the picture and its sound span the same time: {at}")
        app.step("tone-on", "Tone ticked: a linked sound event under the bars, of the same span")
        app.click_probe("tone", "the Tone tick")
        app.expect(app.until(lambda: len(app.live_clips()) == before + 1), f"the tick off took the sound event: {app.live_clips()}")
        app.click_probe("tone", "the Tone tick")
        app.expect(app.until(lambda: len(app.live_clips()) == before + 2), f"the tick brought it again: {app.live_clips()}")
        app.key("Escape")
        ry = app.ruler_y()
        app.drag(app.x_at(int(7.5 * SEC)), ry, app.x_at(int(9 * SEC)), ry)
        app.key("l")
        app.key("space")
        app.expect(app.until(lambda: app.playhead() > 7.6 * SEC, timeout=60), "the sound started after Space")
        loud = []

        def heard():
            loud.append(max(app.geom().get("meter_db", [-120, -120])))
            return len([v for v in loud if v > -40]) >= 3

        app.expect(app.until(heard, timeout=60), f"the meters move on the bars' tone: {loud[-12:]}")
        app.step("tone-plays", "the loop plays over the bars: the meters are lit")
        app.key("space")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/gen_technical"))

#!/usr/bin/env python3
"""Live scenario generators-placement-and-audio (kkt-26.11.01 item 4, registry K4).

The owner: "WHY THE HELL LEAVE IT IN FILE?! … of course the audio ones are needed — you hide
some generators in the tests — pull them out". A real window, real clicks, OUR fixture (a 3 s
synthetic clip):

1. File → no "Create a generator" row;
2. Edit → Create a generator → Audio → Tone → an audio event at the cursor, no video event;
3. a right click on the audio track at 6 s → Create a generator → Audio → Beep train → the event
   starts where the click was;
4. Home, Space → the tone plays and the meters move (their level is read from the program).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "genaudio.otio"), [(0.0, 3.0, "synth:Ramp")])

    with Live("genaudio", out, project=proj) as app:
        app.step("open", "a 3 s clip on the video track, the cursor at 0")

        # 1) File no longer offers it
        app.click_probe("menu.file", "the File menu")
        app.expect(app.has_probe("file.render"), "the File menu is open")
        app.expect(not app.has_probe("gen.new", tries=4), "File offers no “Create a generator”")
        app.step("file-menu", "File: New, Open, Import…, Save… — no “Create a generator”")
        app.key("Escape")

        # 2) Edit → Create a generator → Audio → Tone
        app.click_probe("menu.edit", "the Edit menu")
        app.hover_probe("gen.new", "“Create a generator” in Edit")
        app.hover_probe("gen.new.group.audio", "the Audio group")
        app.step("edit-menu", "Edit › Create a generator › Audio: Tone, Audio noise, Sine sweep, Beep train, Silence")
        app.click_probe("gen.new.tone", "Tone")
        clips = []
        for _ in range(20):
            clips = app.live_clips()
            if len(clips) == 2:
                break
            poll_step(0.25)
        app.expect(len(clips) == 2, f"Tone adds ONE event (no picture): {clips}")
        app.expect(sum(1 for c in clips if c[0] == 0) == 2, f"the tone starts at the cursor: {clips}")
        app.step("edit-tone", "a “Tone” event on the first audio track under the cursor, its form open")
        app.key("Escape")

        # 3) a right click on the audio track at 6 s → Beep train lands there
        x6 = app.x_at(6 * SEC)
        app.right_click(x6, app.lane_y("audio", 0, 0.6))
        app.hover_probe("gen.new", "“Create a generator” in the track menu")
        app.expect(app.has_probe("gen.new.group.audio"), "the audio track lists the Audio group")
        app.expect(not app.has_probe("gen.new.group.bg", tries=3), "no picture groups on an audio track")
        app.hover_probe("gen.new.group.audio", "the Audio group")
        app.step("track-menu", "right click on the audio track at 6 s: only sound kinds")
        app.click_probe("gen.new.beep", "Beep train")
        for _ in range(20):
            clips = app.live_clips()
            if len(clips) == 3:
                break
            poll_step(0.25)
        app.expect(len(clips) == 3, f"the beep was added: {clips}")
        tpp = app.geom()["ticks_per_px"]
        starts = sorted(c[0] for c in clips)
        app.expect(
            any(abs(s - 6 * SEC) <= 2 * tpp for s in starts),
            f"the beep starts where the click was (6 s ± 2 px): {starts}",
        )
        app.step("track-beep", "a “Beep train” event exactly at the click on the audio track")
        app.key("Escape")

        # 4) play the tone: the meters move
        app.key("Home")
        app.key("space")
        lit = []
        for _ in range(30):
            g = app.geom()
            if g.get("playing"):
                lit.append(max(g.get("meter_db", [-120, -120])))
            if len([v for v in lit if v > -40]) >= 3:
                break
            poll_step(0.2)
        app.step("playing", "the tone plays: the meters on the right are lit")
        app.key("space")
        app.expect(len([v for v in lit if v > -40]) >= 3, f"the meters moved while the tone played: {lit}")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

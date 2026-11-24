#!/usr/bin/env python3
"""Live scenario loudness-norms (kkt-26.11.20b B6, `v26.10` T14): the loudness target is the project's, the
dialogue and the program are matched to it, and the render is judged by it.

The project is OUR fixture: a picture and two dialogue lines of one tone 8 dB apart.

PATH (ui-reach spec):
1. File → Project settings → Audio → Loudness: "Streaming (−14 LUFS)" → Apply;
2. right click a dialogue event → "Match loudness…" → Dialogue → Match: both lines' levels move, one Undo
   step;
3. the row under the meters → "Match the program" → the row shows the gain;
4. the matched event gets its old level back → the row says "changed since measured" → "Match the program" → the mark goes;
5. File → Render… → Render… → the result reads "Sound — 0 outside the norm".
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, _clip, project_with_clips, require_tools  # noqa: E402

# the save seam is read at start-up
OWN_PROCESS = True


def lines(out_dir):
    """Two dialogue-like files: a tone broken by short pauses, 8 dB apart."""
    paths = []
    for name, amp in (("quiet", 0.15), ("loud", 0.40)):
        path = os.path.join(out_dir, f"{name}.wav")
        if not os.path.exists(path):
            expr = f"{amp}*sin(2*PI*700*t)*gt(sin(2*PI*1.5*t)\\,-0.6)"
            subprocess.run(
                ["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", f"aevalsrc={expr}|{expr}:s=48000:d=4",
                 "-c:a", "pcm_s16le", path],
                check=True,
            )
        paths.append(path)
    return paths


def type_number(app, text):
    """The number of the field that was clicked."""
    app.key("ctrl+a")
    app.type_text(text)
    app.key("Return")


def main(out):
    require_tools()
    os.makedirs(out, exist_ok=True)
    target = os.path.join(out, "norms.mp4")
    if os.path.exists(target):
        os.remove(target)
    quiet, loud = lines(out)
    proj = project_with_clips(os.path.join(out, "render_norms.otio"), [(0.0, 8.0, "synth:Ramp")])
    with open(proj) as f:
        doc = json.load(f)
    doc["tracks"]["children"].append({
        "OTIO_SCHEMA": "Track.1", "name": "", "kind": "Audio",
        "children": [_clip(2, 4.0, quiet, 2), _clip(3, 4.0, loud, 3)],
        "source_range": None, "effects": [], "markers": [], "enabled": True, "metadata": {},
    })
    with open(proj, "w") as f:
        json.dump(doc, f, indent=4)
    was = os.environ.get("KT_FILE_PICK")
    os.environ["KT_FILE_PICK"] = target
    try:
        with Live("render_norms", out, project=proj, own_process=True) as app:
            app.expect(len(app.live_clips()) == 3, f"a picture and two lines: {app.live_clips()}")

            # 1) the target
            app.click_probe("menu.file", "the File menu")
            app.click_probe("file.project_settings", "File → Project settings")
            app.click_probe("settings.loud", "the Loudness list")
            app.click_probe("settings.loud.ebu", "EBU R128")
            app.click_probe("settings.loud", "the Loudness list")
            app.click_probe("settings.loud.atsc", "ATSC A/85")
            app.click_probe("settings.loud", "the Loudness list")
            app.click_probe("settings.loud.podcast", "Podcast")
            app.click_probe("settings.loud", "the Loudness list")
            app.click_probe("settings.loud.custom", "Custom")
            app.click_probe("settings.loud.target", "the target")
            type_number(app, "-18")
            app.click_probe("settings.loud.ceiling", "the ceiling")
            type_number(app, "-2")
            app.step("custom", "Custom: numbers of one's own")
            app.click_probe("settings.loud", "the Loudness list")
            app.click_probe("settings.loud.streaming", "Streaming (−14 LUFS)")
            app.step("settings", "Project settings: Streaming, −14 LUFS and −1 dBTP")
            app.click_probe("settings.apply", "Apply")
            app.expect(app.wait_gone("settings.apply"), "the settings window closed")

            # 2) the dialogue
            app.right_click(app.x_at(90000), app.lane_y("audio", 0, frac=0.6))
            app.click_probe("clip.menu.match", "Match loudness… in the event's menu")
            app.expect(app.has_probe("match.go"), "the window asks what the events are")
            app.click_probe("match.music", "Music")
            app.click_probe("match.effects", "Effects")
            app.click_probe("match.number", "a number")
            app.click_probe("match.number_value", "its value")
            type_number(app, "-18")
            app.step("match", "Match loudness: what the events are")
            app.click_probe("match.dialogue", "Dialogue")
            app.click_probe("match.go", "Match")
            app.expect(app.until(lambda: not app.has_probe("match.go", tries=1), timeout=60), "the window closed")
            app.step("matched", "the event's level moved")
            # the same from the track's own menu; Esc leaves the levels
            app.right_click(app.x_at(10 * 90000), app.lane_y("audio", 0, frac=0.6))
            app.click_probe("track.menu.match", "Match loudness… in the track's menu")
            app.expect(app.has_probe("match.go"), "the window asks about the track's events")
            app.key("Escape")
            app.expect(app.wait_gone("match.go"), "Esc closes it")

            # 3) the program
            app.click_probe("program.row", "the program row under the meters")
            app.click_probe("program.on", "the stage on")
            app.click_probe("program.on", "and off")
            app.click_probe("program.settings", "Project settings… from the stage's window")
            app.click_probe("settings.apply", "Apply")
            app.expect(app.wait_gone("settings.apply"), "the settings window closed")
            app.click_probe("program.match", "Match the program")
            app.expect(app.until(lambda: app.has_probe("program.gain", tries=1), timeout=120), "the stage has a gain")
            app.step("program", "the stage: its gain and what was measured")

            # 4) an edit
            app.expect(not app.has_probe("program.changed", tries=1), "nothing changed yet")
            app.right_click(app.x_at(2 * 90000), app.lane_y("audio", 0, frac=0.6))
            app.click_probe("clip.menu.reset_gain", "Reset gain in the matched event's menu")
            app.expect(app.until(lambda: app.has_probe("program.changed", tries=1), timeout=30), "the row says it changed")
            app.expect(app.has_probe("program.win_changed"), "the window says it under the gain, with no scroll")
            app.step("changed", "changed since measured")
            app.click_probe("program.match", "Match the program again")
            app.expect(app.wait_gone("program.changed", timeout=120), "the mark went")
            app.step("again", "the stage is measured again")

            # 5) the render
            app.click_probe("program.row", "the stage's window closes")
            app.expect(app.wait_gone("program.match"), "it closed")
            app.click_probe("menu.file", "the File menu")
            app.click_probe("file.render", "File → Render…")
            app.click_probe("render.start", "Render…")
            app.expect(app.until(lambda: app.has_probe("render.close", tries=1), timeout=900), "the render finished")
            good = app.has_probe("render.loud.good")
            app.step("done", "Sound — 0 outside the norm")
            app.expect(good, "nothing of the sound is outside the project's norm")
            app.click_probe("render.close", "Close")
            app.expect(app.wait_gone("render.close"), "the window closed")
            return app.done()
    finally:
        if was is None:
            os.environ.pop("KT_FILE_PICK", None)
        else:
            os.environ["KT_FILE_PICK"] = was


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/render_norms"))

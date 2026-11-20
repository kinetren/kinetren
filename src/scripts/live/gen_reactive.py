#!/usr/bin/env python3
"""Live scenario gen-audio-reactive (`v26.10` G5, the audio-reactive group).

His words (2026-10-30, question 6, in the G5 row): audio-reactive — a waveform, a spectrum, level rings.

PATH (generators "An audio-reactive generator listens to a track's files"):

1. a project with a music file on audio track 1: a 440 Hz tone that sounds 1.5 s and pauses 1.5 s;
2. Edit → Create a generator… → its window → Kind → Reacts to sound → Spectrum: a spectrum over the
   black frame, its window shows "Hears track 1";
3. the playhead at 0.7 s (the tone sounds): the bar of the tone's band stands tall;
4. the playhead at 2.6 s (a pause): the bars are down.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, _clip, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def field(app, name):
    return next((f for f in app.geom().get("fields", []) if f["param"] == name), None)


def tone_media(out_dir):
    """A 6 s 440 Hz tone, on for 1.5 s and off for 1.5 s, stereo 48 kHz. Idempotent."""
    path = os.path.join(out_dir, "tone-gated.wav")
    if not os.path.exists(path):
        subprocess.run(
            ["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=6:sample_rate=48000",
             "-af", "volume='if(lt(mod(t,3),1.5),0.9,0)':eval=frame", "-ac", "2", path],
            check=True,
        )
    return path


def with_music(dest, music):
    """An empty project with `music` on audio track 1 for its 6 s."""
    project_with_clips(dest, [])
    with open(dest) as f:
        doc = json.load(f)
    clip = _clip(1, 6.0, music, 1)
    doc["tracks"]["children"].append({
        "OTIO_SCHEMA": "Track.1", "name": "", "kind": "Audio", "children": [clip], "source_range": None,
        "effects": [], "markers": [], "enabled": True, "metadata": {},
    })
    with open(dest, "w") as f:
        json.dump(doc, f, indent=4)
    return dest


def main(out):
    require_tools()
    music = tone_media(out)
    proj = with_music(os.path.join(out, "gen_reactive.otio"), music)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("gen_reactive", out, project=proj) as app:
        app.key("shift+Home")  # the project's start (Home is the previous edit point in the owner's keys)
        app.step("open", "a music file on audio track 1, the video track empty")
        app.new_generator("spectrum", what="the Spectrum cell")
        app.expect(app.until(lambda: field(app, "lbl.track") is not None or app.has_probe("genw.kind.pick", tries=2)),
                   "the spectrum's window is open")
        app.step("spectrum", "a Spectrum at the cursor, its window with Hears track 1")
        app.key("Escape")
        app.until(lambda: field(app, "preview.frame") is not None)
        frame = field(app, "preview.frame")
        app.expect(frame is not None, "the preview's picture is published")
        if not frame:
            return app.done()
        win = geometry()
        at = lambda fx, fy: (win["x"] + int(frame["x"] + frame["w"] * fx), win["y"] + int(frame["y"] + frame["h"] * fy))
        # 440 Hz falls in band 6 of 16 (40 Hz..16 kHz): its bar's middle, well above the base line
        bar = at(0.1 + 0.8 * 6.5 / 16, 0.72)
        app.click(app.x_at(int(0.7 * SEC)), app.ruler_y())
        tall = app.until(lambda: max(app.pixel(*bar)) > 100, timeout=60)
        app.step("sounds", "the tone sounds at 0.7 s: the bar of its band stands tall")
        app.expect(tall, f"the tone's bar is lit while it sounds: {app.pixel(*bar)}")
        app.click(app.x_at(int(2.6 * SEC)), app.ruler_y())
        low = app.until(lambda: max(app.pixel(*bar)) < 40)
        app.step("pause", "a pause at 2.6 s: the bars are down")
        app.expect(low, f"the bar is down in the pause: {app.pixel(*bar)}")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/gen_reactive"))

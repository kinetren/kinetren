#!/usr/bin/env python3
"""Live scenario render-loudness-norms (F-EXP-010, `v26.10` T14): the render's loudness stands
against its norms, and a bad value is coloured.

The owner (`v26.10` line 56): with one channel too loud or the other silent "you would not notice
such numbers at all", and "it is not clear what the norm of each value is". The project is OUR
fixture: a 2 s picture and a sound with a tone in the LEFT channel only — the right one silent.

PATH (export spec):
1. File → Render… → the settings window → Render…;
2. the render finishes: the Loudness table — integrated, true peak, range, channels L / R — each value
   with its norm beside it; the channels row reads "R silent" in the error colour.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, _clip, project_with_clips, require_tools  # noqa: E402

OWN_PROCESS = True


def left_only(out_dir):
    path = os.path.join(out_dir, "left_only.wav")
    if not os.path.exists(path):
        subprocess.run(
            ["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i",
             "aevalsrc=0.1*sin(2*PI*1000*t)|0:s=48000:d=2", "-c:a", "pcm_s16le", path],
            check=True,
        )
    return path


def main(out):
    require_tools()
    os.makedirs(out, exist_ok=True)
    target = os.path.join(out, "loud.mp4")
    if os.path.exists(target):
        os.remove(target)
    wav = left_only(out)
    proj = project_with_clips(os.path.join(out, "render_loudness.otio"), [(0.0, 2.0, "synth:Ramp")])
    with open(proj) as f:
        doc = json.load(f)
    audio = {
        "OTIO_SCHEMA": "Track.1", "name": "", "kind": "Audio", "children": [_clip(2, 2.0, wav, 2)],
        "source_range": None, "effects": [], "markers": [], "enabled": True, "metadata": {},
    }
    doc["tracks"]["children"].append(audio)
    with open(proj, "w") as f:
        json.dump(doc, f, indent=4)
    was = os.environ.get("KT_FILE_PICK")
    os.environ["KT_FILE_PICK"] = target
    try:
        with Live("render_loudness", out, project=proj, own_process=True) as app:
            app.expect(len(app.live_clips()) == 2, f"a picture and a sound: {app.live_clips()}")
            app.click_probe("menu.file", "the File menu")
            app.click_probe("file.render", "File → Render…")
            app.expect(app.has_probe("render.start"), "the settings window offers Render…")
            app.step("settings", "the render settings window")
            app.click_probe("render.start", "Render… in the settings")
            app.expect(app.has_probe("render.close", tries=240), "the render finished")
            bad = app.has_probe("render.loud.bad")
            app.step("done", "the Loudness table: each value with its norm, the silent right channel in red")
            app.expect(bad, "the silent right channel is marked as a bad value")
            app.click_probe("render.close", "Close")
            app.expect(app.wait_gone("render.close"), "the window closed")
            return app.done()
    finally:
        if was is None:
            os.environ.pop("KT_FILE_PICK", None)
        else:
            os.environ["KT_FILE_PICK"] = was


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/render_loudness"))

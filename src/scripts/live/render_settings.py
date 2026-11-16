#!/usr/bin/env python3
"""Live scenario render-settings (kkt-26.11.15 item 5: the final render's settings are very scant).

The render settings window offers the container, the codec, the rate mode, the encoder speed, the
bit depth, the colour, the frame rate, the audio codec, the range, the metadata and the presets; the
chosen settings reach the file. The save dialog is answered by the `KT_FILE_PICK` seam; the project
is OUR synthetic fixture (a generator), not one of the owner's.

PATH (export spec, t2810):
1. File → Render… → the settings window in groups: Preset, Video, Size and rate, Audio, File; the
   Preset list names the factory presets by codec, size and rate;
2. File: Container → the list (MP4, MOV, MKV, WebM) → MKV;
3. Video: Codec → the list, a codec this FFmpeg or MKV cannot take is greyed with its reason → H.265;
4. Rate → VBR (average and maximum): the average and maximum rows appear;
5. Encoder speed slow, Bit depth 10-bit, Colour BT.709 limited, Resolution Custom (Width, Height
   rows), Frame rate 25, Audio codec FLAC, Range: the region or Whole project → Whole project, a
   Title typed — the estimate reads "Estimated size: up to …";
6. Preset: a name typed, Save as preset → the preset is listed and chosen;
7. Render… → the progress window "Render to MKV" runs to the end → Close; the file on disk is
   Matroska (EBML), named "….mkv".
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

# the save seam is read at start-up: the scenario owns its process
OWN_PROCESS = True


def main(out):
    require_tools()
    os.makedirs(out, exist_ok=True)
    target = os.path.join(out, "kt-settings-4s.mkv")
    if os.path.exists(target):
        os.remove(target)
    was = os.environ.get("KT_FILE_PICK")
    os.environ["KT_FILE_PICK"] = target
    proj = project_with_clips(os.path.join(out, "settings.otio"), [(0.0, 4.0, "synth:Ramp")])
    # a region of the first two seconds: the window offers the region or the whole project
    with open(proj) as f:
        doc = json.load(f)
    doc["metadata"]["kt"]["transport"] = {"region": [0, 180000]}
    with open(proj, "w") as f:
        json.dump(doc, f, indent=4)
    try:
        with Live("render_settings", out, project=proj, own_process=True) as app:
            # 1) the window and its groups
            app.click_probe("menu.file", "the File menu")
            app.click_probe("file.render", "File → Render…")
            app.expect(app.has_probe("render.start"), "the settings window offers Render…")
            for g in ("grp.render.preset", "grp.render.video", "grp.render.size", "grp.render.audio", "grp.render.file"):
                app.expect(app.has_probe(g), f"the group {g} is drawn")
            app.step("settings", "the render settings in groups: Preset, Video, Size and rate, Audio, File")
            app.click_probe("render.preset", "the Preset list")
            app.expect(app.has_probe("render.preset.0"), "the factory presets are listed")
            app.step("presets", "the presets: named by codec, size and rate")
            app.click_probe("render.preset.0", "H.264 · MP4 · quality 23")
            app.hover_probe("render.quality", "the CRF / CQ number")

            # 2) the container
            app.click_probe("render.container", "the Container list")
            app.expect(app.has_probe("render.container.mkv"), "MKV is offered")
            app.step("containers", "the containers: MP4, MOV, MKV, WebM")
            app.click_probe("render.container.mkv", "MKV")

            # 3) the codec
            app.click_probe("render.codec", "the Codec list")
            app.expect(app.has_probe("render.codec.h265"), "H.265 is offered")
            app.step("codecs", "the codecs; what cannot be taken is greyed with its reason")
            app.click_probe("render.codec.h265", "H.265")

            # 4) the rate mode
            app.click_probe("render.rate", "the Rate list")
            app.click_probe("render.rate.vbr", "VBR")
            app.expect(app.has_probe("render.maxrate"), "the maximum row appears")
            app.hover_probe("render.bitrate", "the average, Mbit/s")
            app.step("vbr", "VBR: the average and the maximum, Mbit/s")

            # 5) speed, keyframes, depth, colour, size, frame rate, audio
            app.click_probe("render.speed", "the Encoder speed list")
            app.click_probe("render.speed.slow", "slow")
            app.hover_probe("render.keyint", "a keyframe at least every, s")
            app.hover_probe("render.bframes", "B-frames")
            app.click_probe("render.depth", "the Bit depth list")
            app.hover_probe("render.depth.8", "8-bit")
            app.click_probe("render.depth.10", "10-bit")
            app.click_probe("render.colour", "the Colour list")
            app.click_probe("render.colour.1", "BT.709 limited")
            app.click_probe("render.size", "the Resolution list")
            app.click_probe("render.size.custom", "Custom")
            app.expect(app.has_probe("render.width"), "the width and height rows appear")
            app.hover_probe("render.width", "Width")
            app.hover_probe("render.height", "Height")
            app.step("custom", "a custom frame size: Width and Height")
            app.click_probe("render.fps", "the Frame rate list")
            app.hover_probe("render.fps.project", "Project (30)")
            app.click_probe("render.fps.25", "25")
            app.click_probe("render.audio", "the Audio codec list")
            app.click_probe("render.audio.flac", "FLAC")
            app.scroll_probe_into_view("render.estimate", "render.audio", below="render.start")
            app.click_probe("render.range", "the Range list")
            app.hover_probe("render.range.region", "Region 0:00–0:02")
            app.click_probe("render.range.whole", "Whole project")
            app.click_probe("render.title", "the Title field")
            app.type_text("Settings demo")
            app.hover_probe("render.comment", "the Comment field")
            app.expect(app.has_probe("render.estimate"), "the estimate is drawn")
            app.step("filled", "10-bit, BT.709 limited, 25 fps, FLAC; “Estimated size: up to …”")

            # 6) a preset of one's own
            app.scroll_probe_into_view("render.preset_name", "render.audio", below="render.start")
            app.click_probe("render.preset_name", "the preset name field")
            app.type_text("Archive MKV")
            app.click_probe("render.preset_save", "Save as preset")
            app.expect(app.has_probe("render.preset_delete"), "the preset is saved and chosen")
            app.step("preset", "“Archive MKV” saved and chosen; Delete preset is enabled")

            # 7) the render
            app.click_probe("render.start", "Render…")
            app.expect(app.has_probe("render.close", tries=240), "the render finished")
            app.step("done", "“Render to MKV” finished: the summary, Open file, Close")
            app.click_probe("render.close", "Close")
            app.expect(app.wait_gone("render.close"), "the progress window closed")
            with open(target, "rb") as f:
                head = f.read(4)
            app.expect(head == b"\x1a\x45\xdf\xa3", "the file on disk is Matroska")
            app.step("closed", "the timeline again, the .mkv rendered")
            return app.done()
    finally:
        if was is None:
            os.environ.pop("KT_FILE_PICK", None)
        else:
            os.environ["KT_FILE_PICK"] = was


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

#!/usr/bin/env python3
"""Live scenario render-finish-crash: a render of a short region off the frame grid finishes and the
program lives on.

The owner (kkt-26.11.01b item 2): "it reaches 100 % and that is all, the program folds … even 0.2 s
anywhere". His build announced the floored frame count of the region while the engine rendered from
the frame containing the start: the progress went past 100 % and the render window panicked on the
UI thread. The project is OUR synthetic fixture (a generator) at 24 fps, the region 0.2 s starting a
third of a frame after 1 s — the case every hand-drawn region is in.

PATH (export spec):
1. the project opens with the region drawn → File → Render… → the settings window;
2. Render… → the progress window runs;
3. the bar fills → the summary line, Open file and Open folder, Close at the right;
4. Close → the window is gone, the program keeps drawing, no crash window, no crash report; the
   file on disk is an MP4.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

# the save seam is read at start-up, and the crash window is the supervisor's: its own process
OWN_PROCESS = True

SEC = 90000
# 24 fps: one frame is 3750 ticks; the region starts a third of a frame after 1 s
R0 = SEC + 1250
R1 = R0 + 18000


def main(out):
    require_tools()
    os.makedirs(out, exist_ok=True)
    target = os.path.join(out, "region.mp4")
    if os.path.exists(target):
        os.remove(target)
    proj = project_with_clips(os.path.join(out, "renderregion.otio"), [(0.0, 4.0, "synth:Ramp")], fps=24)
    with open(proj) as f:
        doc = json.load(f)
    doc["metadata"]["kt"]["transport"] = {"region": [R0, R1]}
    with open(proj, "w") as f:
        json.dump(doc, f, indent=4)
    was = os.environ.get("KT_FILE_PICK")
    os.environ["KT_FILE_PICK"] = target
    try:
        with Live("renderregion", out, project=proj, own_process=True) as app:
            region = app.project_now().doc["metadata"]["kt"].get("transport", {}).get("region")
            app.expect(region == [R0, R1], f"the region off the grid is loaded: {region}")
            # 1) the settings
            app.click_probe("menu.file", "the File menu")
            app.click_probe("file.render", "File → Render…")
            app.expect(app.has_probe("render.start"), "the settings window offers Render…")
            app.step("settings", "the render settings window over the region")

            # 2) the run
            app.click_probe("render.start", "Render… in the settings")
            app.step("running", "the progress window runs")

            # 3) the finish: the program did not fold at 100 %
            app.expect(app.has_probe("render.close", tries=240), "the render finished, the window draws")
            app.expect(app.has_probe("render.open_file"), "Open file is offered")
            app.step("done", "the summary line, Open file and Open folder, Close at the right")

            # 4) Close: the program draws on, the supervisor opened no crash window
            app.click_probe("render.close", "Close")
            app.expect(app.wait_gone("render.close"), "the progress window closed")
            app.expect(app.alive() and app.wait_frame(2, timeout=10.0, soft=True), "the program draws after Close")
            app.expect(not os.path.exists(app.geom_path + ".crash.json"), "no crash window")
            crash = os.path.join(out, "xdg", "kinetren", "kt-app-crash.txt")
            app.expect(not os.path.exists(crash), "no crash report")
            with open(target, "rb") as f:
                head = f.read(12)
            app.expect(head[4:8] == b"ftyp", "the file on disk is an MP4")
            app.step("closed", "the timeline again, the region rendered")
            return app.done()
    finally:
        if was is None:
            os.environ.pop("KT_FILE_PICK", None)
        else:
            os.environ["KT_FILE_PICK"] = was


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

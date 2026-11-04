#!/usr/bin/env python3
"""Live scenario render-window-v1-parity: the render window at v1's level of information.

The owner (kkt 2026-10-27, item 4): bring the render window at least to v1 — what it renders, with
which encoder, how fast, how much is left — and ask before writing over a file (ktprompts v26.10
U1). The save dialog is answered by the `KT_FILE_PICK` seam with a file that is already there; the
project is OUR synthetic fixture (a generator), not one of the owner's.

PATH (export spec):
1. File → Render… → the settings window;
2. Render… → the question "kt-demo-200s.mp4 already exists (… MB, date). Replace it?" with Replace and
   Choose another name, the file's bytes untouched;
3. Choose another name → the save step again (the seam answers the same file) → the same question;
4. Replace → the progress window: "Output" (resolution, fps, the encoder that opened, audio,
   container, backend, file) and "Run" (phase, frames, elapsed, remaining, size, bitrate), the speed
   line and the chart, the bar, Pause and Cancel;
5. the Log section opened: the backend line and the opened encoder, each with its elapsed stamp;
6. Pause → "paused" → Resume;
7. the bar fills → the numbers stop, the summary line, Open file and Open folder (hovered here: a
   click would launch the system's player and file manager), Close at the right;
8. Close → the window is gone; the file on disk is a new MP4.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

# the save seam is read at start-up: the scenario owns its process
OWN_PROCESS = True


def main(out):
    require_tools()
    os.makedirs(out, exist_ok=True)
    target = os.path.join(out, "kt-demo-200s.mp4")
    old = bytes(range(256)) * 16
    with open(target, "wb") as f:
        f.write(old)
    was = os.environ.get("KT_FILE_PICK")
    os.environ["KT_FILE_PICK"] = target
    # long enough to still run through the Log and Pause steps: the pipelined export
    # (export-throughput) rendered the former 40 s before the scenario reached Pause
    proj = project_with_clips(os.path.join(out, "render.otio"), [(0.0, 200.0, "synth:Ramp")])

    def untouched():
        with open(target, "rb") as f:
            return f.read() == old

    try:
        with Live("render", out, project=proj, own_process=True) as app:
            # 1) the settings
            app.click_probe("menu.file", "the File menu")
            app.click_probe("file.render", "File → Render…")
            app.expect(app.has_probe("render.start"), "the settings window offers Render…")
            app.step("settings", "the render settings window")

            # 2) the file is there: asked first, nothing written
            app.click_probe("render.start", "Render… in the settings")
            app.expect(app.has_probe("ask.replace"), "an existing file is asked about")
            app.expect(not app.has_probe("render.pause", tries=2), "the render did not start before the answer")
            app.expect(untouched(), "the file is untouched before Replace")
            app.step("overwrite", "“kt-demo-200s.mp4 already exists (… MB, date). Replace it?”")

            # 3) another name: the save step again, the same file, the same question
            app.click_probe("ask.other_name", "Choose another name")
            app.expect(app.has_probe("ask.replace"), "the save step came back with the same file")
            app.expect(untouched(), "the file is untouched after another name")
            app.step("other-name", "the question again after Choose another name")

            # 4) Replace: the progress window with both forms and the chart
            app.click_probe("ask.replace", "Replace")
            app.expect(app.has_probe("render.pause"), "the progress window runs")
            app.expect(app.has_probe("render.chart"), "the speed chart is drawn")
            app.hover_probe("render.cancel", "Cancel while the run goes")
            app.step("running", "Output and Run side by side, the speed line and the chart, the bar")

            # 5) the log section
            app.click_probe("render.log", "the Log header")
            app.step("log", "the log: the backend and the opened encoder, each with its stamp")

            # 6) Pause and Resume
            app.click_probe("render.pause", "Pause")
            app.step("paused", "“paused” next to Resume")
            app.click_probe("render.pause", "Resume")

            # 7) the finish: frozen numbers, the summary and the next actions
            app.expect(app.has_probe("render.close", tries=240), "the render finished")
            app.expect(app.has_probe("render.open_file"), "Open file is offered")
            app.hover_probe("render.open_file", "Open file")
            app.hover_probe("render.open_folder", "Open folder")
            app.expect(not app.has_probe("render.open_log", tries=2), "a successful render offers no log button")
            app.step("done", "the summary line, Open file and Open folder, Close at the right")

            # 8) Close: the window is gone, the file is a new MP4
            app.click_probe("render.close", "Close")
            app.expect(app.wait_gone("render.close"), "the progress window closed")
            with open(target, "rb") as f:
                head = f.read(12)
            app.expect(head[4:8] == b"ftyp", "the file on disk is a new MP4")
            app.step("closed", "the timeline again, the file rendered")
            return app.done()
    finally:
        if was is None:
            os.environ.pop("KT_FILE_PICK", None)
        else:
            os.environ["KT_FILE_PICK"] = was


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

#!/usr/bin/env python3
"""Live scenario render-honest (found-defects 711, architecture review R8: "the final render is
slower than the preview" — the preview composes a smaller picture, and nothing said so).

The render settings window states the preview's size against the render's and a render time
predicted from what the preview's frames cost over the range; before the range has played it says
that nothing was measured. The project is OUR synthetic fixture at 1920×1080, not the owner's.

PATH (export spec, t2903):
1. File → Render… → the File group under the size estimate: "Preview W×H · render 1920×1080: N× the
   pixels of a frame" and "Render time: not measured — play the range once"; Esc closes the window;
2. Space → the range plays to its end → Space;
3. File → Render… → "Render time ≈ m:ss (measured … of … frames)".
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402


def main(out):
    require_tools()
    os.makedirs(out, exist_ok=True)
    proj = project_with_clips(os.path.join(out, "honest.otio"), [(0.0, 3.0, "synth:Ramp")], width=1920, height=1080)
    with Live("render_honest", out, project=proj) as app:
        # 1) nothing measured yet
        app.click_probe("menu.file", "the File menu")
        app.click_probe("file.render", "File → Render…")
        app.expect(app.has_probe("render.start"), "the settings window is open")
        app.scroll_probe_into_view("render.time", "render.audio", below="render.start")
        app.expect(app.has_probe("render.preview_vs"), "the preview's size is stated against the render's")
        app.hover_probe("render.preview_vs", "Preview W×H · render 1920×1080: N× the pixels of a frame")
        app.expect(app.has_probe("render.time.none"), "the render time is not measured yet")
        app.hover_probe("render.time", "Render time: not measured — play the range once")
        app.step("unmeasured", "the size against the preview; “Render time: not measured — play the range once”")
        app.key("Escape")
        app.expect(app.wait_gone("render.start"), "Esc closed the window")

        # 2) the range plays once
        app.key("Home")
        app.key("space")
        app.wait_playhead(lambda p: p >= 2.8 or not app.playing(), "the range played to its end", timeout=60.0)
        if app.playing():
            app.key("space")
        app.step("played", "the range played once: the preview measured its frames")

        # 3) the prediction
        app.click_probe("menu.file", "the File menu")
        app.click_probe("file.render", "File → Render…")
        app.scroll_probe_into_view("render.time", "render.audio", below="render.start")
        app.expect(app.has_probe("render.time.measured", tries=60), "the render time is predicted")
        app.hover_probe("render.time", "Render time ≈ m:ss (measured … of … frames)")
        app.step("predicted", "“Render time ≈ … (measured … of … frames)” under the size against the preview")
        app.key("Escape")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

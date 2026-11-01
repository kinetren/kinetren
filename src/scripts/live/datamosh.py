#!/usr/bin/env python3
"""Live scenario t1249 (datamosh-codec-path): a person moshes a cut in the real program.

Two clips back to back on one track — shot A (colour bars, still) and shot B (a moving test
pattern, scrolled). The person double-clicks B, picks "Datamosh → Cut mosh" in the gallery and
looks at the frame half a second after the cut: the codec dropped the I-frame at the cut, so the
picture there is A's texture dragged by B's motion — not B. Undo brings B back; the two preview
images must differ, and the saved file must carry the slot with the cut-mosh rows.

Our own fixtures (ffmpeg lavfi), never the owner's projects (R.67).
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, pattern_media, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def moving_media(out_dir, name, seconds, fps=30, size="640x360"):
    """A real mp4 with GLOBAL motion: testsrc2 scrolled sideways, so every block has a vector."""
    path = os.path.join(out_dir, f"{name}.mp4")
    if os.path.exists(path):
        return path
    subprocess.run(
        [
            "ffmpeg", "-v", "error", "-y",
            "-f", "lavfi", "-i", f"testsrc2=size={size}:rate={fps}:duration={seconds},scroll=horizontal=0.006",
            "-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo",
            "-shortest",
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-b:a", "96k",
            path,
        ],
        check=True,
    )
    return path


def moshes(text):
    """The datamosh slots of the saved project."""
    found = []

    def walk(node):
        if isinstance(node, dict):
            if node.get("kind") == "datamosh":
                found.append(node)
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)

    walk(json.loads(text))
    return found


def main(out):
    require_tools()
    a = pattern_media(out, "mosh-a", 3)
    b = moving_media(out, "mosh-b", 3)
    proj = os.path.join(out, "datamosh.otio")
    project_with_clips(proj, [(0, 2, a), (2, 2, b)], width=640, height=360)

    with Live("datamosh", out, project=proj) as app:
        y = app.lane_y("video", 0, frac=0.5)
        app.step("timeline", "two clips back to back: bars, then a moving pattern")

        # the frame we judge: half a second after the cut, inside the mosh
        app.click(app.x_at(int(2.5 * SEC)), app.ruler_y())
        center = app.preview_center()
        plain = app.pixel_settled(center)
        plain_shot = app.step("plain", "the frame after the cut without the effect: shot B")

        app.double_click(app.x_at(int(3.0 * SEC)), y)
        app.step("fxwin", "double click on the second clip: its FX window")
        # the window is judged by its picker button, which it publishes once drawn (a screen diff
        # taken right after the gesture can precede the window's first frame)
        app.expect(app.has_probe("fxreg.add"), "the FX window opened (its picker button is there)")

        if not app.click_probe("fxreg.add", "the program gave the rectangle of the picker button"):
            return app.done()
        for ch in "datamosh":
            app.key(ch)
        app.step("gallery", "the gallery narrowed to Datamosh")
        cell = None
        for _ in range(40):
            if app.has_probe("fxreg.var.datamosh.0", tries=1):
                cell = True
                break
            poll_step(0.25)
        app.expect(cell is not None, "the gallery shows the Cut mosh preset")
        if cell is None:
            return app.done()
        if not app.click_probe("fxreg.var.datamosh.0", "the Cut mosh cell is clickable"):
            return app.done()
        app.step("picked", "Cut mosh added to the chain")
        app.key("Escape")
        # the double click moved the playhead into the clip: back to the judged frame
        app.click(app.x_at(int(2.5 * SEC)), app.ruler_y())

        # the preview after the cut is now the decoded mosh (the history warms first)
        moshed = app.pixel_settled(center, differs_from=plain, tries=60)
        mosh_shot = app.step("moshed", "the same frame moshed: the bars dragged by the pattern's motion")
        app.expect(moshed is not None and moshed != plain, f"the frame after the cut changed ({plain} → {moshed})")
        d = app.screen_diff(plain_shot)
        app.expect(d > 0.01, f"the preview differs from shot B (the screen changed by {d:.1%})")

        app.saved_project()
        slots = moshes(app.project_text())
        app.expect(len(slots) == 1, f"exactly one datamosh slot in the file ({len(slots)})")
        if slots:
            s = slots[0]
            app.expect(
                s.get("before") == 12 and s.get("drop_count") == 1,
                f"the slot carries the cut-mosh rows (before={s.get('before')}, drop_count={s.get('drop_count')})",
            )

        # undo brings shot B back
        app.key("ctrl+z")
        back = app.pixel_settled(center, differs_from=moshed, tries=60)
        app.expect(back == plain, f"undo shows shot B's frame again ({back} vs {plain})")
        app.step("undone", "Ctrl+Z: shot B again")
        app.expect(back is not None, "undo changed the frame back")
        app.expect(app.screen_diff(mosh_shot) > 0.01, "the undone frame differs from the moshed one")
        return app.done()


if __name__ == "__main__":
    out_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join("target", "live-out")
    os.makedirs(out_dir, exist_ok=True)
    sys.exit(main(out_dir))

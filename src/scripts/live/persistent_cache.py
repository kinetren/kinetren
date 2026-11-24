#!/usr/bin/env python3
"""Live scenario t3448 (persistent-cache; the owner's kkt-26.11.20 A10: kept as the preview plays it):
a heavy file gets a lighter copy on disk by itself, the card says so, the switch beside the quality turns
it off and the original plays, and as in the preview again the same copy is read.

The project is ours: a 4K long-GOP file and a small one. The stand has no hardware encoder, so the
scenario forces the export's software H.264 (as `render_cache` does). The verdict reads the disk and the
program's own dump:
- with the hands off a copy of the 4K file appears in the project's cache folder, at the size the preview plays, with no
  temporary file left, and none of the small one;
- the card carries the copy's mark, the Diagnostics window lists both kinds;
- at "Off" the mark goes, the copy's file stays, and the choice is kept in the program's record.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from idle import Meter, wait_quiet  # noqa: E402
from kt_live import Live, pattern_media, project_with_clips, require_tools  # noqa: E402

OWN_PROCESS = True


def heavy_media(out_dir):
    """Four seconds of 4K whose frames need others to decode."""
    path = os.path.join(out_dir, "heavy.mp4")
    if not os.path.exists(path):
        subprocess.run(
            ["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", "testsrc2=size=3840x2160:rate=30:duration=4",
             "-c:v", "libx264", "-preset", "ultrafast", "-g", "120", "-pix_fmt", "yuv420p", path],
            check=True,
        )
    return path


def copies(root):
    """The copies in the folder, and the temporary files beside them."""
    done, rest = [], []
    for top, _, files in os.walk(os.path.join(root, "media")):
        for f in files:
            if f.startswith("copy-") and f.endswith(".mov"):
                done.append(f)
            elif "copy-" in f:
                rest.append(f)
    return done, rest


def main(out):
    require_tools()
    os.environ.setdefault("KT_RENDER_CACHE_ENCODER", "libx264")
    home = os.path.join(out, "film")
    os.makedirs(home, exist_ok=True)
    heavy = heavy_media(home)
    light = pattern_media(home, "light", 2)
    proj = project_with_clips(os.path.join(home, "film.otio"), [(0, 4, heavy), (4, 2, light)], width=1920, height=1080)
    root = os.path.join(home, "kinetren-cache")
    # what a person would pick as the camera's copy: the dialog's answer, a file of another length
    os.environ["KT_FILE_PICK"] = light
    with Live("persistent_cache", out, project=proj, own_process=True) as app:
        app.step("open", "a 4K file and a small one on the timeline; the hands are off")
        app.until(lambda: copies(root)[0] and not copies(root)[1], timeout=300)
        done, rest = copies(root)
        app.expect(len(done) == 1 and not rest, f"one copy, of the 4K file, and nothing unfinished: {done} {rest}")
        app.until(lambda: app.has_probe("bin.copy"), timeout=60)
        app.expect(app.has_probe("bin.copy"), "the card of the 4K file carries the copy's mark")
        app.hover_probe("bin.copy", "the copy's mark")
        wait_quiet(Meter(app), "after the copy was made")
        app.step("made", f"the copy is made by itself: {done[0]}; the card shows the disk and its lines")

        app.right_click_probe("bin.copy", "the card of the 4K file")
        app.click_probe("bin.copy.camera", "Use a camera's copy…")
        app.until(lambda: any("another length" in n for n in app.geom().get("notices", [])), timeout=60)
        app.expect(any("another length" in n for n in app.geom().get("notices", [])),
                   "a camera's copy of another length is refused, and the message names the lengths")
        app.step("refused", "a wrong camera copy is refused: its length is named; ours goes on playing")

        if not app.has_probe("tb.diag", tries=2):
            app.click_probe("tb.more", "the toolbar's more button")
        app.click_probe("tb.diag", "the gauge button of the toolbar")
        app.until(lambda: app.has_probe("diag.pcache.copies"), timeout=30)
        app.expect(app.has_probe("diag.pcache.frames") and app.has_probe("diag.pcache.copies"),
                   "Diagnostics list the store's two kinds")
        app.step("diagnostics", "Diagnostics: the persistent cache's frames and source copies, with sizes and reads")
        app.key("Escape")

        app.click_probe("pcache.switch", "the switch beside the quality")
        app.until(lambda: app.has_probe("pcache.quarter"), timeout=30)
        app.step("switch", "the switch: as in the preview, 1/2, 1/4, off")
        app.click_probe("pcache.quarter", "1/4")
        app.until(lambda: app.has_probe("bin.copy"), timeout=60)
        app.expect(copies(root)[0] == done, "a quarter is what the preview plays at: the same copy")
        app.click_probe("pcache.switch", "the switch")
        app.click_probe("pcache.half", "1/2")
        app.until(lambda: copies(root)[0] == ["copy-1920x1080.mov"], timeout=300)
        done = copies(root)[0]
        app.expect(done == ["copy-1920x1080.mov"], f"a better copy is made and the smaller one goes: {done}")
        app.until(lambda: app.has_probe("bin.copy"), timeout=60)
        app.step("half", "1/2: a better copy takes the place of the smaller one")
        app.click_probe("pcache.switch", "the switch")
        app.click_probe("pcache.off", "Off")
        app.wait_gone("bin.copy")
        app.expect(not app.has_probe("bin.copy", tries=2), "at Off the card has no mark: the original plays")
        app.expect(copies(root)[0] == done, "the copy's file stays for the next time")
        app.step("off", "Off: the original plays, the card has no mark")
        record = os.path.join(out, "store", "start.json")
        app.until(lambda: os.path.exists(record) and json.load(open(record)).get("persistent_cache") == "off", timeout=30)
        app.expect(json.load(open(record)).get("persistent_cache") == "off", "the choice is kept between projects")

        app.click_probe("pcache.switch", "the switch again")
        app.click_probe("pcache.preview", "As in the preview")
        app.until(lambda: app.has_probe("bin.copy"), timeout=60)
        app.expect(app.has_probe("bin.copy"), "as in the preview again: the same copy is read, none is made")
        app.expect(copies(root)[0] == done, "no second copy was made")
        app.step("on", "as in the preview again: the copy that lay there is read")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

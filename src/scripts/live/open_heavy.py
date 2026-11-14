#!/usr/bin/env python3
"""Live scenario `open_heavy` (scheduler-followups, found-defects 270): opening a heavy project never
stands the window.

The new app's constructor opened every source (FFmpeg probe and sidecar) and built the player's
first version on the UI thread, and the closed project's player joined its decoders there: on the
heavy showcase the window stood for seconds. Now the file is read and its lock taken on the media
lane, the sources open there, the first version is built on the player's own thread and the closed
project is released on a thread of its own.

PATH (the durability spec, "The heavy showcase opens with the window answering"):
1. a small project is open → File › Open… → the heavy showcase (`kt-cli showcase --heavy`; the
   system dialog's answer is `KT_FILE_PICK` in Xvfb, the item itself is clicked);
2. at once the timeline shows the showcase's events and the preview says Loading; the File menu
   answers a click while it loads;
3. the preview shows the picture.

The verdict: the menu opens during the load, the UI thread's blocking counter (`<dump>.load.json`,
`block.ui`) stays at zero over the whole run, and the picture arrives.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kt_live  # noqa: E402
from kt_live import Live, demo_otio, require_tools  # noqa: E402
from idle import Meter  # noqa: E402
from readme_shots import fixture_sources  # noqa: E402

OWN_PROCESS = True


def build(out):
    sources = fixture_sources(os.path.join(out, "sources"))
    proj = os.path.join(out, "heavy.kt")
    subprocess.run([kt_live.KT_CLI, "showcase", proj, "--sources", sources, "--heavy", "--force"],
                   check=True, capture_output=True)
    with open(proj) as f:
        clips = len(json.load(f)["project"]["clips"])
    return proj, clips


def loading(app):
    """The "Loading" scrim is drawn (the preview publishes its rectangle while it is)."""
    return any(f["param"] == "preview.loading" for f in app.geom()["fields"])


def has_field(app, name):
    return any(f["param"] == name for f in app.geom()["fields"])


def main(out):
    require_tools()
    heavy, clips = build(out)
    small = demo_otio(os.path.join(out, "small.otio"))
    # there is no system dialog in the virtual screen: the menu item is pressed for real, the
    # dialog's answer is substituted
    os.environ["KT_FILE_PICK"] = heavy
    with Live("open_heavy", out, project=small, own_process=True) as app:
        meter = Meter(app)
        app.settle()
        before = len(app.geom().get("clips", []))
        app.step("small", f"a small project is open ({before} events)")

        app.click_probe("menu.file", "the “File” menu")
        app.expect(app.until(lambda: has_field(app, "file.open"), timeout=10.0), "the File menu opened")
        app.click_probe("file.open", "the “Open…” item")
        # the showcase's events are on the timeline as soon as the file is read
        app.expect(app.until(lambda: len(app.geom().get("clips", [])) == clips, timeout=30.0),
                   f"the showcase's {clips} events are on the timeline")
        seen_loading = loading(app)
        app.step("opened", f"the showcase's events at once; the preview {'says Loading' if seen_loading else 'already has its frame'}")

        # the window answers during the load: the File menu opens on a click
        app.click_probe("menu.file", "the “File” menu during the load")
        app.expect(app.until(lambda: has_field(app, "file.open"), timeout=10.0),
                   "the File menu opened while the project loads")
        app.step("menu", "the File menu answers while the showcase loads")
        app.key("Escape")

        app.expect(app.until(lambda: not loading(app), timeout=300.0), "the showcase's picture never came")
        app.step("picture", "the preview shows the showcase's picture")

        block = meter.read().get("block", {})
        app.expect(block.get("ui", -1) == 0, f"the UI thread made no blocking call over the run: {block}")
        print(f"   blocking calls: {block}, Loading seen: {seen_loading}")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

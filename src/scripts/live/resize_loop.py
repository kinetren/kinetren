#!/usr/bin/env python3
"""Live scenario t2724 (gpu-errors-survive, kkt-26.11.15 N1): the window is maximized, restored and
resized while a region loops, and the program lives.

The owner's words are in `docs/ktprompts/kkt-26.11.15.txt`, item 1: kt26.11.10 died when he
double-clicked the window title while a region looped in a simple project with no video. Its report
was a panic in wgpu's default error handler: egui's `Surface::configure` on the UI thread found the
queue busy with the preview's present from the preview's thread, and nothing caught the error.

The project is the demo (generators only, no video), a 2 s loop. Playing, the title strip's free
part is double-clicked six times and the window resized four times; after each gesture the program
is alive and the window has the size the gesture asked for. The journal's GPU lines are printed: a
refused reconfigure is a line there, not a crash.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, demo_project, geometry, require_tools  # noqa: E402
from loop import set_region  # noqa: E402

# its own program: the window it maximizes and resizes is not the stand's next scenario's, and its
# journal is the run's own
OWN_PROCESS = True
SEC = 90000


def gpu_lines(out):
    """The run's journal lines the GPU error sink wrote."""
    path = os.path.join(out, "xdg", "kinetren", "kt-app.log")
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            return [line.rstrip() for line in f if " GPU " in line]
    except OSError:
        return []


def main(out):
    require_tools()
    path = demo_project(os.path.join(out, "resize_loop.otio"))
    set_region(path, start=1.0, end=3.0)
    with Live("resize_loop", out, project=path, own_process=True) as app:
        app.key("space")
        app.wait_playhead(lambda t: t > SEC, "the cursor moved", timeout=15)
        if not app.playing():
            app.expect(False, "playback did not start (no audio device?)")
            return app.done()
        app.step("playing", "the demo project loops 1 s to 3 s")
        # the window's device busy from another thread, as the preview's thread keeps it under the
        # owner's heavy loop: each reconfigure below waits for the queue and finds new work in it
        app._ctl("gpu_busy", "60000")
        # read by a frame (the loop draws them): no poke, which would overwrite the command
        app.wait_frame(4, what="the bench read the busy command", soft=True, poke=False)

        before = geometry()
        for i in range(6):
            app.double_click_probe("title.drag", "the title strip's free part was published")
            app.wait_frame(3, what=f"frames after maximize toggle {i + 1}", soft=True)
            app.settle(timeout=5.0)
            app.expect(app.alive(), f"the program lives after maximize toggle {i + 1}")
            now = geometry()
            grew = now["w"] > before["w"] or now["h"] > before["h"]
            back = abs(now["w"] - before["w"]) <= 2 and abs(now["h"] - before["h"]) <= 2
            # an odd toggle maximizes, an even one restores
            app.expect(grew if i % 2 == 0 else back,
                       f"toggle {i + 1}: the window took its size ({before['w']}x{before['h']} -> {now['w']}x{now['h']})")
            if i == 0:
                app.step("maximized", "the window fills the screen while the loop plays")
        app.expect(app.playing(), "the loop still plays after the toggles")

        for w, h in ((1100, 700), (1500, 900), (900, 680), (1440, 860)):
            app.resize(w, h)
            app.expect(app.alive(), f"the program lives after a resize to {w}x{h}")
            now = geometry()
            app.expect(abs(now["w"] - w) <= 2 and abs(now["h"] - h) <= 2,
                       f"the window has the asked size {w}x{h}: {now['w']}x{now['h']}")
        app.step("resized", "four resizes later the loop still plays and the picture fills the preview")
        app.expect(app.playing(), "the loop still plays after the resizes")
        app.key("space")
        lines = gpu_lines(out)
        refused = [line for line in lines if "Surface::configure" in line]
        print(f"   journal GPU lines: {len(lines)}, refused reconfigures: {len(refused)}")
        for line in lines[:5]:
            print(f"     {line}")
        app.expect(refused, "the busy queue refused a reconfigure, and the journal has its line (the race was met)")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

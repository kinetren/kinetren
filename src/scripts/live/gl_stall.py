#!/usr/bin/env python3
"""Live scenario t2743 (gl-context-holder, kkt-26.11.15 N1 class, found-defects 651): a GL call of the
preview stalled by the system does not end the program.

On GL every call into a device takes its adapter's one context lock, and egui's paint panics after six
seconds without it ("Could not lock adapter context"). In the stage-15 finish.sh three live scenarios
died that way at the same second: a call of ours on the window's device was stalled, and the UI's paint
waited for its lock. The preview now composes on a GL device of its own.

The bench's `gl_stall 9` holds the preview engine's GL context lock for nine seconds from a thread, as
a stalled call would. Meanwhile the person clicks the ruler and the UI keeps drawing; the program
lives, and after the release the preview shows a picture again. On the old code (the preview on the
window's device) the first paint after six seconds panicked.
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, demo_project, require_tools  # noqa: E402

# its own program: a held device must not reach the stand's next scenario
OWN_PROCESS = True
SEC = 90000
HOLD_S = 9


def bench_lines(out):
    path = os.path.join(out, "xdg", "kinetren", "kt-app.log")
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            return [line.rstrip() for line in f if " BENCH " in line]
    except OSError:
        return []


def main(out):
    require_tools()
    path = demo_project(os.path.join(out, "gl_stall.otio"))
    with Live("gl_stall", out, project=path, own_process=True) as app:
        app.expect(app.until(lambda: app.geom()["compose"]["picture"], timeout=60.0), "the preview shows a picture")
        app.step("open", "the demo project, the preview shows its first frame")

        app._ctl("gl_stall", str(HOLD_S))
        # read by a frame: no poke, which would overwrite the command
        app.wait_frame(2, what="the bench read the stall command", soft=True, poke=False)
        app.expect(app.until(lambda: any("is held" in line for line in bench_lines(out)), timeout=30.0),
                   "the bench holds the preview's device")
        start = app.geom()["frame"]
        t0 = time.time()
        # the person goes on working until the device is released: longer than wgpu-hal's six seconds
        i = 0
        while not any("released" in line for line in bench_lines(out)):
            app.click(app.x_at(int((1 + i % 8) * SEC)), app.ruler_y())
            app.wait_frame(2, what=f"a frame after click {i + 1}", soft=True)
            app.expect(app.alive(), f"the program lives {time.time() - t0:.1f} s into the stall")
            i += 1
            if not app.alive() or time.time() - t0 > 3 * HOLD_S:
                break
        held_s = time.time() - t0
        drawn = app.geom()["frame"] - start if app.alive() else 0
        app.expect(held_s > 6.0, f"the clicks went on past six seconds of the stall: {held_s:.1f} s")
        app.expect(drawn > 8, f"the UI kept drawing while the preview's device was held: {drawn} frames")
        app.step("held", "the preview's device is held; the clicks still move the cursor and the window draws")

        app.expect(app.until(lambda: any("released" in line for line in bench_lines(out)), timeout=60.0),
                   "the bench released the preview's device")
        app.click(app.x_at(int(2.5 * SEC)), app.ruler_y())
        app.expect(app.until(lambda: app.geom()["compose"]["picture"], timeout=60.0),
                   "after the release the preview shows a picture again")
        app.expect(app.alive(), "the program lives after the stall")
        app.step("released", "the device is free again, the preview composes the clicked moment")
        for line in bench_lines(out):
            print(f"     {line}")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

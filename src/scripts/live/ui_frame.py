#!/usr/bin/env python3
"""Live scenario ui-frame-cost: a UI frame costs what is visible, not the project.

The owner's words are in `docs/ktprompts/kkt-26.11.07b.txt`, item 1: a pointer over the preview on a
pause took a quarter of the processor, and with the effects window open a frame took half a second.
The project is ours (`kt-cli showcase --heavy`): ten releases of a composition under Slowed, VHS Tape
and Camera ISP over a dense edit. The verdict is the UI thread's own processor time per frame, by
stage, as the program measures it (`frame_cost`, read from `<dump>.load.json`): not percent, which on
this machine is mostly the software rasterizer's.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kt_live  # noqa: E402
from idle import Meter, sweep, wait_quiet  # noqa: E402
from kt_live import Live, require_tools  # noqa: E402
from readme_shots import fixture_sources  # noqa: E402

OWN_PROCESS = True

# The limits ARE the check (t1671 lists them): the program's own drawing of one frame, in
# milliseconds of the UI thread, on the debug build this scenario runs. Measured 2026-11-07 on the
# heavy showcase: about 1 ms over the preview and the timeline and 2 ms with the effects window
# open; the code before the change spent 5-6 ms on the owner's project in a RELEASE build, where
# the debug build is several times slower.
HOVER_LIMIT_MS = 6.0
WINDOW_LIMIT_MS = 10.0


def stages(meter, seconds, act):
    """The frames' average cost by stage while `act` runs, and the work counted."""
    a = meter.read()
    work, cpu = meter.over(seconds, act)
    b = meter.read()
    n = b["frame"]["frames"] - a["frame"]["frames"]
    ms = {k: (b["frame"]["ns"][k] - a["frame"]["ns"][k]) / max(n, 1) / 1e6 for k in b["frame"]["ns"]}
    return n, ms, work


def show(name, n, ms, work):
    print(
        f"   {name}: {n} frames, ms a frame: "
        + ", ".join(f"{k} {v:.2f}" for k, v in ms.items())
        + f"; event passes {work.get('event_passes', 0)}"
    )


def main(out):
    require_tools()
    sources = fixture_sources(os.path.join(out, "sources"))
    proj = os.path.join(out, "heavy.kt")
    subprocess.run(
        [kt_live.KT_CLI, "showcase", proj, "--sources", sources, "--heavy", "--force"],
        check=True, capture_output=True,
    )
    with Live("ui_frame", out, project=proj, own_process=True) as app:
        meter = Meter(app)
        wait_quiet(meter, "after opening")
        app.step("open", "the heavy project is open, nothing touched")
        wait_quiet(meter, "after the screenshot")

        g = app.geom()
        win = kt_live.geometry()
        cx, cy = app.preview_center()
        y_lane = app.lane_y("video", 0, frac=0.6)
        x0, x1 = win["x"] + int(g["timeline"]["left"]) + 40, win["x"] + int(g["timeline"]["right"]) - 80
        phases = [
            ("the preview", (cx - 90, cy - 50), (cx + 90, cy + 50)),
            ("the timeline", (x0, y_lane), (x1, y_lane)),
        ]
        for name, a, b in phases:
            app.step(f"hover-{name.split()[-1]}", f"the pointer goes over {name}")
            n, ms, work = stages(meter, 3.0, sweep(app, a, b))
            show(f"over {name}", n, ms, work)
            app.expect(n > 0, f"over {name}: no frame saw the pointer, so this proves nothing")
            app.expect(ms["frame"] < HOVER_LIMIT_MS, f"over {name} a frame cost {ms['frame']:.2f} ms of the UI thread")
            wait_quiet(meter, f"after {name}")

        # the effects window of a release: Slowed, VHS Tape and Camera ISP
        g = app.geom()
        x_reel = app.x_at(g["playhead"])
        app.click(x_reel, y_lane)
        app.right_click(x_reel, y_lane)
        app.click_probe("tl.fx", "item “Effects…” of the event")
        app.step("fx-open", "the effects window of the composition is open")
        wait_quiet(meter, "with the effects window open")
        r = app._probe_rect("win.fx")
        app.expect(r is not None, "the effects window published its rectangle")
        if r is not None:
            wx, wy = win["x"] + int(r["x"]) + 40, win["y"] + int(r["y"]) + 60
            n, ms, work = stages(meter, 3.0, sweep(app, (wx, wy), (wx + 160, wy + 100)))
            show("over the effects window", n, ms, work)
            app.expect(n > 0, "over the effects window: no frame saw the pointer")
            app.expect(ms["frame"] < WINDOW_LIMIT_MS, f"with the effects window a frame cost {ms['frame']:.2f} ms")
            app.expect(not work.get("composites"), f"a pointer over the window composed the picture: {work}")
        app.key("Escape")
        app.settle()

        # the whole project in the view: many times more events on screen than before
        tpp0 = app.geom()["ticks_per_px"]
        for _ in range(4):
            app._mods(["ctrl"], True)
            app.wheel(cx, y_lane, 10)
            app._mods(["ctrl"], False)
            app.settle()
            if app.geom()["ticks_per_px"] > tpp0 * 8:
                break
        app.expect(app.geom()["ticks_per_px"] > tpp0 * 8, "Ctrl+wheel did not zoom the timeline out")
        app.step("zoomed-out", "the timeline zoomed out: the whole edit is in view")
        wait_quiet(meter, "after the zoom")
        n, ms, work = stages(meter, 3.0, sweep(app, (x0, y_lane), (x1, y_lane)))
        show("over the zoomed-out timeline", n, ms, work)
        app.expect(ms["frame"] < HOVER_LIMIT_MS, f"over the zoomed-out timeline a frame cost {ms['frame']:.2f} ms")

        # the person sees the same number: Diagnostics → Performance
        if not app.has_probe("tb.diag", tries=2):
            app.click_probe("tb.more", "the toolbar's more button")
        app.click_probe("tb.diag", "the Diagnostics button")
        app.hover(cx, cy)
        app.until(lambda: app.has_probe("diag.ui_frame", tries=1), timeout=10.0)
        app.expect(app.has_probe("diag.ui_frame"), "Diagnostics shows what a UI frame costs")
        app.step("diagnostics", "Performance names the UI frame's cost by stage")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

#!/usr/bin/env python3
"""Live scenario edit-without-rebuild (t2007): a slider dragged in the effects window of a big project
while it plays.

The class (`docs/engine-audit.md` M1): an edit, and every step of a slider drag, rebuilt the whole
engine several times — the UI thread built a video renderer and raised the generator sources, the
audio-build thread, prefetch and the probe each built their own. The project is ours
(`kt-cli showcase --heavy`): ten releases of a composition under Slowed, VHS Tape and Camera ISP over a
dense edit. Playing, the VHS Tape dark clip field of the composition's effects window is dragged to
and fro. The verdict is what the program counts: no callback underrun (the sound did not stall), at
most one shared build (an audio and a video renderer) per edit, and a UI frame of the drag costs the
UI thread what a frame with the pointer moving over the same window costs, plus the window's own work
— not a build.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kt_live  # noqa: E402
from kt_live import Live, require_tools  # noqa: E402
from idle import Meter, sweep, wait_quiet  # noqa: E402
from readme_shots import fixture_sources  # noqa: E402

OWN_PROCESS = True


def frame_ms(a, b):
    """The UI thread's processor time per frame between two load readings, in ms."""
    n = b["frame"]["frames"] - a["frame"]["frames"]
    if n <= 0:
        return None, 0
    return (b["frame"]["ns"]["frame"] - a["frame"]["ns"]["frame"]) / n / 1e6, n


def main(out):
    require_tools()
    sources = fixture_sources(os.path.join(out, "sources"))
    proj = os.path.join(out, "heavy.kt")
    subprocess.run(
        [kt_live.KT_CLI, "showcase", proj, "--sources", sources, "--heavy", "--force"],
        check=True, capture_output=True,
    )
    with Live("edit_drag", out, project=proj, own_process=True) as app:
        meter = Meter(app)
        wait_quiet(meter, "after opening")
        app.step("open", "the heavy project is open")
        win = kt_live.geometry()
        g = app.geom()
        x_reel, y_reel = app.x_at(g["playhead"]), app.lane_y("video", 0, frac=0.6)

        # the composition's effects window, its VHS Tape slot, the dark clip field in view
        app.click(x_reel, y_reel)
        app.right_click(x_reel, y_reel)
        app.click_probe("tl.fx", "item “Effects…” of the event")
        deck = None
        for i in range(6):
            if not app.has_probe(f"fx.slot.{i}", tries=2):
                break
            app.click_probe(f"fx.slot.{i}", f"slot {i} of the chain")
            if app.has_probe("deck.standard.0", tries=3):
                deck = i
                break
        app.expect(deck is not None, "the chain has a VHS Tape slot whose deck face draws")
        if deck is None:
            return app.done()
        r = None
        for _ in range(15):
            r, done, w = app._probe_rect("dark_clip"), app._probe_rect("fx.done"), app._probe_rect("win.fx")
            if r is None or done is None or w is None:
                break
            if r["y"] > w["y"] + 30 and r["y"] + r["h"] < done["y"] - 4:
                break
            ticks = 2 if r["y"] + r["h"] >= done["y"] - 4 else -2
            app.wheel(win["x"] + int(w["x"] + w["w"] / 2), win["y"] + int(w["y"] + w["h"] / 2), ticks)
            r = None
        app.expect(r is not None, "the dark clip field scrolls into view")
        if r is None:
            return app.done()
        fx, fy = win["x"] + int(r["x"] + r["w"] / 2), win["y"] + int(r["y"] + r["h"] / 2)
        app.step("fx-open", "the composition's effects window, VHS Tape selected, the dark clip field in view")

        app.key("space")
        app.until(lambda: app.playing(), timeout=10.0)
        app.expect(app.playing(), "playback started (no audio device?)")
        # the first lap of heavy frames: let the start settle before measuring
        kt_live.pause(2.0, "the start's first composites")

        # the pointer moving over the window, no edit: the frame's cost without a build
        a = meter.read()
        meter.over(3.0, sweep(app, (fx - 120, fy - 40), (fx + 40, fy + 40)))
        b = meter.read()
        hover_ms, hover_n = frame_ms(a, b)
        app.step("hover", "playing, the pointer goes over the effects window")

        # the drag: to and fro along the field, an edit per moved frame
        g0, a = app.geom(), meter.read()
        for k in range(4):
            x1 = fx - 160 if k % 2 == 0 else fx + 160
            app.drag(fx, fy, x1, fy, steps=10)
        kt_live.pause(1.0, "the last step's build lands")
        g1, b = app.geom(), meter.read()
        drag_ms, drag_n = frame_ms(a, b)
        edits = b["work"].get("model_edits", 0) - a["work"].get("model_edits", 0)
        builds = b["work"].get("engine_builds", 0) - a["work"].get("engine_builds", 0)
        print(f"   playing: hover frame {hover_ms} ms ({hover_n} frames), drag frame {drag_ms} ms ({drag_n} frames)")
        print(f"   drag: {edits} edits, {builds} renderer builds, "
              f"callback underruns {g0['callback_underruns']} → {g1['callback_underruns']}")
        app.step("dragged", "the dark clip was dragged to and fro four times while playing")
        app.expect(edits > 0, "the drag on the dark clip field was an edit")
        # one shared build per version: an audio and a video renderer; versions in between are
        # never built (the edit-cost of the former code: five renderers per edit)
        app.expect(builds <= 2 * edits, f"at most one shared build per edit: {builds} renderers for {edits} edits")
        app.expect(g1["callback_underruns"] == g0["callback_underruns"],
                   f"the sound did not stall: callback underruns {g0['callback_underruns']} → {g1['callback_underruns']}")
        app.expect(app.playing(), "still playing after the drag")
        if hover_ms is not None and drag_ms is not None:
            # a frame of the drag holds the window's edit (the command, the model, the handover) and
            # no build: within twice the hover frame plus 10 ms, where a video build of this project
            # alone was 6 ms in release and several times that in this debug build
            app.expect(drag_ms <= 2 * hover_ms + 10.0,
                       f"a drag frame costs the UI thread {drag_ms:.1f} ms against {hover_ms:.1f} ms hovering")

        app.key("space")
        app.until(lambda: not app.playing(), timeout=10.0)
        wait_quiet(meter, "after stopping")
        work, cpu = meter.over(3.0)
        app.expect(not work, f"a pause after the drag runs nothing, ran {work}")
        app.step("paused", "stopped: nothing runs")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

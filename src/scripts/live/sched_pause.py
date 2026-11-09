#!/usr/bin/env python3
"""Live scenario one-scheduler: background work pauses while playing and resumes on a pause.

The owner agreed (kkt-26.11.08 H15, "Да") that thumbnails, analysis and the index pause during
playback. The project is ours (`kt-cli showcase --heavy`): on opening its files are indexed (peaks)
and its filmstrips ordered, so background work is pending when Space is pressed. A zoom during
playback orders more thumbnails. The verdict is the scheduler's own counters, which the load watch
writes into `<dump>.load.json` four times a second: no visible or background job starts while
playing, work waits, and it all runs after the pause.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
import kt_live  # noqa: E402
from kt_live import Failure, Live, require_tools  # noqa: E402
from readme_shots import fixture_sources  # noqa: E402

OWN_PROCESS = True
# the limit IS the check here: background work that has not finished by then never will
DRAIN_LIMIT_S = 300.0
PLAYING = 1


def load(app, part="sched"):
    path = app.geom_path + ".load.json"
    deadline = time.time() + 10.0
    while time.time() < deadline:
        try:
            with open(path) as f:
                return json.load(f)[part]
        except (OSError, ValueError, KeyError):
            kt_live._poll(0.05)
    raise Failure("the load watch wrote no scheduler counters")


def started(s):
    """Jobs started in the pausable classes (queued, started, done, cancelled per class)."""
    return s["classes"]["visible"][1] + s["classes"]["background"][1]


def main(out):
    require_tools()
    sources = fixture_sources(os.path.join(out, "sources"))
    proj = os.path.join(out, "heavy.kt")
    subprocess.run(
        [kt_live.KT_CLI, "showcase", proj, "--sources", sources, "--heavy", "--force"],
        check=True, capture_output=True,
    )
    with Live("sched_pause", out, project=proj, own_process=True) as app:
        app.step("open", "the heavy project is open; its filmstrips and waveforms start filling in")
        s = load(app)
        print(f"   after opening: {s}")

        app.key("space")
        app.until(lambda: app.playing(), timeout=20.0)
        app.expect(app.playing(), "Space started playback")
        app.until(lambda: load(app)["paused"] & PLAYING, timeout=10.0)
        app.expect(load(app)["paused"] & PLAYING, "playing paused the background classes")
        app.step("playing", "it plays; the filmstrips stop filling in")

        # more thumbnails are ordered while playing: the timeline zooms under the pointer
        x, y = app.x_at(app.playhead()), app.lane_y("video", 0)
        app._mods(["ctrl"], True)
        app.wheel(x, y, -3)
        app._mods(["ctrl"], False)
        app.step("zoomed", "Ctrl+wheel zoomed the timeline; the new thumbnails wait")

        a = load(app)
        waited = a["waiting"]
        t0 = time.time()
        while time.time() - t0 < 3.0:
            kt_live._poll(0.25)
            waited = max(waited, load(app)["waiting"])
        b = load(app)
        print(f"   while playing: {a} → {b}, waiting at most {waited}")
        app.expect(app.playing(), "it still plays")
        app.expect(started(b) == started(a), f"a visible or background job started while playing: {a} → {b}")
        app.expect(waited > 0, "nothing waited while playing, so this proves nothing")

        app.key("space")
        app.until(lambda: not app.playing(), timeout=20.0)
        app.until(lambda: load(app)["paused"] == 0, timeout=10.0)
        app.expect(load(app)["paused"] == 0, "the pause resumed the background classes")
        app.until(lambda: started(load(app)) > started(b), timeout=30.0)
        app.expect(started(load(app)) > started(b), "nothing resumed after the pause")
        t0 = time.time()
        while time.time() - t0 < DRAIN_LIMIT_S:
            c = load(app)
            if c["waiting"] == 0 and c["running"] == 0:
                break
            kt_live._poll(0.5)
        c = load(app)
        print(f"   after the pause: {c}")
        app.expect(c["waiting"] == 0 and c["running"] == 0, f"background work never drained: {c}")
        app.step("paused", "stopped; the waiting thumbnails and waveforms filled in")

        # found-defects row 140: after a pause the bench once repainted ~70 times a second with
        # nothing composing. Once the work above drained, a pause draws no frame.
        t0 = time.time()
        while time.time() - t0 < 30.0:
            w0 = load(app, "work").get("ui_frames", 0)
            kt_live._poll(3.0)
            frames = load(app, "work").get("ui_frames", 0) - w0
            if frames == 0:
                break
        print(f"   UI frames over 3 s of a settled pause: {frames}")
        app.expect(frames == 0, f"a settled pause after playing still draws: {frames} frames in 3 s")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/sched_pause"))

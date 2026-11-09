#!/usr/bin/env python3
"""Live scenario t2153 (structured-telemetry, kkt-26.11.09 I2/I5/I8): a short loop under a heavy chain plays
from what is kept.

The owner's words are in `docs/ktprompts/kkt-26.11.09.txt`, lines 1-2 and 9: a heavy Datamosh on a
region left the preview on "Loading", even on a pause, and a small piece passed a hundred times still
"did loading" (line 9). His chain on event 2149: Camera ISP → Datamosh (gop 66,
720 lines, 8 px blocks, search 64, rotated vectors on a curve) → VHS Tape. The project is ours
(`kt-cli showcase --heavy`): the longest plain footage event gets that chain, and a 0.7 s loop is set
2 s into it.

The verdict: paused on the loop, the frame comes; played, after its first laps every frame shown is
a kept composite and every burst heard comes from the kept mix; paused again on a frame of the loop,
the picture is there at once (no "Loading"). The structured trace of the run is kept next to the
screenshots and its `loop` answer printed.

`python3 heavy_loop.py --build DIR` only builds the project (the headless runs of `kt-cli preview`).
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kt_live  # noqa: E402
from kt_live import Live, require_tools  # noqa: E402
from idle import Meter, wait_quiet  # noqa: E402
from readme_shots import fixture_sources  # noqa: E402

OWN_PROCESS = True
SEC = 90000

# event 2149 of the owner's project, values as he left them (curves aside)
ISP = {"kind": "camera_isp", "src_saturation": 1.24, "src_wb_r": 2.0, "src_wb_b": 1.39, "distortion": 0.12,
       "lateral_ca": 2.5, "vignetting": 1.5, "exposure_ev": -2.0, "analog_gain": 4.0, "sharpen_amount": 1.5}
MOSH = {"kind": "datamosh", "gop": 66, "qp": 0.0, "lines": "720", "block": "8", "search": 64, "before": 12,
        "mv_scale": 0.9, "mv_rotate": 90.0}
VHS = {"kind": "vhs_tape", "speed": "lp", "decks": "two", "heads": "four", "tbc": True, "tracking_um": 11.0,
       "dropouts_per_s": 451.0, "chroma_db": -24.0, "rec_level_db": 12.0}
LOOP_AT = 2.0
LOOP_LEN = 0.7


def heavy_loop(path):
    """The longest plain footage event takes the owner's chain; a 0.7 s loop 2 s into it."""
    with open(path) as f:
        doc = json.load(f)
    p = doc["project"]
    plain = [c for c in p["clips"] if c["kind"] == "video" and not c.get("fx") and c["duration"] >= 4 * SEC]
    clip = max(plain, key=lambda c: c["duration"])
    clip["fx"] = [dict(ISP), dict(MOSH), dict(VHS)]
    clip["fx_auto"] = [[], [{"param": "mv_rotate", "ease": True, "keys": [
        {"t": 0, "v": 90.0, "shape": {"s": "zigzag", "periods": 60.0, "amplitude": 360.0, "profile": "triangle"}},
        {"t": clip["duration"], "v": 90.0}]}], []]
    at = clip["start"] + int(LOOP_AT * SEC)
    end = at + int(LOOP_LEN * SEC)
    p["region"] = {"start": at, "end": end}
    p["loop_enabled"] = True
    p["ui"]["cursor"] = at
    with open(path, "w") as f:
        json.dump(doc, f)
    return clip["id"], at, end


def build(out):
    sources = fixture_sources(os.path.join(out, "sources"))
    proj = os.path.join(out, "heavy-loop.kt")
    subprocess.run([kt_live.KT_CLI, "showcase", proj, "--sources", sources, "--heavy", "--force"],
                   check=True, capture_output=True)
    return (proj,) + heavy_loop(proj)


def trace_answer(out, question):
    """The run's own trace (its data directory hangs off the scenario's XDG_DATA_HOME)."""
    r = subprocess.run([kt_live.KT_CLI, "trace", os.path.join(out, "xdg", "kinetren", "trace"), question],
                       capture_output=True, text=True)
    return r.stdout or r.stderr


def loading(app):
    """The "Loading" scrim is drawn (the preview publishes its rectangle while it is)."""
    return any(f["param"] == "preview.loading" for f in app.geom()["fields"])


def main(out):
    require_tools()
    proj, clip, at, end = build(out)
    with Live("heavy_loop", out, project=proj, own_process=True) as app:
        meter = Meter(app)
        # paused on the loop's start: the chain's history comes in, then the pause is quiet
        app.expect(app.until(lambda: not loading(app), timeout=120.0),
                   "the paused frame under the heavy chain never came")
        wait_quiet(meter, "after opening")
        app.step("open", f"event {clip} carries Camera ISP → Datamosh → VHS Tape; paused on a 0.7 s loop, the frame is shown")

        app.key("space")
        app.until(lambda: app.playing(), timeout=10.0)
        app.expect(app.playing(), "playback started (no audio device?)")
        app.step("playing", "the loop plays; its first laps compose the chain")

        def covered():
            w, _ = meter.over(1.0)
            return w.get("composite_replays", 0) > 0 and w.get("composites", 0) == w.get("composite_replays", 0)
        app.expect(app.until(covered, timeout=300.0), "the loop never came to play only kept composites")
        app.step("covered", "laps after the first show only kept composites; the strips over the loop are bright")

        work, cpu = meter.over(6.0)
        composed = work.get("composites", 0) - work.get("composite_replays", 0)
        print(f"   playing the covered loop: {work}, {cpu:.0f} % of a core")
        app.expect(composed <= 2, f"a covered loop composes nothing: {composed} composed")
        app.expect(work.get("audio_renders", 0) <= 2, f"no burst runs the effects: {work}")
        app.expect(work.get("decoded", 0) == 0, f"nothing is decoded: {work}")

        # paused on a frame inside the loop: it is a kept moment, shown at once
        app.key("space")
        app.until(lambda: not app.playing(), timeout=10.0)
        wait_quiet(meter, "after pausing")
        app.expect(not loading(app), "the paused frame of the covered loop is shown, no Loading")
        app.step("paused", "paused inside the covered loop: the frame is there, no Loading")
    print(trace_answer(out, "loop"))


if __name__ == "__main__":
    if len(sys.argv) > 2 and sys.argv[1] == "--build":
        os.makedirs(sys.argv[2], exist_ok=True)
        print(*build(sys.argv[2]))
    else:
        sys.exit(main(sys.argv[1]))

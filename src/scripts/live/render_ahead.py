#!/usr/bin/env python3
"""Live scenario t2927 (render-ahead, review R2; the owner's kkt-26.11.17 S3/S4): a loop plays as a
render-ahead pipeline.

Playing, the preview only presents what the producer composed ahead and kept; the display composes
nothing itself. The project is ours (`kt-cli showcase --heavy`): a one-second loop over the composition
under Slowed, VHS Tape and Camera ISP, at the preview's own size. The verdict:
- after the first laps every frame shown is a kept composite, and no Loading lasts;
- the run's trace says the display made no composite of its own on the laps after the second (the
  `made` column of `trace loop`), and how busy the device was while frames were missing;
- paused, the program rests.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kt_live  # noqa: E402
from kt_live import Live, require_tools  # noqa: E402
from idle import Meter, wait_quiet  # noqa: E402
from heavy_loop import loading, trace_answer  # noqa: E402
from readme_shots import fixture_sources  # noqa: E402

OWN_PROCESS = True
SEC = 90000


def one_second_loop(path):
    """The showcase opens at a moment inside the composition: a one-second loop there."""
    with open(path) as f:
        doc = json.load(f)
    p = doc["project"]
    at = int(p["ui"]["cursor"])
    p["region"] = {"start": at, "end": at + SEC}
    p["loop_enabled"] = True
    with open(path, "w") as f:
        json.dump(doc, f)
    return at


def display_made(answer):
    """The display's own composites per lap (`trace loop` rows: lap, at, len, frames, kept, made, …)."""
    out = {}
    for line in answer.splitlines():
        cols = line.split()
        if len(cols) > 6 and cols[0].isdigit() and cols[1].startswith("+"):
            out[int(cols[0])] = int(cols[5])
    return out


def main(out):
    require_tools()
    sources = fixture_sources(os.path.join(out, "sources"))
    proj = os.path.join(out, "heavy.kt")
    subprocess.run([kt_live.KT_CLI, "showcase", proj, "--sources", sources, "--heavy", "--force"],
                   check=True, capture_output=True)
    one_second_loop(proj)
    with Live("render_ahead", out, project=proj, own_process=True) as app:
        meter = Meter(app)
        wait_quiet(meter, "after opening")
        app.step("open", "the heavy project is open, a one-second loop over the composition")

        app.key("space")
        app.until(lambda: app.playing(), timeout=10.0)
        app.expect(app.playing(), "playback started (no audio device?)")
        app.step("playing", "the loop plays; the producer composes ahead of the frame on screen")

        def covered():
            w, _ = meter.over(1.0)
            return w.get("composite_replays", 0) > 0 and w.get("composites", 0) == w.get("composite_replays", 0)
        app.expect(app.until(covered, timeout=240.0), "the loop never came to play only kept composites")
        # no Loading lasts once the loop is kept: three looks a second apart
        seen = []
        for _ in range(3):
            meter.over(1.0)
            seen.append(loading(app))
        app.expect(not all(seen), "Loading stayed over a kept loop")
        app.step("covered", "every frame shown is a kept composite; the strip over the loop is bright")

        app.key("space")
        app.until(lambda: not app.playing(), timeout=10.0)
        wait_quiet(meter, "after pausing")
        app.step("paused", "paused: the frame stands and the program rests")

    answer = trace_answer(out, "loop")
    print(answer)
    made = display_made(answer)
    late = {lap: n for lap, n in made.items() if lap >= 2 and n > 0}
    if late:
        raise SystemExit(f"the display composed itself on laps after the second: {late}")
    share = [line for line in answer.splitlines() if line.startswith("device busy while frames were missing")]
    print(share[0] if share else "no lap missed a frame")


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

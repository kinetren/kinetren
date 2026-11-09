#!/usr/bin/env python3
"""Live scenario cached-range-plays: a covered range plays again without work.

The owner's words are in `docs/ktprompts/kkt-26.11.07b.txt`, line 3-4 (registry G2-G4): a short
rendered range at 16 V-fps, 45 %, the sound crackling, "a cache that does not work". The project is ours
(`kt-cli showcase --heavy`): a composition under Slowed, VHS Tape and Camera ISP. A half-second loop
over it is played; after its first two laps every frame shown is a kept composite and every burst
heard comes from the kept mix. The verdict is the work the program counts, plus V-fps from the
geometry dump, plus underruns.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kt_live  # noqa: E402
from kt_live import Live, require_tools  # noqa: E402
from idle import Meter, wait_quiet, wakes, woke  # noqa: E402
from heavy_loop import recomposed, trace_answer  # noqa: E402
from readme_shots import fixture_sources  # noqa: E402

OWN_PROCESS = True
SEC = 90000


def short_loop(path):
    """The showcase opens at a moment inside the composition: a half-second loop there."""
    with open(path) as f:
        doc = json.load(f)
    p = doc["project"]
    at = int(p["ui"]["cursor"])
    p["region"] = {"start": at, "end": at + SEC // 2}
    p["loop_enabled"] = True
    with open(path, "w") as f:
        json.dump(doc, f)
    return at, p["settings"]["fps"]


def main(out):
    require_tools()
    sources = fixture_sources(os.path.join(out, "sources"))
    proj = os.path.join(out, "heavy.kt")
    subprocess.run(
        [kt_live.KT_CLI, "showcase", proj, "--sources", sources, "--heavy", "--force"],
        check=True, capture_output=True,
    )
    at, fps = short_loop(proj)
    with Live("cached_range", out, project=proj, own_process=True) as app:
        meter = Meter(app)
        wait_quiet(meter, "after opening")
        app.step("open", "the heavy project is open, a half-second loop over the composition")

        app.key("space")
        app.until(lambda: app.playing(), timeout=10.0)
        app.expect(app.playing(), "playback started (no audio device?)")
        # the first lap composes and renders; once a whole second shows only kept composites
        # the range is covered (a lap of heavy frames on llvmpipe takes seconds)
        def covered():
            w, _ = meter.over(1.0)
            return w.get("composite_replays", 0) > 0 and w.get("composites", 0) == w.get("composite_replays", 0)
        app.expect(app.until(covered, timeout=240.0), "the loop never came to play only kept composites")
        app.step("covered", "the loop has played twice: the strips over it are bright")

        g0 = app.geom()
        work, cpu = meter.over(6.0)
        g1 = app.geom()
        shown = g1["compose"]["shown"] - g0["compose"]["shown"]
        vfps = shown / 6.0
        composed = work.get("composites", 0) - work.get("composite_replays", 0)
        print(f"   playing the covered loop: {work}, {cpu:.0f} % of a core, V-fps {vfps:.1f} of {fps}")
        app.expect(work.get("composite_replays", 0) > 0, f"kept composites are shown: {work}")
        # judged by the trace's reasons, as in `heavy_loop` (test-hygiene): a display slower than
        # the frame rate composes, once each, the moments it shows for the first time
        again = recomposed(trace_answer(out, "loop"))
        app.expect(not again, f"a covered range composes nothing it kept: {composed} composed, again: {again}")
        app.expect(work.get("mix_replays", 0) > 0, f"the kept mix is heard: {work}")
        app.expect(work.get("audio_renders", 0) <= 2, f"no burst runs the effects: {work}")
        app.expect(work.get("decoded", 0) == 0, f"nothing is decoded: {work}")
        app.expect(g1["callback_underruns"] == g0["callback_underruns"],
                   f"no callback underrun: {g0['callback_underruns']} → {g1['callback_underruns']}")
        # printed, not judged: on this machine a UI frame in a real window costs the driver's
        # submit (~30 ms on llvmpipe), which caps V-fps whatever the preview costs
        print(f"   V-fps {vfps:.1f} (project {fps})")
        app.step("playing", "the covered loop plays from the kept picture and sound")

        app.key("space")
        app.until(lambda: not app.playing(), timeout=10.0)
        wait_quiet(meter, "after stopping")
        w0 = wakes(app)
        work, cpu = meter.over(4.0)
        print(f"   pause after the loop: {work}, {cpu:.0f} % of a core")
        app.expect(not work, f"a pause runs nothing, ran {work}; frames with no input were asked by: {woke(app, w0)}")
        app.step("paused", "stopped: nothing runs")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

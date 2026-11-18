#!/usr/bin/env python3
"""Live scenario t2988 (render-cache-disk; the owner's kkt-26.11.17 S5 "Сам в фоне"): a slow loop is
prerendered to disk at rest, by itself, and plays from disk after a restart.

The project is ours (`kt-cli showcase --heavy`): a one-second loop over the composition under Slowed,
VHS Tape and Camera ISP, which the software rasterizer cannot compose in real time. The verdict:
- playing measures the loop's moments slow; paused with the hands off, the program writes them to its
  render cache by itself and then rests (the owner's one exception to "at rest costs nothing", bounded
  by what was measured);
- after a restart the loop plays from the disk: its run's trace counts moments restored from disk in the
  first lap.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kt_live  # noqa: E402
from kt_live import Live, require_tools  # noqa: E402
from idle import Meter, wait_quiet  # noqa: E402
from heavy_loop import trace_answer  # noqa: E402
from readme_shots import fixture_sources  # noqa: E402
from render_ahead import one_second_loop  # noqa: E402

OWN_PROCESS = True


def cache_files(out):
    """The render cache's files under the scenario's data directory."""
    root = os.path.join(out, "xdg", "kinetren", "render-cache")
    n = 0
    for _, _, files in os.walk(root):
        n += sum(1 for f in files if f.endswith(".ktf"))
    return n


def counted(answer, prefix):
    """The number after `prefix` in a `trace cache` line ("written 12 (…)" → 12)."""
    for line in answer.splitlines():
        if line.startswith(prefix):
            return int(line[len(prefix):].split()[0])
    return 0


def main(out):
    require_tools()
    sources = fixture_sources(os.path.join(out, "sources"))
    proj = os.path.join(out, "heavy.kt")
    subprocess.run([kt_live.KT_CLI, "showcase", proj, "--sources", sources, "--heavy", "--force"],
                   check=True, capture_output=True)
    one_second_loop(proj)
    with Live("render_cache", out, project=proj, own_process=True) as app:
        meter = Meter(app)
        wait_quiet(meter, "after opening")
        app.expect(cache_files(out) == 0, "the cache had files before anything was measured")
        app.step("open", "the heavy project is open, a one-second loop over the composition")

        app.key("space")
        app.until(lambda: app.playing(), timeout=10.0)
        app.expect(app.playing(), "playback started (no audio device?)")
        meter.over(8.0)
        app.step("playing", "the loop plays: its moments are measured slow")
        app.key("space")
        app.until(lambda: not app.playing(), timeout=10.0)
        # hands off: the program prerenders the slow moments by itself, then rests
        app.expect(app.until(lambda: cache_files(out) > 0, timeout=120.0), "nothing was written to the render cache at rest")
        app.step("prerendering", "paused, the mouse still: the program writes the slow moments to disk")
        wait_quiet(meter, "after the prerender")
        written = cache_files(out)
        app.step("prerendered", f"{written} moments on disk; the program rests")
    first = trace_answer(out, "cache")
    print(first)
    if counted(first, "written ") == 0 or counted(first, "prerendered heavy: ") == 0:
        raise SystemExit("the first run's trace records no prerendered heavy moment")

    with Live("render_cache_restart", out, project=proj, own_process=True) as app:
        meter = Meter(app)
        wait_quiet(meter, "after opening again")
        app.key("space")
        app.until(lambda: app.playing(), timeout=10.0)
        app.expect(app.playing(), "playback started again")
        meter.over(3.0)
        app.step("restart_playing", "after a restart the loop plays from the disk")
        app.key("space")
        app.until(lambda: not app.playing(), timeout=10.0)
        wait_quiet(meter, "after pausing the restart")
    second = trace_answer(out, "cache")
    print(second)
    restored = counted(second, "restored ")
    if restored == 0:
        raise SystemExit("after the restart nothing was restored from disk")
    lap0 = [line for line in second.splitlines() if line.strip().startswith("lap 0 ")]
    if lap0 and " 0 restored" in lap0[0]:
        raise SystemExit(f"the first lap after the restart restored nothing: {lap0[0]}")
    print(f"render cache: {written} moments written at rest, {restored} restored after the restart")


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

#!/usr/bin/env python3
"""Live scenario t2988 (render-cache-disk; the owner's kkt-26.11.17 S5: "by itself in the background"): a slow loop is
prerendered to disk at rest, by itself, and plays from disk after a restart.

The project is ours (`kt-cli showcase --heavy`): a one-second loop over the composition under Slowed,
VHS Tape and Camera ISP, which the software rasterizer cannot compose in real time. The verdict:
- the frame on screen at the opening is composed and measured slow, and the loop plays and is measured
  slow; with the hands off the program writes the slow run to its render cache by itself and then rests
  (the owner's one exception to "at rest costs nothing", bounded by what was measured);
- after a restart the loop plays from the disk: its run's trace counts moments restored from disk in the
  first lap, and over eight laps no more restores than moments (t3358: each is restored once).
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
    """The moments the render cache holds: the keys of its segments' manifests."""
    root = os.path.join(out, "xdg", "kinetren", "render-cache")
    n = 0
    for top, _, files in os.walk(root):
        for f in files:
            if f.endswith(".idx"):
                with open(os.path.join(top, f)) as m:
                    n += sum(1 for line in m if line.startswith("key "))
    return n


def counted(answer, prefix):
    """The number after `prefix` in a `trace cache` line ("written 12 (…)" → 12)."""
    for line in answer.splitlines():
        if line.startswith(prefix):
            return int(line[len(prefix):].split()[0])
    return 0


def main(out):
    require_tools()
    # the virtual screen has no hardware encoder, and the cache takes no processor codec on its own
    # (kkt-26.11.18b U4): the scenario forces the export's software H.264 to run the same path
    os.environ.setdefault("KT_RENDER_CACHE_ENCODER", "libx264")
    sources = fixture_sources(os.path.join(out, "sources"))
    proj = os.path.join(out, "heavy.kt")
    subprocess.run([kt_live.KT_CLI, "showcase", proj, "--sources", sources, "--heavy", "--force"],
                   check=True, capture_output=True)
    one_second_loop(proj)
    with Live("render_cache", out, project=proj, own_process=True) as app:
        meter = Meter(app)
        # the frame on screen is composed at the opening and measured slow: with the hands off, the
        # program writes it and its slow neighbours by itself, then rests
        wait_quiet(meter, "after opening and its prerender")
        # the program's own word that the prerender is done (found-defects 950): at load ~300 the
        # program got so little processor that its share read as quiet while the prerender still
        # composed, and the first segment was committed 5 s after the "quiet"
        app.until(lambda: app.geom().get("rc_settled") and cache_files(out) > 0, timeout=300)
        opened = cache_files(out)
        app.expect(opened > 0, "the program wrote slow moments to disk at rest after the opening")
        app.step("open", f"the heavy project is open; at rest the program wrote {opened} slow moments to disk")

        app.key("space")
        app.until(lambda: app.playing(), timeout=10.0)
        app.expect(app.playing(), "playback started (no audio device?)")
        meter.over(8.0)
        app.step("playing", "the loop plays")
        app.key("space")
        app.until(lambda: not app.playing(), timeout=10.0)
        wait_quiet(meter, "after the play and its prerender")
        app.until(lambda: app.geom().get("rc_settled"), timeout=300)
        written = cache_files(out)
        app.expect(written >= opened, "the render cache kept its files")
        app.step("prerendered", f"{written} moments on disk; the program rests")
        verdict = app.done()
    if verdict:
        return verdict
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
        # several laps of the one-second loop (t3358, found-defects 774): each moment is restored ONCE,
        # into the executor's kept memo, and later laps read it from there — the stand's preview is GL,
        # the bytes path, which read and decoded the store on every lap
        meter.over(8.0)
        app.step("restart_playing", "after a restart the loop plays from the disk")
        app.key("space")
        app.until(lambda: not app.playing(), timeout=10.0)
        wait_quiet(meter, "after pausing the restart")
        verdict = app.done()
    if verdict:
        return verdict
    second = trace_answer(out, "cache")
    print(second)
    restored = counted(second, "restored ")
    if restored == 0:
        raise SystemExit("after the restart nothing was restored from disk")
    lap0 = [line for line in second.splitlines() if line.strip().startswith("lap 0 ")]
    if lap0 and " 0 restored" in lap0[0]:
        raise SystemExit(f"the first lap after the restart restored nothing: {lap0[0]}")
    if restored > written:
        raise SystemExit(f"{restored} restores of {written} moments over the laps: a moment is read from disk "
                         f"on every lap (found-defects 774)")
    print(f"render cache: {written} moments written at rest, {restored} restored after the restart")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

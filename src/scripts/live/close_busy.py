#!/usr/bin/env python3
"""Live scenario t2954 (close-while-busy, kkt-26.11.18 T1): the window closes while the preview catches up.

The owner's words are in `docs/ktprompts/kkt-26.11.18.txt`, line 1: closing the program while a loop
was "catching up" did nothing, and he had to kill the process. The project is ours
(`kt-cli showcase --heavy`, the chain of `heavy_loop`: Camera ISP → Datamosh → VHS Tape on the longest
footage event, a 0.7 s loop 2 s into it).

Three closes, each in a process of its own, each in a busy state:
  1. paused on the loop just after opening, while the stream history walks ("Loading — catching up");
  2. playing the loop on its first laps, while the render-ahead producer and the history fill it;
  3. playing, closed by File → Quit instead of the cross.
The verdict: the process exits within `BOUND_S` of the close, and the run's trace names the close
(`shutdown` records: which worker the exit waited for and how long).
"""
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
import kt_live  # noqa: E402
from kt_live import Live, require_tools, poll_step  # noqa: E402
from heavy_loop import build, loading  # noqa: E402

OWN_PROCESS = True
# the close of a busy program: the window goes and the process ends within this. Generous for a
# software rasterizer in Xvfb under a full live run; the owner's process never ended at all
BOUND_S = 15.0


def exits_within(app, t0, bound):
    """Seconds from `t0` (the close) until the process ended, or None past `bound`."""
    while time.time() - t0 < bound:
        if not app.alive():
            return time.time() - t0
        poll_step(0.1)
    return None


def shutdown_records(out):
    """The `shutdown` answer of the run's trace."""
    r = subprocess.run([kt_live.KT_CLI, "trace", os.path.join(out, "xdg", "kinetren", "trace"), "shutdown"],
                       capture_output=True, text=True)
    return r.stdout or r.stderr


def close_case(out, proj, name, play, quit_menu=False):
    case = os.path.join(out, name)
    os.makedirs(case, exist_ok=True)
    with Live(f"close_busy_{name}", case, project=proj, own_process=True) as app:
        app.until(lambda: app.geom().get("clips"), timeout=60.0)
        if play:
            app.key("space")
            app.until(lambda: app.playing(), timeout=10.0)
            app.expect(app.playing(), "playback started (no audio device?)")
            # the first laps: the producer and the history are at work
            t0 = time.time()
            while time.time() - t0 < 3.0:
                poll_step(0.25)
            app.step(f"{name}-busy", "the loop plays its first laps: the preview catches up")
        else:
            busy = app.until(lambda: loading(app), timeout=30.0)
            app.step(f"{name}-busy", f"paused on the loop just opened: Loading — catching up ({busy})")
        if quit_menu:
            app.click_probe("menu.file")
            t0 = time.time()
            app.click_probe("file.quit")
        else:
            t0 = time.time()
            app.close_window()
        took = exits_within(app, t0, BOUND_S)
        app.expect(took is not None, f"{name}: the program closed within {BOUND_S:.0f} s while busy")
        print(f"   {name}: exited {took if took is None else round(took, 2)} s after the close")
        if took is None:
            app.shot(f"{name}-hung")
            return app.done()
        answer = shutdown_records(case)
        print(answer)
        app.expect("the program closes" in answer, f"{name}: the trace names the close: {answer[:400]}")
        app.expect("FORCED" not in answer, f"{name}: the exit ended by itself, not by its watcher")
        return app.done()


def main(out):
    require_tools()
    proj, _clip, _at, _end = build(out)
    rc = 0
    rc |= close_case(out, proj, "paused", play=False)
    rc |= close_case(out, proj, "playing", play=True)
    rc |= close_case(out, proj, "quit", play=True, quit_menu=True)
    return rc


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

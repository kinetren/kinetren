#!/usr/bin/env python3
"""Live scenario idle-cpu: the program at rest costs nothing, and this is measured.

The owner's words are in `docs/ktprompts/kkt-26.11.07.txt`, items E1-E4. The project is ours (`kt-cli showcase --heavy`): nine releases of
a composition with Slowed, VHS Tape and Camera ISP over each, and a dense edit under them. The
verdict is the WORK the program counts, not percent: this machine rasterizes in software, so a
frame looks expensive here and nowhere else. `<dump>.load.json` carries the counters, because at
rest no frame writes the geometry dump.
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

# The limit IS the check here: a project that has not gone quiet by then never will.
QUIET_LIMIT_S = 300.0
QUIET_S = 3.0
# what a pointer passing over static things must not cause at all
NOT_ON_HOVER = (
    "composites", "device_submits", "decode_requests", "decoded", "thumb_orders", "fx_thumbs",
    "reel_jobs", "prefetch_passes",
)


class Meter:
    def __init__(self, app):
        self.app = app
        self.path = app.geom_path + ".load.json"

    def read(self):
        deadline = time.time() + 10.0
        while time.time() < deadline:
            try:
                with open(self.path) as f:
                    return json.load(f)
            except (OSError, ValueError):
                kt_live._poll(0.05)
        raise Failure("the load watch wrote no counters")

    def over(self, seconds, act=None):
        """Work and processor over `seconds`, with `act` running."""
        a, t0 = self.read(), time.time()
        while time.time() - t0 < seconds:
            if act:
                act()
            else:
                kt_live._poll(0.05)
        # the watch writes four times a second: the last counts must be in
        kt_live._poll(0.3)
        b = self.read()
        dt = time.time() - t0
        work = {k: b["work"][k] - a["work"].get(k, 0) for k in b["work"]}
        ticks = sum(b["ticks"].values()) - sum(a["ticks"].values())
        return {k: v for k, v in work.items() if v}, ticks / dt


def wait_quiet(meter, step):
    """Everything the opening started is over, and nothing runs."""
    t0, last, since = time.time(), meter.read()["work"], time.time()
    while time.time() - t0 < QUIET_LIMIT_S:
        kt_live._poll(0.25)
        now = meter.read()["work"]
        if now != last:
            last, since = now, time.time()
        elif time.time() - since >= QUIET_S:
            print(f"   {step}: quiet after {time.time() - t0:.0f} s")
            return
    raise Failure(f"{step}: still working after {QUIET_LIMIT_S:.0f} s of a pause: {last}")


def sweep(app, a, b):
    """A pointer going to and fro between two points."""
    state = {"i": 0}

    def act():
        state["i"] += 1
        k = abs((state["i"] % 40) / 20.0 - 1.0)
        app._move(int(a[0] + (b[0] - a[0]) * k), int(a[1] + (b[1] - a[1]) * k))
        kt_live._poll(0.03)

    return act


def main(out):
    require_tools()
    sources = fixture_sources(os.path.join(out, "sources"))
    proj = os.path.join(out, "heavy.kt")
    subprocess.run(
        [kt_live.KT_CLI, "showcase", proj, "--sources", sources, "--heavy", "--force"],
        check=True, capture_output=True,
    )
    with Live("idle", out, project=proj, own_process=True) as app:
        meter = Meter(app)
        wait_quiet(meter, "after opening")
        app.step("open", "the project is open, nothing touched")

        work, cpu = meter.over(5.0)
        print(f"   pause: {work}, {cpu:.0f} % of a core")
        app.expect(not work, f"a pause runs nothing, ran {work}")

        g = app.geom()
        win = kt_live.geometry()
        cx, cy = app.preview_center()
        phases = [
            ("preview", (cx - 90, cy - 50), (cx + 90, cy + 50)),
            ("status bar", (win["x"] + 900, win["y"] + win["h"] - 6), (win["x"] + 1300, win["y"] + win["h"] - 6)),
        ]
        # the release of the composition under the playhead, as the project sets it
        x_reel = app.x_at(g["playhead"])
        y_reel = app.lane_y("video", 0, frac=0.6)
        # within one tiny event of this dense edit: crossing an edge is a change, and is paid
        phases.append(("composition", (x_reel - 2, y_reel), (x_reel + 2, y_reel)))
        for name, a, b in phases:
            app.step(f"hover-{name.replace(' ', '-')}", f"the pointer goes over the {name}")
            work, cpu = meter.over(3.0, sweep(app, a, b))
            frames = work.get("ui_frames", 0)
            print(f"   over the {name}: {work}, {cpu:.0f} % of a core")
            app.expect(frames > 0, f"over the {name}: no frame saw the pointer, so this proves nothing")
            bad = {k: v for k, v in work.items() if k in NOT_ON_HOVER}
            app.expect(not bad, f"over the {name}: a static hover caused {bad}")
            passes = work.get("event_passes", 0)
            app.expect(passes <= max(4, frames // 3),
                       f"over the {name}: {passes} event passes in {frames} frames")
            work, _ = meter.over(1.5)
            work, cpu = meter.over(2.0)
            app.expect(not work, f"after the {name}: rest again, ran {work}")

        # its effects: the chain of Slowed, VHS Tape and Camera ISP in the window
        app.click(x_reel, y_reel)
        app.right_click(x_reel, y_reel)
        app.click_probe("tl.fx", "item “Effects…” of the event")
        app.step("fx-open", "the effects window of the composition is open")
        wait_quiet(meter, "with the effects window open")
        work, cpu = meter.over(5.0)
        print(f"   effects window, pause: {work}, {cpu:.0f} % of a core")
        app.expect(not work, f"an open effects window runs nothing, ran {work}")
        r = app._probe_rect("win.fx")
        app.expect(r is not None, "the effects window published its rectangle")
        x0, y0 = win["x"] + int(r["x"]) + 40, win["y"] + int(r["y"]) + 60
        work, cpu = meter.over(3.0, sweep(app, (x0, y0), (x0 + 160, y0 + 100)))
        print(f"   over the effects window: {work}, {cpu:.0f} % of a core")
        bad = {k: v for k, v in work.items() if k in NOT_ON_HOVER}
        app.expect(not bad, f"over the effects window: a static hover caused {bad}")
        app.step("fx-hover", "the pointer went over the window, the program is at rest again")
        # the one click that hands the proof over: it writes the file, and it is on the machine
        app.key("Escape")
        app.settle()
        # a 1440-point window keeps the gauge under the toolbar's "more" button
        if not app.has_probe("tb.diag", tries=2):
            app.click_probe("tb.more", "the toolbar's more button")
        app.click_probe("tb.diag", "the Diagnostics button")
        app.expect(app.has_probe("diag.load_report"), "Diagnostics opened with the load section")
        app.click_probe("diag.load_report", "the load report button")
        report = os.path.join(out, "xdg", "kinetren", "load-report.txt")
        app.until(lambda: os.path.exists(report), timeout=10.0)
        app.expect(os.path.exists(report), "one click wrote the load report")
        text = open(report).read() if os.path.exists(report) else ""
        app.expect("devices:" in text and "the choice:" in text, "the report names the devices and the choice")
        app.step("report", "Diagnostics shows the load, and the report is on disk")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

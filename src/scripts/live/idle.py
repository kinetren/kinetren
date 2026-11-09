#!/usr/bin/env python3
"""Live scenario idle-cpu: the program at rest costs nothing, and this is measured.

The owner's words are in `docs/ktprompts/kkt-26.11.07.txt`, items E1-E4. The project is ours (`kt-cli showcase --heavy`): ten releases of
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
    "reel_jobs", "prefetch_passes", "model_edits", "project_swaps", "engine_builds",
)
# input-without-work: a thread holding this share of a core while no counter moves is work nobody
# counts (the owner's report: "at rest" at 196 %, prefetch and the audio render rebuilding the
# engine on every mouse move). Processor ticks, not counters, are the measure here.
ENGINE_THREADS = ("prefetch", "audio-render")
ENGINE_LIMIT_PERCENT = 5.0
UNCOUNTED_PERCENT = 10.0


def trace_bytes(out):
    """What the structured trace holds on disk (structured-telemetry): at rest it must not grow."""
    d = os.path.join(out, "xdg", "kinetren", "trace")
    try:
        return sum(os.path.getsize(os.path.join(d, f)) for f in os.listdir(d))
    except OSError:
        return 0


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
        # per thread group, percent of one core (100 ticks a second)
        self.threads = {
            k: (v - a["ticks"].get(k, 0)) / dt for k, v in b["ticks"].items() if v > a["ticks"].get(k, 0)
        }
        return {k: v for k, v in work.items() if v}, ticks / dt

    def engine_busy(self):
        """The engine threads' share of a core over the last `over`, those above the limit."""
        return {k: round(v) for k, v in self.threads.items() if k in ENGINE_THREADS and v > ENGINE_LIMIT_PERCENT}

    def uncounted(self, work):
        """A busy thread with every counter still: never rest."""
        if work:
            return {}
        return {k: round(v) for k, v in self.threads.items() if k != "load-watch" and v >= UNCOUNTED_PERCENT}


# the live bench's own threads: they answer the scenario, not the program's work
BENCH_THREADS = ("load-watch", "live-ctl", "geom-dump")
# what a thread at rest may take while the quiet is judged: below the "pause at 2 % of a core"
QUIET_PERCENT = 5.0


def wakes(app):
    """What asked for the frames that read no input so far: {"file:line": count} (the dump's
    `rest_wakes`, the bench only)."""
    out = {}
    for part in (app.geom().get("rest_wakes") or "").split(", "):
        if "×" in part:
            k, n = part.rsplit("×", 1)
            out[k] = int(n)
    return out


def woke(app, before):
    """The repaint causes of the frames with no input since `before` (a `wakes` reading)."""
    now = wakes(app)
    return {k: n - before.get(k, 0) for k, n in now.items() if n > before.get(k, 0)}


def program_ticks(sample):
    """The program's processor ticks so far, the bench's own threads left out."""
    return sum(v for k, v in sample["ticks"].items() if k not in BENCH_THREADS)


def wait_quiet(meter, step):
    """Everything the opening started is over, and nothing runs: no counter moved AND no thread of
    the program worked for QUIET_S (test-hygiene, found-defects 133/170). A job in flight moves its
    counter only when it ends: on a loaded machine a reel preview or a recheck ran past three still
    seconds, landed in the next window, and that window's "rest" saw its frames."""
    first = meter.read()
    t0, last, since, ticks0 = time.time(), first["work"], time.time(), program_ticks(first)
    while time.time() - t0 < QUIET_LIMIT_S:
        kt_live._poll(0.25)
        sample = meter.read()
        now = sample["work"]
        busy = (program_ticks(sample) - ticks0) / max(time.time() - since, 0.25)
        if now != last or busy > QUIET_PERCENT:
            last, since, ticks0 = now, time.time(), program_ticks(sample)
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
        # a screenshot's wait pokes the stand for a frame: that is the scenario's own work
        wait_quiet(meter, "after the screenshot")

        t0, w0 = trace_bytes(out), wakes(app)
        work, cpu = meter.over(5.0)
        grew = trace_bytes(out) - t0
        print(f"   pause: {work}, {cpu:.0f} % of a core, threads {meter.threads}, trace +{grew} bytes")
        app.expect(not work, f"a pause runs nothing, ran {work}; frames with no input were asked by: {woke(app, w0)}")
        app.expect(grew == 0, f"a pause records nothing in the trace: +{grew} bytes")
        app.expect(not meter.uncounted(work), f"a pause holds a thread no counter sees: {meter.uncounted(work)}")

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
            t0 = trace_bytes(out)
            work, cpu = meter.over(3.0, sweep(app, a, b))
            # on a loaded machine the program can stand a whole sweep in one frame (test-hygiene):
            # sweep on until frames have seen the pointer, as a person goes on moving it
            deadline = time.time() + kt_live.stretched(30.0)
            while not work.get("ui_frames", 0) and time.time() < deadline:
                work, cpu = meter.over(3.0, sweep(app, a, b))
            frames = work.get("ui_frames", 0)
            # printed: the load watch's busy seconds (the UI frames counted) go into the journal and
            # the trace once a second; no frame of the pointer is recorded
            print(f"   over the {name}: {work}, {cpu:.0f} % of a core, trace +{trace_bytes(out) - t0} bytes")
            app.expect(frames > 0, f"over the {name}: no frame saw the pointer, so this proves nothing")
            bad = {k: v for k, v in work.items() if k in NOT_ON_HOVER}
            app.expect(not bad, f"over the {name}: a static hover caused {bad}")
            # printed, not judged: a frame with no input after each hover frame still lays the
            # events out, about every second frame. The limit is hover-static-load's to set.
            print(f"   over the {name}: {work.get('event_passes', 0)} event passes in {frames} frames")
            # the moves still queued are read by the next frames: a frame drawn after the sweep has
            # read them all, or the "rest" below sees the backlog's frames (test-hygiene: 14 of 17)
            app.wait_frame(2, what=f"a frame after the sweep over the {name}", soft=True)
            wait_quiet(meter, f"after the {name}")
            w0 = wakes(app)
            work, cpu = meter.over(2.0)
            app.expect(not work, f"after the {name}: rest again, ran {work}; frames with no input were asked by: {woke(app, w0)}")

        # its effects: the chain of Slowed, VHS Tape and Camera ISP in the window
        app.click(x_reel, y_reel)
        app.right_click(x_reel, y_reel)
        app.click_probe("tl.fx", "item “Effects…” of the event")
        app.step("fx-open", "the effects window of the composition is open")
        wait_quiet(meter, "with the effects window open")
        t0 = trace_bytes(out)
        work, cpu = meter.over(5.0)
        grew = trace_bytes(out) - t0
        print(f"   effects window, pause: {work}, {cpu:.0f} % of a core, trace +{grew} bytes")
        app.expect(not work, f"an open effects window runs nothing, ran {work}")
        app.expect(grew == 0, f"an open effects window records nothing in the trace: +{grew} bytes")
        r = app._probe_rect("win.fx")
        app.expect(r is not None, "the effects window published its rectangle")
        x0, y0 = win["x"] + int(r["x"]) + 40, win["y"] + int(r["y"]) + 60
        work, cpu = meter.over(3.0, sweep(app, (x0, y0), (x0 + 160, y0 + 100)))
        print(f"   over the effects window: {work}, {cpu:.0f} % of a core")
        bad = {k: v for k, v in work.items() if k in NOT_ON_HOVER}
        app.expect(not bad, f"over the effects window: a static hover caused {bad}")
        app.step("fx-hover", "the pointer went over the window, the program is at rest again")

        # input-without-work (kkt-26.11.07b G5, G7): the VHS Tape device panel selected, a number
        # at the floor its f32 row cannot hold (-80 % is -0.800000012), the pointer moving. Each
        # move was an edit and a rebuild of the whole engine on the owner's project.
        deck = None
        for i in range(6):
            if not app.has_probe(f"fx.slot.{i}", tries=2):
                break
            app.click_probe(f"fx.slot.{i}", f"slot {i} of the chain")
            if app.has_probe("deck.standard.0", tries=3):
                deck = i
                break
        app.expect(deck is not None, "the chain has a VHS Tape slot whose deck face draws")
        if deck is not None:
            app.step("fx-deck", "VHS Tape is selected: the deck face and its grid")
            # the field lies under the fold of the window's scrolling body: a rectangle published
            # there is not on screen, and a drag at it lands on the timeline
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
            app.expect(r is not None, "the dark clip field scrolls into view, above the window's action row")
            if r is not None:
                fx = win["x"] + int(r["x"] + r["w"] / 2)
                fy = win["y"] + int(r["y"] + r["h"] / 2)
                edits = meter.read()["work"].get("model_edits", 0)
                # a real drag far to the left: the field stops at its floor
                app.drag(fx, fy, max(win["x"] + 5, fx - 420), fy, steps=8)
                app.until(lambda: meter.read()["work"].get("model_edits", 0) > edits, timeout=10.0)
                app.expect(meter.read()["work"].get("model_edits", 0) > edits, "the drag on the dark clip field was an edit")
                app.step("fx-floor", "the dark clip is dragged to its floor, -80 %")
                wait_quiet(meter, "after the drag")
                gx, gy = win["x"] + int(r["x"]) - 150, win["y"] + int(r["y"]) - 120
                work, cpu = meter.over(4.0, sweep(app, (gx, gy), (gx + 220, gy + 160)))
                print(f"   over the deck panel at the floor: {work}, {cpu:.0f} % of a core, threads {meter.threads}")
                bad = {k: v for k, v in work.items() if k in NOT_ON_HOVER}
                app.expect(work.get("ui_frames", 0) > 0, "over the deck panel: no frame saw the pointer")
                app.expect(not bad, f"over the deck panel: a static hover caused {bad}")
                app.expect(not meter.engine_busy(), f"over the deck panel: the engine threads worked {meter.engine_busy()} % of a core")
                app.step("fx-deck-hover", "the pointer went over the deck panel, the engine stayed asleep")
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

#!/usr/bin/env python3
"""The frame of live scenarios in the virtual screen (R.27).

The scenario gets a directory for artifacts and works like this:

    from kt_live import Live, project_with_clips

    with Live("handles", out) as app:
        app.drag(...)             # a real mouse via XTest
        app.key("ctrl+s")         # a real keyboard
        p = app.saved_project()   # the state AFTER the gesture, through the door of export
        app.expect(len(p.transitions) == 1, "overlap created the transition")
        app.shot("after-overlap") # screenshot into the artifacts directory

The main difference from harness tests: here it is the real binary, a real X and
a real wgpu. The main difference from a manual check: the scenario checks itself
and prints what exactly diverged.
"""
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import time

# scripts/live/kt_live.py → src/ two levels up
RUST = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
ROOT = os.path.dirname(RUST)
KT_APP = os.path.join(RUST, "target", "debug", "kinetren")

# WORKER STAND (R.68): the directory with `ctl`, `geom.json` and `window.json` of an
# already running program — it is set up by `scripts/live/worker.py`. Without the variable
# each scenario starts its own process, as before R.68.
BENCH = os.environ.get("KT_LIVE_BENCH")
KT_CLI = os.path.join(RUST, "target", "debug", "kt-cli")

# ui-probe.py — the ready-made mouse/keyboard driver (python3-xlib + XTest);
# a name with a hyphen is not imported by an ordinary import
_spec = importlib.util.spec_from_file_location(
    "ui_probe", os.path.join(RUST, "scripts", "ui-probe.py")
)
probe = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(probe)


class Failure(Exception):
    pass


class Clip:
    def __init__(self, d, start):
        sr = d.get("source_range") or {}
        self.name = d.get("name", "")
        self.start = start
        self.duration = _ticks(sr.get("duration"))
        self.source_in = _ticks(sr.get("start_time"))
        self.track = ""

    @property
    def end(self):
        return self.start + self.duration

    def mid(self):
        return self.start + self.duration // 2

    def __repr__(self):
        return (
            f"Clip({self.name!r}, {self.start}..{self.end}, in={self.source_in})"
        )


class Timeline:
    """The parsed exchange file: what the program really saved. Positions
    are counted by the order of the children of the track (Gap moves the cursor, Transition —
    does not), so the scenario can aim at the JOINT, not at a number from the head.

    R.70: the project on disk is its OWN `.kt` format. Scenarios read it through
    the door of export (`kt-cli export-otio`): so `doc` remains the same OTIO with the
    envelope `metadata.kt` as before, and each parse also
    runs the real export. `otio_path` — where it landed."""

    def __init__(self, path):
        self.otio_path = path
        with open(path) as f:
            self.doc = json.load(f)
        if "kinetren" in self.doc:
            self.otio_path = export_project(path)
            with open(self.otio_path) as f:
                self.doc = json.load(f)
        self.clips, self.transitions, self.gaps = [], [], []
        for track in self.doc.get("tracks", {}).get("children", []):
            if not str(track.get("OTIO_SCHEMA", "")).startswith("Track"):
                continue
            cursor = 0
            for item in track.get("children", []):
                tag = str(item.get("OTIO_SCHEMA", ""))
                dur = _ticks((item.get("source_range") or {}).get("duration"))
                if tag.startswith("Clip"):
                    c = Clip(item, cursor)
                    c.track = track.get("kind", "")
                    self.clips.append(c)
                    cursor += dur
                elif tag.startswith("Gap"):
                    self.gaps.append((cursor, dur))
                    cursor += dur
                elif tag.startswith("Transition"):
                    self.transitions.append(
                        {
                            "at": cursor,
                            "in": _ticks(item.get("in_offset")),
                            "out": _ticks(item.get("out_offset")),
                        }
                    )

    def clip(self, name):
        return next(c for c in self.clips if c.name == name)

    def joint_after(self, name):
        """The joint after the clip — the place where the crossfade lives."""
        c = self.clip(name)
        return c.start + c.duration


def _ticks(t):
    """RationalTime → ticks (our rate 90000; a foreign one is quantized the same way as in kt-otio)."""
    if not isinstance(t, dict):
        return 0
    rate = t.get("rate") or 90000.0
    return round(t.get("value", 0.0) * 90000.0 / rate)


# HOW MANY SECONDS THE SCENARIO SLEPT (R.69). The reason in words turned out to be a weak measure:
# all 265 s of sleep in the frame were “lawful” — each pause carried an
# explanation longer than forty characters. So sleep is now also COUNTED:
# the guard holds a static limit of declared seconds, and this number is the
# actual one, for the report and the next measurement. It cannot be a verdict:
# under four workers it is not sleep that grows but waiting, and a threshold would catch
# load instead of regression.
_SLEPT = [0.0]
# the count starts from the frame import, not from `with`: a scenario sometimes has two
# `with Live(...)` blocks in a row (`stills`), and the time of the second one is not the scenario’s time
_STARTED = time.time()


def slept():
    """The actual fixed sleep of the scenario: the framework PLUS driver X."""
    return _SLEPT[0] + probe.SLEPT[0]


def pause(seconds, reason):
    """A pause WITH A REASON — the only legitimate way to sleep (R.68).

    Measurement R.68: 96 % of the live run time was unconditional sleep
    (`nestparam` — 130 s of 136 in 591 calls), because the framework waited “by eye”
    for a software-rendered frame. Now they wait for the FRAME (`Live.wait_frame`), and the
    pause remains only where there is nothing to wait for: the time threshold of a gesture in
    egui, the autosave clock, the drag step. The reason — in words and not shorter than forty
    characters; the guard `scripts/live-sleeps.py` fails on a pause without a reason,
    because the rule in prose did not hold here.

    R.69: each pause is also ACCUMULATED (`slept()`) — see `_SLEPT`.
    """
    if len(reason) < 40:
        raise Failure(f"pause without a clear reason: {reason!r}")
    _SLEPT[0] += seconds
    time.sleep(seconds)


# SPREAD OF DOUBLE-CLICK EVENTS (R.34, confirmed R.69).
#
# The egui double click has a TEMPORARY semantics, and the window is narrow on two sides:
# four events that drift into one frame count as one click, and
# those spaced more than 0.3 s apart count as two single clicks. The earlier 0.09 s between
# press and release gave 0.22 s from the first press to the second and
# fell into this window; in R.69 they disappeared together with the driver’s pauses, and
# the double click began to fail under load — `nestparam` red twice,
# `formula` on the second try. There is nothing to wait for here: the TIME itself is checked.

def _poll(seconds=0.02):
    """A waiting poll step: not a pause, but the frequency of the question “already?”."""
    time.sleep(seconds)


def poll_step(seconds=0.25):
    """A POLL STEP IN A SCENARIO LOOP — not a pause “just in case”.

    A scenario that waits for a widget, a change of picture or a file to appear
    asks the program in a loop; between the questions a gap is needed, otherwise the loop
    runs through its attempts in milliseconds and gives up without waiting
    for anything (so after R.68 several scenarios “went silent”: each has
    its own polling loop with its own step). This is a legitimate sleep: there is nothing to wait for here —
    the APPEARANCE itself is checked.
    """
    time.sleep(seconds)


class Live:
    """A launched application with a project. All inside `with`.

    By default the scenario takes the STAND of a worker (R.68): the program is already
    running, and the project opens in it right after the previous one — instead of
    a cold start of 3-5 s for each of the fifty scenarios. A scenario that checks the START
    ITSELF (recovery after a crash, the question of unsaved
    changes, the lock, import of a foreign version, autoplay from the first frame),
    declares `own_process=True` and gets its own process: this is the decision of
    the scenario, not a setting of the run.
    """

    def __init__(self, name, out_dir, project=None, autoplay=False, own_process=False, raw=False):
        self.name = name
        self.out = out_dir
        # R.70: the scenario builds an EXCHANGE FILE (it has ready helpers
        # and the kt envelope), while the program opens a PROJECT FILE `.kt` — which
        # is made by the import door on entry (`import_project`). `raw=True` —
        # hand the program the file as it is: this is how the refusal of “Open” itself is checked.
        self.source = project
        self.raw = raw
        self.project = project
        self.autoplay = autoplay
        self.proc = None
        self.problems = []
        self._reported = False
        self._shot_no = 0
        self._raised = False
        # a direct read of a point is accepted only after reconciliation with the reference
        self._direct_px = None
        # OBSERVED frame step: under four workers the software render goes
        # noticeably slower than alone, and all waits must stretch
        # together with it. Fixed thresholds are exactly the
        # blindness because of which “all went red at once” (§468).
        self._dt = 0.2
        # the stand can handle neither autoplay (KT_AUTOPLAY is read at process start),
        # nor “a program without a project” (an unnamed document — this is
        # exactly what the start scenarios check)
        self.own = bool(own_process or autoplay or project is None or not BENCH)

    def __enter__(self):
        os.makedirs(self.out, exist_ok=True)
        if self.source and self.source.endswith(".otio") and not self.raw:
            self.project = import_project(self.source)
        if not self.own:
            return self._attach_bench()
        env = dict(os.environ)
        # software render: there is no graphics card in the virtual screen
        env.setdefault("WGPU_BACKEND", "gl")
        env.setdefault("LIBGL_ALWAYS_SOFTWARE", "1")
        # the program itself reports the timeline geometry — we aim exactly,
        # not “roughly at the bottom third” (R.27)
        self.geom_path = os.path.join(self.out, "geom.json")
        env["KT_GEOM_DUMP"] = self.geom_path
        # its own channel of the stand even for “its own process” (R.68): it removes the dump
        # throttling, and the wait for a frame works the same everywhere
        self.ctl_path = os.path.join(self.out, "ctl")
        env["KT_LIVE_CTL"] = self.ctl_path
        # a process of its own has its own storage (R.68): otherwise the scenario sees
        # the workplace of the previous one. The run of `nestpreview` was green
        # alone and red in the full run for exactly this reason.
        store = os.path.join(self.out, "store")
        os.makedirs(store, exist_ok=True)
        env["KT_USER_STORE_DIR"] = store
        # and its OWN XDG_DATA_HOME: the unnamed recovery, the journal and the project locks live
        # by it — without it the scenario finds someone else’s
        # unsaved work in the developer’s home directory
        env["XDG_DATA_HOME"] = os.path.join(self.out, "xdg")
        if self.autoplay:
            env["KT_AUTOPLAY"] = "1"
        log = open(os.path.join(self.out, "kt-app.log"), "w")
        args = [KT_APP] + ([self.project] if self.project else [])
        self.proc = subprocess.Popen(args, stdout=log, stderr=log, env=env)
        try:
            self._wait_window()
        except BaseException:
            # __exit__ on a fall inside __enter__ is NOT called: the program would
            # stay alive and eat the processor, poisoning all the following
            # scenarios (R.34: a forgotten kt-app at 500% CPU turned seven
            # green scenarios red — a “flake” that never existed)
            self.__exit__(None, None, None)
            raise
        return self

    def _attach_bench(self):
        """Take the STAND of a worker: open a project of its own in the already running program.

        Cleanliness is not promised, but it comes by design: the `open` command
        inside the program runs the same code as `File → Open`, that is,
        the state is built from scratch. But it is checked anyway BEFORE the first
        gesture — otherwise a state leak between scenarios would give exactly the
        class “green, because they looked in the wrong place”, from which Kinetren has already
        suffered.
        """
        self.geom_path = os.path.join(BENCH, "geom.json")
        self.ctl_path = os.path.join(BENCH, "ctl")
        # THE ENVIRONMENT THAT IS READ AT PROCESS START cannot be handed to the stand:
        # the program is already running. Silently ignoring it is not allowed —
        # the scenario would check not what it thinks; so the mismatch is
        # red and names what to do (R.68).
        for var, bench_value in (("KT_AUTOSAVE_SECS", "600"), ("KT_FILE_PICK", None)):
            got = os.environ.get(var)
            if got is not None and got != bench_value:
                raise Failure(
                    f"the scenario sets {var}={got!r}, but the stand is started with "
                    f"{bench_value!r}: such a scenario needs its own process — "
                    "declare `OWN_PROCESS = True` in its file"
                )
        # a storage per SCENARIO, not per worker: the workplace,
        # presets and the journal of the previous scenario have no right to affect the
        # verdict of the next one (`nestpreview`: green alone, red in the
        # full run). A scenario that ITSELF prepared the storage (a preset library)
        # hands over its own to the stand.
        store = os.environ.get("KT_USER_STORE_DIR") or os.path.join(self.out, "store")
        os.makedirs(store, exist_ok=True)
        os.makedirs(store, exist_ok=True)
        self._ctl("store", store)
        seq = self._ctl("open", os.path.abspath(self.project))
        self._wait_ctl(seq)
        self.wait_frame(2, what="the first frame of the new project in the stand")
        self.wait_loaded()
        g = self.geom()
        got = os.path.realpath(g.get("project", ""))
        self.expect(
            got == os.path.realpath(self.project),
            f"the stand opened the project of the scenario: {os.path.basename(got)}",
        )
        self.expect(
            int(g.get("windows", 0)) == 0,
            f"the program is clean: open windows {g.get('windows')}",
        )
        # the window of the previous scenario could have stayed low or shifted
        # (`resize` checks the form contract, R.66): we return EXACTLY the
        # geometry of the X window with which the stand started.
        want = json.load(open(os.path.join(BENCH, "window.json")))
        now = geometry()
        if any(abs(now[k] - want[k]) > 4 for k in ("x", "y", "w", "h")):
            win = probe.window()
            win.configure(x=want["x"], y=want["y"], width=want["w"], height=want["h"])
            probe.d.sync()
            self._raised = False
            self.wait_frame(3, what="relayout after the window geometry is restored", soft=True)
            self.settle()
        return self

    def wait_loaded(self, timeout=30.0):
        """Wait until the program SHOWS the whole project.

        “Frame drawn” is not yet “project on screen”: the bin fills up
        asynchronously, and while it grows, the timeline area MOVES. A scenario
        that computed the coordinates from such a dump aims too far left — so
        a double click on an event hit the track label and opened the rename
        (the live `curves` in the stand). We wait until the dump
        has as many events as the file, and until the picture
        settles.
        """
        want = len(Timeline(self.project).clips)
        deadline = time.time() + timeout
        seen = None
        stable_since = None
        while time.time() < deadline:
            g = self.geom()
            now = (
                len(g.get("clips", [])),
                round(g.get("view_start", 0.0), 1),
                round(g.get("ticks_per_px", 0.0), 3),
                round(g["timeline"]["left"], 1),
                round(g["timeline"]["top"], 1),
                len(g.get("rows", [])),
            )
            if now[0] >= want and now == seen:
                # THE VIEW MUST SETTLE, not merely “the frame is drawn”: while the
                # bin fills, the timeline zone and the zoom MOVE, and the coordinate
                # computed from such a dump hits the track label —
                # a double click on the event opened the rename of V1
                # (a live `curves` in the stand).
                if stable_since and time.time() - stable_since >= 0.4:
                    break
                stable_since = stable_since or time.time()
            else:
                seen, stable_since = now, None
            _poll(0.05)
        self.settle()
        return True

    def _ctl(self, cmd, arg=""):
        """Command to the stand; returns its number."""
        seq = int(time.time() * 1000)
        with open(self.ctl_path, "w") as f:
            f.write(f"{seq} {cmd} {arg}\n")
        return seq

    def _wait_ctl(self, seq, timeout=30.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                with open(self.geom_path) as f:
                    if int(json.load(f).get("bench_done", 0)) >= seq:
                        return
            except (OSError, ValueError):
                pass
            _poll(0.05)
        raise Failure(f"the stand did not execute the command {seq} — is the program alive? {self.out}")

    def _report_sleep(self):
        """SLEEP COST — INTO THE REPORT (R.69).

        The number is factual and is not a verdict: under four workers
        it is not the sleep that grows but the waiting, and a threshold on it would catch the load instead of
        a regression. The verdict is held by the STATIC limit of the declared seconds
        (`scripts/live-sleeps.py`).

        It is written from `__exit__`, not from `done()`: four scenarios
        (`envclip`, `forms`, `recover`, `sidechain`) print the verdict themselves and
        do not call `done()` — their sleep would not get into the report.
        """
        took = time.time() - _STARTED
        print(f"   sleep: {slept():.1f} s of {took:.1f} s of the scenario")
        # INTO THE FILE, so that the run can sum the sleep of all scenarios: the cost table
        # (`times.txt`) is the input of the distribution by worker (`balance.py`), and
        # a third field in it would break the parsing.
        try:
            with open(os.path.join(self.out, "slept.txt"), "w") as f:
                f.write(f"{slept():.1f}\t{took:.1f}\n")
        except OSError:
            pass

    def __exit__(self, *exc):
        self._report_sleep()
        if self.own and self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        # THE VERDICT DOES NOT DEPEND ON THE SCENARIO AUTHOR’S MEMORY (R.67). `expect`
        # accumulates divergences, but the return code is given by `done()` — and seven of
        # the fifty scenarios never called it: `curvesrc` printed FAIL
        # and exited with ZERO, that is, red was counted as green in exactly the same way
        # as in §443 and §458. Now an unreported divergence brings the scenario down right here: forgetting
        # the verdict is no longer possible.
        if exc[0] is None and self.problems and not self._reported:
            raise Failure(
                "the scenario did not report discrepancies (no `return app.done()`): "
                + "; ".join(self.problems)
            )
        return False

    def _wait_window(self, timeout=40.0):
        """Wait for the window and the DRAWN frames.

        Before R.68 “frame drawn” was checked by a screenshot: xwd plus
        `convert` for each attempt, and on top another 0.5 s for safety — 3-5 s for
        each of the fifty scenarios. The reason for that check was right
        (R.33 item 228: a black window was compared with black and “passed”), but
        one must ask not for the picture but the program: it reports the NUMBER
        of the drawn frame itself (`bench.tick`, the geometry dump).
        """
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.proc.poll() is not None:
                raise Failure(f"kt-app crashed at startup, see {self.out}/kt-app.log")
            try:
                probe.window()
            except SystemExit:
                _poll(0.1)
                continue
            while time.time() < deadline:
                if self.proc.poll() is not None:
                    raise Failure(f"kt-app crashed at startup, see {self.out}/kt-app.log")
                if self._frame_now() >= 2:
                    # the program is still finishing the first frame (sources, thumbnails,
                    # layout): a gesture delivered at this moment goes off
                    # nowhere. We wait until the project is on screen in full.
                    if self.project:
                        self.wait_loaded()
                    else:
                        self.settle()
                    return
                _poll(0.05)
            raise Failure("the program drew not a single frame — the window stayed empty")
        raise Failure("the kt-app window did not appear")

    # ---- waiting for a FRAME, not for the clock (R.68) ---------------------------------

    def _counters(self):
        """(drawn frames, read input events) from the dump."""
        try:
            with open(self.geom_path) as f:
                g = json.load(f)
            return int(g.get("frame", 0)), int(g.get("input", 0))
        except (OSError, ValueError):
            return 0, 0

    def _frame_now(self):
        """Number of the drawn frame from the dump; 0 — there is no dump yet."""
        return self._counters()[0]

    def _input_now(self):
        return self._counters()[1]

    def seen(self, what="gesture"):
        """Return the mark of “how many input events the program has already read.”

        A frame is not enough: a frame could have STARTED before the event arrived from the
        X server, and “frame drawn” does not yet mean “gesture seen” — this is exactly why the
        live scenario `level` silently lost the drag. The mark is taken BEFORE the
        gesture, `_wait_gesture` waits for its growth.

        R.69: the mark is taken from a FRESH dump. The dump is written at 10 Hz, that is,
        what was read in the last 100 ms is not yet reflected in it — and the growth
        of the counter after the gesture could have been ALIEN, from the tail of the previous
        gesture. The wait then ended too early, and the next gesture
        went into a program that had not yet redrawn: the context menu
        did not open, the effects window did not appear. This was caught only under
        the load of four workers — by two reruns per run
        (`formula`, `nestparam`). Therefore the mark is PAIRED — (frame, input) — and
        the wait requires not only the growth of input but also a dump written
        certainly LATER than the mark (see `_after_gesture`)."""
        return self._counters()

    def _note_frame_gap(self, gap):
        """Remember the current frame number (smoothed, with a ceiling)."""
        if 0.0 < gap < 5.0:
            self._dt = max(0.05, min(2.0, 0.5 * self._dt + 0.5 * gap))

    def settle(self, timeout=None, quiet=None):
        """Wait until the program REACTS to the gesture.

        “Read the input” is not yet “finished”: on the release of a drag the model
        is corrected in a batch, and the frame with the new state comes next. Waiting
        for a fixed pause (0.6 s in the previous framework) is pointless — the program
        says itself when it has settled: frames stop coming, because
        a redraw happens ONLY on movement (invariant No. 12).
        """
        timeout = timeout if timeout is not None else max(10.0, 40.0 * self._dt)
        deadline = time.time() + timeout
        last, since = self._frame_now(), time.time()
        while time.time() < deadline:
            f = self._frame_now()
            if f != last:
                self._note_frame_gap(time.time() - since)
                last, since = f, time.time()
            else:
                # “quiet” means SEVERAL frame steps, not 0.12 s: under
                # load a frame takes a second, and a fixed threshold would
                # declare the program calm in the middle of a redraw
                need = quiet if quiet is not None else max(0.12, 2.5 * self._dt)
                if time.time() - since >= need:
                    return True
            _poll()
        return False

    def _wait_gesture(self, mark, timeout):
        """The program read EXACTLY THIS gesture (R.69).

        Two conditions are not enough one by one. “The input grew” deceives: the mark is taken from a
        dump that can be up to 100 ms old, and the growth may be alien — from the tail of the
        previous gesture. “Frame drawn” deceives too: a frame could
        have STARTED before the event arrived from the X server (lesson of R.68).

        Together they hold: we wait for a dump certainly written LATER than the mark (the frame number
        grew by two — that is, the dump was written by not the same redraw that gave the
        mark, nor the one following it), AND the growth of the number of events read.
        """
        frame0, input0 = mark
        deadline = time.time() + timeout
        while time.time() < deadline:
            frame, seen = self._counters()
            if seen > input0 and frame >= frame0 + 2:
                return True
            _poll()
        return False

    def wait_frame(self, n=1, timeout=None, what="frame after the gesture", soft=False):
        """Wait until the program draws another `n` frames.

        This replaces fixed pauses: a gesture always produces input, input
        produces a redraw, and it is exactly that which must be waited for. Under load the wait
        stretches together with the frame — what the pause “0.35 s” could not do, and
        this is where false reds grew from (§468).

        `soft=True` — where a frame may not come at all: the program
        redraws ONLY on movement (invariant No. 12), and a waiting
        “just in case” legitimately ends with nothing.
        """
        target = self._frame_now() + n
        # the FLOOR of the waits is generous (R.68): under workers a frame takes three times
        # longer, and the extra seconds are spent only where something really broke —
        # a red one is re-run alone anyway.
        timeout = timeout if timeout is not None else max(20.0, 30.0 * self._dt * n)
        started = time.time()
        deadline = started + timeout
        while time.time() < deadline:
            if self._frame_now() >= target:
                self._note_frame_gap((time.time() - started) / max(n, 1))
                return True
            _poll()
        if soft:
            return False
        raise Failure(f"the program did not draw a frame: {what}")

    # ---- geometry from the program ITSELF --------------------------------------

    def resize(self, w, h):
        """Change the size of the program window (R.66): the form contract is checked
        in a LOW window — where the content outgrows the screen."""
        win = probe.window()
        win.configure(width=int(w), height=int(h))
        probe.d.sync()
        self._raised = False
        # OBSERVED frame step: under four workers the software render goes
        # noticeably slower than alone, and all waits must stretch
        # together with it. Fixed thresholds are exactly the
        # blindness because of which “all went red at once” (§468).
        self._dt = 0.2
        # after a resize we wait for SEVERAL frames: the re-layout does not go in a single
        # pass (scrolls and groups recount their heights)
        self.wait_frame(3, what="relayout after changing the window size")

    def geom(self):
        """Fresh geometry dump: the track area, zoom, row positions."""
        deadline = time.time() + 10.0
        while time.time() < deadline:
            try:
                with open(self.geom_path) as f:
                    g = json.load(f)
                if g.get("rows"):
                    self._geom = g
                    return g
            except (OSError, ValueError):
                pass
            _poll(0.05)
        raise Failure("the program did not give the geometry (KT_GEOM_DUMP)")

    def x_at(self, tick):
        """Screen x for a timeline tick."""
        g = self.geom()
        win = geometry()
        return win["x"] + int(g["timeline"]["left"] + (tick - g["view_start"]) / g["ticks_per_px"])

    def lane_y(self, kind="video", lane=0, frac=0.5):
        """Screen y inside a track row; frac — the share of the height from the top.
        Color samples are better taken below the middle: the name plate lies at the top."""
        g = self.geom()
        win = geometry()
        row = next(r for r in g["rows"] if r["kind"] == kind and r["lane"] == lane)
        # the height of EACH row is its own: the service ones (morph/camera) are three times
        # lower than the clip tracks (R.35 item 1)
        return win["y"] + int(row["top"] + row.get("h", g["lane_h"]) * frac)

    def seconds(self, s):
        return round(s * 90000)

    def playhead(self):
        """Cursor position in ticks — from the program ITSELF, not from the picture."""
        return int(self.geom().get("playhead", 0))

    def playing(self):
        return bool(self.geom().get("playing", False))

    def wait_playhead(self, pred, what, timeout=8.0):
        """Waits until the cursor satisfies the condition; returns the history."""
        deadline = time.time() + timeout
        seen = []
        while time.time() < deadline:
            t = self.playhead()
            seen.append(t)
            if pred(t):
                return seen
            _poll(0.05)
        self.expect(False, f"{what} (the cursor went: {seen[-8:]})")
        return seen

    def ruler_y(self):
        """The time ruler above the tracks — a click on it places the cursor."""
        g = self.geom()
        win = geometry()
        first_row = min(r["top"] for r in g["rows"])
        return win["y"] + int((g["timeline"]["top"] + first_row) * 0.5)

    def preview_center(self):
        """Center of the preview window: above the timeline, at the center of the window."""
        g = self.geom()
        win = geometry()
        return (
            win["x"] + int(g["window"]["w"] * 0.5),
            win["y"] + int(g["timeline"]["top"] * 0.45),
        )

    def brightness_red(self, xy):
        r, g, b = self.pixel(*xy)
        return r > b + 40 and r > g + 40

    def brightness(self, xy):
        r, g, b = self.pixel(*xy)
        return (r + g + b) / 3.0

    # ---- actions ----------------------------------------------------------

    def _mods(self, names, pressed):
        """Hold/release modifiers around a mouse gesture: in egui they
        are read from the STATE of the frame’s input, not from the button event."""
        from Xlib import X, XK

        table = {"ctrl": "Control_L", "shift": "Shift_L", "alt": "Alt_L"}
        mark = self.seen()
        for n in names:
            code = probe.d.keysym_to_keycode(XK.string_to_keysym(table[n]))
            probe.xtest.fake_input(
                probe.d, X.KeyPress if pressed else X.KeyRelease, code
            )
        probe.d.sync()
        # the modifier needs its OWN frame: we wait until the program has read it
        self._after_gesture(mark, "modifier")

    def _after_gesture(self, mark, what):
        """Wait until the program READS the gesture (R.69).

        Replaces fixed pauses around gestures: the program reports how many
        input events it has read, and it is exactly that which must be waited for. Under load the wait
        stretches together with the program — what the pause “0.35 s” could not do, and
        this is where false reds grew from (§468).

        The deadline is SHORT and with a fallback, and this is not a loophole: egui legitimately does NOT produce
        an event for a move to the same point or for a key into a window without focus, and
        such a wait would never end. The fallback is a NAMED pause: it
        gets into the sleep budget and in front of the guard. A silent fallback would bring back
        exactly that blindness, for the removal of which the round was opened.
        """
        _t0 = time.time()
        if self._wait_gesture(mark, timeout=max(2.0, 12.0 * self._dt)):
            if os.environ.get("KT_LIVE_DEBUG_WAIT"):
                print(f"      [waited] {what}: {time.time() - _t0:.2f} s", file=sys.stderr)
            return True
        if os.environ.get("KT_LIVE_DEBUG_WAIT"):
            print(f"      [TIMEOUT] {what}: {time.time() - _t0:.2f} s", file=sys.stderr)
        pause(
            0.25,
            "the program did not read the gesture in time: an event that egui legitimately "
            "does not produce (a move to the same point, a key into a window without focus) — "
            "there is nothing to wait for here, the frame will not come",
        )
        return False

    def click(self, x, y, mods=()):
        # the window is raised on the FIRST gesture and when the window changes, not before
        # each one (R.69): the X window is one, the program’s modal forms are egui layers
        # inside it, and there is no point raising it between two clicks in a row.
        # The reason for the former unconditional raise (the previous scenario’s window
        # stayed on top, item 199) is closed by the stand: the process is one, and a scenario with
        # “its own process” gets a screen to itself.
        self.raise_once()
        if mods:
            self._mods(mods, True)
        mark = self.seen()
        probe.move(x, y, pause=0.0)
        probe.button(True, pause=0.0)
        probe.button(False, pause=0.0)
        self._after_gesture(mark, "click")
        if mods:
            self._mods(mods, False)

    def right_click(self, x, y):
        """RMB: the target of the context menu and the menu itself. A separate method, because
        `click` presses button 1, while the menu opens with the third."""
        self.raise_once()
        mark = self.seen()
        probe.move(x, y, pause=0.0)
        probe.button(True, 3, pause=0.0)
        probe.button(False, 3, pause=0.0)
        self._after_gesture(mark, "right click")

    def double_click(self, x, y):
        """Double click: in egui this is a TEMPORARY gesture (two clicks closer than 0.3 s),
        so between them one must neither raise the window nor sleep 0.35 s —
        two plain click() calls in a row add up to two single clicks.

        The gap between the clicks is SMALL and this is not a typo (R.34): in
        software rendering a frame takes ~0.2 s, and egui takes the time of a click from the frame
        in which it fell. A pause of 0.1 s becomes 0.3+ s for egui —
        and the double click falls apart into two single ones. Verified: 0.04 s
        opens the window in 5 runs out of 5, 0.10 s — in 0 of 5."""
        self.raise_once()
        # a frame AFTER the hover: the first click at the fresh position is otherwise lost
        # together with the input frame
        mark = self.seen()
        probe.move(x, y, pause=0.0)
        self._after_gesture(mark, "hover before the double click")
        for i in range(2):
            # A GAP between clicks is needed: four events in a row in the
            # software render land in ONE input frame, and egui counts
            # one click instead of two (the gesture worked every other time). 0.04 s is
            # noticeably less than the double-click threshold (0.3 s).
            if i:
                pause(0.04, "egui double-click time threshold: the gap between clicks is less than 0.3 s but more than one frame (verified 5 of 5)")
            probe.button(True, pause=0.0)
            pause(0.09, "egui double click is a TEMPORARY gesture: without spacing, all four events land in one input frame and count as one click (R.34 measurement: 0.04 s between clicks — 5 of 5, 0.10 s — 0 of 5)")
            probe.button(False, pause=0.0)
            pause(0.09, "egui double click is a TEMPORARY gesture: without spacing, all four events land in one input frame and count as one click (R.34 measurement: 0.04 s between clicks — 5 of 5, 0.10 s — 0 of 5)")
        mark = self.seen()
        probe.d.sync()
        self._after_gesture(mark, "double click")

    def drag(self, x0, y0, x1, y1, steps=6, mods=()):
        """Drag LIKE A HUMAN: hover → press → nudge a little → move
        in several steps → release. A jump straight to the target counts in egui
        as the start of the drag already at the end point, and the object does not move
        (rule from apps/kt-app/CLAUDE.md). The intermediate steps are also needed
        because the software renderer produces frames rarely: a frame must pass between positions.

        R.69: “a frame must pass” is exactly the wait for a FRAME, not for the clock;
        each step waits for the program to read the movement."""
        self.raise_once()
        if mods:
            self._mods(mods, True)
        mark = self.seen()
        probe.move(x0, y0, pause=0.0)
        self._after_gesture(mark, "hover before drag")
        mark = self.seen()
        probe.button(True, pause=0.0)
        self._after_gesture(mark, "drag press")
        # the press must survive a FRAME before the first shift: egui counts
        # a drag by the threshold AND by the frame
        self.wait_frame(1, what="frame after drag press", soft=True)
        step = 8 if x1 >= x0 else -8
        mark = self.seen()
        probe.move(x0 + step, y0, pause=0.0)
        self._after_gesture(mark, "first drag shift")
        for i in range(1, steps + 1):
            mark = self.seen()
            probe.move(
                x0 + step + (x1 - x0 - step) * i // steps,
                y0 + (y1 - y0) * i // steps,
                pause=0.0,
            )
            self._after_gesture(mark, f"drag step {i}")
        # the last position must get into a FRAME before the release: otherwise the gesture
        # “undershoots” by 10–20 % of the path (item 199, analysis in R.33)
        self.wait_frame(1, what="frame with last drag position", soft=True)
        mark = self.seen()
        probe.button(False, pause=0.0)
        self._after_gesture(mark, "drag release")
        # AFTER THE RELEASE — ONE MORE FRAME, NOT SILENCE (R.69).
        #
        # The gesture is applied as a BATCH in the frame that read the release,
        # and the next frame already carries the new state. The `settle()` that suggested itself
        # is not suitable here and cost 10 s on EVERY drag (measured:
        # it burned its whole timeout): the program legitimately redraws
        # after a gesture too — the highlight under the pointer, animations, preview — and
        # “the frames stopped coming” simply never happens. Silence is the answer to
        # the question “the picture has settled”, not to “the gesture has been applied”.
        self.wait_frame(2, what="frame with gesture applied", soft=True)
        if mods:
            self._mods(mods, False)

    def focus(self):
        """Give the window input focus: XTest keys go to the ACTIVE window, and without
        this Space/Ctrl+S goes nowhere (the mouse works without focus)."""
        from Xlib import X

        w = probe.window()
        try:
            w.set_input_focus(X.RevertToParent, X.CurrentTime)
            w.configure(stack_mode=X.Above)
            probe.d.sync()
        except Exception:
            pass
        self._raised = True
        # lifting a window is the work of the window manager, not of the program:
        # it has no input event of its own here, so there is nothing to wait for
        pause(
            0.2,
            "raising the window is done by the WINDOW MANAGER, and the program "
            "gets no input event of its own here: nothing to wait for, but without a gap the next gesture "
            "goes to a window that has not been raised yet",
        )

    def raise_once(self):
        """Raise the window, if this has not been done yet (R.68)."""
        if not self._raised:
            self.focus()

    def key(self, spec):
        """A key; for a chord, the MODIFIER goes in a SEPARATE FRAME (R.69).

        The modifier is read from the input STATE of the frame, not from the key event:
        if they arrive in one batch, egui sees “s” without Ctrl.
        That is how Ctrl+S silently stopped working: the gesture went through on screen, while
        the script read from disk a file WITHOUT the edit and blamed the gesture for it
        (an hour and a half of digging; this exact rule is already recorded for `_mods`
        for mouse gestures).
        """
        self.raise_once()
        *mods, name = spec.split("+")
        mods = [m.lower() for m in mods]
        if mods:
            self._mods(mods, True)
        mark = self.seen()
        probe.key(name, pause=0.0, after=0.0)
        self._after_gesture(mark, f"key {spec}")
        if mods:
            self._mods(mods, False)

    # X key names for characters that cannot be typed by a letter: `key()`
    # parses the string by “+”, so the signs must travel by name
    # XTest sends a KEY, not a character: a sign that lives on the keyboard
    # through Shift must travel with it (otherwise `*` arrives as an eight, and
    # `+` as an equals sign; caught by the live run of R.63)
    KEYSYM = {
        "+": "shift+plus",
        "*": "shift+asterisk",
        "(": "shift+parenleft",
        ")": "shift+parenright",
        "_": "shift+underscore",
        ":": "shift+colon",
        "-": "minus",
        "/": "slash",
        ".": "period",
        ",": "comma",
        " ": "space",
    }

    def type_text(self, text, gap=0.0):
        """Type the string IN FULL (R.69; decision of R.68, `design.md` item 2).

        egui reads input events per FRAME, and the whole string lands in one frame:
        instead of `len(text)` waits of 0.77 s each, there is a single one.

        A character typed WITH SHIFT (`*`, `+`, `(`) drops out of the batch
        and goes separately: the modifier is read from the input state of the frame,
        not from the key event, and in a shared batch `*` arrives as an eight (R.63),
        now also because holding the key needs its own frame
        (R.69, Ctrl+S analysis).

        `gap` stays in the label for a scenario that needs a gap between characters
        for a real reason (the field grows as you type and pushes its neighbours);
        then each character goes with its own wait.
        """
        self.raise_once()
        batch = []

        def flush():
            if not batch:
                return
            mark = self.seen()
            for spec in batch:
                probe.key(spec, pause=0.0, after=0.0)
            probe.d.sync()
            self._after_gesture(mark, f"string {''.join(batch)!r}")
            batch.clear()

        for ch in text:
            spec = self.KEYSYM.get(ch, ch)
            if "+" in spec:
                flush()
                self.key(spec)
            else:
                batch.append(spec)
                if gap:
                    flush()
                    pause(
                        gap,
                        "the scenario asks for a gap between characters: the field grows from "
                        "typing and pushes its neighbours, and the coordinates are taken before typing",
                    )
        flush()

    def close_window(self):
        """The window close button (the cross). The program intercepts it to
        ask about unsaved work (R.49, item 354)."""
        self.raise_once()
        mark = self.seen()
        probe.close_window()
        # the dialog does not come up in the same frame as the cross: we wait until the
        # program has read the window event and stops redrawing
        self._after_gesture(mark, "window close button")
        # the deadline is short for the same reason as for hovering: there may be no silence
        # at all, and the probe of the rising modal window does not exist yet
        self.settle(timeout=max(1.0, 6.0 * self._dt))

    def alive(self):
        if self.own:
            return self.proc is not None and self.proc.poll() is None
        # in the stand the process is not ours: liveness is seen by whether the
        # program draws frames
        return self.wait_frame(1, timeout=3.0, soft=True) or self._frame_now() > 0

    def hover(self, x, y, pause=0.0):
        """Hover the pointer and wait for the REACTION (R.69).

        Tooltips and submenus open not instantly, but the wait is not in hours:
        the program reads the movement and redraws. `pause` stays in the
        label for a scenario that needs a gap for a real reason."""
        self.raise_once()
        mark = self.seen()
        probe.move(x, y, pause=0.0)
        self._after_gesture(mark, "hover")
        # for HOVERING the consequence is the picture (highlight, hint, submenu), and
        # the scenario compares the SCREEN: we wait until the program stops
        # redrawing. Here `settle` is not a duplicate of the probe — there may be no probe at
        # all here (`analysis` measures the share of the changed screen).
        #
        # THE DEADLINE IS SHORT, and this is a measurement, not caution: silence does not come
        # always — the program legitimately redraws under the pointer —, and
        # `settle` with its default deadline burned 10 s on EVERY
        # hover (`analysis` — 54 s against 17 s).
        self.settle(timeout=max(1.0, 6.0 * self._dt))
        if pause:
            globals()["pause"](
                pause,
                "the scenario asks for a hover delay: the egui submenu opens "
                "by the hold time of the pointer, not by an event",
            )

    def has_probe(self, name, tries=12):
        """A widget with this name is shown right now.

        `tries` stays in the label for the calls of scenarios (negative checks have
        a deliberately small value), but it counts as a DEADLINE: the dump is written
        at 10 Hz, so “twelve frames” is a little over a second."""
        # FIRST A FRESH DUMP: probes ACCUMULATE between dump records (each
        # frame adds its rectangles, and the record takes them once every
        # 100 ms), so the very first dump after a gesture may still carry
        # what was there BEFORE it. A negative check (“frame is off”) on
        # such a dump is always falsely red — exactly what the 0.6 s pause
        # in the `geomctl` scenario was hiding.
        self.wait_frame(2, what="fresh probe dump", soft=True)
        deadline = time.time() + max(0.5 * tries, 1.0)
        while time.time() < deadline:
            if any(f["param"] == name for f in self.geom().get("fields", [])):
                return True
            self.wait_frame(1, what=f"frame while waiting for a widget {name}", soft=True)
        return False

    def wait_gone(self, name, timeout=None):
        """Wait until the widget `name` is NO LONGER in the dump (R.69).

        A negative check is subtler than a positive one: the probes JITTER
        between dump records, while egui collapses and popups are animated — as long as
        the animation runs, fields are still being drawn and honestly publish their rectangles.
        A fixed pause inside the gesture used to hide this, and `conform` flickered
        on “a collapsed group hid the fields”.

        We wait until a FRESH dump stops containing the name, rather than asking
        just once at random.
        """
        timeout = timeout if timeout is not None else max(5.0, 25.0 * self._dt)
        deadline = time.time() + timeout
        while time.time() < deadline:
            self.wait_frame(2, what=f"fresh dump without {name}", soft=True)
            if not any(f["param"] == name for f in self.geom().get("fields", [])):
                return True
        return False

    def live_clips(self):
        """LIVE model events (without saving). Needed where the unsaved-work guard
        is checked: Ctrl+S would check the save, not the guard."""
        return sorted((c["start"], c["dur"]) for c in self.geom().get("clips", []))

    def _probe_rect(self, name):
        """Widget rectangle, once the picture has SETTLED (R.68).

        The dump is written every frame, and the first frame with a popup catches it
        mid-animation: a click on such a rectangle landed NEXT TO the item.
        The old scaffold hid this with a fixed pause; instead we wait
        for the program to stop redrawing,
        and then read the rectangle again.
        """
        # A DEADLINE, NOT A NUMBER OF PASSES: the geometry dump is written 10 times a second,
        # and “twenty frames” turned into two seconds instead of the former five —
        # the probe gave up sooner than the program managed to open the menu.
        self.wait_frame(2, what="fresh probe dump", soft=True)
        deadline = time.time() + max(8.0, 40.0 * self._dt)
        while time.time() < deadline:
            f = next((f for f in self.geom().get("fields", []) if f["param"] == name), None)
            if f:
                return f
            self.wait_frame(1, what=f"frame while waiting for a widget {name}", soft=True)
        return None

    def double_click_probe(self, name, what=None):
        """Double click on a widget from the geometry dump."""
        f = self._probe_rect(name)
        if f:
            win = geometry()
            self.double_click(
                win["x"] + int(f["x"] + f["w"] / 2),
                win["y"] + int(f["y"] + f["h"] / 2),
            )
            return True
        self.expect(False, what or f"the program returned the rectangle “{name}”")
        return False

    def right_click_probe(self, name, what=None):
        """Right click on a widget from the geometry dump. Needed by the field labels:
        publishing a composition property lives exactly there (R.59, stage 3)."""
        f = self._probe_rect(name)
        if f:
            win = geometry()
            self.right_click(
                win["x"] + int(f["x"] + f["w"] / 2),
                win["y"] + int(f["y"] + f["h"] / 2),
            )
            return True
        self.expect(False, what or f"the program returned the rectangle “{name}”")
        return False

    def hover_probe(self, name, what=None):
        """Hover over a widget by its rectangle from the dump. An egui submenu opens
        on HOVER, and a click on it closes the whole menu."""
        f = self._probe_rect(name)
        if f:
            win = geometry()
            self.hover(
                win["x"] + int(f["x"] + f["w"] / 2),
                win["y"] + int(f["y"] + f["h"] / 2),
            )
            # the submenu opens by HOVERING — we wait until it arrives
            self.settle()
            return True
        self.expect(False, what or f"the program returned the rectangle “{name}”")
        return False

    def click_probe(self, name, what=None):
        """Click a widget whose rectangle the program returned in the geometry dump
        (`fields`). Modal egui windows are not searched in the accessibility tree,
        and the script must not guess offsets inside a window (R.27)."""
        f = self._probe_rect(name)
        if f:
            win = geometry()
            self.click(
                win["x"] + int(f["x"] + f["w"] / 2),
                win["y"] + int(f["y"] + f["h"] / 2),
            )
            return True
        self.expect(False, what or f"the program returned the rectangle “{name}”")
        return False

    def shot(self, tag):
        """SCREEN snapshot via xwd.

        `import -window root` (ImageMagick) on an eframe window returns a black
        frame: the GL window lives in a 32-bit visual, and XGetImage of a depth-24 root
        window does not see the child’s contents. The symptom is treacherous:
        the script does not crash, but compares black with black and “passes”.
        xwd reads the same root window correctly (R.33 item 228)."""
        self._shot_no += 1
        path = os.path.join(self.out, f"{self._shot_no:02d}-{tag}.png")
        xwd = subprocess.run(
            ["xwd", "-root", "-display", os.environ["DISPLAY"]],
            check=True,
            capture_output=True,
        ).stdout
        subprocess.run(["convert", "xwd:-", path], input=xwd, check=True)
        return path

    def wheel(self, x, y, ticks=3):
        """Mouse wheel over a point (ticks > 0 — down). XTest: buttons 4/5."""
        self.raise_once()
        mark = self.seen()
        probe.move(x, y, pause=0.0)
        btn = 5 if ticks > 0 else 4
        for _ in range(abs(ticks)):
            probe.button(True, btn, pause=0.0)
            probe.button(False, btn, pause=0.0)
        probe.d.sync()
        self._after_gesture(mark, f"wheel {ticks}")

    def scroll_probe_into_view(self, name, anchor, what=None, tries=12, below=None):
        """Scroll the wheel over the widget `anchor` until the rectangle `name` is in the visible part
        (bottom below the edge — scroll down; top above it — scroll up). The edge is `below`:
        a widget OUTSIDE the scroll area at the bottom of the window (an action button, R.66);
        without it, the bottom of the program window is used. An egui window with a scrolling
        body otherwise hides a block behind a fold, and the probe returns coordinates
        of the invisible one too: the click went to the timeline."""
        win = geometry()
        for _ in range(tries):
            f = {x["param"]: x for x in self.geom().get("fields", [])}
            r = f.get(name)
            a = f.get(anchor)
            if r is None or a is None:
                self.wait_frame(1, what=f"frame while waiting for widgets {name}/{anchor}", soft=True)
                continue
            h = self.geom()["window"]["h"]
            if below and below in f:
                h = min(h, f[below]["y"] - 2)
            if r["y"] >= 0 and r["y"] + r["h"] <= h:
                return True
            ticks = 3 if r["y"] + r["h"] > h else -3
            # the anchor could have slid past the edge — the wheel turns at a VISIBLE point
            ay = min(max(a["y"] + a["h"] / 2, 4), h - 4)
            self.wheel(win["x"] + int(a["x"] + a["w"] / 2), win["y"] + int(ay), ticks)
        self.expect(False, what or f"“{name}” could not scroll into the window")
        return False

    def step(self, name, what=None):
        """USER PATH STEP (R.67): a screenshot for each step of `PATH` from the spec,
        file `NN-step-<name>.png` in the scenario directory; `finish.sh`
        prints their list, the agent’s report carries them. The run is on our own
        fixtures, not on the owner’s projects (“item 1 but not on my projects”)."""
        path = self.shot(f"step-{name}")
        print(f"   step {self._shot_no:02d}: {what or name} → {os.path.basename(path)}")
        return path

    # READING A SCREEN POINT: the direct path and the REFERENCE (R.69).
    #
    # The direct path is the same XGetImage that `xwd` uses, only without a
    # file and without parsing a PNG: 0.04 ms against 84 ms, and reads per run
    # ≈341. But it cannot be taken on faith: the eframe window has a 32-bit visual,
    # and ImageMagick on the root window already gave BLACK where the picture
    # was (R.33 item 228) — the scenario then compared black with black and
    # “passed”. So the first read of the run goes by BOTH paths on one frame;
    # if they diverge — the whole run reads by the reference and SAYS so.
    PIXEL_TOL = 8

    def _pixel_direct(self, x, y):
        return probe.pixels(x, y)[0]

    def _pixel_shot(self, x, y):
        img = self.shot(f"px-{x}-{y}")
        out = subprocess.run(
            ["convert", img, "-format", f"%[pixel:p{{{x},{y}}}]", "info:-"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        nums = [
            int(v)
            for v in out.replace("(", ",").replace(")", ",").split(",")
            if v.strip().isdigit()
        ]
        os.unlink(img)
        self._shot_no -= 1
        return tuple(nums[:3])

    def _pixel_path(self):
        """Check the direct read path against the reference ONCE per run."""
        if self._direct_px is not None:
            return self._direct_px
        for _ in range(3):
            # the picture could change between reads — we try on a settled one
            self.wait_frame(1, what="frame before checking the point read", soft=True)
            x, y = self.preview_center()
            ref = self._pixel_shot(x, y)
            got = self._pixel_direct(x, y)
            if all(abs(a - b) <= self.PIXEL_TOL for a, b in zip(ref, got)):
                self._direct_px = True
                return True
        print(
            "   WARNING: the direct point read diverged from the reference (xwd) — "
            "the whole run reads by the reference",
            file=sys.stderr,
        )
        self._direct_px = False
        return False

    def pixel(self, x, y):
        """Color of the screen pixel (r, g, b) — “what a person sees”."""
        if self._pixel_path():
            return self._pixel_direct(x, y)
        return self._pixel_shot(x, y)

    def ink_box(self, xy, half=60, step=6):
        """The brightest pixel of the SQUARE around the point — one screen snapshot.

        Whether there is anything light at this place at all. A point
        `brightness` does not suit the letters of a title: a font size of 7 % of the frame gives a thin
        stroke, and hitting it with the center of the preview is a lottery, and a lottery
        is not a test (the rule “assert the consequence”).
        """
        x0, y0 = xy[0] - half, xy[1] - half
        n = 2 * half // step
        if self._pixel_path():
            # by a DIRECT READ (R.69): a 120×120 square is read with one request
            # to X, not by a snapshot of the whole screen plus three ImageMagick actions
            px = probe.pixels(x0, y0, 2 * half, 2 * half)
            return float(max(max(c) for c in px)) if px else 0.0
        img = self.shot(f"ink-{xy[0]}-{xy[1]}")
        out = subprocess.run(
            [
                "convert",
                img,
                "-crop",
                f"{2 * half}x{2 * half}+{x0}+{y0}",
                "+repage",
                "-resize",
                f"{n}x{n}!",
                "-format",
                "%[fx:maxima*255]",
                "info:-",
            ],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        os.unlink(img)
        self._shot_no -= 1
        try:
            return float(out.strip())
        except ValueError:
            raise Failure(f"could not read the brightness of the area: {out!r}")

    def pixel_settled(self, xy, differs_from=None, tries=20):
        """Color of the pixel AFTER the picture has settled.

        `pixel` reads the screen IMMEDIATELY, and after a seek the cursor moves
        before the decoded frame arrives: the preview legitimately HOLDS
        the last frame (invariant “a miss at the deadline → last frame”),
        and the read returns the picture of the PREVIOUS position. Under load
        this reproduces every time — so `loop` “saw” a blue clip
        to the left of the region, and the weak wait “until two reads match”
        simply settled on a stale frame.

        So we wait for a CHANGE: `differs_from` is the color that was on screen
        BEFORE the gesture. Once the change has come, we take one more match in a row
        so as not to catch an intermediate state. Without `differs_from`
        (the frame may not change at all) stability is enough.

        R.68: the “settled” wait is also tied to the program’s frames —
        `settle` seconds of sleep are replaced by a wait for a pair of frames, and between
        pixel reads a frame is waited for (softly: the picture may also have frozen)."""
        self.wait_frame(2, what="frames before reading a settled pixel", soft=True)
        prev = None
        # WE WAIT BY THE CLOCK, but do not sleep: the frame is drawn before the
        # DECODED video arrives (the engine normally keeps the last frame), and
        # “the frames stopped coming” here does not mean “the picture is the same”. So we
        # read the pixel again and again up to the deadline, not a fixed number of times.
        deadline = time.time() + max(10.0, 60.0 * self._dt)
        while time.time() < deadline:
            px = self.pixel(*xy)
            fresh = differs_from is None or px != tuple(differs_from)
            # BLACK is not a frame, but “the composite is not yet full”: the engine normally
            # returns an empty layer until the frame arrives (invariant “a miss by the
            # deadline → the last frame/silence”). If it is not filtered out — the
            # wait for a CHANGE happily settles on this blackness, declaring
            # “a black clip to the left of the region”.
            if fresh and px == prev and sum(px) > 24:
                return px
            prev = px
            self.wait_frame(1, what="frame between pixel reads", soft=True)
        return prev or (0, 0, 0)

    def screen_diff(self, before):
        """Fraction of the screen that changed relative to the `before` snapshot
        (0..1). More reliable than a sparse grid of points: the window could have opened where
        the grid did not touch it — so the check “window appeared” broke
        for no reason (R.34)."""
        after = self.shot("diff")
        out = subprocess.run(
            ["compare", "-metric", "AE", "-fuzz", "3%", before, after, "null:"],
            capture_output=True,
            text=True,
        ).stderr.strip()
        os.unlink(after)
        self._shot_no -= 1
        # ImageMagick prints “12345” or “1.2345e+07” — the second broke the
        # naive digit sampling and gave “changed 514%”
        try:
            px = float(out.split()[0]) if out else 0.0
        except ValueError:
            px = 0.0
        g = geometry()
        return min(px / float(max(g["w"] * g["h"], 1)), 1.0)

    def pixel_of(self, image, x, y):
        """Color of the pixel from BEFORE the taken screenshot — a “before and after” comparison
        without a repeated screen snapshot."""
        out = subprocess.run(
            ["convert", image, "-format", f"%[pixel:p{{{x},{y}}}]", "info:-"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        nums = [
            int(v)
            for v in out.replace("(", ",").replace(")", ",").split(",")
            if v.strip().isdigit()
        ]
        return tuple(nums[:3])

    def project_now(self):
        """Project as it lies on disk (without saving)."""
        return Timeline(self.project)

    def save_now(self):
        """Ctrl+S and a wait for the WRITE, not “one and a half seconds just in case”.

        The program writes the project by atomically replacing the file — so the wait must be for
        a change of mtime, exactly as long as it took (R.68).
        """
        before = os.path.getmtime(self.project)
        self.key("ctrl+s")
        deadline = time.time() + max(40.0, 90.0 * self._dt)
        while time.time() < deadline:
            if os.path.getmtime(self.project) != before:
                return True
            # THERE WAS NOTHING TO WRITE — this is not a refusal: the program does not rewrite
            # a file that already holds the current revision (there is no asterisk,
            # `dirty` in the dump is false). To require a write would always mean
            # declaring a defect exactly the behaviour we asked for.
            if not self.geom().get("dirty", False):
                return False
            _poll(0.05)
        raise Failure("the program did not write the project after Ctrl+S, although there are edits")

    def project_text(self):
        """TEXT of the project file as it lies on disk (without saving).

        R.70: the project is its own `.kt` format, and the scaffold knows the path to it
        (`self.project`), not the scenario: the scenario builds the EXCHANGE FILE, from
        which the import door made the project. A scenario that read its own
        source after Ctrl+S was looking at a file that the program never touched
        at all — exactly how five scenarios turned red in R.70."""
        with open(self.project, encoding="utf-8") as f:
            return f.read()

    def saved_text(self):
        """Ctrl+S and the TEXT of the project file: claims about the MODEL (curves,
        chains, geometry, generator parameters) are read here.
        Claims about the SCHEMA of the exchange file — via `saved_project().doc`."""
        self.save_now()
        return self.project_text()

    def saved_project(self):
        """Ctrl+S and parsing of what the program wrote to disk (via the export door:
        `doc` — the document of the EXCHANGE file, `otio_path` — its path)."""
        before = os.path.getmtime(self.project)
        self.key("ctrl+s")
        deadline = time.time() + 10.0
        while time.time() < deadline:
            if os.path.getmtime(self.project) != before:
                break
            _poll(0.05)
        return Timeline(self.project)

    # ---- checks ----------------------------------------------------------

    def expect(self, ok, what):
        print(f"   {'ok  ' if ok else 'FAILURE'} {what}")
        if not ok:
            self.problems.append(what)

    def near(self, got, want, tol, what):
        self.expect(abs(got - want) <= tol, f"{what}: {got} ≈ {want} (±{tol})")

    def done(self):
        self._reported = True
        if self.problems:
            for p in self.problems:
                print(f"   mismatch: {p}", file=sys.stderr)
            return 1
        return 0


def _cli(*args):
    subprocess.run([KT_CLI, *args], check=True, capture_output=True)


def project_kt_path(otio_path):
    """Name of the PROJECT file next to the scenario’s exchange file: `x.otio` → `x.kt`."""
    return os.path.splitext(otio_path)[0] + ".kt"


def import_project(src, dest=None):
    """Scenario exchange file → PROJECT file through the same door as
    `File → Import` (R.70). Idempotent: a `.kt` that is NEWER than the source
    is not overwritten — it may hold what the program saved
    (a second `with Live` of the same scenario must not overwrite it)."""
    if src.endswith(".kt"):
        return src
    dest = dest or project_kt_path(src)
    if not os.path.exists(dest) or os.path.getmtime(dest) < os.path.getmtime(src):
        _cli("import", src, dest, "--force")
    return dest


def export_project(kt_path, dest=None):
    """Project file → exchange file through the export door (`kt-cli export-otio`)."""
    dest = dest or kt_path + ".export.otio"
    _cli("export-otio", kt_path, dest, "--force")
    return dest


def new_otio(dest):
    """Empty EXCHANGE FILE for editing by a scenario: `kt-cli new` writes the project,
    export gives OTIO with a kt envelope — the format stays with the library."""
    tmp = dest + ".new.kt"
    _cli("new", tmp, "--force")
    _cli("export-otio", tmp, dest, "--force")
    os.remove(tmp)
    return dest


def demo_otio(dest):
    """Demo project as an EXCHANGE FILE (see `new_otio`)."""
    tmp = dest + ".demo.kt"
    _cli("demo", tmp, "--force")
    _cli("export-otio", tmp, dest, "--force")
    os.remove(tmp)
    return dest


def demo_project(dest):
    """A fresh demo project by the program itself (kt-cli demo) — a single writer of the format.

    `--force`: scenarios are restarted in place, while kt-cli with R.49 (item 359)
    does not silently overwrite an existing file. The demo media of the same round are
    synthetic, so the scenario has nothing to fetch from outside.
    R.70: it is handed over as an exchange file, which `Live` imports into the program."""
    return demo_otio(dest)


def project_with_clips(dest, clips, fps=30, width=1280, height=720):
    """Minimal project from video clips: a list of `(start_s, dur_s)` or
    `(start_s, dur_s, "synth:Pattern")`. Different sources give different colors
    of the tiles — this lets the scenario tell WHOSE material was drawn.
    Written via `kt-cli new` + export + editing the exchange file: the format
    stays with our library; `Live` imports it into the program."""
    new_otio(dest)
    with open(dest) as f:
        doc = json.load(f)
    doc["metadata"]["kt"]["settings"].update(fps=fps, width=width, height=height)
    # empty tracks are not written to the file (R.25), so the video track
    # we create ourselves
    track = next(
        (t for t in doc["tracks"]["children"] if t.get("kind") == "Video"), None
    )
    if track is None:
        track = {
            "OTIO_SCHEMA": "Track.1",
            "name": "",
            "kind": "Video",
            "children": [],
            "source_range": None,
            "effects": [],
            "markers": [],
            "enabled": True,
            "metadata": {},
        }
        doc["tracks"]["children"].append(track)
    track["children"] = []
    media = {}
    cursor = 0.0
    for i, spec in enumerate(clips, start=1):
        start_s, dur_s = spec[0], spec[1]
        path = spec[2] if len(spec) > 2 else "synth:Ramp"
        mid = media.setdefault(path, len(media) + 1)
        if start_s > cursor + 1e-9:
            track["children"].append(_gap(start_s - cursor))
            cursor = start_s
        track["children"].append(_clip(i, dur_s, path, mid))
        cursor += dur_s
    doc["metadata"]["kt"]["bin_media"] = []
    with open(dest, "w") as f:
        json.dump(doc, f, indent=4)
    return dest


def _rt(seconds):
    return {"OTIO_SCHEMA": "RationalTime.1", "value": round(seconds * 90000), "rate": 90000.0}


def _range(start_s, dur_s):
    return {"OTIO_SCHEMA": "TimeRange.1", "duration": _rt(dur_s), "start_time": _rt(start_s)}


def _gap(dur_s):
    return {
        "OTIO_SCHEMA": "Gap.1",
        "name": "",
        "source_range": _range(0, dur_s),
        "effects": [],
        "markers": [],
        "enabled": True,
        "metadata": {},
    }


def _clip(idx, dur_s, path="synth:Ramp", media_id=1):
    return {
        "OTIO_SCHEMA": "Clip.2",
        "name": f"c{idx}",
        "source_range": _range(0, dur_s),
        "media_references": {
            "DEFAULT_MEDIA": {
                "OTIO_SCHEMA": "ExternalReference.1",
                "name": "",
                "target_url": path,
                "available_range": None,
                "available_image_bounds": None,
                "metadata": {},
            }
        },
        "active_media_reference_key": "DEFAULT_MEDIA",
        "effects": [],
        "markers": [],
        "enabled": True,
        "metadata": {"kt": {"id": idx, "media": media_id}},
    }


def color_media(out_dir, name, rgb, seconds, fps=30, size="640x360"):
    """Real mp4 of solid color: `synth:` sources in the app give only
    sound, while preview scenarios need a picture. Idempotent."""
    path = os.path.join(out_dir, f"{name}.mp4")
    if os.path.exists(path):
        return path
    subprocess.run(
        [
            "ffmpeg", "-v", "error", "-y",
            "-f", "lavfi", "-i", f"color=c=0x{rgb}:size={size}:rate={fps}:duration={seconds}",
            "-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo",
            "-shortest",
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-b:a", "96k",
            path,
        ],
        check=True,
    )
    return path


def pattern_media(out_dir, name, seconds, fps=30, size="640x360"):
    """Real mp4 with STRUCTURE (SMPTE color bars). A solid fill
    (`color_media`) suits “is there a picture”, but not “did it move”:
    for a single-tone frame any shift and any zoom give the same pixel.
    Idempotent."""
    path = os.path.join(out_dir, f"{name}.mp4")
    if os.path.exists(path):
        return path
    subprocess.run(
        [
            "ffmpeg", "-v", "error", "-y",
            "-f", "lavfi", "-i", f"smptebars=size={size}:rate={fps}:duration={seconds}",
            "-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo",
            "-shortest",
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-b:a", "96k",
            path,
        ],
        check=True,
    )
    return path


def geometry():
    """Bounds of the kt-app window in screen coordinates."""
    x, y, w, h = probe.origin(probe.window())
    return {"x": x, "y": y, "w": w, "h": h}


def require_tools():
    for tool in ("xwd", "convert"):
        if not shutil.which(tool):
            raise Failure(f"not installed {tool} (x11-apps/ImageMagick) — screenshots cannot be taken")

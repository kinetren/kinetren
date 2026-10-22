#!/usr/bin/env python3
"""WORKER of the live run (R.68): its own screen, its own stand, its own list of scenarios.

Previously the live check was strictly sequential: 50 scenarios, 1012 s, and each
paid 3-5 s for a cold start of kt-app. The ban on parallel runs (“live scenarios in two
threads are not run”) appeared in R.59 §468 after a second run under load of ~14 turned green
scenarios red; the owner lifted it in R.68 together with the mechanism that removes the cause:
each worker has ITS OWN virtual screen,
ITS OWN user storage, and a restricted software renderer;
a red scenario is rerun alone
(this is done by live-check.sh).

Worker:
  * brings up Xvfb and openbox on its screen number;
  * keeps ONE kt-app (the stand) and opens the scenarios’ projects in it one after another;
  * a scenario that checks the program’s own START itself is not given the stand: such a
    scenario declares `OWN_PROCESS = True` in its file and runs with the stand stopped —
    otherwise there would be two kinetren windows on the screen,
    and `probe.window()` would aim at the other one.

    usage: worker.py <screen number> <directory> <scenario> [<scenario> ...]

Verdicts are written to <directory>/verdicts/<scenario> (“ok” or “fail”), the output of the
scenario — to the common stream of the worker.
"""
import contextlib
import json
import os
import re
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
RUST = os.path.abspath(os.path.join(HERE, "..", ".."))
KT_APP = os.path.join(RUST, "target", "debug", "kinetren")
KT_CLI = os.path.join(RUST, "target", "debug", "kt-cli")
SCREEN = os.environ.get("KT_LIVE_SCREEN", "1920x1080x24")
# how many threads of software rendering are given to ONE kt-app: without a limit one
# llvmpipe takes about seven cores, and the workers took cores from each other —
# exactly the load because of which the ban on parallelism appeared
LP_THREADS = os.environ.get("KT_LIVE_LP_THREADS", "4")

OWN = re.compile(r"^OWN_PROCESS\s*=\s*True", re.M)
# The stand-in scenario of the guard (t801/t802) does not launch the program at all —
# there is no point raising an X server for it. This is not a trifle: the guards live INSIDE
# a workshop run, and an extra Xvfb for each of their launches took time from
# the engine tests sensitive to it (t663 cost 60 s instead of 0.02).
NO_DISPLAY = re.compile(r"^NO_DISPLAY\s*=\s*True", re.M)


def _src(name):
    return open(os.path.join(HERE, f"{name}.py"), encoding="utf-8").read()


def wants_own_process(name):
    """Did the scenario declare that it needs its own process?"""
    return bool(OWN.search(_src(name)))


def needs_display(names):
    """Does at least one scenario need the screen?"""
    return any(not NO_DISPLAY.search(_src(n)) for n in names)


class Screen:
    """Its own Xvfb and its own openbox. The window manager is needed to maximize the
    window (the rule “check in the MAXIMIZED window”, R.13) and for gestures with
    the edges."""

    def __init__(self, display, out):
        self.display = display
        self.out = out
        self.procs = []

    def free_display(self):
        """Remove the LOCK of a dead X server on its screen number.

        An interrupted run leaves `/tmp/.X<n>-lock`, and the next Xvfb
        refuses to start (“Server is already active for display N”).
        The program then connects to nowhere and crashes inside winit, and
        this looks like “the scenario broke”. The lock is removed ONLY if
        there is no live server behind it.
        """
        n = self.display.lstrip(":")
        alive = subprocess.run(
            ["pgrep", "-f", f"Xvfb {self.display}"], capture_output=True, text=True
        ).stdout.split()
        if alive:
            return
        for path in (f"/tmp/.X{n}-lock", f"/tmp/.X11-unix/X{n}"):
            try:
                os.unlink(path)
            except OSError:
                pass

    def __enter__(self):
        self.free_display()
        # the log is ITS OWN for each screen: the workers overwrote one shared file for
        # each other, and the reason for the X server’s refusal disappeared
        logs = open(os.path.join(self.out, f"xvfb{self.display.lstrip(':')}.log"), "w")
        self.procs.append(
            subprocess.Popen(
                ["Xvfb", self.display, "-screen", "0", SCREEN, "-nolisten", "tcp"],
                stdout=logs,
                stderr=logs,
            )
        )
        os.environ["DISPLAY"] = self.display
        for _ in range(80):
            try:
                subprocess.run(
                    [
                        sys.executable,
                        "-c",
                        "import Xlib.display, os; Xlib.display.Display(os.environ['DISPLAY'])",
                    ],
                    check=True,
                    capture_output=True,
                )
                break
            except subprocess.CalledProcessError:
                time.sleep(0.25)
        else:
            raise SystemExit(
                f"X server on {self.display} did not come up, see "
                f"{self.out}/xvfb{self.display.lstrip(':')}.log"
            )
        if shutil.which("openbox"):
            wm = open(os.path.join(self.out, f"openbox{self.display.lstrip(':')}.log"), "w")
            self.procs.append(subprocess.Popen(["openbox"], stdout=wm, stderr=wm))
            time.sleep(0.5)
        return self

    def __exit__(self, *exc):
        for p in self.procs:
            p.terminate()
        for p in self.procs:
            try:
                p.wait(timeout=5)
            except subprocess.TimeoutExpired:
                p.kill()
        return False


class Bench:
    """One kt-app of the worker: in it the scenarios open their projects one after another."""

    def __init__(self, out, display):
        # the stand directory is ITS OWN FOR EACH WORKER: a shared `out` was one for the
        # run, and a shared `bench/` meant a shared command channel and a shared geometry
        # dump — the workers opened projects in each other’s places and read others’
        # frames. This is what looked like “parallelism paints everything”.
        self.dir = os.path.join(out, "bench" + display.replace(":", ""))
        os.makedirs(self.dir, exist_ok=True)
        self.ctl = os.path.join(self.dir, "ctl")
        self.geom = os.path.join(self.dir, "geom.json")
        self.store = os.path.join(self.dir, "store")
        self.blank = os.path.join(self.dir, "blank.kt")
        self.proc = None

    def env(self):
        e = dict(os.environ)
        e.setdefault("WGPU_BACKEND", "gl")
        e.setdefault("LIBGL_ALWAYS_SOFTWARE", "1")
        e["LP_NUM_THREADS"] = LP_THREADS
        e["KT_GEOM_DUMP"] = self.geom
        e["KT_LIVE_CTL"] = self.ctl
        e["KT_USER_STORE_DIR"] = self.store
        # XDG_DATA_HOME — ALSO its own: the program looks up by it the unnamed
        # recovery, the journal and the project locks. Without it the run climbs into the
        # developer’s home directory and finds someone else’s unsaved work there —
        # the “unsaved” scenario got a modal window before the first
        # gesture and silently checked the wrong thing.
        e["XDG_DATA_HOME"] = os.path.join(self.dir, "xdg")
        # the periodic autosave is switched off in the stand: scenarios that need it
        # by the second (the recovery check) take a process of their own
        e["KT_AUTOSAVE_SECS"] = "600"
        return e

    def kill_stale(self):
        """Kill a FOREIGN program left in this stand directory.

        An interrupted run (Ctrl+C, a killed parent) leaves kt-app alive, and
        it keeps writing ITS OWN geometry dump to the same file: the next
        run reads frames and the project of a foreign process and turns red for no clear
        reason. The stand directory is the worker’s property, so we clean it
        before the start.
        """
        try:
            out = subprocess.run(
                ["pgrep", "-f", self.dir], capture_output=True, text=True
            ).stdout.split()
        except OSError:
            return
        me = str(os.getpid())
        for pid in out:
            if pid == me:
                continue
            try:
                os.kill(int(pid), 15)
            except (ProcessLookupError, PermissionError, ValueError):
                pass

    def ensure(self):
        """Is the stand alive? Otherwise bring it up (first time or after a crash)."""
        if self.proc is not None and self.proc.poll() is None:
            return
        self.kill_stale()
        # RIG STORAGE — CLEAN ON EACH LAUNCH: eframe remembers in it
        # the WINDOW SIZE, and the form-contract scenario (R.66), which deliberately
        # makes the window low, left this size for all following runs.
        # In a small window the zoom and view differ — gesture coordinates
        # drifted into track labels, and it looked like “the rig breaks everything”.
        shutil.rmtree(self.store, ignore_errors=True)
        shutil.rmtree(os.path.join(self.dir, "xdg"), ignore_errors=True)
        os.makedirs(self.store, exist_ok=True)
        subprocess.run([KT_CLI, "new", self.blank, "--force"], check=True, capture_output=True)
        for p in (self.ctl, self.geom):
            if os.path.exists(p):
                os.unlink(p)
        log = open(os.path.join(self.dir, "kt-app.log"), "a")
        self.proc = subprocess.Popen(
            [KT_APP, self.blank], stdout=log, stderr=log, env=self.env()
        )
        # we wait for DRAWN frames: the program reports their number itself
        deadline = time.time() + 60
        while time.time() < deadline:
            if self.proc.poll() is not None:
                raise SystemExit(f"stand did not come up, see {self.dir}/kt-app.log")
            try:
                with open(self.geom) as f:
                    if int(json.load(f).get("frame", 0)) >= 2:
                        break
            except (OSError, ValueError):
                pass
            time.sleep(0.05)
        else:
            raise SystemExit("the stand drew not a single frame")
        # WINDOW REFERENCE — X-window geometry IN PIXELS, not the canvas size from
        # the dump: the form-contract scenario (R.66) sets the window low, and the
        # next one must get it back. The egui canvas size is in POINTS, and restoring
        # from it shrank the window on every run.
        import importlib.util

        spec = importlib.util.spec_from_file_location(
            "ui_probe", os.path.join(RUST, "scripts", "ui-probe.py")
        )
        probe = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(probe)
        # GEOMETRY IS CAPTURED WHEN IT HAS SETTLED: the window manager
        # maximizes the window NOT instantly, and a snapshot right after the first
        # frames recorded the window BEFORE it was maximized. Every next scenario
        # “restored” that small size — and at it the zoom and view differ,
        # and gesture coordinates drifted into track labels.
        last, stable = None, 0.0
        deadline = time.time() + 20
        while time.time() < deadline:
            g = probe.origin(probe.window())
            if g == last:
                stable += 0.1
                if stable >= 0.6:
                    break
            else:
                last, stable = g, 0.0
            time.sleep(0.1)
        x, y, w, h = last
        with open(os.path.join(self.dir, "window.json"), "w") as f:
            json.dump({"x": x, "y": y, "w": w, "h": h}, f)

    def stop(self):
        if self.proc is None:
            return
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        self.proc = None


def main():
    display, out = sys.argv[1], os.path.abspath(sys.argv[2])
    names = sys.argv[3:]
    os.makedirs(out, exist_ok=True)
    verdicts = os.path.join(out, "verdicts")
    os.makedirs(verdicts, exist_ok=True)
    fail = 0
    screen = Screen(display, out) if needs_display(names) else contextlib.nullcontext()
    with screen:
        bench = Bench(out, display)
        try:
            for name in names:
                own = wants_own_process(name)
                scen_out = os.path.join(out, name)
                os.makedirs(scen_out, exist_ok=True)
                # RUN STORAGE — CLEAN ON EACH RUN (R.68):
                # a failed run leaves a recovery file and a working place in it, and a
                # RERUN on its own starts from another run's state: a scenario that is
                # green by itself turned red a second time in a row, and the
                # “red only twice” mechanism stopped telling a flake
                # from a defect.
                for leftover in ("store", "xdg"):
                    shutil.rmtree(os.path.join(scen_out, leftover), ignore_errors=True)
                # and the sleep report (R.69): a failed scenario leaves no
                # number of its own, or the run would add the cost of the PREVIOUS time
                try:
                    os.unlink(os.path.join(scen_out, "slept.txt"))
                except OSError:
                    pass
                env = dict(os.environ)
                if own:
                    # two kinetren windows on one screen — that is aiming at someone else's
                    bench.stop()
                    env.pop("KT_LIVE_BENCH", None)
                    # storage — its own, and of a scenario with its own process:
                    # a neighbor's working place has no right to change the verdict
                    env["KT_USER_STORE_DIR"] = os.path.join(scen_out, "store")
                    env["XDG_DATA_HOME"] = os.path.join(scen_out, "xdg")
                else:
                    bench.ensure()
                    env["KT_LIVE_BENCH"] = bench.dir
                    env["KT_AUTOSAVE_SECS"] = "600"
                print(f"== {name} ==", flush=True)
                started = time.time()
                rc = subprocess.run(
                    [sys.executable, os.path.join(HERE, f"{name}.py"), scen_out], env=env
                ).returncode
                # SCENARIO COST — INTO THE SHARED TABLE: the next run uses it to
                # spread scenarios across workers (balance.py). Without it
                # one worker gets the longest scenario and holds the whole
                # run while the others have already finished.
                try:
                    with open(os.path.join(out, "times.txt"), "a") as f:
                        f.write(f"{name}\t{time.time() - started:.1f}\n")
                except OSError:
                    pass
                ok = rc == 0
                with open(os.path.join(verdicts, name), "w") as f:
                    f.write("ok" if ok else "fail")
                if ok:
                    print("   ok", flush=True)
                else:
                    print(f"   FAILURE (screenshots: {scen_out})", flush=True)
                    fail = 1
        finally:
            bench.stop()
    return fail


if __name__ == "__main__":
    sys.exit(main())

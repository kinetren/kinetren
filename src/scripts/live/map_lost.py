#!/usr/bin/env python3
"""Live scenario map_lost (found-defects 830, 954): a window manager that drops the program's first request
to be shown. openbox does it while it starts (reproduced: a map asked 16–32 ms into its start is lost, the
window stays unmapped and nobody's), and then a person has a running program and no window.

Its own X server and a window manager of the scenario's own, so what is dropped is a fact and not a race:

1. the program starts; its first map request is dropped;
2. the program looks, finds its window nobody took, asks again; the window is shown and drawn;
3. the journal says so; nothing looks any more.
"""
import os
import subprocess
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(__file__))
import kt_live  # noqa: E402
from kt_live import KT_APP, demo_project, require_tools, stretched  # noqa: E402

OWN_PROCESS = True


class DroppingWm(threading.Thread):
    """Takes the root's map requests, drops the first, honours the others."""

    def __init__(self, display):
        super().__init__(daemon=True)
        from Xlib import X
        self.X = X
        self.d = display
        self.dropped = self.shown = 0
        self.d.screen().root.change_attributes(event_mask=X.SubstructureRedirectMask)
        self.d.sync()

    def run(self):
        while True:
            e = self.d.next_event()
            if e.type == self.X.MapRequest:
                if e.window.get_geometry().width <= 400:
                    e.window.map()
                elif not self.dropped:
                    self.dropped = 1
                else:
                    self.shown += 1
                    e.window.map()
            elif e.type == self.X.ConfigureRequest:
                e.window.configure(width=e.width, height=e.height)
            self.d.sync()


def free_display():
    for n in range(300, 400):
        if not os.path.exists(f"/tmp/.X{n}-lock"):
            return f":{n}"
    raise SystemExit("no free display")


def main(out):
    require_tools()
    os.makedirs(out, exist_ok=True)
    proj = demo_project(os.path.join(out, "map_lost.otio"))
    if proj.endswith(".otio"):
        proj = demo_project(os.path.join(out, "map_lost.otio"))
    disp = free_display()
    problems = []

    def expect(ok, what):
        print(f"   {'ok  ' if ok else 'FAILURE'} {what}", flush=True)
        if not ok:
            problems.append(what)

    xv = subprocess.Popen(["Xvfb", disp, "-screen", "0", "1600x1000x24", "-nolisten", "tcp", "-noreset"],
                          stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    app = None
    try:
        from Xlib import X
        from Xlib import display as xdisplay
        end = time.time() + stretched(30.0)
        d = None
        while d is None and time.time() < end:
            try:
                d = xdisplay.Display(disp)
            except Exception:
                kt_live._poll(0.1)
        expect(d is not None, "the X server is up")
        wm = DroppingWm(xdisplay.Display(disp))
        wm.start()
        env = dict(os.environ, DISPLAY=disp, WGPU_BACKEND="gl", LIBGL_ALWAYS_SOFTWARE="1",
                   KT_USER_STORE_DIR=os.path.join(out, "store"), XDG_DATA_HOME=os.path.join(out, "xdg"),
                   KT_NO_EXTERNAL_OPEN="1", DBUS_SESSION_BUS_ADDRESS="disabled:")
        env.pop("WAYLAND_DISPLAY", None)
        for k in ("KT_GEOM_DUMP", "KT_LIVE_CTL", "KT_LIVE_BENCH"):
            env.pop(k, None)
        log = open(os.path.join(out, "kt-app.log"), "w")
        app = subprocess.Popen([KT_APP, proj], env=env, stdout=log, stderr=log)

        def until(cond, limit):
            end = time.time() + stretched(limit)
            while time.time() < end:
                if cond():
                    return True
                kt_live._poll(0.1)
            return False

        expect(until(lambda: wm.dropped, 120), "the program asked to be shown, and the window manager dropped it")
        expect(until(lambda: wm.shown, 60), "it asked again, and the window manager took the window")

        def viewable():
            for w in d.screen().root.query_tree().children:
                try:
                    if w.get_geometry().width > 400 and w.get_attributes().map_state == X.IsViewable:
                        return w
                except Exception:
                    pass
            return None

        expect(until(lambda: viewable() is not None, 30), "the window is on screen")
        journal = os.path.join(out, "xdg", "kinetren", "kt-app.log")

        def said():
            return os.path.exists(journal) and "did not take the window" in open(journal, errors="replace").read()

        expect(until(said, 30), "the journal says the window manager did not take the window")
        subprocess.run(["import", "-display", disp, "-window", "root", os.path.join(out, "01-step-shown.png")])
        n = wm.shown
        kt_live.pause(3.0, "the look's own step is a second: nothing asks in three")
        expect(wm.shown == n, "nothing asks any more")
    finally:
        if app is not None:
            app.terminate()
            try:
                app.wait(timeout=10)
            except subprocess.TimeoutExpired:
                app.kill()
        xv.terminate()
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/map_lost"))

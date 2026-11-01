#!/usr/bin/env python3
"""Live scenario render-crash-and-recovery: the crash window appears RIGHT AFTER the crash.

The owner (kkt-26.11.01 item 9, decision of 2026-11-01): "I want the crash window right after
the crash, not who knows when". The program is crashed on purpose on the UI thread (the bench
command `crash`, the place the owner's render panic came from); the supervisor — the process the
scenario started — must open its window with the panic, the version and the log; "Restore the
project" must start the editor again, which offers the unsaved edit, and the edit must be back.
Our fixtures, not the owner's projects.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Failure, Live, geometry, import_project, poll_step, project_with_clips, require_tools  # noqa: E402

# ITS OWN PROCESS (R.68): the scenario kills the editor and restarts it — the worker stand
# cannot host that.
OWN_PROCESS = True


def wait_json(path, ok, timeout=60.0, what="file"):
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with open(path) as f:
                d = json.load(f)
            if ok(d):
                return d
        except (OSError, ValueError):
            pass
        poll_step(0.1)
    raise Failure(f"{what} did not appear within {timeout} s: {path}")


def main(out):
    require_tools()
    proj = import_project(project_with_clips(
        os.path.join(out, "crash.otio"),
        [(0.0, 2.0, "synth:Ramp"), (3.0, 2.0, "synth:Ramp")],
    ))
    rec = proj + ".recovery.kt"
    if os.path.exists(rec):
        os.remove(rec)
    os.environ["KT_AUTOSAVE_SECS"] = "2"
    os.environ["KT_LANG"] = "en"

    with Live("crash", out, project=proj, own_process=True) as app:
        app.step("open", "the project is open")
        p = app.project_now()
        src = p.clips[1]
        y = app.lane_y("video", 0)
        x0 = app.x_at(src.mid())
        app.drag(x0, y, x0 + 160.0, y)
        app.step("edited", "the second event dragged right (unsaved)")
        moved = max(c.start for c in app.project_now().clips)
        deadline = time.time() + 60
        while not (os.path.exists(rec) and os.path.getsize(rec) > 0):
            if time.time() > deadline:
                raise Failure("the recovery file did not appear")
            poll_step(0.25)

        crash_dump = app.geom_path + ".crash.json"
        if os.path.exists(crash_dump):
            os.remove(crash_dump)
        app._ctl("crash")
        d = wait_json(crash_dump, lambda d: d.get("fields"), what="the crash window")
        # the editor's dump is stale from here: the next editor writes a fresh one
        os.remove(app.geom_path)
        app.step("crash-window", "the crash window, right after the crash")
        app.expect(app.proc.poll() is None, "the supervisor is alive and shows the window")
        app.expect("deliberate panic" in d["panic"], f"the window names the panic: {d['panic']!r}")
        app.expect("internal error" in d["what"], f"the window says what happened: {d['what']!r}")
        app.expect(d["version"].startswith("kt"), f"the window names the version: {d['version']!r}")
        app.expect(d["tail"] > 0, "the window shows the crashed run's log lines")
        report = d["report"]
        app.expect(os.path.exists(report), f"the report is in place: {report}")
        text = open(report).read()
        app.expect("shown: " in text, "the report is marked as shown")
        app.expect(os.path.exists(app.out + "/xdg/kinetren/kt-app.log"), "the run's log is in the data directory")

        f = next(f for f in d["fields"] if f["param"] == "crash.restore")
        win = geometry()
        app.click(win["x"] + int(f["x"] + f["w"] / 2), win["y"] + int(f["y"] + f["h"] / 2))
        # the editor starts again: wait for its own dump, then for the recovery window
        wait_json(app.geom_path, lambda g: g.get("rows") and int(g.get("frame", 0)) >= 2,
                  what="the restarted editor")
        app.step("restarted", "the editor started again and offers the unsaved work")
        if not app.click_probe("ask.restore", "the recovery window offers the work"):
            raise Failure("no recovery window after Restore the project")
        app.settle()
        app.step("restored", "the edit is back in the editor")
        got = [c.start for c in app.project_now().clips]
        app.expect(max(got) == moved, f"the dragged event is back where it was dragged ({got}, {moved})")
        g = app.geom()
        app.expect(
            not any("ended abnormally" in n for n in g.get("notices", [])),
            f"the shown crash is not announced again: {g.get('notices')}",
        )
        app.expect(app.project_now() is not None and g.get("project", "").endswith("crash.kt"),
                   f"the restored edit is under its project's name: {g.get('project')}")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/crash"))

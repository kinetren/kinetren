#!/usr/bin/env python3
"""Live scenario segv-crashes: a NATIVE crash says where it happened.

The owner (kkt-26.11.08 H5): the crash report was scanty — a SIGSEGV left the signal and nothing
else. The editor is faulted on purpose (the bench command `fault`: a write through a null pointer on
a thread named `live-fault`); the supervisor's crash window must name the signal, the thread and the
function, show the run's journal lines, and the report must hold the stack with file and line and the
journal. Our fixtures, not the owner's projects.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Failure, Live, import_project, poll_step, project_with_clips, require_tools  # noqa: E402
from kt_live import stretched  # noqa: E402

# ITS OWN PROCESS (R.68): the scenario kills the editor.
OWN_PROCESS = True


def wait_json(path, ok, timeout=60.0, what="file"):
    deadline = time.time() + stretched(timeout)
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
    proj = import_project(project_with_clips(os.path.join(out, "fault.otio"), [(0.0, 2.0, "synth:Ramp")]))
    # a new editor runs the last bench command it finds: a `fault` left by a previous run would
    # fault it at start
    ctl = os.path.join(out, "ctl")
    if os.path.exists(ctl):
        os.remove(ctl)
    os.environ["KT_LANG"] = "en"

    with Live("fault", out, project=proj, own_process=True) as app:
        app.step("open", "the project is open")
        crash_dump = app.geom_path + ".crash.json"
        if os.path.exists(crash_dump):
            os.remove(crash_dump)
        app._ctl("fault")
        d = wait_json(crash_dump, lambda d: d.get("fields"), what="the crash window")
        app._ctl("noop")
        os.remove(app.geom_path)
        # the dump is written while the first frame is laid out: give the software renderer a
        # moment to present it
        poll_step(2.0)
        app.step("crash-window", "the crash window names the signal, the thread and the function")
        app.expect(app.proc.poll() is None, "the supervisor is alive and shows the window")
        app.expect("SIGSEGV" in d["what"], f"the window says what happened: {d['what']!r}")
        fault = d.get("fault", "")
        app.expect(fault.startswith('SIGSEGV in thread "live-fault"'), f"the window names the thread: {fault!r}")
        app.expect("kt_crash_probe_segv" in fault, f"the window names the function: {fault!r}")
        app.expect(d["tail"] > 0, "the window shows the run's journal lines")
        text = open(d["report"]).read()
        app.expect("summary: " + fault in text.replace("\\", "/"), "the report carries the summary line")
        app.expect("kt_crash::testing::kt_crash_probe_segv (libs/kt-crash/src/lib.rs:" in text,
                   "the report holds the stack with file and line")
        app.expect("live-fault  ← faulted" in text, "the report lists the threads, the faulted one marked")
        app.expect("--- telemetry journal, the run's last lines ---" in text and "RUN" in text,
                   "the report holds the run's journal")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/fault"))

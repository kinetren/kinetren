#!/usr/bin/env python3
"""Live scenario R.49 A (items 351, 353): the work survives the KILLING of the program.

It checks exactly what the whole wave was for: edit → SIGKILL (neither `on_exit` nor
`Drop` is called — the kernel simply terminates the process) → restart → the program itself
offers to restore → the work is in place.

And what the owner decided: **only Ctrl+S writes to the project file.** Until the
user has saved, his project file must stay untouched — even after
the background write has completed.

The second round checks rule 14: restoration works AGAIN, not just
once, and does not consume its own source.
"""
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Failure, Live, import_project, poll_step, project_with_clips, require_tools  # noqa: E402

# ITS OWN PROCESS (R.68): the scenario checks the program START ITSELF, not the work in an
# already open project — the worker stand does not suit it.
OWN_PROCESS = True

SEC = 90000


def recovery_of(project):
    return project + ".recovery.kt"


def wait_file(path, timeout=90.0, what="file"):
    deadline = time.time() + timeout
    while time.time() < deadline:
        if os.path.exists(path) and os.path.getsize(path) > 0:
            return True
        poll_step(0.25)
    raise Failure(f"{what} did not appear within {timeout} s: {path}")


def main(out):
    require_tools()
    # a PROJECT file (R.70): the scenario builds an exchange file, the program lives in .kt
    proj = import_project(project_with_clips(
        os.path.join(out, "recover.otio"),
        [(0.0, 2.0, "synth:Ramp"), (3.0, 2.0, "synth:Ramp")],
    ))
    before = open(proj).read()
    rec = recovery_of(proj)
    if os.path.exists(rec):
        os.remove(rec)

    # background save every 2 seconds: waiting half a minute in each run
    # is dishonest with respect to the check
    os.environ["KT_AUTOSAVE_SECS"] = "2"

    problems = []
    with Live("recover", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 2, f"start: two events, {p.clips}")

        # an edit by a real gesture: we drag the second event to the right
        src = p.clips[1]
        y = app.lane_y("video", 0)
        x0 = app.x_at(src.mid())
        app.drag(x0, y, x0 + 160.0, y)
        app.shot("moved")

        wait_file(rec, what="recovery file")
        app.expect(
            open(proj).read() == before,
            "the project file was NOT touched by the background write (only Ctrl+S changes it)",
        )

        # SIGKILL: neither on_exit nor Drop has time to run — as in a fall
        app.proc.kill()
        app.proc.wait(timeout=10)
        problems = app.problems

    if not os.path.exists(rec):
        raise Failure("after the killing there is no recovery file — the work is lost")
    snapshot = open(rec).read()

    moved_start = None
    for round_no in ("first", "second"):
        # an identical starting setup for both rounds: the project as it was,
        # the recovery — that very snapshot, and certainly FRESHER than the project
        open(proj, "w").write(before)
        open(rec, "w").write(snapshot)
        old_t = time.time() - 600
        os.utime(proj, (old_t, old_t))

        with Live(f"recover-{round_no}", out, project=proj) as app:
            app.shot(f"restart-{round_no}")
            # the recovery window opened by itself — we press “recover”
            if not app.click_probe("ask.restore", f"{round_no} round: the work was offered"):
                raise Failure("no recovery window — the work was not offered")
            app.shot(f"restored-{round_no}")
            app.expect(
                os.path.exists(rec),
                f"{round_no} round: the restoration did not consume its own source",
            )

            tl = app.saved_project()
            got = [c.start for c in tl.clips]
            if moved_start is None:
                moved_start = max(got)
            app.expect(
                max(got) == moved_start and moved_start > 270000,
                f"{round_no} round: the restored position is in place ({got})",
            )
            problems += app.problems

    if problems:
        for p in problems:
            print(f"   mismatch: {p}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/recover"))

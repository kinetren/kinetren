#!/usr/bin/env python3
"""Live scenario of change telemetry-journal: what the person does lands in the telemetry journal,
by name, with the clock time of the moment, and he can mark the moment himself.

The owner (kkt-26.11.07b, translated): the log was written late and he cannot say the exact time;
how would anyone tell from load-report.txt what he did? The real program on our fixture: play and pause,
a marker, Diagnostics opened, a mark with a note from its button, Diagnostics closed, a mark by
Ctrl+Alt+M. Then the journal file in the rig's own data directory is read back: every gesture is
there in order, each stamped inside the scenario's own wall-clock window, and the Diagnostics pane
showed the mark while the window was open.
"""
import datetime
import os
import re
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

# its own process: the journal lives in the data directory the process started with
OWN_PROCESS = True

STAMP = re.compile(r"^(\d{4}-\d\d-\d\d \d\d:\d\d:\d\d\.\d{3})([+-]\d\d:\d\d)  \+\d+\.\d{3}  (\S+)\s+(.*)$")


def records(path):
    """(moment, kind, text) of every journal line."""
    out = []
    try:
        lines = open(path, encoding="utf-8").read().splitlines()
    except OSError:
        return out
    for line in lines:
        m = STAMP.match(line)
        if m:
            at = datetime.datetime.fromisoformat(m.group(1) + m.group(2))
            out.append((at, m.group(3), m.group(4)))
    return out


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "journal.otio"), [(0.0, 8.0, "synth:Ramp")])
    journal = os.path.join(out, "xdg", "kinetren", "telemetry-journal.log")
    with Live("journal", out, project=proj, own_process=True) as app:
        app.resize(1900, 1000)
        t0 = datetime.datetime.now().astimezone() - datetime.timedelta(seconds=1)
        app.expect(
            app.until(lambda: any(k == "RUN" for _, k, _ in records(journal)), timeout=30.0),
            f"the run opened its journal in the rig's data directory ({journal})",
        )
        app.step("start", "the project is open; the journal has its RUN line")

        # --- the gestures ----------------------------------------------------------------------
        app.key("space")
        app.wait_playhead(lambda t: t > 90000, "a second played", timeout=15.0)
        app.key("space")
        app.until(lambda: not app.playing())
        app.step("played", "played a second and paused")
        app.key("m")
        app.until(lambda: any("AddMarker" in t for _, _, t in records(journal)))
        app.step("marker", "M put a marker at the playhead")
        app.click_probe("tb.diag", "the gauge button of the toolbar")
        app.expect(app.has_probe("win.diag"), "the gauge opened Diagnostics")
        app.expect(app.has_probe("diag.log"), "Diagnostics shows the journal pane")
        app.click_probe("diag.mark_note", "the note field of the journal pane")
        app.type_text("stall here")
        before = app.geom().get("status", "")
        app.hover_probe("diag.open_folder", "Open folder of the journal pane")
        app.until(lambda: app.geom().get("status", "") != before)
        app.expect(app.geom().get("status", "") != before, "hovering Open folder says where the journal lies")
        app.click_probe("diag.mark", "Mark the moment")
        app.until(lambda: any(k == "MARK" and t == "stall here" for _, k, t in records(journal)))
        app.settle()
        app.step("marked", "Diagnostics: the journal pane lists the mark “stall here” at the bottom, with its time")
        app.key("Escape")
        app.expect(app.wait_gone("win.diag"), "Esc closed Diagnostics")
        app.key("ctrl+alt+m")
        app.until(lambda: sum(1 for _, k, _ in records(journal) if k == "MARK") >= 2)
        app.step("key-mark", "Ctrl+Alt+M marked the moment again; the status line says so")
        t1 = datetime.datetime.now().astimezone() + datetime.timedelta(seconds=1)

        # --- read back -------------------------------------------------------------------------
        recs = records(journal)
        print("journal:")
        for at, k, t in recs:
            print(f"  {at.time()}  {k:<6} {t}")
        want = [
            ("PLAY", lambda t: t.startswith("PLAY")),
            ("PLAY", lambda t: t.startswith("PAUSE")),
            ("PLAY", lambda t: t.startswith("playback stopped after")),
            ("ACT", lambda t: t.startswith("edit AddMarker")),
            ("ACT", lambda t: t.startswith("window opened:")),
            ("MARK", lambda t: t == "stall here"),
            ("ACT", lambda t: t.startswith("window closed:")),
            ("MARK", lambda t: t == "the person marked this moment"),
        ]
        at_i, found = 0, []
        for kind, pred in want:
            hit = next((i for i in range(at_i, len(recs)) if recs[i][1] == kind and pred(recs[i][2])), None)
            app.expect(hit is not None, f"the journal holds {kind} after record {at_i}: {[r[2] for r in recs[at_i:at_i + 6]]}")
            if hit is None:
                break
            found.append(recs[hit])
            at_i = hit + 1
        if len(found) == len(want):
            times = [a for a, _, _ in found]
            app.expect(times == sorted(times), "the gestures' times are in the order they were made")
            app.expect(
                all(t0 <= a <= t1 for a in times),
                f"every gesture is stamped inside the scenario's own minute ({t0.time()} … {t1.time()})",
            )
            gap = (found[-1][0] - found[-3][0]).total_seconds()
            app.expect(gap > 0.2, f"the two marks carry their own moments, {gap:.2f} s apart")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "/tmp/kt-live-journal"))

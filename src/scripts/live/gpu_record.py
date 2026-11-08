#!/usr/bin/env python3
"""Live scenario of change gpu-pass-cost: while the real program plays, the telemetry journal gets a
GPU record each busy second — the adapter, the frame and decode size, what a frame cost the device
and each pass — and Diagnostics shows it; at rest there is none.

The owner (kkt-26.11.08 H9, translated): "on my computer everything could work much faster", and
on acceleration: "I am not going to take measurements" — a normal run must carry the numbers. Our
fixture: play, open Diagnostics while playing (its load section shows the GPU line), close it,
pause, the load watch's return to rest. The rig's llvmpipe on GL writes timestamps, so the passes
are named; a device without them gives the frame's submit-to-done time and says so (t2020).
"""
import datetime
import os
import re
import sys

sys.path.insert(0, os.path.dirname(__file__))
from journal import records  # noqa: E402
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

# its own process: the journal lives in the data directory the process started with
OWN_PROCESS = True


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "gpu_record.otio"), [(0.0, 20.0, "synth:Pattern")])
    journal = os.path.join(out, "xdg", "kinetren", "telemetry-journal.log")
    with Live("gpu_record", out, project=proj, own_process=True) as app:
        app.resize(1900, 1000)
        app.expect(
            app.until(lambda: any(k == "RUN" for _, k, _ in records(journal)), timeout=30.0),
            f"the run opened its journal in the rig's data directory ({journal})",
        )
        app.step("start", "the project is open; the journal has its RUN line")

        t0 = datetime.datetime.now().astimezone()
        app.key("space")
        app.wait_playhead(lambda t: t > 90000, "a second played", timeout=20.0)
        app.click_probe("tb.diag", "the gauge button of the toolbar")
        app.expect(app.has_probe("win.diag"), "the gauge opened Diagnostics")
        app.expect(
            app.until(lambda: any(k == "GPU" for _, k, _ in records(journal)), timeout=20.0),
            "a played second wrote a GPU record",
        )
        app.settle()
        app.step("diag", "Diagnostics while playing: the load section ends with “Video card that second: …”")
        app.key("Escape")
        app.expect(app.wait_gone("win.diag"), "Esc closed Diagnostics")
        app.key("space")
        app.until(lambda: not app.playing())
        t1 = datetime.datetime.now().astimezone()
        app.step("paused", "paused")
        # three load-watch seconds after the pause, busy or not: nothing composes in them, so none may
        # carry a GPU record (the UI's own frames are not the device's work this record is about)
        quiet = t1 + datetime.timedelta(seconds=1)
        app.expect(
            app.until(lambda: sum(1 for a, k, _ in records(journal) if k == "LOAD" and a >= quiet) >= 3, timeout=30.0),
            "the load watch went on recording after the pause",
        )
        rest_from = quiet
        app.step("rest", "paused: nothing composes")

        recs = records(journal)
        gpu = [(a, t) for a, k, t in recs if k == "GPU"]
        print("GPU records:")
        for a, t in gpu:
            print(f"  {a.time()}  {t}")
        played = [t for a, t in gpu if t0 <= a <= t1 + datetime.timedelta(seconds=1.5)]
        app.expect(len(played) >= 1, f"the playback's seconds carry GPU records ({len(played)})")
        for t in played[:1]:
            app.expect(re.search(r"\| frame \d+×\d+, decode \d+×\d+ ×1 \|", t) is not None, f"the record names the frame and decode size: {t}")
            app.expect("preview" in t, f"the record counts the preview's frames: {t}")
            app.expect(
                " | " in t and ("ms a frame" in t),
                f"the record gives the device time of a frame: {t}",
            )
            app.expect(
                "no timestamps" in t or "composite" in t,
                f"the record names the passes, or says the device has no timestamps: {t}",
            )
        rest = [t for a, t in gpu if a >= rest_from]
        app.expect(not rest, f"no GPU record in the seconds after the pause, when nothing composes: {rest}")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "/tmp/kt-live-gpu_record"))

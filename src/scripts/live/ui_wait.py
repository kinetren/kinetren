#!/usr/bin/env python3
"""Live scenario ui-never-waits: no interface action waits for a picture.

The owner's words: `docs/ktprompts/kkt-26.11.07.txt` item E8 (an anomalous Datamosh parameter
lagged the whole interface until he could not undo his own effect) and `kkt-26.11.08` H9 (his
journal: the UI thread blocked 0.3-4.5 s inside single frames during playback, at a quarter of a
core). The project is ours: `kt-cli showcase --heavy` (ten releases of a composition under Slowed,
VHS Tape and Camera ISP over a dense edit), with Datamosh at the far end of its range (the
widest motion search, no keyframe for 600 frames, 300 frames of history, mosh on) on every file
event of the first ten seconds. One such frame takes seconds to compose here.

The person plays, opens an event's effects window, adds a Datamosh from the gallery, undoes it
with Ctrl+Z, closes the window and stops. Each consequence is read off the live model, and the
journal's notes of UI frames blocked for 100 ms or more are the verdict.
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
import kt_live  # noqa: E402
from journal import records  # noqa: E402
from kt_live import Live, poll_step, require_tools  # noqa: E402
from readme_shots import fixture_sources  # noqa: E402

OWN_PROCESS = True
SEC = 90000

# The limit IS the check (t1671 lists it): the longest UI frame the journal notes while the
# composites take seconds. The debug build in software rendering draws a heavy frame in a few tens
# of milliseconds; the code before the change stood in its frames for the whole composite, 0.3-3.7 s
# a frame on this project (kt-cli pause-scan), 0.3-4.5 s in the owner's release run.
BLOCK_LIMIT_MS = 400
MOSH = {"kind": "datamosh", "gop": 600, "search": 64, "before": 300, "mosh": 1.0, "mv_scale": 16.0}


def moshed_project(out, sources):
    """The heavy showcase with Datamosh at its extremes on the file events of the first 10 s, and
    the event the person opens."""
    proj = os.path.join(out, "heavy-mosh.kt")
    subprocess.run(
        [kt_live.KT_CLI, "showcase", proj, "--sources", sources, "--heavy", "--force"],
        check=True, capture_output=True,
    )
    doc = json.load(open(proj))
    p = doc["project"]
    files = {m["id"] for m in p["media"] if not m["path"].startswith(("synth:", "gen:", "seq:"))}
    moshed = []
    for c in p["clips"]:
        if c["kind"] == "video" and c.get("media") in files and c["start"] < 10 * SEC:
            c.setdefault("fx", []).insert(0, dict(MOSH))
            moshed.append(c)
    json.dump(doc, open(proj, "w"), indent=1)
    target = max(moshed, key=lambda c: c["duration"]) if moshed else None
    return proj, len(moshed), target


def fx_of(app, clip_id):
    for c in app.geom().get("clips", []):
        if c["id"] == clip_id:
            return c.get("fx")
    return None


def blocked(journal, since):
    """The UI frames the journal noted as blocked since `since`, in ms."""
    out = []
    for at, kind, text in records(journal):
        if at >= since and kind == "NOTE" and text.startswith("the UI thread was blocked for"):
            out.append(int(text.split()[6]))
    return out


def main(out):
    require_tools()
    sources = fixture_sources(os.path.join(out, "sources"))
    proj, n, target = moshed_project(out, sources)
    journal = os.path.join(out, "xdg", "kinetren", "telemetry-journal.log")
    with Live("ui_wait", out, project=proj, own_process=True) as app:
        app.expect(n > 0 and target is not None, f"the fixture moshed {n} events")
        if target is None:
            return app.done()
        app.expect(
            app.until(lambda: any(k == "RUN" for _, k, _ in records(journal)), timeout=30.0),
            f"the run opened its journal ({journal})",
        )
        import datetime
        since = datetime.datetime.now().astimezone()
        app.key("Home")
        app.step("open", f"the heavy showcase with Datamosh at its extremes on {n} events")

        app.key("space")
        app.wait_playhead(lambda t: t > SEC // 2, "playback moves over the moshed events", timeout=60.0)
        app.step("playing", "playing over the moshed events: each frame takes seconds to compose")

        cid = target["id"]
        was = fx_of(app, cid)
        lane = target["lane"]
        x = app.x_at(target["start"] + target["duration"] // 2)
        y = app.lane_y("video", lane, frac=0.6)
        t0 = time.time()
        app.double_click(x, y)
        opened = app.has_probe("fxreg.add")
        app.expect(opened, "a double click opened the event's effects window during playback")
        print(f"   effects window answered in {time.time() - t0:.1f} s")
        app.step("fx-open", "the effects window of a moshed event, opened while it plays")
        if not opened:
            return app.done()

        app.click_probe("fxreg.add", "the picker button")
        for ch in "datamosh":
            app.key(ch)
        cell = None
        for _ in range(40):
            if app.has_probe("fxreg.var.datamosh.0", tries=1):
                cell = True
                break
            poll_step(0.25)
        app.expect(cell is not None, "the gallery shows Datamosh")
        if cell is None:
            return app.done()
        app.click_probe("fxreg.var.datamosh.0", "a Datamosh cell")
        app.expect(app.until(lambda: fx_of(app, cid) == was + 1), "the Datamosh was added to the event")
        app.step("added", "a second Datamosh on the event: its rows in the window")

        t0 = time.time()
        app.key("ctrl+z")
        undone = app.until(lambda: fx_of(app, cid) == was)
        print(f"   Ctrl+Z took effect in {time.time() - t0:.1f} s")
        app.expect(undone, "Ctrl+Z removed the effect while the composites take seconds")
        app.step("undone", "Ctrl+Z: the added Datamosh is gone")

        t0 = time.time()
        app.click_probe("fx.done", "the window's Done button")
        app.expect(app.wait_gone("win.fx"), "Done closed the effects window")
        print(f"   Done closed the window in {time.time() - t0:.1f} s")
        app.key("space")
        app.expect(app.until(lambda: not app.playing()), "Space stopped playback")
        app.step("stopped", "the window closed and playback stopped")

        # the journal writes a long frame when it ends: let the last ones land
        poll_step(1.0)
        ms = blocked(journal, since)
        print(f"   UI frames blocked >= 100 ms: {len(ms)}, longest {max(ms) if ms else 0} ms: {sorted(ms)[-10:]}")
        app.expect(
            not ms or max(ms) < BLOCK_LIMIT_MS,
            f"no UI frame stood {BLOCK_LIMIT_MS} ms or more while the composites took seconds: {sorted(ms)[-10:]}",
        )
        return app.done()


if __name__ == "__main__":
    out_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join("target", "live-out")
    os.makedirs(out_dir, exist_ok=True)
    sys.exit(main(out_dir))

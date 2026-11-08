#!/usr/bin/env python3
"""Live scenario t2045 (decode-request-storm): a short loop over a Datamosh event plays.

The owner's words are in `docs/ktprompts/kkt-26.11.08.txt`, line 14 (registry H8): "а
loop-picture-throughput может связан с тяжелым эффектом? типо я поменял там что-то и оно пошло
считать?" His journal: after he selected Datamosh, 1570 decode requests a second while 15–100
frames were decoded, every one dropped before it was read, and four loops in a row at V-fps 0.

The project is ours (`kt-cli showcase --heavy`): a footage event gets a cut mosh at 1080 lines, and
a 0.4 s loop is set 3.3 s into it, so the frame's history (~95 frames of 8 MB) is larger than the
preview's 512 MB cache. Paused there the history must come in and the pause fall quiet; then the
loop is played. The verdict is the work the program counts: the decoder is asked for bounded work
per frame it decodes (no request storm), and the loop shows frames (V-fps above 0 from the geometry
dump; on llvmpipe a real window's UI frame caps the number itself).
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kt_live  # noqa: E402
from kt_live import Live, require_tools  # noqa: E402
from idle import Meter, wait_quiet  # noqa: E402
from readme_shots import fixture_sources  # noqa: E402

OWN_PROCESS = True
SEC = 90000


def mosh_loop(path):
    """The longest plain footage event gets a cut mosh at 1080 lines; a 0.4 s loop 3.3 s into it."""
    with open(path) as f:
        doc = json.load(f)
    p = doc["project"]
    plain = [c for c in p["clips"] if c["kind"] == "video" and not c.get("fx") and c["duration"] >= 4 * SEC]
    clip = min(plain, key=lambda c: c["start"])
    clip["fx"] = [{"kind": "datamosh", "lines": "1080", "before": 12, "mosh": 1.0}]
    at = clip["start"] + int(3.3 * SEC)
    p["region"] = {"start": at, "end": at + int(0.4 * SEC)}
    p["loop_enabled"] = True
    p["ui"]["cursor"] = at
    with open(path, "w") as f:
        json.dump(doc, f)
    return clip["id"], at


def main(out):
    require_tools()
    sources = fixture_sources(os.path.join(out, "sources"))
    proj = os.path.join(out, "mosh-loop.kt")
    subprocess.run(
        [kt_live.KT_CLI, "showcase", proj, "--sources", sources, "--heavy", "--force"],
        check=True, capture_output=True,
    )
    clip, at = mosh_loop(proj)
    with Live("decode_storm", out, project=proj, own_process=True) as app:
        meter = Meter(app)
        # paused on the loop's start: the mosh's history (~95 frames of 8 MB, past the 512 MB cache)
        # is decoded window by window and computed by the prefetch thread, then the pause is quiet —
        # before decode-request-storm the paused frame asked ~1500 requests a second for ever
        wait_quiet(meter, "after opening")
        work, _ = meter.over(3.0)
        print(f"   paused on the mosh after it settled: {work}")
        app.expect(work.get("decode_requests", 0) <= 5 and work.get("decoded", 0) <= 5,
                   f"the paused frame asks for nothing more once its history is in: {work}")
        app.step("open", f"the heavy showcase is open: event {clip} carries a cut mosh, a 0.4 s loop over it")

        app.key("space")
        app.until(lambda: app.playing(), timeout=10.0)
        app.expect(app.playing(), "playback started (no audio device?)")
        app.step("playing", "the loop plays; the mosh's history is decoded and computed")

        # while the history warms, the decoder is asked for what it decodes, not for the whole
        # history again on every composite (the owner's journal: 1570 requests for 15–100 decoded)
        g0 = app.geom()
        storms, windows, shown = [], 0, 0
        while windows < 60 and shown == 0:
            work, _ = meter.over(3.0)
            windows += 1
            asked, decoded = work.get("decode_requests", 0), work.get("decoded", 0)
            print(f"   window {windows}: {asked} requests, {decoded} decoded, {work.get('composites', 0)} composites")
            if asked > 3 * max(decoded, 1) + 60:
                storms.append((asked, decoded))
            shown = app.geom()["compose"]["shown"] - g0["compose"]["shown"]
        app.expect(not storms, f"no window asks far more than it decodes: {storms}")
        app.expect(shown > 0, f"the loop shows frames within {windows * 3} s (V-fps 0)")
        app.step("moving", "the moshed loop moves on screen")

        g1 = app.geom()
        work, cpu = meter.over(6.0)
        g2 = app.geom()
        vfps = (g2["compose"]["shown"] - g1["compose"]["shown"]) / 6.0
        print(f"   playing the moshed loop: {work}, {cpu:.0f} % of a core, V-fps {vfps:.1f}")
        app.expect(vfps > 0, f"the loop keeps showing frames: V-fps {vfps:.1f}")
        app.key("space")
        return app.done()


if __name__ == "__main__":
    out_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join("target", "live-out")
    os.makedirs(out_dir, exist_ok=True)
    sys.exit(main(out_dir))

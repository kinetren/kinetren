#!/usr/bin/env python3
"""Live scenario R.63 (item 13): a jump into a long file does not mute the sound.

The owner’s complaint verbatim: “I simply jumped to a section with one long
event (there are others above it, and this one has effects), but that is no
reason to load the video so slowly with such underruns”. By the flight log, each
jump gave 1-6 s of complete silence at the cost of the render itself at 2 % of real
time.

Here this is checked in a real window: jumps across a long H.264 during playback,
and the underrun counter is read FROM THE PROGRAM ITSELF (the geometry dump), not from the
picture.

THE OWNER’S FILE — ONLY ON REQUEST (R.68). The scenario was set up in R.63 and
silently picked up `sources/kt-untitled-review-870s.mp4`, simply because
it lay on the machine. In R.67 the owner said directly: “item 1 but not on my
projects” — the run goes on OUR fixtures. The second run remains a
diagnostic and is enabled explicitly:

    KT_OWNER_MEDIA=/path/to/long.mp4 ./scripts/live-check.sh seekjump

HONEST CAVEAT (R.63): on THIS machine the scenario is green even on the code BEFORE
the fix — it was not possible to reproduce the starvation even with three layers on top of
a long event. So it is a guard against regression, not proof of the
fix: the proof is the engine test t758 (on the previous code 0 of 8
bursts in 250 ms) and the owner’s decode cost measurements (audio burst
14 µs versus 22-61 ms per 1080p frame). The owner’s scene was heavier:
six video tracks with effects, four media, GPU composite.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, pause, poll_step, project_with_clips, require_tools  # noqa: E402


def stack_tracks(dest, copies):
    """Stack ANOTHER N video tracks with the same media: the owner had others lying above the long
    event, and that is exactly how the span gains its length — the order goes
    across several layers at once."""
    doc = json.load(open(dest))
    vid = [t for t in doc["tracks"]["children"] if t.get("kind") == "Video"]
    base = vid[0]
    nid = 100
    for _ in range(copies):
        clone = json.loads(json.dumps(base))
        for c in clone["children"]:
            if c.get("OTIO_SCHEMA", "").startswith("Clip"):
                nid += 1
                c["metadata"]["kt"]["id"] = nid
        doc["tracks"]["children"].append(clone)
    json.dump(doc, open(dest, "w"), indent=4)
    return dest

SEC = 90000
# the path is NOT hardcoded: someone else’s (even the owner’s) video does not enter the normal run
OWNER_FILE = os.environ.get("KT_OWNER_MEDIA", "")


def long_media(out, seconds):
    """Long H.264 1080p: the frame is expensive, as the owner’s is."""
    path = os.path.join(out, f"long{seconds}.mp4")
    if os.path.exists(path):
        return path
    subprocess.run(
        [
            "ffmpeg", "-v", "error", "-y",
            "-f", "lavfi", "-i", f"testsrc=size=1920x1080:rate=30:duration={seconds}",
            "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000",
            "-shortest",
            "-c:v", "libx264", "-preset", "veryfast", "-g", "60", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-b:a", "128k",
            path,
        ],
        check=True,
    )
    return path


def run_case(app, name, dur_s, jumps):
    """Jumps during playback: after each one the sound must go on without starvation.

    We jump with the TIMECODE MASK (Ctrl+L), not by clicking the ruler: the montage is
    320 s wider than the window, and `x_at` would return a coordinate off the screen — exactly
    the trap recorded in R.37. The mask is the real human way
    to get to an arbitrary place."""
    app.key("space")
    pause(
        1.5,
        "playback runs in REAL time: one and a half seconds of play are needed "
        "for the cursor to really move away from the start",
    )
    base = app.geom().get("underruns", 0)
    worst = 0
    for frac in jumps:
        secs = int(dur_s * frac)
        app.key("ctrl+l")
        for ch in str(secs):
            app.key(ch)
        app.key("Return")
        # the sound must go FAST: we give twice as much output ring
        pause(
            1.2,
            "the output ring lives in REAL time: we wait twice its "
            "length to tell “went fast” from “went at all”",
        )
        poll_step(1.2)
        now = app.geom().get("underruns", 0)
        grew = now - base
        worst = max(worst, grew)
        base = now
        print(f"      {name}: jump of {frac:.0%} — underruns increased by {grew}")
    app.key("space")
    # threshold: half a second of silence (24 000 frames) — this is already audible; before R.63
    # each jump gave 48 000 frames per second, and so on for several seconds
    app.expect(
        worst < 24_000,
        f"{name}: the jump does not mute the sound (worst underrun increase {worst})",
    )


def main(out):
    require_tools()
    dur = 320
    media = long_media(out, dur)
    path = project_with_clips(os.path.join(out, "seekjump.otio"), [(0.0, float(dur), media)])
    stack_tracks(path, 2)  # three layers of one media: the order is wider, the span is longer
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("seekjump", out, project=path) as app:
        app.expect(len(app.live_clips()) >= 1, f"event on the montage: {app.live_clips()}")
        run_case(app, "synthetic 320 s", dur, [0.9, 0.5, 0.75])
        app.shot("after-jumps")
        # the verdict of the FIRST pass is taken here: before R.68 it was lost entirely —
        # when the second pass was skipped the scenario returned zero regardless of
        # what it had found (the same class as “no return app.done()”, R.67)
        rc = app.done()
    if rc:
        return rc

    if not OWNER_FILE or not os.path.exists(OWNER_FILE):
        print("   (the second run — on request: KT_OWNER_MEDIA=<long file>)")
        return 0

    # THE VERY file on which the defect was observed
    path2 = project_with_clips(os.path.join(out, "owner.otio"), [(0.0, 870.0, OWNER_FILE)])
    stack_tracks(path2, 2)
    with Live("seekjump-owner", out, project=path2) as app:
        run_case(app, "review 870 s", 870, [0.94, 0.6, 0.2])
        app.shot("owner-after-jumps")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/seekjump"))

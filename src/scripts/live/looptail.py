#!/usr/bin/env python3
"""Live scenario loop-pitch-tail-dip: a loop over an event with an echo before its pitch.

Found by pitch-tail-choice: a loop region starting at an event whose chain has an echo BEFORE the
pitch began every lap with a dip — ≈ 75 ms of silence with "rings on", half the level with "cut".
The sound itself is proven by t1440/t1441 (the harness hears nothing); here the person's path runs in
the program they launch, on OUR fixture: a 440 Hz event with an echo and a pitch (+7, "rings on"),
a drag on the ruler from the event's start to past its end, L, Space — the cursor laps the region
and the program keeps playing; then the same place on repeat through the engine (`kt-cli probe
--time`) plays its laps without a miss.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, require_tools  # noqa: E402
from kt_live import _clip, _gap, new_otio  # noqa: E402

SEC = 90000
START, LEN, PAST = 1.0, 0.6, 0.4


def build(path):
    new_otio(path)
    with open(path) as f:
        doc = json.load(f)
    doc["metadata"]["kt"]["settings"].update(fps=30, width=1280, height=720)
    doc["metadata"]["kt"]["bin_media"] = []
    clip = _clip(1, LEN, "synth:sine:440", 1)
    for kt in ({"kind": "delay", "time_ms": 100.0, "feedback": 0.3, "mix": 0.5},
               {"kind": "pitch_shift", "semitones": 7.0, "echo_before": "ring"}):
        clip["effects"].append({
            "OTIO_SCHEMA": "Effect.1", "name": "", "effect_name": "kt:" + kt["kind"], "enabled": True,
            "metadata": {"kt": kt},
        })
    track = {
        "OTIO_SCHEMA": "Track.1", "name": "", "kind": "Audio", "children": [_gap(START), clip],
        "source_range": None, "effects": [], "markers": [], "enabled": True, "metadata": {},
    }
    doc["tracks"]["children"] = [t for t in doc["tracks"]["children"] if t.get("kind") != "Audio"] + [track]
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)


def main(out):
    require_tools()
    path = os.path.join(out, "looptail.otio")
    build(path)
    s, e = START, START + LEN + PAST
    with Live("looptail", out, project=path) as app:
        p = app.project_now()
        app.expect(len(p.clips) == 1, f"one audio event: {p.clips}")
        x0, x1 = app.x_at(int(s * SEC)), app.x_at(int(e * SEC))
        app.expect(0 < x0 < x1, f"the region's ends are on screen: x {x0}…{x1}")
        app.step("open", "the project: one event at 1.0 s with Delay and Pitch Shift (rings on)")
        app.drag(x0, app.ruler_y(), x1, app.ruler_y())
        app.step("region", "drag on the ruler 1.0 s → 2.0 s: the region from the event's start to past its end")
        app.key("l")
        app.step("loop", "L: the loop is on")
        app.key("space")
        app.wait_playhead(lambda t: t > (s + 0.2) * SEC, "the cursor moved", timeout=10)
        if not app.playing():
            app.expect(False, "playback did not start (no audio device?)")
            return app.done()
        seen = []
        for lap in (1, 2, 3):
            seen += app.wait_playhead(lambda t: t > (e - 0.25) * SEC, f"lap {lap}: the cursor near the region end", timeout=10)
            seen += app.wait_playhead(lambda t: t < (s + 0.25) * SEC, f"wrap {lap}: the cursor back at the event's start", timeout=10)
        app.expect(app.playing(), "the program still plays after three wraps")
        app.expect(all(t <= e * SEC + SEC // 2 for t in seen), f"the cursor stayed in the region: max {max(seen)}")
        app.step("laps", "Space: three laps, each starting at the event's start, playback goes on")
        app.key("space")
        ok = app.done()
    # the same place on repeat through the engine: four laps, no miss (the cost is judged on release)
    run = subprocess.run([KT_CLI, "probe", app.project, str(s), str(e), "--time"], capture_output=True, text=True)
    looped = [ln for ln in run.stdout.splitlines() if ln.strip().startswith("looped")]
    print(f"   {looped[0].strip() if looped else run.stdout[-300:]}")
    if run.returncode != 0 or not looped or not looped[0].rstrip().endswith("underruns 0"):
        print("   FAILURE the looped engine run of the region has misses or did not run")
        return 1
    print("   ok   the engine plays the region's laps without a miss")
    return ok


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

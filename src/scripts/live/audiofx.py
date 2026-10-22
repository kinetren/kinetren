#!/usr/bin/env python3
"""Live scenario R.47 (item 330): new audio effects are reachable from the FX window
and survive saving.

The demo project carries a Filter slot on the audio track (an edit of .otio by our own
library: the format is one). A double click on an event opens the FX window —
there is a chain strip with “Filter” and a grid of parameters (Cutoff/Resonance).
Check of the consequence: (1) the window opened (the screen changed), (2) ctrl+s —
the kt:filter slot with cutoff_hz survived a round trip through the real program
(the UI save neither loses nor distorts the new kinds of effects).
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401

SEC = 90000


def main(out):
    require_tools()
    proj = os.path.join(out, "audiofx.otio")
    demo_otio(proj)
    # to the first AUDIO event of the demo project we append a Filter slot with
    # a non-default cutoff — by it we find our slot after the save
    with open(proj) as f:
        doc = json.load(f)
    audio_clip = None
    for tr in doc["tracks"]["children"]:
        if tr.get("kind") == "Audio":
            for ch in tr["children"]:
                if ch.get("OTIO_SCHEMA", "").startswith("Clip"):
                    audio_clip = ch
                    break
        if audio_clip:
            break
    assert audio_clip is not None, "demo project must have an audio clip"
    audio_clip.setdefault("effects", []).append({
        "OTIO_SCHEMA": "Effect.1",
        "name": "",
        "effect_name": "kt:filter",
        "enabled": True,
        "metadata": {
            "kt": {
                "kind": "filter",
                "mode": "lowpass",
                "cutoff_hz": 1234.0,
                "resonance": 2.5,
            }
        },
    })
    with open(proj, "w") as f:
        json.dump(doc, f, indent=4)

    with Live("audiofx", out, project=proj) as app:
        y = app.lane_y("audio", 0, frac=0.5)
        before = app.shot("open")
        x = app.x_at(3 * SEC)
        app.double_click(x, y)
        app.shot("fx-window")
        d = app.screen_diff(before)
        app.expect(d > 0.01, f"double click opened the FX window (the screen changed by {d:.1%})")

        # round trip through the real program: the slot survived and was not distorted
        saved = app.saved_project().doc  # ctrl+s — the file is overwritten by the program
        slots = [
            e
            for tr in saved["tracks"]["children"]
            for ch in tr.get("children", [])
            for e in ch.get("effects", [])
            if e.get("effect_name") == "kt:filter"
        ]
        app.expect(bool(slots), "the kt:filter slot survived the save by the program")
        if slots:
            kt = slots[0]["metadata"]["kt"]
            app.expect(
                abs(kt.get("cutoff_hz", 0) - 1234.0) < 1e-6,
                f"cutoff_hz saved without distortion: {kt.get('cutoff_hz')}",
            )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

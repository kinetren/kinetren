#!/usr/bin/env python3
"""Live scenario fx-graph v1a (kkt-26.11.18b U3): a morpher layer's device sound switches off alone.

The owner (kkt-26.11.18b line 5): adding Datamosh / VHS Tape / ISP with a morpher, there is no way
to turn the effect's morph off for the audio. The scene is OURS (R.67): a video event and its linked audio under a morpher
layer carrying VHS Tape on the picture, written as a file of the time before the patch held both
halves (only the picture's tape).

PATH (morpher spec, "The sound half's switch in the morpher window"):
1. double click the morpher block → the morpher window: "VHS Tape" and its sound half marked with
   the speaker in the chain (the file held only the picture; the reading gave the layer its sound);
2. click the sound half's checkbox → the half is struck through, the picture half stays on;
3. the saved file holds the sound half switched off and the picture's tape on.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, require_tools  # noqa: E402

SEC = 90000


def layer_with_tape(dest):
    clip = lambda i, kind: {  # noqa: E731
        "id": i, "link_group": 1, "kind": kind, "lane": 0, "start": 0,
        "duration": 10 * SEC, "source_in": 0, "media": 1, "gain_db": 0.0, "fx": [],
    }
    project = {
        "format_version": 1,
        "settings": {"fps": 25, "width": 640, "height": 360, "sample_rate": 48000},
        "media": [{"id": 1, "path": "synth:ramp"}],
        "clips": [clip(1, "video"), clip(2, "audio")],
        "tracks": [{"id": 1, "kind": "video"}, {"id": 2, "kind": "audio"}, {"id": 3, "kind": "morph"}],
        "morphers": [{
            "id": 1, "lane": 0, "start": 0, "duration": 10 * SEC,
            "targets": {"on": "clips", "ids": [1, 2]},
            "patch": {"video_fx": [{"kind": "vhs_tape"}]},
        }],
        "overlap_layers": True,
    }
    with open(dest, "w") as f:
        json.dump({"kinetren": 1, "project": project}, f, indent=2)
    return dest


def patches(text):
    """The morpher patches of the saved project."""
    found = []

    def walk(node):
        if isinstance(node, dict):
            if "patch" in node and isinstance(node["patch"], dict):
                found.append(node["patch"])
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)

    walk(json.loads(text))
    return found


def kinds(chain):
    return [s.get("kind") for s in chain or []]


def main(out):
    require_tools()
    proj = layer_with_tape(os.path.join(out, "morph_pair.kt"))

    with Live("morph_pair", out, project=proj) as app:
        y = app.lane_y("morph", 0, frac=0.5)
        app.double_click(app.x_at(5 * SEC), y)
        app.expect(app.until(lambda: any(f["param"] == "fx.slot.1.on" for f in app.geom().get("fields", []))),
                   "the morpher window's chain shows two slots (the tape and its sound half)")
        app.step("window", "double click on the morpher block → its chain: VHS Tape and its sound half (🔊 VHS Tape)")

        if not app.click_probe("fx.slot.1.on", "the sound half's checkbox"):
            return app.done()
        app.expect(app.until(lambda: any(
            (p.get("audio_bypass") or [False])[0] for p in patches(app.saved_text()))),
            "the sound half switched off in the model")
        app.step("off", "the sound half's checkbox → it is struck through; the picture's tape stays on")

        ps = patches(app.saved_text())
        app.expect(len(ps) == 1, f"one layer ({len(ps)})")
        if ps:
            p = ps[0]
            app.expect(kinds(p.get("video_fx")) == ["vhs_tape"], f"the picture holds the tape: {kinds(p.get('video_fx'))}")
            app.expect(kinds(p.get("audio_fx")) == ["vhs_tape_sound"], f"the layer holds its sound half: {kinds(p.get('audio_fx'))}")
            app.expect((p.get("audio_bypass") or [False])[0] is True, "the sound half is switched off")
            app.expect(not (p.get("video_bypass") or [False])[0], "the picture half stayed on")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

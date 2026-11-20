#!/usr/bin/env python3
"""Live scenario track-device-sound (kkt-26.11.18b line 29): VHS Tape on a whole
video track sounds on the track's sound track, with its own switch.

The scene is OURS (R.67): a video event and its linked audio on Video 1 / Audio 1, no effects.

PATH (ui-reach spec, "The tape on a track and its sound half's switch"):
1. the FX badge of Video 1 → the track window opens;
2. add VHS Tape from the graph's picker → the graph shows the picture lane and the sound lane with
   the tape's two halves in one column, and the row "Device sound: Audio 1" appears;
3. click the sound half's switch → it is off, the picture half stays on;
4. pick "None" in the row → the sound lane goes, the row stays; the saved file has no link.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, require_tools  # noqa: E402

SEC = 90000


def scene(dest):
    clip = lambda i, kind: {  # noqa: E731
        "id": i, "link_group": 1, "kind": kind, "lane": 0, "start": 0,
        "duration": 10 * SEC, "source_in": 0, "media": 1, "gain_db": 0.0, "fx": [],
    }
    project = {
        "format_version": 1,
        "settings": {"fps": 25, "width": 640, "height": 360, "sample_rate": 48000},
        "media": [{"id": 1, "path": "synth:ramp"}],
        "clips": [clip(1, "video"), clip(2, "audio")],
        "tracks": [{"id": 1, "kind": "video"}, {"id": 2, "kind": "audio"}],
        "overlap_layers": True,
    }
    with open(dest, "w") as f:
        json.dump({"kinetren": 1, "project": project}, f, indent=2)
    return dest


def tracks(text):
    """(video track, audio track) of the saved project."""
    ts = json.loads(text)["project"]["tracks"]
    v = next(t for t in ts if t.get("kind") == "video")
    a = next(t for t in ts if t.get("kind") == "audio")
    return v, a


def kinds(t):
    return [s.get("kind") for s in t.get("fx") or []]


def has(app, name):
    return any(f["param"] == name for f in app.geom().get("fields", []))


def main(out):
    require_tools()
    proj = scene(os.path.join(out, "track_sound.kt"))

    with Live("track_sound", out, project=proj) as app:
        if not app.click_probe("track.v1.fx", "the FX badge of Video 1"):
            return app.done()
        app.expect(app.has_probe("track.name"), "the track window opened")
        app.step("window", "the FX badge of Video 1 → the track window with its graph")

        app.click_probe("fxreg.add", "the graph's picker")
        for ch in "vhs":
            app.key(ch)
        app.click_probe("fxreg.tpl.vhs_tape", "“VHS Tape” in the picker")
        app.expect(app.until(lambda: has(app, "fx.slot.1.on") and has(app, "track.sound")),
                   "the graph shows the tape's sound half and the Device sound row")
        v, a = tracks(app.saved_text())
        app.expect(kinds(v) == ["vhs_tape"], f"the video track holds the tape: {kinds(v)}")
        app.expect(v.get("sound") == a.get("id"), f"the sound track is Audio 1: {v.get('sound')}")
        app.expect(kinds(a) == ["vhs_tape_sound"], f"Audio 1 holds the tape's sound half: {kinds(a)}")
        app.step("tape", "VHS Tape added → the picture and sound lanes, the halves in one column, “Device sound: Audio 1”")

        app.click_probe("fx.slot.1.on", "the sound half's switch")
        app.expect(app.until(lambda: (tracks(app.saved_text())[1].get("fx_bypass") or [False])[0] is True),
                   "the sound half switched off on Audio 1")
        v, _ = tracks(app.saved_text())
        app.expect(not (v.get("fx_bypass") or [False])[0], "the picture half stayed on")
        app.step("off", "the sound half's switch → it is off, the picture's tape stays on")

        app.click_probe("track.sound", "the Device sound row")
        app.click_probe("track.sound.none", "“None”")
        app.expect(app.until(lambda: "sound" not in tracks(app.saved_text())[0]), "the link is cleared")
        _, a = tracks(app.saved_text())
        app.expect(kinds(a) == [], f"clearing took the half off Audio 1: {kinds(a)}")
        app.expect(not has(app, "fx.slot.1.on"), "the sound lane went")
        app.expect(has(app, "track.sound"), "the row stays while the tape is on the track")
        app.step("none", "“None” in the row → the sound lane goes, the row stays")
        app.click_probe("track.close", "close the track window")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

#!/usr/bin/env python3
"""Live scenario pitch-tail-choice (kkt-26.11.01 item 15): the pitch's echo choice in the REAL program.

The harness (t1337) picks the choice inside one process; here the person's path runs in the program
they launch, on OUR fixture: an audio event of the demo project carries an echo and a pitch after
it; a double click opens its effects, the pitch slot shows the line "Echo of the effects before the
pitch: cut at the event's end", the person picks "rings on after the event", and Ctrl+S writes the
choice into the project — a format-3 file, which an older build refuses instead of cutting the echo.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, require_tools  # noqa: E402
from kt_live import demo_otio  # noqa: E402

SEC = 90000


def pitch_slots(text):
    """The pitch_shift slots of the saved project."""
    found = []

    def walk(node):
        if isinstance(node, dict):
            if node.get("kind") == "pitch_shift":
                found.append(node)
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)

    walk(json.loads(text))
    return found


def main(out):
    require_tools()
    proj = os.path.join(out, "pitchtail.otio")
    demo_otio(proj)
    # the first AUDIO event of the demo project gets an echo and a pitch after it
    with open(proj) as f:
        doc = json.load(f)
    clip = None
    for tr in doc["tracks"]["children"]:
        if tr.get("kind") == "Audio":
            clip = next((ch for ch in tr["children"] if ch.get("OTIO_SCHEMA", "").startswith("Clip")), None)
        if clip:
            break
    assert clip is not None, "the demo project has an audio event"
    for kt in ({"kind": "delay", "time_ms": 120.0, "feedback": 0.3, "mix": 0.4},
               {"kind": "pitch_shift", "semitones": 5.0}):
        clip.setdefault("effects", []).append({
            "OTIO_SCHEMA": "Effect.1", "name": "", "effect_name": "kt:" + kt["kind"], "enabled": True,
            "metadata": {"kt": kt},
        })
    with open(proj, "w") as f:
        json.dump(doc, f, indent=4)

    with Live("pitchtail", out, project=proj) as app:
        y = app.lane_y("audio", 0, frac=0.5)
        app.double_click(app.x_at(3 * SEC), y)
        app.step("fxwin", "double click on the audio event → its effects: Delay, Pitch Shift")
        if not app.click_probe("fx.slot.1", "the Pitch Shift slot on the strip"):
            return app.done()
        app.wait_frame(2, what="the pitch panel", soft=True)
        app.step("pitch", "the Pitch Shift slot → under the semitones: “Echo of the effects before the pitch: cut at the event's end”")
        if not app.click_probe("fx.echo_before", "the echo line"):
            return app.done()
        app.wait_frame(2, what="the choice list", soft=True)
        app.step("choices", "open the line → two choices: cut at the event's end / rings on after the event")
        if not app.click_probe("fx.echo_before.1", "the choice “rings on after the event”"):
            return app.done()
        app.wait_frame(2, what="the line redrawn", soft=True)
        app.step("rings", "“rings on after the event” → the line shows it")

        text = app.saved_text()
        slots = pitch_slots(text)
        app.expect(len(slots) == 1, f"the file holds one pitch slot ({len(slots)})")
        if slots:
            app.expect(slots[0].get("echo_before") == "ring", f"the choice is in the file ({slots[0].get('echo_before')})")
        app.expect('"kinetren": 3' in text, "the project is format 3: an older build refuses it")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

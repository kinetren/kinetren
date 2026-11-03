#!/usr/bin/env python3
"""Live scenario sim-audio-pairs (kkt-26.11.03b D1): the VHS deck's sound in the REAL program.

A video event and its linked audio. The person opens the pair's effects, adds "VHS Tape" from the
picker (to either half: the other gets its counterpart), and sees the deck face with its Sound
section on both chains; "Normal (linear)" and "Dolby B" on the face reach both
halves — and the saved file holds a `vhs_tape` slot on the picture and a `vhs_tape_sound` slot on
the audio with the same deck rows.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, require_tools  # noqa: E402
from kt_live import project_with_clips  # noqa: E402

SEC = 90000


def linked_av(dest):
    """One 10 s event on the video track and the same media on an audio track, linked."""
    project_with_clips(dest, [(0.0, 10.0, "synth:Ramp")])
    with open(dest) as f:
        doc = json.load(f)
    video = next(t for t in doc["tracks"]["children"] if t.get("kind") == "Video")
    clip = video["children"][0]
    clip["metadata"]["kt"]["link_group"] = 1
    audio_clip = json.loads(json.dumps(clip))
    audio_clip["name"] = "a1"
    audio_clip["metadata"]["kt"]["id"] = 2
    doc["tracks"]["children"].append(
        {
            "OTIO_SCHEMA": "Track.1",
            "name": "",
            "kind": "Audio",
            "children": [audio_clip],
            "source_range": None,
            "effects": [],
            "markers": [],
            "enabled": True,
            "metadata": {},
        }
    )
    with open(dest, "w") as f:
        json.dump(doc, f, indent=4)
    return dest


def slots(text, kind):
    """The slots of `kind` in the saved project."""
    found = []

    def walk(node):
        if isinstance(node, dict):
            if node.get("kind") == kind:
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
    proj = os.path.join(out, "vhs_sound.otio")
    linked_av(proj)

    with Live("vhs_sound", out, project=proj) as app:
        y = app.lane_y("video", 0, frac=0.5)
        app.double_click(app.x_at(3 * SEC), y)
        app.step("fxwin", "double click on the video event → the effects window of the A/V pair")

        # the window holds both halves' chains; either "+ effect…" offers VHS Tape — the picture's
        # tape or the deck's sound — and either puts the deck on both
        if not app.click_probe("fxreg.add", "a picker button"):
            return app.done()
        for ch in "vhs tape":
            app.key("space" if ch == " " else ch)
        app.step("picker", "+ effect, typed “vhs tape” → the gallery shows VHS Tape")
        cell = None
        for _ in range(40):
            params = {f["param"] for f in app.geom().get("fields", [])}
            cell = next((p for p in ("fxreg.tpl.vhs_tape", "fxreg.tpl.vhs_tape_sound") if p in params), None)
            if cell:
                break
            poll_step(0.25)
        app.expect(cell is not None, "the picker shows the VHS Tape row")
        if cell is None or not app.click_probe(cell, "the VHS Tape row"):
            return app.done()
        names = set()
        for _ in range(40):
            names = {f["param"] for f in app.geom().get("fields", [])}
            if "deck.audio_out.1" in names:
                break
            app.wait_frame(1, what="the deck face with its sound section", soft=True)
        app.step("deck-sound", "picked → the deck face; below the heads its Sound section: Hi-Fi / Normal / Mix, the audio head")
        for p in ("deck.audio_out.0", "deck.audio_out.1", "deck.linear_tracks.1"):
            app.expect(p in names, f"the face shows “{p}”")

        # the deck is on both halves already: the saved file says so before any sound edit
        text = app.saved_text()
        app.expect(len(slots(text, "vhs_tape")) == 1, "the picture got the tape")
        app.expect(len(slots(text, "vhs_tape_sound")) == 1, "the linked audio got the deck's sound")

        if not app.click_probe("deck.audio_out.1", "Normal (linear)"):
            return app.done()
        app.wait_frame(2, what="Normal pressed", soft=True)
        app.step("normal", "Normal (linear) → the deck plays its linear track: duller at LP, its hiss, the capstan's flutter")
        app.click_probe("deck.speed.1", "the LP switch")
        app.wait_frame(2, what="LP redrawn", soft=True)
        app.step("lp", "LP → the picture is noisier, the linear sound loses its top")

        text = app.saved_text()
        pic = slots(text, "vhs_tape")
        snd = slots(text, "vhs_tape_sound")
        app.expect(len(pic) == 1 and len(snd) == 1, f"one deck on each half ({len(pic)}, {len(snd)})")
        if pic and snd:
            for row, want in (("audio_out", "normal"), ("speed", "lp")):
                app.expect(pic[0].get(row) == want, f"the picture's deck: {row} = {pic[0].get(row)}")
                app.expect(snd[0].get(row) == want, f"the sound's deck: {row} = {snd[0].get(row)}")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

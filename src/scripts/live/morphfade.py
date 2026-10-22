#!/usr/bin/env python3
"""Live scenario R.55 (C2): morpher “by time” on an event WITH A FADE.

The owner’s complaint verbatim: “also on events with a transition (like a fade)
the morpher breaks and applies the edits to the whole event instead of “by time”.”

The cause was in the CUT: the layer cut the target into pieces, cutting inside the
transition window is not allowed, and the boundary was pushed out of the forbidden zone to the NEAREST
edge — that is, it drifted where the person did not ask. The cure is not to cut
the chain at all, but to mute its contribution with the strength curve.

What is checked here in the REAL program, and what is not visible from the harness:
the layer starts acting EXACTLY where its window stands, not from the edge
of the fade. The scene is chosen so the difference is visible to the EYE: a fade-in
3 s long, the layer window starts at 2.0 s — that is, in the INNER half of the
forbidden zone, where the frame is still bright. The old code pushed the boundary to 3.0 s,
and at the point 2.5 s the layer did not work; the new one works, because the person
asked from 2.0.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000
FADE_S = 3.0
WIN_S = 2.0
WIN_LEN_S = 4.0


def build(dest):
    """Clip 8 s with a 3 s fade-in and a morpher-LAYER [2.0, 6.0) in the Edges patch."""
    project_with_clips(dest, [(0.0, 8.0, "synth:Ramp")])
    with open(dest) as f:
        doc = json.load(f)
    track = next(t for t in doc["tracks"]["children"] if t.get("kind") == "Video")
    track["children"].insert(
        0,
        {
            "OTIO_SCHEMA": "Transition.1",
            "name": "",
            "transition_type": "SMPTE_Dissolve",
            "in_offset": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 90000.0},
            "out_offset": {
                "OTIO_SCHEMA": "RationalTime.1",
                "value": round(FADE_S * SEC),
                "rate": 90000.0,
            },
            "metadata": {},
        },
    )
    kt = doc["metadata"]["kt"]
    kt["morph_lanes"] = 1
    # the morph row ON TOP: the default puts it below, and “everything below it”
    # would be empty (finding 3 of R.35)
    kt["lane_order"] = [["morph", 0], ["video", 0], ["audio", 0]]
    kt["morphers"] = [
        {
            "id": 1,
            "lane": 0,
            "start": int(WIN_S * SEC),
            "duration": int(WIN_LEN_S * SEC),
            "patch": {"video_fx": [{"kind": "edges", "threshold": 24}]},
        }
    ]
    kt["v"] = 4
    with open(dest, "w") as f:
        json.dump(doc, f, indent=2)
    return dest


def main(out):
    require_tools()
    proj = build(os.path.join(out, "morphfade.otio"))

    with Live("morphfade", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 1, f"start: one clip, {p.clips}")
        app.expect(len(p.transitions) == 1, f"fade read from the file: {p.transitions}")

        # --- scene: the probe point INSIDE the fade window and INSIDE the layer window -----
        # the brightness of the frame cannot be compared here with another POINT IN TIME:
        # `synth:Ramp` changes colour from pts, and the fade multiplies by alpha. But
        # Edges on a SOLID frame gives black at any alpha — and this is the
        # absolute sign that “the layer fired” (the same device as in morph.py).
        fade_end = p.transitions[0]["out"]
        app.expect(
            WIN_S * SEC < 2.5 * SEC < fade_end,
            f"the scene degenerated: the 2.5 s probe must lie BOTH inside the layer window "
            f"(from {WIN_S} s), AND inside the fade (up to {fade_end / SEC:.1f} s)",
        )

        # --- MAIN: the layer acts EXACTLY from its own window -----------------
        # The old code pushed the boundary to the edge of the fade (3.0 s), and here
        # Edges did not work: the frame stayed coloured.
        app.click(app.x_at(int(2.5 * SEC)), app.ruler_y())
        app.shot("inside-window-inside-fade")
        inside = app.brightness(app.preview_center())
        app.expect(
            inside < 12,
            f"the layer did not act from {WIN_S} s, although the window stands there: brightness "
            f"{inside:.1f} instead of black — the boundary drifted to the edge of the fade",
        )

        # --- and does NOT act outside its own window -------------------------------
        app.click(app.x_at(int(7.0 * SEC)), app.ruler_y())
        app.shot("after-window")
        after = app.brightness(app.preview_center())
        app.expect(
            after > 30,
            f"after the layer window the frame must return colored: {after:.1f}",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

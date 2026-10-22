#!/usr/bin/env python3
"""Live scenario R.37: slowed VIDEO.

The user’s words: “slowed&reverb for video (a separate checkbox by
default…)” and “slowed&reverb must be taken into account in the audio/video preview of
events”.

A synthetic frame — a solid color from the pts, so “which frame is on screen”
is read straight from the pixel of the preview. We check what the harness would not show:
in a REAL window the picture of the slowed event really goes
more slowly, and removing the slot checkbox returns the normal speed.

We measure DIRECTLY: which frame is shown. A difference metric cannot be used here — the
ramp color is not monotonic across channels and wraps around, so
“the frames diverged more” does not mean “more material has passed”.

Frame blending is deliberately NOT checked here: at 30 fps the neighboring frames of the
ramp differ by ~2/255, and the blend is indistinguishable by eye from the nearest one and
in the pixel. Byte for byte this is covered by test 250, where the blend is checked against
`dissolve_into` directly.

Events (all from one source, so that the colors are comparable):
  0–4 s   ordinary clip
  4–8 s   0.5×
  8–12 s  0.5×, slot OFF (this is the “separate checkbox”)

All 12 s must fit into the view at the starting zoom: `x_at` only handles
what is on screen, and a click outside the visible range goes to the edge.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def build(dest):
    project_with_clips(
        dest,
        [(0.0, 4.0, "synth:Ramp"), (4.0, 4.0, "synth:Ramp"), (8.0, 4.0, "synth:Ramp")],
    )
    with open(dest) as f:
        doc = json.load(f)
    doc["metadata"]["kt"]["v"] = 4

    def node(enabled):
        # the clip effect lives in the OTIO `effects` array, not in metadata.kt:
        # the reader takes `kt` from the metadata of the EFFECT itself, and bypass is the
        # removed `enabled`
        return {
            "OTIO_SCHEMA": "Effect.1",
            "name": "",
            "effect_name": "kt:slowed_reverb",
            "enabled": enabled,
            "metadata": {
                "kt": {
                    "kind": "slowed_reverb",
                    "rate": 0.5,
                    "mix": 0.0,
                    "reverb_delay_ms": 85.0,
                }
            },
        }

    plan = [None, node(True), node(False)]
    for track in doc["tracks"]["children"]:
        clips = [
            c
            for c in track.get("children", [])
            if str(c.get("OTIO_SCHEMA", "")).startswith("Clip")
        ]
        if len(clips) != len(plan):
            continue
        for c, n in zip(clips, plan):
            if n:
                c["effects"] = [n]
    with open(dest, "w") as f:
        json.dump(doc, f, indent=2)
    return dest


def frame_at(app, seconds, prev=None):
    """Color of the preview at the moment `seconds` — which is also the number of the source frame.

    Read via `pixel_settled`: after a seek the cursor moves
    before the compositor draws the new frame, and a bare `pixel` would return
    the picture of the PREVIOUS seek."""
    was = prev if prev is not None else app.pixel(*app.preview_center())
    app.click(app.x_at(int(seconds * SEC)), app.ruler_y())
    return app.pixel_settled(app.preview_center(), differs_from=was)


def near_px(app, got, want, what, tol=10):
    ok = all(abs(x - y) <= tol for x, y in zip(got, want))
    app.expect(ok, f"{what}: {got} ≈ {want} (±{tol})")


def main(out):
    require_tools()
    proj = build(os.path.join(out, "slowed.otio"))

    with Live("slowed", out, project=proj) as app:
        app.shot("open")
        app.expect(len(app.project_now().clips) == 3, "three events in the project")

        # reference: frames of an ordinary event at 0.5 s and 1.5 s of material
        a = frame_at(app, 0.5)
        b = frame_at(app, 1.5)
        app.shot("plain")
        app.expect(a != b, f"ordinary clip: frames differ, {a} vs {b}")

        # 0.5×: the same material arrives TWICE LATER from the start of the event
        near_px(app, frame_at(app, 4.0 + 1.0), a, "0.5×: +1 s inside the event = frame 0.5 s")
        near_px(app, frame_at(app, 4.0 + 3.0), b, "and +3 s = frame 1.5 s")
        app.shot("slow")

        # the unticked slot checkbox returns the normal speed
        near_px(app, frame_at(app, 8.0 + 0.5), a, "slot off: +0.5 s = frame 0.5 s")
        near_px(app, frame_at(app, 8.0 + 1.5), b, "and +1.5 s = frame 1.5 s")
        app.shot("bypass")

        # --- SLICE of a slowed event (R.39) -----------------------------
        # Until R.39 `clip_slice` computed source_in linearly, and the right half
        # started not from the right material: at 0.5× the error equals half of
        # the cut-off part. We check it on the PICTURE: the frame at the same point
        # must remain the same as before the cut.
        probe = 4.0 + 3.0  # inside the slowed event, to the right of the future cut
        before = frame_at(app, probe)
        app.click(app.x_at(int(6.0 * SEC)), app.ruler_y())
        app.key("s")
        saved = app.saved_project()
        n = len([c for c in saved.clips])
        app.expect(n == 4, f"event cut: the number of clips became {n}")
        after = frame_at(app, probe, prev=before)
        near_px(
            app,
            after,
            before,
            "after cutting the slowed event the frame at the same spot — THE SAME",
        )
        app.shot("split")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

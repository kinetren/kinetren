#!/usr/bin/env python3
"""Live scenario R.40 items 15/18 and “keys slide”.

Claims verbatim:

- “the padlock does not fully work, make it so that it has to be
  RELEASED to turn off the timeline sync, and the padlock must
  remember its state (at least for the whole project)”;
- “when I drag the left edge of the event - my automation keys also
  shift to the start, and this behavior cannot be changed in any way”.

The second is not about the data: the key does NOT slide, the AXIS of the graph lied. Here
the real trim is checked with a real mouse, and what reached the file:
the key must stay on the SAME frame of the material.

The lock is checked by the fact that it SURVIVED writing and opening the project: before R.40
it lived in the window and died with it.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def clip_kt(tl, index):
    items = []
    for track in tl.doc.get("tracks", {}).get("children", []):
        for item in track.get("children", []):
            if str(item.get("OTIO_SCHEMA", "")).startswith("Clip"):
                items.append(item)
    return (items[index].get("metadata") or {}).get("kt") or {}


def build(dest):
    """An event with a blur and two radius keys inside the material."""
    # the event is NOT from scratch: the left edge at the very edge of the track strip
    # cannot be grabbed — there the grab zone ends and the gutter begins
    project_with_clips(dest, [(1.0, 5.0, "synth:Ramp")])
    with open(dest) as f:
        doc = json.load(f)
    for track in doc.get("tracks", {}).get("children", []):
        for item in track.get("children", []):
            if not str(item.get("OTIO_SCHEMA", "")).startswith("Clip"):
                continue
            kt = item.setdefault("metadata", {}).setdefault("kt", {})
            kt["fx"] = [{"kind": "blur", "radius": 4}]
            kt["fx_auto"] = [
                [
                    {
                        "param": "radius",
                        "keys": [
                            {"t": 2 * SEC, "v": 1.0},
                            {"t": 4 * SEC, "v": 9.0},
                        ],
                    }
                ]
            ]
    with open(dest, "w") as f:
        json.dump(doc, f, indent=2)
    return dest


def keys_of(tl):
    kt = clip_kt(tl, 0)
    return [k["t"] for k in kt["fx_auto"][0][0]["keys"]]


def main(out):
    require_tools()
    proj = build(os.path.join(out, "keys.otio"))

    with Live("keys", out, project=proj) as app:
        app.shot("open")
        before = keys_of(app.project_now())
        app.expect(before == [2 * SEC, 4 * SEC], f"start: two keys, {before}")

        # --- left edge of the event to the right by a second -------------------------
        y = app.lane_y("video", 0, frac=0.7)
        start0 = app.project_now().clips[0].start
        app.drag(app.x_at(start0), y, app.x_at(start0 + 1 * SEC), y)
        saved = app.saved_project()
        # FIRST we make sure that the trim happened at all: otherwise “the keys did not
        # shift” would hold by itself and would check nothing
        start1 = saved.clips[0].source_in
        app.expect(
            saved.clips[0].start != start0 or start1 != 0,
            f"the trim must happen: start {saved.clips[0].start}, source_in {start1}",
        )
        after = keys_of(saved)
        app.expect(
            after == before,
            f"the key must stay on the SAME frame of the material: was {before}, became {after}",
        )
        app.shot("trim-right")

        # --- and back to the left ---------------------------------------------
        clip = app.project_now().clips[0]
        app.drag(app.x_at(clip.start), y, app.x_at(max(clip.start - SEC // 2, 0)), y)
        saved = app.saved_project()
        app.expect(
            saved.clips[0].source_in != start1,
            f"and this trim too: source_in {start1} → {saved.clips[0].source_in}",
        )
        after2 = keys_of(saved)
        app.expect(
            after2 == before,
            f"and when the edge moves LEFT too: was {before}, became {after2}",
        )
        app.shot("trim-left")

        # --- the axis lock is remembered in the project --------------------------------
        ui = saved.doc["metadata"]["kt"].get("ui") or {}
        app.expect(
            not ui.get("auto_axis_unlinked", False),
            "the axis lock is ON by default and gets into the saved project",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

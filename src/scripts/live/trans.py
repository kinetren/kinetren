#!/usr/bin/env python3
"""Live scenario R.33 C4: a transition is not always a dissolve.

What a person sees is checked. Two clips of solid color (red and
blue) with a transition in the middle; the cursor is put in the MIDDLE of the transition window and
the pixels of the preview to the left and right are read:

  - for a dissolve both points are the same (a mix of red with blue);
  - for a wipe to the left, on the left there is still red, on the right already blue — different colors.

This is end-to-end through the app’s real wgpu: the harness tests run the
CPU reference, and say nothing about what the GPU shows in a live window.
Additionally: a double click on the transition window must open its properties.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    red = color_media(out, "red", "C81E1E", seconds=10)
    blue = color_media(out, "blue", "1E3CC8", seconds=10)
    # clips end to end: the joint at 4 s, the transition window [3, 5)
    path = project_with_clips(
        os.path.join(out, "trans.otio"), [(0.0, 4.0, red), (4.0, 4.0, blue)]
    )

    results = []
    for kind, tag in ((None, "crossfade"), ({"kind": "wipe", "dir": "left"}, "wipe")):
        add_transition(path, seconds=1.0, kind=kind)
        with Live(f"trans-{tag}", out, project=path) as app:
            p = app.project_now()
            app.expect(len(p.transitions) == 1, f"transition read from the file: {p.transitions}")
            cx, cy = app.preview_center()
            g = app.geom()
            # points to the left and right of the centre of the preview, inside the picture
            dx = int(g["window"]["w"] * 0.15)
            # the reference colour BEFORE the scrub: we wait for the change exactly from it (after
            # an edit of the llvmpipe shader the first composite frame compiles for seconds —
            # a raw read returned the frame of the PREVIOUS position)
            base = app.pixel(cx + dx, cy)
            # the cursor in the middle of the transition window
            app.click(app.x_at(4 * SEC), app.ruler_y())
            app.near(app.playhead(), 4 * SEC, 6000, "cursor placed in the middle of the transition")
            right = app.pixel_settled((cx + dx, cy), differs_from=base)
            left = app.pixel_settled((cx - dx, cy))
            app.shot(f"mid-{tag}")
            print(f"   {tag}: on the left {left}, on the right {right}")
            results.append((tag, left, right))
            if tag == "wipe":
                # wipe to the left: the blue arrives from the RIGHT
                app.expect(
                    right[2] > right[0] + 30,
                    f"on the right the incoming blue must be: {right}",
                )
                app.expect(
                    left[0] > left[2] + 30,
                    f"on the left the outgoing red must remain: {left}",
                )
                # transition properties by a double click on its window
                before = app.shot("before-dblclick")
                y = app.lane_y("video", 0, frac=0.8)
                app.double_click(app.x_at(int(4 * SEC)), y)
                # the window needs time to draw in the software render
                d = 0.0
                for _ in range(10):
                    poll_step(0.5)
                    d = app.screen_diff(before)
                    if d > 0.01:
                        break
                app.expect(
                    d > 0.01,
                    f"double click opened the transition window (the screen changed by {d:.1%})",
                )
                app.shot("props")
            code = app.done()
            if code:
                return code

    # dissolve and wipe must be told apart by the PICTURE, not only by the model
    (_, cl, cr), (_, wl, wr) = results
    cross_uniform = abs(cl[0] - cr[0]) < 25 and abs(cl[2] - cr[2]) < 25
    if not cross_uniform:
        print(f"!! a dissolve must be uniform across the frame: {cl} vs {cr}")
        return 1

    # R.51: inversion — the same sides the other way round (v7)
    add_transition(path, seconds=1.0, kind={"kind": "wipe", "dir": "left"}, invert=True)
    with Live("trans-invert", out, project=path) as app:
        cx, cy = app.preview_center()
        g = app.geom()
        dx = int(g["window"]["w"] * 0.15)
        base = app.pixel(cx - dx, cy)
        app.click(app.x_at(4 * SEC), app.ruler_y())
        # with inversion the blue arrives from the LEFT — we wait for the change there
        left = app.pixel_settled((cx - dx, cy), differs_from=base)
        right = app.pixel_settled((cx + dx, cy))
        app.shot("mid-invert")
        print(f"   invert: on the left {left}, on the right {right}")
        app.expect(left[2] > left[0] + 30, f"invert: on the left already the incoming blue: {left}")
        app.expect(right[0] > right[2] + 30, f"invert: on the right still red: {right}")
        code = app.done()
        if code:
            return code

    # R.51: a transition ON AN OVERLAP — the wipe cuts the overlap zone, not
    # a hard Vegas cut
    opath = project_with_overlap(os.path.join(out, "trans-ov.otio"), red, blue)
    with Live("trans-overlap", out, project=opath) as app:
        # an overlapped transition is not a child of the track, but the envelope of the reel
        ot = app.project_now().doc["metadata"]["kt"].get("overlap_transitions", [])
        app.expect(len(ot) == 1, f"overlapping transition in the envelope: {ot}")
        cx, cy = app.preview_center()
        g = app.geom()
        dx = int(g["window"]["w"] * 0.15)
        base = app.pixel(cx + dx, cy)
        app.click(app.x_at(int(3.5 * SEC)), app.ruler_y())
        right = app.pixel_settled((cx + dx, cy), differs_from=base)
        left = app.pixel_settled((cx - dx, cy))
        app.shot("mid-overlap")
        print(f"   overlap: on the left {left}, on the right {right}")
        app.expect(left[0] > left[2] + 30, f"overlap: on the left the outgoing red: {left}")
        app.expect(right[2] > right[0] + 30, f"overlap: on the right the incoming blue: {right}")
        code = app.done()
        if code:
            return code

    print("== transitions: the dissolve is uniform, the wipe cuts the frame, invert and overlap are alive ==")
    return 0


def add_transition(path, seconds, kind=None, invert=False):
    """Transition between the first and the second clip; `kind` is our kt envelope."""
    with open(path) as f:
        doc = json.load(f)
    track = next(t for t in doc["tracks"]["children"] if t.get("kind") == "Video")
    track["children"] = [c for c in track["children"] if c["OTIO_SCHEMA"] != "Transition.1"]
    off = {"OTIO_SCHEMA": "RationalTime.1", "value": round(seconds * SEC), "rate": 90000.0}
    kt = {}
    if kind:
        kt["kind"] = kind
    if invert:
        kt["invert"] = True
    node = {
        "OTIO_SCHEMA": "Transition.1",
        "name": "",
        "transition_type": "SMPTE_Wipe" if kind else "SMPTE_Dissolve",
        "in_offset": off,
        "out_offset": off,
        "metadata": {"kt": kt} if kt else {},
    }
    # the transition is written BEFORE its incoming clip
    track["children"].insert(1, node)
    if invert:
        doc["metadata"]["kt"]["v"] = 7
    elif kind:
        doc["metadata"]["kt"]["v"] = 3
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)


def project_with_overlap(dest, red, blue):
    """A “staircase” pair on one track (the second layer is an adjacent Track with
    a layer map, item 165) + an overlapping transition in the reel envelope (R.51)."""
    import copy

    from kt_live import _clip, _gap

    path = project_with_clips(dest, [(0.0, 4.0, red)])
    with open(path) as f:
        doc = json.load(f)
    video = next(t for t in doc["tracks"]["children"] if t.get("kind") == "Video")
    t2 = copy.deepcopy(video)
    t2["children"] = [_gap(3.0), _clip(2, 4.0, blue, 2)]
    doc["tracks"]["children"].insert(doc["tracks"]["children"].index(video) + 1, t2)
    kt = doc["metadata"]["kt"]
    kt["layers"] = {"video": [2]}
    kt["overlap_transitions"] = [
        {"a": 1, "b": 2, "kind": {"kind": "wipe", "dir": "left"}}
    ]
    kt["v"] = 7
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)
    return path


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

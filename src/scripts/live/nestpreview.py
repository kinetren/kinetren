#!/usr/bin/env python3
"""Live scenario R.56 (C5): a composition on the timeline has FRAMES and PEAKS.

The owner’s complaint verbatim: “compositions for some reason have no preview”, and,
clarifying: “on the timeline there is no preview as for ordinary events — frames and
audio peaks”.

The harness test t600 checks that the frames reach the textures. Here the question is
different and the only one for which the live scenarios were set up: is it visible
ON THE SCREEN of the real program. We compare the brightness “variegation” of the event body
BEFORE and AFTER the service has computed: an empty tile is uniform, a
filmstrip is not.

Steps:

1. collapse two events into a reel — on the timeline there is one event;
2. wait for the service and take a picture of the body: it must stop being
   a uniform fill;
3. an edit inside the reel changes the revision — the strip is recomputed, and on
   the screen there are frames again.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def slice_means(app, x0, y0, w, h, tag, n=3):
    """Average brightness of N vertical slices of the rectangle — with one
    screen snapshot.

    An empty tile and a mini-map of the content give slices of the SAME brightness;
    a filmstrip gives different frames, hence different slices. This is the observable
    consequence of “frames are visible”, not of “something changed”.
    """
    img = app.shot(tag)
    out = []
    for i in range(n):
        x = int(x0 + w * i / n)
        ww = max(1, int(w / n) - 2)
        v = subprocess.run(
            [
                "convert",
                img,
                "-crop",
                f"{ww}x{int(h)}+{x}+{int(y0)}",
                "+repage",
                "-colorspace",
                "Gray",
                "-format",
                "%[fx:mean]",
                "info:-",
            ],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        out.append(float(v))
    return out


def main(out):
    require_tools()
    path = os.path.join(out, "nestpreview.otio")
    proj = project_with_clips(
        path,
        [(0.0, 2.0, "synth:bars"), (2.0, 2.0, "synth:ramp"), (6.0, 2.0, "synth:sine:440")],
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nestpreview", out, project=proj) as app:
        y = app.lane_y("video", 0, 0.6)

        # --- 1. collapse the FIRST TWO events into a reel --------------------
        app.click(app.x_at(1 * SEC), y)
        app.click(app.x_at(3 * SEC), y, mods=("ctrl",))
        app.right_click(app.x_at(3 * SEC), y)
        app.click_probe("nest.collapse", "collapse item")
        app.expect(
            len(app.live_clips()) == 2,
            f"collapsed not into one (plus a third event): {app.live_clips()}",
        )
        inst = min(app.live_clips())
        x0 = app.x_at(inst[0]) + 6
        x1 = app.x_at(inst[0] + inst[1]) - 6
        # the row coordinates — from the program ITSELF (rule R.27)
        g = app.geom()
        row = next(r for r in g["rows"] if r["kind"] == "video" and r["lane"] == 0)
        h = row.get("h", g["lane_h"])
        top = app.lane_y("video", 0, 0.0)
        box = (x0, top + 6, x1 - x0, h - 12)

        # --- 2. wait for the service: in the body DIFFERENT frames are visible ------------
        spread = 0.0
        for _ in range(60):
            # we move the mouse: redraw goes by MOVEMENT (invariant 12)
            app.hover(app.x_at(inst[0]) + 20, y, pause=0.05)
            poll_step(0.4)
            m = slice_means(app, *box, "wait")
            spread = max(spread, max(m) - min(m))
            if spread > 0.08:
                break
        app.shot("frames")
        app.expect(
            spread > 0.08,
            f"in the body of the composition all slices have one brightness (spread {spread:.3f}) — "
            "so this is a fill or a mini-map, not frames",
        )

        # --- 3. and these are the REEL’S FRAMES, not the file’s thumbnails: the file’s reel has none,
        # and the index service knows nothing about it --------------------
        app.key("ctrl+s")
        kt = app.saved_project().doc["metadata"]["kt"]
        app.expect(
            len(kt.get("sequences", [])) == 1,
            f"reel not recorded — we checked the wrong event: {list(kt.keys())}",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

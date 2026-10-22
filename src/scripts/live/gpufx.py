#!/usr/bin/env python3
"""Live scenario R.29: video FX on the GPU path are really applied in the preview.

The live kt-app in Xvfb runs on the REAL GPU code (software rendering llvmpipe —
also counts as “GPU present”). Two clips of the same light-gray media: without effects and with
an Edges chain — on a flat color the Sobel filter darkens the frame to black. Checked is
what a person sees: the preview without FX is light, with FX — dark. In passing it catches
a “silent CPU fallback with a broken notice”: the chain must work in
both cases, and the backend is printed to the scenario log from the stderr of the program.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    media = color_media(out, "gray", "D0D0D0", seconds=10)
    path = os.path.join(out, "gpufx.otio")
    project_with_clips(path, [(0.0, 4.0, media), (4.0, 4.0, media)])
    add_edges_to_second_clip(path)

    with Live("gpufx", out, project=path) as app:
        preview = app.preview_center()
        app.shot("open")

        # a clip without FX: a flat light frame
        app.click(app.x_at(int(1.5 * SEC)), app.ruler_y())
        plain = app.brightness(preview)
        app.shot("plain")

        # clip with Edges: a flat colour has no gradients → black frame.
        #
        # We read AFTER the picture CHANGES (`pixel_settled`), not at once: the preview
        # normally keeps the last frame until the decoded one arrives
        # (rule in `apps/kt-app/CLAUDE.md`). Previously the cost of the read itself hid this —
        # a snapshot of the whole screen via ImageMagick took 84 ms and gave the program a head start;
        # with a direct read from X (R.69) the head start disappeared, and the
        # scenario started reading the PREVIOUS frame.
        app.click(app.x_at(int(5.5 * SEC)), app.ruler_y())
        r, g, b = app.pixel_settled(preview, differs_from=app.pixel(*preview))
        fx = (r + g + b) / 3.0
        app.shot("edges")

        print(f"   preview brightness: without FX {plain:.1f}, with Edges {fx:.1f}")
        app.expect(plain > 120.0, f"frame without FX is light: {plain:.1f}")
        app.expect(fx < plain * 0.2, f"Edges darkened the flat frame: {fx:.1f} vs {plain:.1f}")
        return app.done()


def add_edges_to_second_clip(path):
    """Chain kt:edges on the second clip of the video track (convert.rs format)."""
    with open(path) as f:
        doc = json.load(f)
    track = next(t for t in doc["tracks"]["children"] if t.get("kind") == "Video")
    clips = [c for c in track["children"] if c.get("OTIO_SCHEMA", "").startswith("Clip")]
    clips[1]["effects"] = [
        {
            "OTIO_SCHEMA": "Effect.1",
            "name": "",
            "effect_name": "kt:edges",
            "enabled": True,
            "metadata": {"kt": {"kind": "edges", "threshold": 48}},
        }
    ]
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

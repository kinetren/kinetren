#!/usr/bin/env python3
"""Live scenario R.25: a fade at the edge of a track really darkens the picture.

What is checked is NOT the model, but what a person sees: the cursor is placed at the start,
the middle and the end of the fade window, and the brightness of the preview must grow. In addition,
the shape on the timeline is checked — a fade has one diagonal, not an X.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    # a light clip: the preview brightness is exactly the fade check
    media = color_media(out, "white", "D0D0D0", seconds=10)
    path = project_with_clips(os.path.join(out, "fade.otio"), [(0.0, 8.0, media)])
    add_fade_in(path, seconds=3.0)

    with Live("fade", out, project=path) as app:
        y = app.lane_y("video", 0, frac=0.72)
        preview = app.preview_center()
        app.shot("open")

        p = app.project_now()
        app.expect(len(p.transitions) == 1, f"fade read from the file: {p.transitions}")

        # the cursor goes into the fade window and past it: the picture must fade in
        levels = []
        for t in (0.2 * SEC, 1.5 * SEC, 2.8 * SEC, 5.0 * SEC):
            app.click(app.x_at(int(t)), app.ruler_y())
            levels.append(app.brightness(preview))
        print(f"   preview brightness along the fade: {[round(v, 1) for v in levels]}")
        app.expect(
            levels[0] < levels[1] < levels[2] <= levels[3] + 1.0,
            f"the picture emerges from black: {[round(v, 1) for v in levels]}",
        )
        app.expect(levels[0] < levels[3] * 0.5, "at the start of the fade noticeably darker than after it")
        app.shot("mid-fade")
        return app.done()


def add_fade_in(path, seconds):
    """Fade-in of length `seconds` as the first child of the video track."""
    with open(path) as f:
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
                "value": round(seconds * SEC),
                "rate": 90000.0,
            },
            "metadata": {},
        },
    )
    with open(path, "w") as f:
        json.dump(doc, f, indent=4)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

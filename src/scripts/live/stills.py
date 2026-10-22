#!/usr/bin/env python3
"""Live scenario R.67 (item 13): a STATIC PICTURE in a real window.

The owner verbatim: “static pictures do not display at all, ever.. there is an eternal
Loading... THIS IS A DISASTER. Not only does it not work — you also find out
about it only at the moment of viewing”.

PATH (media-index spec, “a picture is read, planned, shown”):
1. a project with a 20 s event on a picture → cursor to the start → the preview shows the
   picture at once, without “Loading…”;
2. cursor to the end of the event (19.5 s) → the picture is in place: it has no end of
   material;
3. the picture is REMOVED from disk, the project is opened again → the frame shows a dark
   placeholder WITH THE FILE NAME, not an eternal “Loading…”.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401

SEC = 90000


def still_project(dest, img, secs=20.0):
    new_otio(dest)
    doc = json.load(open(dest))
    doc["metadata"]["kt"]["settings"].update(fps=30, width=1280, height=720)
    track = next((t for t in doc["tracks"]["children"] if t.get("kind") == "Video"), None)
    if track is None:
        track = {
            "OTIO_SCHEMA": "Track.1", "name": "", "kind": "Video", "children": [],
            "source_range": None, "effects": [], "markers": [], "enabled": True, "metadata": {},
        }
        doc["tracks"]["children"].append(track)
    rt = lambda s: {"OTIO_SCHEMA": "RationalTime.1", "value": round(s * SEC), "rate": 90000.0}
    track["children"] = [
        {
            "OTIO_SCHEMA": "Clip.2",
            "name": "still",
            "source_range": {"OTIO_SCHEMA": "TimeRange.1", "start_time": rt(0), "duration": rt(secs)},
            "media_references": {
                "DEFAULT_MEDIA": {
                    "OTIO_SCHEMA": "ExternalReference.1", "name": "", "target_url": img,
                    "available_range": None, "available_image_bounds": None, "metadata": {},
                }
            },
            "active_media_reference_key": "DEFAULT_MEDIA",
            "effects": [], "markers": [], "enabled": True,
            "metadata": {"kt": {"id": 1, "media": 1}},
        }
    ]
    doc["metadata"]["kt"]["bin_media"] = [{"id": 1, "path": img}]
    with open(dest, "w") as f:
        json.dump(doc, f, indent=4)
    return dest


def looks_like_picture(app, box):
    """Spread of brightnesses in the preview area, NORMALIZED 0..1 (as ImageMagick
    returns it): an empty frame gives ~0.0, a picture — tenths."""
    shot = app.shot("probe")
    out = subprocess.run(
        ["convert", shot, "-crop", f"{box[2]}x{box[3]}+{box[0]}+{box[1]}", "+repage",
         "-colorspace", "Gray", "-format", "%[fx:standard_deviation]", "info:-"],
        capture_output=True, text=True, check=True,
    ).stdout.strip()
    return float(out)


def main(out):
    require_tools()
    # the picture is the program’s own fixture (a deterministic generator)
    img = os.path.join(out, "logo.jpg")
    subprocess.run(
        ["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i",
         "testsrc=size=640x360:rate=1:duration=1", "-frames:v", "1", "-q:v", "3", img],
        check=True,
    )
    path = still_project(os.path.join(out, "stills.otio"), img)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("stills", out, project=path) as app:
        y = app.lane_y("video", 0, frac=0.5)
        app.click(app.x_at(1 * SEC), y)
        app.settle()
        cx, cy = app.preview_center()
        box = (cx - 200, cy - 110, 400, 220)

        # 1) the picture is visible AT ONCE
        sd = looks_like_picture(app, box)
        app.expect(sd > 0.15, f"in the preview a picture, not an empty frame (spread {sd:.2f})")
        app.step("still-start", "picture in the preview at the start of the event")

        # 2) at the end of the event — the same picture (the material has no end for it)
        app.click(app.x_at(int(19.5 * SEC)), y)
        app.settle()
        sd_end = looks_like_picture(app, box)
        app.expect(sd_end > 0.15, f"at 19.5 s the picture is in place (spread {sd_end:.2f})")
        app.step("still-end", "the picture is in place at the end of the twenty-second event")

    # 3) the picture is gone — a placeholder with the name, not “Loading…”
    os.remove(img)
    with Live("stills-gone", out, project=path) as app:
        y = app.lane_y("video", 0, frac=0.5)
        app.click(app.x_at(1 * SEC), y)
        app.settle()
        g = app.geom()
        notes = " ".join(g.get("notices", []))
        app.expect("logo.jpg" in notes, f"the missing file was named to the person: {notes[:120]}")
        cx, cy = app.preview_center()
        box = (cx - 200, cy - 110, 400, 220)
        sd = looks_like_picture(app, box)
        app.expect(sd > 0.02, f"in the frame a placeholder with the file name, not emptiness (spread {sd:.2f})")
        app.step("still-gone", "missing picture: placeholder with the file name")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

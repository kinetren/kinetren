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

PATH (video-plan spec, still-image-frame-rule, “the person sees the rule work”):
4. a SQUARE red picture in a 16:9 project → the preview shows it fitted: a red square with
   side margins, not squashed to 16:9;
5. double click on the event → the effects window; click the leading “Geometry” node →
   its row of rule words; click “fill” → the red covers the width of the frame; “stretch” →
   the same width, squashed; “as is” → a smaller centred square; “fit” → back to the first.
   The red width is measured on screen after each word (one screenshot per rule).
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio, poll_step  # noqa: E402,F401

SEC = 90000


def red_extent(app, cx, cy, tag):
    """(right edge, height) of the red picture in one screenshot: the red run from the preview
    centre to the right, and the red run of the column at the preview centre. The effects window
    covers the left of the preview, never its centre; the picture's own centre is found from the
    frame (stretch), not assumed to be the panel's."""
    img = app.shot(tag)

    def line(crop):
        out = subprocess.run(
            ["convert", img, "-crop", crop, "+repage", "-depth", "8", "rgb:-"],
            capture_output=True,
            check=True,
        ).stdout
        return [out[3 * i] > 150 and out[3 * i + 1] < 90 and out[3 * i + 2] < 90 for i in range(len(out) // 3)]

    row = line(f"0x1+0+{cy}")
    right = cx
    while right < len(row) and row[right]:
        right += 1
    col = line(f"1x0+{cx}+0")
    top = bottom = cy
    while top > 0 and col[top - 1]:
        top -= 1
    while bottom < len(col) and col[bottom]:
        bottom += 1
    return right, bottom - top

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

    # 4–5) a SQUARE still in 16:9 follows the frame rule (still-image-frame-rule)
    sq = os.path.join(out, "square.png")
    subprocess.run(
        ["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i", "color=c=0xC81E1E:size=360x360",
         "-frames:v", "1", sq],
        check=True,
    )
    path = still_project(os.path.join(out, "stills-square.otio"), sq)
    with Live("stills-square", out, project=path) as app:
        y = app.lane_y("video", 0, frac=0.5)
        app.click(app.x_at(1 * SEC), app.ruler_y())
        app.settle()
        cx, cy = app.preview_center()
        r, g, b = app.pixel_settled((cx, cy))
        app.expect(r > 150 and g < 90, f"preview center — the red square: RGB {r},{g},{b}")
        start = red_extent(app, cx, cy, "square-start")
        app.double_click(app.x_at(1 * SEC), y)
        app.expect(app.has_probe("fx.slot.0"), "the effects window opened with the chain strip")
        app.click_probe("fx.slot.0", "the leading “Geometry” node in the chain strip")
        app.expect(app.has_probe("geo.conform.fill"), "the node shows its row of rule words")

        def after(word, before):
            """Click the rule word, wait for the preview to change its red width."""
            app.click_probe(f"geo.conform.{word}", f"rule “{word}” of the leading node")
            w = before
            for _ in range(20):
                w = red_extent(app, cx, cy, f"square-{word}")
                if w != before:
                    break
                poll_step(0.2)
            return w

        fill = after("fill", start)
        app.step("square-fill", "rule “fill”: the red covers the width of the frame")
        # a solid square under stretch covers what fill covers: there is no width change to wait
        # for, the program's own settle says the click was applied
        app.click_probe("geo.conform.stretch", "rule “stretch” of the leading node")
        app.settle()
        stretch = red_extent(app, cx, cy, "square-stretch")
        app.step("square-stretch", "rule “stretch”: the square is squashed to the whole frame")
        as_is = after("as_is", stretch)
        app.step("square-as-is", "rule “as is”: a smaller square in the centre, one pixel per pixel")
        fit = after("fit", as_is)
        app.step("square-fit", "rule “fit”: the square with side margins again")
        # the frame: stretch covers it; its centre and width come from its right edge and height
        frame_w = stretch[1] * 16 / 9
        centre = stretch[0] - frame_w / 2
        width = lambda e: 2 * (e[0] - centre)  # noqa: E731
        app.expect(abs(width(fill) - frame_w) <= 4, f"“fill” covers the frame width: {width(fill):.0f} of {frame_w:.0f}")
        # (fill runs past the frame: its filtered edge row may land one pixel outside)
        app.expect(abs(fill[1] - stretch[1]) <= 2, f"“fill” covers the frame height: {fill[1]} vs {stretch[1]}")
        # 9/16 of the frame width for fit, 360 of 1280 for as is (both its sides: 360 of 720 high)
        app.expect(
            abs(width(fit) / frame_w - 9 / 16) < 0.02 and abs(fit[1] - stretch[1]) <= 2,
            f"“fit”: a full-height square, 9/16 of the frame: {width(fit):.0f} of {frame_w:.0f}",
        )
        app.expect(abs(width(start) - width(fit)) <= 4, f"the project rule was fit from the start: {start} vs {fit}")
        app.expect(
            abs(width(as_is) / frame_w - 360 / 1280) < 0.02 and abs(as_is[1] / stretch[1] - 0.5) < 0.02,
            f"“as is”: 360×360 of a 1280×720 project: {width(as_is):.0f}×{as_is[1]} in a {frame_w:.0f}×{stretch[1]} frame",
        )
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

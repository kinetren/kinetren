#!/usr/bin/env python3
"""Live scenario R.62 (change B): a TRACK in a real window.

The harness tests t742-t745 press badges and menus inside one process on a
synthetic scene. Here the real program answers the owner’s question
(“I would like to mute tracks, apply effects”) the way
a person would see it:

1. two video tracks, green over red (in the OTIO the upper track is
   the LAST in the file); the M badge on the upper one — and in the preview it turns RED:
   muting acts on the picture, not only on the model;
2. pressing it again brings back the red; solo on the lower one gives the same green
   by another route;
3. lock: a gesture on a locked track does NOT move the event, and the status line
   names the track;
4. the FX badge opens the track window, and the effect placed from there lands on
   the TRACK, not on the event;
5. the render window lists the muted tracks BEFORE the start.

The color is read by `pixel_settled` (we wait for a CHANGE, not for “until it settles”: the preview
routinely holds the last frame), the badge coordinates — from the program itself
(`KT_GEOM_DUMP`, fields `track.<row>.<badge>`).
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def two_video_tracks(dest, red, green):
    """Project of TWO video tracks: the green lies ABOVE the red.

    `project_with_clips` handles one track, and the whole scenario is about the second:
    without it “muted the upper one” cannot be told apart from “muted everything”."""
    project_with_clips(dest, [(0.0, 6.0, red)])
    doc = json.load(open(dest))
    vid = [t for t in doc["tracks"]["children"] if t.get("kind") == "Video"]
    top = vid[0]
    second = json.loads(json.dumps(top))
    for i, c in enumerate(second["children"]):
        c["name"] = "g"
        c["metadata"]["kt"]["id"] = 100 + i
        c["metadata"]["kt"]["media"] = 2
        c["media_references"]["DEFAULT_MEDIA"]["target_url"] = green
    doc["tracks"]["children"].insert(doc["tracks"]["children"].index(top) + 1, second)
    json.dump(doc, open(dest, "w"), indent=4)
    return dest


def main(out):
    require_tools()
    red = color_media(out, "red", "C81E1E", seconds=6)
    green = color_media(out, "green", "1EA81E", seconds=6)
    path = two_video_tracks(os.path.join(out, "track.otio"), red, green)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("track", out, project=path) as app:
        cx, cy = app.preview_center()
        app.click(app.x_at(2 * SEC), app.ruler_y())
        top = app.pixel_settled((cx, cy))
        app.expect(
            top[1] > 120 and top[0] < 100,
            f"the GREEN track lies on top (the last in the file): RGB {top}",
        )
        app.shot("both")

        # --- 1. muting the upper track CHANGES the PICTURE --------------------
        app.expect(app.has_probe("track.v1.mute"), "the program returned the rectangle of the M badge")
        app.click_probe("track.v1.mute", "M badge of the upper track")
        muted = app.pixel_settled((cx, cy), differs_from=top)
        app.shot("muted")
        app.expect(
            muted[0] > 120 and muted[1] < 100,
            f"under the muted track the RED is visible: RGB {muted}",
        )

        # --- 2. repeated press and solo -----------------------------------
        app.click_probe("track.v1.mute", "M badge once more")
        back = app.pixel_settled((cx, cy), differs_from=muted)
        app.expect(back[1] > 120 and back[0] < 100, f"muting lifted, green again: RGB {back}")
        app.click_probe("track.v2.solo", "S badge of the lower track")
        solo = app.pixel_settled((cx, cy), differs_from=back)
        app.shot("solo")
        app.expect(
            solo[0] > 120 and solo[1] < 100,
            f"solo on the lower left only it in the frame: RGB {solo}",
        )
        app.click_probe("track.v2.solo", "lift the solo")
        app.pixel_settled((cx, cy), differs_from=solo)

        # --- 3. the lock refuses the gesture and names the track -------------------
        before = app.live_clips()
        app.click_probe("track.v2.lock", "lock badge of the lower track")
        y = app.lane_y("video", 1, frac=0.6)
        app.drag(app.x_at(2 * SEC), y, app.x_at(4 * SEC), y)
        app.shot("locked")
        after = app.live_clips()
        app.expect(before == after, f"the locked track did not hand the event over to the drag: {before} → {after}")
        status = ""
        for _ in range(12):
            status = app.geom().get("status", "")
            if "V2" in status or "v2" in status:
                break
            poll_step(0.2)
        app.expect(
            "V2" in status or "v2" in status,
            f"the refusal names the track and the place where to remove the lock: “{status}”",
        )
        app.click_probe("track.v2.lock", "remove the lock")

        # --- 4. the track chain --------------------------------------------
        app.click_probe("track.v1.fx", "FX badge of the upper track")
        app.expect(app.has_probe("track.name"), "track window opened with its own form")
        app.expect(app.has_probe("fxreg.add"), "in the track window there is a chain strip with a picker")
        app.shot("trackwin")
        app.click_probe("fxreg.add", "the picker button in the track’s chain strip")
        for ch in "vign":
            app.key(ch)
        app.click_probe("fxreg.tpl.vignette", "“Vignette” line in the picker")
        app.shot("vignette")
        app.click_probe("track.close", "close button of the track window")
        doc = app.saved_project().doc
        # the track chain lives in the `effects` of the OTIO track ITSELF — in the same place
        # where any foreign reader of the format expects it
        on_track = [
            e
            for t in doc["tracks"]["children"]
            for e in t.get("effects", [])
            if e.get("metadata", {}).get("kt", {}).get("kind") == "vignette"
        ]
        app.expect(
            len(on_track) == 1,
            f"the vignette landed on the TRACK: {[t.get('effects') for t in doc['tracks']['children']]}",
        )
        clip_fx = [
            c
            for t in doc["tracks"]["children"]
            for c in t.get("children", [])
            if c.get("effects")
        ]
        app.expect(not clip_fx, f"the events were not given the track’s chain: {len(clip_fx)}")

        # --- 5. the render window lists the muted AND the locked solo ---------------------
        app.click_probe("track.v1.mute", "mute the upper one again")
        # solo on the bottom: only it will go into the render, and the window must say
        # so before the start — the second half of the warning, which the live check never reached
        # once (R.68, task 5.3)
        app.click_probe("track.v2.solo", "solo on the lower track before the render")
        app.click_probe("menu.file", "“File” menu")
        app.click_probe("file.render", "“Render” item")
        app.expect(
            app.has_probe("render.muted"),
            "the render window reported the muted track BEFORE the start",
        )
        app.expect(
            app.has_probe("render.solo"),
            "the render window reported the solo too: the render will not take the whole montage",
        )
        app.shot("render")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/track"))

#!/usr/bin/env python3
"""Live scenario R.40: tracks and service blocks.

The user’s complaints, verbatim:

- “item 11 not done - the track changes its height but the events are still tiny -
  you did not check a damn thing”;
- “item 2, adding a morpher to a track via RMB, not done”;
- “item 2: I cannot move morphers between different tracks”;
- “For some reason morphers are not selected via area-select, you did not
  check a damn thing”.

All of this was GREEN in the R.38 harness and broken in the window: the tests probed the spot
where I put the function, not the spot where a person clicks. So here
a real mouse in a real window is used, and each step is checked by an OBSERVABLE
CONSEQUENCE — what reached the project file.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def build(dest):
    """A clip, TWO morph tracks, and a block on the top one — room to carry it."""
    project_with_clips(dest, [(0.0, 8.0, "synth:Ramp")])
    with open(dest) as f:
        doc = json.load(f)
    kt = doc["metadata"]["kt"]
    kt["v"] = 4
    kt["morph_lanes"] = 2
    kt["morphers"] = [
        {"id": 1, "lane": 0, "start": 2 * SEC, "duration": 2 * SEC, "patch": {}}
    ]
    # camera tracks and the camera on the top (item 5: “I do not understand how something
    # can be added to a camera track — what is it for?????”)
    kt["camera_lanes"] = 2
    kt["lane_order"] = [
        ["morph", 0], ["morph", 1], ["camera", 0], ["camera", 1],
        ["video", 0], ["audio", 0],
    ]
    kt["cameras"] = [
        {"id": 1, "lane": 0, "start": 2 * SEC, "duration": 2 * SEC, "cam": {}}
    ]
    with open(dest, "w") as f:
        json.dump(doc, f, indent=2)
    return dest


def main(out):
    require_tools()
    proj = build(os.path.join(out, "lanes.otio"))

    with Live("lanes", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 1, f"start: one clip, {p.clips}")

        g = app.geom()
        rows = {(r["kind"], r["lane"]): r for r in g["rows"]}
        video_h0 = rows[("video", 0)]["h"]
        win = geometry()

        # --- 1. the row height reaches the EVENT (item 11) --------------------
        # We drag the bottom edge of the video row down, and then grab the clip THERE,
        # where it was not before R.40: below the old boundary. If the event is
        # still drawn at the base height — the gesture goes into the void and the
        # clip does not move.
        gx = win["x"] + g["timeline"]["left"] - 8.0
        edge_y = app.lane_y("video", 0, frac=1.0)
        grow = 46
        app.drag(gx, edge_y, gx, edge_y + grow)
        g2 = app.geom()
        rows2 = {(r["kind"], r["lane"]): r for r in g2["rows"]}
        app.near(
            rows2[("video", 0)]["h"],
            video_h0 + grow,
            12,
            "the row grew by as much as was pulled",
        )
        # a point BELOW the old row boundary, but inside the new one
        deep_y = rows2[("video", 0)]["top"] + win["y"] + video_h0 + grow * 0.5
        start0 = p.clips[0].start
        x0 = app.x_at(start0 + SEC)
        app.drag(x0, deep_y, x0 + 120, deep_y)
        saved = app.saved_project()
        moved = saved.clips[0].start
        app.expect(
            moved != start0,
            f"clip grabbed BELOW the old row boundary: {start0} → {moved}. "
            "Did not move — so the event’s rectangle did not grow together with the row",
        )
        app.shot("row-height-reaches-clip")

        # --- 2. RMB on the morph track ITSELF gives a menu (item 2) -----------------
        # before R.40 here a menu of EMPTY SPACE opened: the item “add morpher here”
        # lived only in the 34-pixel header of the row
        my = app.lane_y("morph", 1, frac=0.5)
        mx = app.x_at(6 * SEC)
        before = app.shot("before-lane-menu")
        app.right_click(mx, my)
        app.shot("lane-menu")
        d = app.screen_diff(before)
        app.expect(
            d > 0.001,
            f"RMB on the morph track must open the menu (the screen changed by {d:.1%})",
        )
        app.key("Escape")

        # --- 3. the morph block moves to the neighbouring track (item 2) ------------
        y0 = app.lane_y("morph", 0, frac=0.5)
        y1 = app.lane_y("morph", 1, frac=0.5)
        bx = app.x_at(3 * SEC)
        app.drag(bx, y0, bx, y1)
        saved = app.saved_project()
        m = saved.doc["metadata"]["kt"]["morphers"][0]
        app.expect(
            m["lane"] == 1,
            f"the morph block must move to the track under the cursor, but it went to {m['lane']}",
        )
        app.shot("morph-moved")

        # --- 4. the frame catches the morpher, Del removes it -----------------------
        # Ctrl+drag on empty space — the selection frame (an ordinary drag there
        # draws the region). The consequence is checked by deletion: the caught thing disappears
        # from the file.
        y1 = app.lane_y("morph", 1, frac=0.5)
        rx0 = app.x_at(int(1.5 * SEC))
        rx1 = app.x_at(int(4.5 * SEC))
        app.drag(rx0, y1 - 6, rx1, y1 + 6, mods=("ctrl",))
        app.key("Delete")
        saved = app.saved_project()
        left = saved.doc["metadata"]["kt"].get("morphers", [])
        app.expect(
            not left,
            f"the frame must select the morpher, and Del must remove the caught one; remaining {left}",
        )
        app.shot("rubber-deleted")

        # --- 5. the camera track also responds to RMB and moves the block --------
        # the claim verbatim: “I do not understand how to add something to a camera
        # track - what is it for?????”. Before R.40 RMB on the camera row gave a
        # menu of empty space, and the camera could be added only from RMB on the
        # 34-pixel header of ANOTHER row.
        cy = app.lane_y("camera", 1, frac=0.5)
        before = app.shot("before-camera-menu")
        app.right_click(app.x_at(6 * SEC), cy)
        app.shot("camera-menu")
        d = app.screen_diff(before)
        app.expect(
            d > 0.001,
            f"RMB on the camera track must open the menu (the screen changed by {d:.1%})",
        )
        app.key("Escape")

        cy0 = app.lane_y("camera", 0, frac=0.5)
        cy1 = app.lane_y("camera", 1, frac=0.5)
        cx = app.x_at(3 * SEC)
        app.drag(cx, cy0, cx, cy1)
        saved = app.saved_project()
        cam = saved.doc["metadata"]["kt"]["cameras"][0]
        app.expect(
            cam["lane"] == 1,
            f"the camera block must move to the track under the cursor, but it went to {cam['lane']}",
        )
        app.shot("camera-moved")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

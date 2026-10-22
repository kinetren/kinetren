#!/usr/bin/env python3
"""Live scenario R.68 (task 5.3): track verbs FROM THE MENU and FROM THE WINDOW.

Why it appeared. The live coverage report (`scripts/live-coverage.py`)
showed ten places that a person in the check had NOT REACHED BY MOUSE
EVEN ONCE, and all ten are a second path to what `track.py` does with badges:

* row menu — `track.menu.props`, `.mute`, `.solo`, `.lock`, `.fx`;
* track window — checkboxes `track.mute`/`track.solo`/`track.lock`, blend mode
  `track.blend.{}` and the “Cancel” button `track.cancel`.

The owner’s decision (R.68): “click through the complex moments, or those that were not
clicked even once”. Rule 17 (“paired ways of one action behave
identically”) has so far been held only by the harness t741: it checks
that the item and the badge lead to ONE command. Here something else is checked, the only
thing for which the live scenarios were set up — that one can REACH
the item with a mouse in a real window, and that the consequence is VISIBLE in the frame.

PATH (spec `ui-reach`, “A track has the full set of verbs…”):
1. right click on an empty spot of the UPPER video track → in the “Mute” menu →
   in the preview the green becomes red (the lower track is visible);
2. the same way, “Mute” once more → green came back (the gesture is repeated);
3. right click on the LOWER track → “Solo” → only it is in the frame, red;
   once more — solo lifted;
4. “Lock” from the menu → a drag of an event on the locked track does not move it;
   “Lock” once more — lifted;
5. “Properties…” from the menu → the track window opened; the “Mute” checkbox in it
   colors the frame red, the blend mode “Add” — yellow;
6. “Cancel” in the window → both the checkbox and the mode were rolled back in one step, and
   the frame is green again.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def two_video_tracks(dest, red, green):
    """Two video tracks: green ABOVE red (in the OTIO the upper one is the last).

    The same fixture as `track.py`: without the second track “muted the upper one” is
    indistinguishable from “muted everything” — the frame would simply be empty."""
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


def lane_menu(app, lane):
    """Open the row menu with a right click.

    We aim at an EMPTY spot of the track (events are at 0..6 s, we press at 8 s):
    a right click on an event would open the event menu, not the row’s.

    The item name in `click_probe` is written as a LITERAL at the call site, not
    carried here in a variable: the live coverage report reads the scenario sources
    and would not expand the variable — the place would count as not clicked, although
    it is clicked by it too (caught by this same scenario)."""
    app.right_click(app.x_at(8 * SEC), app.lane_y("video", lane, 0.6))


def main(out):
    require_tools()
    red = color_media(out, "red", "C81E1E", seconds=6)
    green = color_media(out, "green", "1EA81E", seconds=6)
    path = two_video_tracks(os.path.join(out, "trackverbs.otio"), red, green)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    def is_green(px):
        return px[1] > 120 and px[0] < 100

    def is_red(px):
        return px[0] > 120 and px[1] < 100

    with Live("trackverbs", out, project=path) as app:
        cx, cy = app.preview_center()
        # the cursor INTO THE BODY of the events: 8 s of material is no longer there, and the frame would be empty
        app.click(app.x_at(2 * SEC), app.ruler_y())
        top = app.pixel_settled((cx, cy))
        app.expect(is_green(top), f"start: the green track on top, RGB {top}")

        # --- 1. “Mute” FROM the row MENU -------------------------------
        lane_menu(app, 0)
        app.click_probe("track.menu.mute", "“Mute” item in the row menu")
        muted = app.pixel_settled((cx, cy), differs_from=top)
        app.expect(is_red(muted), f"under the track muted from the menu the red is visible: RGB {muted}")
        app.step("menu-mute", "row menu → “Mute”: the lower, red track is in the frame")

        # --- 2. and the same item CANCELS the mute (rule 14) -------------
        lane_menu(app, 0)
        app.click_probe("track.menu.mute", "“Mute” item a second time")
        back = app.pixel_settled((cx, cy), differs_from=muted)
        app.expect(is_green(back), f"the item worked again, green once more: RGB {back}")

        # --- 3. “Solo” on the BOTTOM track ---------------------------------
        lane_menu(app, 1)
        app.click_probe("track.menu.solo", "“Solo” item in the menu of the lower row")
        solo = app.pixel_settled((cx, cy), differs_from=back)
        app.expect(is_red(solo), f"solo from the menu left only the lower one in the frame: RGB {solo}")
        app.step("menu-solo", "row menu → “Solo”: only the lower track is in the frame")
        lane_menu(app, 1)
        app.click_probe("track.menu.solo", "lift the solo with the same item")
        app.pixel_settled((cx, cy), differs_from=solo)

        # --- 4. “Lock” from the menu refuses the drag -------------------------
        before = app.live_clips()
        lane_menu(app, 1)
        app.click_probe("track.menu.lock", "“Lock” item in the menu of the lower row")
        y = app.lane_y("video", 1, frac=0.6)
        app.drag(app.x_at(2 * SEC), y, app.x_at(4 * SEC), y)
        after = app.live_clips()
        app.expect(
            before == after,
            f"the track locked FROM THE MENU did not hand the event over to the drag: {before} → {after}",
        )
        app.step("menu-lock", "row menu → “Lock”: the event does not move with the drag")
        lane_menu(app, 1)
        app.click_probe("track.menu.lock", "lift the lock with the same item")

        # --- 5. “Properties…” and the checkboxes of the window ITSELF --------------------------
        lane_menu(app, 0)
        app.click_probe("track.menu.props", "“Properties…” item in the row menu")
        app.expect(app.has_probe("track.name"), "track window opened from the row menu")
        app.step("menu-props", "row menu → “Properties…”: the track window of the upper track")

        was = app.pixel_settled((cx, cy))
        app.click_probe("track.mute", "the “Mute” checkbox in the track window")
        win_muted = app.pixel_settled((cx, cy), differs_from=was)
        app.expect(is_red(win_muted), f"the checkbox of the window muted the track: RGB {win_muted}")
        app.click_probe("track.mute", "lift the “Mute” checkbox")
        app.pixel_settled((cx, cy), differs_from=win_muted)
        # solo and lock — the same checkboxes; the consequence of solo has been shown above,
        # here what matters is that they can be reached by the mouse and they switch
        app.click_probe("track.solo", "the “Solo” checkbox in the track window")
        app.click_probe("track.solo", "lift the “Solo” checkbox")
        app.click_probe("track.lock", "the “Lock” checkbox in the track window")
        app.click_probe("track.lock", "lift the “Lock” checkbox")

        # the blend mode: green over red in “Add” gives YELLOW
        blended_from = app.pixel_settled((cx, cy))
        app.click_probe("track.blend.add", "blend mode “Add”")
        blended = app.pixel_settled((cx, cy), differs_from=blended_from)
        app.expect(
            blended[0] > 120 and blended[1] > 120,
            f"adding green to red gives yellow: RGB {blended}",
        )
        app.step("window-blend", "track window: blend mode “Add” — the frame is yellow")

        # --- 6. “Cancel” rolls back the window session in ONE step --------------
        app.click_probe("track.cancel", "the “Cancel” button in the track window")
        undone = app.pixel_settled((cx, cy), differs_from=blended)
        app.expect(
            is_green(undone),
            f"“Cancel” returned the track as it was: RGB {undone} against the start {top}",
        )
        app.step("window-cancel", "track window: “Cancel” — the frame is green again")
        doc = app.saved_project().doc
        blends = [
            t.get("metadata", {}).get("kt", {}).get("blend")
            for t in doc["tracks"]["children"]
        ]
        app.expect(
            not any(b == "add" for b in blends),
            f"“Cancel” did not leave the blend mode in the file: {blends}",
        )

        # --- 7. “Effects…” from the row menu lead to THE SAME form ------------
        # “Cancel” closed the window, so the second path opens it from scratch:
        # the person gets into the track chain strip without going into properties.
        lane_menu(app, 0)
        app.click_probe("track.menu.fx", "“Effects…” item in the row menu")
        app.expect(app.has_probe("track.name"), "“Effects…” opened the track window")
        app.expect(app.has_probe("fxreg.add"), "in the open window there is a chain strip")
        app.step("menu-fx", "row menu → “Effects…”: the track window with the chain strip")
        app.click_probe("track.close", "close button of the track window")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/trackverbs"))

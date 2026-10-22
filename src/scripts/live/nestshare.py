#!/usr/bin/env python3
"""Live scenario R.50: one reel in three places — an edit changes ALL of them.

This is exactly the meaning of the owner’s decision “a reference to an object inside the project”:
a montage placed in several places is edited once. The reconnaissance
named the common trouble of Premiere, FCP and VEGAS that on a clip on the timeline it is NOT
VISIBLE how many more places will change together with it — so here we also
check the “×N” badge, and that it appears ONLY when there are more places than
one.

The consequence is checked IN THE FILE: the reel body is inlined into each instance,
and after an edit inside, all three inlines must become identical — otherwise
a foreign editor would see THREE DIFFERENT montages where the person edited one.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def stacks_of(doc):
    """Instances of the reel in the EXCHANGE FILE: the claim here is precisely about what
    a foreign editor will see, so the export door is read (R.70), not the project file —
    in the latter the reel lies ONCE and an instance is a reference."""
    return [
        c
        for track in doc["tracks"]["children"]
        for c in track.get("children", [])
        if c.get("OTIO_SCHEMA", "").startswith("Stack")
    ]


def main(out):
    require_tools()
    path = os.path.join(out, "nestshare.otio")
    proj = project_with_clips(
        path, [(0.0, 2.0, "synth:sine:440"), (2.0, 2.0, "synth:ramp")]
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nestshare", out, project=proj) as app:
        y = app.lane_y("video", 0, 0.6)
        # --- collapse both events into one reel ------------------------
        app.click(app.x_at(1 * SEC), y)
        app.click(app.x_at(3 * SEC), y, mods=("ctrl",))
        app.right_click(app.x_at(3 * SEC), y)
        app.click_probe("nest.collapse", "collapse item")
        app.expect(len(app.live_clips()) == 1, f"collapsed not into one: {app.live_clips()}")
        app.shot("one")

        # --- put the same reel in twice more -------------------------
        # THE HAND PATH — the “Edit” menu: the system clipboard may not exist
        # at all in the virtual screen, and the menu item calls the same body
        # as the chord (rule 17)
        inst = app.live_clips()[0]
        app.click(app.x_at(inst[0] + inst[1] // 2), y)
        app.click_probe("menu.edit", "“Edit” menu button")
        app.click_probe("edit.copy", "“Copy” item")
        app.expect(app.geom().get("clipboard") == 1, "copy did not fill the buffer")
        for at in (5.0, 9.0):
            app.click(app.x_at(int(at * SEC)), app.ruler_y())
            poll_step(0.3)
            app.click_probe("menu.edit", "“Edit” menu button")
            app.click_probe("edit.paste", "“Paste” item")
            poll_step(0.6)
        app.shot("three")
        app.expect(
            len(app.live_clips()) == 3,
            f"the reel must stand in THREE places, but it stands in {len(app.live_clips())}",
        )

        # --- badge ×N ---------------------------------------------------
        # it lives on the event’s caption; we check it via a file-independent
        # sign: the program itself counts the places and gives them in the dump.
        # A FULL entry — a menu item (R.55, C5): from this round a double click
        # expands in place, and the program gives the places counter in the dump
        # precisely for the place YOU ARE STANDING IN.
        app.right_click(app.x_at(inst[0] + inst[1] // 2), y)
        app.click_probe("nest.enter", "item “Enter the reel”")
        nest = app.geom()["nest"]
        app.expect(
            nest["uses"] == 3,
            f"the program counted {nest['uses']} places instead of three — “how many will change "
            f"together” has nothing to show it on",
        )

        # --- edit INSIDE --------------------------------------------
        inside = app.live_clips()
        first = inside[0]
        # Y IS RECOUNTED: inside the reel the rows have a different number, and the row
        # height is different — a coordinate taken from outside hits the wrong
        # row. Outside this was not visible while the scenario entered the reel
        # straight after collapsing.
        y = app.lane_y("video", 0, 0.6)
        x0 = app.x_at(first[0] + first[1] // 2)
        # first a SINGLE click: the egui menu closes on the next click and
        # swallows its press, and a drag right after the menu item then does not
        # start at all (not a defect of the program — the habit of the popup)
        app.click(x0, y)
        app.drag(x0, y, x0 + 70, y)
        moved = app.live_clips()
        app.expect(
            moved != inside,
            f"the edit inside did not go through: was {inside}, became {moved}",
        )
        app.key("Escape")
        app.key("Escape")
        app.expect(app.geom()["nest"]["depth"] == 0, "did not come out")
        app.shot("edited")

        # --- consequence: ALL THREE places are the same -----------------------
        st = stacks_of(app.saved_project().doc)
        app.expect(len(st) == 3, f"in the file there are not three instances, but {len(st)}")
        bodies = [json.dumps(s.get("children"), sort_keys=True) for s in st]
        app.expect(
            len(set(bodies)) == 1,
            "the bodies of the three instances DIVERGED: the edit inside reached not all of them, and a foreign "
            "editor would see three different montages",
        )
        seqs = {s["metadata"]["kt"]["seq"] for s in st}
        app.expect(len(seqs) == 1, f"the instances look at DIFFERENT reels: {seqs}")

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/nestshare"))

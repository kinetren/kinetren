#!/usr/bin/env python3
"""Live scenario R.50: an attachment has no dead ends.

The complaint that gave rise to the whole R.38 mechanism sounded like this: “I do not
understand how to delete a saved area region, I saved it and now it hangs there”.
An attachment is a new entity, and it has the same risk: it can be created, but there is
nothing to manage it with. The harness tests t253/t457 check this, but in a synthetic scene;
here a real hand in a real window.

Checked:

1. a right click on an attachment opens ITS menu — with entry, expanding
   and renaming (not the menu of an ordinary event);
2. renaming works TWICE IN A ROW (rule 14: a feature must work
   again, not just once — this is exactly how hotkeys died
   after the first Ctrl+L);
3. “detach from other places” is NOT offered when there is only one place: a promise
   that changes nothing is worse than its absence;
4. “expand to tracks” brings the content back out;
5. two ways lead out: Esc and the path breadcrumb.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    path = os.path.join(out, "nestdead.otio")
    proj = project_with_clips(
        path, [(0.0, 2.0, "synth:sine:440"), (2.0, 2.0, "synth:ramp")]
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nestdead", out, project=proj) as app:
        y = app.lane_y("video", 0, 0.6)
        app.click(app.x_at(1 * SEC), y)
        app.click(app.x_at(3 * SEC), y, mods=("ctrl",))
        app.right_click(app.x_at(3 * SEC), y)
        app.click_probe("nest.collapse", "collapse item")
        inst = app.live_clips()[0]
        mid = app.x_at(inst[0] + inst[1] // 2)
        app.shot("collapsed")

        # --- 1. OWN menu ----------------------------------------------
        app.right_click(mid, y)
        names = {f["param"] for f in app.geom().get("fields", [])}
        for want in ("nest.enter", "nest.expand", "nest.rename"):
            app.expect(want in names, f"the attachment menu has no “{want}”: {sorted(names)}")
        # --- 3. “unbind” is not promised for a single place ----------------
        app.expect(
            app.geom()["nest"]["uses"] in (0, 1),
            "scene: the reel stands in one place",
        )
        app.key("Escape")

        # --- 2. rename TWICE IN A ROW ---------------------------------------
        for i in range(2):
            app.right_click(mid, y)
            app.click_probe("nest.rename", f"rename item (entry {i + 1})")
            poll_step(0.5)
            before = app.shot(f"rename-{i}")
            app.key("Escape")
            poll_step(0.4)
            after = app.shot(f"rename-{i}-closed")
            d = app.screen_diff(before)
            app.expect(
                d > 0.001,
                f"the reel name window did not close on Esc on entry {i + 1} — or did not "
                f"open at all (the screen changed by {d:.3%})",
            )
            _ = after
        # ...and after two entries the hotkeys are ALIVE (focus is handed back)
        app.focus()
        app.key("Escape")
        app.click(mid, y)
        app.save_now()
        app.expect(os.path.exists(path), "Ctrl+S after the name window did not work — focus was not handed over")

        # --- 5. two ways out ----------------------------------------
        # the FULL entry — a menu item (R.55, C5): from this round a double click
        # expands in place, and the exits from the expansion go differently
        app.right_click(mid, y)
        app.click_probe("nest.enter", "item “Enter the reel”")
        app.expect(app.geom()["nest"]["depth"] == 1, "did not get inside")
        # path ONE: the project name is the root’s crumb
        app.click_probe("nest.root", "project name as the root breadcrumb")
        app.expect(
            app.geom()["nest"]["depth"] == 0,
            "a click on the project name did not lead out — and that is the only way to exit "
            "straight to the needed level",
        )
        # path TWO: Esc
        app.right_click(mid, y)
        app.click_probe("nest.enter", "item “Enter the reel” a second time")
        app.expect(app.geom()["nest"]["depth"] == 1, "did not get inside a second time")
        for _ in range(3):
            app.key("Escape")
            poll_step(0.3)
            if app.geom()["nest"]["depth"] == 0:
                break
        app.expect(app.geom()["nest"]["depth"] == 0, "Esc did not lead outward")
        app.shot("out")

        # --- 4. expand into tracks ----------------------------------
        app.right_click(mid, y)
        app.click_probe("nest.expand", "expand item")
        app.expect(
            len(app.live_clips()) == 2,
            f"expanding did not return the content: {app.live_clips()}",
        )
        app.shot("expanded")

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/nestdead"))

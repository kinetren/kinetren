#!/usr/bin/env python3
"""Live scenario R.33: the selection frame and moving a batch.

The user’s words (item 7): “the main timeline lacks area-selection…
one can select via area by holding ctrl… and this must naturally be
taken into account everywhere”. The harness tests t202-t205 check this in simulation, but here —
a REAL window: the mouse is driven by XTest with Ctrl held, and the result is read from the
.otio saved by the program.

The chain the person uses is checked: a marquee took an event →
a Ctrl+click added a second → one was dragged, BOTH moved, each by the same
shift, the durations intact.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "select.otio"),
        [(0.0, 4.0, "synth:Ramp"), (6.0, 4.0, "synth:Dc")],
    )

    with Live("select", out, project=proj) as app:
        y = app.lane_y("video", 0, frac=0.72)
        app.shot("open")

        p = app.project_now()
        c1, c2 = p.clip("c1"), p.clip("c2")
        app.expect(
            c1.start == 0 and c2.start == 6 * SEC, f"starting montage: {p.clips}"
        )

        # 1. Ctrl+drag from an EMPTY place (gap 4..6 s) to the right past the end of c2:
        # the frame must take c2 and NOT create a region
        app.drag(
            app.x_at(5 * SEC), y, app.x_at(11 * SEC), y, mods=("ctrl",)
        )
        app.shot("after-rubber")
        p = app.saved_project()
        region = (
            p.doc.get("metadata", {}).get("kt", {}).get("transport", {}).get("region")
        )
        app.expect(not region, f"Ctrl+drag created a region instead of a frame: {region}")

        # 2. Ctrl+click on the first event adds it to the selection
        app.click(app.x_at(2 * SEC), y, mods=("ctrl",))
        app.shot("after-ctrl-click")

        # 3. drag the FIRST event to the right by 2 s — both must move
        app.drag(app.x_at(2 * SEC), y, app.x_at(4 * SEC), y)
        app.shot("after-block-move")
        p = app.saved_project()
        c1, c2 = p.clip("c1"), p.clip("c2")
        app.near(c1.start, 2 * SEC, 27000, "the first event moved by 2 s")
        app.near(c2.start, 8 * SEC, 27000, "the second event of the batch moved by the same amount")
        app.near(c1.duration, 4 * SEC, 3000, "duration c1 intact")
        app.near(c2.duration, 4 * SEC, 3000, "duration c2 intact")
        app.expect(not p.transitions, f"moving the batch does not create transitions: {p.transitions}")

        # 4. ONE selected event wins over the region (ktprompts 26.10.18 item 4): c1 is at
        # 2–6 s, c2 at 8–12 s; a region over 1–9 s crosses both, one click selects c1, Copy takes
        # c1 WHOLE (the Edit menu: the virtual screen has no system clipboard, the item calls the
        # same body as Ctrl+C), Paste at 12.5 s (the view holds ~13 s) puts exactly it
        ry = app.ruler_y()
        app.drag(app.x_at(1 * SEC), ry, app.x_at(9 * SEC), ry)
        app.step("region-over-both", "a region dragged on the ruler over both events")
        app.click(app.x_at(3 * SEC), y)
        app.click_probe("menu.edit", "“Edit” menu button")
        app.click_probe("edit.copy", "“Copy” item")
        app.expect(app.geom().get("clipboard") == 1, "Copy took the ONE selected event, not the region")
        app.step("copied-one", "one event clicked and copied — the status line names the selected event")
        n0 = len(app.live_clips())
        app.click(app.x_at(int(12.5 * SEC)), ry)
        app.click_probe("menu.edit", "“Edit” menu button")
        app.click_probe("edit.paste", "“Paste” item")
        clips = app.live_clips()
        app.expect(len(clips) == n0 + 1, f"Paste put one event: {n0} → {len(clips)}")
        pasted = [c for c in clips if abs(c[0] - int(12.5 * SEC)) < SEC // 2]
        app.expect(
            len(pasted) == 1 and abs(pasted[0][1] - 4 * SEC) < 3000,
            f"the pasted event is WHOLE (4 s), not cropped to the region: {pasted}",
        )
        app.step("pasted-whole", "Edit → Paste: the event stands at 12.5 s, whole")

        # 5. a TRIPLE click on c2 sets the region over it (item 5) and leaves no FX window
        app.triple_click(app.x_at(10 * SEC), y)
        app.expect(app.wait_gone("win.fx"), "the triple click closed the FX window its second click opened")
        saved = app.saved_project()
        region = saved.doc.get("metadata", {}).get("kt", {}).get("transport", {}).get("region")
        # the earlier region was 1–9 s; the triple click puts it over c2 exactly (8–12 s)
        # the saved project gives the region as [start, end] in ticks
        ok = bool(region) and abs(region[0] - 8 * SEC) < 3000 and abs(region[1] - 12 * SEC) < 3000
        app.expect(ok, f"the triple click set the region over the event (8–12 s): {region}")
        app.step("triple-region", "triple click on the second event — the region band over it, no FX window")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/select"))

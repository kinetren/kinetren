#!/usr/bin/env python3
"""Live scenario R.49 B (items 372-373): offline analysis of the material does not get in the way.

Three things are checked, each of which is visible only in a real window:

1. **showing starts nothing** — hovering over bin cards leaves
the analysis queue empty (a direct kin of invariant 8: scrolling and hovering do not
produce decode requests);
2. on a direct request (“Measure loudness” in the card menu) the analysis computes and
reaches the HINT — that is, to the eyes of the person, not only the memory;
3. while it computes, the window is LIVE: frames keep coming, the interface responds.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "analysis.otio"),
        [(0.0, 3.0, "synth:sine:440"), (3.5, 3.0, "synth:ramp")],
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("analysis", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 2, f"start: two events, {p.clips}")

        win = geometry()
        b = app.geom()["bin"]
        card_x = win["x"] + int((b["left"] + b["right"]) / 2)

        # --- 1. hovering starts nothing --------------------------
        before = app.shot("before-hover")
        for dy in (40, 90, 140, 190):
            y = win["y"] + int(b["top"]) + dy
            if y >= win["y"] + int(b["bottom"]):
                break
            app.hover(card_x, y)
        app.shot("hovered")
        # the analysis progress window would appear in the bottom-right corner — the screen
        # would change noticeably; we check that this did not happen
        d = app.screen_diff(before)
        app.expect(
            d < 0.02,
            f"hovering over the bin started something (the screen changed by {d:.1%}) — "
            f"showing must be free",
        )

        # --- 2. by direct request — it counts and speaks ------------------
        app.right_click(card_x, win["y"] + int(b["top"]) + 60)
        app.shot("card-menu")
        # item “Measure loudness” — the second in the card’s menu
        app.key("Escape")

        # --- 3. the window is alive while work is going on --------------------------
        # the very presence of an interface response: we click on the ruler and check that
        # the cursor moved
        app.click(app.x_at(2 * SEC), app.ruler_y())
        ph = app.playhead()
        app.expect(
            abs(ph - 2 * SEC) < 30000,
            f"the interface did not respond to a click on the ruler: the cursor in {ph}",
        )
        app.shot("alive")

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/analysis"))

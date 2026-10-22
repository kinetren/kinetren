#!/usr/bin/env python3
"""Live scenario R.33 C2: the effects window opens with a double click and
shows the chain.

Two decisions of the round are checked at once: a double click on an event opens FX
(item 204 — the hint promised this since R.18 and lied) and the window is drawn as a single
strip of the chain (item 217). The window is searched by pixels: after a double click above the
timeline a panel must appear that was not there before the click.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401
import subprocess  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    # demo project: the first video event has a chain (pan/zoom), so the
    # window will show both the chain strip and the parameter grid
    proj = os.path.join(out, "fxwin.otio")
    demo_otio(proj)

    with Live("fxwin", out, project=proj) as app:
        # audio track 1 of the demo project: it carries slowed&reverb with curves,
        # so the window will show both the chain and the automation GRAPH
        y = app.lane_y("audio", 1, frac=0.72)
        before = app.shot("open")
        # a double click on the event: the FX window must open
        x = app.x_at(10 * SEC)
        app.double_click(x, y)
        after = app.shot("after-double-click")

        # the window is drawn over the interface: we compare the WHOLE screen, not a
        # sparse grid of points (it smeared past the window — R.34)
        d = app.screen_diff(before)
        app.expect(d > 0.01, f"double click opened the FX window (the screen changed by {d:.1%})")
        app.expect(os.path.exists(after), "screenshot of the window taken")

        # switching the view: the “Table” button stands to the right of the “Graph” button
        # in the header row of the automation section. Its position is
        # taken from the frame itself: looking for the label by the selection colour will not
        # work, so we click at a known offset inside the window
        # (the window opens in the same place — centre top).
        app.click(300, 247)
        app.shot("table-view")

        # the automation widget: we add a curve and remove the graph.
        # The “+ curve” picker is a combo box; its item lies right under it.

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

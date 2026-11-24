#!/usr/bin/env python3
"""Live scenario window_chrome (window-chrome, `v26.10` T5, T8, T9, U10, W3).

His words are in `docs/ktprompts/v26.10.txt`, lines 36, 47, 50, 51, 66. A real window, real clicks, OUR
fixture (a 3 s synthetic clip):

1. a double click on the event → its window, titled by the event; no strip beside the open chain;
2. the window's title dragged to the top, over the project's title → the title does not show through;
3. the window narrowed to 900: the wheel over the bottom row slides it, the wheel back puts it back;
4. the title strip double-clicked (maximized), the program closed and started again: it starts
   maximized; the strip dragged → the window is smaller than the screen and under the pointer.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

# it closes and starts the program: the stand's one is not its own to close
OWN_PROCESS = True
SEC = 90000


def field(app, name):
    return next((f for f in app.geom().get("fields", []) if f["param"] == name), None)


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "window_chrome.otio"), [(0.0, 3.0, "synth:Ramp")])
    with Live("window_chrome", out, project=proj, own_process=True) as app:
        # 1) the window of the event
        app.double_click(app.x_at(SEC), app.lane_y("video", 0, 0.6))
        app.expect(app.until(lambda: field(app, "win.fx")), "the effects window opened")
        app.step("event-window", "the effects window: the event's name in its title, #1 at the chain's head, no strip")

        # 2) over the project's title
        origin = geometry()
        app.hover_probe("title.project", "the project's title")
        title = field(app, "title.project")
        w = field(app, "win.fx")
        x0, y0 = origin["x"] + int(w["x"] + w["w"] / 2), origin["y"] + int(w["y"]) + 12
        app.drag(x0, y0, x0 + int(title["x"] - w["x"]) - 10, origin["y"] + 14)

        def over():
            w = field(app, "win.fx")
            return w and w["x"] <= title["x"] + title["w"] / 2 <= w["x"] + w["w"] and w["y"] <= title["y"] + title["h"]

        app.expect(app.until(over), f"the window stands over the project's title: {field(app, 'win.fx')} {title}")
        app.step("over-title", "the window at the top: its own title row, the project's title does not show through")
        app.key("Escape")
        app.expect(app.wait_gone("win.fx"), "the effects window closed")

        # 3) the narrow toolbar
        app.resize(900, 800)
        app.expect(app.until(lambda: field(app, "tb.more")), "the row does not fit: the “more” menu is there")
        more = field(app, "tb.more")
        app.expect(not app.has_probe("tb.slid", tries=2), "the row is not slid")
        app.step("narrow", "a 900 wide window: the bottom row ends in “more”")
        g = geometry()
        at = (g["x"] + int(more["x"]) - 120, g["y"] + int(more["y"] + more["h"] / 2))
        app.wheel(*at, ticks=12)
        app.expect(app.has_probe("tb.slid"), "the wheel slid the row")
        app.step("slid", "the wheel down: the row's end is in view, the transport stays")
        app.wheel(*at, ticks=-24)
        app.expect(app.wait_gone("tb.slid"), "the wheel back put the row back")
        app.resize(1440, 860)

        # 4) maximized, closed, started again
        app.double_click_probe("title.drag", "the title strip's free part")
        app.expect(app.until(lambda: geometry()["w"] > 1500), f"maximized: {geometry()}")
        app.close_window()
        app.expect(app.until(lambda: not app.alive()), "the program closed")

    with Live("window_chrome", out, project=proj, own_process=True) as app:
        app.expect(app.until(lambda: geometry()["w"] > 1500), f"it starts maximized: {geometry()}")
        full = geometry()
        app.expect(app.until(lambda: field(app, "title.drag")), "the title strip is published")
        strip = field(app, "title.drag")
        app.step("started-max", "the program starts maximized")
        x0, y0 = full["x"] + int(strip["x"] + strip["w"] / 2), full["y"] + int(strip["y"] + strip["h"] / 2)
        app.drag(x0, y0, x0 - 200, y0 + 120)
        app.expect(app.until(lambda: geometry()["w"] < 0.9 * full["w"]), f"dragged, it is smaller: {geometry()} of {full}")
        g = geometry()
        app.expect(g["x"] <= x0 - 200 <= g["x"] + g["w"], f"and under the pointer: {g}, pointer {x0 - 200}")
        app.step("dragged", "the drag restored the window to its normal size, under the pointer")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/window_chrome"))

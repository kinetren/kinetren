#!/usr/bin/env python3
"""Live scenario windows-fill-their-area (kkt-26.11.01 item 12): windows USE their area.

The owner: "if you make the Guide window bigger its elements will not take the whole area, and
again you look through a needle's eye on a big window … and so everywhere … I am tired of
reminding". The harness (t1125–t1132) holds the rule for every window; here the real program, at
a small and a large size, shows what the person sees:

1. a 1100-wide window: the bottom toolbar ends with "⋯", nothing is cut; the menu holds Render;
2. a 1700-wide window: no "⋯", the whole row is there;
3. the effects window at its default size, then dragged large by its corner — it follows;
4. "+ effect" in the large window: the gallery is as wide as the window;
5. the Guide at its default size, then dragged large; the keys page with its grid tables.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, demo_otio, geometry, require_tools  # noqa: E402

SEC = 90000


def rect(app, name):
    return {f["param"]: f for f in app.geom().get("fields", [])}.get(name)


def drag_corner(app, r, dx, dy):
    """Drag a window's bottom-right corner by (dx, dy) as a person does."""
    win = geometry()
    x0 = win["x"] + int(r["x"] + r["w"]) - 3
    y0 = win["y"] + int(r["y"] + r["h"]) - 3
    app.drag(x0, y0, x0 + dx, y0 + dy)
    app.settle()


def main(out):
    require_tools()
    path = os.path.join(out, "window_fill.otio")
    demo_otio(path)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("window_fill", out, project=path) as app:
        # 1) a narrow program window: the toolbar overflows into "⋯" instead of cutting Render
        app.resize(1100, 760)
        app.expect(app.has_probe("tb.more"), "a 1100-wide window: the toolbar ends with the “⋯” button")
        m = rect(app, "tb.more")
        if m:
            app.expect(m["x"] + m["w"] <= app.geom()["window"]["w"], "the “⋯” button is inside the window")
        app.step("toolbar-narrow", "1100 wide: the toolbar ends with “⋯” at the right edge, nothing cut")
        app.click_probe("tb.more", "the toolbar's “⋯” button")
        app.settle()
        app.step("toolbar-more", "the “⋯” menu: Undo, Redo, Render, Search, Diagnostics, Save")
        app.key("Escape")
        app.settle()

        # 2) a wide program window: the whole row, no "⋯"
        app.resize(1700, 1000)
        app.expect(not app.has_probe("tb.more", tries=4), "a 1700-wide window has no “⋯” button")
        app.step("toolbar-wide", "1700 wide: the whole row, Render in it")

        # 3) the effects window: default, then dragged large by its corner
        y = app.lane_y("video", 0, frac=0.5)
        app.double_click(app.x_at(15 * SEC), y)
        app.expect(app.has_probe("win.fx"), "the effects window opened")
        r0 = rect(app, "win.fx")
        app.step("fx-default", "the effects window at its default size")
        if r0:
            drag_corner(app, r0, 640, 300)
            r1 = rect(app, "win.fx")
            app.expect(
                r1 is not None and r1["w"] > r0["w"] + 450 and r1["h"] > r0["h"] + 200,
                f"the effects window followed its corner: {r0['w']:.0f}×{r0['h']:.0f} → "
                f"{(r1 or r0)['w']:.0f}×{(r1 or r0)['h']:.0f}",
            )
            app.step("fx-large", "the effects window dragged large: the chain and the fields stretch, "
                                 "the room is inside the scroll above Done/Cancel")

            # 4) the picker in the large window: one gallery as wide as the window
            app.click_probe("fxreg.add", "“+ effect”")
            app.expect(app.has_probe("fxreg.popup"), "the effect picker opened")
            p = rect(app, "fxreg.popup")
            if p and r1:
                app.expect(p["w"] >= r1["w"] * 0.85,
                           f"the gallery is as wide as its window: {p['w']:.0f} of {r1['w']:.0f}")
                app.expect(p["h"] > 520, f"the gallery uses the screen below it: {p['h']:.0f} tall")
            app.step("picker-large", "the gallery in the large window: presets, then effects, many "
                                     "cells per row, down the screen")
            app.key("Escape")
            app.settle()
        app.key("Escape")
        app.expect(app.wait_gone("win.fx"), "Esc closed the effects window")

        # 5) the Guide: default, then large, with the keys page and its grid tables
        app.click_probe("menu.help", "the Help menu")
        app.click_probe("help.guide", "Help → Guide…")
        app.expect(app.has_probe("guide.window"), "Help → Guide… opened the Guide")
        g0 = rect(app, "guide.window")
        app.step("guide-default", "the Guide at its default size: “I want to…” cards on the left")
        if g0:
            drag_corner(app, g0, 560, 330)
            g1 = rect(app, "guide.window")
            app.expect(g1 is not None and g1["w"] > g0["w"] + 400,
                       f"the Guide followed its corner: {g0['w']:.0f} → {(g1 or g0)['w']:.0f} wide")
            app.step("guide-large", "the Guide dragged large: the cards column widens a little, "
                                    "the page takes the rest, both run to the bottom")
        keys = next((f["param"] for f in app.geom().get("fields", [])
                     if f["param"].startswith("guide.page.")), None)
        # the keys page is the shortcuts reference: the page before What's new (guide::PAGES)
        pages = sorted((f["param"] for f in app.geom().get("fields", [])
                        if f["param"].startswith("guide.page.")),
                       key=lambda n: int(n.rsplit(".", 1)[1]))
        app.expect(bool(pages), "the Guide lists its pages")
        if pages:
            for name in pages:
                app.scroll_probe_into_view(name, keys or name)
            app.click_probe(pages[-2], "the “Keyboard and mouse” page")
            app.settle()
            app.step("guide-tables", "a guide page with its tables: shaded header, rules between "
                                     "the cells, the key column as wide as its keys")
        app.key("Escape")
        app.expect(app.wait_gone("guide.window"), "Esc closed the Guide")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out")

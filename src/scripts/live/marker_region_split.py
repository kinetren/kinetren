#!/usr/bin/env python3
"""Live scenario marker-region-split (the owner 2026-11-07, `v26.10` U7): the region-edge menu's
"Marker over the region" is always new, "Stretch the marker over the region" is its own item.

The harness (t3104) drives the menu in egui; here the real window:

1. a region 1–3 s (I, O); right click on its start edge → "Marker over the region": a 2 s range;
2. the item again removes the equal range; with the region cleared, M at 1 s puts a point marker
   there; the region again;
3. right click on the edge → "Marker over the region": the point marker stays, the status line names
   the stretch item;
4. right click → "Stretch the marker over the region": the marker spans the region.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def markers(app):
    return [tuple(m) for m in app.geom().get("markers", [])]


def main(out):
    require_tools()
    teal = color_media(out, "teal", "1E8C8C", seconds=6)
    path = project_with_clips(os.path.join(out, "marker_region_split.otio"), [(0.0, 6.0, teal)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("marker_region_split", out, project=path) as app:
        y = app.lane_y("video", 0)

        def region():
            app.click(app.x_at(1 * SEC), app.ruler_y())
            app.key("i")
            app.click(app.x_at(3 * SEC), app.ruler_y())
            app.key("o")

        def edge_menu():
            app.right_click(app.x_at(1 * SEC), y)

        # --- 1. a new range over the region -----------------------------------------------
        region()
        edge_menu()
        app.has_probe("tl.marker_region")
        app.step("menu-new", "right click on the region's edge: “Marker over the region” only")
        app.expect(app.has_probe("tl.marker_region"), "the menu offers “Marker over the region”")
        names = [f["param"] for f in app.geom().get("fields", [])]
        app.expect("tl.marker_stretch" not in names, "no stretch item with nothing to stretch")
        app.click_probe("tl.marker_region", "the item “Marker over the region”")
        made = app.until(lambda: markers(app) == [(1 * SEC, 2 * SEC)])
        app.step("range", "a 2 s range marker spans the region")
        app.expect(made, f"the item made a range over the region: {markers(app)}")

        # --- 2. a point marker at the region's start ------------------------------------------
        edge_menu()
        app.click_probe("tl.marker_region", "the item again: removes the equal range")
        app.until(lambda: markers(app) == [])
        app.key("Escape")  # the Esc cascade clears the region
        app.click(app.x_at(1 * SEC), app.ruler_y())
        app.key("m")
        app.key("Escape")
        point = app.until(lambda: markers(app) == [(1 * SEC, 0)])
        app.expect(point, f"a point marker at 1 s: {markers(app)}")
        region()

        # --- 3. the new item leaves it and says why --------------------------------------------
        edge_menu()
        two = app.has_probe("tl.marker_stretch")
        app.step("menu-two", "over a point marker the menu has both items")
        app.expect(two, "over a point marker the stretch item is offered")
        app.click_probe("tl.marker_region", "“Marker over the region” over the point marker")
        said = app.until(lambda: any("Stretch the marker" in n for n in app.geom().get("notices", [])))
        app.step("refused", "the point marker stays; the status line names the stretch item")
        app.expect(markers(app) == [(1 * SEC, 0)], f"the new item left the point marker: {markers(app)}")
        app.expect(said, f"the refusal names the stretch item: {app.geom().get('notices', [])}")

        # --- 4. stretch ------------------------------------------------------------------------
        edge_menu()
        app.click_probe("tl.marker_stretch", "the item “Stretch the marker over the region”")
        stretched = app.until(lambda: markers(app) == [(1 * SEC, 2 * SEC)])
        app.step("stretched", "the point marker now spans the region")
        app.expect(stretched, f"the stretch item stretched the marker: {markers(app)}")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/marker_region_split"))

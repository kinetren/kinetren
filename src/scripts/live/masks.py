#!/usr/bin/env python3
"""Live scenario shape-masks (kkt-26.11.01 S1): classic masks in a real window.

The owner's claim (verbatim, `docs/ktprompts/kkt-26.11.01.txt` line 66): “a convenient
mechanism of classic masks”. The harness holds the gestures (t1276–t1282); here a real mouse
on OUR fixture (a stripes pattern) and the PICTURE is what is measured.

PATH (ui-reach “Masks live where a person looks for them”, ui-timeline “Masks are drawn and
edited on the preview picture”):

1. click the video event → right click on the picture → Mask → Pen;
2. three clicks and a click-and-pull draw four points, a click on the first one closes the
   path — outside it the picture is gone;
3. the feather diamond above the path, pulled up — the edge softens;
4. effects window → the Geometry slot → Masks → the path's stopwatch; Done; the playhead moves;
   a point is dragged — the file holds the path keyed at two ticks;
5. the row's Invert — the inside and the outside swap;
6. “+ effect” → Duotone → its Masks → Ellipse → a drag over the picture — the effect shows only
   inside the ellipse; Ctrl+S — the slot's mask is in the file.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import (  # noqa: E402
    Live,
    geometry,
    pattern_media,
    poll_step,
    project_with_clips,
    require_tools,
)
from geomctl import CHANGED, frame_diff, picture  # noqa: E402

SEC = 90000


def masks_in(text):
    doc = json.loads(text)
    for c in doc["project"].get("clips", []):
        if c.get("masks"):
            return c["masks"]
    return {}


def wait_change(app, box, before, what, tries=14):
    now, d = before, 0.0
    for _ in range(tries):
        poll_step(0.35)
        now = picture(app, "wait", box)
        d = frame_diff(before, now)
        if d > CHANGED:
            break
    app.expect(d > CHANGED, f"{what} (fingerprint difference {d:.1f} at threshold {CHANGED})")
    return now


def centre(f):
    win = geometry()
    return win["x"] + int(f["x"] + f["w"] / 2), win["y"] + int(f["y"] + f["h"] / 2)


def field(app, pred, what):
    """The rectangle the program published for a place (polled: it is drawn on the next frame)."""
    for _ in range(20):
        f = next((f for f in app.geom().get("fields", []) if pred(f)), None)
        if f:
            return f
        poll_step(0.25)
    app.expect(False, what)
    return None


def frame_point(app, fx, fy):
    """A point of the picture under the mask editor (shares of the frame) on the screen."""
    f = field(app, lambda f: f["param"] == "mask.picture", "the mask editor's picture")
    win = geometry()
    return win["x"] + int(f["x"] + f["w"] * fx), win["y"] + int(f["y"] + f["h"] * fy)


def open_effects(app, x, y):
    """Double click on the event opens its effects window; under load the second click can fall
    out of the double-click window — then the person clicks again."""
    for _ in range(3):
        app.double_click(x, y)
        if app.has_probe("fx.slot.0", tries=8):
            return True
    return False


def main(out):
    require_tools()
    bars = pattern_media(out, "bars", seconds=8)
    path = project_with_clips(os.path.join(out, "masks.otio"), [(0.0, 8.0, bars)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("masks", out, project=path) as app:
        y = app.lane_y("video", 0, frac=0.5)
        app.click(app.x_at(2 * SEC), y)
        cx, cy = app.preview_center()
        box = (cx - 280, cy - 150, 560, 300)
        base = picture(app, "base", box)

        # 1) right click on the picture → Mask → Pen
        menu = False
        for _ in range(3):
            app.right_click(cx, cy)
            menu = app.has_probe("mask.menu", tries=8)
            if menu:
                break
            app.key("Escape")
        app.expect(menu, "the preview menu has the Mask item")
        app.click_probe("mask.menu", "Mask")
        app.click_probe("mask.menu.pen", "Mask → Pen")
        app.step("pen-armed", "right click on the picture → Mask → Pen")

        # 2) four points, the last one pulled smooth; a click on the first closes the path
        for n, (fx_, fy_) in enumerate(((0.30, 0.25), (0.70, 0.25), (0.75, 0.70))):
            # under load a click can be lost: the person sees no dot and clicks again
            for _ in range(3):
                app.click(*frame_point(app, fx_, fy_))
                if app.has_probe(f"mask.pen.{n}", tries=6):
                    break
        x0, y0 = frame_point(app, 0.30, 0.75)
        app.drag(x0, y0, x0 - 60, y0 - 30)
        app.step("pen-points", "four points, the last one smooth with its handles")
        first = field(app, lambda f: f["param"] == "mask.pen.0", "the first pen point")
        if first:
            app.click(*centre(first))
        app.expect(app.has_probe("mask.contour.0"), "the path closed into a mask")
        masked = wait_change(app, box, base, "outside the path the picture is gone")
        app.step("bezier", "a closed Bézier path cuts the picture")

        # 3) the feather: the diamond above the path, pulled up
        fe = field(app, lambda f: f["param"] == "mask.feather", "the feather handle")
        if fe:
            x, y0_ = centre(fe)
            app.drag(x, y0_, x, y0_ - 40)
            masked = wait_change(app, box, masked, "the feather softened the edge")
        app.step("feather", "the dashed outer edge — the feather")

        # the smooth point's handle bends the curve
        # the smooth point is the last one drawn: the vertex with the largest id
        vs = sorted(f["param"] for f in app.geom().get("fields", []) if f["param"].startswith("mask.v."))
        last = vs[-1] if vs else "mask.v.3"
        v3 = field(app, lambda f: f["param"] == last, "the smooth point")
        if v3:
            app.click(*centre(v3))
        out3 = field(app, lambda f: f["param"].startswith("mask.out."), "the selected point's handle")
        if out3:
            x, y_ = centre(out3)
            app.drag(x, y_, x + 120, y_ + 90)
            masked = wait_change(app, box, masked, "the handle bent the curve")
        app.step("tangent", "the handle of the smooth point pulled")

        # 4) the path's stopwatch in the Geometry node's Masks group, then a point at another tick
        app.expect(open_effects(app, app.x_at(2 * SEC), y), "the effects window opened")
        app.click_probe("fx.slot.0", "the Geometry node")
        app.expect(app.has_probe("grp.masks"), "the Geometry node has its Masks group")
        app.expect(app.has_probe("masks.row.0.key"), "the Masks group lists the path")
        app.scroll_probe_into_view("masks.row.0.key", "grp.masks", "the path's row", below="fx.done")
        app.click_probe("masks.row.0.key", "the path's stopwatch")
        app.step("stopwatch", "the stopwatch keyed the whole shape")
        app.click_probe("fx.done", "Done")
        app.click(app.x_at(5 * SEC), y)
        poll_step(0.5)
        v1 = field(app, lambda f: f["param"] == "mask.v.1", "a vertex handle of the path")
        if v1:
            vx, vy = centre(v1)
            app.drag(vx, vy, vx + 160, vy + 110)
            masked = wait_change(app, box, masked, "the dragged point bent the path")
        m = masks_in(app.saved_text())
        auto = (m.get("picture") or [{}])[0].get("auto", [])
        ticks = {k["t"] for c in auto for k in c.get("keys", [])}
        app.expect(len(ticks) >= 2, f"the path is keyed at two ticks in the file: {sorted(ticks)}")
        app.step("two-keys", "the path keyed at the first tick and at this one")

        # 5) Invert
        app.expect(open_effects(app, app.x_at(5 * SEC), y), "the effects window opened again")
        app.click_probe("fx.slot.0", "the Geometry node")
        app.scroll_probe_into_view("masks.row.0.invert", "grp.masks", "the path's row", below="fx.done")
        app.click_probe("masks.row.0.invert", "Invert")
        masked = wait_change(app, box, masked, "Invert swapped the inside and the outside")
        app.step("invert", "the mask inverted")

        # 6) an effect limited to a mask: + effect → Duotone → its Masks → Ellipse → drag
        app.click_probe("fxreg.add", "+ effect")
        for ch in "duotone":
            app.key(ch)
        app.click_probe("fxreg.tpl.duotone", "Duotone")
        poll_step(0.5)
        app.expect(app.has_probe("grp.masks"), "the effect's own Masks group is under its parameters")
        app.click_probe("grp.masks", "open the effect's Masks group")
        app.expect(app.has_probe("masks.tool.ellipse"), "the group shows the mask tools")
        app.scroll_probe_into_view("masks.tool.ellipse", "grp.masks", "the effect's mask tools", below="fx.done")
        app.click_probe("masks.tool.ellipse", "Ellipse of the effect's masks")
        app.click_probe("fx.done", "Done")
        before_slot = picture(app, "before-slot", box)
        # a long ellipse: its turn must show in the picture (a circle would turn unseen)
        x0, y0 = frame_point(app, 0.25, 0.45)
        x1, y1 = frame_point(app, 0.75, 0.58)
        app.drag(x0, y0, x1, y1)
        wait_change(app, box, before_slot, "the effect shows inside the ellipse")
        rot = field(app, lambda f: f["param"] == "mask.rot", "the ellipse's rotation handle")
        if rot:
            rx, ry = centre(rot)
            app.drag(rx, ry, rx + 160, ry + 120)
            poll_step(0.5)
        m = masks_in(app.saved_text())
        slots = m.get("slots") or []
        app.expect(any(len(s) == 1 for s in slots), f"the effect's mask is in the file: {slots}")
        rots = [m.get("rot", 0.0) for s in slots for m in s]
        app.expect(any(abs(r) > 5.0 for r in rots), f"the rotation handle turned the ellipse: {rots}")
        app.step("slot-mask", "the duotone only inside the ellipse")
        return app.done()


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out")

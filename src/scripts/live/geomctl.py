#!/usr/bin/env python3
"""Live scenario R.64: the on-screen geometry controls in a real window.

Claim of the owner (R.63 item 6): “AT MINIMUM we should have
editing directly in the preview that is convenient”. The harness holds the arithmetic and gestures
(t760–t764), but whether a person hit the thing with the mouse and whether the PICTURE changed, it
does not see at all. Here a real mouse:

1. right click on the frame → the item turns the controls on, the frame appears;
2. moving by the body, scaling by a corner and rotating — the preview PIXEL CHANGES;
3. moving the ANCHOR by its point — the preview picture does NOT change: the anchor moves, while
   the picture stays (the place where After Effects has a defect);
4. after Ctrl+S the file holds the new geometry numbers.

The coordinates of the handles are taken from the program ITSELF (`geom.*` in the geometry dump),
not guessed from the preview.
"""
import json
import os
import subprocess
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

SEC = 90000


def picture(app, tag, box):
    """FINGERPRINT of the frame: the preview area reduced to a grid of 14×8.

    Point probes do not fit: for colored stripes a move by 60 px leaves
    the point inside the same stripe, and a working gesture looks dead (as it
    turned out in the first run). The grid sees the SHIFT of boundaries, while the thin outline
    of the frame hardly falls into it — so “the anchor does not move the picture”
    remains a claim about the PICTURE, not about the outline."""
    x, y, w, h = box
    img = app.shot(tag)
    out = subprocess.run(
        [
            "convert", img,
            "-crop", f"{w}x{h}+{x}+{y}", "+repage",
            "-resize", "14x8!",
            "-depth", "8", "rgb:-",
        ],
        capture_output=True,
        check=True,
    ).stdout
    os.unlink(img)
    app._shot_no -= 1
    return list(out)


def frame_diff(a, b):
    """Mean difference of fingerprints (0 — the frame is the same)."""
    if not a or len(a) != len(b):
        return 255.0
    return sum(abs(x - y) for x, y in zip(a, b)) / len(a)


# the threshold “picture is different”: moving the strips changes the fingerprint by tens
# of levels, while stirring a thin outline — by fractions
CHANGED = 6.0


def wait_change(app, box, before, what, tries=14):
    """We wait for a CHANGE of the picture (rule pixel_settled: the preview legitimately holds
    the last frame, and an immediate read returns the previous one)."""
    now, d = before, 0.0
    for _ in range(tries):
        poll_step(0.35)
        now = picture(app, "wait", box)
        d = frame_diff(before, now)
        if d > CHANGED:
            break
    app.expect(d > CHANGED, f"{what} (fingerprint difference {d:.1f} at threshold {CHANGED})")
    return now


def probe_center(app, name, what):
    """Center of the rectangle given by the program, in screen coordinates."""
    for _ in range(20):
        f = next((f for f in app.geom().get("fields", []) if f["param"] == name), None)
        if f:
            win = geometry()
            return (
                win["x"] + int(f["x"] + f["w"] / 2),
                win["y"] + int(f["y"] + f["h"] / 2),
            )
        poll_step(0.25)
    app.expect(False, what)
    return None


def saved_geometry(text):
    """Geometry of the event from the project file — what is really written.
    R.70: the geometry is a field of the MODEL, and it is read from the project file
    (`app.project_text()`), not from the source of the scenario."""
    doc = json.loads(text)
    for c in doc["project"].get("clips", []):
        if c.get("geometry"):
            return c["geometry"]
    return {}


def main(out):
    require_tools()
    # “Ramp” — a gradient: any shift, scale or rotation changes the
    # pixel under the cursor, while a solid fill would not change at all.
    # “synth:” gives only sound and an EVEN green fill: on it the shift
    # and zoom are indistinguishable from their absence. Stripes are needed.
    bars = pattern_media(out, "bars", seconds=6)
    path = project_with_clips(os.path.join(out, "geomctl.otio"), [(0.0, 6.0, bars)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("geomctl", out, project=path) as app:
        y = app.lane_y("video", 0, frac=0.5)
        app.click(app.x_at(2 * SEC), y)  # we select the event and put the cursor into it
        cx, cy = app.preview_center()

        # --- 1. the frame is there AT ONCE, the button on the picture turns it off (R.67) -----
        #
        # PATH (spec ui-timeline, “the frame is there at once, the switch-off is remembered”):
        # open project → click the video event → the frame is at once on the picture →
        # the button in the corner of the preview turns it off → turns it back on.
        app.expect(app.has_probe("geom.frame"), "the frame is there at once, without a single action")
        app.step("frame-by-default", "the frame above the picture for the selected event")
        app.expect(app.has_probe("preview.geom_toggle"), "show button — in the corner of the frame")
        app.click_probe("preview.geom_toggle", "frame show button")
        app.expect(not app.has_probe("geom.frame", tries=4), "the button turned the frame off")
        app.step("frame-off", "frame turned off by the button")
        app.click_probe("preview.geom_toggle", "frame show button (back)")
        app.expect(app.has_probe("geom.frame"), "the button brought the frame back")
        app.shot("frame")

        # the measurement area — around the centre of the preview, certainly inside the picture
        box = (cx - 280, cy - 150, 560, 300)
        base = picture(app, "base", box)

        # --- 2. move past the body ------------------------------------------
        frame = next(f for f in app.geom()["fields"] if f["param"] == "geom.frame")
        win = geometry()
        bx = win["x"] + int(frame["x"] + frame["w"] * 0.32)
        by = win["y"] + int(frame["y"] + frame["h"] * 0.34)
        app.drag(bx, by, bx + 90, by + 40)
        moved = wait_change(app, box, base, "moving by the body changed the picture")
        app.shot("moved")

        # --- 3. scale by a corner --------------------------------------------
        tl = probe_center(app, "geom.tl", "the scale handle “top left” was given by the program")
        if tl:
            app.drag(tl[0], tl[1], tl[0] - 70, tl[1] - 40)
            moved = wait_change(app, box, moved, "scaling by a corner changed the picture")

        # --- 4. rotation ---------------------------------------------------
        rot = probe_center(app, "geom.rot", "the rotation handle was given by the program")
        if rot:
            app.drag(rot[0], rot[1], rot[0] + 80, rot[1] + 50)
            moved = wait_change(app, box, moved, "rotation changed the picture")
        app.shot("turned")

        before_anchor = picture(app, "before-anchor", box)

        # --- 5. ANCHOR: numbers change, the picture stands still ---------------------
        # no modifier: Alt+drag is intercepted by the WINDOW
        # MANAGER (in openbox — moving the window), the gesture does not reach the program
        anchor = probe_center(app, "geom.anchor", "the anchor point was given by the program")
        saved_before = dict(saved_geometry(app.project_text()))
        if anchor:
            app.drag(anchor[0], anchor[1], anchor[0] - 55, anchor[1] + 35)
            still = picture(app, "still", box)
            d = frame_diff(before_anchor, still)
            app.expect(
                d <= CHANGED,
                f"moving the anchor must NOT move the picture (fingerprint difference {d:.1f})",
            )
        app.shot("anchor")

        # --- 6. write to disk -------------------------------------------
        g = saved_geometry(app.saved_text())
        app.expect(bool(g), "event geometry written to the file")
        app.expect(
            abs(g.get("pos_x", 0.0)) > 1e-4 or abs(g.get("pos_y", 0.0)) > 1e-4,
            f"the move from the gesture lies in the file: {g}",
        )
        app.expect(
            abs(g.get("zoom", 1.0) - 1.0) > 1e-4 and abs(g.get("rot", 0.0)) > 1e-4,
            f"the scale and rotation from the gestures lie in the file: {g}",
        )
        app.expect(
            abs(g.get("anchor_x", 0.5) - 0.5) > 1e-4 or abs(g.get("anchor_y", 0.5) - 0.5) > 1e-4,
            f"the anchor move is recorded (the numbers changed, the picture did not): {g}, was {saved_before}",
        )

        # --- 7. FORM of the “Geometry” node: scheme, rule, reset -------------
        #
        # R.68, task 5.3: before the frame scheme, the row of conform rules and the reset
        # button the person NEVER reached them in the check — the node’s form lives in the
        # effects window, while this scenario worked only over the picture. Here too
        # it arrives: the frame has already been moved by gestures, and “Reset” must bring back
        # the picture — this is the consequence the person sees.
        after_gestures = picture(app, "before-reset", box)
        app.double_click(app.x_at(2 * SEC), y)
        app.expect(app.has_probe("fx.slot.0"), "the effects window opened with the chain strip")
        app.click_probe("fx.slot.0", "the leading “Geometry” node in the chain strip")
        app.expect(app.has_probe("geo.reset"), "the shape of the “Geometry” node is shown")
        app.step("geo-form", "effects window: the shape of the leading “Geometry” node")
        # the frame scheme and the row of rules — targets of HOVERING: the hint for both
        # lives in the status line (rule 10 — no pop-ups)
        app.hover_probe("geo.schema", "frame diagram in the node shape")
        app.expect(bool(app.geom().get("status", "")), "hovering over the diagram said it in words")
        app.hover_probe("geo.conform", "row of conform rules of the leading node")
        app.expect(bool(app.geom().get("status", "")), "hovering over the row of rules said it in words")
        app.click_probe("geo.reset", "“Reset” button of the node shape")
        reset_px = picture(app, "after-reset", box)
        app.expect(
            frame_diff(after_gestures, reset_px) > CHANGED,
            "“Reset” returned the picture: the fingerprint differs from what the gestures gave",
        )
        app.step("geo-reset", "“Reset” of the node shape returned the frame to the original geometry")
        g = saved_geometry(app.saved_text())
        app.expect(
            abs(g.get("zoom", 1.0) - 1.0) < 1e-4 and abs(g.get("rot", 0.0)) < 1e-4,
            f"“Reset” reached the file: {g}",
        )
        return app.done()


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out")

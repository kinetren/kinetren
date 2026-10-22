#!/usr/bin/env python3
"""Live scenario R.62: a vertical source in a 16:9 project.

The harness tests t715/t718/t722 hold the conform rule in a SYNTHETIC
scene. Here the real window answers the question of the owner (“4:3 in 16:9
gets stretched in a stupid way”) the way a person would see it:

1. a vertical file 360×640 in a 1280×720 project is shown FITTED: the red
   fill takes a narrow column in the middle of the preview, not the whole frame;
2. placing the same file from the bin gives a hint in the status line —
   with the file name, dimensions and the address where the decision changes (rule 19);
3. “File / Project settings…” opens, the “fill” rule is applied,
   and the preview changes: the fill grows to the full width of the frame.

The width of the fill is measured ON SCREEN (a red run along the line through the center of
the preview), not by the internal field — the consequence is what is asserted.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def red_run(app, cy, tag):
    """Length of the red run along the line y=cy in one screenshot."""
    img = app.shot(tag)
    out = subprocess.run(
        ["convert", img, "-crop", f"0x1+0+{cy}", "+repage", "-depth", "8", "rgb:-"],
        capture_output=True,
        check=True,
    ).stdout
    n = len(out) // 3
    red = [out[3 * i] > 150 and out[3 * i + 1] < 90 and out[3 * i + 2] < 90 for i in range(n)]
    best = cur = 0
    for r in red:
        cur = cur + 1 if r else 0
        best = max(best, cur)
    return best


def main(out):
    require_tools()
    vert = color_media(out, "vert", "C81E1E", seconds=6, size="360x640")
    path = project_with_clips(os.path.join(out, "conform.otio"), [(0.0, 6.0, vert)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("conform", out, project=path) as app:
        cx, cy = app.preview_center()
        app.click(app.x_at(2 * SEC), app.ruler_y())
        r, g, b = app.pixel_settled((cx, cy))
        app.expect(r > 150 and g < 90, f"preview center — the red fill of the source: RGB {r},{g},{b}")
        fit = red_run(app, cy, "fit")
        # a 9:16 fitted into 16:9 takes 9/16·9/16 ≈ 32 % of the frame width
        win_w = app.geom()["window"]["w"]
        app.expect(0 < fit < win_w * 0.3, f"“fit”: the red column is narrow ({fit} px of {win_w})")

        # --- 2. placement from the bin → a hint about the mismatch ---------------
        app.click(app.x_at(7 * SEC), app.ruler_y())
        card = next((f for f in app.geom().get("fields", []) if f["param"] == "bin.file"), None)
        app.expect(card is not None, "the file card is shown in the bin")
        if card:
            from kt_live import geometry

            win = geometry()
            app.right_click(
                win["x"] + int(card["x"] + card["w"] / 2),
                win["y"] + int(card["y"] + card["h"] / 2),
            )
            app.click_probe("bin.place", "the “Place” item on the file card")
            status = ""
            for _ in range(12):
                status = app.geom().get("status", "")
                if "360" in status:
                    break
                poll_step(0.2)
            app.shot("placed")
            app.expect(
                "360" in status and "640" in status and "1280" in status,
                f"the hint names the source and the project: “{status}”",
            )
            # a file with sound is placed as a PAIR of video+audio: 1 + 2 = 3 events
            placed = app.live_clips()
            app.expect(
                len(placed) == 3 and any(s == 7 * SEC for s, _ in placed),
                f"second instance started after 7 s: {placed}",
            )

        # --- 3. settings window: the “fill” rule ---------------------------
        app.click(app.x_at(2 * SEC), app.ruler_y())
        app.click_probe("menu.file", "“File” menu")
        app.click_probe("file.project_settings", "“Project settings…” item")
        app.expect(app.has_probe("settings.conform.fill"), "the settings window showed the rules")
        app.shot("settings")
        # ALL FOUR RULES are clicked with the mouse (R.68, task 5.3): before this round
        # the live check knew from the window exactly one — “fill” — and the other
        # three the person NEVER reached. The choice is the window’s state,
        # it goes into the project only on “Apply”, so the enumeration is safe
        # and is confirmed by the same consequence as before.
        app.click_probe("settings.conform.as_is", "rule “as is”")
        app.click_probe("settings.conform.stretch", "rule “stretch”")
        app.click_probe("settings.conform.fit", "rule “fit”")
        app.click_probe("settings.conform.fill", "rule “fill”")
        app.click_probe("settings.apply", "“Apply” button")
        app.expect(not app.has_probe("settings.apply", tries=2), "the window closed after applying")
        r2 = app.pixel_settled((cx, cy))
        app.expect(r2[0] > 150 and r2[1] < 90, f"after changing the rule the center is still red: {r2}")
        fill = red_run(app, cy, "fill")
        app.expect(
            fill > fit * 2,
            f"“fill” stretched the fill to the width of the frame: {fill} px against {fit} px",
        )
        saved = app.saved_project()
        doc = saved.doc if hasattr(saved, "doc") else None
        if doc is not None:
            conform = doc["metadata"]["kt"].get("settings", {}).get("conform")
            app.expect(conform == "fill", f"the project rule was written to the file: {conform}")

        # --- 4. RASTER AND FRAME RATE are edited by hand (R.68, task 5.3) ----
        # The raster and fps fields are the second thing the live check never reached
        # once: the window was opened only for the conform rule. Here numbers are
        # TYPED into them with the real keyboard, and the consequence is looked for in the file —
        # project 1280×720@30 becomes 640×480@25.
        app.click_probe("menu.file", "“File” menu a second time")
        app.click_probe("file.project_settings", "“Project settings…” item a second time")
        app.expect(app.has_probe("settings.width"), "the settings window showed the raster fields")
        # the “Frame” group collapses and expands by its header (form contract R.66):
        # the header is the same gesture target as a field
        app.click_probe("grp.settings.frame", "heading of the “Frame” group — collapse")
        # the collapse in egui is ANIMATED: while it moves, the fields are still drawn and
        # honestly publish their rectangles. We wait until a FRESH dump stops carrying
        # them, instead of asking once at random (R.69).
        app.expect(
            app.wait_gone("settings.width"),
            "the collapsed group hid its fields",
        )
        app.click_probe("grp.settings.frame", "heading of the “Frame” group — expand")
        app.expect(app.has_probe("settings.width"), "the expanded group brought the fields back")
        for probe, text, what in (
            ("settings.width", "640", "frame width field"),
            ("settings.height", "480", "frame height field"),
            ("settings.fps", "25", "frame rate field"),
        ):
            app.click_probe(probe, what)
            app.key("ctrl+a")
            app.type_text(text)
        app.shot("settings-raster")
        app.click_probe("settings.apply", "“Apply” button after editing the raster")
        app.expect(
            not app.has_probe("settings.apply", tries=2),
            "the window closed after applying the raster",
        )
        saved = app.saved_project()
        st = saved.doc["metadata"]["kt"].get("settings", {})
        app.expect(
            (st.get("width"), st.get("height")) == (640, 480),
            f"the entered raster reached the file: {st.get('width')}×{st.get('height')}",
        )
        app.expect(
            abs(float(st.get("fps", 0)) - 25.0) < 1e-6,
            f"the entered frame rate reached the file: {st.get('fps')}",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/conform"))

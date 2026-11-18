#!/usr/bin/env python3
"""Live scenario bin-scrub-timecode (F-MED-008, `v26.10` MW1): scrubbing a bin card writes the
file's moment under the cursor.

The harness (t3100) reads the painted text; here the real window shows it:

1. the pointer hovers a file card at two thirds of its thumbnail: a dark chip appears over the
   bottom of the BRIGHT thumbnail (the pixel at the chip's edge is dark, the card's middle is not);
2. the pointer moves to a quarter: the chip is still there, over the same thumbnail;
3. the pointer leaves the card: the chip is gone from the dump.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, geometry, project_with_clips, require_tools  # noqa: E402


def field(app, name):
    return next((f for f in app.geom().get("fields", []) if f["param"] == name), None)


def main(out):
    require_tools()
    bright = color_media(out, "bright", "E6E6E6", seconds=12)
    path = project_with_clips(os.path.join(out, "bin_scrub_tc.otio"), [(0.0, 12.0, bright)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("bin_scrub_tc", out, project=path) as app:
        app.until(lambda: field(app, "bin.file") is not None)
        card = field(app, "bin.file")
        app.expect(card is not None, "the file card is shown in the bin")
        if not card:
            return app.done()
        win = geometry()
        cy = win["y"] + int(card["y"] + card["h"] / 2)

        def at(frac):
            return win["x"] + int(card["x"] + card["w"] * frac)

        # --- 1. hover at two thirds ------------------------------------------------
        app.hover(at(0.66), cy)
        shown = app.until(lambda: field(app, "bin.scrub.tc") is not None)
        app.step("hover", "the pointer over the card: a timecode chip at the bottom of the thumbnail")
        app.expect(shown, "the scrub writes a timecode over the thumbnail")
        chip = field(app, "bin.scrub.tc")
        if chip:
            inside = (
                chip["x"] >= card["x"] - 1
                and chip["x"] + chip["w"] <= card["x"] + card["w"] + 1
                and chip["y"] + chip["h"] <= card["y"] + card["h"] + 1
            )
            app.expect(inside, f"the chip stays inside the thumbnail: {chip} in {card}")
            # the chip's backing over a bright thumbnail is dark; the thumbnail above it is bright
            r, g, b = app.pixel(win["x"] + int(chip["x"] + 1), win["y"] + int(chip["y"] + chip["h"] / 2))
            app.expect(max(r, g, b) < 120, f"the chip's backing is dark over the bright card: RGB {r},{g},{b}")
            r, g, b = app.pixel(win["x"] + int(card["x"] + 4), win["y"] + int(card["y"] + 4))
            app.expect(min(r, g, b) > 150, f"the thumbnail above the chip stays bright: RGB {r},{g},{b}")

        # the chip itself is over the card: hovering it keeps the scrub (and the chip) alive
        app.hover_probe("bin.scrub.tc", "the chip's rectangle")
        app.expect(field(app, "bin.scrub.tc") is not None, "hovering the chip keeps it")

        # --- 2. move to a quarter --------------------------------------------------
        app.hover(at(0.25), cy)
        app.step("move", "the pointer moved left: the chip names an earlier moment")
        app.expect(field(app, "bin.scrub.tc") is not None, "the chip follows the pointer")

        # --- 3. leave the card ------------------------------------------------------
        app.hover(at(0.5), win["y"] + int(card["y"] + card["h"] + 160))
        gone = app.until(lambda: field(app, "bin.scrub.tc") is None)
        app.step("leave", "the pointer left the card: no chip")
        app.expect(gone, "the chip leaves with the pointer")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/bin_scrub_tc"))

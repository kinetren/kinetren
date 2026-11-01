#!/usr/bin/env python3
"""Live scenario camera-isp-simulation (kkt-26.11.01 request I1): the camera effect in the REAL program.

The owner: "a real physical effect of a camera sensor's ISP — the whole path from RGB to Bayer, the
calibrations, the denoisers, to YUV". OUR fixture (a 3 s synthetic ramp), real clicks:

1. a double click on the event → the effects window;
2. "+ effect…", type "camera" → the picker shows Camera ISP;
3. a click on "Camera ISP" → the window shows the camera as its pipeline (lens, sensor, ISP raw,
   demosaic, colour, YUV), the preview is the simulated camera's picture;
4. untick "White balance" in the ISP's raw block → the preview turns green (a raw's colour: the
   green pixels are the most sensitive, nothing balances them any more).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def greenness(rgb):
    r, g, b = rgb
    return g - (r + b) / 2


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "camera.otio"), [(0.0, 3.0, "synth:Ramp")])

    with Live("camera", out, project=proj) as app:
        app.step("open", "a 3 s ramp on the video track")
        before = app.shot("before")
        app.double_click(app.x_at(1 * SEC), app.lane_y("video", 0, frac=0.5))
        app.expect(app.screen_diff(before) > 0.01, "the effects window opened")
        app.step("fxwin", "a double click on the event: its effects window")

        if not app.click_probe("fxreg.add", "the picker button"):
            return app.done()
        app.type_text("camera")
        app.expect(app.has_probe("fxreg.tpl.camera_isp", tries=20), "the search shows Camera ISP")
        app.step("search", "“+ effect…”, typed “camera”: Camera ISP and its six camera presets")

        app.click_probe("fxreg.tpl.camera_isp", "Camera ISP")
        app.expect(app.has_probe("isp.stage.wb", tries=20), "the window shows the camera's pipeline")
        app.step("pipeline", "the window is the pipeline: source, lens, sensor, ISP raw, demosaic, lens correction, colour, YUV — each stage with its bypass, each correcting stage naming the defect it removes")

        # look inside: the raw mosaic, in its filter colours
        app.click_probe("isp.combo.view", "the View list")
        app.click_probe("isp.choice.view.raw", "Raw (ADC)")
        app.step("view-raw", "View: Raw (ADC) — the preview shows the Bayer mosaic in its filter colours")
        app.click_probe("isp.combo.view", "the View list")
        app.click_probe("isp.choice.view.output", "Output")

        app.key("Escape")
        poll_step(0.5)
        on = app.pixel_settled(app.preview_center())
        app.step("preview", f"the preview is the simulated camera's picture of the ramp (centre {on})")

        app.double_click(app.x_at(1 * SEC), app.lane_y("video", 0, frac=0.5))
        app.expect(app.has_probe("isp.stage.wb", tries=20), "the window opened again on the camera")
        app.click_probe("isp.stage.wb", "White balance in the ISP raw block")
        app.step("wb-off", "“White balance” unticked")
        app.key("Escape")
        poll_step(0.5)
        off = app.pixel_settled(app.preview_center(), differs_from=on)
        app.expect(
            on is not None and off is not None and greenness(off) > greenness(on) + 15,
            f"without white balance the picture is green: {on} → {off}",
        )
        app.step("green", f"without white balance the raw's green cast: centre {off}")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

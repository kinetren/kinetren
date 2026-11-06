#!/usr/bin/env python3
"""Live scenario camera-isp-simulation (kkt-26.11.01 request I1): the camera effect in the REAL program.

The owner: "a real physical effect of a camera sensor's ISP — the whole path from RGB to Bayer, the
calibrations, the denoisers, to YUV". OUR fixture (a 3 s synthetic ramp), real clicks:

1. a double click on the event → the effects window;
2. "+ effect…", type "camera" → the picker shows Camera ISP;
3. a click on "Camera ISP" → the window shows the camera as its pipeline (source, lens, sensor,
   ISP raw, demosaic, lens correction, colour, YUV), the preview is the simulated camera's picture;
4. View → "Raw (ADC)": the preview shows the mosaic in its filter colours; back to "Output";
5. the wheel down to the ISP's raw block, untick "White balance" → red and blue lose against green
   (a raw's colour: the green pixels are the most sensitive, nothing balances them any more).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


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
        # the gallery takes the keyboard on its first frame: type once it is drawn, again if the
        # first letters went before it
        poll_step(0.5)
        app.type_text("camera")
        if not app.has_probe("fxreg.tpl.camera_isp", tries=10):
            app.type_text("camera")
        app.expect(app.has_probe("fxreg.tpl.camera_isp", tries=20), "the search shows Camera ISP")
        app.step("search", "“+ effect…”, typed “camera”: Camera ISP and its six camera presets")

        app.click_probe("fxreg.tpl.camera_isp", "Camera ISP")
        app.expect(app.has_probe("isp.stage.wb", tries=20), "the window shows the camera's pipeline")
        app.step("pipeline", "the window is the pipeline: source, lens, sensor, ISP raw, demosaic, lens correction, colour, YUV — each stage with its bypass, each correcting stage naming the defect it removes")

        # look inside: the raw mosaic, in its filter colours
        out_px = app.pixel_settled(app.preview_center())
        app.click_probe("isp.combo.view", "the View list")
        app.expect(app.has_probe("isp.choice.view.raw", tries=20), "the View list opened")
        app.click_probe("isp.choice.view.raw", "Raw (ADC)")
        app.expect(app.wait_gone("isp.choice.view.raw"), "the list closed on the choice")
        raw_px = app.pixel_settled(app.preview_center(), differs_from=out_px)
        app.expect(raw_px is not None and raw_px != out_px, f"the preview shows another stage: {out_px} → {raw_px}")
        app.step("view-raw", "View: Raw (ADC) — the preview shows the Bayer mosaic in its filter colours")
        app.click_probe("isp.combo.view", "the View list")
        app.expect(app.has_probe("isp.choice.view.output", tries=20), "the View list opened")
        app.click_probe("isp.choice.view.output", "Output")
        app.expect(app.wait_gone("isp.choice.view.output"), "the list closed on the choice")

        # the preview stands beside the window (its centre is not under it)
        on = app.pixel_settled(app.preview_center())
        app.step("preview", f"the preview is the simulated camera's picture of the ramp (centre {on})")

        # the pipeline is longer than the window: the wheel brings the raw block's stage into view
        app.scroll_probe_into_view("isp.stage.wb", "win.fx", below="fx.cancel", tries=40,
                                   what="“White balance” scrolled into the window")
        app.step("wb-scrolled", "the wheel turned down: the ISP raw block, each stage with the defect it removes")
        app.click_probe("isp.stage.wb", "White balance in the ISP raw block")
        app.step("wb-off", "“White balance” unticked")
        off = app.pixel_settled(app.preview_center(), differs_from=on)
        # the raw's balance: green kept, red and blue lose by their quantum efficiency against green
        # (linear × 0.42/0.55 for red — about 17 levels off the ramp's 143 in sRGB)
        app.expect(
            on is not None and off is not None and off[0] < on[0] - 10 and abs(off[1] - on[1]) <= 4,
            f"without white balance red loses against green: {on} → {off}",
        )
        app.step("green", f"without white balance the raw's colour: red lost against green, centre {off}")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

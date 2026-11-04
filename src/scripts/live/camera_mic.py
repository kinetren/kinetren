#!/usr/bin/env python3
"""Live scenario sim-audio-pairs (kkt-26.11.03b D1): Camera ISP on a linked A/V pair in the REAL
program: the person adds it from the picker (to either half), sees its panel, and the saved file
holds the camera on the picture and its microphone on the audio.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, require_tools  # noqa: E402
from stream_sound import add_pair  # noqa: E402
from vhs_sound import linked_av, slots  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = os.path.join(out, "camera_mic.otio")
    linked_av(proj)
    with Live("camera_mic", out, project=proj) as app:
        app.double_click(app.x_at(3 * SEC), app.lane_y("video", 0, frac=0.5))
        app.step("fxwin", "double click on the video event → the effects window of the A/V pair")
        if not add_pair(app, "camera isp", "camera_isp", "camera_isp_sound"):
            return app.done()
        app.wait_frame(3, what="the panel after the pick", soft=True)
        app.step("panel", "picked → the camera's pipeline, ending in its Microphone block")
        text = app.saved_text()
        app.expect(len(slots(text, "camera_isp")) == 1, "the picture got the camera")
        app.expect(len(slots(text, "camera_isp_sound")) == 1, "the linked audio got the camera's microphone")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

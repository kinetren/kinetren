#!/usr/bin/env python3
"""Live scenario sim-audio-pairs (kkt-26.11.03b D1): Datamosh on a linked A/V pair in the REAL program.

The person opens the pair's effects, adds Datamosh from the picker (to either half) and sees its
panel with the stream's sound controls; the saved file holds the effect on the picture and its sound
on the audio.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, require_tools  # noqa: E402
from vhs_sound import linked_av, slots  # noqa: E402

SEC = 90000


def add_pair(app, query, video_kind, sound_kind):
    """The picker, typed `query`: click whichever half's row it offers, wait for the slot."""
    if not app.click_probe("fxreg.add", "a picker button"):
        return False
    for ch in query:
        app.key("space" if ch == " " else ch)
    app.step("picker", f"+ effect, typed “{query}” → the gallery shows the effect")
    cell = None
    for _ in range(40):
        params = {f["param"] for f in app.geom().get("fields", [])}
        cell = next((p for p in (f"fxreg.tpl.{video_kind}", f"fxreg.tpl.{sound_kind}") if p in params), None)
        if cell:
            break
        poll_step(0.25)
    app.expect(cell is not None, f"the picker shows the row of {video_kind}")
    return cell is not None and app.click_probe(cell, "the effect's row")


def main(out):
    require_tools()
    proj = os.path.join(out, "stream_sound.otio")
    linked_av(proj)
    with Live("stream_sound", out, project=proj) as app:
        app.double_click(app.x_at(3 * SEC), app.lane_y("video", 0, frac=0.5))
        app.step("fxwin", "double click on the video event → the effects window of the A/V pair")
        if not add_pair(app, "datamosh", "datamosh", "datamosh_sound"):
            return app.done()
        app.wait_frame(3, what="the panel after the pick", soft=True)
        app.step("panel", "picked → Datamosh's panel with its Sound part: the audio bitrate, spectral prediction")
        text = app.saved_text()
        app.expect(len(slots(text, "datamosh")) == 1, "the picture got the Datamosh")
        app.expect(len(slots(text, "datamosh_sound")) == 1, "the linked audio got the stream's sound")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

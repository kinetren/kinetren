#!/usr/bin/env python3
"""Live scenario R.52: a generator is created from the bin and lives as media.

A real window, real clicks: RMB on the bin background → “Create generator” →
“Solid color” (menu items are clicked by the rectangles that the program
itself gives — the scenario must not guess menu offsets, R.27/R.49);
the parameter window opened; a click on the card puts the clip on the cursor, and in the
preview a FRAME OF THE GENERATOR arrives (switching the colored ramp frame to a black
solid — this is the proof that the frame without a file arrived); Ctrl+S —
the file has the canonical gen: path and a first-class GeneratorReference.1.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "generator.otio"),
        [(0.0, 3.0, "synth:Ramp")],
    )

    with Live("generator", out, project=proj) as app:
        win = geometry()
        b = app.geom()["bin"]
        app.shot("open")

        # we read the synth clip’s frame BEFORE creating the generator: since R.53 the clip
        # of the generator lands under the cursor at once and covers this track
        preview = app.preview_center()
        app.click(app.x_at(int(1.5 * SEC)), app.ruler_y())
        ramp_px = app.pixel_settled(preview)
        app.expect(sum(ramp_px) > 30, f"frame of the synth clip is not black: {ramp_px}")
        app.click(app.x_at(0), app.ruler_y())

        # 1. create a generator from the bin background menu (R.52)
        app.right_click(
            win["x"] + int((b["left"] + b["right"]) / 2),
            win["y"] + int(b["bottom"]) - 10,
        )
        app.click_probe("gen.new", "“Create generator” item in the bin menu")
        # R.53: there are nineteen kinds, laid out by groups; an egui submenu
        # opens by HOVERING, a click on it would close the whole menu
        app.hover_probe("gen.new.group.bg", "“Background” group in the submenu")
        app.click_probe("gen.new.solid", "kind “Solid color” in the submenu")
        app.expect(
            app.has_probe("genw.color"),
            "the generator parameter window opened right after creation",
        )
        app.shot("created")

        # 2. R.53 (item 416): creation puts the clip under the cursor AT ONCE — in the bin
        # there is no need to walk behind the card; the title is not typed blindly
        clips = None
        for _ in range(20):
            clips = app.live_clips()
            if len(clips) == 2:
                break
            poll_step(0.25)
        app.expect(
            clips is not None and len(clips) == 2,
            f"creating a generator must place a clip: {clips}",
        )
        if clips and len(clips) == 2:
            app.near(clips[-1][1], 5 * SEC, 3000, "default duration 5 s")

        # 3. the cursor goes past the end of the synth clip — the generator frame is visible there
        app.click(app.x_at(4 * SEC), app.ruler_y())

        # 4. the generator frame has reached the preview: the coloured ramp frame has changed
        # to BLACK (the default solid colour). pixel_settled is not suitable here:
        # it legitimately does not trust black, and black is THE generator frame.
        gen_px = None
        for _ in range(30):
            gen_px = app.pixel(*preview)
            if gen_px != ramp_px:
                break
            poll_step(0.3)
        app.expect(
            gen_px is not None and max(gen_px) < 16,
            f"the generator frame (black solid) in the preview, not {gen_px}",
        )
        app.shot("placed")

        # 5. in the EXPORT — a first-class generator scheme (R.70: the OTIO scheme
        # lives in the exchange file; in the project file the view and values are kept by the model)
        app.expect('"gen:solid' in app.saved_text(), "in the project file the generator is a kind in the media path")
        with open(app.project_now().otio_path) as f:
            text = f.read()
        app.expect(
            "GeneratorReference.1" in text and '"kt.solid"' in text,
            "the generator clip went out with the GeneratorReference.1 schema (export)",
        )
        app.expect(
            "available_range" in text.split("GeneratorReference.1", 1)[1][:800],
            "GeneratorReference has available_range",
        )

        # 6. COLOUR EDIT reaches the picture (R.53, item 412): the hex field of the
        # parameter window, Enter — and the preview pixel changes; the decoder
        # stream is not recreated meanwhile (this is asserted by t502)
        # the window is opened by a DOUBLE click on the card (R.53, item 416): it does not
        # multiply one clip, it opens the window.
        app.double_click_probe("bin.gen", "generator card in the bin")
        app.expect(app.has_probe("genw.color.hex"), "hex color field in the generator window")
        app.click_probe("genw.color.hex", "hex color field")
        # the field is cleared with BackSpace: Ctrl+A in the window may be intercepted by
        # the application itself, and then the typed text just gets appended
        for _ in range(10):
            app.key("BackSpace")
        for ch in "ff8800":
            app.key(ch)
        app.key("Return")
        lit = None
        for _ in range(40):
            lit = app.pixel(*preview)
            if lit[0] > 150 and lit[2] < 80:
                break
            poll_step(0.3)
        app.expect(
            lit is not None and lit[0] > 150 and lit[2] < 80,
            f"a color edit must reach the picture: {lit}",
        )
        app.shot("recolored")

        # 7. undo removes the colour edit entirely (the window session is one step)
        app.key("Escape")
        app.key("ctrl+z")
        back = None
        for _ in range(20):
            back = app.pixel(*preview)
            if max(back) < 40:
                break
            poll_step(0.3)
        app.expect(
            back is not None and max(back) < 40,
            f"undo must bring back the black solid: {back}",
        )
        app.expect(app.has_probe("bin.gen"), "generator card is alive after undo")
        app.shot("undone")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

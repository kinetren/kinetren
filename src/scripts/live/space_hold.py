#!/usr/bin/env python3
"""Live scenario R.72, USABILITY 2-4 (the space bar) and T11 (the frame step).

The owner, verbatim: “if I began holding space (and ctrl+space) outside playback, releasing the
space stops playback”. And R.16: “don't break the functions of the double and single press”.

The harness feeds the clock by hand; here the real output device runs it, and the
key really repeats while held, which is what the first Space handler broke on.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from Xlib import X, XK  # noqa: E402
from kt_live import Live, probe, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def space(down):
    """The bare key: a held key really repeats on the X server, and the one display connection of
    the driver stays on this thread, so a screenshot can be taken while it is down."""
    code = probe.d.keysym_to_keycode(XK.string_to_keysym("space"))
    probe.xtest.fake_input(probe.d, X.KeyPress if down else X.KeyRelease, code)
    probe.d.sync()


def held(app, ctrl, name, what):
    """A real hold, with the screenshot taken while the key is down. The length of the hold is
    how far the head has run: the program's own clock, not a sleep."""
    app.raise_once()
    if ctrl:
        app._mods(["ctrl"], True)
    start = app.playhead()
    space(True)
    # the head runs forward at first during playback, so a reverse is proven by its way back:
    # measured from the moment it turned, which a slow frame puts off by as much as it likes
    if ctrl:
        last = [start]

        def turned(t):
            back = t < last[0]
            last[0] = t
            return back

        app.wait_playhead(turned, "the head must turn back under the key", timeout=16.0)
    else:
        app.wait_playhead(lambda t: abs(t - start) > SEC // 2, "the head must run under the key")
    first = app.playhead()
    app.wait_playhead(lambda t: abs(t - first) > SEC // 3, "the head must keep running under the key")
    playing = app.playing()
    last = app.playhead()
    app.step(name, what)
    space(False)
    if ctrl:
        app._mods(["ctrl"], False)
    app.settle()
    return playing, last - first


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "space.otio"), [(0.0, 40.0, "synth:Ramp")])
    with Live("space_hold", out, project=proj) as app:
        app.click(app.x_at(10 * SEC), app.ruler_y())
        app.wait_playhead(lambda t: abs(t - 10 * SEC) < 3000, "the click places the cursor")
        app.step("paused", "paused at 10 s")

        start = app.playhead()
        playing, _ = held(app, False, "held", "Space held from pause: the picture plays")
        app.expect(playing, "holding Space from pause must play")
        app.expect(not app.playing(), "releasing Space must stop playback")
        reached = app.playhead()
        app.expect(reached > start + SEC, f"the head must run while held: {start} → {reached}")
        app.wait_frame(3, what="frames after the release")
        app.expect(app.playhead() == reached, "the head must stay where it got to")
        app.step("released", "released: the picture stands where it got to")

        playing, _ = held(app, True, "ctrl-held", "Ctrl+Space held: the picture runs backward")
        app.expect(playing, "holding Ctrl+Space must play")
        app.expect(not app.playing(), "releasing Ctrl+Space from pause must stop playback")
        back = app.playhead()
        app.expect(back < reached, f"Ctrl+Space must run BACKWARD: {reached} → {back}")
        app.step("ctrl-released", "released: stopped earlier in the picture")

        # the short press keeps playing; two quick presses stop at the start
        app.key("space")
        app.expect(app.until(lambda: app.playing()), "a short press must play")
        began = app.playhead()
        app.wait_playhead(lambda t: t > began + SEC // 2, "a short press must keep playing")
        app.expect(app.playing(), "a short press must keep playing")
        app.step("short-press", "a short press plays and keeps playing")

        # during playback: reverse while held, forward again after it
        playing, way = held(app, True, "ctrl-in-play", "Ctrl+Space during playback: backward")
        app.expect(playing and way < 0, f"under Ctrl+Space the head must run backward: {way}")
        app.expect(app.playing(), "the release during playback must keep playing")
        after = app.playhead()
        app.wait_playhead(lambda t: t > after, "after the release it runs forward again")

        # the double press: the second lands one frame after the first, not after the two a
        # gesture's wait costs, or a busy machine's frames alone outlast the double-press window
        app.key("space")
        seen = app._input_now()
        probe.key("space", pause=0.0, after=0.0)
        app.until(lambda: app._input_now() > seen)
        app.wait_playhead(lambda t: t == 0, "two quick presses stop at the project start")
        app.expect(not app.playing(), "two quick presses leave it stopped")
        app.step("double-press", "two quick presses: stopped at the start")

        # the frame step
        g = app.geom()
        tick = int(g["view_start"] + 300 * g["ticks_per_px"])
        app.click(app.x_at(tick), app.ruler_y())
        app.wait_playhead(lambda t: abs(t - tick) < 6000, "the click places the cursor")
        at = app.playhead()
        frame = SEC // 30
        for _ in range(3):
            app.key("Right")
        app.key("Left")
        app.wait_playhead(lambda t: abs(t - (at // frame + 2) * frame) <= 2, "→ → → ← is +2 frames")
        app.step("frame-step", "the arrows: two frames forward")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

#!/usr/bin/env python3
"""Live scenario R.57: EXACT transport speed.

The owner’s complaint verbatim:

    in slowed, and also in the timeline, the rate controller must let me set
    values without turning the range -0.25...0...0.25 into 0, i.e. I must be
    able to set 0.05 or -0.03, for example.

The harness already checks this (t46/t626-t629), but the harness is not the program: it
knows neither the window focus, nor the X11 keyboard, nor whether a typed number reaches the
PLAYER. So here everything is done with real
gestures in a real window.

The speed is asserted BY THE GEOMETRY DUMP (`base_rate`), not by the pixels of the
strip: an attempt to read the value from the picture would measure drawing, not state.
We judge the TEMPO only RELATIVELY (how far the cursor went at 1× against 0.05×) —
the debug build in software rendering is slower than real time, so the absolute numbers
mean nothing here (item 301).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, pause, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000
PLAY_WINDOW = 3.0


def speed_box(app):
    """Rectangle of the speed control — from the program ITSELF (R.27)."""
    for _ in range(20):
        f = next((f for f in app.geom().get("fields", []) if f["param"] == "speed"), None)
        if f:
            win = geometry()
            return (
                win["x"] + f["x"],
                win["y"] + f["y"],
                f["w"],
                f["h"],
            )
        poll_step(0.25)
    app.expect(False, "the program returned the rectangle of the speed control")
    return None


def base_rate(app):
    return float(app.geom().get("base_rate", 0.0))


def travel(app, seconds=PLAY_WINDOW):
    """How many ticks the cursor went through in the playback window.

    The count starts AFTER the cursor has really moved: the first
    run in software rendering goes to warm-up (decode, first texture
    fill), and a measurement “from pressing space” would measure that, not the tempo."""
    app.key("Home")
    app.key("space")
    app.wait_playhead(lambda t: t > 0, "cursor moved", timeout=15)
    t0 = app.playhead()
    pause(
        seconds,
        "the playback tempo is measured by REAL time: this much the program "
        "must play so that the cursor’s progress is comparable with the clock",
    )
    t1 = app.playhead()
    app.key("space")
    return t1 - t0


def type_text(app, s):
    """Type the number with real keys (keysyms, not Event::Text)."""
    table = {".": "period", ",": "comma", "-": "minus", "+": "plus"}
    for ch in s:
        app.key(table.get(ch, ch))


def main(out):
    require_tools()
    path = project_with_clips(os.path.join(out, "rate.otio"), [(0.0, 30.0, "synth:Ramp")])

    with Live("rate", out, project=path) as app:
        app.shot("open")
        x, y, w, h = speed_box(app)
        # the UPPER strip is the constant speed (the lower one is a shuttle with
        # auto-return, there is nothing to measure on it)
        upper_y = int(y + h * 0.25)
        cx = int(x + w / 2)

        app.expect(abs(base_rate(app) - 1.0) < 1e-3, f"start at 1×: {base_rate(app)}")
        fast = travel(app)
        app.expect(fast > 0, f"at 1× the cursor moves: {fast} ticks in {PLAY_WINDOW} s")

        # --- 1. DRAG of the strip to a low speed -----------------------------
        # we pull left of the centre to a point a few pixels right of it:
        # until R.57 this whole section was a dead zone and gave exactly 0, and
        # its edge immediately equalled 0.25×
        app.drag(cx - 40, upper_y, cx + 6, upper_y)
        slow = base_rate(app)
        app.shot("dragged")
        app.expect(
            0.0 < slow < 0.25,
            f"a drag at the center gave a representable SMALL speed: {slow}",
        )

        # --- 2. and the cursor really goes slower --------------------
        crawl = travel(app)
        app.expect(crawl > 0, f"at {slow}× the cursor still moves: {crawl} ticks")
        app.expect(
            crawl * 4 < fast,
            f"at {slow}× the cursor moves noticeably slower: {crawl} against {fast} ticks",
        )

        # --- 3. NUMBER INPUT: right click on the strip -----------------------
        app.right_click(cx, upper_y)
        type_text(app, "0.05")
        app.key("Return")
        app.shot("typed")
        got = base_rate(app)
        app.expect(abs(got - 0.05) < 1e-3, f"the typed 0.05 reached the model: {got}")

        # --- 4. Esc closes the field and changes NOTHING --------------------
        app.right_click(cx, upper_y)
        type_text(app, "2")
        app.key("Escape")
        after = base_rate(app)
        app.expect(abs(after - 0.05) < 1e-3, f"Esc left the speed unchanged: {after}")

        # --- 5. and after all this the space bar still toggles play --
        # (rule 14: the focus of the field must return, otherwise the hotkeys are dead)
        was = app.playing()
        app.key("space")
        app.expect(app.playing() != was, "space after closing the field toggles playback")
        app.key("space")
        app.shot("done")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

#!/usr/bin/env python3
"""Live scenario tour-lessons-fixes: all five lessons walked as a person (kkt-26.11.01 items 2–8).

The owner (2026-11-01): "Play a part in a loop… did not count until I closed the text generator's
window"; "the agents did not work" about the lesson text; "in all the other lessons there is not a
single hint"; "I do not understand what Show does … in lesson 4 Show opened the morpher, which
confuses the user, he still does not know where the morpher is"; lesson 3: "I moved the event and
nothing happens in the lesson"; lesson 4: "I added an effect, and the lesson did not tell me
something changed … I have to guess to bring the cursor back"; lesson 5: "you could make some silly
jokes in the lessons block while the render runs".

Its own process with its own store (the tour starts from the welcome window of a first start).
Every step takes a screenshot (`kt_live.step`); every verdict reads the program's own records: the
tour file in the store, the probes of the outlines (`lesson.mark.*`), the geometry dump's playhead.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, poll_step, require_tools  # noqa: E402

OWN_PROCESS = True
S = 90000


def read_json(path):
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def wait_for(pred, timeout=30.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        v = pred()
        if v:
            return v
        poll_step(0.25)
    return None


def main(out):
    require_tools()
    store = os.path.join(out, "store")
    if os.path.isdir(store):
        for name in os.listdir(store):
            p = os.path.join(store, name)
            if os.path.isfile(p):
                os.remove(p)

    def done(lesson):
        return ((read_json(os.path.join(store, "tour.json")) or {}).get("done") or {}).get(str(lesson)) or []

    def closed(lesson, step, timeout=20.0):
        return wait_for(lambda: step in done(lesson) or None, timeout) is True

    # the system save dialog does not exist in Xvfb: the answer is substituted, the gesture is real
    film = os.path.join(out, "lesson-5.mp4")
    if os.path.exists(film):
        os.remove(film)
    os.environ["KT_FILE_PICK"] = film
    with Live("lessons", out, own_process=True) as app:
        def rect(name):
            return app._probe_rect(name)

        def at(name):
            """The screen centre of a probe rect."""
            f = rect(name)
            win = geometry()
            return win["x"] + int(f["x"] + f["w"] / 2), win["y"] + int(f["y"] + f["h"] / 2)

        # the owner's working width: below ~1460 px the toolbar row cuts its Render button with or
        # without the lessons (a follow-up of item 12); the Loop button at 1440 is t1094's
        app.resize(1900, 1000)
        # --- lesson 1 ------------------------------------------------------------------------
        app.expect(app.has_probe("welcome.tour"), "the first start offers the tour")
        app.click_probe("welcome.tour", "Start the tour")
        app.expect(app.has_probe("lessons.panel"), "the lessons panel opened")
        app.expect(app.has_probe("lessons.step"), "the current step is drawn as its own lines")
        app.step("l1-start", "lesson 1: the step as lines with keycaps, the Show button")
        app.click_probe("lessons.show", "Show")
        app.expect(app.has_probe("lesson.mark.menu"), "Show opened the menu and the generator item is outlined")
        app.expect(not done(1), "Show did not perform the step")
        app.step("l1-show", "the Edit menu open, Create a generator… and its group outlined")
        app.hover_probe("gen.new", "Create a generator…")
        app.hover_probe("gen.new.group.titles", "Titles and countdown")
        app.click_probe("gen.new.text", "Text")
        app.expect(closed(1, 1), "the title step closed by itself")
        # the title's window stays OPEN on purpose for the loop step (kkt-26.11.01 item 2)
        app.expect(app.has_probe("win.fx"), "the title's window is open")
        app.step("l1-title", "a title event, step 1 checked; step 2 names the marker; the marker outlined")
        app.expect(app.has_probe("lesson.mark.marker"), "the CUT HERE marker is outlined")
        # the cut: a click on the marker puts the playhead on it (it once did nothing — t1093)
        app.click(app.x_at(6 * S), app.ruler_y())
        app.wait_playhead(lambda t: abs(t - 6 * S) < S // 30, "the click on the marker moved the playhead onto it")
        app.hover(app.x_at(20 * S), app.lane_y("video", 1, 0.5))
        app.key("s")
        app.expect(closed(1, 2), "the cut at the marker closed step 2")
        app.expect(app.has_probe("lesson.mark.clip"), "the event under NIGHT CITY is outlined")
        app.step("l1-cut", "the event cut at the marker; the event under NIGHT CITY outlined")
        y = app.lane_y("video", 0, 0.7)
        # grabbed clear of the title, which the generator menu laid over it at the playhead
        app.drag(app.x_at(62 * S), y, app.x_at(60 * S), y)
        app.expect(closed(1, 3), "moving the event closed step 3")
        app.expect(app.has_probe("lesson.mark.ruler"), "the ruler is outlined")
        app.expect(app.has_probe("lesson.mark.tb.loop") and app.has_probe("tb.loop"), "the loop button is outlined")
        app.step("l1-moved", "the event moved; the ruler and the loop button outlined")
        if not app.has_probe("win.fx", tries=2):
            app.double_click(app.x_at(90 * S), app.lane_y("video", 0, 0.7))
        app.expect(app.has_probe("win.fx"), "a window is open during the loop step")
        ry = app.ruler_y()
        app.drag(app.x_at(20 * S), ry, app.x_at(30 * S), ry)
        app.click_probe("tb.loop", "the outlined loop button")
        app.expect(closed(1, 4, 5.0), "the loop step closed with the window still open (item 2)")
        app.expect(app.has_probe("win.fx"), "the window is still open")
        app.expect(app.has_probe("lessons.result"), "the panel says what the loop does now")
        app.step("l1-loop", "region + loop: step 4 checked at once, the window still open, the result line")

        # --- lesson 2 ------------------------------------------------------------------------
        app.click_probe("lessons.next", "Next lesson")
        app.expect(wait_for(lambda: app.has_probe("lesson.mark.clip") or None) is True, "lesson 2 outlines an event")
        app.step("l2-start", "lesson 2: the first event outlined")
        app.click_probe("lessons.show", "Show")
        app.expect(not app.has_probe("win.fx", tries=2), "Show did not open the effects window")
        x, yy = at("lesson.mark.clip")
        app.double_click(x, yy)
        app.expect(closed(2, 1), "the double-click opened the effects and closed step 1")
        app.expect(app.has_probe("lesson.mark.fxreg.add"), "+ effect… in the window is outlined")
        app.step("l2-fx", "the effects window, + effect… outlined")
        app.click_probe("fxreg.add", "+ effect…")
        for ch in "blur":
            app.key(ch)
        app.click_probe("fxreg.tpl.blur", "Blur")
        app.expect(closed(2, 2), "the blur closed step 2")
        app.expect(app.has_probe("lessons.result"), "the panel says the event is blurred")
        ph = app.playhead()
        app.expect(0 <= ph < 12 * S, f"the playhead moved into the blurred event: {ph / S:.2f} s")
        app.step("l2-blur", "the blur added, the line says so, the playhead inside the event")
        app.expect(app.has_probe("lesson.mark.fx.auto_add") and app.has_probe("fx.auto_add"), "+ curve is outlined")
        app.click_probe("auto.add", "+ curve")
        app.click_probe("auto.add.radius", "radius")
        app.expect(closed(2, 3), "the curve closed step 3")
        app.step("l2-curve", "a curve on the blur, the lesson done")

        # --- lesson 3 ------------------------------------------------------------------------
        app.click_probe("lessons.next", "Next lesson")
        app.expect(wait_for(lambda: app.has_probe("lesson.mark.clip") or None) is True, "lesson 3 outlines the second event")
        app.step("l3-start", "lesson 3: the second event outlined, the text names Transition with previous")
        y = app.lane_y("video", 0, 0.7)
        app.drag(app.x_at(23 * S), y, app.x_at(20 * S), y)
        app.settle()
        app.expect(not done(3), "an overlap made by a drag did not close the transition step (it is a cut)")
        app.step("l3-overlap", "the events overlap as a cut; the step stays")
        app.right_click(app.x_at(26 * S), y)
        app.expect(app.has_probe("lesson.mark.menu"), "Transition with previous is outlined in the menu")
        app.step("l3-menu", "the right-click menu, Transition with previous outlined")
        app.hover_probe("lesson.mark.menu", "Transition with previous")
        app.click_probe("tl.trans.crossfade", "Crossfade")
        app.expect(closed(3, 1), "Transition with previous closed step 1")
        ph = app.playhead()
        app.expect(9 * S < ph < 15 * S, f"the playhead stands in the transition: {ph / S:.2f} s")
        app.expect(app.has_probe("lesson.mark.transition"), "the transition is outlined")
        app.step("l3-transition", "a transition, the result line, the playhead in it; the transition outlined")
        x, yy = at("lesson.mark.transition")
        app.double_click(x, yy)
        app.expect(app.has_probe("win.transition"), "the double-click opened the transition window")
        app.expect(app.has_probe("lesson.mark.tr.kind"), "the kind field is outlined")
        app.click_probe("tr.kind", "the outlined kind field")
        kinds = [f["param"] for f in app.geom().get("fields", []) if f["param"].startswith("trkind.") and f["param"] != "trkind.crossfade"]
        app.expect(bool(kinds), f"the kind list is open: {kinds[:5]}")
        if kinds:
            app.click_probe(kinds[0], "another kind")
        app.expect(closed(3, 2), "another kind closed step 2")
        app.step("l3-kind", "the transition has another kind, the lesson done")
        app.key("Escape")

        # --- lesson 4 ------------------------------------------------------------------------
        app.click_probe("lessons.next", "Next lesson")
        app.expect(app.has_probe("lessons.panel"), "lesson 4 opened")
        app.click_probe("lessons.show", "Show")
        app.expect(app.has_probe("lesson.mark.morpher"), "Show brought the BATCH block into view, outlined")
        app.expect(not app.has_probe("win.morpher", tries=2), "Show did not open the morpher (item 5)")
        app.expect(not done(4), "Show did not perform the step")
        app.step("l4-show", "the BATCH morpher block outlined, no window opened")
        x, yy = at("lesson.mark.morpher")
        app.double_click(x, yy)
        app.expect(closed(4, 1), "the double-click opened the morpher and closed step 1")
        app.expect(app.has_probe("lesson.mark.fxreg.add"), "+ effect… in the morpher window is outlined")
        app.click(app.x_at(2 * S), app.ruler_y())
        app.click_probe("fxreg.add", "+ effect…")
        for ch in "blur":
            app.key(ch)
        app.click_probe("fxreg.tpl.blur", "Blur")
        app.expect(closed(4, 2), "an effect in the batch closed step 2")
        app.expect(app.has_probe("lessons.result"), "the panel says the effect works under BATCH")
        ph = app.playhead()
        app.expect(48 * S <= ph < 80 * S, f"the playhead moved into the morpher's span: {ph / S:.2f} s")
        app.step("l4-effect", "the effect in the batch, the line says where, the playhead in the span")
        app.key("Escape")

        # --- lesson 5 ------------------------------------------------------------------------
        app.click_probe("lessons.next", "Next lesson")
        app.expect(app.has_probe("lesson.mark.tb.render") and app.has_probe("tb.render"), "the render button is outlined")
        app.click_probe("lessons.show", "Show")
        app.expect(app.has_probe("lesson.mark.menu"), "Show opened File with Render to MP4… outlined")
        app.step("l5-show", "the File menu, Render to MP4… outlined")
        app.click_probe("file.render", "Render to MP4…")
        app.expect(closed(5, 1), "the render settings closed step 1")
        app.expect(app.has_probe("lesson.mark.render.start"), "Start is outlined")
        app.step("l5-settings", "the render settings, Start outlined")
        app.click_probe("render.start", "Render…")
        app.expect(app.has_probe("lessons.joke"), "a joke shows while the render runs")
        app.step("l5-joke", "the render runs, the lessons panel jokes")
        first = rect("lessons.joke")
        poll_step(7.0)
        app.step("l5-joke-2", "a few seconds later, another joke")
        app.expect(rect("lessons.joke") is not None or closed(5, 2, 1.0), "the joke stays for the whole render")
        _ = first
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "/tmp/kt-live-lessons"))

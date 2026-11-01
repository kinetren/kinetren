#!/usr/bin/env python3
"""Live scenario welcome-window-and-tour: the welcome window, the Guide and the first tour step.

The owner (2026-10-28): "an intro window (as in programming IDEs) offering either to create a
project from a template or a list of recent projects (with thumbnails), and right there a tile
'start the tour'… a main tile where the most likely action lands: for the first opening it is
the tour". And (2026-10-27): "We have no guideline inside the program explaining what can be
done and where."

Its own process with its own store and XDG_DATA_HOME (the start itself is checked): the first
start shows the window with "Start the tour" primary → a template → Ctrl+S (the save path is
the scenario's own file) → Help → Welcome window… → the recent card of that project, with its
poster → Help → Guide… → search "loop" → Help → Start the tour → step 1 closes by adding a
title → Help → What's new: the start window in three panes, What's new scrolls alone, the window
dragged large → File › Preferences…: the start switch and Economy UI reach the store
(welcome-window-and-settings). A screenshot per step (`kt_live.step`); every verdict reads the
program's own records (the store files, the probes), not pixels.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, poll_step, require_tools  # noqa: E402

# ITS OWN PROCESS (R.68): the scenario checks the program START — the welcome window opens
# only at start, over an unnamed document, and from an empty store
OWN_PROCESS = True


def read_json(path):
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def rect(app, name):
    return {f["param"]: f for f in app.geom().get("fields", [])}.get(name)


def drag_corner(app, r, dx, dy):
    """Drag a window's bottom-right corner by (dx, dy) as a person does."""
    win = geometry()
    x0 = win["x"] + int(r["x"] + r["w"]) - 3
    y0 = win["y"] + int(r["y"] + r["h"]) - 3
    app.drag(x0, y0, x0 + dx, y0 + dy)
    app.settle()


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
    # a fresh store: the first start (Live creates the folder, the scenario empties it)
    if os.path.isdir(store):
        for name in os.listdir(store):
            p = os.path.join(store, name)
            if os.path.isfile(p):
                os.remove(p)
    made = os.path.join(out, "welcome-made.kt")
    if os.path.exists(made):
        os.remove(made)
    # the system save dialog does not exist in Xvfb: the answer is substituted, the gesture is real
    os.environ["KT_FILE_PICK"] = made
    with Live("welcome", out, own_process=True) as app:
        # --- 1. the first start: the welcome window, "Start the tour" primary ---------------
        app.expect(app.has_probe("welcome.window"), "the first start opened the welcome window")
        app.expect(app.has_probe("welcome.tour"), "the window has the tour tile")
        app.expect(app.has_probe("welcome.tpl.v916"), "the window has the 9:16 template tile")
        start = read_json(os.path.join(store, "start.json")) or {}
        app.expect(start.get("started_before") is True, f"the start record is written: {start}")
        app.step("first-start", "the welcome window over an empty editor, the tour tile primary")

        # --- 2. a template: an unnamed 1080×1920 project ------------------------------------
        app.click_probe("welcome.tpl.v916", "the 9:16 template tile")
        app.expect(app.wait_gone("welcome.window"), "a template click closed the welcome window")
        start = wait_for(lambda: (read_json(os.path.join(store, "start.json")) or {}).get("last_template"))
        app.expect(start == "v916", f"the last-used template is remembered: {start}")
        app.step("template", "a tall, narrow preview: the unnamed 9:16 project")

        # --- 2b. File → Project settings… → Save as template… ----------------------------
        app.click_probe("menu.file", "the File menu")
        app.click_probe("file.project_settings", "File → Project settings…")
        app.click_probe("settings.save_template", "Save as template…")
        app.click_probe("settings.template_name", "the template name field")
        # lower case: XTest typing of capitals depends on the virtual keyboard map
        app.type_text("shorts 30")
        app.click_probe("settings.template_save", "Save (the template)")
        tpl = wait_for(lambda: (read_json(os.path.join(store, "templates.json")) or {}).get("templates"))
        app.expect(
            bool(tpl) and tpl[0] == {"name": "shorts 30", "width": 1080, "height": 1920, "fps": 30},
            f"the template is saved with the project's frame: {tpl}",
        )
        app.step("template-saved", "Project settings: the template “shorts 30” saved")
        app.key("Escape")
        app.expect(app.wait_gone("settings.width"), "Esc closed Project settings")

        # --- 3. save it: it becomes the recent project, its poster goes to the store -------
        app.key("ctrl+s")
        app.expect(wait_for(lambda: os.path.exists(made)), "Ctrl+S wrote the project")
        recent = wait_for(lambda: (read_json(os.path.join(store, "recent.json")) or {}).get("recent"))
        app.expect(bool(recent) and recent[0]["name"] == "welcome-made", f"the saved project heads Recent: {recent}")
        # an empty project gets no poster (nothing to show); the placeholder card is the honest look
        app.expect(
            not any(n.startswith("welcome-made") and not n.endswith(".kt") and ".ktindex" not in n
                    for n in os.listdir(out)),
            f"no poster file appeared next to the project: {sorted(os.listdir(out))}",
        )

        # --- 4. Help → Welcome window… → the recent card --------------------------------
        app.click_probe("menu.help", "the Help menu")
        app.click_probe("help.welcome", "Help → Welcome window…")
        app.expect(app.has_probe("welcome.window"), "Help → Welcome window… reopened the window")
        app.expect(app.has_probe("welcome.recent.0"), "the window lists the saved project")
        app.step("reopened", "the welcome window again, the saved project as the first recent card")
        app.click_probe("welcome.guide", "the window's Guide link")
        app.expect(app.has_probe("guide.window"), "the Guide link opened the Guide")
        app.key("Escape")
        app.expect(app.wait_gone("guide.window"), "Esc closed the Guide (the top window)")
        app.expect(app.has_probe("welcome.window"), "the welcome window stays under the closed Guide")
        app.click_probe("welcome.recent.0", "the first recent card")
        app.expect(app.wait_gone("welcome.window"), "the recent card opened its project")
        app.step("recent-opened", "the recent project is open")

        # --- 5. Help → Guide… → "loop" ---------------------------------------------------
        app.click_probe("menu.help", "the Help menu")
        app.click_probe("help.guide", "Help → Guide…")
        app.expect(app.has_probe("guide.window"), "Help → Guide… opened the Guide")
        app.click_probe("guide.search", "the Guide's search field")
        app.type_text("loop")
        app.settle()
        app.step("guide-loop", "the Guide: “loop” leaves the loop entry and “Markers and the region”")
        app.key("Escape")
        app.expect(app.wait_gone("guide.window"), "Esc closed the Guide")

        # --- 6. Help → Start the tour → step 1 closes by adding a title -----------------
        app.click_probe("menu.help", "the Help menu")
        app.click_probe("help.tour", "Help → Start the tour")
        app.expect(app.has_probe("lessons.panel"), "the tour opened its lessons panel")
        app.step("tour", "the lesson project and the panel “Lesson 1 of 5 · The first edit”")
        app.click_probe("lessons.show", "the step's Show button")
        app.expect(app.has_probe("gen.new"), "Show opened the File menu with “Create a generator…”")
        app.step("show", "the File menu open, “Create a generator…” outlined in the accent colour")
        app.hover_probe("gen.new", "Create a generator…")
        app.hover_probe("gen.new.group.titles", "Titles and countdown")
        app.click_probe("gen.new.text", "Text")
        tour = wait_for(lambda: ((read_json(os.path.join(store, "tour.json")) or {}).get("done") or {}).get("1"))
        app.expect(tour == [1], f"step 1 closed by itself when the title appeared: {tour}")
        app.key("Escape")
        app.settle()
        app.step("title-added", "a title on the timeline, step 1 checked, step 2 “Cut the event at the marker”")

        # --- 7. "Start over" in the panel: the edited lesson asks first, then a fresh copy ----
        app.click_probe("lessons.start_over", "the panel's Start over")
        app.click_probe("ask.discard", "Don't save (the lesson's edits)")
        tour = wait_for(lambda: (read_json(os.path.join(store, "tour.json")) or {}).get("done") == {} or None)
        app.expect(tour is True, "Start over cleared the tour progress")
        app.step("start-over", "a fresh lesson copy, step 1 again")
        # and the same from the Help menu (no edits since: no question)
        app.click_probe("menu.help", "the Help menu")
        app.click_probe("help.start_over", "Help → Start the tour over")
        app.expect(app.has_probe("lessons.panel"), "the lessons panel is open after Help → Start the tour over")

        # --- 8. Help → What's new → the start window at its default and enlarged ---------
        # (welcome-window-and-settings: three panes, each scrolls alone, the window grows)
        app.click_probe("menu.help", "the Help menu")
        app.click_probe("help.whats_new", "Help → What's new")
        app.expect(app.has_probe("welcome.window"), "Help → What's new opened the welcome window")
        app.expect(app.has_probe("welcome.preferences"), "the window has the Preferences… door")
        w0 = rect(app, "welcome.window")
        app.step("whats-new", "the start window at its default size: Start | Recent | What's new")
        app.hover_probe("welcome.lesson.1", "the first lesson row")
        lesson = rect(app, "welcome.lesson.1")
        if w0:
            # What's new scrolls alone: the Start pane's lesson row stays where it is
            news_x = w0["x"] + w0["w"] - 120
            app.wheel(news_x, w0["y"] + w0["h"] / 2, ticks=6)
            app.settle()
            moved = rect(app, "welcome.lesson.1")
            app.expect(lesson is not None and moved is not None and abs(moved["y"] - lesson["y"]) < 1,
                       f"scrolling What's new left the Start pane in place: {lesson} → {moved}")
            app.step("whats-new-scrolled", "only the What's new pane scrolled")
            # to the program window's corner: an egui window cannot outgrow its viewport, and at
            # the default the start window already takes most of it (1180 of 1440 here)
            win = geometry()
            dx = int(win["w"] - 8 - (w0["x"] + w0["w"]))
            dy = int(win["h"] - 8 - (w0["y"] + w0["h"]))
            drag_corner(app, w0, dx, dy)
            w1 = rect(app, "welcome.window")
            # the corner stays under the hand (an anchored window re-centred itself and its corner
            # ran half as far as the pointer — t1290)
            off = None if w1 is None else (w1["x"] + w1["w"] - (w0["x"] + w0["w"] + dx - 3),
                                           w1["y"] + w1["h"] - (w0["y"] + w0["h"] + dy - 3))
            app.expect(w1 is not None and w1["w"] > w0["w"] and w1["h"] > w0["h"]
                       and abs(off[0]) < 8 and abs(off[1]) < 8,
                       f"the start window's corner followed the pointer: {w0} → {w1}, off by {off}")
            app.step("welcome-large", "the start window enlarged: the panes take the room")
        app.click_probe("welcome.close", "Close")
        app.expect(app.wait_gone("welcome.window"), "Close closed the welcome window")

        # --- 9. File › Preferences… → the start switch, off and on again --------------------
        app.click_probe("menu.file", "the File menu")
        app.click_probe("file.preferences", "File → Preferences…")
        app.expect(app.has_probe("prefs.window"), "File → Preferences… opened its window")
        app.step("preferences", "Preferences: Start, Interface, Privacy and updates, FFmpeg")
        app.hover_probe("prefs.telemetry", "Send anonymous usage statistics")
        app.click_probe("prefs.language", "the Language list")
        app.step("preferences-language", "the Language list: Auto (system) and five languages by their own names")
        app.click_probe("prefs.language", "the Language list again (closes it)")
        app.expect(app.has_probe("prefs.window"), "the list closed, Preferences stay")
        app.click_probe("prefs.welcome_at_start", "Open the welcome window at start")
        on = wait_for(lambda: (read_json(os.path.join(store, "start.json")) or {}).get("welcome_at_start") is False or None)
        app.expect(on is True, "unchecking the start switch reached the start record")
        app.click_probe("prefs.welcome_at_start", "Open the welcome window at start, again")
        on = wait_for(lambda: (read_json(os.path.join(store, "start.json")) or {}).get("welcome_at_start") is True or None)
        app.expect(on is True, "the start switch is on again")
        app.click_probe("prefs.economy", "Economy UI")
        eco = wait_for(lambda: (read_json(os.path.join(store, "start.json")) or {}).get("economy_ui") is True or None)
        app.expect(eco is True, "Economy UI reached the preferences record")
        app.step("preferences-set", "Preferences: the start switch on, Economy UI on")
        app.click_probe("prefs.close", "Close (Preferences)")
        app.expect(app.wait_gone("prefs.window"), "Close closed Preferences")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "/tmp/kt-live-welcome"))

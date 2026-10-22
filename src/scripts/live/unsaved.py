#!/usr/bin/env python3
"""Live scenario R.49 A6 (item 354): unsaved work is not thrown away silently.

Before R.49 the program asked nothing, and that was a legitimate choice: the background
write went DIRECTLY into the user’s file, so there was nothing to lose. The owner removed this —
together with the right not to ask.

Here is the real “File → New” menu with unsaved edits: “Stay” leaves the work
in place, “Do not save” releases it. There are three answers, not two,
precisely because “cancel” and “do not save” are DIFFERENT things, and confusing them
means losing the montage by pressing “Cancel”.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, import_project, poll_step, project_with_clips, require_tools  # noqa: E402

# ITS OWN PROCESS (R.68): the scenario checks the program START ITSELF, not the work in an
# already open project — the worker stand does not suit it.
OWN_PROCESS = True

SEC = 90000


def main(out):
    require_tools()
    # a PROJECT file (R.70): the scenario builds an exchange file, the program lives in .kt
    proj = import_project(project_with_clips(
        os.path.join(out, "unsaved.otio"),
        [(0.0, 2.0, "synth:Ramp"), (3.0, 2.0, "synth:Ramp")],
    ))
    before = open(proj).read()
    # the background save must not manage to interfere: the guard is checked, not it
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("unsaved", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 2, f"start: two events, {p.clips}")

        # an edit by a real gesture
        src = p.clips[1]
        y = app.lane_y("video", 0)
        x0 = app.x_at(src.mid())
        app.drag(x0, y, x0 + 160.0, y)
        app.shot("moved")
        moved = app.live_clips()
        app.expect(
            moved and max(s for s, _ in moved) > src.start,
            f"the event moved by gesture: {moved}",
        )

        # the window’s cross: the program MUST ASK, not close
        app.close_window()
        app.shot("asked")
        app.expect(app.alive(), "the window did not close without asking")
        app.expect(
            app.has_probe("ask.stay"),
            "closing with unsaved work ASKS, and does not throw away silently",
        )

        # “Stay” — the program is alive, the work is in place
        app.click_probe("ask.stay")
        app.shot("stayed")
        app.expect(app.alive(), "“Stay” closed the program anyway")
        after = app.live_clips()
        app.expect(
            after == moved,
            f"“Stay” kept the work in place: was {moved}, became {after}",
        )

        # the same question REPEATED (rule 14), now “Don’t save”
        app.close_window()
        app.shot("asked-again")
        app.expect(app.has_probe("ask.discard"), "the question is asked again")
        app.click_probe("ask.discard")
        for _ in range(40):
            if not app.alive():
                break
            poll_step(0.25)
        app.expect(not app.alive(), "“Do not save” must close the program")
        # and the project stayed as it was: only Ctrl+S changes it
        app.expect(
            open(proj).read() == before,
            "the project file is not touched by refusing to save",
        )

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/unsaved"))

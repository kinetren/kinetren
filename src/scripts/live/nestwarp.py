#!/usr/bin/env python3
"""Live scenario R.55 (C4): REVERSE and SLOW-DOWN on a composition.

The owner’s complaints verbatim: “slowedandreverb does not work on
precompositions” and “reverse does not work on a composition”.

Not “did not work” — it was FORBIDDEN: `validate` rejected a mirrored or
time-stretching instance of the reel, because the sub-mix was read strictly in
order. The command failed, and English technical text went to the status bar.
Now the sub-mix has a producer with a pre-roll, and both gestures pass.

The harness tests already check this on synthetic scenes (t428, t561-t565).
Here the question is different: does the GESTURE by hand reach the model, and does it survive
saving — that is, does the path work end to end, from the point where
the person acts.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, pause, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    path = os.path.join(out, "nestwarp.otio")
    proj = project_with_clips(
        path, [(0.0, 2.0, "synth:sine:440"), (2.0, 2.0, "synth:ramp")]
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nestwarp", out, project=proj) as app:
        y = app.lane_y("video", 0, 0.6)
        # --- collapse both events into a reel -------------------------------
        app.click(app.x_at(1 * SEC), y)
        app.click(app.x_at(3 * SEC), y, mods=("ctrl",))
        app.right_click(app.x_at(3 * SEC), y)
        app.click_probe("nest.collapse", "collapse item")
        app.expect(len(app.live_clips()) == 1, f"collapsed not into one: {app.live_clips()}")
        app.shot("collapsed")

        # --- REVERSE OF NESTING by hand, via the event menu ------------------
        inst = app.live_clips()[0]
        mid = inst[0] + inst[1] // 2
        app.right_click(app.x_at(mid), y)
        app.click_probe("tl.reverse", "item “Reverse”")
        app.shot("reversed")
        # CONSEQUENCE in the model, not “the command did not fall”
        app.key("ctrl+s")
        saved = app.saved_project()
        kt = saved.doc["metadata"]["kt"]
        app.expect(
            len(kt.get("sequences", [])) == 1,
            f"reel not recorded: {list(kt.keys())}",
        )
        # the reverse of the instance lies in the effects of the envelope’s Stack
        dump = str(saved.doc)
        app.expect(
            "LinearTimeWarp" in dump,
            "the attachment’s reverse did not reach the file — the command failed silently",
        )

        # --- PLAYS, and without starvation ------------------------------------
        before = app.geom().get("underruns", 0)
        app.click(app.x_at(int(0.5 * SEC)), app.ruler_y())
        app.key("space")
        pause(
            1.5,
            "playback runs in REAL time: one and a half seconds of play — "
            "this is the check itself, not a wait for the interface reaction",
        )
        app.key("space")
        after = app.geom().get("underruns", 0)
        app.expect(
            app.playhead() > int(0.5 * SEC),
            f"cursor did not move: {app.playhead()}",
        )
        app.expect(
            after - before < 200,
            f"the mirrored attachment starves: underruns increased by {after - before}",
        )
        app.shot("played")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

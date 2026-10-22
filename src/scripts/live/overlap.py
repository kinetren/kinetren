#!/usr/bin/env python3
"""Live scenario R.28: pushed over — nothing cut off — pulled apart — everything in place.

The user’s words (item 6): “NOTHING ANYWHERE SHOULD GROW BACK OR BE CUT OFF”.
With interleaving (the project default, item 162) an overlap of clips is legitimate: the crossfade
is heard from the overlap itself, and nobody trims. It is checked in the
REAL window: the mouse is driven by XTest, and the result is read from the .otio saved by the
program — the scenario does not trust its own ideas about where things
lie, and asks the project each time.

Previously this scenario (R.26, `handles`) checked the opposite — that an overlap cuts the
clips into a crossfade, and pulling apart returns the material. That mode is alive with
interleaving switched off and is covered by tests t139/t140/t159/t160.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000  # ticks per second


def main(out):
    require_tools()
    # DIFFERENT sources: the clips have different label colours, so it is visible whose
    # material is drawn in the overlap zone
    proj = project_with_clips(
        os.path.join(out, "overlap.otio"),
        [(0.0, 6.0, "synth:Ramp"), (8.0, 6.0, "synth:Dc")],
    )

    with Live("overlap", out, project=proj) as app:
        y = app.lane_y("video", 0, frac=0.72)  # below the clip name label
        app.shot("open")

        p = app.project_now()
        c1, c2 = p.clip("c1"), p.clip("c2")
        app.expect(
            c1.duration == 6 * SEC and c2.start == 8 * SEC,
            f"starting montage: {p.clips}",
        )

        # 1. overlap: we place the second clip so that it enters the first by 2 s
        target = c1.end - 2 * SEC
        app.drag(app.x_at(c2.mid()), y, app.x_at(c2.mid() + (target - c2.start)), y)
        p = app.saved_project()
        app.expect(not p.transitions, f"the overlap does NOT create a transition: {p.transitions}")
        for c in p.clips:
            app.near(c.duration, 6 * SEC, 3000, f"duration {c.name} is untouched")
        app.expect(
            all(c.source_in == 0 for c in p.clips),
            f"entry point did not move: {p.clips}",
        )
        a, b = p.clip("c1"), p.clip("c2")
        app.near(b.start, target, 3 * SEC // 10, "the second clip landed where it was pulled")
        app.expect(b.start < a.end, f"the overlap is there: {p.clips}")
        app.shot("after-overlap")

        # 2. in the overlap zone the top one is the one that STARTED LATER (Vegas rule,
        # item 177) — and the strip must show what the render will show
        body_a = app.pixel(app.x_at(a.start + (a.end - a.start) // 4), y)
        body_b = app.pixel(app.x_at(b.end - (b.end - b.start) // 4), y)
        mid = app.pixel(app.x_at((b.start + a.end) // 2), y)
        app.expect(body_a != body_b, f"sources are distinguishable by color: {body_a} / {body_b}")
        # the zone of a BARE overlap from R.51 is marked with hatching (item 399: the target of
        # the RMB gesture “Transition from…” must be visible) — the zone’s pixel is a bit
        # lighter than the clip body, comparison with a tolerance
        app.expect(
            all(abs(m - b) <= 12 for m, b in zip(mid, body_b))
            and any(abs(m - a) > 20 for m, a in zip(mid, body_a)),
            f"in the overlap, on top, the late clip: {mid} against {body_b} (the early {body_a})",
        )

        # 3. drag back — everything returns, nothing has grown
        c2 = p.clip("c2")
        target = a.end + 3 * SEC
        app.drag(app.x_at(c2.mid()), y, app.x_at(c2.mid() + (target - c2.start)), y)
        p = app.saved_project()
        app.expect(not p.transitions, f"transitions are still absent: {p.transitions}")
        for c in p.clips:
            app.near(c.duration, 6 * SEC, 3000, f"duration {c.name} intact")
        app.near(p.clip("c2").start, target, 3 * SEC // 10, "clip landed where it was pulled")
        app.expect(
            all(c.source_in == 0 for c in p.clips),
            f"entry point intact: {p.clips}",
        )
        app.shot("after-pull-apart")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

#!/usr/bin/env python3
"""Live scenario R.40 item 14: the ruler shows time and tick marks.

Complaint verbatim: “on the automation timeline, if the event/morpher
is small, the time is shown on the ruler only as 0:00, the very first one, which is dumb.
And if it is zoomed in, the time flies off to the side… the ruler must
show the next second too, and tick marks as well, which is desirable (and there are no tick marks
on the main timeline either).”

Both troubles have one cause: the ladder of steps started from a SECOND, and the label
is chosen by a 70 px threshold — at 1 s ≥ 70 px the minor step coincided with
the label, and not a single tick mark was drawn.

Checked by the PIXELS of the ruler strip: how many vertical
strokes it has. The harness would not show this — it does not draw.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def tick_columns(png, y, x0, x1):
    """How many “light” columns there are on row y in the range [x0, x1).

    The ruler’s tick marks are vertical 1 px strokes on a dark background, so it is
    enough to count the local brightness spikes.
    """
    out = subprocess.run(
        ["convert", png, "-crop", f"{int(x1 - x0)}x1+{int(x0)}+{int(y)}",
         "-depth", "8", "txt:-"],
        capture_output=True, text=True, check=True,
    ).stdout
    vals = []
    for line in out.splitlines()[1:]:
        if "srgb(" not in line and "gray(" not in line:
            continue
        body = line.split("(", 1)[1].split(")", 1)[0]
        nums = [int(v) for v in body.split(",")[:3] if v.strip().isdigit()]
        vals.append(sum(nums) / max(len(nums), 1))
    runs, prev = 0, 0.0
    for v in vals:
        if v > prev + 12:
            runs += 1
        prev = v
    return runs


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "ruler.otio"), [(0.0, 6.0, "synth:Ramp")]
    )

    with Live("ruler", out, project=proj) as app:
        shot = app.shot("open")
        g = app.geom()
        tl = g["timeline"]
        # the geometry dump gives coordinates RELATIVE TO THE WINDOW, and the snapshot is
        # screen-based: forgetting the window origin means measuring in the wrong place (the R.38 pitfall)
        win = geometry()
        x0, x1 = win["x"] + tl["left"] + 20, win["x"] + tl["left"] + 320
        # TWO rows of pixels, and that is the whole point of the measurement:
        #   y_minor — the very top of the strip of ticks (STRIPES_H = 11), there reach
        #     BOTH the minor and the major ticks, and the caption is not there yet
        #     (its top is at 12);
        #   y_major — below the captions, only the major ticks remain there.
        # The absolute number of strokes proves nothing: at the first attempt the
        # measure stood at 20 px, caught the antialiasing of the DIGITS, and was green
        # on the old ladder too.
        y_minor = win["y"] + tl["top"] + 11
        y_major = win["y"] + tl["top"] + 30
        n_minor = tick_columns(shot, y_minor, x0, x1)
        n_major = tick_columns(shot, y_major, x0, x1)
        app.expect(
            n_minor >= 3 * n_major + 5,
            f"minor tick marks must be MANY TIMES more than labels: "
            f"{n_minor} on the tick strip versus {n_major} below the labels "
            f"(at 100 px/s a step of 0.1 s — that is roughly 30 versus 3)",
        )
        app.shot("ruler-ticks")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

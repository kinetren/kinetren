#!/usr/bin/env python3
"""Live scenario t2976 (mosh-sharp, kkt-26.11.18b U1, U7): a person sharpens a 144-line mosh.

Two clips back to back — shot A (colour bars, still) and shot B (a moving test pattern) — in a
1280×720 project. The person double-clicks B, picks "Datamosh → Cut mosh" and sets the encoded height
to 144: a new Datamosh is sharp (U7), so the break (the bars dragged by the pattern's motion) is carried
at the preview's size, and the file names the row. Unticking "Sharp" gives the reference: the same
break, the codec's 144-line picture enlarged — less fine detail, and the file leaves the row out.

Our own fixtures (ffmpeg lavfi), never the owner's projects (R.67).
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from datamosh import moshes, moving_media  # noqa: E402
from idle import NOT_ON_HOVER, Meter, sweep, wait_quiet, wakes, woke  # noqa: E402
from kt_live import Live, pattern_media, poll_step, project_with_clips, require_tools, screen_image  # noqa: E402

# the rest measure reads the program's own threads (as `idle`)
OWN_PROCESS = True
SEC = 90000


def detail(app, half=(160, 80)):
    """The mean horizontal luma step in a box of the preview around its centre: fine detail."""
    img = screen_image().convert("L")
    cx, cy = app.preview_center()
    box = img.crop((cx - half[0], cy - half[1], cx + half[0], cy + half[1]))
    px = list(box.getdata())
    w = box.width
    steps = [abs(px[i] - px[i - 1]) for i in range(len(px)) if i % w]
    return sum(steps) / max(len(steps), 1)


def open_fx(app, y):
    app.double_click(app.x_at(int(3.0 * SEC)), y)
    return app.has_probe("fxreg.add")


def main(out):
    require_tools()
    # an edit's autosave thirty seconds later must not land in the rest measure (found-defects 765)
    os.environ.setdefault("KT_AUTOSAVE_SECS", "600")
    a = pattern_media(out, "msharp-a", 3, size="1280x720")
    b = moving_media(out, "msharp-b", 6, size="1280x720")
    proj = os.path.join(out, "datamosh_sharp.otio")
    project_with_clips(proj, [(0, 2, a), (2, 5, b)], width=1280, height=720)

    with Live("datamosh_sharp", out, project=proj, own_process=True) as app:
        y = app.lane_y("video", 0, frac=0.5)
        app.step("timeline", "two clips back to back: bars, then a moving pattern")
        app.click(app.x_at(int(2.5 * SEC)), app.ruler_y())
        center = app.preview_center()
        plain = app.pixel_settled(center)

        app.expect(open_fx(app, y), "the FX window opened")
        app.step("fxwin", "double click on the second clip: its FX window")
        if not app.click_probe("fxreg.add", "the picker button"):
            return app.done()
        for ch in "datamosh":
            app.key(ch)
        cell = None
        for _ in range(40):
            if app.has_probe("fxreg.var.datamosh.0", tries=1):
                cell = True
                break
            poll_step(0.25)
        app.expect(cell is not None, "the gallery shows the Cut mosh preset")
        if cell is None or not app.click_probe("fxreg.var.datamosh.0", "the Cut mosh cell"):
            return app.done()
        app.step("picked", "Cut mosh added: the codec's rows, the encoded height, the Sharp tick")
        app.expect(app.has_probe("mosh.sharp"), "the panel shows the Sharp tick")
        if not app.click_probe("mosh.lines", "the encoded height's list"):
            return app.done()
        if not app.click_probe("mosh.lines.0", "the 144 item"):
            return app.done()
        app.step("lines144", "the encoded height set to 144; a new Datamosh is sharp (kkt-26.11.18b U7)")
        app.key("Escape")
        app.click(app.x_at(int(2.5 * SEC)), app.ruler_y())
        # the history catches up first (Loading — catching up): the judged picture is the moment's own
        app.wait_gone("preview.catching_up", timeout=120.0)
        moshed = app.pixel_settled(center, differs_from=plain, tries=60)
        app.wait_still(timeout=60.0)
        sharp_shot = app.step("sharp", "the 144-line break carried at the preview's size: the pattern's detail")
        d_sharp = detail(app)
        app.expect(moshed is not None and moshed != plain, f"the frame after the cut is moshed ({plain} → {moshed})")
        app.saved_project()
        slots = moshes(app.project_text())
        app.expect(len(slots) == 1 and slots[0].get("sharp") is True, f"a new Datamosh is sharp in the file ({json.dumps(slots)[:200]})")

        # the reference: the same break, soft — the tick taken off explicitly
        app.expect(open_fx(app, y), "the FX window opened again")
        if not app.has_probe("mosh.sharp"):
            # the window opens on its last slot; the datamosh panel is the selected slot's
            app.expect(False, "the reopened window shows the datamosh panel")
            return app.done()
        if not app.click_probe("mosh.sharp", "the Sharp tick"):
            return app.done()
        app.step("unticked", "Sharp unticked")
        app.key("Escape")
        app.click(app.x_at(int(2.5 * SEC)), app.ruler_y())
        app.wait_gone("preview.catching_up", timeout=120.0)
        app.pixel_settled(center, tries=60)
        app.wait_still(timeout=60.0)
        app.step("soft", "the same break, soft: the codec's 144-line picture enlarged")
        d_soft = detail(app)
        diff = app.screen_diff(sharp_shot)
        app.expect(diff > 0.005, f"the soft frame differs from the sharp one (the screen changed by {diff:.1%})")
        app.expect(d_sharp > 1.15 * d_soft, f"the sharp frame has more fine detail ({d_sharp:.2f} against {d_soft:.2f})")
        app.saved_project()
        slots = moshes(app.project_text())
        app.expect(len(slots) == 1 and not slots[0].get("sharp"), f"the soft slot leaves the row out ({json.dumps(slots)[:200]})")

        # the program at rest costs nothing (owner, 2026-11-07): the sharp mosh's window open, a pause,
        # then the pointer over the window
        meter = Meter(app)
        app.expect(open_fx(app, y), "the FX window opened for the rest measure")
        if not app.click_probe("mosh.sharp", "the Sharp tick, back on"):
            return app.done()
        # the Ctrl+S above queued the project's poster, a Background render that walks the mosh's
        # history and shows no counter until it ends (found-defects 767): rest begins after it
        app.expect(app.until(lambda: app.geom().get("posters_busy") is False, timeout=300.0), "the poster of the save is done")
        # the save's status line goes out six seconds after it (a frame for a visible change, not
        # rest work): rest begins when no short clock is left (found-defects 768)
        app.expect(app.until(lambda: not 0 <= app.geom().get("rest_clock_ms", -1) < 60_000, timeout=60.0), "no status or guard clock is left")
        wait_quiet(meter, "sharp, its window open")
        w0 = wakes(app)
        work, cpu = meter.over(5.0)
        print(f"   sharp window, pause: {work}, {cpu:.0f} % of a core, threads {meter.threads}")
        app.expect(not work, f"an open sharp Datamosh window at rest runs nothing, ran {work}; frames with no input were asked by: {woke(app, w0)}")
        r = app._probe_rect("win.fx")
        if r is not None:
            from kt_live import geometry

            win = geometry()
            x0, y0 = win["x"] + int(r["x"]) + 40, win["y"] + int(r["y"]) + 60
            # the pointer enters the window first (a focus change may poke the media watch: not a hover)
            meter.over(1.0, sweep(app, (x0, y0), (x0 + 160, y0 + 100)))
            wait_quiet(meter, "the pointer in the sharp window")
            work, cpu = meter.over(3.0, sweep(app, (x0, y0), (x0 + 160, y0 + 100)))
            print(f"   over the sharp window: {work}, {cpu:.0f} % of a core, threads {meter.threads}")
            bad = {k: v for k, v in work.items() if k in NOT_ON_HOVER}
            app.expect(not bad, f"over the sharp window: a static hover caused {bad}")
            app.expect(not meter.engine_busy(), f"over the sharp window: the engine worked {meter.engine_busy()} % of a core")
        app.step("rest", "the window open, the pointer over it: the program at rest")
        app.key("Escape")

        app.saved_project()
        slots = moshes(app.project_text())
        app.expect(len(slots) == 1, f"one datamosh slot in the file ({len(slots)})")
        if slots:
            s = slots[0]
            app.expect(s.get("sharp") in (True, 1, 1.0), f"the slot carries sharp ({json.dumps(s)[:200]})")
        return app.done()


if __name__ == "__main__":
    out_dir = sys.argv[1] if len(sys.argv) > 1 else os.path.join("target", "live-out")
    os.makedirs(out_dir, exist_ok=True)
    sys.exit(main(out_dir))

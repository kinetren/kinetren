#!/usr/bin/env python3
"""Live scenario diagnostics-three-graphs: Performance lives in Diagnostics, with three graphs.

The owner (kkt-26.11.01 item 14, line 24 of the verbatim file): Performance must move into the
Diagnostics window, so that there are three graphs there. The real program on our fixture: the left column holds only the bin; the gauge
button opens Diagnostics with its "Performance" section and the three graphs; playback runs them;
the corner drag makes them taller; Esc closes the window, View → Diagnostics… opens it again;
View → "Economy UI" says what it does. Every verdict reads the program's own probes.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

GRAPHS = ("diag.graph.quality", "diag.graph.decode", "diag.graph.churn")


def main(out):
    require_tools()
    proj = project_with_clips(os.path.join(out, "diagnostics.otio"), [(0.0, 8.0, "synth:Ramp")])
    with Live("diagnostics", out, project=proj) as app:
        # the owner's working width: at 1440 the bottom row cuts the gauge button off at the right
        # edge (the toolbar-width follow-up the lessons scenario names too)
        app.resize(1900, 1000)
        win = geometry()
        g = app.geom()
        bin_ = g["bin"]
        # --- 1. the left column is the bin's: its frame reaches the column's bottom, which is the
        # timeline's top (the column stands above the timeline) -------------------------------
        gap = g["timeline"]["top"] - bin_["bottom"]
        app.expect(
            0 <= gap < 40,
            f"the bin reaches the bottom of the column (bin bottom {bin_['bottom']:.0f}, "
            f"timeline top {g['timeline']['top']:.0f})",
        )
        app.step("column", "the left column: only the media bin, down to the bottom")

        # --- 2. the gauge opens Diagnostics with three graphs ---------------------------------
        app.click_probe("tb.diag", "the gauge button of the toolbar")
        app.expect(app.has_probe("win.diag"), "the gauge opened the Diagnostics window")
        app.expect(
            app.has_probe("diag.graph.quality")
            and app.has_probe("diag.graph.decode")
            and app.has_probe("diag.graph.churn"),
            "the three graphs are drawn: Quality, FFmpeg activity, Cache churn",
        )
        rects = [app._probe_rect(g) for g in GRAPHS]
        w = app._probe_rect("win.diag")
        app.expect(
            all(r["y"] >= w["y"] and r["y"] + r["h"] <= w["y"] + w["h"] for r in rects if r),
            "the three graphs are inside the window",
        )
        app.expect(
            rects[0]["y"] < rects[1]["y"] < rects[2]["y"], "the graphs stand one under another"
        )
        app.step("diagnostics-open", "Diagnostics: Performance rows, occupancy, three graphs, the log")

        # --- 2b. the memory numbers say what they count (frames-cache-growth, found-defects 397):
        # the status bar's frames against their bound and the program's memory, each with its hint;
        # Diagnostics names the whole process under the caches' line ----------------------------
        app.expect(app.has_probe("diag.memory"), "Diagnostics names the program's memory, the caches and the rest")
        app.expect(app.has_probe("status.frames") and app.has_probe("status.memory"),
                   "the status bar shows the frames against their bound and the program's memory")
        before = app.geom().get("status", "")
        app.hover_probe("status.frames", "the status bar's frames")
        app.until(lambda: app.geom().get("status", "") != before)
        said = app.geom().get("status", "")
        app.expect(bool(said) and said != before, f"hovering the frames says what they count: {said!r}")
        app.step("memory-numbers", "the status bar: frames held/bound and the program's memory; "
                 "Diagnostics: the caches' line and the program's memory with the rest named")

        # --- 3. playback runs the graphs --------------------------------------------------------
        app.key("space")
        # three seconds of playback: the sampler pushes six samples into every graph
        app.wait_playhead(lambda t: t > 3 * 90000, "three seconds played", timeout=15.0)
        app.step("playing", "while playing the three graphs draw their lines")
        app.key("space")

        # --- 4. the corner drag makes the graphs taller -----------------------------------------
        h0 = [r["h"] for r in rects]
        w = app._probe_rect("win.diag")
        cx = win["x"] + int(w["x"] + w["w"]) - 3
        cy = win["y"] + int(w["y"] + w["h"]) - 3
        app.drag(cx, cy, cx + 160, cy + 140)
        app.settle()
        w2 = app._probe_rect("win.diag")
        app.expect(w2["h"] > w["h"] + 60, f"the corner drag enlarged the window: {w['h']:.0} → {w2['h']:.0}")
        h1 = [app._probe_rect(g)["h"] for g in GRAPHS]
        app.expect(all(b > a + 15 for a, b in zip(h0, h1)), f"each graph grew: {h0} → {h1}")
        app.expect(app.has_probe("diag.log"), "the problem log is drawn")
        log = app._probe_rect("diag.log")
        app.expect(
            log is not None and log["y"] + log["h"] <= w2["y"] + w2["h"] + 1,
            "the problem log stays inside the enlarged window",
        )
        app.step("enlarged", "the window dragged larger: the three graphs stretched, the log stays at the bottom")

        # --- 5. Esc closes it, View → Diagnostics… opens it again ------------------------------
        app.key("Escape")
        app.expect(app.wait_gone("win.diag"), "Esc closed Diagnostics")
        # Economy UI is a program preference: File › Preferences… (welcome-window-and-settings)
        app.click_probe("menu.file", "the File menu")
        app.click_probe("file.preferences", "File → Preferences…")
        before = app.geom().get("status", "")
        app.hover_probe("prefs.economy", "Preferences → Economy UI")
        app.until(lambda: app.geom().get("status", "") != before)
        said = app.geom().get("status", "")
        app.expect(bool(said) and said != before, f"hovering Economy UI says what it does: {said!r}")
        app.step("economy-hint", "Preferences → Economy UI: the status line says what the switch does")
        app.key("Escape")
        app.expect(app.wait_gone("prefs.window"), "Esc closed Preferences")
        app.click_probe("menu.view", "the View menu")
        app.click_probe("view.diagnostics", "View → Diagnostics…")
        app.expect(app.has_probe("win.diag"), "View → Diagnostics… opened the window again")
        app.step("reopened", "View → Diagnostics… opens the same window, at the size it was given")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "/tmp/kt-live-diagnostics"))

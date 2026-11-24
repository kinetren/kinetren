#!/usr/bin/env python3
"""Live scenario fx-graph-dag v2d (kkt-26.11.18b U7, "like in Blender"; his A5, A6): branches and
merges made by the mouse in the real program. The scene is OURS (R.67): one video event with Pixelate.

PATH (ui-reach spec, "The node editor makes branches and merges"):
1. double click the event → its node editor: In → Geometry → Pixelate → Out;
2. drag a wire from Geometry's output to the empty space under the graph → the picker opens there →
   Vignette → a second row under the trunk, dim: it reaches no Out yet;
3. drag from Geometry's output again → Merge, the picker's first row → Merge before Out, Pixelate its
   Background, Geometry its Foreground;
4. drag from Vignette's output onto Merge's Matte socket → the preview shows the clean picture where
   the vignette is bright and Pixelate at its dark edges;
5. click Vignette, pick Slowed+Reverb → it stands in the arm marked "Reads the source again", and the
   event's coverage line has a second row over it;
6. drag the Geometry node onto Slowed in the arm → the status line refuses, nothing changes;
7. Ctrl+Shift+click Vignette → the badge "Viewer: Vignette" → Esc;
8. Ctrl+Z four times → the line it was.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, require_tools  # noqa: E402

SEC = 90000


def scene(dest):
    project = {
        "format_version": 1,
        "settings": {"fps": 25, "width": 640, "height": 360, "sample_rate": 48000},
        "media": [{"id": 1, "path": "synth:ramp"}],
        "clips": [{
            "id": 1, "link_group": None, "kind": "video", "lane": 0, "start": 0, "duration": 6 * SEC,
            "source_in": 0, "media": 1, "gain_db": 0.0, "fx": [{"kind": "pixelate"}],
        }],
        "tracks": [{"id": 1, "kind": "video"}, {"id": 2, "kind": "audio"}],
    }
    with open(dest, "w") as f:
        json.dump({"kinetren": 1, "project": project}, f, indent=2)
    return dest


def field(app, name):
    return next((f for f in app.geom().get("fields", []) if f["param"] == name), None)


def chain(text):
    """The event's slots in the file: their kinds, and the inputs that are not the line."""
    found = []

    def walk(node):
        if isinstance(node, dict):
            if isinstance(node.get("fx"), list) and node.get("kind") == "video" and "media" in node:
                found.append(node["fx"])
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)

    walk(json.loads(text))
    slots = found[0] if found else []
    return [(s.get("kind"), s.get("in")) for s in slots]


def kinds(app):
    return [k for k, _ in chain(app.saved_text())]


def center(app, name):
    f, win = field(app, name), geometry()
    if not f:
        app.expect(False, f"the program published no {name}")
        return None
    return win["x"] + int(f["x"] + f["w"] / 2), win["y"] + int(f["y"] + f["h"] / 2)


def wire(app, slot, to):
    """A wire out of a node's output to a place on the screen."""
    app.hover_probe(f"fx.out.{slot}", f"the output of node {slot}")
    drag(app, center(app, f"fx.out.{slot}"), to)


def drag(app, a, b):
    if a and b:
        app.drag(a[0], a[1], b[0], b[1])


def under(app):
    g, win = field(app, "fx.graph"), geometry()
    return (win["x"] + int(g["x"] + g["w"] / 2), win["y"] + int(g["y"] + g["h"] + 14)) if g else None


def pick(app, text, probe):
    app.expect(app.until(lambda: field(app, "fxreg.popup") is not None), "the picker opened where the wire was dropped")
    app.type_text(text)
    app.until(lambda: field(app, probe) is not None)
    app.click_probe(probe, f"the picker's row {probe}")
    app.wait_gone("fxreg.popup")


def main(out):
    require_tools()
    proj = scene(os.path.join(out, "node_graph_branch.kt"))
    # the strip (Geometry is its slot 0): Pixelate 1
    with Live("node_graph_branch", out, project=proj) as app:
        app.double_click(app.x_at(3 * SEC), app.lane_y("video", 0, frac=0.5))
        app.expect(app.until(lambda: field(app, "fx.slot.1") is not None), "the event's node editor")
        app.step("line", "double click on the event → In, Geometry, Pixelate, Out")

        # the first branch, its picker in view
        wire(app, 0, under(app))
        app.expect(app.until(lambda: field(app, "fxreg.popup") is not None), "a wire let go on empty space opened the picker")
        app.step("picker", "a wire from Geometry let go under the graph → the picker there, Merge its first row")
        pick(app, "vignette", "fxreg.tpl.vignette")
        app.expect(app.until(lambda: kinds(app) == ["vignette", "pixelate"]), "Vignette is the arm's effect")
        v, p = field(app, "fx.node.1"), field(app, "fx.node.2")
        app.expect(v and p and v["y"] > p["y"] + p["h"] - 1, f"the arm is a row under the trunk: {v} / {p}")
        app.step("branch", "Vignette → a second row under the trunk, dim until it reaches Out")

        # an unused branch dropped on Out becomes the result, one step back
        app.hover_probe("fx.dst.0", "the lane's Out")
        wire(app, 1, center(app, "fx.dst.0"))
        app.expect(app.until(lambda: kinds(app) == ["pixelate", "vignette"]), "the branch dropped on Out is the result")
        app.step("to-out", "a wire from Vignette onto Out → Vignette is the result, Pixelate the unused branch")
        app.key("ctrl+z")
        app.expect(app.until(lambda: kinds(app) == ["vignette", "pixelate"]), "one undo gave the trunk back")
        # a wire from In would skip the leading Geometry: refused
        app.hover_probe("fx.src.0", "the Source's output")
        drag(app, center(app, "fx.src.0"), center(app, "fx.in.2"))
        app.expect(kinds(app) == ["vignette", "pixelate"], "a wire past the leading Geometry changed nothing")

        wire(app, 0, under(app))
        app.expect(app.until(lambda: field(app, "fxreg.popup") is not None), "the picker opened where the wire was dropped")
        app.type_text("merge")
        app.click_probe("fxreg.merge", "Merge, the first row")
        app.wait_gone("fxreg.popup")
        app.expect(app.until(lambda: kinds(app) == ["vignette", "pixelate", "merge"]), "Merge stands before Out")
        app.step("merge", "a wire from Geometry → Merge: Pixelate in Background, Geometry in Foreground")

        # its mode, in the form below (Merge is selected)
        app.click_probe("fx.mode", "Merge's mode")
        app.click_probe("fx.mode.screen", "Screen")
        app.expect(app.until(lambda: '"screen"' in app.saved_text()), "the mode is in the file")
        app.key("ctrl+z")
        app.hover_probe("fx.wire.3", "the wire into Merge's Background")
        app.hover_probe("fx.wire.3.fg", "the wire into Merge's Foreground")
        app.hover_probe("fx.wire.out.0", "the wire into Out")

        before = app.shot("before-matte")
        wire(app, 1, center(app, "fx.matte.3"))
        app.expect(app.until(lambda: (chain(app.saved_text())[2][1] or {}).get("matte") is not None), "the matte wire is in the file")
        app.expect(app.screen_diff(before) > 0.005, "the preview shows the matte")
        app.hover_probe("fx.wire.3.matte", "the wire into Merge's Matte")
        app.step("matte", "a wire from Vignette onto Merge's Matte → the clean picture in its bright middle, Pixelate at the edges")

        app.click_probe("fx.slot.1", "the Vignette node")
        app.click_probe("fxreg.add", "the picker's button")
        pick(app, "slowed", "fxreg.tpl.slowed_reverb")
        app.expect(app.until(lambda: "slowed_reverb" in kinds(app)), "Slowed+Reverb landed on the arm")
        slot = kinds(app).index("slowed_reverb") + 1
        app.hover_probe(f"fx.read.{slot}", "the mark of a second read")
        app.expect(app.until(lambda: field(app, "clip.1.read.0") is not None), "the event's coverage line has the read's row")
        app.hover_probe("clip.1.read.0", "the read's row over the event's coverage line")
        app.step("second-read", "Slowed+Reverb on the arm → marked as a second read; a second row over the event's coverage line")

        was = chain(app.saved_text())
        slot = [k for k, _ in was].index("slowed_reverb") + 1
        drag(app, center(app, "fx.slot.0"), center(app, f"fx.slot.{slot}"))
        app.settle()
        app.step("refused", "the Geometry node dragged into the arm → the status line refuses: it stays before the first branch")
        app.expect(chain(app.saved_text()) == was, "the refused drag changed nothing")

        at = center(app, "fx.slot.1")
        if at:
            app.click(at[0], at[1], mods=("ctrl", "shift"))
        app.expect(app.until(lambda: field(app, "preview.viewer") is not None), "the Viewer on a node of the arm")
        app.step("viewer", "Ctrl+Shift+click on Vignette → the arm's own picture, the badge “Viewer: Vignette”")
        app.key("Escape")
        app.expect(app.until(lambda: field(app, "preview.viewer") is None), "Esc gave the graph's picture back")

        for _ in range(4):
            app.key("ctrl+z")
        app.expect(app.until(lambda: chain(app.saved_text()) == [("pixelate", None)]), "four undos gave the line back")
        app.step("undone", "Ctrl+Z four times → the line the event opened with")
        for _ in range(4):
            app.key("ctrl+y")
        app.expect(app.until(lambda: "slowed_reverb" in kinds(app)), "four redos gave the graph again")
        app.click_probe("fx.done", "Done")
        app.click_probe("tb.diag", "the gauge button of the toolbar")
        app.expect(app.has_probe("diag.reads"), "the performance section names the read")
        app.hover_probe("diag.reads", "the reads' line")
        app.step("performance", "Diagnostics → “Reads ×1: #1 Slowed+Reverb”")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

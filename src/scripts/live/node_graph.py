#!/usr/bin/env python3
"""Live scenario fx-graph v1b (kkt-26.11.18b U3, U7, U8): the effects chain as a node editor.

The owner asked for a compact graph in place of the chain strip, one style in every window, fitting
without scrolling, with two or three sliders in a node and a big device kept small. The scene is
OURS (R.67): a video event and its linked audio under a morpher layer carrying Blur and VHS Tape on
the picture and a Gain on the sound, and a second event carrying Camera ISP.

PATH (ui-reach spec, "Every effect chain is edited in one node editor that fits its window"):
1. double click the morpher block → its window: the node editor, Blur and VHS Tape on the picture
   lane, Gain and the tape's sound on the sound lane, the tape's two halves in one column;
2. drag Blur onto VHS Tape → Blur stands after the tape (one lane, one undo step);
3. double click the audio event → its effects window: its nodes, then the layer's box and the track's
   box with its key → double click the track's box → the track's window with the key box under its
   compressor → double click the layer's box → the layer's window;
4. double click the Camera ISP event → its effects window: the ISP node with its main sliders, the
   whole ISP panel below.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, require_tools  # noqa: E402

SEC = 90000


def scene(dest):
    clip = lambda i, kind, start, fx: {  # noqa: E731
        "id": i, "link_group": 1 if i < 3 else None, "kind": kind, "lane": 0, "start": start,
        "duration": 6 * SEC, "source_in": 0, "media": 1, "gain_db": 0.0, "fx": fx,
    }
    project = {
        "format_version": 1,
        "settings": {"fps": 25, "width": 640, "height": 360, "sample_rate": 48000},
        "media": [{"id": 1, "path": "synth:ramp"}],
        "clips": [
            clip(1, "video", 0, []),
            clip(2, "audio", 0, []),
            clip(3, "video", 8 * SEC, [{"kind": "camera_isp"}]),
        ],
        # the audio track ducks under the music track: its compressor is keyed by track 4
        "tracks": [
            {"id": 1, "kind": "video"},
            {"id": 2, "kind": "audio", "fx": [{"kind": "compressor", "key": 4}]},
            {"id": 3, "kind": "morph"},
            {"id": 4, "kind": "audio"},
        ],
        "morphers": [{
            "id": 1, "lane": 0, "start": 0, "duration": 6 * SEC,
            "targets": {"on": "clips", "ids": [1, 2]},
            "patch": {
                "video_fx": [{"kind": "blur", "radius": 4}, {"kind": "vhs_tape"}],
                "audio_fx": [{"kind": "gain"}],
            },
        }],
        "overlap_layers": True,
    }
    with open(dest, "w") as f:
        json.dump({"kinetren": 1, "project": project}, f, indent=2)
    return dest


def field(app, name):
    return next((f for f in app.geom().get("fields", []) if f["param"] == name), None)


def patch(text):
    doc = json.loads(text)
    found = []

    def walk(node):
        if isinstance(node, dict):
            if isinstance(node.get("patch"), dict):
                found.append(node["patch"])
            for v in node.values():
                walk(v)
        elif isinstance(node, list):
            for v in node:
                walk(v)

    walk(doc)
    return found[0] if found else {}


def kinds(chain):
    return [s.get("kind") for s in chain or []]


def main(out):
    require_tools()
    proj = scene(os.path.join(out, "node_graph.kt"))

    with Live("node_graph", out, project=proj) as app:
        app.double_click(app.x_at(3 * SEC), app.lane_y("morph", 0, frac=0.5))
        app.expect(app.until(lambda: field(app, "fx.slot.3") is not None), "the morpher's node editor shows four nodes")
        app.step("morpher", "double click on the morpher block → the node editor: picture lane over sound lane, the tape's halves in one column")
        tape, snd = field(app, "fx.node.1"), field(app, "fx.node.3")
        app.expect(tape is not None and snd is not None, "the tape's two nodes are drawn")
        if tape and snd:
            app.expect(abs(tape["x"] - snd["x"]) < 1.5 and snd["y"] > tape["y"], f"the halves stand in one column: {tape} / {snd}")
        graph = field(app, "fx.graph")
        app.expect(graph is not None, "the graph publishes its place")

        blur, vhs = field(app, "fx.slot.0"), field(app, "fx.slot.1")
        if blur and vhs:
            win = geometry()
            app.drag(win["x"] + int(blur["x"] + blur["w"] / 2), win["y"] + int(blur["y"] + blur["h"] / 2),
                     win["x"] + int(vhs["x"] + vhs["w"] / 2), win["y"] + int(vhs["y"] + vhs["h"] / 2))
        app.expect(app.until(lambda: kinds(patch(app.saved_text()).get("video_fx")) == ["vhs_tape", "blur"]),
                   "Blur dragged onto VHS Tape stands after it")
        app.step("reordered", "Blur dragged onto VHS Tape → it stands after the tape; the sound lane is untouched")
        p = patch(app.saved_text())
        app.expect(kinds(p.get("audio_fx")) == ["gain", "vhs_tape_sound"], f"the sound lane kept its order: {kinds(p.get('audio_fx'))}")
        app.click_probe("fx.graph.fit", "Frame all")
        app.step("framed", "Frame all → the whole graph in view again")
        app.key("Escape")

        # what applies after the event: the layer's box and the track's box with its key
        # above the volume rail (a double click on it resets the level)
        app.double_click(app.x_at(3 * SEC), app.lane_y("audio", 0, frac=0.15))
        app.expect(app.until(lambda: field(app, "fx.ctx.1") is not None), "the audio event's graph shows the layer and the track as boxes")
        app.step("context", "double click on the audio event → after its own nodes: the layer's box and the track's box with its key")
        app.double_click_probe("fx.ctx.1", "the track's box")
        app.expect(app.until(lambda: field(app, "fx.key.0") is not None), "the track's window draws the key under its compressor")
        app.hover_probe("fx.key.0", "the key box")
        app.step("track-window", "double click on the track's box → the track's window: the key box wired into the compressor")
        app.double_click_probe("fx.ctx.0", "the layer's box")
        app.expect(app.until(lambda: field(app, "fx.head") is not None), "the layer's box opened the morpher's window")
        app.step("layer-window", "double click on the layer's box → the morpher's own window")
        for _ in range(3):
            app.key("Escape")

        app.double_click(app.x_at(11 * SEC), app.lane_y("video", 0, frac=0.5))
        app.expect(app.until(lambda: field(app, "fx.graph") is not None and field(app, "fx.node.1") is not None),
                   "the ISP event's node editor")
        app.step("isp", "double click on the Camera ISP event → its node: the look's main sliders; the whole panel below")
        node = field(app, "fx.node.1")
        app.hover_probe("fx.graph", "the graph's canvas")
        app.click_probe("fx.node.1", "the ISP node")
        app.step("isp-selected", "a click on the ISP node selects it; its whole panel stays below")
        if node:
            app.expect(node["h"] <= 22 + 3 * 22 + 4 + 1, f"the ISP node stays small: {node['h']} px")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

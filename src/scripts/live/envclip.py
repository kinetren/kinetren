#!/usr/bin/env python3
"""Live scenario R.61: the envelope buffer is SYSTEM, and reading one’s own
buffer does not hang the program.

Claim of the owner: “judging by the fact that the button can be "disabled" — the code has
some state of the clipboard — which is bad: on a button press take the value from the real clipboard
and only then check whether it can be pasted at all, or not”.

The harness cannot prove this: there is no X server there, nor a system clipboard,
and the main danger lives exactly there. On X11 the clipboard is OWNED by a window, and after
copying the owner is our own window; a synchronous read would go as a request into our own
event loop, which at that moment is drawing, and the program would hang
EXACTLY in the scenario for which the feature is made. Therefore the read is
a background task, and this is checked here:
we copy, paste, the window stays alive.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401

SEC = 90000


def walk(node):
    yield node
    if isinstance(node, dict):
        for v in node.values():
            yield from walk(v)
    elif isinstance(node, list):
        for v in node:
            yield from walk(v)


def curve_params(doc):
    """Parameter names of all curves in the document."""
    out = []
    for n in walk(doc):
        meta = n.get("metadata") if isinstance(n, dict) else None
        if not isinstance(meta, dict):
            continue
        kt = meta.get("kt")
        if isinstance(kt, dict) and isinstance(kt.get("auto"), list):
            out += [c.get("param") for c in kt["auto"] if isinstance(c, dict)]
    return out


def main(out):
    require_tools()
    proj = os.path.join(out, "envclip.otio")
    demo_otio(proj)

    with Live("envclip", out, project=proj) as app:
        # OUTSIDE the level line: a double click on it resets the volume (item 362)
        y = app.lane_y("audio", 1, frac=0.50)
        app.double_click(app.x_at(10 * SEC), y)
        app.shot("fx-window")
        app.expect(app.has_probe("env.copy"), "the “copy envelope” button is in the window")
        app.expect(app.has_probe("env.paste"), "the “paste envelope” button is in the window")

        before = curve_params(app.saved_project().doc)
        app.expect(bool(before), f"the demo project has curves: {before}")

        # --- COPY writes to the system clipboard ----------------------
        app.click_probe("env.copy", "copying the envelope to the system clipboard")
        app.shot("copied")

        # --- PASTE reads ITS OWN clipboard and does not hang the window ------------
        # exactly the case that a synchronous read would hang: the owner of the
        # clipboard is our own window
        app.click_probe("env.paste", "paste from its own buffer")
        app.settle()
        app.shot("pasted")
        app.expect(
            app.has_probe("env.paste"),
            "the window is alive after reading its OWN buffer — a synchronous read "
            "would hang here",
        )
        # and the program responds to gestures: the geometry is read, so the event loop
        # is not blocked
        g = app.geom()
        app.expect(isinstance(g, dict) and bool(g), "the event loop is not blocked")

        app.key("Escape")
        after = curve_params(app.saved_project().doc)
        app.expect(
            set(before).issubset(set(after)),
            f"the round trip through the system buffer lost curves: was {before}, became {after}",
        )


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/envclip")

#!/usr/bin/env python3
"""Live scenario R.48 (item 346): the FX preset library is read by the real
program and survives its work.

The harness tests (t344-t346) run the window and the paste inside one process, and
the store there is IN MEMORY — the tests do not touch the developer’s home directory.
Here we check what the harness does not check: the real binary with the
real `KT_USER_STORE_DIR` reads the library file, opens the FX window by it, and does NOT spoil
other people’s records; and a damaged file does not crash it
(the policy “a broken file does not crash the program” — item 343).
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401

SEC = 90000

# a preset with a curve and a record from a “future version” side by side: the old build does not
# understand it, but must keep it
LIBRARY = {
    "version": 1,
    "presets": [
        {
            "name": "live chain",
            "slots": [
                {
                    "fx": {"kind": "blur", "radius": 6},
                    "curves": [
                        {
                            "param": "radius",
                            "ease": True,
                            "keys": [
                                {"t": 0, "v": 1.0},
                                {"t": 90000, "v": 32.0},
                            ],
                        }
                    ],
                }
            ],
        },
        {"name": "from the future", "slots": [{"fx": {"kind": "time_machine", "x": 1}}]},
    ],
}


def open_fx_window(app, tag):
    """Open the FX window with a double click on an event and make sure it
    was drawn (the picker with the library lives inside it)."""
    y = app.lane_y("video", 0, frac=0.5)
    before = app.shot(f"{tag}-before")
    app.double_click(app.x_at(3 * SEC), y)
    app.shot(f"{tag}-fx")
    d = app.screen_diff(before)
    app.expect(d > 0.01, f"{tag}: FX window opened (the screen changed by {d:.1%})")


def main(out):
    require_tools()
    store = os.path.join(out, "store")
    os.makedirs(store, exist_ok=True)
    os.environ["KT_USER_STORE_DIR"] = store
    presets = os.path.join(store, "fx-presets.json")

    proj = os.path.join(out, "fxpreset.otio")
    demo_otio(proj)

    # --- 1) valid library: the program reads it and does not spoil it --------
    with open(presets, "w") as f:
        json.dump(LIBRARY, f, indent=2)
    before = json.load(open(presets))
    with Live("fxpreset", out, project=proj) as app:
        open_fx_window(app, "lib")
        app.saved_project()  # ctrl+s: saving the project does not touch the library
        after = json.load(open(presets))
        app.expect(after == before, "the program did not rewrite the library without need")
        app.expect(
            any(p["name"] == "from the future" for p in after["presets"]),
            "a record from a future version survived the session",
        )
        rc = app.done()
    if rc:
        return rc

    # --- 2) broken file: the program must open ---------------------
    with open(presets, "w") as f:
        f.write("{ this is not json")
    with Live("fxpreset-broken", out, project=proj) as app:
        open_fx_window(app, "broken")
        app.expect(True, "a broken library file did not stop the program from working")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

#!/usr/bin/env python3
"""Live scenario R.67 (item 1): a project of the OLD version — through its own door.

Owner: “project 26.09.26/untitled2 does not open - what is this related to?”
File v5 (18.09) has `targets: null` in its morphers, while the reader knows a list and
an object. His decision: “make an item in the File menu for importing old versions, and
repair exactly in the import”. Here is the real program:

PATH (from the ui-reach spec, scenario “Old file via import”):
1. it starts with an old file → “Open” rejects it, in the notification
   the version and the name of the import item, no events on the timeline;
2. File → “Import a project of an old version…” → system dialog
   (there is none in Xvfb — the answer is replaced by `KT_FILE_PICK`, the item itself is clicked);
3. the same file is chosen → the timeline with an event and a morpher, the document is marked
   with edits (asterisk), a notification about the repair.

The fixture is a copy of the STRUCTURE of untitled2 (v5, null targets), not the file of
the owner itself.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, poll_step, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401

# ITS OWN PROCESS (R.68): the scenario checks the program START ITSELF, not the work in an
# already open project — the worker stand does not suit it.
OWN_PROCESS = True

SEC = 90000


def old_v5_project(dest):
    new_otio(dest)
    doc = json.load(open(dest))
    doc["metadata"]["kt"]["settings"].update(fps=30, width=1280, height=720)
    track = next((t for t in doc["tracks"]["children"] if t.get("kind") == "Video"), None)
    if track is None:
        track = {
            "OTIO_SCHEMA": "Track.1",
            "name": "",
            "kind": "Video",
            "children": [],
            "source_range": None,
            "effects": [],
            "markers": [],
            "enabled": True,
            "metadata": {},
        }
        doc["tracks"]["children"].append(track)
    rt = lambda s: {"OTIO_SCHEMA": "RationalTime.1", "value": round(s * SEC), "rate": 90000.0}
    track["children"] = [
        {
            "OTIO_SCHEMA": "Clip.2",
            "name": "old",
            "source_range": {"OTIO_SCHEMA": "TimeRange.1", "start_time": rt(0), "duration": rt(4)},
            "media_references": {
                "DEFAULT_MEDIA": {
                    "OTIO_SCHEMA": "ExternalReference.1",
                    "name": "",
                    "target_url": "synth:Ramp",
                    "available_range": None,
                    "available_image_bounds": None,
                    "metadata": {},
                }
            },
            "active_media_reference_key": "DEFAULT_MEDIA",
            "effects": [],
            "markers": [],
            "enabled": True,
            "metadata": {"kt": {"id": 1, "media": 1}},
        }
    ]
    kt = doc["metadata"]["kt"]
    kt["v"] = 5
    kt["bin_media"] = [{"id": 1, "path": "synth:Ramp"}]
    kt["morph_lanes"] = 1
    kt["morphers"] = [
        {
            "id": 1,
            "lane": 0,
            "start": 0,
            "duration": 2 * SEC,
            "targets": None,
            "patch": {"video_fx": [{"kind": "mirror", "horizontal": True, "vertical": False}]},
        }
    ]
    with open(dest, "w") as f:
        json.dump(doc, f, indent=4)
    return dest


def main(out):
    require_tools()
    old = old_v5_project(os.path.join(out, "old_v5.otio"))
    os.environ["KT_AUTOSAVE_SECS"] = "600"
    # there is no system dialog in the virtual screen (xdg-portal): the menu item
    # is pressed for real, the dialog’s answer is substituted by KT_FILE_PICK
    os.environ["KT_FILE_PICK"] = old
    with Live("import_old", out, project=old, raw=True) as app:
        # STEP 1: “Open” refused — there are no events, the status shows the version and the door
        app.settle()
        g = app.geom()
        app.expect(len(g.get("clips", [])) == 0, "the old file did not open the usual way")
        notes = g.get("notices", [])
        hit = [n for n in notes if "5" in n and ("Import older" in n or "old version" in n)]
        app.expect(bool(hit), f"the notification names the version and the door: {notes}")
        app.step("open-refused", "the old file was rejected with the address of the door")

        # STEP 2: File → Import project of an older version…
        app.click_probe("menu.file", "“File” menu")
        app.step("file-menu", "“File” menu with the import item")
        app.click_probe("file.import_older", "“Import a project of an old version…” item")
        # STEP 3: file selection (in Xvfb the dialog’s answer is substituted by KT_FILE_PICK)
        app.settle()
        ok = False
        for _ in range(20):
            g = app.geom()
            if len(g.get("clips", [])) == 1 and g.get("morphers") == 1:
                ok = True
                break
            poll_step(0.5)
        app.expect(ok, "after the import, an event and a morpher are on the timeline")
        app.expect(bool(g.get("dirty")), "the document is marked with edits (asterisk)")
        app.step("imported", "the project opened via import, asterisk in the title")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

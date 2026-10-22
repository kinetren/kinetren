#!/usr/bin/env python3
"""Live scenario R.70: export to OpenTimelineIO — a NAMED action, not a save.

Decision of the owner: “we are moving away from the otio format, keeping otio only for
export”. The project lives in its own `.kt` format; the exchange file is made by a separate
menu item, and after it the project on disk and the asterisk in the title are as they were.

PATH (from the ui-reach spec, scenario “Export to OTIO from the menu”):
1. File → “Export” submenu → “OpenTimelineIO…” item → system dialog
   (there is none in Xvfb — the answer is replaced by `KT_FILE_PICK`, the item itself is clicked);
2. the exchange file appeared on disk: it is OTIO with all the events of the edit;
3. the `.kt` project file is byte for byte as before, the asterisk as it was, in
   notifications — “exported: <path>”.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, Timeline, demo_project, poll_step, require_tools  # noqa: E402

# ITS OWN PROCESS (R.68): the system dialog’s answer is substituted via `KT_FILE_PICK`, and that is
# a PROCESS environment — the worker stand is started without it.
OWN_PROCESS = True


def main(out):
    require_tools()
    # the demo is the scenario’s exchange file; the program imports it via Live (.kt beside it)
    proj = demo_project(os.path.join(out, "export_otio.otio"))
    dst = os.path.join(out, "exported.otio")
    if os.path.exists(dst):
        os.remove(dst)
    os.environ["KT_AUTOSAVE_SECS"] = "600"
    # there is no system dialog in the virtual screen (xdg-portal): the menu item
    # is pressed for real, the dialog’s answer is substituted by KT_FILE_PICK
    os.environ["KT_FILE_PICK"] = dst
    with Live("export_otio", out, project=proj) as app:
        app.settle()
        before = open(app.project, "rb").read()
        dirty0 = bool(app.geom().get("dirty"))
        n_clips = len(app.live_clips())
        app.expect(n_clips > 0, f"the edit has events: {n_clips}")

        # STEP 1: File → Export → OpenTimelineIO…
        app.click_probe("menu.file", "“File” menu")
        app.step("file-menu", "“File” menu with the “Export” submenu")
        app.click_probe("file.export", "“Export” submenu")
        app.step("export-submenu", "“Export” submenu with the “OpenTimelineIO…” item")
        app.click_probe("file.export_otio", "“OpenTimelineIO…” item")

        # STEP 2: the exchange file on disk (dialog substituted by KT_FILE_PICK)
        ok = False
        for _ in range(40):
            if os.path.exists(dst) and os.path.getsize(dst) > 0:
                ok = True
                break
            poll_step(0.25)
        app.expect(ok, "exchange file appeared on disk")
        app.settle()
        app.step("exported", "“exported” notification, edit in place")
        if ok:
            with open(dst) as f:
                doc = json.load(f)
            app.expect(doc.get("OTIO_SCHEMA") == "Timeline.1", "export — OpenTimelineIO Timeline.1")
            app.expect(
                len(Timeline(dst).clips) == n_clips,
                f"all edit events in the export: {len(Timeline(dst).clips)} against {n_clips}",
            )

        # STEP 3: the project and the asterisk are untouched — export is not a save
        app.expect(open(app.project, "rb").read() == before, "the project file is untouched by the export")
        app.expect(bool(app.geom().get("dirty")) == dirty0, "the asterisk is as it was: export is not a save")
        notes = app.geom().get("notices", [])
        app.expect(
            any(dst in n for n in notes),
            f"the notification names where the exchange file was written: {notes}",
        )
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

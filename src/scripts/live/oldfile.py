#!/usr/bin/env python3
"""Live scenario R.63 (items 1-2): the old file opens, the tail is honest.

Two complaints from the owner in one window, and both were green in the harness:

1. “my untitled project stopped opening in the new version” — speed curves were written with the old field `ease: true`,
   which the R.61 reader expanded into “smooth”, while the speed validator accepts only linear. Here
   the real program opens a file of the same shape
   and plays it.
2. “a region loop placed explicitly at the end breaks the sound, the silence gets taken” — the event is
   longer than the material: past the end of the file there is silence, but each of its frames was counted as an
   underrun. Here the loop is placed ON THE TAIL, and the scenario reads underruns from
   the program itself (the geometry dump), not from the picture.

The fixture is made by hand: a short mp4 and an event twice as long as the material with a slow effect —
exactly the shape of the paintitblack project.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, color_media, pause, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio, project_kt_path  # noqa: E402,F401

# ITS OWN PROCESS (R.68): the scenario checks the program START ITSELF, not the work in an
# already open project — the worker stand does not suit it.
OWN_PROCESS = True

SEC = 90000


def old_project(dest, media, media_secs, clip_secs):
    """Project of the PREVIOUS shape: a speed curve with `ease: true` and an event
    that is longer than the material."""
    new_otio(dest)
    doc = json.load(open(dest))
    doc["metadata"]["kt"]["settings"].update(fps=30, width=1280, height=720)
    # an empty project is written without tracks (R.25) — we create the video track ourselves,
    # as `project_with_clips` of the common frame does
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
            "name": "long",
            "source_range": {
                "OTIO_SCHEMA": "TimeRange.1",
                "start_time": rt(0),
                "duration": rt(clip_secs),
            },
            "media_references": {
                "DEFAULT_MEDIA": {
                    "OTIO_SCHEMA": "ExternalReference.1",
                    "name": "",
                    "target_url": media,
                    "available_range": None,
                    "available_image_bounds": None,
                    "metadata": {},
                }
            },
            "active_media_reference_key": "DEFAULT_MEDIA",
            # SPEED of the old form: `ease: true` on the curve `rate`
            "effects": [
                {
                    "OTIO_SCHEMA": "Effect.1",
                    "name": "",
                    "effect_name": "kt:slowed_reverb",
                    "enabled": True,
                    "metadata": {
                        "kt": {
                            "kind": "slowed_reverb",
                            "rate": 0.5,
                            "mix": 0.0,
                            "reverb_delay_ms": 120.0,
                            "damp": 0.1,
                            "decay": 0.5,
                            "wet_gain": 1.0,
                            "auto": [
                                {
                                    "param": "rate",
                                    "ease": True,
                                    "keys": [
                                        {"t": 0, "v": 0.5},
                                        {"t": round(clip_secs * SEC), "v": 0.5},
                                    ],
                                }
                            ],
                        }
                    },
                }
            ],
            "markers": [],
            "enabled": True,
            "metadata": {"kt": {"id": 1, "media": 1}},
        }
    ]
    doc["metadata"]["kt"]["bin_media"] = []
    json.dump(doc, open(dest, "w"), indent=4)
    return dest


def main(out):
    require_tools()
    media_secs, clip_secs = 4, 12  # 4 s of material at rate 0.5 = 8 s of the event
    media = color_media(out, "short", "1E7FC8", seconds=media_secs)
    path = old_project(os.path.join(out, "oldfile.otio"), media, media_secs, clip_secs)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    # 1) a file of the OLD form must be readable — this is the claim 1.
    # R.70: old forms are read by the IMPORT DOOR; the program lives in .kt
    imp = subprocess.run(
        [KT_CLI, "import", path, project_kt_path(path), "--force"], capture_output=True, text=True
    )
    path = project_kt_path(path)
    probe = subprocess.run(
        [KT_CLI, "probe", path, "0", "1"], capture_output=True, text=True
    )
    ok = imp.returncode == 0 and "unsupported schema" not in (imp.stderr + probe.stdout + probe.stderr)
    print(f"   {'ok  ' if ok else 'FAILURE'} file with the old speed curves is read")
    if not ok:
        print(probe.stdout[-400:], probe.stderr[-400:])
        return 1

    with Live("oldfile", out, project=path) as app:
        app.expect(len(app.live_clips()) >= 1, f"event on the montage: {app.live_clips()}")
        app.shot("opened")

        # 2) THE TAIL WITHOUT MATERIAL is visible: the program gives its rectangle
        app.expect(
            app.has_probe("clip.1.no_material"),
            "the tail without material is marked on the event (hatching)",
        )

        # 3) LOOP ON THE TAIL does not produce underruns
        # the material ends at 8 s (4 s at rate 0.5), the event runs to 12 s
        app.click(app.x_at(int(9.2 * SEC)), app.ruler_y())
        app.drag(app.x_at(int(9.0 * SEC)), app.ruler_y(), app.x_at(int(11.0 * SEC)), app.ruler_y())
        app.key("l")  # turn on loop
        before = app.geom().get("underruns", 0)
        app.key("space")
        pause(
            4.0,
            "the loop on the tail is checked by LIVE playback: four seconds of real "
            "time — this is the check itself, not a wait for the interface",
        )
        after = app.geom().get("underruns", 0)
        playing = app.playing()
        app.key("space")
        app.shot("loop-tail")
        app.expect(playing, "playback went on the tail")
        app.expect(
            after == before,
            f"the loop on the tail WITHOUT material does not produce underruns: was {before}, became {after}",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/oldfile"))

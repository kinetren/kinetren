#!/usr/bin/env python3
"""Live scenario t3842 (project-cache-folder; the owner's kkt-26.11.20 A14: a subfolder of the project's folder with the caches):
the project's caches lie in `kinetren-cache` beside it, Preferences list the folder, Delete empties it.

The project is ours (`kt-cli showcase`). The verdict reads the disk: the folder's marks, the media's
entries after the opening, no former sidecar, nothing of the project's work touched by Delete, and the
entries back after a restart.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
import kt_live  # noqa: E402
from kt_live import Live, require_tools  # noqa: E402
from readme_shots import fixture_sources  # noqa: E402

OWN_PROCESS = True


def entries(root, name):
    """How many media of the folder hold the entry `name`."""
    media = os.path.join(root, "media")
    if not os.path.isdir(media):
        return 0
    return sum(1 for d in os.listdir(media) if os.path.exists(os.path.join(media, d, name)))


def main(out):
    require_tools()
    sources = fixture_sources(os.path.join(out, "sources"))
    home = os.path.join(out, "film")
    os.makedirs(home, exist_ok=True)
    proj = os.path.join(home, "film.kt")
    subprocess.run([kt_live.KT_CLI, "showcase", proj, "--sources", sources, "--force"], check=True, capture_output=True)
    root = os.path.join(home, "kinetren-cache")
    text = open(proj).read()
    before = sorted(os.listdir(home))
    with Live("cache_folder", out, project=proj, own_process=True) as app:
        app.until(lambda: entries(root, "peaks") > 0 and entries(root, "thumbs") > 0, timeout=120)
        app.expect(entries(root, "peaks") > 0, "the waveforms were kept in the folder beside the project")
        app.expect(entries(root, "thumbs") > 0, "the filmstrips were kept there")
        app.expect(all(os.path.exists(os.path.join(root, f)) for f in ("CACHEDIR.TAG", "README.txt", ".gitignore")),
                   "the folder says what it is")
        # the stand's own export beside the project is not the cache's
        new = [f for f in sorted(os.listdir(home)) if f not in before and not f.endswith(".export.otio")]
        app.expect(new == ["kinetren-cache"], f"one folder appeared beside the project: {new}")
        app.step("open", "the project is open: its waveforms and filmstrips are kept in kinetren-cache beside it")

        app.click_probe("menu.file", "the File menu")
        app.click_probe("file.preferences", "File → Preferences…")
        app.expect(app.has_probe("prefs.window"), "File → Preferences… opened its window")
        app.until(lambda: app.has_probe("prefs.cache.0.delete"), timeout=30)
        app.expect(app.has_probe("prefs.cache.0.delete"), "the section “Persistent cache” lists the folder")
        app.hover_probe("prefs.cache.0.cap", "Cap, GB")
        app.step("preferences", "Preferences: “Persistent cache” lists the folder with its project, size and cap")
        app.click_probe("prefs.cache.0.delete", "Delete (the folder)")
        app.until(lambda: entries(root, "peaks") == 0, timeout=60)
        app.expect(entries(root, "peaks") == 0, "Delete emptied the folder")
        app.expect(open(proj).read() == text, "the project was not touched")
        app.wait_frame(2, what="the frame after Delete", soft=True)
        app.step("deleted", "the folder is empty; the timeline still draws its waveforms and filmstrips")
        app.click_probe("prefs.close", "Close (Preferences)")
        app.expect(app.wait_gone("prefs.window"), "Close closed Preferences")
        verdict = app.done()
    if verdict:
        return verdict

    # the folder of the next run is full: an old result takes all of a small cap, and the first entry
    # computed again takes its place
    old = os.path.join(root, "results", "v0", "old", "frames", "old.mkv")
    os.makedirs(os.path.dirname(old), exist_ok=True)
    with open(old, "wb") as f:
        f.write(b"\0" * 3_000_000)
    with open(os.path.join(out, "xdg", "kinetren", "cache-folders.tsv"), "w") as f:
        f.write(f"{root}\t0\t0\t3000000\t{proj}\n")
    with Live("cache_folder_restart", out, project=proj, own_process=True) as app:
        app.until(lambda: entries(root, "peaks") > 0, timeout=120)
        app.expect(entries(root, "peaks") > 0, "after a restart what was deleted is computed and kept again")
        app.until(lambda: not os.path.exists(old), timeout=60)
        app.expect(not os.path.exists(old), "the old result gave its place")
        app.until(lambda: any("is full" in n for n in app.geom().get("notices", [])), timeout=30)
        full = [n for n in app.geom().get("notices", []) if "is full" in n]
        app.expect(len(full) == 1, f"one notice says the folder is full: {full}")
        app.step("restart", "the project opened again: the folder fills again, the old result replaced")
        app.click_probe("bell", "the notices")
        app.expect(app.has_probe("notices.persistent_cache"), "the notice has its button")
        app.step("notice", "the notice of the full folder with its button")
        app.click_probe("notices.persistent_cache", "Persistent cache…")
        app.expect(app.has_probe("prefs.window"), "the button opened Preferences")
        app.step("from-notice", "Preferences at “Persistent cache”, opened from the notice")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))

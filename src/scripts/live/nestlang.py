#!/usr/bin/env python3
"""Live scenario i18n-gaps (D3): a reel the program named follows the interface language.

The owner (ktprompts 26.10.18 item 10): «Лента» in a published composition parameter in the
English interface. The collapse gesture names a new reel with the interface's default word and
writes it into the project; reopened in another language the word stayed Russian.

PATH (ui-timeline spec, "The reel's name in the interface language"):
1. in the RUSSIAN interface two events are collapsed — the program names the reel «Лента» and
   the project is saved so;
2. the program is started again in ENGLISH on the saved file → the bin card reads "Reel";
3. "Composition parameters" on the instance → the head of its chain strip reads "📌 Reel";
4. entering the reel → the crumbs read "Reel"; the file still holds «Лента».
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

# the language is read at start-up: each half owns its process
OWN_PROCESS = True


def main(out):
    require_tools()
    path = os.path.join(out, "nestlang.otio")
    proj = project_with_clips(path, [(0.0, 2.0, "synth:Ramp"), (2.0, 2.0, "synth:Ramp")])
    was = os.environ.get("KT_LANG")
    os.environ["KT_LANG"] = "ru"
    try:
        with Live("nestlang-ru", out, project=proj, own_process=True) as app:
            y = app.lane_y("video", 0, 0.6)
            clips = sorted(app.live_clips())
            app.click(app.x_at(clips[0][0] + clips[0][1] // 2), y)
            app.click(app.x_at(clips[1][0] + clips[1][1] // 2), y, mods=("ctrl",))
            app.right_click(app.x_at(clips[1][0] + clips[1][1] // 2), y)
            app.click_probe("nest.collapse", "collapse item in the Russian interface")
            app.saved_project()
            kt = app.project
            with open(kt, encoding="utf-8") as f:
                stored = f.read()
            app.expect('"Лента"' in stored, "the program named the reel «Лента» in the file")
            app.step("ru-collapsed", "Russian interface: the reel is collapsed and saved as «Лента»")
            ru = app.done()
        os.environ["KT_LANG"] = "en"
        with Live("nestlang-en", out, project=kt, own_process=True) as app:
            y = app.lane_y("video", 0, 0.6)
            inst = sorted(app.live_clips())[0]
            ix = app.x_at(inst[0] + inst[1] // 2)
            app.step("en-bin", "English interface on the same file: the bin card reads “Reel”")
            app.right_click(ix, y)
            app.click_probe("pub.inst_open", "“Composition parameters” on the instance")
            app.expect(app.has_probe("fx.head"), "the instance's FX window shows its strip head")
            app.step("en-strip-head", "the head of the instance's chain strip reads “📌 Reel”")
            app.key("Escape")
            app.right_click(ix, y)
            app.click_probe("nest.enter", "“Enter the reel” on the instance")
            name = app.geom()["nest"]["name"]
            app.expect(name == "Reel", f"inside the reel the crumbs read “Reel”: {name!r}")
            app.step("en-crumbs", "inside the reel: the crumbs read “Reel”")
            with open(kt, encoding="utf-8") as f:
                app.expect('"Лента"' in f.read(), "showing the name did not change the file")
            return max(ru, app.done())
    finally:
        if was is None:
            os.environ.pop("KT_LANG", None)
        else:
            os.environ["KT_LANG"] = was


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

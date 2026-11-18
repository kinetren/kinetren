#!/usr/bin/env python3
"""Live scenario curve-key-selection (`v26.10` A1, the owner's answer U9 of kkt-26.11.18b): several
keys of a curve are selected and moved together.

His words (`docs/ktprompts/v26.10.txt` line 70): the automation timeline needs a way to select
keys and move them; asked how, option one, plus Shift-clicking the wanted keys or Shift-selecting
more segments.

PATH (automation "The selected keys of a curve move together"):

1. FX window of the event → "+ curve" → parameter; three clicks — five keys, four segments;
2. Ctrl+click on the second segment → it is tinted, its two keys turn white;
3. Shift+click on the fourth key → it turns white too;
4. the second key is pulled right → the three white keys slide together, the first and the last stay;
5. a plain click on the first key drops the selection; pulling the third key then moves it alone.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, poll_step, require_tools  # noqa: E402
from kt_live import demo_otio  # noqa: E402

SEC = 90000


def probes(app):
    return {f["param"]: f for f in app.geom().get("fields", [])}


def at(r, fx=0.5, fy=0.5):
    win = geometry()
    return win["x"] + int(r["x"] + r["w"] * fx), win["y"] + int(r["y"] + r["h"] * fy)


def key_xs(app):
    f = probes(app)
    return [f[f"auto.key.{i}"]["x"] + f[f"auto.key.{i}"]["w"] / 2 if f"auto.key.{i}" in f else None
            for i in range(5)]


def main(out):
    require_tools()
    path = os.path.join(out, "curve_keys.otio")
    demo_otio(path)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("curve_keys", out, project=path) as app:
        # 1) the FX window, a curve, five keys
        app.double_click(app.x_at(1 * SEC), app.lane_y("video", 0, frac=0.5))
        app.expect(app.has_probe("fx.cancel"), "the effects window opened")
        f = probes(app)
        add = next((k for k in f if k.startswith("auto.add.")), None)
        if add is None:
            app.click_probe("auto.add", "“+ curve” button")
            f = probes(app)
            add = next((k for k in f if k.startswith("auto.add.")), None)
        app.expect(add is not None, "the “+ curve” menu has a parameter")
        if add:
            app.click_probe(add, f"parameter “{add}”")
        app.expect(app.has_probe("auto.graph"), "the automation graph is shown")
        # the graph whole inside the window (the wheel over "+ curve": the graph takes the wheel)
        app.scroll_probe_into_view("auto.graph", "auto.add", below="fx.cancel",
                                   what="the graph scrolled into the window")
        g = probes(app)["auto.graph"]
        for fx, fy in ((0.25, 0.4), (0.5, 0.7), (0.75, 0.4)):
            app.click(*at(g, fx, fy))
            poll_step(0.3)
        app.expect(app.has_probe("auto.key.4"), "five keys on the curve")
        app.step("keys", "five keys, four segments")

        # 2) Ctrl+click the second segment: its keys are selected
        f = probes(app)
        app.click(*at(f["auto.seg.1"], 0.5, 0.1), mods=("ctrl",))
        app.step("segment", "the second segment tinted, its two keys white")

        # 3) Shift+click the fourth key
        app.click(*at(probes(app)["auto.key.3"]), mods=("shift",))
        app.step("shift-key", "the fourth key white too")
        app.scroll_probe_into_view("auto.graph", "auto.add", below="fx.cancel",
                                   what="the graph still inside the window")
        before = key_xs(app)
        app.expect(None not in before, f"every key is published: {before}")

        # 4) pull the second key right: the three white keys move together
        k1 = probes(app)["auto.key.1"]
        x, y = at(k1)
        room = (before[2] - before[1]) * 0.4
        app.drag(x, y, x + int(room), y)
        moved = app.until(lambda: (lambda a: a[1] is not None and a[1] - before[1] > 3)(key_xs(app)))
        after = key_xs(app)
        app.step("moved", "the three white keys slid right together, the first and the last stayed")
        app.expect(moved, f"the grabbed key moved: {before} → {after}")
        shifts = [a - b for a, b in zip(after, before)]
        app.expect(abs(shifts[0]) < 1 and abs(shifts[4]) < 1, f"the unselected keys stayed: {shifts}")
        app.expect(max(shifts[1:4]) - min(shifts[1:4]) < 1.5, f"the selected keys moved by one shift: {shifts}")

        # 5) a plain click on a key outside the selection drops it: the next pull moves one key
        app.click(*at(probes(app)["auto.key.0"]))
        poll_step(0.3)
        start = key_xs(app)
        x, y = at(probes(app)["auto.key.2"])
        app.drag(x, y, x + int((start[3] - start[2]) * 0.4), y)
        app.until(lambda: (lambda a: a[2] is not None and a[2] - start[2] > 3)(key_xs(app)))
        end = key_xs(app)
        app.step("alone", "after a plain click elsewhere the third key moves alone")
        shifts = [a - b for a, b in zip(end, start)]
        app.expect(shifts[2] > 3, f"the third key moved: {start} → {end}")
        app.expect(all(abs(shifts[i]) < 1 for i in (0, 1, 3, 4)), f"no other key moved with it: {shifts}")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

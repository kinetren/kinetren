#!/usr/bin/env python3
"""README screenshots (readme-showcase): the real editor on real footage.

Builds the showcase project with `kt-cli showcase`, then takes every image
the README and `docs/guide/` show — each from a fresh `kt-app` process with
the English UI. Views are reached through the probe rectangles the program
publishes (`win.fx`, `win.transition`, `tb.diag`, …) and real gestures;
crops are those rectangles, never constants. Every step checks its own
precondition and stops NAMING THE STEP instead of writing a wrong picture.

Two ways in:

  * under `live-check.sh` (no arguments but the artifact directory): the
    sources are copies of the generated fixture videos, the images go to
    `<out>/images` — the pipeline is exercised on every `finish.sh`;
  * `src/scripts/readme-shots.py --sources DIR [--out DIR]` — the manual
    run on real footage, writing `docs/images/` by default.

`SHOTS` is read by `src/scripts/readme-check.py` with `ast.literal_eval`:
keep it a plain literal. Every name in it is one `docs/images/<name>.webp`,
and the guard requires the docs to reference exactly this set.
"""
import json
import os
import shutil
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))

# the step must own the process: KT_LANG is read at start-up
OWN_PROCESS = True

SHOTS = [
    ("hero", "the whole window during playback: free-layout bin, layered preview, live meters"),
    ("timeline", "the timeline: a nested reel expanded in place mid-edit, its veiled tails"),
    ("media-bin", "the media bin in free layout with its groups"),
    ("reel-parameters", "the published parameters of a reel instance"),
    ("search", "Ctrl+F: searching the project"),
    ("morpher-before-after", "the same frame without and with the morpher"),
    ("slowed-reverb-automation", "Slowed+Reverb on the music bed with rate/mix curves"),
    ("video-fx", "the video FX editor with the effect picker open"),
    ("titles", "a title generator in the event window"),
    ("transitions", "the transition editor"),
    ("morpher", "the morpher batch editor"),
    ("render-dialog", "the render dialog"),
    ("perf-panel", "the diagnostics panel during playback"),
    ("fx-datamosh", "the same frame after a cut without and with Datamosh (Cut mosh)"),
    ("fx-camera-isp", "the same frame without and with Camera ISP (Phone at night)"),
    ("fx-vhs-tape", "the VHS Tape deck face with a clogged head, and the frame it gives"),
    ("masks", "a Pen mask drawn on the preview, a Duotone limited to it"),
]

# Shots the pipeline takes whose images wait for the owner's run on the showcase footage (he
# fetches it himself, 2026-10-21): `readme-check.py` lets one be absent while nothing references
# it. Once `readme-shots.py` has written it, reference it in the docs and drop it from here.
AWAITING_FOOTAGE = ["fx-datamosh", "fx-camera-isp", "fx-vhs-tape", "masks"]

RUST = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
FIXTURES = os.path.join(RUST, "fixtures")
WINDOW = (1920, 1080)


class StepError(Exception):
    pass


def fixture_sources(dest):
    """Copies of the generated fixture videos: the pipeline, not the look."""
    subprocess.run(["bash", os.path.join(FIXTURES, "gen.sh")], cwd=FIXTURES, check=True,
                   capture_output=True)
    os.makedirs(dest, exist_ok=True)
    for name in sorted(os.listdir(os.path.join(FIXTURES, "out"))):
        # the videos plus one audio-only file: the music bed, as with real footage
        if name.endswith(".mp4") or name == "sine48k_stereo.flac":
            shutil.copy(os.path.join(FIXTURES, "out", name), os.path.join(dest, name))
    return dest


def build_project(kt_cli, sources, work):
    path = os.path.join(work, "showcase.kt")
    r = subprocess.run([kt_cli, "showcase", path, "--sources", sources, "--force"],
                       capture_output=True, text=True)
    if r.returncode != 0:
        raise StepError(f"kt-cli showcase failed: {r.stderr.strip()}")
    print(f"   {r.stdout.strip()}")
    return path


def targets(project):
    """What the shots point at, by meaning, from the project file."""
    doc = json.load(open(project))["project"]
    clips = sorted(doc["clips"], key=lambda c: (c["start"], c["id"]))
    by_id = {c["id"]: c for c in clips}

    def first(what, pred):
        for c in clips:
            if pred(c):
                return c
        raise StepError(f"showcase project: no {what}")

    music = first("Slowed+Reverb bed", lambda c: c["kind"] == "audio" and c["lane"] == 1
                  and any(f.get("kind") == "slowed_reverb" for f in c.get("fx", [])))
    # a double click inside a transition window opens the transition, not
    # the event: the styled cut must have none on either side
    joined = {t[k] for t in doc["transitions"] for k in ("a", "b") if t.get(k)}
    styled = [c for c in clips if c["kind"] == "video" and c["lane"] == 1 and c.get("fx")
              and c["id"] not in joined]
    if not styled:
        raise StepError("showcase project: no styled montage cut without transitions")
    video = max(styled, key=lambda c: (c["duration"], -c["start"]))
    title = first("title", lambda c: (c.get("gen") or {}).get("kind") == "text")
    trans = next((t for t in doc["transitions"]
                  if t["kind"].get("kind") != "crossfade"
                  and by_id.get(t["b"], {}).get("kind") == "video"), None)
    if trans is None:
        raise StepError("showcase project: no non-crossfade video transition")
    if not doc.get("morphers"):
        raise StepError("showcase project: no morpher")
    if not doc.get("sequences"):
        raise StepError("showcase project: no nested reel")
    reel_id = doc["sequences"][0]["id"]
    # the instance that sets its own values (the one mid-edit)
    reel = first("reel instance with overrides",
                 lambda c: c["media"] == reel_id and c.get("overrides"))
    # full-frame cuts for the effect shots: a main-track video event nothing else covers (no
    # other video track at its time, no transition, not a reel or a generator)
    reels = {q["id"] for q in doc["sequences"]}
    paths = {m["id"]: m.get("path", "") for m in doc.get("media", [])}
    videos = [c for c in clips if c["kind"] == "video"]

    def alone(c):
        end = c["start"] + c["duration"]
        return (c["kind"] == "video" and c["lane"] == 1 and c["duration"] >= 72000
                and c["id"] not in joined and c["media"] not in reels and not c.get("gen")
                and not any(o["lane"] != c["lane"] and o["start"] < end
                            and o["start"] + o["duration"] > c["start"] for o in videos))

    plain = [c for c in clips if alone(c)]
    if not plain:
        raise StepError("showcase project: no main-track event alone on screen")
    after_cut = [c for c in plain
                 if any(o["lane"] == c["lane"] and o["kind"] == "video"
                        and o["start"] + o["duration"] == c["start"] and o["media"] != c["media"]
                        for o in clips)]
    if not after_cut:
        raise StepError("showcase project: no main-track cut between two sources")
    # a dark clip shows the night phone best (the Charlotte night drone shot of the footage)
    night = next((c for c in plain if "charlotte" in paths.get(c["media"], "").lower()), plain[0])
    return {"reel": reel, "music": music, "video": video, "title": title,
            "transition": (trans, by_id[trans["b"]]), "morph": doc["morphers"][0],
            "cut": after_cut[len(after_cut) // 2], "night": night, "plain": plain[len(plain) // 2]}


class Shooter:
    def __init__(self, kt_live, project, tg, raw):
        self.kt = kt_live
        self.project = project
        self.tg = tg
        self.raw = raw
        self.crops = {}  # name -> (full-screen png, (x, y, w, h) on screen)
        self.keep = {}   # name -> rectangles kept opaque inside the crop

    # ---- the app -------------------------------------------------------

    def fresh(self, step):
        out = os.path.join(self.raw, step)
        shutil.rmtree(out, ignore_errors=True)
        # a previous shot may have left a recovery file next to the project
        # (expanding the reel is an edit); the next app would offer to
        # restore it in a modal over the view — every shot starts clean
        base = os.path.basename(self.project)
        folder = os.path.dirname(self.project)
        for name in os.listdir(folder):
            if name.startswith(base + ".recovery"):
                os.remove(os.path.join(folder, name))
        app = self.kt.Live(f"readme-{step}", out, project=self.project, own_process=True)
        app.__enter__()
        try:
            app.resize(*WINDOW)
            self.require(step, app.wait_loaded(), "project on screen")
            still = self.still(app, step, "screen settled (thumbnails and waveforms loaded)")
            lit = self.preview_lit(app, still)
            self.require(step, lit > 0.01, f"decoded frame in the preview (lit {lit:.1%})")
        except BaseException:
            self.close(app)
            raise
        return app

    @staticmethod
    def close(app):
        app._reported = True
        app.__exit__(None, None, None)

    def require(self, step, ok, what):
        if not ok:
            raise StepError(f"step “{step}”: expected {what}")

    def preview_lit(self, app, png):
        """Share of non-dark pixels in the middle band above the timeline."""
        from PIL import Image

        g = app.geom()
        x0, x1 = g["window"]["w"] * 0.2, g["window"]["w"] * 0.8
        x, y, w, h = self.abs((x0, 0, x1 - x0, g["timeline"]["top"]))
        with Image.open(png) as im:
            px = list(im.convert("L").crop((x, y, x + w, y + h)).reduce(4).getdata())
        return sum(1 for v in px if v > 40) / max(len(px), 1)

    def still(self, app, step, what, timeout=90.0, quiet=0.5):
        """The screen unchanged for `quiet` seconds: thumbnails, waveforms
        and the decoded preview arrive asynchronously and are done only
        then. Filmstrips of 4K footage arrive with pauses between frames,
        so a zoomed timeline needs a longer quiet period."""
        deadline = time.time() + timeout
        prev = app.shot("still")
        calm = 0.0
        while time.time() < deadline:
            self.kt.poll_step(0.5)
            if app.screen_diff(prev) == 0.0:
                calm += 0.5
                if calm >= quiet:
                    return prev
            else:
                calm = 0.0
                prev = app.shot("still")
        self.require(step, False, what)

    def probe(self, app, step, name, shown):
        """`shown` is `app.has_probe("<name>")` written AT THE CALL SITE: the
        coverage guard (live-coverage.py, t804) reads probe names literally."""
        self.require(step, shown, f"probe “{name}” on screen")
        f = app._probe_rect(name)
        self.require(step, f is not None, f"rectangle of probe “{name}”")
        return f

    def abs(self, rect):
        win = self.kt.geometry()
        x, y, w, h = rect
        return (win["x"] + round(x), win["y"] + round(y), round(w), round(h))

    def capture(self, app, step, name, r, settle=True):
        path = self.still(app, step, f"screen settled before {name}") if settle else app.shot(name)
        box = self.abs((r["x"], r["y"], r["w"], r["h"]))
        self.require(step, box[2] >= 50 and box[3] >= 50, f"non-trivial crop for {name}: {box}")
        self.crops[name] = (path, box)
        app.step(name, dict(SHOTS)[name])

    def below_header(self, app, step, name, shown):
        """Floating windows open over the menu bar; drag one down by its
        title strip so the app header does not show through it."""
        r = self.probe(app, step, name, shown)
        menu = self.probe(app, step, "menu.file", app.has_probe("menu.file"))
        floor = menu["y"] + menu["h"] + 8
        if r["y"] >= floor:
            return r
        win = self.kt.geometry()
        x = win["x"] + int(r["x"] + r["w"] / 2)
        y = win["y"] + int(r["y"] + 10)
        app.drag(x, y, x, y + int(floor - r["y"]))
        moved = self.probe(app, step, name, True)
        self.require(step, moved["y"] >= floor - 2, f"“{name}” below the menu bar")
        return moved

    @staticmethod
    def union(a, b):
        x0, y0 = min(a["x"], b["x"]), min(a["y"], b["y"])
        x1 = max(a["x"] + a["w"], b["x"] + b["w"])
        y1 = max(a["y"] + a["h"], b["y"] + b["h"])
        return {"x": x0, "y": y0, "w": x1 - x0, "h": y1 - y0}

    def point(self, app, step, kind, lane, t, frac=0.6):
        x = app.x_at(t)
        y = app.lane_y(kind, lane, frac=frac)
        win = self.kt.geometry()
        self.require(step, win["x"] < x < win["x"] + win["w"], f"{kind} lane {lane} at {t} in view")
        return x, y

    def double_click_until(self, app, step, x, y, name, tries=3):
        """Double click until the window's probe shows: a slow UI frame can
        split the two clicks in egui's frame clock (kt-app CLAUDE.md)."""
        for _ in range(tries):
            app.double_click(x, y)
            if app.has_probe(name, tries=8):
                return
        self.require(step, False, f"“{name}” opened by a double click")

    def open_event(self, app, step, clip):
        x, y = self.point(app, step, clip["kind"], clip["lane"], clip["start"] + clip["duration"] // 2)
        # the context menu, not a double click: under a heavy decode the
        # UI frame is slow and two clicks fall apart in egui's frame clock
        app.click(x, y)
        app.right_click(x, y)
        self.require(step, app.click_probe("tl.fx"), "click on “tl.fx”")
        return self.below_header(app, step, "win.fx", app.has_probe("win.fx"))

    # ---- the shots -----------------------------------------------------

    def widen_bin(self, app, step, width=470):
        """Drag the left column's edge so the bin shows its free layout."""
        bin_ = app.geom()["bin"]
        if bin_["right"] - bin_["left"] >= width - 20:
            return
        win = self.kt.geometry()
        y = win["y"] + int((bin_["top"] + bin_["bottom"]) / 2)
        for off in (5, 8, 3, 11):
            x = win["x"] + int(bin_["right"]) + off
            app.drag(x, y, x + int(width - (bin_["right"] - bin_["left"])), y)
            now = app.geom()["bin"]
            if now["right"] - now["left"] > bin_["right"] - bin_["left"] + 50:
                return
        self.require(step, False, f"the bin panel widened (still {app.geom()['bin']})")

    def overview(self):
        step = "overview"
        app = self.fresh(step)
        try:
            self.widen_bin(app, step)
            self.still(app, step, "screen settled with the wide bin")
            # playback: the meters show live levels, the preview moves
            app.key("space")
            self.kt.pause(3.0, "meters and preview need a moment of playback to show levels")
            self.require(step, app.playing(), "playback running")
            g = app.geom()
            ww, wh = self.kt.geometry()["w"], self.kt.geometry()["h"]
            tl, bin_ = g["timeline"], g["bin"]
            self.capture(app, step, "hero", {"x": 0, "y": 0, "w": ww, "h": wh}, settle=False)
            hero = self.crops["hero"][0]
            bottom = min(bin_["bottom"], tl["top"])
            m = min(bin_["left"], 6)
            self.crops["media-bin"] = (hero, self.abs(
                (bin_["left"] - m, bin_["top"], bin_["right"] - bin_["left"] + 2 * m,
                 bottom - bin_["top"])))
            app.step("media-bin", dict(SHOTS)["media-bin"])
            app.key("space")
        finally:
            self.close(app)

    def timeline(self):
        """The mid-edit reel instance, expanded in place and zoomed so it sits
        in the middle with its veiled tails on both sides."""
        step = "timeline"
        app = self.fresh(step)
        try:
            reel = self.tg["reel"]
            rows_before = len(app.geom()["rows"])
            mid = reel["start"] + reel["duration"] // 2
            x, y = self.point(app, step, "video", reel["lane"], mid)
            # under load a slow frame can swallow the selection or the key:
            # select and press Enter again until the reel's rows appear
            for _ in range(3):
                app.click(x, y)
                app.key("Return")
                for _ in range(20):
                    if len(app.geom()["rows"]) > rows_before:
                        break
                    app.wait_frame(1, what="the reel expands in place", soft=True)
                if len(app.geom()["rows"]) > rows_before:
                    break
            self.require(step, len(app.geom()["rows"]) > rows_before,
                         f"the reel expanded in place (rows {rows_before} → {len(app.geom()['rows'])})")
            # zoom in around the instance until it is ~a third of the panel
            win = self.kt.geometry()
            tl = app.geom()["timeline"]
            wy = win["y"] + int((tl["top"] + tl["bottom"]) / 2)
            for _ in range(12):
                g = app.geom()
                if reel["duration"] / g["ticks_per_px"] >= (tl["right"] - tl["left"]) / 3:
                    break
                wx = int(app.x_at(mid))
                app._mods(["ctrl"], True)
                app.wheel(wx, wy, -1)
                app._mods(["ctrl"], False)
            g = app.geom()
            self.require(step, reel["duration"] / g["ticks_per_px"] >= 300,
                         f"the reel instance is wide on screen ({reel['duration'] / g['ticks_per_px']:.0f} px)")
            # centre it: plain wheel scrolls time
            for _ in range(30):
                cx = app.x_at(mid) - win["x"]
                centre = (tl["left"] + tl["right"]) / 2
                if abs(cx - centre) < 80:
                    break
                app.wheel(win["x"] + int(centre), wy, 1 if cx > centre else -1)
            self.still(app, step, "screen settled with the reel expanded", timeout=180.0, quiet=6.0)
            ww, wh = win["w"], win["h"]
            top = app.geom()["timeline"]["top"]
            self.capture(app, step, "timeline", {"x": 0, "y": top, "w": ww, "h": wh - top})
        finally:
            self.close(app)

    def search(self):
        step = "search"
        app = self.fresh(step)
        try:
            app.key("ctrl+f")
            self.require(step, app.has_probe("win.search"), "the search window opened")
            app.type_text("char")
            r = self.probe(app, step, "win.search", app.has_probe("win.search"))
            self.capture(app, step, "search", r)
        finally:
            self.close(app)

    def reel_parameters(self):
        step = "reel-parameters"
        app = self.fresh(step)
        try:
            r = self.open_event(app, step, self.tg["reel"])
            self.capture(app, step, "reel-parameters", r)
        finally:
            self.close(app)

    def morpher_before_after(self):
        """The same frame from the project without the morpher (a copy with
        the morpher switched off) and with it, side by side."""
        from PIL import Image, ImageDraw, ImageFont

        step = "morpher-before-after"
        m = self.tg["morph"]
        at = m["start"] + m["duration"] // 2
        doc = json.load(open(self.project))
        doc["project"]["morphers"][0]["enabled"] = False
        before_project = self.project.replace(".kt", "-no-morpher.kt")
        with open(before_project, "w") as f:
            json.dump(doc, f, indent=2, ensure_ascii=False)
            f.write("\n")
        frames = []
        for tag, project in (("before", before_project), ("after", self.project)):
            saved = self.project
            self.project = project
            try:
                app = self.fresh(f"{step}-{tag}")
            finally:
                self.project = saved
            try:
                x = app.x_at(at)
                app.click(x, app.ruler_y())
                self.still(app, step, f"the {tag} frame settled")
                fr = self.probe(app, step, "preview.frame", app.has_probe("preview.frame"))
                png = app.shot(f"{tag}-frame")
                box = self.abs((fr["x"], fr["y"], fr["w"], fr["h"]))
                with Image.open(png) as im:
                    frames.append(im.convert("RGB").crop(
                        (box[0], box[1], box[0] + box[2], box[1] + box[3])))
            finally:
                self.close(app)
        w = 900
        h = round(frames[0].height * w / frames[0].width)
        label_h, gap = 44, 12
        out = Image.new("RGB", (2 * w + gap, h + label_h), (24, 24, 28))
        draw = ImageDraw.Draw(out)
        try:
            font = ImageFont.truetype("DejaVuSans-Bold.ttf", 24)
        except OSError:
            font = ImageFont.load_default()
        for i, (img, label) in enumerate(zip(frames, ("Without the morpher", "With the morpher: source swap + VHS + glow"))):
            out.paste(img.resize((w, h)), (i * (w + gap), label_h))
            draw.text((i * (w + gap) + 12, 10), label, fill=(230, 230, 230), font=font)
        path = os.path.join(self.raw, "morpher-before-after.png")
        out.save(path)
        self.crops["morpher-before-after"] = (path, (0, 0, out.width, out.height))

    # ---- the effect shots (effect-shots-and-update-strip, the site's request) ----

    def seek(self, app, t):
        app.click(app.x_at(t), app.ruler_y())

    def frame_image(self, app, step, tag, differs_from=None, tries=40):
        """The preview frame as an image once it settled; with `differs_from`, first wait for
        the picture to change (after a seek the preview holds the last frame until the new
        one is decoded)."""
        from PIL import Image, ImageChops

        fr = self.probe(app, step, "preview.frame", app.has_probe("preview.frame"))
        box = self.abs((fr["x"], fr["y"], fr["w"], fr["h"]))

        def grab(name):
            with Image.open(app.shot(name)) as im:
                return im.convert("RGB").crop((box[0], box[1], box[0] + box[2], box[1] + box[3]))

        if differs_from is not None:
            for _ in range(tries):
                img = grab(f"{tag}-wait")
                if img.size == differs_from.size and ImageChops.difference(img, differs_from).getbbox():
                    break
                self.kt.poll_step(0.5)
            else:
                self.require(step, False, f"the {tag} frame differs from the frame before")
        self.still(app, step, f"the {tag} frame settled")
        return grab(tag)

    def add_effect(self, app, step, clip, typed, cell):
        """The person's way: the event's effects window, “+ effect…”, type, click the cell."""
        self.open_event(app, step, clip)
        self.require(step, app.click_probe("fxreg.add.video"), "click on “+ effect…” (video)")
        for ch in typed:
            app.key("space" if ch == " " else ch)
        self.require(step, app.has_probe(cell, tries=40), f"the gallery shows “{cell}”")
        self.require(step, app.click_probe(cell), f"click on “{cell}”")
        app.wait_frame(2, what="the effect added to the chain", soft=True)

    def side_by_side(self, name, left, right):
        """Two frames, 900 px each, 12 px apart on the window's background; no text: the site
        captions the halves in its own five languages."""
        from PIL import Image

        w, gap = 900, 12
        h = round(left.height * w / left.width)
        out = Image.new("RGB", (2 * w + gap, h), (24, 24, 28))
        for i, img in enumerate((left, right)):
            out.paste(img.resize((w, h), Image.LANCZOS), (i * (w + gap), 0))
        path = os.path.join(self.raw, f"{name}.png")
        out.save(path)
        self.crops[name] = (path, (0, 0, out.width, out.height))

    def effect_before_after(self, name, clip, at, typed, cell):
        app = self.fresh(name)
        try:
            self.seek(app, at)
            self.still(app, name, "the frame without the effect settled")
            before = self.frame_image(app, name, "before")
            self.add_effect(app, name, clip, typed, cell)
            app.key("Escape")
            self.require(name, not app.has_probe("win.fx", tries=4), "the effects window closed")
            # the next Esc clears the selection: the selected event's transform frame is off
            # the picture, as in the frame before
            app.key("Escape")
            self.seek(app, at)
            after = self.frame_image(app, name, "after", differs_from=before)
            self.side_by_side(name, before, after)
            app.step(name, dict(SHOTS)[name])
        finally:
            self.close(app)

    def datamosh(self):
        c = self.tg["cut"]
        # a few frames after the cut: the new shot's motion drags the old shot's picture
        self.effect_before_after("fx-datamosh", c, c["start"] + 21600, "datamosh",
                                 "fxreg.var.datamosh.0")

    def camera_isp(self):
        c = self.tg["night"]
        self.effect_before_after("fx-camera-isp", c, c["start"] + c["duration"] // 2, "camera isp",
                                 "fxreg.var.camera_isp.0")

    def vhs_tape(self):
        """The deck face beside the frame it gives: head A clogged by the person's gesture."""
        from PIL import Image

        step = "fx-vhs-tape"
        c = self.tg["plain"]
        at = c["start"] + c["duration"] // 2
        app = self.fresh(step)
        try:
            self.seek(app, at)
            self.still(app, step, "the frame without the effect settled")
            before = self.frame_image(app, step, "before")
            self.add_effect(app, step, c, "vhs tape", "fxreg.tpl.vhs_tape")
            self.require(step, app.has_probe("deck.head.0.0", tries=40), "the deck face drawn")
            self.require(step, app.click_probe("deck.head.0.0"), "click on head A of the drum")
            self.require(step, app.has_probe("deck.cond.Clogged", tries=20), "the condition row of head A")
            self.require(step, app.click_probe("deck.cond.Clogged"), "click on “Clogged”")
            # the panel first (the window covers part of the preview), then the frame without it
            win = self.probe(app, step, "win.fx", app.has_probe("win.fx"))
            png = self.still(app, step, "the deck face settled")
            box = self.abs((win["x"], win["y"], win["w"], win["h"]))
            with Image.open(png) as im:
                panel = im.convert("RGB").crop((box[0], box[1], box[0] + box[2], box[1] + box[3]))
            app.key("Escape")
            self.require(step, not app.has_probe("win.fx", tries=4), "the effects window closed")
            app.key("Escape")
            self.seek(app, at)
            frame = self.frame_image(app, step, "tape", differs_from=before)
            fw = 960
            fh = round(frame.height * fw / frame.width)
            gap = 12
            h = max(panel.height, fh)
            out = Image.new("RGB", (panel.width + gap + fw, h), (24, 24, 28))
            out.paste(panel, (0, (h - panel.height) // 2))
            out.paste(frame.resize((fw, fh), Image.LANCZOS), (panel.width + gap, (h - fh) // 2))
            path = os.path.join(self.raw, f"{step}.png")
            out.save(path)
            self.crops[step] = (path, (0, 0, out.width, out.height))
            app.step(step, dict(SHOTS)[step])
        finally:
            self.close(app)

    def masks(self):
        """A Duotone limited to a Pen mask drawn around the middle of the frame, feathered."""
        step = "masks"
        c = self.tg["plain"]
        at = c["start"] + c["duration"] // 2
        app = self.fresh(step)
        try:
            self.seek(app, at)
            self.add_effect(app, step, c, "duotone", "fxreg.tpl.duotone")
            self.require(step, app.has_probe("grp.masks", tries=20), "the effect's Masks group")
            app.click_probe("grp.masks", "open the effect's Masks group")
            self.require(step, app.has_probe("masks.tool.pen", tries=20), "the group's mask tools")
            app.scroll_probe_into_view("masks.tool.pen", "grp.masks", "the Pen tool", below="fx.done")
            self.require(step, app.click_probe("masks.tool.pen"), "click on “Pen”")
            self.require(step, app.click_probe("fx.done"), "click on “Done”")
            g = self.kt.geometry()

            def point(fx, fy):
                pic = self.probe(app, step, "mask.picture", app.has_probe("mask.picture"))
                return g["x"] + int(pic["x"] + pic["w"] * fx), g["y"] + int(pic["y"] + pic["h"] * fy)

            # corners by a click, smooth points by a click and a pull
            path = [((0.36, 0.22), None), ((0.60, 0.20), (50, 10)), ((0.68, 0.55), None),
                    ((0.55, 0.82), (-60, 20)), ((0.33, 0.66), None)]
            for n, ((fx, fy), pull) in enumerate(path):
                for _ in range(3):
                    x, y = point(fx, fy)
                    if pull:
                        app.drag(x, y, x + pull[0], y + pull[1])
                    else:
                        app.click(x, y)
                    if app.has_probe(f"mask.pen.{n}", tries=6):
                        break
                self.require(step, app.has_probe(f"mask.pen.{n}"), f"pen point {n + 1} placed")
            self.require(step, app.click_probe("mask.pen.0"), "click on the first point: close")
            self.require(step, app.has_probe("mask.contour.0", tries=12), "the path closed into a mask")
            if app.has_probe("mask.feather", tries=8):
                fe = self.probe(app, step, "mask.feather", True)
                x, y = g["x"] + int(fe["x"] + fe["w"] / 2), g["y"] + int(fe["y"] + fe["h"] / 2)
                app.drag(x, y, x, y - 30)
            fr = self.probe(app, step, "preview.frame", app.has_probe("preview.frame"))
            self.capture(app, step, "masks", fr)
        finally:
            self.close(app)

    def event_window(self, name, clip, picker=False):
        app = self.fresh(name)
        try:
            r = self.open_event(app, name, clip)
            if picker:
                self.require(name, app.click_probe("fxreg.add.video"), "click on “+ effect…” (video)")
                self.still(app, name, "effect picker opened")
                parts = [self.probe(app, name, "win.fx", app.has_probe("win.fx")),
                         self.probe(app, name, "fxreg.popup", app.has_probe("fxreg.popup"))]
                r = self.union(*parts)
                self.keep[name] = [self.abs((q["x"], q["y"], q["w"], q["h"])) for q in parts]
            self.capture(app, name, name, r)
        finally:
            self.close(app)

    def transition(self):
        step = "transitions"
        trans, b = self.tg["transition"]
        app = self.fresh(step)
        try:
            # the joint is b's start; the transition window spans it
            x, y = self.point(app, step, "video", b["lane"], b["start"], frac=0.5)
            self.double_click_until(app, step, x, y, "win.transition")
            r = self.below_header(app, step, "win.transition", app.has_probe("win.transition"))
            self.capture(app, step, "transitions", r)
        finally:
            self.close(app)

    def morpher(self):
        step = "morpher"
        m = self.tg["morph"]
        app = self.fresh(step)
        try:
            x, y = self.point(app, step, "morph", m["lane"], m["start"] + m["duration"] // 2, 0.5)
            self.double_click_until(app, step, x, y, "win.morpher")
            r = self.below_header(app, step, "win.morpher", app.has_probe("win.morpher"))
            self.capture(app, step, "morpher", r)
        finally:
            self.close(app)

    def render(self):
        step = "render-dialog"
        app = self.fresh(step)
        try:
            self.require(step, app.click_probe("menu.file"), "click on “File”")
            self.require(step, app.click_probe("file.render"), "click on “Render…”")
            r = self.probe(app, step, "win.render", app.has_probe("win.render"))
            self.capture(app, step, "render-dialog", r)
        finally:
            self.close(app)

    def perf(self):
        step = "perf-panel"
        app = self.fresh(step)
        try:
            self.require(step, app.click_probe("tb.diag"), "click on the diagnostics button")
            self.probe(app, step, "win.diag", app.has_probe("win.diag"))
            app.key("space")
            # the activity graph is worth showing with a few seconds of
            # playback behind it; a playing app never settles, so no `still`
            self.kt.pause(6.0, "diagnostics graph needs seconds of playback to draw")
            self.require(step, app.playing(), "playback running")
            r = self.below_header(app, step, "win.diag", app.has_probe("win.diag"))
            self.capture(app, step, "perf-panel", r, settle=False)
            app.key("space")
        finally:
            self.close(app)

    def run(self):
        self.overview()
        self.timeline()
        self.reel_parameters()
        self.search()
        self.morpher_before_after()
        self.event_window("slowed-reverb-automation", self.tg["music"])
        self.event_window("video-fx", self.tg["video"], picker=True)
        self.event_window("titles", self.tg["title"])
        self.transition()
        self.morpher()
        self.render()
        self.perf()
        self.datamosh()
        self.camera_isp()
        self.vhs_tape()
        self.masks()
        missing = [n for n, _ in SHOTS if n not in self.crops]
        if missing:
            raise StepError(f"no shot for: {', '.join(missing)}")


def publish(crops, keep, out):
    """Crop and write WebP; the previous set is replaced only when every
    declared shot is in hand (a failed run leaves the old set intact)."""
    from PIL import Image

    staged = out.rstrip("/") + ".new"
    shutil.rmtree(staged, ignore_errors=True)
    os.makedirs(staged)
    for name, _ in SHOTS:
        src, (x, y, w, h) = crops[name]
        with Image.open(src) as im:
            img = im.convert("RGB").crop((x, y, x + w, y + h))
        if name in keep:
            mask = Image.new("L", img.size, 0)
            for kx, ky, kw, kh in keep[name]:
                mask.paste(255, (kx - x, ky - y, kx - x + kw, ky - y + kh))
            img.putalpha(mask)
        img.save(os.path.join(staged, f"{name}.webp"), "WEBP", quality=92, method=6)
    os.makedirs(out, exist_ok=True)
    for name, _ in SHOTS:
        os.replace(os.path.join(staged, f"{name}.webp"), os.path.join(out, f"{name}.webp"))
    shutil.rmtree(staged, ignore_errors=True)


def shoot(sources, images, work, keep_raw=False):
    """The whole pipeline; returns the list of written images."""
    import kt_live

    os.environ["KT_LANG"] = "en"
    # the render dialog's file pick (KT_FILE_PICK seam): a presentable name
    os.environ["KT_FILE_PICK"] = os.path.join(work, "kinetren-showcase.mp4")
    project = build_project(kt_live.KT_CLI, sources, work)
    shooter = Shooter(kt_live, project, targets(project), os.path.join(work, "raw"))
    shooter.run()
    publish(shooter.crops, shooter.keep, images)
    if not keep_raw:
        shutil.rmtree(os.path.join(work, "raw"), ignore_errors=True)
    return [os.path.join(images, f"{n}.webp") for n, _ in SHOTS]


def main(out):
    """live-check entry: fixture media, images into the artifact dir."""
    from kt_live import require_tools

    require_tools()
    os.makedirs(out, exist_ok=True)
    images = os.path.join(out, "images")
    try:
        written = shoot(fixture_sources(os.path.join(out, "sources")), images, out, keep_raw=True)
    except StepError as e:
        print(f"   FAILURE {e}")
        return 1
    from PIL import Image

    bad = []
    for path in written:
        with Image.open(path) as im:
            if im.size[0] < 50 or im.size[1] < 50:
                bad.append(f"{os.path.basename(path)} {im.size}")
    for b in bad:
        print(f"   FAILURE tiny image: {b}")
    print(f"   ok   {len(written) - len(bad)} of {len(SHOTS)} images in {images}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

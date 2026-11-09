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
    ("welcome", "the welcome window: the tour, templates, the recent project with its poster, What's new"),
    ("timeline", "the timeline: a nested reel expanded in place mid-edit, its veiled tails"),
    ("media-bin", "the media bin in free layout with its groups"),
    ("reel-parameters", "the published parameters of a reel instance"),
    ("search", "Ctrl+F: searching the project"),
    ("morpher-before-after", "the same frame without and with the morpher"),
    ("slowed-reverb-automation", "Slowed+Reverb on the music bed with rate/mix curves"),
    ("video-fx", "the effect gallery scrolled to the stylize, camera and tape cells"),
    ("titles", "a title generator in the event window"),
    ("transitions", "the transition editor"),
    ("morpher", "the morpher batch editor"),
    ("render-dialog", "the render dialog"),
    ("perf-panel", "the diagnostics panel during playback"),
    ("fx-datamosh", "the same frame after a cut without and with Datamosh (Cut mosh)"),
    ("fx-camera-isp", "the same frame without and with Camera ISP (Phone at night)"),
    ("fx-vhs-tape", "the VHS Tape deck face with a chipped head, and the frame it gives"),
    ("masks", "a Pen mask drawn on the preview, a Duotone limited to it"),
]

# Shots the pipeline takes whose images wait for the owner's run on the showcase footage (he
# fetches it himself, 2026-10-21): `readme-check.py` lets one be absent while nothing references
# it. Once `readme-shots.py` has written it, reference it in the docs and drop it from here.
AWAITING_FOOTAGE = []

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
    paths = {m["id"]: m.get("path", "") for m in doc.get("media", [])}

    def named(word, cands):
        """The first candidate cut from the footage file whose name has `word`; otherwise the
        middle one (the fixtures have no such names)."""
        return next((c for c in cands if word in paths.get(c["media"], "").lower()),
                    cands[len(cands) // 2])

    # the gallery previews every effect on the event's own frame AS IT IS NOW (under the
    # playhead): a plain cut, or every cell would show the event's look on top of its own; the
    # park of Coffee Run is bright and colourful, so the presets visibly differ (a dark aerial
    # frame made them look alike)
    unstyled = [c for c in clips if c["kind"] == "video" and c["lane"] == 1 and not c.get("fx")
                and c["id"] not in joined and c["duration"] >= 72000]
    if not unstyled:
        raise StepError("showcase project: no plain montage cut without transitions")
    video = named("coffee", sorted(unstyled, key=lambda c: c["start"]))
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
    # other video track at its time, no transition, not a reel or a generator) and with no look
    # of its own — the "before" half is the plain frame
    reels = {q["id"] for q in doc["sequences"]}
    videos = [c for c in clips if c["kind"] == "video"]

    def alone(c):
        end = c["start"] + c["duration"]
        return (c["kind"] == "video" and c["lane"] == 1 and c["duration"] >= 72000
                and c["id"] not in joined and c["media"] not in reels and not c.get("gen")
                and not c.get("fx")
                # its own blurred backdrop (the same clip under a slightly wider frame) is part
                # of its picture; anything else on screen is not
                and not any(o["lane"] != c["lane"] and o["start"] < end
                            and o["start"] + o["duration"] > c["start"]
                            and not (o["media"] == c["media"] and o["start"] == c["start"])
                            for o in videos))

    plain = [c for c in clips if alone(c)]
    if not plain:
        raise StepError("showcase project: no main-track event alone on screen")
    after_cut = [c for c in plain
                 if any(o["lane"] == c["lane"] and o["kind"] == "video"
                        and o["start"] + o["duration"] == c["start"] and o["media"] != c["media"]
                        for o in clips)]
    if not after_cut:
        raise StepError("showcase project: no main-track cut between two sources")
    # a dark clip shows the night phone best (the Charlotte night drone shot of the footage); the
    # night city cut right after another drone shot moshes best: its lights bleed into the old
    # picture. The tape needs a bright picture: a clogged head over a night city is
    # snow and nothing else; the bright cartoon of Coffee Run shows the tape's bleed and dropouts
    # best. The mask is drawn on the longest night shot (the skyline)
    night = named("charlotte", plain)
    longest = sorted(plain, key=lambda c: (-c["duration"], c["start"]))
    # the morpher's frame: the middle of its longest plain target from another source than the
    # one it swaps in (or "before" and "after" show the same film)
    m = doc["morphers"][0]
    swap = m.get("patch", {}).get("media")
    ids = set(m.get("targets", {}).get("ids", []))
    under = [c for c in clips if c["id"] in ids and not c.get("fx") and c["media"] != swap]
    span = (m["start"], m["start"] + m["duration"])
    under = [c for c in under if c["start"] >= span[0] and c["start"] + c["duration"] <= span[1]]
    morph_at = (max(under, key=lambda c: (c["duration"], -c["start"])) if under else None)
    morph_at = (morph_at["start"] + morph_at["duration"] // 2 if morph_at
                else m["start"] + m["duration"] // 2)
    # the lightest stretch to play under Diagnostics: the NASA vertical clip and the square one
    # after it are small files; the start of the edit has the 4K Charge under the intro reel
    # (the fixtures have no such file: the earliest plain cut, so the 6 s of playback with the
    # loop off do not run past the end of their short edit)
    by_start = sorted(unstyled, key=lambda c: c["start"])
    first_plain = next((c for c in by_start if "webb" in paths.get(c["media"], "").lower()),
                       by_start[0])
    return {"reel": reel, "music": music, "video": video, "title": title, "first_plain": first_plain,
            "transition": (trans, by_id[trans["b"]]), "morph": m, "morph_at": morph_at,
            "cut": named("charlotte", sorted(after_cut, key=lambda c: (-c["duration"], c["start"]))),
            "night": night,
            "bright": named("coffee", longest), "mask": named("charlotte", longest)}


class Shooter:
    def __init__(self, kt_live, project, tg, raw, recent_dir=None):
        self.kt = kt_live
        self.project = project
        # where the welcome shot's copy of the project lives: its recent card prints the folder,
        # and a README must not publish a machine's path
        self.recent_dir = recent_dir or os.path.join(os.path.dirname(project), "Videos")
        self.tg = tg
        self.raw = raw
        self.crops = {}  # name -> (full-screen png, (x, y, w, h) on screen)
        self.keep = {}   # name -> rectangles kept opaque inside the crop

    # ---- the app -------------------------------------------------------

    def fresh(self, step):
        print(f"   step “{step}” at {time.time() - self.kt._STARTED:.0f} s", flush=True)
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
        so a zoomed timeline needs a longer quiet period.

        Compared straight from X: an `xwd`, a `convert` and a `compare` per look cost this run
        about a second each, 340 looks in all, and the picture is written once, when it is
        final."""
        self.require(step, app.wait_still(timeout, quiet), what)
        return app.shot("still")

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
        app.until(lambda: any(f["param"] == name and f["y"] >= floor - 2
                              for f in app.geom().get("fields", [])))
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

    def open_event(self, app, step, clip, at=None):
        """`at`: where on the event to click — the click also puts the playhead there."""
        at = clip["start"] + clip["duration"] // 2 if at is None else at
        x, y = self.point(app, step, clip["kind"], clip["lane"], at)
        # the context menu, not a double click: under a heavy decode the
        # UI frame is slow and two clicks fall apart in egui's frame clock.
        # The playhead goes by the ruler: on the event near the view's edge a click would be a jump
        app.click(x, app.ruler_y())
        app.right_click(x, y)
        self.require(step, app.click_probe("tl.fx"), "click on “tl.fx”")
        return self.below_header(app, step, "win.fx", app.has_probe("win.fx"))

    def read_bell(self, app, step):
        """This virtual screen has no GPU, and the program says so once ("software render").
        A person reads it by clicking the bell; unread, it is a "1" in the status strip."""
        self.require(step, app.click_probe("bell"), "click on the bell (read the notice)")
        # Esc closes the window on top of the LAST frame: pressed before the window has been
        # drawn once, it goes down the cascade, and the window stays
        self.require(step, app.until(lambda: int(app.geom().get("windows", 0)) > 0),
                     "the notices window opened")
        app.key("Escape")
        # the dump is written ten times a second: two frames can still show the window, and what
        # counts is that a fresh dump stops showing it
        deadline = time.time() + 30.0
        while int(app.geom().get("windows", 0)) != 0 and time.time() < deadline:
            app.wait_frame(1, what="the notices window closed", soft=True)
        self.require(step, int(app.geom().get("windows", 0)) == 0, "the notices window closed")

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
            # the width read as a consequence, from fresh dumps (test-hygiene): the dump right
            # after the release can be one written before it landed
            wide = lambda: app.geom()["bin"]["right"] - app.geom()["bin"]["left"] > bin_["right"] - bin_["left"] + 50  # noqa: E731
            if app.until(wide, timeout=5.0):
                return
        self.require(step, False, f"the bin panel widened (still {app.geom()['bin']})")

    def overview(self):
        step = "overview"
        app = self.fresh(step)
        try:
            self.widen_bin(app, step)
            self.still(app, step, "screen settled with the wide bin")
            self.read_bell(app, step)
            # warm the passage the way a person scrubs it: the playhead stepped through it on
            # pause. Played cold, 2.6 s after opening, the cut into a new 4K source starves the
            # audio for ~0.5 s on this machine (release and debug alike; a second pass is clean),
            # and the status line would publish those underruns
            cur = app.geom().get("playhead", 0)
            for k in range(1, 7):
                self.seek(app, cur + k * 90000)
                self.still(app, step, f"the passage warmed, step {k}")
            self.seek(app, cur)
            self.still(app, step, "the playhead back on the project's cursor")
            # playback from the project's cursor, a moment before the mid-edit reel instance: the
            # shot waits for the playhead to be well inside it (the stinger over a styled cut)
            app.key("space")
            self.kt.pause(3.0, "meters and preview need a moment of playback to show levels")
            self.require(step, app.playing(), "playback running")
            reel = self.tg["reel"]
            region = json.load(open(self.project))["project"].get("region") or {}
            lo = reel["start"] + min(180000, reel["duration"] * 2 // 5)
            hi = min(reel["start"] + reel["duration"], region.get("end", 1 << 62))
            # ... and for a decoded picture: the "Loading…" scrim over a frame still being
            # composed is published by the program, and the same dump must not have it
            deadline = time.time() + 60.0
            while True:
                g = app.geom()
                loading = any(f["param"] == "preview.loading" for f in g.get("fields", []))
                if lo <= g.get("playhead", 0) < hi and not loading:
                    # the status line shows these: a published hero has none
                    self.require(step, g.get("underruns", 0) == 0 and g.get("callback_underruns", 0) == 0,
                                 f"no underruns before the shot (render {g.get('underruns')}, "
                                 f"callback {g.get('callback_underruns')})")
                    break
                self.require(step, time.time() < deadline,
                             f"the playhead inside the mid-edit reel ({lo}..{hi} ticks) "
                             "with a decoded picture (no “preview.loading”)")
                self.kt.poll_step(0.1)
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
            self.read_bell(app, step)
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
            # the window grows with its results a frame or two after the typing: its rectangle
            # is read once the screen settled, or the crop holds the field alone
            self.still(app, step, "the search results listed")
            r = self.probe(app, step, "win.search", app.has_probe("win.search"))
            self.require(step, r["h"] > 120, f"the search window lists results (height {r['h']})")
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
        at = self.tg["morph_at"]
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
        for i, (img, label) in enumerate(zip(frames, ("Without the morpher", "With the morpher: another source from a worn VHS tape"))):
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
            self.require(name, app.wait_gone("win.fx"), "the effects window closed")
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
        # 20 frames after the cut: the new shot's motion has dragged the old shot's picture far
        # enough to see, and both shots are still recognisable
        # (half the cut when it is shorter: the fixtures' cuts are)
        self.effect_before_after("fx-datamosh", c, c["start"] + min(72000, c["duration"] // 2), "datamosh",
                                 "fxreg.var.datamosh.0")

    def camera_isp(self):
        c = self.tg["night"]
        self.effect_before_after("fx-camera-isp", c, c["start"] + c["duration"] // 2, "camera isp",
                                 "fxreg.var.camera_isp.0")

    def vhs_tape(self):
        """The deck face beside the frame it gives: head A clogged by the person's gesture."""
        from PIL import Image

        step = "fx-vhs-tape"
        c = self.tg["bright"]
        at = c["start"] + c["duration"] // 2
        app = self.fresh(step)
        try:
            self.seek(app, at)
            self.still(app, step, "the frame without the effect settled")
            before = self.frame_image(app, step, "before")
            self.add_effect(app, step, c, "vhs tape", "fxreg.tpl.vhs_tape")
            self.require(step, app.has_probe("deck.head.0.0", tries=40), "the deck face drawn")
            self.require(step, app.click_probe("deck.head.0.0"), "click on head A of the drum")
            # Chipped: the picture stays, with the dropouts and noise of a damaged head. A dirty or
            # clogged head loses the FM carrier altogether — honest snow, but nothing to look at
            self.require(step, app.has_probe("deck.cond.Chipped", tries=20), "the condition row of head A")
            self.require(step, app.click_probe("deck.cond.Chipped"), "click on “Chipped”")
            # the panel first (the window covers part of the preview), then the frame without it
            win = self.probe(app, step, "win.fx", app.has_probe("win.fx"))
            png = self.still(app, step, "the deck face settled")
            box = self.abs((win["x"], win["y"], win["w"], win["h"]))
            with Image.open(png) as im:
                panel = im.convert("RGB").crop((box[0], box[1], box[0] + box[2], box[1] + box[3]))
            app.key("Escape")
            self.require(step, app.wait_gone("win.fx"), "the effects window closed")
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
        c = self.tg["mask"]
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
        saved = self.project
        if picker:
            # a copy with an empty sidecar index: the gallery's base frame is the thumbnail at
            # the playhead, and a missing one is filled from the NEAREST thumbnail already in
            # the index (up to 2 s away, the bin-scrub rule) — after the earlier shots that was
            # the grey storm at the start of the Coffee Run shot, not the park under the playhead
            folder = os.path.join(self.raw, f"{name}-project")
            os.makedirs(folder, exist_ok=True)
            self.project = os.path.join(folder, os.path.basename(saved))
            shutil.copy(saved, self.project)
        try:
            app = self.fresh(name)
        finally:
            self.project = saved
        try:
            r = self.open_event(app, name, clip)
            if picker:
                self.require(name, app.click_probe("fxreg.add.video"), "click on “+ effect…” (video)")
                self.still(app, name, "effect picker opened")
                # the gallery's end: the stylize looks, Camera ISP and VHS Tape, each cell
                # rendered on the event's own frame. A cell's rectangle is published from the
                # laid-out list, below the fold too: "on screen" is its bottom inside the popup
                pop = self.probe(app, name, "fxreg.popup", app.has_probe("fxreg.popup"))
                win = self.kt.geometry()
                px, py = win["x"] + int(pop["x"] + pop["w"] / 2), win["y"] + int(pop["y"] + pop["h"] * 0.6)

                def last_cell_shown():
                    c = app._probe_rect("fxreg.var.vhs_tape.0") if app.has_probe("fxreg.var.vhs_tape.0", tries=2) else None
                    return c is not None and pop["y"] < c["y"] and c["y"] + c["h"] <= pop["y"] + pop["h"]

                for _ in range(40):
                    if last_cell_shown():
                        break
                    app.wheel(px, py, 5)
                    app.wait_frame(2, what="the gallery scrolled", soft=True)
                self.require(name, last_cell_shown(), "the gallery scrolled to the VHS Tape cells")
                # the cells render one by one; the search field keeps its blinking caret, so
                # "settled" ignores a change no wider than the caret
                from PIL import Image, ImageChops
                deadline, calm = time.time() + 180.0, 0
                prev = Image.open(app.shot("cells")).convert("RGB")
                while calm < 3:
                    self.require(name, time.time() < deadline, "the gallery cells rendered")
                    self.kt.poll_step(1.0)
                    cur = Image.open(app.shot("cells")).convert("RGB")
                    box = ImageChops.difference(cur, prev).getbbox()
                    calm = calm + 1 if box is None or box[2] - box[0] <= 4 else 0
                    prev = cur
                parts = [self.probe(app, name, "win.fx", app.has_probe("win.fx")),
                         self.probe(app, name, "fxreg.popup", app.has_probe("fxreg.popup"))]
                r = self.union(*parts)
                self.keep[name] = [self.abs((q["x"], q["y"], q["w"], q["h"])) for q in parts]
            self.capture(app, name, name, r)
        finally:
            self.close(app)

    def welcome(self):
        """The welcome window with the showcase as its recent project: a template closes the
        showcase (its poster is rendered at close), Help → Welcome window… shows the card."""
        step = "welcome"
        os.makedirs(self.recent_dir, exist_ok=True)
        copy = os.path.join(self.recent_dir, os.path.basename(self.project))
        shutil.copy(self.project, copy)  # the media are referenced by absolute paths
        saved = self.project
        self.project = copy
        try:
            app = self.fresh(step)
        finally:
            self.project = saved
        try:
            store = os.path.join(self.raw, step, "store")
            self.require(step, app.click_probe("menu.help"), "click on “Help”")
            self.require(step, app.click_probe("help.welcome"), "click on “Welcome window…”")
            self.require(step, app.has_probe("welcome.window", tries=20), "the welcome window opened")
            self.require(step, app.click_probe("welcome.tpl.1080p25"), "click on the 1080p25 template")
            # the showcase is untouched: no question about unsaved work, though its reel preview
            # has arrived by now (t1413 — the preview used to mark the project edited)
            self.require(step, not app.has_probe("ask.discard", tries=6),
                         "no “save your changes?” over the untouched showcase")
            self.require(step, app.wait_gone("welcome.window"), "the template closed the window")
            posters = os.path.join(store, "posters")
            deadline = time.time() + 180
            while time.time() < deadline and not (os.path.isdir(posters) and os.listdir(posters)):
                self.kt.poll_step(0.5)
            self.require(step, os.path.isdir(posters) and bool(os.listdir(posters)),
                         f"the showcase poster rendered into {posters}")
            self.require(step, app.click_probe("menu.help"), "click on “Help”")
            self.require(step, app.click_probe("help.welcome"), "click on “Welcome window…”")
            self.require(step, app.has_probe("welcome.recent.0", tries=20), "the showcase as a recent card")
            r = self.below_header(app, step, "welcome.window", app.has_probe("welcome.window"))
            self.capture(app, step, "welcome", r)
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
            # a stretch a person plays, not the heaviest one: the region cleared (Esc — with the
            # loop merely off, playback stops at the region's end) and the playhead on the edit's
            # lightest plain cut. Inside the loop the debug build decodes 4K footage under a reel
            # and two effects, and the problem log fills with underruns that describe the build,
            # not the program. Seek first: the open window's graphs redraw on their own, and the
            # screen never settles under them
            app.key("Escape")
            first = self.tg["first_plain"]
            self.seek(app, first["start"])
            self.still(app, step, "the playhead on the lightest plain cut")
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
        # the hero is retaken in a fresh program until its status line is clean: one take in
        # a few has a single callback-underrun frame at the start of playback (a real ~20 µs
        # gap, not a counting error), and a published "0/1" reads like a fault
        for attempt in range(3):
            try:
                self.overview()
                break
            except StepError as e:
                if "no underruns" not in str(e) or attempt == 2:
                    raise
                print(f"   retake of the hero: {e}", flush=True)
        self.welcome()
        self.timeline()
        self.reel_parameters()
        self.search()
        self.morpher_before_after()
        self.event_window("slowed-reverb-automation", self.tg["music"])
        # opened by a click in the event's middle: the playhead lands there, and the cells are
        # drawn on that frame (a ruler click near a cut snaps to it — the event's first frames
        # are the grey start of the shot)
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


def shoot(sources, images, work, keep_raw=False, recent_dir=None):
    """The whole pipeline; returns the list of written images."""
    import kt_live

    os.environ["KT_LANG"] = "en"
    # the render dialog's file pick (KT_FILE_PICK seam): a presentable name
    os.environ["KT_FILE_PICK"] = os.path.join(work, "kinetren-showcase.mp4")
    project = build_project(kt_live.KT_CLI, sources, work)
    shooter = Shooter(kt_live, project, targets(project), os.path.join(work, "raw"), recent_dir)
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

#!/usr/bin/env python3
"""Каркас живых сценариев в виртуальном экране (Р.27).

Сценарий получает каталог для артефактов и работает так:

    from kt_live import Live, project_with_clips

    with Live("handles", out) as app:
        app.drag(...)             # настоящей мышью через XTest
        app.key("ctrl+s")         # настоящей клавиатурой
        p = app.saved_project()   # состояние ПОСЛЕ жеста, разобранное из .otio
        app.expect(len(p.transitions) == 1, "наезд создал переход")
        app.shot("after-overlap") # скриншот в каталог артефактов

Главное отличие от harness-тестов: здесь настоящий бинарь, настоящий X и
настоящий wgpu. Главное отличие от ручной проверки: сценарий сам себя
проверяет и печатает, что именно разошлось.
"""
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
RUST = os.path.join(ROOT, "src")
KT_APP = os.path.join(RUST, "target", "debug", "kt-app")
KT_CLI = os.path.join(RUST, "target", "debug", "kt-cli")

# ui-probe.py — уже готовый водитель мыши/клавиатуры (python3-xlib + XTest);
# имя с дефисом не импортируется обычным import
_spec = importlib.util.spec_from_file_location(
    "ui_probe", os.path.join(RUST, "scripts", "ui-probe.py")
)
probe = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(probe)


class Failure(Exception):
    pass


class Clip:
    def __init__(self, d, rate):
        sr = d.get("source_range") or {}
        self.name = d.get("name", "")
        self.duration = _ticks(sr.get("duration"))
        self.source_in = _ticks(sr.get("start_time"))
        self.kind = d.get("OTIO_SCHEMA", "")

    def __repr__(self):
        return f"Clip({self.name!r}, dur={self.duration}, in={self.source_in})"


class Timeline:
    """Разобранный .otio: то, что реально сохранила программа."""

    def __init__(self, path):
        with open(path) as f:
            self.doc = json.load(f)
        self.clips, self.transitions, self.gaps = [], [], []
        self._walk(self.doc)

    def _walk(self, node):
        if isinstance(node, dict):
            tag = node.get("OTIO_SCHEMA", "")
            if tag.startswith("Clip"):
                self.clips.append(Clip(node, 90000))
            elif tag.startswith("Transition"):
                self.transitions.append(
                    (_ticks(node.get("in_offset")), _ticks(node.get("out_offset")))
                )
            elif tag.startswith("Gap"):
                self.gaps.append(_ticks((node.get("source_range") or {}).get("duration")))
            for v in node.values():
                self._walk(v)
        elif isinstance(node, list):
            for v in node:
                self._walk(v)


def _ticks(t):
    """RationalTime → тики (наш rate 90000; чужой квантуется так же, как в kt-otio)."""
    if not isinstance(t, dict):
        return 0
    rate = t.get("rate") or 90000.0
    return round(t.get("value", 0.0) * 90000.0 / rate)


class Live:
    """Запущенное приложение с проектом. Всё внутри `with`."""

    def __init__(self, name, out_dir, project=None, autoplay=False):
        self.name = name
        self.out = out_dir
        self.project = project
        self.autoplay = autoplay
        self.proc = None
        self.problems = []
        self._shot_no = 0

    def __enter__(self):
        os.makedirs(self.out, exist_ok=True)
        env = dict(os.environ)
        # софт-рендер: в виртуальном экране видеокарты нет
        env.setdefault("WGPU_BACKEND", "gl")
        env.setdefault("LIBGL_ALWAYS_SOFTWARE", "1")
        # программа сама сообщает геометрию таймлайна — целимся точно,
        # а не «примерно в нижнюю треть» (Р.27)
        self.geom_path = os.path.join(self.out, "geom.json")
        env["KT_GEOM_DUMP"] = self.geom_path
        if self.autoplay:
            env["KT_AUTOPLAY"] = "1"
        log = open(os.path.join(self.out, "kt-app.log"), "w")
        args = [KT_APP] + ([self.project] if self.project else [])
        self.proc = subprocess.Popen(args, stdout=log, stderr=log, env=env)
        self._wait_window()
        return self

    def __exit__(self, *exc):
        if self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        return False

    def _wait_window(self, timeout=40.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.proc.poll() is not None:
                raise Failure(f"kt-app упал на старте, см. {self.out}/kt-app.log")
            try:
                probe.window()
                # окно появилось — даём кадрам устояться (софт-рендер медленный)
                time.sleep(2.0)
                return
            except SystemExit:
                time.sleep(0.4)
        raise Failure("окно kt-app не появилось")

    # ---- геометрия из САМОЙ программы --------------------------------------

    def geom(self):
        """Свежий дамп геометрии: зона дорожек, зум, позиции строк."""
        for _ in range(40):
            try:
                with open(self.geom_path) as f:
                    g = json.load(f)
                if g.get("rows"):
                    self._geom = g
                    return g
            except (OSError, ValueError):
                pass
            time.sleep(0.25)
        raise Failure("программа не отдала геометрию (KT_GEOM_DUMP)")

    def x_at(self, tick):
        """Экранный x для тика таймлайна."""
        g = self.geom()
        win = geometry()
        return win["x"] + int(g["timeline"]["left"] + (tick - g["view_start"]) / g["ticks_per_px"])

    def lane_y(self, kind="video", lane=0):
        """Экранный y середины строки дорожки."""
        g = self.geom()
        win = geometry()
        row = next(r for r in g["rows"] if r["kind"] == kind and r["lane"] == lane)
        return win["y"] + int(row["center"])

    def seconds(self, s):
        return round(s * 90000)

    # ---- действия ----------------------------------------------------------

    def click(self, x, y):
        probe.move(x, y)
        probe.button(True)
        probe.button(False)
        time.sleep(0.35)

    def drag(self, x0, y0, x1, y1):
        """Драг КАК ЧЕЛОВЕК: нажать → сдвинуть чуть → вести → отпустить.
        Прыжок сразу в цель egui засчитывает как начало драга уже в конечной
        точке, и объект не двигается (правило из apps/kt-app/CLAUDE.md)."""
        probe.move(x0, y0)
        probe.button(True)
        step = 8 if x1 >= x0 else -8
        probe.move(x0 + step, y0)
        probe.move((x0 + x1) // 2, (y0 + y1) // 2)
        probe.move(x1, y1)
        probe.button(False)
        time.sleep(0.4)

    def key(self, spec):
        probe.key(spec)
        time.sleep(0.4)

    def shot(self, tag):
        self._shot_no += 1
        path = os.path.join(self.out, f"{self._shot_no:02d}-{tag}.png")
        subprocess.run(
            ["import", "-window", "root", "-display", os.environ["DISPLAY"], path],
            check=True,
        )
        return path

    def pixel(self, x, y):
        """Цвет пикселя экрана (r, g, b) — «что человек видит»."""
        img = self.shot(f"px-{x}-{y}")
        out = subprocess.run(
            ["convert", img, "-format", "%[pixel:p{%d,%d}]" % (x, y), "info:-"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        nums = [int(v) for v in out.replace("(", ",").replace(")", ",").split(",") if v.strip().isdigit()]
        os.unlink(img)
        self._shot_no -= 1
        return tuple(nums[:3])

    def saved_project(self):
        """Ctrl+S и разбор того, что программа записала на диск."""
        before = os.path.getmtime(self.project)
        self.key("ctrl+s")
        for _ in range(20):
            if os.path.getmtime(self.project) != before:
                break
            time.sleep(0.25)
        return Timeline(self.project)

    # ---- проверки ----------------------------------------------------------

    def expect(self, ok, what):
        print(f"   {'ok  ' if ok else 'ПРОВАЛ'} {what}")
        if not ok:
            self.problems.append(what)

    def near(self, got, want, tol, what):
        self.expect(abs(got - want) <= tol, f"{what}: {got} ≈ {want} (±{tol})")

    def done(self):
        if self.problems:
            for p in self.problems:
                print(f"   расхождение: {p}", file=sys.stderr)
            return 1
        return 0


def demo_project(dest):
    """Свежий демо-проект программой же (kt-cli demo) — один писатель формата."""
    subprocess.run([KT_CLI, "demo", dest], check=True, capture_output=True)
    return dest


def project_with_clips(dest, clips, fps=30, width=1280, height=720):
    """Минимальный проект: список (start_s, dur_s) видео-клипов одного
    синтетического источника. Пишется через kt-cli new + правку .otio, чтобы
    формат оставался за нашей библиотекой."""
    subprocess.run([KT_CLI, "new", dest], check=True, capture_output=True)
    with open(dest) as f:
        doc = json.load(f)
    doc["metadata"]["kt"]["settings"].update(fps=fps, width=width, height=height)
    track = next(
        t for t in doc["tracks"]["children"] if t.get("kind") == "Video"
    )
    track["children"] = []
    for i, (start_s, dur_s) in enumerate(clips, start=1):
        if start_s > 0 and not track["children"]:
            track["children"].append(_gap(start_s))
        track["children"].append(_clip(i, dur_s))
    doc["metadata"]["kt"]["bin_media"] = []
    with open(dest, "w") as f:
        json.dump(doc, f, indent=4)
    return dest


def _rt(seconds):
    return {"OTIO_SCHEMA": "RationalTime.1", "value": round(seconds * 90000), "rate": 90000.0}


def _range(start_s, dur_s):
    return {"OTIO_SCHEMA": "TimeRange.1", "duration": _rt(dur_s), "start_time": _rt(start_s)}


def _gap(dur_s):
    return {
        "OTIO_SCHEMA": "Gap.1",
        "name": "",
        "source_range": _range(0, dur_s),
        "effects": [],
        "markers": [],
        "enabled": True,
        "metadata": {},
    }


def _clip(idx, dur_s):
    return {
        "OTIO_SCHEMA": "Clip.2",
        "name": f"c{idx}",
        "source_range": _range(0, dur_s),
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
        "metadata": {"kt": {"id": idx, "media": 1}},
    }


def geometry():
    """Границы окна и полезные координаты таймлайна (в координатах экрана)."""
    w = probe.window()
    g = w.get_geometry()
    x, y = probe.origin(w)
    return {"x": x, "y": y, "w": g.width, "h": g.height}


def require_tools():
    for tool in ("import", "convert"):
        if not shutil.which(tool):
            raise Failure(f"нет {tool} (ImageMagick) — скриншоты не снять")

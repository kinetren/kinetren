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

# scripts/live/kt_live.py → src/ на два уровня выше
RUST = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
ROOT = os.path.dirname(RUST)
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
    def __init__(self, d, start):
        sr = d.get("source_range") or {}
        self.name = d.get("name", "")
        self.start = start
        self.duration = _ticks(sr.get("duration"))
        self.source_in = _ticks(sr.get("start_time"))
        self.track = ""

    @property
    def end(self):
        return self.start + self.duration

    def mid(self):
        return self.start + self.duration // 2

    def __repr__(self):
        return (
            f"Clip({self.name!r}, {self.start}..{self.end}, in={self.source_in})"
        )


class Timeline:
    """Разобранный .otio: то, что реально сохранила программа. Позиции
    считаются по порядку детей дорожки (Gap двигает курсор, Transition —
    нет), поэтому сценарий может целиться в СТЫК, а не в число из головы."""

    def __init__(self, path):
        with open(path) as f:
            self.doc = json.load(f)
        self.clips, self.transitions, self.gaps = [], [], []
        for track in self.doc.get("tracks", {}).get("children", []):
            if not str(track.get("OTIO_SCHEMA", "")).startswith("Track"):
                continue
            cursor = 0
            for item in track.get("children", []):
                tag = str(item.get("OTIO_SCHEMA", ""))
                dur = _ticks((item.get("source_range") or {}).get("duration"))
                if tag.startswith("Clip"):
                    c = Clip(item, cursor)
                    c.track = track.get("kind", "")
                    self.clips.append(c)
                    cursor += dur
                elif tag.startswith("Gap"):
                    self.gaps.append((cursor, dur))
                    cursor += dur
                elif tag.startswith("Transition"):
                    self.transitions.append(
                        {
                            "at": cursor,
                            "in": _ticks(item.get("in_offset")),
                            "out": _ticks(item.get("out_offset")),
                        }
                    )

    def clip(self, name):
        return next(c for c in self.clips if c.name == name)

    def joint_after(self, name):
        """Стык за клипом — то место, где живёт кроссфейд."""
        c = self.clip(name)
        return c.start + c.duration


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

    def lane_y(self, kind="video", lane=0, frac=0.5):
        """Экранный y внутри строки дорожки; frac — доля высоты сверху.
        Замеры цвета лучше брать ниже середины: сверху лежит плашка имени."""
        g = self.geom()
        win = geometry()
        row = next(r for r in g["rows"] if r["kind"] == kind and r["lane"] == lane)
        return win["y"] + int(row["top"] + g["lane_h"] * frac)

    def seconds(self, s):
        return round(s * 90000)

    def playhead(self):
        """Позиция курсора в тиках — из САМОЙ программы, а не по картинке."""
        return int(self.geom().get("playhead", 0))

    def playing(self):
        return bool(self.geom().get("playing", False))

    def wait_playhead(self, pred, what, timeout=8.0):
        """Ждёт, пока курсор не удовлетворит условию; возвращает историю."""
        deadline = time.time() + timeout
        seen = []
        while time.time() < deadline:
            t = self.playhead()
            seen.append(t)
            if pred(t):
                return seen
            time.sleep(0.2)
        self.expect(False, f"{what} (курсор шёл: {seen[-8:]})")
        return seen

    def ruler_y(self):
        """Строка времени над дорожками — клик по ней ставит курсор."""
        g = self.geom()
        win = geometry()
        first_row = min(r["top"] for r in g["rows"])
        return win["y"] + int((g["timeline"]["top"] + first_row) * 0.5)

    def preview_center(self):
        """Центр окна превью: над таймлайном, по центру окна."""
        g = self.geom()
        win = geometry()
        return (
            win["x"] + int(g["window"]["w"] * 0.5),
            win["y"] + int(g["timeline"]["top"] * 0.45),
        )

    def brightness_red(self, xy):
        r, g, b = self.pixel(*xy)
        return r > b + 40 and r > g + 40

    def brightness(self, xy):
        r, g, b = self.pixel(*xy)
        return (r + g + b) / 3.0

    # ---- действия ----------------------------------------------------------

    def click(self, x, y):
        probe.move(x, y)
        probe.button(True)
        probe.button(False)
        time.sleep(0.35)

    def drag(self, x0, y0, x1, y1, steps=6):
        """Драг КАК ЧЕЛОВЕК: навести → нажать → сдвинуть чуть → вести
        несколькими шагами → отпустить. Прыжок сразу в цель egui засчитывает
        как начало драга уже в конечной точке, и объект не двигается (правило
        из apps/kt-app/CLAUDE.md). Промежуточные шаги нужны ещё и потому, что
        софт-рендер выдаёт кадры редко: между позициями обязан пройти кадр."""
        probe.move(x0, y0, pause=0.15)
        probe.button(True)
        time.sleep(0.2)
        step = 8 if x1 >= x0 else -8
        probe.move(x0 + step, y0, pause=0.15)
        for i in range(1, steps + 1):
            probe.move(
                x0 + step + (x1 - x0 - step) * i // steps,
                y0 + (y1 - y0) * i // steps,
                pause=0.12,
            )
        probe.button(False)
        time.sleep(0.6)

    def focus(self):
        """Отдать окну фокус ввода: XTest-клавиши идут в АКТИВНОЕ окно, и без
        этого Space/Ctrl+S уходят в пустоту (мышь работает и без фокуса)."""
        from Xlib import X

        w = probe.window()
        try:
            w.set_input_focus(X.RevertToParent, X.CurrentTime)
            w.configure(stack_mode=X.Above)
            probe.d.sync()
        except Exception:
            pass
        time.sleep(0.2)

    def key(self, spec):
        self.focus()
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
            ["convert", img, "-format", f"%[pixel:p{{{x},{y}}}]", "info:-"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        nums = [int(v) for v in out.replace("(", ",").replace(")", ",").split(",") if v.strip().isdigit()]
        os.unlink(img)
        self._shot_no -= 1
        return tuple(nums[:3])

    def project_now(self):
        """Проект как он лежит на диске (без сохранения)."""
        return Timeline(self.project)

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
    """Минимальный проект из видео-клипов: список `(start_s, dur_s)` или
    `(start_s, dur_s, "synth:Pattern")`. Разные источники дают разные цвета
    плашек — это позволяет сценарию отличать, ЧЕЙ материал нарисован.
    Пишется через `kt-cli new` + правку .otio: формат остаётся за нашей
    библиотекой."""
    subprocess.run([KT_CLI, "new", dest], check=True, capture_output=True)
    with open(dest) as f:
        doc = json.load(f)
    doc["metadata"]["kt"]["settings"].update(fps=fps, width=width, height=height)
    # пустые дорожки в файл не пишутся (Р.25), поэтому видео-дорожку
    # заводим сами
    track = next(
        (t for t in doc["tracks"]["children"] if t.get("kind") == "Video"), None
    )
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
    track["children"] = []
    media = {}
    cursor = 0.0
    for i, spec in enumerate(clips, start=1):
        start_s, dur_s = spec[0], spec[1]
        path = spec[2] if len(spec) > 2 else "synth:Ramp"
        mid = media.setdefault(path, len(media) + 1)
        if start_s > cursor + 1e-9:
            track["children"].append(_gap(start_s - cursor))
            cursor = start_s
        track["children"].append(_clip(i, dur_s, path, mid))
        cursor += dur_s
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


def _clip(idx, dur_s, path="synth:Ramp", media_id=1):
    return {
        "OTIO_SCHEMA": "Clip.2",
        "name": f"c{idx}",
        "source_range": _range(0, dur_s),
        "media_references": {
            "DEFAULT_MEDIA": {
                "OTIO_SCHEMA": "ExternalReference.1",
                "name": "",
                "target_url": path,
                "available_range": None,
                "available_image_bounds": None,
                "metadata": {},
            }
        },
        "active_media_reference_key": "DEFAULT_MEDIA",
        "effects": [],
        "markers": [],
        "enabled": True,
        "metadata": {"kt": {"id": idx, "media": media_id}},
    }


def color_media(out_dir, name, rgb, seconds, fps=30, size="640x360"):
    """Настоящий mp4 сплошного цвета: `synth:`-источники в приложении дают
    только звук, а сценариям про превью нужна картинка. Идемпотентно."""
    path = os.path.join(out_dir, f"{name}.mp4")
    if os.path.exists(path):
        return path
    subprocess.run(
        [
            "ffmpeg", "-v", "error", "-y",
            "-f", "lavfi", "-i", f"color=c=0x{rgb}:size={size}:rate={fps}:duration={seconds}",
            "-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo",
            "-shortest",
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-b:a", "96k",
            path,
        ],
        check=True,
    )
    return path


def geometry():
    """Границы окна kt-app в координатах экрана."""
    x, y, w, h = probe.origin(probe.window())
    return {"x": x, "y": y, "w": w, "h": h}


def require_tools():
    for tool in ("import", "convert"):
        if not shutil.which(tool):
            raise Failure(f"нет {tool} (ImageMagick) — скриншоты не снять")

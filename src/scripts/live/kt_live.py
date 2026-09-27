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

# СТЕНД РАБОТНИКА (Р.68): каталог с `ctl`, `geom.json` и `window.json` уже
# запущенной программы — его ставит `scripts/live/worker.py`. Без переменной
# каждый сценарий стартует свой процесс, как до Р.68.
BENCH = os.environ.get("KT_LIVE_BENCH")
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


def pause(seconds, reason):
    """Пауза С ПРИЧИНОЙ — единственный законный способ спать (Р.68).

    Замер Р.68: 96 % времени живого прогона было безусловным сном
    (`nestparam` — 130 с из 136 в 591 вызове), потому что каркас ждал «на
    глазок» кадра софт-рендера. Теперь ждут КАДР (`Live.wait_frame`), а
    пауза остаётся только там, где ждать нечего: временной порог жеста у
    egui, часы автосейва, шаг драга. Причина — словами и не короче сорока
    символов; сторож `scripts/live-sleeps.py` падает на паузе без причины,
    потому что правило прозой здесь уже не удержалось.
    """
    if len(reason) < 40:
        raise Failure(f"пауза без внятной причины: {reason!r}")
    time.sleep(seconds)


def _poll(seconds=0.02):
    """Шаг опроса ожидания: не пауза, а частота вопроса «уже?»."""
    time.sleep(seconds)


class Live:
    """Запущенное приложение с проектом. Всё внутри `with`.

    По умолчанию сценарий берёт СТЕНД работника (Р.68): программа уже
    запущена, и проект открывается в ней подряд за предыдущим — вместо
    холодного старта по 3-5 с на каждый из пятидесяти сценариев. Сценарий,
    который проверяет САМ старт (восстановление после падения, вопрос о
    несохранённом, замок, импорт чужой версии, автоигра с первого кадра),
    объявляет `own_process=True` и получает свой процесс: это решение
    сценария, а не настройка прогона.
    """

    def __init__(self, name, out_dir, project=None, autoplay=False, own_process=False):
        self.name = name
        self.out = out_dir
        self.project = project
        self.autoplay = autoplay
        self.proc = None
        self.problems = []
        self._reported = False
        self._shot_no = 0
        self._raised = False
        # НАБЛЮДАЕМЫЙ шаг кадра: под четырьмя работниками софт-рендер идёт
        # заметно медленнее, чем в одиночку, и все ожидания обязаны
        # растягиваться вместе с ним. Фиксированные пороги — это ровно та
        # слепота, из-за которой «покраснели разом» (§468).
        self._dt = 0.2
        # стенд не умеет ни автоигры (KT_AUTOPLAY читается при старте
        # процесса), ни «программы без проекта» (безымянный документ — это
        # как раз то, что проверяют сценарии старта)
        self.own = bool(own_process or autoplay or project is None or not BENCH)

    def __enter__(self):
        os.makedirs(self.out, exist_ok=True)
        if not self.own:
            return self._attach_bench()
        env = dict(os.environ)
        # софт-рендер: в виртуальном экране видеокарты нет
        env.setdefault("WGPU_BACKEND", "gl")
        env.setdefault("LIBGL_ALWAYS_SOFTWARE", "1")
        # программа сама сообщает геометрию таймлайна — целимся точно,
        # а не «примерно в нижнюю треть» (Р.27)
        self.geom_path = os.path.join(self.out, "geom.json")
        env["KT_GEOM_DUMP"] = self.geom_path
        # свой канал стенда даже у «своего процесса» (Р.68): он снимает
        # троттлинг дампа, и ожидание кадра работает всюду одинаково
        self.ctl_path = os.path.join(self.out, "ctl")
        env["KT_LIVE_CTL"] = self.ctl_path
        # у своего процесса — своё хранилище (Р.68): иначе сценарий видит
        # рабочее место предыдущего. Прогон `nestpreview` был зелёным в
        # одиночку и красным в полном прогоне ровно по этой причине.
        store = os.path.join(self.out, "store")
        os.makedirs(store, exist_ok=True)
        env["KT_USER_STORE_DIR"] = store
        # и СВОЙ XDG_DATA_HOME: по нему живут безымянное восстановление,
        # журнал и замки проектов — без него сценарий находит чужую
        # несохранённую работу в домашнем каталоге разработчика
        env["XDG_DATA_HOME"] = os.path.join(self.out, "xdg")
        if self.autoplay:
            env["KT_AUTOPLAY"] = "1"
        log = open(os.path.join(self.out, "kt-app.log"), "w")
        args = [KT_APP] + ([self.project] if self.project else [])
        self.proc = subprocess.Popen(args, stdout=log, stderr=log, env=env)
        try:
            self._wait_window()
        except BaseException:
            # __exit__ при падении в __enter__ НЕ зовётся: программа
            # осталась бы жить и жрать процессор, отравляя все следующие
            # сценарии (Р.34: забытый kt-app на 500% CPU превратил семь
            # зелёных сценариев в красные — «флак», которого не было)
            self.__exit__(None, None, None)
            raise
        return self

    def _attach_bench(self):
        """Взять СТЕНД работника: открыть свой проект в уже живой программе.

        Чистота не обещается, а получается по устройству: команда `open`
        внутри программы идёт тем же кодом, что `File → Открыть`, то есть
        состояние строится с нуля. Но проверяется она всё равно ДО первого
        жеста — иначе утечка состояния между сценариями дала бы ровно тот
        класс «зелёное, потому что смотрели не туда», которым Kinotrain уже
        болел.
        """
        self.geom_path = os.path.join(BENCH, "geom.json")
        self.ctl_path = os.path.join(BENCH, "ctl")
        # ОКРУЖЕНИЕ, КОТОРОЕ ЧИТАЕТСЯ ПРИ СТАРТЕ ПРОЦЕССА, стенду не
        # передать: программа уже запущена. Молча проигнорировать его
        # нельзя — сценарий проверял бы не то, что думает; поэтому
        # несовпадение красное и называет, что делать (Р.68).
        for var, bench_value in (("KT_AUTOSAVE_SECS", "600"), ("KT_FILE_PICK", None)):
            got = os.environ.get(var)
            if got is not None and got != bench_value:
                raise Failure(
                    f"сценарий ставит {var}={got!r}, а стенд запущен с "
                    f"{bench_value!r}: такому сценарию нужен свой процесс — "
                    "объявите `OWN_PROCESS = True` в его файле"
                )
        # своё хранилище на СЦЕНАРИЙ, а не на работника: рабочее место,
        # пресеты и журнал предыдущего сценария не имеют права влиять на
        # вердикт следующего (`nestpreview`: зелёный в одиночку, красный в
        # полном прогоне). Сценарий, который САМ подготовил хранилище
        # (библиотека пресетов), отдаёт стенду своё.
        store = os.environ.get("KT_USER_STORE_DIR") or os.path.join(self.out, "store")
        os.makedirs(store, exist_ok=True)
        os.makedirs(store, exist_ok=True)
        self._ctl("store", store)
        seq = self._ctl("open", os.path.abspath(self.project))
        self._wait_ctl(seq)
        self.wait_frame(2, what="первый кадр нового проекта в стенде")
        self.settle()
        g = self.geom()
        got = os.path.realpath(g.get("project", ""))
        self.expect(
            got == os.path.realpath(self.project),
            f"стенд открыл проект сценария: {os.path.basename(got)}",
        )
        self.expect(
            int(g.get("windows", 0)) == 0,
            f"программа чиста: открытых окон {g.get('windows')}",
        )
        # окно предыдущего сценария могло остаться низким (`resize`)
        want = json.load(open(os.path.join(BENCH, "window.json")))
        if abs(g["window"]["w"] - want["w"]) > 4 or abs(g["window"]["h"] - want["h"]) > 4:
            self.resize(want["w"], want["h"])
        return self

    def _ctl(self, cmd, arg=""):
        """Команда стенду; возвращает её номер."""
        seq = int(time.time() * 1000)
        with open(self.ctl_path, "w") as f:
            f.write(f"{seq} {cmd} {arg}\n")
        return seq

    def _wait_ctl(self, seq, timeout=30.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            try:
                with open(self.geom_path) as f:
                    if int(json.load(f).get("bench_done", 0)) >= seq:
                        return
            except (OSError, ValueError):
                pass
            _poll(0.05)
        raise Failure(f"стенд не выполнил команду {seq} — программа жива? {self.out}")

    def __exit__(self, *exc):
        if self.own and self.proc and self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        # ВЕРДИКТ НЕ ЗАВИСИТ ОТ ПАМЯТИ АВТОРА СЦЕНАРИЯ (Р.67). `expect`
        # копит расхождения, но код возврата отдаёт `done()` — и семь
        # сценариев из пятидесяти его не звали: `curvesrc` печатал ПРОВАЛ
        # и выходил с НУЛЁМ, то есть красное считалось зелёным ровно тем
        # же способом, что в §443 и §458. Теперь нерассказанное расхождение
        # роняет сценарий отсюда: забыть вердикт больше нельзя.
        if exc[0] is None and self.problems and not self._reported:
            raise Failure(
                "сценарий не доложил о расхождениях (нет `return app.done()`): "
                + "; ".join(self.problems)
            )
        return False

    def _wait_window(self, timeout=40.0):
        """Дождаться окна и НАРИСОВАННЫХ кадров.

        До Р.68 «кадр нарисован» проверялось снимком экрана: xwd плюс
        `convert` на каждую попытку, и сверху ещё 0.5 с про запас — 3-5 с на
        каждый из пятидесяти сценариев. Повод для той проверки был верный
        (Р.33 п.228: чёрное окно сравнивалось с чёрным и «проходило»), но
        спрашивать надо не картинку, а программу: она сообщает НОМЕР
        нарисованного кадра сама (`bench.tick`, дамп геометрии).
        """
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.proc.poll() is not None:
                raise Failure(f"kt-app упал на старте, см. {self.out}/kt-app.log")
            try:
                probe.window()
            except SystemExit:
                _poll(0.1)
                continue
            while time.time() < deadline:
                if self.proc.poll() is not None:
                    raise Failure(f"kt-app упал на старте, см. {self.out}/kt-app.log")
                if self._frame_now() >= 2:
                    # программа ещё доводит первый кадр (источники, тумбы,
                    # раскладка): жест, поданный в этот момент, уезжает в
                    # никуда. Ждём тишины, а не «ещё полсекунды сверху».
                    self.settle()
                    return
                _poll(0.05)
            raise Failure("программа не нарисовала ни одного кадра — окно осталось пустым")
        raise Failure("окно kt-app не появилось")

    # ---- ожидание КАДРА, а не часов (Р.68) ---------------------------------

    def _counters(self):
        """(нарисованные кадры, прочитанные события ввода) из дампа."""
        try:
            with open(self.geom_path) as f:
                g = json.load(f)
            return int(g.get("frame", 0)), int(g.get("input", 0))
        except (OSError, ValueError):
            return 0, 0

    def _frame_now(self):
        """Номер нарисованного кадра из дампа; 0 — дампа ещё нет."""
        return self._counters()[0]

    def _input_now(self):
        return self._counters()[1]

    def seen(self, what="жест"):
        """Вернуть метку «сколько событий ввода программа уже прочитала».

        Кадра мало: кадр мог НАЧАТЬСЯ раньше, чем событие доехало от
        X-сервера, и «кадр нарисован» ещё не значит «жест увиден» — именно
        на этом живой сценарий `level` молча терял драг. Метка берётся ДО
        жеста, `wait_seen` ждёт её роста."""
        return self._input_now()

    def _note_frame_gap(self, gap):
        """Запомнить, сколько сейчас идёт кадр (сглаженно, с потолком)."""
        if 0.0 < gap < 5.0:
            self._dt = max(0.05, min(2.0, 0.5 * self._dt + 0.5 * gap))

    def settle(self, timeout=None, quiet=None):
        """Дождаться, пока программа ДОРЕАГИРУЕТ на жест.

        «Прочитала ввод» — ещё не «закончила»: на отпускании драга модель
        правится батчем, а кадр с новым состоянием идёт следующим. Ждать
        фиксированную паузу (0.6 с в прежнем каркасе) незачем — программа
        сама говорит, когда успокоилась: кадры перестают идти, потому что
        перерисовка бывает ТОЛЬКО при движении (инвариант №12).
        """
        timeout = timeout if timeout is not None else max(10.0, 40.0 * self._dt)
        deadline = time.time() + timeout
        last, since = self._frame_now(), time.time()
        while time.time() < deadline:
            f = self._frame_now()
            if f != last:
                self._note_frame_gap(time.time() - since)
                last, since = f, time.time()
            else:
                # «тихо» — это НЕСКОЛЬКО шагов кадра, а не 0.12 с: под
                # нагрузкой кадр идёт секунду, и фиксированный порог
                # объявлял бы программу успокоившейся посреди отрисовки
                need = quiet if quiet is not None else max(0.12, 2.5 * self._dt)
                if time.time() - since >= need:
                    return True
            _poll()
        return False

    def wait_seen(self, mark, what="жест", timeout=None, frames=1, soft=False):
        """Ждать, пока программа ПРОЧИТАЕТ события жеста и нарисует кадр."""
        timeout = timeout if timeout is not None else max(25.0, 60.0 * self._dt)
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self._input_now() > mark:
                return self.wait_frame(frames, what=what, soft=True)
            _poll()
        if soft:
            return False
        raise Failure(f"программа не прочитала ввод: {what}")

    def wait_frame(self, n=1, timeout=None, what="кадр после жеста", soft=False):
        """Ждать, пока программа нарисует ещё `n` кадров.

        Это замена фиксированным паузам: жест всегда рождает ввод, ввод
        рождает перерисовку, и ждать надо именно её. Под нагрузкой ожидание
        растягивается вместе с кадром — чего пауза «0.35 с» не умела, и
        отсюда росли ложные покраснения (§468).

        `soft=True` — там, где кадра может и не быть вовсе: программа
        перерисовывается ТОЛЬКО при движении (инвариант №12), и ожидание
        «просто так» законно ничем не кончается.
        """
        target = self._frame_now() + n
        # ПОЛ ожиданий щедрый (Р.68): под работниками кадр идёт втрое
        # дольше, а лишние секунды тратятся только там, где и правда что-то
        # сломалось — красное всё равно перегоняется в одиночку.
        timeout = timeout if timeout is not None else max(20.0, 30.0 * self._dt * n)
        started = time.time()
        deadline = started + timeout
        while time.time() < deadline:
            if self._frame_now() >= target:
                self._note_frame_gap((time.time() - started) / max(n, 1))
                return True
            _poll()
        if soft:
            return False
        raise Failure(f"программа не нарисовала кадр: {what}")

    # ---- геометрия из САМОЙ программы --------------------------------------

    def resize(self, w, h):
        """Изменить размер окна программы (Р.66): контракт формы проверяется
        в НИЗКОМ окне — там, где содержимое перерастает экран."""
        win = probe.window()
        win.configure(width=int(w), height=int(h))
        probe.d.sync()
        self._raised = False
        # НАБЛЮДАЕМЫЙ шаг кадра: под четырьмя работниками софт-рендер идёт
        # заметно медленнее, чем в одиночку, и все ожидания обязаны
        # растягиваться вместе с ним. Фиксированные пороги — это ровно та
        # слепота, из-за которой «покраснели разом» (§468).
        self._dt = 0.2
        # после ресайза ждём НЕСКОЛЬКО кадров: перевёрстка идёт не в один
        # проход (прокрутки и группы пересчитывают свои высоты)
        self.wait_frame(3, what="перевёрстка после смены размера окна")

    def geom(self):
        """Свежий дамп геометрии: зона дорожек, зум, позиции строк."""
        deadline = time.time() + 10.0
        while time.time() < deadline:
            try:
                with open(self.geom_path) as f:
                    g = json.load(f)
                if g.get("rows"):
                    self._geom = g
                    return g
            except (OSError, ValueError):
                pass
            _poll(0.05)
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
        # высота у КАЖДОЙ строки своя: служебные (morph/camera) втрое
        # ниже дорожек клипов (Р.35 п.1)
        return win["y"] + int(row["top"] + row.get("h", g["lane_h"]) * frac)

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
            _poll(0.05)
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

    def _mods(self, names, pressed):
        """Зажать/отпустить модификаторы вокруг жеста мыши: у egui они
        читаются из СОСТОЯНИЯ ввода кадра, а не из события кнопки."""
        from Xlib import X, XK

        table = {"ctrl": "Control_L", "shift": "Shift_L", "alt": "Alt_L"}
        mark = self.seen()
        for n in names:
            code = probe.d.keysym_to_keycode(XK.string_to_keysym(table[n]))
            probe.xtest.fake_input(
                probe.d, X.KeyPress if pressed else X.KeyRelease, code
            )
        probe.d.sync()
        # МОДИФИКАТОР ОБЯЗАН БЫТЬ ПРОЧИТАН ДО ЖЕСТА: egui берёт состояние
        # модификаторов из кадра, и клик, попавший в кадр раньше нажатия
        # Ctrl, приезжает обычным (Ctrl+клик в `nestshare` так и терял
        # второе событие из выделения).
        self.wait_seen(mark, "модификатор прочитан", frames=1, soft=True)

    def click(self, x, y, mods=()):
        # окно поднимается ОДИН раз за сценарий (Р.68): мышь работает и без
        # фокуса, а поднятие стоило 0.2 с на каждый клик. Повод для прежнего
        # «перед каждым жестом» был в том, что окно ПРЕДЫДУЩЕГО сценария
        # могло остаться поверх (п.199) — со стендом окно одно, и поднять
        # его достаточно единожды (после ресайза флаг сбрасывается).
        self.raise_once()
        if mods:
            self._mods(mods, True)
        mark = self.seen()
        probe.move(x, y, pause=0.0)
        probe.button(True, pause=0.01)
        probe.button(False, pause=0.01)
        self.wait_seen(mark, f"клик ({x}, {y})", frames=2)
        self.settle()
        if mods:
            self._mods(mods, False)

    def right_click(self, x, y):
        """ПКМ: цель контекст-меню и само меню. Отдельный метод, потому что
        `click` жмёт кнопку 1, а меню открывает третья."""
        self.raise_once()
        mark = self.seen()
        probe.move(x, y, pause=0.0)
        probe.button(True, 3, pause=0.01)
        probe.button(False, 3, pause=0.01)
        # меню появляется НЕ в том же кадре, что нажатие: egui открывает его
        # на следующем проходе
        self.wait_seen(mark, f"контекст-меню ({x}, {y})", frames=2)
        self.settle()

    def double_click(self, x, y):
        """Двойной клик: у egui это ВРЕМЕННОЙ жест (два клика ближе 0.3 с),
        поэтому между ними нельзя ни поднимать окно, ни спать по 0.35 с —
        два обычных click() подряд складываются в два одиночных.

        Зазор между кликами МАЛЕНЬКИЙ и это не описка (Р.34): в
        софт-рендере кадр идёт ~0.2 с, а время клика egui берёт из кадра,
        в который тот попал. Пауза 0.1 с превращается для egui в 0.3+ с —
        и двойной клик рассыпается на два одиночных. Проверено: 0.04 с
        открывает окно в 5 прогонах из 5, 0.10 с — в 0 из 5."""
        self.raise_once()
        # кадр ПОСЛЕ наведения: первый клик по свежей позиции иначе
        # теряется вместе с кадром ввода
        mark = self.seen()
        probe.move(x, y, pause=0.0)
        self.wait_seen(mark, "наведение перед двойным кликом", soft=True)
        pause(
            0.2,
            "время клика egui берёт из КАДРА, в который тот попал: свежей позиции "
            "нужен свой кадр, иначе первый клик двойного теряется вместе с ним",
        )
        for i in range(2):
            if i:
                # МЕЖДУ кликами нужен зазор: четыре события подряд в
                # софт-рендере попадают в ОДИН кадр ввода, и egui
                # засчитывает один клик вместо двух. Кадром здесь ждать
                # НЕЛЬЗЯ: у двойного клика egui временной порог (0.3 с), а
                # кадр в софт-рендере идёт ~0.2 с — ожидание кадра
                # превратило бы жест в два одиночных.
                pause(
                    0.04,
                    "временной порог двойного клика egui: между кликами нужен зазор "
                    "меньше 0.3 с, а кадр в софт-рендере идёт ~0.2 с (проверено 5 из 5)",
                )
            probe.button(True, pause=0.01)
            probe.button(False, pause=0.01)
        self.wait_frame(2, what="кадр после двойного клика")
        self.settle()

    def drag(self, x0, y0, x1, y1, steps=6, mods=()):
        """Драг КАК ЧЕЛОВЕК: навести → нажать → сдвинуть чуть → вести
        несколькими шагами → отпустить. Прыжок сразу в цель egui засчитывает
        как начало драга уже в конечной точке, и объект не двигается (правило
        из apps/kt-app/CLAUDE.md). Промежуточные шаги нужны ещё и потому, что
        софт-рендер выдаёт кадры редко: между позициями обязан пройти кадр.

        «Между позициями обязан пройти КАДР» — это и есть условие ожидания
        (Р.68): раньше здесь стояли паузы 0.12-0.35 с «на всякий случай», и
        именно они составляли большую часть цены живого прогона. Ждём кадр —
        и тогда жест не «недоезжает» даже под нагрузкой (п.199)."""
        self.raise_once()
        if mods:
            self._mods(mods, True)
        mark = self.seen()
        probe.move(x0, y0, pause=0.0)
        self.wait_seen(mark, "указатель в начале драга")
        mark = self.seen()
        probe.button(True, pause=0.01)
        self.wait_seen(mark, "нажатие кнопки (начало драга)", frames=2)
        pause(
            0.2,
            "egui засчитывает драг по порогу И по кадру: нажатие обязано пережить "
            "целый кадр до первого сдвига, иначе жест начинается уже в цели",
        )
        step = 8 if x1 >= x0 else -8
        mark = self.seen()
        probe.move(x0 + step, y0, pause=0.0)
        self.wait_seen(mark, "первый сдвиг драга", frames=2)
        for i in range(1, steps + 1):
            mark = self.seen()
            probe.move(
                x0 + step + (x1 - x0 - step) * i // steps,
                y0 + (y1 - y0) * i // steps,
                pause=0.0,
            )
            # ЖДАТЬ ОБЯЗАТЕЛЬНО, и не «мягко»: непрочитанные движения
            # X-сервер и winit складывают в одно, драг превращается в
            # прыжок сразу в цель, и egui начинает его уже в конечной точке
            # — объект не двигается вовсе (правило из apps/kt-app/CLAUDE.md)
            self.wait_seen(mark, f"шаг драга {i}/{steps}")
        # последняя позиция указателя обязана быть ПРОЧИТАНА до отпускания,
        # иначе жест «недоезжал» на 10-20 % пути (п.199, Р.33)
        mark = self.seen()
        probe.button(False, pause=0.01)
        self.wait_seen(mark, "отпускание драга", frames=2)
        self.settle()
        if mods:
            self._mods(mods, False)

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
        self._raised = True
        self.wait_frame(1, what="кадр после поднятия окна", soft=True)

    def raise_once(self):
        """Поднять окно, если этого ещё не делали (Р.68)."""
        if not self._raised:
            self.focus()

    def key(self, spec):
        # клавиатуре фокус НУЖЕН (в отличие от мыши), но поднимать окно
        # достаточно один раз за сценарий
        self.raise_once()
        mark = self.seen()
        probe.key(spec, pause=0.01, after=0.01)
        self.wait_seen(mark, f"клавиша {spec}", frames=2)
        self.settle()

    # имена клавиш X для символов, которых не набрать буквой: `key()`
    # разбирает строку по «+», поэтому знаки обязаны ехать именами
    # XTest шлёт КЛАВИШУ, а не символ: знак, который на клавиатуре живёт
    # через Shift, обязан ехать с ним (иначе `*` приезжает восьмёркой, а
    # `+` — знаком равенства; поймано живым прогоном Р.63)
    KEYSYM = {
        "+": "shift+plus",
        "*": "shift+asterisk",
        "(": "shift+parenleft",
        ")": "shift+parenright",
        "_": "shift+underscore",
        ":": "shift+colon",
        "-": "minus",
        "/": "slash",
        ".": "period",
        ",": "comma",
        " ": "space",
    }

    def type_text(self, text):
        """Набрать строку ЦЕЛИКОМ, как человек (Р.63/Р.68).

        По символу с ожиданием кадра на каждый — это и была самая дорогая
        мелочь живого прогона: 0.65 с на КАЖДЫЙ знак (поднять окно, клавиша,
        пауза). egui читает события ввода кадром, и вся строка законно
        приезжает в один кадр — ждём один раз в конце.
        """
        self.raise_once()
        mark = self.seen()
        for ch in text:
            probe.key(self.KEYSYM.get(ch, ch), pause=0.005, after=0.005)
        self.wait_seen(mark, f"набранная строка {text!r}", frames=2)
        self.settle()

    def close_window(self):
        """Крестик оконного менеджера. Программа его перехватывает, чтобы
        спросить про несохранённую работу (Р.49, п.354)."""
        self.raise_once()
        probe.close_window()
        self.wait_frame(2, what="кадр после крестика окна")

    def alive(self):
        if self.own:
            return self.proc is not None and self.proc.poll() is None
        # в стенде процесс не наш: живость видно по тому, рисует ли
        # программа кадры
        return self.wait_frame(1, timeout=3.0, soft=True) or self._frame_now() > 0

    def hover(self, x, y, pause=0.0):
        """Навести указатель и дать окну кадр на реакцию.

        `pause` остаётся в подписи ради вызовов сценариев, но ждём мы кадр:
        реакция на наведение (подсказка, подменю) приходит кадром, а не по
        часам."""
        self.raise_once()
        mark = self.seen()
        probe.move(x, y, pause=0.0)
        self.wait_seen(mark, f"наведение на ({x}, {y})", frames=2, soft=True)

    def has_probe(self, name, tries=12):
        """Виджет с таким именем показан прямо сейчас."""
        for _ in range(tries):
            if any(f["param"] == name for f in self.geom().get("fields", [])):
                return True
            self.wait_frame(1, what=f"кадр в ожидании виджета {name}", soft=True)
        return False

    def live_clips(self):
        """События ЖИВОЙ модели (без сохранения). Нужно там, где проверяется
        сторож несохранённого: Ctrl+S проверил бы запись, а не сторож."""
        return sorted((c["start"], c["dur"]) for c in self.geom().get("clips", []))

    def _probe_rect(self, name):
        """Прямоугольник виджета, когда картинка УСТОЯЛАСЬ (Р.68).

        Дамп пишется каждым кадром, и первый кадр с попапом застаёт его в
        середине анимации: клик по такому прямоугольнику уходил РЯДОМ с
        пунктом. Прежний каркас прятал это фиксированной паузой; ждём
        вместо неё, пока программа перестанет перерисовывать, и читаем
        прямоугольник заново.
        """
        for _ in range(20):
            f = next((f for f in self.geom().get("fields", []) if f["param"] == name), None)
            if f:
                self.settle()
                again = next(
                    (x for x in self.geom().get("fields", []) if x["param"] == name), None
                )
                return again or f
            self.wait_frame(1, what=f"кадр в ожидании виджета {name}", soft=True)
        return None

    def double_click_probe(self, name, what=None):
        """Двойной клик по виджету из дампа геометрии."""
        f = self._probe_rect(name)
        if f:
            win = geometry()
            self.double_click(
                win["x"] + int(f["x"] + f["w"] / 2),
                win["y"] + int(f["y"] + f["h"] / 2),
            )
            return True
        self.expect(False, what or f"программа отдала прямоугольник «{name}»")
        return False

    def right_click_probe(self, name, what=None):
        """Правый клик по виджету из дампа геометрии. Нужен подписям полей:
        публикация свойства композиции живёт ровно там (Р.59, стадия 3)."""
        f = self._probe_rect(name)
        if f:
            win = geometry()
            self.right_click(
                win["x"] + int(f["x"] + f["w"] / 2),
                win["y"] + int(f["y"] + f["h"] / 2),
            )
            return True
        self.expect(False, what or f"программа отдала прямоугольник «{name}»")
        return False

    def hover_probe(self, name, what=None):
        """Навести на виджет по его прямоугольнику из дампа. Подменю egui
        раскрывается НАВЕДЕНИЕМ, а клик по нему закрывает всё меню."""
        f = self._probe_rect(name)
        if f:
            win = geometry()
            self.hover(
                win["x"] + int(f["x"] + f["w"] / 2),
                win["y"] + int(f["y"] + f["h"] / 2),
            )
            # подменю раскрывается НАВЕДЕНИЕМ — ждём, пока оно доедет
            self.settle()
            return True
        self.expect(False, what or f"программа отдала прямоугольник «{name}»")
        return False

    def click_probe(self, name, what=None):
        """Кликнуть виджет, чей прямоугольник программа отдала в дампе
        геометрии (`fields`). Модальные окна egui в дереве доступности не
        ищутся, а угадывать смещения внутри окна сценарий не должен (Р.27)."""
        f = self._probe_rect(name)
        if f:
            win = geometry()
            self.click(
                win["x"] + int(f["x"] + f["w"] / 2),
                win["y"] + int(f["y"] + f["h"] / 2),
            )
            return True
        self.expect(False, what or f"программа отдала прямоугольник «{name}»")
        return False

    def shot(self, tag):
        """Снимок ЭКРАНА через xwd.

        `import -window root` (ImageMagick) на окне eframe отдаёт чёрный
        кадр: GL-окно живёт в 32-битном визуале, и XGetImage корневого
        окна глубины 24 содержимого ребёнка не видит. Симптом коварный —
        сценарий не падает, а сравнивает чёрное с чёрным и «проходит».
        xwd читает то же корневое окно правильно (Р.33 п.228)."""
        self._shot_no += 1
        path = os.path.join(self.out, f"{self._shot_no:02d}-{tag}.png")
        xwd = subprocess.run(
            ["xwd", "-root", "-display", os.environ["DISPLAY"]],
            check=True,
            capture_output=True,
        ).stdout
        subprocess.run(["convert", "xwd:-", path], input=xwd, check=True)
        return path

    def wheel(self, x, y, ticks=3):
        """Колесо мыши над точкой (ticks > 0 — вниз). XTest: кнопки 4/5."""
        self.raise_once()
        probe.move(x, y, pause=0.0)
        btn = 5 if ticks > 0 else 4
        mark = self.seen()
        for _ in range(abs(ticks)):
            probe.button(True, btn, pause=0.005)
            probe.button(False, btn, pause=0.005)
        self.wait_seen(mark, f"колесо над ({x}, {y})", frames=2)
        self.settle()

    def scroll_probe_into_view(self, name, anchor, what=None, tries=12, below=None):
        """Крутить колесо над виджетом `anchor`, пока прямоугольник `name` не
        окажется в видимой части (низ ниже кромки — вниз, верх выше —
        вверх). Кромка — `below`: виджет ВНЕ прокрутки у низа окна (кнопка
        действия, Р.66); без него — низ окна программы. Окно egui с
        телом-прокруткой иначе прячет блок за сгибом, а щуп отдаёт
        координаты и невидимого: клик по ним уходил на таймлайн."""
        win = geometry()
        for _ in range(tries):
            f = {x["param"]: x for x in self.geom().get("fields", [])}
            r = f.get(name)
            a = f.get(anchor)
            if r is None or a is None:
                self.wait_frame(1, what=f"кадр в ожидании виджетов {name}/{anchor}", soft=True)
                continue
            h = self.geom()["window"]["h"]
            if below and below in f:
                h = min(h, f[below]["y"] - 2)
            if r["y"] >= 0 and r["y"] + r["h"] <= h:
                return True
            ticks = 3 if r["y"] + r["h"] > h else -3
            # якорь мог уехать за кромку — колесо крутится в ВИДИМОЙ точке
            ay = min(max(a["y"] + a["h"] / 2, 4), h - 4)
            self.wheel(win["x"] + int(a["x"] + a["w"] / 2), win["y"] + int(ay), ticks)
        self.expect(False, what or f"«{name}» не удалось прокрутить в окно")
        return False

    def step(self, name, what=None):
        """ШАГ ПУТИ ПОЛЬЗОВАТЕЛЯ (Р.67): скриншот на каждый шаг `ПУТЬ` из
        спеки, файл `NN-step-<имя>.png` в каталоге сценария; `finish.sh`
        печатает их список, отчёт агента их несёт. Прогон — на наших
        фикстурах, не на проектах владельца («п.1 но не на моих проектах»)."""
        path = self.shot(f"step-{name}")
        print(f"   шаг {self._shot_no:02d}: {what or name} → {os.path.basename(path)}")
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

    def ink_box(self, xy, half=60, step=6):
        """Самый яркий пиксель КВАДРАТА вокруг точки — один снимок экрана.

        Есть ли в этом месте изображение хоть что-нибудь светлое. Точечный
        `brightness` для букв титра не годится: кегль 7 % кадра даёт тонкий
        штрих, и попасть в него центром превью — лотерея, а тест-лотерея
        не тест (правило «утверждать следствие»).
        """
        img = self.shot(f"ink-{xy[0]}-{xy[1]}")
        x0, y0 = xy[0] - half, xy[1] - half
        n = 2 * half // step
        out = subprocess.run(
            [
                "convert",
                img,
                "-crop",
                f"{2 * half}x{2 * half}+{x0}+{y0}",
                "+repage",
                "-resize",
                f"{n}x{n}!",
                "-format",
                "%[fx:maxima*255]",
                "info:-",
            ],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        os.unlink(img)
        self._shot_no -= 1
        try:
            return float(out.strip())
        except ValueError:
            raise Failure(f"не удалось прочитать яркость области: {out!r}")

    def pixel_settled(self, xy, differs_from=None, tries=20):
        """Цвет пикселя ПОСЛЕ того, как картинка устоялась.

        `pixel` читает экран НЕМЕДЛЕННО, а после подмотки курсор двигается
        раньше, чем приедет декодированный кадр: превью законно ДЕРЖИТ
        последний кадр (инвариант «промах к дедлайну → последний кадр»),
        и чтение возвращает картинку ПРЕДЫДУЩЕЙ позиции. Под нагрузкой
        это воспроизводится каждый раз — так `loop` «видел» синий клип
        слева от региона, а слабое ожидание «пока два чтения совпадут»
        просто устаивалось на устаревшем кадре.

        Поэтому ждём СМЕНЫ: `differs_from` — цвет, который был на экране
        ДО жеста. Дождавшись смены, добираем ещё одно совпадение подряд —
        чтобы не поймать промежуточное состояние. Без `differs_from`
        (кадр может и не измениться) достаточно устойчивости.

        Р.68: ожидание «устоялось» тоже привязано к кадрам программы —
        `settle` секунд сна заменены ожиданием пары кадров, а между
        чтениями пикселя ждётся кадр (мягко: картинка могла и замереть)."""
        self.wait_frame(2, what="кадры перед чтением устоявшегося пикселя", soft=True)
        prev = None
        for _ in range(tries):
            px = self.pixel(*xy)
            fresh = differs_from is None or px != tuple(differs_from)
            # ЧЁРНОЕ — не кадр, а «композит ещё не полон»: движок штатно
            # отдаёт пустой слой, пока кадр не приехал (инвариант «промах к
            # дедлайну → последний кадр/тишина»). Не отсеять его — и
            # ожидание СМЕНЫ радостно устоится на этой черноте, объявив
            # «слева от региона чёрный клип».
            if fresh and px == prev and sum(px) > 24:
                return px
            prev = px
            self.wait_frame(1, what="кадр между чтениями пикселя", soft=True)
        return prev or (0, 0, 0)

    def screen_diff(self, before):
        """Доля изменившегося на экране относительно снимка `before`
        (0..1). Надёжнее редкой сетки точек: окно могло открыться там,
        где сетка его не задела, — так проверка «окно появилось» ломалась
        на ровном месте (Р.34)."""
        after = self.shot("diff")
        out = subprocess.run(
            ["compare", "-metric", "AE", "-fuzz", "3%", before, after, "null:"],
            capture_output=True,
            text=True,
        ).stderr.strip()
        os.unlink(after)
        self._shot_no -= 1
        # ImageMagick печатает «12345» или «1.2345e+07» — второе ломало
        # наивную выборку цифр и давало «изменилось 514%»
        try:
            px = float(out.split()[0]) if out else 0.0
        except ValueError:
            px = 0.0
        g = geometry()
        return min(px / float(max(g["w"] * g["h"], 1)), 1.0)

    def pixel_of(self, image, x, y):
        """Цвет пикселя РАНЕЕ снятого скриншота — сравнение «до и после»
        без повторного снимка экрана."""
        out = subprocess.run(
            ["convert", image, "-format", f"%[pixel:p{{{x},{y}}}]", "info:-"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        nums = [
            int(v)
            for v in out.replace("(", ",").replace(")", ",").split(",")
            if v.strip().isdigit()
        ]
        return tuple(nums[:3])

    def project_now(self):
        """Проект как он лежит на диске (без сохранения)."""
        return Timeline(self.project)

    def save_now(self):
        """Ctrl+S и ожидание ЗАПИСИ, а не «полторы секунды на всякий случай».

        Программа пишет проект атомарной заменой файла — значит ждать надо
        смену mtime, и ровно столько, сколько она заняла (Р.68).
        """
        before = os.path.getmtime(self.project)
        self.key("ctrl+s")
        deadline = time.time() + max(40.0, 90.0 * self._dt)
        while time.time() < deadline:
            if os.path.getmtime(self.project) != before:
                return True
            # ПИСАТЬ БЫЛО НЕЧЕГО — это не отказ: программа не переписывает
            # файл, в котором уже лежит текущая редакция (звёздочки нет,
            # `dirty` в дампе — false). Требовать записи всегда значило бы
            # объявлять дефектом ровно то поведение, которое мы просили.
            if not self.geom().get("dirty", False):
                return False
            _poll(0.05)
        raise Failure("программа не записала проект после Ctrl+S, хотя правки есть")

    def saved_project(self):
        """Ctrl+S и разбор того, что программа записала на диск."""
        before = os.path.getmtime(self.project)
        self.key("ctrl+s")
        deadline = time.time() + 10.0
        while time.time() < deadline:
            if os.path.getmtime(self.project) != before:
                break
            _poll(0.05)
        return Timeline(self.project)

    # ---- проверки ----------------------------------------------------------

    def expect(self, ok, what):
        print(f"   {'ok  ' if ok else 'ПРОВАЛ'} {what}")
        if not ok:
            self.problems.append(what)

    def near(self, got, want, tol, what):
        self.expect(abs(got - want) <= tol, f"{what}: {got} ≈ {want} (±{tol})")

    def done(self):
        self._reported = True
        if self.problems:
            for p in self.problems:
                print(f"   расхождение: {p}", file=sys.stderr)
            return 1
        return 0


def demo_project(dest):
    """Свежий демо-проект программой же (kt-cli demo) — один писатель формата.

    `--force`: сценарии перезапускаются по месту, а kt-cli с Р.49 (п.359) не
    затирает существующий файл молча. Медиа демо с того же раунда —
    синтетические, так что сценарию нечего доставать со стороны."""
    subprocess.run([KT_CLI, "demo", dest, "--force"], check=True, capture_output=True)
    return dest


def project_with_clips(dest, clips, fps=30, width=1280, height=720):
    """Минимальный проект из видео-клипов: список `(start_s, dur_s)` или
    `(start_s, dur_s, "synth:Pattern")`. Разные источники дают разные цвета
    плашек — это позволяет сценарию отличать, ЧЕЙ материал нарисован.
    Пишется через `kt-cli new` + правку .otio: формат остаётся за нашей
    библиотекой."""
    # --force: сценарии перезапускаются по месту, а kt-cli с Р.49
    # не затирает существующий файл молча (п.359)
    subprocess.run([KT_CLI, "new", dest, "--force"], check=True, capture_output=True)
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


def pattern_media(out_dir, name, seconds, fps=30, size="640x360"):
    """Настоящий mp4 со СТРУКТУРОЙ (цветные полосы SMPTE). Сплошная заливка
    (`color_media`) годится для «есть ли картинка», но не для «сдвинулась
    ли она»: у однотонного кадра любой перенос и любой зум дают тот же
    пиксель. Идемпотентно."""
    path = os.path.join(out_dir, f"{name}.mp4")
    if os.path.exists(path):
        return path
    subprocess.run(
        [
            "ffmpeg", "-v", "error", "-y",
            "-f", "lavfi", "-i", f"smptebars=size={size}:rate={fps}:duration={seconds}",
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
    for tool in ("xwd", "convert"):
        if not shutil.which(tool):
            raise Failure(f"нет {tool} (x11-apps/ImageMagick) — скриншоты не снять")

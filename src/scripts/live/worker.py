#!/usr/bin/env python3
"""РАБОТНИК живого прогона (Р.68): свой экран, свой стенд, свой список сценариев.

Прежде живая проверка была строго последовательной: 50 сценариев, 1012 с, и
каждый платил 3-5 с за холодный старт kt-app. Запрет на параллель («живые
сценарии в два потока не гоняются») появился в Р.59 §468 после того, как
второй прогон под нагрузкой ~14 покрасил зелёные сценарии; владелец снял его
в Р.68 вместе с механизмом, который закрывает причину: у каждого работника
СВОЙ виртуальный экран, СВОЁ пользовательское хранилище и урезанный
софт-рендер, а красный сценарий перегоняется один в одиночку (это делает
live-check.sh).

Работник:
  * поднимает Xvfb и openbox на своём номере экрана;
  * держит ОДИН kt-app (стенд) и открывает в нём проекты сценариев подряд;
  * сценарию, который проверяет САМ старт программы, стенд не даёт: такой
    сценарий объявляет `OWN_PROCESS = True` у себя в файле и запускается со
    остановленным стендом — иначе на экране оказались бы два окна kinotrain,
    и `probe.window()` целился бы в чужое.

    usage: worker.py <номер экрана> <каталог> <сценарий> [<сценарий> ...]

Вердикты пишутся в <каталог>/verdicts/<сценарий> («ok» или «fail»), вывод
сценария — в общий поток работника.
"""
import json
import os
import re
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
RUST = os.path.abspath(os.path.join(HERE, "..", ".."))
KT_APP = os.path.join(RUST, "target", "debug", "kt-app")
KT_CLI = os.path.join(RUST, "target", "debug", "kt-cli")
SCREEN = os.environ.get("KT_LIVE_SCREEN", "1920x1080x24")
# сколько потоков софт-рендера даётся ОДНОМУ kt-app: без ограничения один
# llvmpipe занимает около семи ядер, и работники отбирали ядра друг у друга —
# ровно та нагрузка, из-за которой запрет на параллель и появился
LP_THREADS = os.environ.get("KT_LIVE_LP_THREADS", "4")

OWN = re.compile(r"^OWN_PROCESS\s*=\s*True", re.M)


def wants_own_process(name):
    """Сценарий объявил, что ему нужен свой процесс?"""
    src = open(os.path.join(HERE, f"{name}.py"), encoding="utf-8").read()
    return bool(OWN.search(src))


class Screen:
    """Свой Xvfb и свой openbox. Оконный менеджер нужен для разворачивания
    окна (правило «проверять в РАЗВЁРНУТОМ окне», р.13) и для жестов с
    кромками."""

    def __init__(self, display, out):
        self.display = display
        self.out = out
        self.procs = []

    def free_display(self):
        """Снять ЗАМОК мёртвого X-сервера на своём номере экрана.

        Прерванный прогон оставляет `/tmp/.X<n>-lock`, и следующий Xvfb
        отказывается стартовать («Server is already active for display N»).
        Программа тогда подключается в никуда и падает внутри winit, а
        выглядит это как «сценарий сломался». Замок снимается ТОЛЬКО если
        за ним нет живого сервера.
        """
        n = self.display.lstrip(":")
        alive = subprocess.run(
            ["pgrep", "-f", f"Xvfb {self.display}"], capture_output=True, text=True
        ).stdout.split()
        if alive:
            return
        for path in (f"/tmp/.X{n}-lock", f"/tmp/.X11-unix/X{n}"):
            try:
                os.unlink(path)
            except OSError:
                pass

    def __enter__(self):
        self.free_display()
        # лог — СВОЙ у каждого экрана: общий файл работники затирали друг
        # другу, и причина отказа X-сервера пропадала
        logs = open(os.path.join(self.out, f"xvfb{self.display.lstrip(':')}.log"), "w")
        self.procs.append(
            subprocess.Popen(
                ["Xvfb", self.display, "-screen", "0", SCREEN, "-nolisten", "tcp"],
                stdout=logs,
                stderr=logs,
            )
        )
        os.environ["DISPLAY"] = self.display
        for _ in range(80):
            try:
                subprocess.run(
                    [
                        sys.executable,
                        "-c",
                        "import Xlib.display, os; Xlib.display.Display(os.environ['DISPLAY'])",
                    ],
                    check=True,
                    capture_output=True,
                )
                break
            except subprocess.CalledProcessError:
                time.sleep(0.25)
        else:
            raise SystemExit(
                f"X-сервер на {self.display} не поднялся, см. "
                f"{self.out}/xvfb{self.display.lstrip(':')}.log"
            )
        if shutil.which("openbox"):
            wm = open(os.path.join(self.out, f"openbox{self.display.lstrip(':')}.log"), "w")
            self.procs.append(subprocess.Popen(["openbox"], stdout=wm, stderr=wm))
            time.sleep(0.5)
        return self

    def __exit__(self, *exc):
        for p in self.procs:
            p.terminate()
        for p in self.procs:
            try:
                p.wait(timeout=5)
            except subprocess.TimeoutExpired:
                p.kill()
        return False


class Bench:
    """Один kt-app работника: в нём сценарии открывают свои проекты подряд."""

    def __init__(self, out, display):
        # каталог стенда — СВОЙ У КАЖДОГО РАБОТНИКА: общий `out` один на
        # прогон, и общий `bench/` означал общий канал команд и общий дамп
        # геометрии — работники открывали проекты друг другу и читали чужие
        # кадры. Это и выглядело как «параллель всё красит».
        self.dir = os.path.join(out, "bench" + display.replace(":", ""))
        os.makedirs(self.dir, exist_ok=True)
        self.ctl = os.path.join(self.dir, "ctl")
        self.geom = os.path.join(self.dir, "geom.json")
        self.store = os.path.join(self.dir, "store")
        self.blank = os.path.join(self.dir, "blank.otio")
        self.proc = None

    def env(self):
        e = dict(os.environ)
        e.setdefault("WGPU_BACKEND", "gl")
        e.setdefault("LIBGL_ALWAYS_SOFTWARE", "1")
        e["LP_NUM_THREADS"] = LP_THREADS
        e["KT_GEOM_DUMP"] = self.geom
        e["KT_LIVE_CTL"] = self.ctl
        e["KT_USER_STORE_DIR"] = self.store
        # XDG_DATA_HOME — ТОЖЕ свой: по нему программа ищет безымянное
        # восстановление, журнал и замки проектов. Без него прогон лезет в
        # домашний каталог разработчика и находит там ЧУЖУЮ несохранённую
        # работу — сценарий «несохранённое» получал модальное окно ещё до
        # первого жеста и молча проверял не то.
        e["XDG_DATA_HOME"] = os.path.join(self.dir, "xdg")
        # периодический автосейв в стенде выключен: сценарии, которым он
        # нужен по секундам (проверка восстановления), берут свой процесс
        e["KT_AUTOSAVE_SECS"] = "600"
        return e

    def kill_stale(self):
        """Убить ЧУЖУЮ программу, оставшуюся в этом каталоге стенда.

        Прерванный прогон (Ctrl+C, убитый родитель) оставляет kt-app живым, и
        он продолжает писать СВОЙ дамп геометрии в тот же файл: следующий
        прогон читает кадры и проект чужого процесса и краснеет непонятно
        почему. Каталог стенда — собственность работника, поэтому чистим его
        перед запуском.
        """
        try:
            out = subprocess.run(
                ["pgrep", "-f", self.dir], capture_output=True, text=True
            ).stdout.split()
        except OSError:
            return
        me = str(os.getpid())
        for pid in out:
            if pid == me:
                continue
            try:
                os.kill(int(pid), 15)
            except (ProcessLookupError, PermissionError, ValueError):
                pass

    def ensure(self):
        """Стенд жив? Иначе поднять (первый раз или после падения)."""
        if self.proc is not None and self.proc.poll() is None:
            return
        self.kill_stale()
        os.makedirs(self.store, exist_ok=True)
        subprocess.run([KT_CLI, "new", self.blank, "--force"], check=True, capture_output=True)
        for p in (self.ctl, self.geom):
            if os.path.exists(p):
                os.unlink(p)
        log = open(os.path.join(self.dir, "kt-app.log"), "a")
        self.proc = subprocess.Popen(
            [KT_APP, self.blank], stdout=log, stderr=log, env=self.env()
        )
        # ждём НАРИСОВАННЫХ кадров: программа сообщает их номер сама
        deadline = time.time() + 60
        while time.time() < deadline:
            if self.proc.poll() is not None:
                raise SystemExit(f"стенд не поднялся, см. {self.dir}/kt-app.log")
            try:
                with open(self.geom) as f:
                    if int(json.load(f).get("frame", 0)) >= 2:
                        break
            except (OSError, ValueError):
                pass
            time.sleep(0.05)
        else:
            raise SystemExit("стенд не нарисовал ни одного кадра")
        # размер окна стенда — эталон для сценариев: один из них мог
        # оставить окно низким (`resize` проверяет контракт формы)
        with open(self.geom) as f:
            g = json.load(f)
        with open(os.path.join(self.dir, "window.json"), "w") as f:
            json.dump(g["window"], f)

    def stop(self):
        if self.proc is None:
            return
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        self.proc = None


def main():
    display, out = sys.argv[1], os.path.abspath(sys.argv[2])
    names = sys.argv[3:]
    os.makedirs(out, exist_ok=True)
    verdicts = os.path.join(out, "verdicts")
    os.makedirs(verdicts, exist_ok=True)
    fail = 0
    with Screen(display, out):
        bench = Bench(out, display)
        try:
            for name in names:
                own = wants_own_process(name)
                scen_out = os.path.join(out, name)
                os.makedirs(scen_out, exist_ok=True)
                # ХРАНИЛИЩЕ СЦЕНАРИЯ — ЧИСТОЕ НА КАЖДЫЙ ПРОГОН (Р.68):
                # упавший прогон оставляет в нём файл восстановления и
                # рабочее место, и ПЕРЕГОН в одиночку начинается с чужого
                # состояния — сценарий, зелёный сам по себе, краснел второй
                # раз подряд, и механизм «красное только дважды» переставал
                # отличать флак от дефекта.
                for leftover in ("store", "xdg"):
                    shutil.rmtree(os.path.join(scen_out, leftover), ignore_errors=True)
                env = dict(os.environ)
                if own:
                    # два окна kinotrain на одном экране — это прицел в чужое
                    bench.stop()
                    env.pop("KT_LIVE_BENCH", None)
                    # хранилище — своё и у сценария со своим процессом:
                    # рабочее место соседа не имеет права менять вердикт
                    env["KT_USER_STORE_DIR"] = os.path.join(scen_out, "store")
                    env["XDG_DATA_HOME"] = os.path.join(scen_out, "xdg")
                else:
                    bench.ensure()
                    env["KT_LIVE_BENCH"] = bench.dir
                    env["KT_AUTOSAVE_SECS"] = "600"
                print(f"== {name} ==", flush=True)
                rc = subprocess.run(
                    [sys.executable, os.path.join(HERE, f"{name}.py"), scen_out], env=env
                ).returncode
                ok = rc == 0
                with open(os.path.join(verdicts, name), "w") as f:
                    f.write("ok" if ok else "fail")
                if ok:
                    print("   ok", flush=True)
                else:
                    print(f"   ПРОВАЛ (скриншоты: {scen_out})", flush=True)
                    fail = 1
        finally:
            bench.stop()
    return fail


if __name__ == "__main__":
    sys.exit(main())

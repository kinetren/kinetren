#!/usr/bin/env python3
"""Живой сценарий Р.49 А (пп.351, 353): работа переживает УБИЙСТВО программы.

Проверяется ровно то, ради чего вся волна: правка → SIGKILL (ни `on_exit`, ни
`Drop` не зовутся — ядро просто гасит процесс) → перезапуск → программа сама
предлагает восстановить → работа на месте.

И то, что решил владелец: **в файл проекта пишет только Ctrl+S.** Пока
пользователь не сохранился, его .otio обязан остаться нетронутым — даже после
того, как фоновая запись отработала.

Второй круг проверяет правило 14: восстановление работает ПОВТОРНО, а не один
раз, и не съедает свой источник.
"""
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Failure, Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def recovery_of(project):
    return project + ".recovery.otio"


def wait_file(path, timeout=30.0, what="файл"):
    deadline = time.time() + timeout
    while time.time() < deadline:
        if os.path.exists(path) and os.path.getsize(path) > 0:
            return True
        time.sleep(0.25)
    raise Failure(f"{what} не появился за {timeout} с: {path}")


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "recover.otio"),
        [(0.0, 2.0, "synth:Ramp"), (3.0, 2.0, "synth:Ramp")],
    )
    before = open(proj).read()
    rec = recovery_of(proj)
    if os.path.exists(rec):
        os.remove(rec)

    # фоновая запись раз в 2 секунды: ждать полминуты в каждом прогоне
    # нечестно по отношению к проверке
    os.environ["KT_AUTOSAVE_SECS"] = "2"

    problems = []
    with Live("recover", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 2, f"старт: два события, {p.clips}")

        # правка настоящим жестом: тянем второе событие вправо
        src = p.clips[1]
        y = app.lane_y("video", 0)
        x0 = app.x_at(src.mid())
        app.drag(x0, y, x0 + 160.0, y)
        app.shot("moved")

        wait_file(rec, what="файл восстановления")
        app.expect(
            open(proj).read() == before,
            "файл проекта НЕ тронут фоновой записью (его меняет только Ctrl+S)",
        )

        # SIGKILL: ни on_exit, ни Drop не успевают — как при падении
        app.proc.kill()
        app.proc.wait(timeout=10)
        problems = app.problems

    time.sleep(0.5)
    if not os.path.exists(rec):
        raise Failure("после убийства файла восстановления нет — работа потеряна")
    snapshot = open(rec).read()

    moved_start = None
    for round_no in ("первый", "второй"):
        # одинаковая стартовая обстановка на оба круга: проект как был,
        # восстановление — тот самый снимок и заведомо СВЕЖЕЕ проекта
        open(proj, "w").write(before)
        open(rec, "w").write(snapshot)
        old_t = time.time() - 600
        os.utime(proj, (old_t, old_t))

        with Live(f"recover-{round_no}", out, project=proj) as app:
            app.shot(f"restart-{round_no}")
            # окно восстановления открылось само — жмём «восстановить»
            if not app.click_probe("ask.restore", f"{round_no} круг: работа предложена"):
                raise Failure("окна восстановления нет — работа не предложена")
            app.shot(f"restored-{round_no}")
            app.expect(
                os.path.exists(rec),
                f"{round_no} круг: восстановление не съело свой источник",
            )

            tl = app.saved_project()
            got = [c.start for c in tl.clips]
            if moved_start is None:
                moved_start = max(got)
            app.expect(
                max(got) == moved_start and moved_start > 270000,
                f"{round_no} круг: восстановленная позиция на месте ({got})",
            )
            problems += app.problems

    if problems:
        for p in problems:
            print(f"   расхождение: {p}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/recover"))

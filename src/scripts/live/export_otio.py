#!/usr/bin/env python3
"""Живой сценарий Р.70: экспорт в OpenTimelineIO — НАЗВАННОЕ действие, не сохранение.

Решение владельца: «мы уходим от otio-формата, оставляем otio только на
экспорт». Проект живёт в своём формате `.kt`; файл обмена делает отдельный
пункт меню, и после него проект на диске и звёздочка в заголовке — как были.

ПУТЬ (из спеки ui-reach, сценарий «Экспорт в OTIO из меню»):
1. Файл → подменю «Экспорт» → пункт «OpenTimelineIO…» → системный диалог
   (в Xvfb его нет — ответ подменяет `KT_FILE_PICK`, сам пункт нажимается);
2. файл обмена появился на диске: это OTIO со всеми событиями монтажа;
3. файл проекта `.kt` байт-в-байт прежний, звёздочка как была, в
   уведомлениях — «экспортировано: <путь>».
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, Timeline, demo_project, poll_step, require_tools  # noqa: E402

# СВОЙ ПРОЦЕСС (Р.68): ответ системного диалога подменяет `KT_FILE_PICK`, а это
# окружение ПРОЦЕССА — стенд работника запущен без него.
OWN_PROCESS = True


def main(out):
    require_tools()
    # демо — файл обмена сценария; программе его импортирует Live (.kt рядом)
    proj = demo_project(os.path.join(out, "export_otio.otio"))
    dst = os.path.join(out, "exported.otio")
    if os.path.exists(dst):
        os.remove(dst)
    os.environ["KT_AUTOSAVE_SECS"] = "600"
    # системного диалога в виртуальном экране нет (xdg-portal): пункт меню
    # нажимается по-настоящему, ответ диалога подменяет KT_FILE_PICK
    os.environ["KT_FILE_PICK"] = dst
    with Live("export_otio", out, project=proj) as app:
        app.settle()
        before = open(app.project, "rb").read()
        dirty0 = bool(app.geom().get("dirty"))
        n_clips = len(app.live_clips())
        app.expect(n_clips > 0, f"на монтаже есть события: {n_clips}")

        # ШАГ 1: Файл → Экспорт → OpenTimelineIO…
        app.click_probe("menu.file", "меню «Файл»")
        app.step("file-menu", "меню «Файл» с подменю «Экспорт»")
        app.click_probe("file.export", "подменю «Экспорт»")
        app.step("export-submenu", "подменю «Экспорт» с пунктом «OpenTimelineIO…»")
        app.click_probe("file.export_otio", "пункт «OpenTimelineIO…»")

        # ШАГ 2: файл обмена на диске (диалог подменён KT_FILE_PICK)
        ok = False
        for _ in range(40):
            if os.path.exists(dst) and os.path.getsize(dst) > 0:
                ok = True
                break
            poll_step(0.25)
        app.expect(ok, "файл обмена появился на диске")
        app.settle()
        app.step("exported", "уведомление «экспортировано», монтаж на месте")
        if ok:
            with open(dst) as f:
                doc = json.load(f)
            app.expect(doc.get("OTIO_SCHEMA") == "Timeline.1", "экспорт — OpenTimelineIO Timeline.1")
            app.expect(
                len(Timeline(dst).clips) == n_clips,
                f"в экспорте все события монтажа: {len(Timeline(dst).clips)} против {n_clips}",
            )

        # ШАГ 3: проект и звёздочка не тронуты — экспорт не сохранение
        app.expect(open(app.project, "rb").read() == before, "файл проекта экспортом не тронут")
        app.expect(bool(app.geom().get("dirty")) == dirty0, "звёздочка как была: экспорт — не сохранение")
        notes = app.geom().get("notices", [])
        app.expect(
            any(dst in n for n in notes),
            f"уведомление называет, куда записан файл обмена: {notes}",
        )
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

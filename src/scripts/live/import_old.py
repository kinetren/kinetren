#!/usr/bin/env python3
"""Живой сценарий Р.67 (п.1): проект СТАРОЙ версии — через свою дверь.

Владелец: «проект 26.09.26/untitled2 не открывается - с чем это связано?»
Файл v5 (18.09) несёт у морферов `targets: null`, а ридер знает список и
объект. Его решение: «делай пункт в меню File для импорта старых версий, и
именно в импорте чини». Здесь настоящая программа:

ПУТЬ (из спеки ui-reach, сценарий «Старый файл через импорт»):
1. запускается со старым файлом → «Открыть» отвергает его, в уведомлении
   версия и имя пункта импорта, событий на таймлайне нет;
2. Файл → «Импортировать проект старой версии…» → системный диалог
   (в Xvfb его нет — ответ подменяет `KT_FILE_PICK`, сам пункт нажимается);
3. выбран тот же файл → таймлайн с событием и морфером, документ помечен
   правками (звёздочка), уведомление о починке.

Фикстура — копия СТРУКТУРЫ untitled2 (v5, null-цели), не сам файл
владельца.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, poll_step, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401

# СВОЙ ПРОЦЕСС (Р.68): сценарий проверяет САМ СТАРТ программы, а не работу в
# уже открытом проекте, — стенд работника ему не подходит.
OWN_PROCESS = True

SEC = 90000


def old_v5_project(dest):
    new_otio(dest)
    doc = json.load(open(dest))
    doc["metadata"]["kt"]["settings"].update(fps=30, width=1280, height=720)
    track = next((t for t in doc["tracks"]["children"] if t.get("kind") == "Video"), None)
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
    rt = lambda s: {"OTIO_SCHEMA": "RationalTime.1", "value": round(s * SEC), "rate": 90000.0}
    track["children"] = [
        {
            "OTIO_SCHEMA": "Clip.2",
            "name": "old",
            "source_range": {"OTIO_SCHEMA": "TimeRange.1", "start_time": rt(0), "duration": rt(4)},
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
            "metadata": {"kt": {"id": 1, "media": 1}},
        }
    ]
    kt = doc["metadata"]["kt"]
    kt["v"] = 5
    kt["bin_media"] = [{"id": 1, "path": "synth:Ramp"}]
    kt["morph_lanes"] = 1
    kt["morphers"] = [
        {
            "id": 1,
            "lane": 0,
            "start": 0,
            "duration": 2 * SEC,
            "targets": None,
            "patch": {"video_fx": [{"kind": "mirror", "horizontal": True, "vertical": False}]},
        }
    ]
    with open(dest, "w") as f:
        json.dump(doc, f, indent=4)
    return dest


def main(out):
    require_tools()
    old = old_v5_project(os.path.join(out, "old_v5.otio"))
    os.environ["KT_AUTOSAVE_SECS"] = "600"
    # системного диалога в виртуальном экране нет (xdg-portal): пункт меню
    # нажимается по-настоящему, ответ диалога подменяет KT_FILE_PICK
    os.environ["KT_FILE_PICK"] = old
    with Live("import_old", out, project=old, raw=True) as app:
        # ШАГ 1: «Открыть» отверг — событий нет, в статусе версия и дверь
        app.settle()
        g = app.geom()
        app.expect(len(g.get("clips", [])) == 0, "старый файл обычным путём не открылся")
        notes = g.get("notices", [])
        hit = [n for n in notes if "5" in n and ("Import older" in n or "старой версии" in n)]
        app.expect(bool(hit), f"уведомление называет версию и дверь: {notes}")
        app.step("open-refused", "старый файл отвергнут с адресом двери")

        # ШАГ 2: Файл → Импортировать проект старой версии…
        app.click_probe("menu.file", "меню «Файл»")
        app.step("file-menu", "меню «Файл» с пунктом импорта")
        app.click_probe("file.import_older", "пункт «Импортировать проект старой версии…»")
        # ШАГ 3: выбор файла (в Xvfb — ответ диалога подменён KT_FILE_PICK)
        app.settle()
        ok = False
        for _ in range(20):
            g = app.geom()
            if len(g.get("clips", [])) == 1 and g.get("morphers") == 1:
                ok = True
                break
            poll_step(0.5)
        app.expect(ok, "после импорта на таймлайне событие и морфер")
        app.expect(bool(g.get("dirty")), "документ помечен правками (звёздочка)")
        app.step("imported", "проект открыт через импорт, звёздочка в заголовке")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

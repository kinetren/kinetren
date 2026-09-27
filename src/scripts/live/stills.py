#!/usr/bin/env python3
"""Живой сценарий Р.67 (п.13): СТАТИЧНАЯ КАРТИНКА в настоящем окне.

Владелец дословно: «статичные картинки не отображаюся, вообще.. там вечный
Loading... ЭТО ПИЗДЕЦ. мало того что не работает - так ты еще об этом
узнаешь только в момент просмотра».

ПУТЬ (спека media-index, «Картинка читается, планируется, показывается»):
1. проект с событием 20 с на картинке → курсор в начало → в превью сразу
   картинка, без «Loading…»;
2. курсор к концу события (19.5 с) → картинка на месте: у неё нет конца
   материала;
3. картинка УБИРАЕТСЯ с диска, проект открывается снова → в кадре тёмная
   заглушка С ИМЕНЕМ файла, а не вечное «Loading…».
"""
import json
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, require_tools  # noqa: E402

SEC = 90000


def still_project(dest, img, secs=20.0):
    subprocess.run([KT_CLI, "new", dest, "--force"], check=True, capture_output=True)
    doc = json.load(open(dest))
    doc["metadata"]["kt"]["settings"].update(fps=30, width=1280, height=720)
    track = next((t for t in doc["tracks"]["children"] if t.get("kind") == "Video"), None)
    if track is None:
        track = {
            "OTIO_SCHEMA": "Track.1", "name": "", "kind": "Video", "children": [],
            "source_range": None, "effects": [], "markers": [], "enabled": True, "metadata": {},
        }
        doc["tracks"]["children"].append(track)
    rt = lambda s: {"OTIO_SCHEMA": "RationalTime.1", "value": round(s * SEC), "rate": 90000.0}
    track["children"] = [
        {
            "OTIO_SCHEMA": "Clip.2",
            "name": "still",
            "source_range": {"OTIO_SCHEMA": "TimeRange.1", "start_time": rt(0), "duration": rt(secs)},
            "media_references": {
                "DEFAULT_MEDIA": {
                    "OTIO_SCHEMA": "ExternalReference.1", "name": "", "target_url": img,
                    "available_range": None, "available_image_bounds": None, "metadata": {},
                }
            },
            "active_media_reference_key": "DEFAULT_MEDIA",
            "effects": [], "markers": [], "enabled": True,
            "metadata": {"kt": {"id": 1, "media": 1}},
        }
    ]
    doc["metadata"]["kt"]["bin_media"] = [{"id": 1, "path": img}]
    with open(dest, "w") as f:
        json.dump(doc, f, indent=4)
    return dest


def looks_like_picture(app, box):
    """Разброс яркостей в области превью, НОРМИРОВАННЫЙ 0..1 (так его
    отдаёт ImageMagick): пустой кадр даёт ~0.0, картинка — десятые."""
    shot = app.shot("probe")
    out = subprocess.run(
        ["convert", shot, "-crop", f"{box[2]}x{box[3]}+{box[0]}+{box[1]}", "+repage",
         "-colorspace", "Gray", "-format", "%[fx:standard_deviation]", "info:-"],
        capture_output=True, text=True, check=True,
    ).stdout.strip()
    return float(out)


def main(out):
    require_tools()
    # картинка — фикстурой программы же (детерминированный генератор)
    img = os.path.join(out, "logo.jpg")
    subprocess.run(
        ["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i",
         "testsrc=size=640x360:rate=1:duration=1", "-frames:v", "1", "-q:v", "3", img],
        check=True,
    )
    path = still_project(os.path.join(out, "stills.otio"), img)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("stills", out, project=path) as app:
        y = app.lane_y("video", 0, frac=0.5)
        app.click(app.x_at(1 * SEC), y)
        time.sleep(1.0)
        cx, cy = app.preview_center()
        box = (cx - 200, cy - 110, 400, 220)

        # 1) картинка видна СРАЗУ
        sd = looks_like_picture(app, box)
        app.expect(sd > 0.15, f"в превью картинка, а не пустой кадр (разброс {sd:.2f})")
        app.step("still-start", "картинка в превью в начале события")

        # 2) у конца события — та же картинка (конца материала у неё нет)
        app.click(app.x_at(int(19.5 * SEC)), y)
        time.sleep(1.2)
        sd_end = looks_like_picture(app, box)
        app.expect(sd_end > 0.15, f"на 19.5 с картинка на месте (разброс {sd_end:.2f})")
        app.step("still-end", "картинка на месте у конца двадцатисекундного события")

    # 3) картинка пропала — заглушка с именем, а не «Loading…»
    os.remove(img)
    with Live("stills-gone", out, project=path) as app:
        y = app.lane_y("video", 0, frac=0.5)
        app.click(app.x_at(1 * SEC), y)
        time.sleep(1.5)
        g = app.geom()
        notes = " ".join(g.get("notices", []))
        app.expect("logo.jpg" in notes, f"пропавший файл назван человеку: {notes[:120]}")
        cx, cy = app.preview_center()
        box = (cx - 200, cy - 110, 400, 220)
        sd = looks_like_picture(app, box)
        app.expect(sd > 0.02, f"в кадре заглушка с именем файла, а не пустота (разброс {sd:.2f})")
        app.step("still-gone", "пропавшая картинка: заглушка с именем файла")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

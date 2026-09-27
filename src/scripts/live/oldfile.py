#!/usr/bin/env python3
"""Живой сценарий Р.63 (пп.1-2): прежний файл открывается, хвост честен.

Две претензии владельца в одном окне, и обе были зелены в харнессе:

1. «мой проект untitled перестал открываться в новой версии» — кривые
   СКОРОСТИ записаны прежним полем `ease: true`, читатель Р.61 разворачивал
   их в «плавно», а валидатор скорости принимает только прямую. Здесь
   настоящая программа открывает файл той же формы и играет его.
2. «регион-loop явно в конце ломает звук, там тишина берётся» — событие
   длиннее материала: за концом файла тишина, но каждый её кадр считался
   underrun'ом. Здесь луп ставится НА ХВОСТ, и сценарий читает underruns из
   самой программы (дамп геометрии), а не с картинки.

Фикстура делается руками: короткий mp4 и событие вдвое длиннее материала со
слоу-эффектом — ровно форма проекта paintitblack.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, color_media, pause, require_tools  # noqa: E402

# СВОЙ ПРОЦЕСС (Р.68): сценарий проверяет САМ СТАРТ программы, а не работу в
# уже открытом проекте, — стенд работника ему не подходит.
OWN_PROCESS = True

SEC = 90000


def old_project(dest, media, media_secs, clip_secs):
    """Проект ПРЕЖНЕЙ формы: кривая скорости с `ease: true` и событие,
    которое длиннее материала."""
    subprocess.run([KT_CLI, "new", dest, "--force"], check=True, capture_output=True)
    doc = json.load(open(dest))
    doc["metadata"]["kt"]["settings"].update(fps=30, width=1280, height=720)
    # пустой проект пишется без дорожек (Р.25) — заводим видео-дорожку сами,
    # как это делает `project_with_clips` общего каркаса
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
            "name": "long",
            "source_range": {
                "OTIO_SCHEMA": "TimeRange.1",
                "start_time": rt(0),
                "duration": rt(clip_secs),
            },
            "media_references": {
                "DEFAULT_MEDIA": {
                    "OTIO_SCHEMA": "ExternalReference.1",
                    "name": "",
                    "target_url": media,
                    "available_range": None,
                    "available_image_bounds": None,
                    "metadata": {},
                }
            },
            "active_media_reference_key": "DEFAULT_MEDIA",
            # СКОРОСТЬ прежней формы: `ease: true` у кривой `rate`
            "effects": [
                {
                    "OTIO_SCHEMA": "Effect.1",
                    "name": "",
                    "effect_name": "kt:slowed_reverb",
                    "enabled": True,
                    "metadata": {
                        "kt": {
                            "kind": "slowed_reverb",
                            "rate": 0.5,
                            "mix": 0.0,
                            "reverb_delay_ms": 120.0,
                            "damp": 0.1,
                            "decay": 0.5,
                            "wet_gain": 1.0,
                            "auto": [
                                {
                                    "param": "rate",
                                    "ease": True,
                                    "keys": [
                                        {"t": 0, "v": 0.5},
                                        {"t": round(clip_secs * SEC), "v": 0.5},
                                    ],
                                }
                            ],
                        }
                    },
                }
            ],
            "markers": [],
            "enabled": True,
            "metadata": {"kt": {"id": 1, "media": 1}},
        }
    ]
    doc["metadata"]["kt"]["bin_media"] = []
    json.dump(doc, open(dest, "w"), indent=4)
    return dest


def main(out):
    require_tools()
    media_secs, clip_secs = 4, 12  # 4 с материала при rate 0.5 = 8 с события
    media = color_media(out, "short", "1E7FC8", seconds=media_secs)
    path = old_project(os.path.join(out, "oldfile.otio"), media, media_secs, clip_secs)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    # 1) файл ПРЕЖНЕЙ формы обязан открыться — это и есть претензия 1
    probe = subprocess.run(
        [KT_CLI, "probe", path, "0", "1"], capture_output=True, text=True
    )
    ok = "unsupported schema" not in (probe.stdout + probe.stderr)
    print(f"   {'ok  ' if ok else 'ПРОВАЛ'} файл с прежними кривыми скорости читается")
    if not ok:
        print(probe.stdout[-400:], probe.stderr[-400:])
        return 1

    with Live("oldfile", out, project=path) as app:
        app.expect(len(app.live_clips()) >= 1, f"событие на монтаже: {app.live_clips()}")
        app.shot("opened")

        # 2) ХВОСТ БЕЗ МАТЕРИАЛА виден: программа отдаёт его прямоугольник
        app.expect(
            app.has_probe("clip.1.no_material"),
            "хвост без материала помечен на событии (штриховка)",
        )

        # 3) ЛУП НА ХВОСТЕ не порождает underrun'ов
        # материал кончается на 8 с (4 с при rate 0.5), событие идёт до 12 с
        app.click(app.x_at(int(9.2 * SEC)), app.ruler_y())
        app.drag(app.x_at(int(9.0 * SEC)), app.ruler_y(), app.x_at(int(11.0 * SEC)), app.ruler_y())
        app.key("l")  # включить loop
        before = app.geom().get("underruns", 0)
        app.key("space")
        pause(
            4.0,
            "луп на хвосте проверяется ЖИВОЙ игрой: четыре секунды реального "
            "времени — это сама проверка, а не ожидание интерфейса",
        )
        after = app.geom().get("underruns", 0)
        playing = app.playing()
        app.key("space")
        app.shot("loop-tail")
        app.expect(playing, "воспроизведение шло на хвосте")
        app.expect(
            after == before,
            f"луп на хвосте БЕЗ материала не порождает underrun'ов: было {before}, стало {after}",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/oldfile"))

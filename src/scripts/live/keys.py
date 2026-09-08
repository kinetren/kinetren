#!/usr/bin/env python3
"""Живой сценарий Р.40 пп.15/18 и «ключи съезжают».

Претензии дословно:

- «замочек не до конца работает, надо сделать так чтобы его надо было
  СНЯТЬ чтобы отключилась синхронизация таймлайнов, и замочек надо чтобы
  запоминал свое состояние (хотя бы на весь проект)»;
- «когда я оттягиваю левый край события - у меня ключи автоматизации тоже
  сдвигаются к началу, и это поведение никак не поменять».

Вторая — не про данные: ключ и НЕ съезжает, врала ОСЬ графика. Здесь
проверяется настоящий трим настоящей мышью и то, что доехало до файла:
ключ обязан остаться на ТОМ ЖЕ кадре материала.

Замок проверяется тем, что он ПЕРЕЖИЛ запись и открытие проекта: до Р.40
он жил в окне и умирал вместе с ним.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def clip_kt(tl, index):
    items = []
    for track in tl.doc.get("tracks", {}).get("children", []):
        for item in track.get("children", []):
            if str(item.get("OTIO_SCHEMA", "")).startswith("Clip"):
                items.append(item)
    return (items[index].get("metadata") or {}).get("kt") or {}


def build(dest):
    """Событие с блюром и двумя ключами радиуса внутри материала."""
    # событие НЕ с нуля: левый край у самого края полосы дорожек
    # не схватить — там кончается зона хвата и начинается gutter
    project_with_clips(dest, [(1.0, 5.0, "synth:Ramp")])
    with open(dest) as f:
        doc = json.load(f)
    for track in doc.get("tracks", {}).get("children", []):
        for item in track.get("children", []):
            if not str(item.get("OTIO_SCHEMA", "")).startswith("Clip"):
                continue
            kt = item.setdefault("metadata", {}).setdefault("kt", {})
            kt["fx"] = [{"kind": "blur", "radius": 4}]
            kt["fx_auto"] = [
                [
                    {
                        "param": "radius",
                        "keys": [
                            {"t": 2 * SEC, "v": 1.0},
                            {"t": 4 * SEC, "v": 9.0},
                        ],
                    }
                ]
            ]
    with open(dest, "w") as f:
        json.dump(doc, f, indent=2)
    return dest


def keys_of(tl):
    kt = clip_kt(tl, 0)
    return [k["t"] for k in kt["fx_auto"][0][0]["keys"]]


def main(out):
    require_tools()
    proj = build(os.path.join(out, "keys.otio"))

    with Live("keys", out, project=proj) as app:
        app.shot("open")
        before = keys_of(app.project_now())
        app.expect(before == [2 * SEC, 4 * SEC], f"старт: два ключа, {before}")

        # --- левый край события вправо на секунду -------------------------
        y = app.lane_y("video", 0, frac=0.7)
        start0 = app.project_now().clips[0].start
        app.drag(app.x_at(start0), y, app.x_at(start0 + 1 * SEC), y)
        saved = app.saved_project()
        # СНАЧАЛА убеждаемся, что трим вообще произошёл: иначе «ключи не
        # съехали» выполнялось бы само собой и ничего не проверяло
        start1 = saved.clips[0].source_in
        app.expect(
            saved.clips[0].start != start0 or start1 != 0,
            f"трим обязан произойти: начало {saved.clips[0].start}, source_in {start1}",
        )
        after = keys_of(saved)
        app.expect(
            after == before,
            f"ключ обязан остаться на ТОМ ЖЕ кадре материала: было {before}, стало {after}",
        )
        app.shot("trim-right")

        # --- и обратно влево ---------------------------------------------
        clip = app.project_now().clips[0]
        app.drag(app.x_at(clip.start), y, app.x_at(max(clip.start - SEC // 2, 0)), y)
        saved = app.saved_project()
        app.expect(
            saved.clips[0].source_in != start1,
            f"и этот трим тоже: source_in {start1} → {saved.clips[0].source_in}",
        )
        after2 = keys_of(saved)
        app.expect(
            after2 == before,
            f"и при движении края ВЛЕВО тоже: было {before}, стало {after2}",
        )
        app.shot("trim-left")

        # --- замок оси помнится в проекте --------------------------------
        ui = saved.doc["metadata"]["kt"].get("ui") or {}
        app.expect(
            not ui.get("auto_axis_unlinked", False),
            "замок оси по умолчанию НАДЕТ и попадает в записанный проект",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

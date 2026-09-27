#!/usr/bin/env python3
"""Живой сценарий Р.55 (С5): РАСКРЫТИЕ композиции НА МЕСТЕ.

Претензия владельца дословно: «мы не очищаем таймлайн и открываем новый, а
"раздвигаем/раскрываем" композицию в том же таймлайне… а по умолчанию
гибридный удобный режим нужен, который и по основному таймлайну даст
проигрывание и по композиции одновременно».

Харнесс-тесты t570-t573 проверяют модель и вид в синтетической сцене. Здесь
другой вопрос — тот, ради которого живые сценарии и заведены: проходит ли
путь ЦЕЛИКОМ настоящей мышью и остаётся ли после него в ФАЙЛЕ то, что
человек сделал.

Шаги, каждый со своим наблюдаемым следствием:

1. свернуть два события в ленту — на таймлайне одно событие;
2. двойной клик по нему РАСКРЫВАЕТ на месте: строк стало БОЛЬШЕ, а
   глубина входа осталась НУЛЕВОЙ (из корня мы не вышли);
3. подвинуть событие ВНУТРИ полосы — правка обязана уехать в ЛЕНТУ, а не
   в корень: проверяется по сохранённому .otio;
4. проиграть — курсор идёт по КОРНЮ и пересекает и корневые события, и
   полосу: это и есть «по основному таймлайну и по композиции сразу»;
5. Esc сворачивает раскрытие — строк снова столько же.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, pause, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    path = os.path.join(out, "nestopen.otio")
    proj = project_with_clips(
        path,
        [(0.0, 2.0, "synth:sine:440"), (2.0, 2.0, "synth:ramp"), (6.0, 2.0, "synth:Ramp")],
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nestopen", out, project=proj) as app:
        y = app.lane_y("video", 0, 0.6)
        rows_before = len(app.geom()["rows"])

        # --- 1. свернуть ПЕРВЫЕ ДВА события в ленту ---------------------
        app.click(app.x_at(1 * SEC), y)
        app.click(app.x_at(3 * SEC), y, mods=("ctrl",))
        app.right_click(app.x_at(3 * SEC), y)
        app.click_probe("nest.collapse", "пункт сворачивания")
        app.expect(
            len(app.live_clips()) == 2,
            f"свёрнуто не в одно (плюс третье событие): {app.live_clips()}",
        )
        app.shot("collapsed")

        # --- 2. ДВОЙНОЙ КЛИК раскрывает НА МЕСТЕ ------------------------
        inst = min(app.live_clips())
        mid = inst[0] + inst[1] // 2
        app.double_click(app.x_at(mid), y)
        g = app.geom()
        app.shot("expanded")
        app.expect(
            g["nest"]["depth"] == 0,
            f"двойной клик ушёл в ПОЛНЫЙ вход: глубина {g['nest']['depth']} — "
            "тогда играла бы лента, а не программа",
        )
        app.expect(
            len(g["rows"]) > rows_before,
            f"строк не прибавилось — полосы не видно: {len(g['rows'])} против {rows_before}",
        )
        app.expect(
            len(app.live_clips()) > 2,
            f"содержимого ленты не видно на таймлайне: {app.live_clips()}",
        )

        # --- 3. правка ВНУТРИ полосы уезжает в ЛЕНТУ --------------------
        # событие полосы лежит на дорожке НИЖЕ корневых (хвост пространства)
        y_band = app.lane_y("video", 1, 0.6)
        inner = sorted(c for c in app.live_clips() if c not in ())
        app.click(app.x_at(int(0.5 * SEC)), y_band)
        app.drag(app.x_at(int(0.5 * SEC)), y_band, app.x_at(int(1.2 * SEC)), y_band)
        app.shot("edited")
        app.key("ctrl+s")
        saved = app.saved_project()
        kt = saved.doc["metadata"]["kt"]
        app.expect(
            len(kt.get("sequences", [])) == 1,
            f"лента не записалась: {list(kt.keys())}",
        )
        # Тело ленты лежит Stack'ом ВНУТРИ трека (сайдкар `kt.sequences`
        # несёт только дорожки и порядок строк) — значит следствие правки
        # надо искать там же, где его увидит чужой редактор.
        stacks = [
            c
            for track in saved.doc["tracks"]["children"]
            for c in track.get("children", [])
            if c.get("OTIO_SCHEMA", "").startswith("Stack")
        ]
        app.expect(bool(stacks), "вложение не записано Stack'ом внутри трека")
        inner_tracks = stacks[0].get("children", [])
        inner_clips = [
            c
            for t in inner_tracks
            for c in t.get("children", [])
            if c.get("OTIO_SCHEMA", "").startswith("Clip")
        ]
        app.expect(
            len(inner_clips) == 2,
            f"содержимое ленты уехало из неё: {len(inner_clips)} событий внутри Stack",
        )
        # и в КОРНЕ треке событий по-прежнему два: экземпляр и третий клип
        root_clips = [
            c
            for track in saved.doc["tracks"]["children"]
            for c in track.get("children", [])
            if c.get("OTIO_SCHEMA", "").startswith("Clip")
        ]
        app.expect(
            len(root_clips) == 1,
            f"событие ленты уехало в КОРЕНЬ файла: {len(root_clips)} клипов рядом со Stack",
        )
        _ = inner

        # --- 4. ИГРАЕТ ВСЯ ПРОГРАММА -----------------------------------
        before = app.geom().get("underruns", 0)
        app.click(app.x_at(int(0.2 * SEC)), app.ruler_y())
        app.key("space")
        pause(
            1.6,
            "воспроизведение идёт в РЕАЛЬНОМ времени: эти секунды игры и есть "
            "проверка голодания кольца, ускорить их нечем",
        )
        app.key("space")
        after = app.geom().get("underruns", 0)
        app.expect(
            app.playhead() > int(0.2 * SEC),
            f"курсор не поехал: {app.playhead()}",
        )
        app.expect(
            after - before < 200,
            f"раскрытая композиция голодает: underrun'ов прибавилось {after - before}",
        )
        app.shot("played")

        # --- 5. Esc сворачивает раскрытие -------------------------------
        app.key("Escape")
        app.key("Escape")
        app.shot("collapsed-back")
        app.expect(
            len(app.geom()["rows"]) == rows_before,
            f"после свёртки строк не столько же: {len(app.geom()['rows'])} против {rows_before}",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

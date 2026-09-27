#!/usr/bin/env python3
"""Живой сценарий Р.50: свернуть, войти, поправить, выйти, сохранить.

Харнесс-тесты t427/t456/t457 это уже проверяют, но в СИНТЕТИЧЕСКОЙ сцене
без окна. Здесь настоящая мышь в настоящем окне отвечает на другой вопрос:
проходит ли путь ЦЕЛИКОМ и остаётся ли после него в ФАЙЛЕ то, что человек
сделал. Ровно так в прошлых раундах находилось, что «зелено в cargo test»
и «работает в окне» — разные утверждения.

Шаги, каждый со своим наблюдаемым следствием:

1. выделить два события и свернуть их пунктом меню — на таймлайне
   становится ОДНО событие;
2. двойной клик по нему заводит ВНУТРЬ — программа сама говорит глубину и
   имя, а список событий подменяется содержимым ленты;
3. сдвинуть внутреннее событие — правка обязана остаться ВНУТРИ;
4. Esc выводит наружу — снова одно событие и глубина ноль;
5. Ctrl+S — и в .otio лежат и лента, и правка внутри неё.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    path = os.path.join(out, "nest.otio")
    proj = project_with_clips(
        path, [(0.0, 3.0, "synth:sine:440"), (3.0, 3.0, "synth:ramp")]
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nest", out, project=proj) as app:
        app.shot("open")
        app.expect(len(app.live_clips()) == 2, f"старт: два события, {app.live_clips()}")
        app.expect(app.geom()["nest"]["depth"] == 0, "старт: мы в корне")

        # --- 1. свернуть выделение ------------------------------------
        y = app.lane_y("video", 0, 0.6)
        app.click(app.x_at(1 * SEC), y)
        app.click(app.x_at(4 * SEC), y, mods=("ctrl",))
        app.shot("selected")
        app.right_click(app.x_at(4 * SEC), y)
        app.click_probe("nest.collapse", "пункт «Свернуть выделение в ленту»")
        app.shot("collapsed")
        app.expect(
            len(app.live_clips()) == 1,
            f"после сворачивания обязано остаться ОДНО событие, а их {app.live_clips()}",
        )

        # --- 2. войти ПУНКТОМ МЕНЮ -------------------------------------
        # С Р.55 (С5) двойной клик даёт РАСКРЫТИЕ НА МЕСТЕ — режим по
        # умолчанию: быстрый жест даёт умолчание, названный пункт меню —
        # названный режим. Полный вход, который проверяет этот сценарий,
        # живёт теперь в пункте «Войти в ленту»; раскрытие на месте
        # проверяет `nestopen.py`.
        inst_t = app.live_clips()[0][0] + app.live_clips()[0][1] // 2
        app.right_click(app.x_at(inst_t), y)
        app.click_probe("nest.enter", "пункт «Войти в ленту»")
        nest = app.geom()["nest"]
        app.shot("inside")
        app.expect(nest["depth"] == 1, f"пункт меню не завёл внутрь: {nest}")
        app.expect(nest["name"] != "", f"программа не назвала ленту: {nest}")
        inside = app.live_clips()
        app.expect(
            len(inside) == 2,
            f"внутри ленты обязано быть ЕЁ содержимое, а видно {inside}",
        )

        # --- 3. правка ВНУТРИ ------------------------------------------
        first = inside[0]
        x0 = app.x_at(first[0] + first[1] // 2)
        app.drag(x0, y, x0 + 60, y)
        moved = app.live_clips()
        app.expect(
            moved != inside,
            f"перенос внутри ленты ничего не изменил: {moved}",
        )
        app.shot("edited-inside")

        # --- 4. выйти ---------------------------------------------------
        app.key("Escape")  # снять выделение
        app.key("Escape")  # выйти наружу
        nest = app.geom()["nest"]
        app.expect(nest["depth"] == 0, f"Esc не вывел наружу: {nest}")
        app.expect(
            len(app.live_clips()) == 1,
            f"снаружи снова обязано быть одно событие: {app.live_clips()}",
        )
        app.shot("outside")

        # --- 5. следствие В ФАЙЛЕ ---------------------------------------
        app.save_now()
        with open(path) as f:
            doc = json.load(f)
        seqs = doc["metadata"]["kt"].get("sequences", [])
        app.expect(bool(seqs), "в сохранённом файле нет ленты")
        app.expect(
            doc["metadata"]["kt"]["v"] >= 6,
            f"файл с вложением обязан нести версию 6: {doc['metadata']['kt']['v']}",
        )
        # содержимое ленты лежит Stack'ом внутри трека — его и читает
        # чужой редактор
        stacks = [
            c
            for track in doc["tracks"]["children"]
            for c in track.get("children", [])
            if c.get("OTIO_SCHEMA", "").startswith("Stack")
        ]
        app.expect(bool(stacks), "вложение не записано Stack'ом внутри трека")
        app.expect(
            bool(stacks[0].get("children")),
            "тело ленты не инлайнено в экземпляр — чужой редактор увидел бы пустоту",
        )

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/nest"))

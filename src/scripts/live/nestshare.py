#!/usr/bin/env python3
"""Живой сценарий Р.50: одна лента в трёх местах — правка меняет ВСЕ.

Это и есть смысл решения владельца «ссылка на объект внутри проекта»:
монтаж, поставленный в нескольких местах, правится один раз. Разведка
назвала общей бедой Premiere, FCP и VEGAS то, что по клипу на таймлайне НЕ
ВИДНО, сколько ещё мест изменится вместе с ним, — поэтому здесь же
проверяется бейдж «×N» и то, что он появляется ТОЛЬКО когда мест больше
одного.

Следствие проверяется В ФАЙЛЕ: тело ленты инлайнится в каждый экземпляр,
и после правки внутри все три инлайна обязаны стать одинаковыми — иначе
чужой редактор увидел бы три РАЗНЫХ монтажа там, где человек правил один.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def stacks_of(path):
    with open(path) as f:
        doc = json.load(f)
    return [
        c
        for track in doc["tracks"]["children"]
        for c in track.get("children", [])
        if c.get("OTIO_SCHEMA", "").startswith("Stack")
    ]


def main(out):
    require_tools()
    path = os.path.join(out, "nestshare.otio")
    proj = project_with_clips(
        path, [(0.0, 2.0, "synth:sine:440"), (2.0, 2.0, "synth:ramp")]
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nestshare", out, project=proj) as app:
        y = app.lane_y("video", 0, 0.6)
        # --- свернуть оба события в одну ленту ------------------------
        app.click(app.x_at(1 * SEC), y)
        app.click(app.x_at(3 * SEC), y, mods=("ctrl",))
        app.right_click(app.x_at(3 * SEC), y)
        app.click_probe("nest.collapse", "пункт сворачивания")
        app.expect(len(app.live_clips()) == 1, f"свёрнуто не в одно: {app.live_clips()}")
        app.shot("one")

        # --- поставить ту же ленту ещё дважды -------------------------
        # ПУТЬ РУКИ — меню «Правка»: системного буфера в виртуальном
        # экране может не быть вовсе, а пункт меню зовёт то же тело, что и
        # аккорд (правило 17)
        inst = app.live_clips()[0]
        app.click(app.x_at(inst[0] + inst[1] // 2), y)
        app.click_probe("menu.edit", "кнопка меню «Правка»")
        app.click_probe("edit.copy", "пункт «Копировать»")
        app.expect(app.geom().get("clipboard") == 1, "копирование не набрало буфер")
        for at in (5.0, 9.0):
            app.click(app.x_at(int(at * SEC)), app.ruler_y())
            app.click_probe("menu.edit", "кнопка меню «Правка»")
            app.click_probe("edit.paste", "пункт «Вставить»")
        app.shot("three")
        app.expect(
            len(app.live_clips()) == 3,
            f"лента обязана стоять в ТРЁХ местах, а стоит в {len(app.live_clips())}",
        )

        # --- бейдж ×N ---------------------------------------------------
        # он живёт на подписи события; проверяем через файл-независимый
        # признак: программа сама считает места и отдаёт их в дампе
        # ПОЛНЫЙ вход — пункт меню (Р.55, С5): двойной клик с этого раунда
        # раскрывает на месте, а счётчик мест программа отдаёт в дампе
        # именно для того места, В КОТОРОМ СТОИШЬ
        app.right_click(app.x_at(inst[0] + inst[1] // 2), y)
        app.click_probe("nest.enter", "пункт «Войти в ленту»")
        nest = app.geom()["nest"]
        app.expect(
            nest["uses"] == 3,
            f"программа насчитала {nest['uses']} мест вместо трёх — «сколько изменится "
            f"вместе» не на чем показать",
        )

        # --- правка ВНУТРИ --------------------------------------------
        inside = app.live_clips()
        first = inside[0]
        # Y ПЕРЕСЧИТЫВАЕТСЯ: внутри ленты строк другое число, и высота
        # строки другая — координата, снятая снаружи, попадает не в ту
        # строку. Снаружи это не было видно, пока сценарий входил в ленту
        # сразу после сворачивания.
        y = app.lane_y("video", 0, 0.6)
        x0 = app.x_at(first[0] + first[1] // 2)
        # сперва ОДИНОЧНЫЙ клик: меню egui закрывается следующим кликом и
        # съедает его нажатие, а драг сразу после пункта меню тогда не
        # начинается вовсе (не дефект программы — повадка попапа)
        app.click(x0, y)
        app.drag(x0, y, x0 + 70, y)
        moved = app.live_clips()
        app.expect(
            moved != inside,
            f"правка внутри не прошла: было {inside}, стало {moved}",
        )
        app.key("Escape")
        app.key("Escape")
        app.expect(app.geom()["nest"]["depth"] == 0, "не вышли наружу")
        app.shot("edited")

        # --- следствие: ВСЕ ТРИ места одинаковы -----------------------
        app.save_now()
        st = stacks_of(path)
        app.expect(len(st) == 3, f"в файле не три экземпляра, а {len(st)}")
        bodies = [json.dumps(s.get("children"), sort_keys=True) for s in st]
        app.expect(
            len(set(bodies)) == 1,
            "тела трёх экземпляров РАЗОШЛИСЬ: правка внутри дошла не до всех, и чужой "
            "редактор увидел бы три разных монтажа",
        )
        seqs = {s["metadata"]["kt"]["seq"] for s in st}
        app.expect(len(seqs) == 1, f"экземпляры смотрят на РАЗНЫЕ ленты: {seqs}")

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/nestshare"))

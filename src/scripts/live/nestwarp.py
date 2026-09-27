#!/usr/bin/env python3
"""Живой сценарий Р.55 (С4): РЕВЕРС и ЗАМЕДЛЕНИЕ на композиции.

Претензии владельца дословно: «А slowedandreverb не срабатывает на
прекомпозициях» и «на композицию не работает реверсы».

Не «не срабатывало» — было ЗАПРЕЩЕНО: `validate` отвергал зеркальный или
растягивающий время экземпляр ленты, потому что суб-микс читался строго по
порядку. Команда падала, и в статусбар уходил английский технический
текст. Теперь у суб-микса есть продюсер с пре-роллом, и оба жеста проходят.

Харнесс-тесты это уже проверяют на синтетических сценах (t428, t561-t565).
Здесь другой вопрос: доходит ли ЖЕСТ РУКОЙ до модели и переживает ли это
сохранение — то есть работает ли путь целиком, из точки, откуда действует
человек.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, pause, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    path = os.path.join(out, "nestwarp.otio")
    proj = project_with_clips(
        path, [(0.0, 2.0, "synth:sine:440"), (2.0, 2.0, "synth:ramp")]
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nestwarp", out, project=proj) as app:
        y = app.lane_y("video", 0, 0.6)
        # --- свернуть оба события в ленту -------------------------------
        app.click(app.x_at(1 * SEC), y)
        app.click(app.x_at(3 * SEC), y, mods=("ctrl",))
        app.right_click(app.x_at(3 * SEC), y)
        app.click_probe("nest.collapse", "пункт сворачивания")
        app.expect(len(app.live_clips()) == 1, f"свёрнуто не в одно: {app.live_clips()}")
        app.shot("collapsed")

        # --- РЕВЕРС ВЛОЖЕНИЯ рукой, через меню события ------------------
        inst = app.live_clips()[0]
        mid = inst[0] + inst[1] // 2
        app.right_click(app.x_at(mid), y)
        app.click_probe("tl.reverse", "пункт «Реверс»")
        app.shot("reversed")
        # СЛЕДСТВИЕ в модели, а не «команда не упала»
        app.key("ctrl+s")
        saved = app.saved_project()
        kt = saved.doc["metadata"]["kt"]
        app.expect(
            len(kt.get("sequences", [])) == 1,
            f"лента не записалась: {list(kt.keys())}",
        )
        # реверс экземпляра лежит в эффектах Stack'а конверта
        dump = str(saved.doc)
        app.expect(
            "LinearTimeWarp" in dump,
            "реверс вложения не доехал до файла — команда упала молча",
        )

        # --- ИГРАЕТ, и без голодания ------------------------------------
        before = app.geom().get("underruns", 0)
        app.click(app.x_at(int(0.5 * SEC)), app.ruler_y())
        app.key("space")
        pause(
            1.5,
            "воспроизведение идёт в РЕАЛЬНОМ времени: полторы секунды игры — "
            "это сама проверка, а не ожидание реакции интерфейса",
        )
        app.key("space")
        after = app.geom().get("underruns", 0)
        app.expect(
            app.playhead() > int(0.5 * SEC),
            f"курсор не поехал: {app.playhead()}",
        )
        app.expect(
            after - before < 200,
            f"зеркальное вложение голодает: underrun'ов прибавилось {after - before}",
        )
        app.shot("played")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

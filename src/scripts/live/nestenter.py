#!/usr/bin/env python3
"""Живой сценарий Р.56 (С4): Enter по композиции ведёт ВГЛУБЬ.

Претензия владельца дословно: «если я нажимаю enter на выбраной раскрытой
композиции то я не попадаю внутрь, хотя на пкм пункт так и называется и
подписан enter и работает. если просто enter нажать то композиция скрывается
обратно, я хочу чтобы двойной enter переходил в нее».

Харнесс-тесты t457/t597 проверяют решение и выделение в синтетической сцене.
Здесь вопрос другой: доходит ли путь целиком настоящей клавиатурой — и в
частности, ПЕРЕЖИВАЕТ ли выделение экземпляра первое нажатие. Именно на этом
дефект и держался: после раскрытия выделение сбрасывалось, и второй Enter не
делал ВООБЩЕ ничего.

Шаги, каждый со своим наблюдаемым следствием:

1. свернуть два события в ленту;
2. клик по экземпляру, Enter — РАСКРЫТИЕ на месте: строк больше, глубина 0;
3. Enter ещё раз — ПОЛНЫЙ ВХОД: глубина 1, полосы больше нет;
4. ВНУТРИ ленты (Р.58, п.460): свернуть её события во вложенную ленту,
   Enter по экземпляру — строки полосы появляются, глубина по-прежнему 1
   («открывая композицию не могу раскрыть в ней вложенную» — претензия
   владельца Р.58);
5. Esc-каскад наружу — и полоса, через которую входили, ВОЗВРАЩАЕТСЯ
   (решение владельца Р.58, пересмотр Р.56 п.5: «при переходе между
   композициями они сбрасывают состояние раскрытости, а хотелось бы
   чтобы оставляли»).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    path = os.path.join(out, "nestenter.otio")
    proj = project_with_clips(
        path,
        [(0.0, 2.0, "synth:sine:440"), (2.0, 2.0, "synth:ramp"), (6.0, 2.0, "synth:Ramp")],
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nestenter", out, project=proj) as app:
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

        # --- 2. ПЕРВЫЙ Enter раскрывает на месте ------------------------
        inst = min(app.live_clips())
        mid = inst[0] + inst[1] // 2
        app.click(app.x_at(mid), y)
        app.key("Return")
        g = app.geom()
        app.shot("expanded")
        app.expect(
            g["nest"]["depth"] == 0,
            f"первый Enter ушёл в полный вход: глубина {g['nest']['depth']}",
        )
        app.expect(
            len(g["rows"]) > rows_before,
            f"полосы не видно: строк {len(g['rows'])} против {rows_before}",
        )

        # --- 3. ВТОРОЙ Enter заводит ВНУТРЬ -----------------------------
        # без клика мышью: ровно тут дефект и жил — выделение экземпляра
        # обязано пережить раскрытие, иначе жест бьёт в пустоту
        app.key("Return")
        g = app.geom()
        app.shot("entered")
        app.expect(
            g["nest"]["depth"] == 1,
            f"второй Enter не завёл внутрь ленты: глубина {g['nest']['depth']}",
        )
        app.expect(
            len(app.live_clips()) == 2,
            f"внутри ленты видно не её содержимое: {app.live_clips()}",
        )

        # --- 4. ВНУТРИ ленты: полоса по вложению (Р.58, п.460) -----------
        rows_inside = len(g["rows"])
        inner = sorted(app.live_clips())
        y_in = app.lane_y("video", 0, 0.6)
        app.click(app.x_at(inner[0][0] + inner[0][1] // 2), y_in)
        app.click(app.x_at(inner[1][0] + inner[1][1] // 2), y_in, mods=("ctrl",))
        app.right_click(app.x_at(inner[1][0] + inner[1][1] // 2), y_in)
        app.click_probe("nest.collapse", "пункт сворачивания внутри ленты")
        app.expect(
            len(app.live_clips()) == 1,
            f"внутри ленты не свернулось в одно вложение: {app.live_clips()}",
        )
        y_inst = min(app.live_clips())
        app.click(app.x_at(y_inst[0] + y_inst[1] // 2), y_in)
        app.key("Return")
        g = app.geom()
        app.shot("band-inside")
        app.expect(
            g["nest"]["depth"] == 1,
            f"Enter по вложению внутри ленты ушёл в полный вход: глубина {g['nest']['depth']}",
        )
        app.expect(
            len(g["rows"]) > rows_inside,
            f"полосы внутри ленты не видно: строк {len(g['rows'])} против {rows_inside}",
        )

        # --- 5. Esc-каскад наружу — ПОЛОСА ВОЗВРАЩАЕТСЯ (Р.58, п.461) ----
        # снять выделение → свернуть полосу → выйти (t573); после выхода
        # раскрытие, через которое входили, снова на месте (t636)
        for _ in range(3):
            app.key("Escape")
        g = app.geom()
        app.shot("left")
        app.expect(g["nest"]["depth"] == 0, f"Esc не вывел наружу: глубина {g['nest']['depth']}")
        app.expect(
            len(g["rows"]) > rows_before,
            f"выход не вернул полосу, через которую входили (решение владельца Р.58): "
            f"строк {len(g['rows'])} против {rows_before}",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

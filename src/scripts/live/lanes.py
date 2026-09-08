#!/usr/bin/env python3
"""Живой сценарий Р.40: дорожки и служебные блоки.

Претензии пользователя дословно:

- «п.11 не выполнен - дорожка меняет высоту а события до сих пор мелкие -
  ты нихуя не проверял»;
- «п.2 про ПКМ добавление морфера по треку не выполнен»;
- «п.2 я не могу перемещать морферы по разным дорожкам»;
- «Морферы почему-то не выделяются через area-select, нихуя ты не
  проверял».

Всё это было ЗЕЛЕНО в харнессе Р.38 и сломано в окне: тесты щупали место,
куда я положил функцию, а не место, куда тычет человек. Поэтому здесь
настоящая мышь в настоящем окне, и каждый шаг проверяется НАБЛЮДАЕМЫМ
СЛЕДСТВИЕМ — тем, что доехало до файла проекта.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def build(dest):
    """Клип, ДВЕ морф-дорожки и блок на верхней — есть куда его везти."""
    project_with_clips(dest, [(0.0, 8.0, "synth:Ramp")])
    with open(dest) as f:
        doc = json.load(f)
    kt = doc["metadata"]["kt"]
    kt["v"] = 4
    kt["morph_lanes"] = 2
    kt["lane_order"] = [["morph", 0], ["morph", 1], ["video", 0], ["audio", 0]]
    kt["morphers"] = [
        {"id": 1, "lane": 0, "start": 2 * SEC, "duration": 2 * SEC, "patch": {}}
    ]
    with open(dest, "w") as f:
        json.dump(doc, f, indent=2)
    return dest


def main(out):
    require_tools()
    proj = build(os.path.join(out, "lanes.otio"))

    with Live("lanes", out, project=proj) as app:
        app.shot("open")
        p = app.project_now()
        app.expect(len(p.clips) == 1, f"старт: один клип, {p.clips}")

        g = app.geom()
        rows = {(r["kind"], r["lane"]): r for r in g["rows"]}
        video_h0 = rows[("video", 0)]["h"]
        win = geometry()

        # --- 1. высота строки доходит ДО СОБЫТИЯ (п.11) --------------------
        # Тянем нижнюю кромку видео-строки вниз, а потом хватаем клип ТАМ,
        # где его до Р.40 не было: ниже старой границы. Если событие
        # по-прежнему рисуется базовой высотой — жест уйдёт в пустоту и
        # клип не сдвинется.
        gx = win["x"] + g["timeline"]["left"] - 8.0
        edge_y = app.lane_y("video", 0, frac=1.0)
        grow = 46
        app.drag(gx, edge_y, gx, edge_y + grow)
        g2 = app.geom()
        rows2 = {(r["kind"], r["lane"]): r for r in g2["rows"]}
        app.near(
            rows2[("video", 0)]["h"],
            video_h0 + grow,
            12,
            "строка выросла на столько, на сколько тянули",
        )
        # точка НИЖЕ старой границы строки, но внутри новой
        deep_y = rows2[("video", 0)]["top"] + win["y"] + video_h0 + grow * 0.5
        start0 = p.clips[0].start
        x0 = app.x_at(start0 + SEC)
        app.drag(x0, deep_y, x0 + 120, deep_y)
        saved = app.saved_project()
        moved = saved.clips[0].start
        app.expect(
            moved != start0,
            f"клип схвачен НИЖЕ старой границы строки: {start0} → {moved}. "
            "Не сдвинулся — значит прямоугольник события не вырос вместе со строкой",
        )
        app.shot("row-height-reaches-clip")

        # --- 2. ПКМ по САМОЙ морф-дорожке даёт меню (п.2) -----------------
        # до Р.40 здесь открывалось меню ПУСТОГО МЕСТА: пункт «добавить
        # морфер сюда» жил только в 34-пиксельном заголовке строки
        my = app.lane_y("morph", 1, frac=0.5)
        mx = app.x_at(6 * SEC)
        before = app.shot("before-lane-menu")
        app.right_click(mx, my)
        app.shot("lane-menu")
        d = app.screen_diff(before)
        app.expect(
            d > 0.001,
            f"ПКМ по морф-дорожке обязан открыть меню (экран изменился на {d:.1%})",
        )
        app.key("Escape")

        # --- 3. морф-блок переезжает на соседнюю дорожку (п.2) ------------
        y0 = app.lane_y("morph", 0, frac=0.5)
        y1 = app.lane_y("morph", 1, frac=0.5)
        bx = app.x_at(3 * SEC)
        app.drag(bx, y0, bx, y1)
        saved = app.saved_project()
        m = saved.doc["metadata"]["kt"]["morphers"][0]
        app.expect(
            m["lane"] == 1,
            f"морф-блок обязан переезжать на дорожку под курсором, а он на {m['lane']}",
        )
        app.shot("morph-moved")

        # --- 4. рамка ловит морфер, Del его убирает -----------------------
        # Ctrl+драг по пустому месту — рамка выделения (обычный драг там
        # рисует регион). Следствие проверяем удалением: пойманное исчезает
        # из файла.
        y1 = app.lane_y("morph", 1, frac=0.5)
        rx0 = app.x_at(int(1.5 * SEC))
        rx1 = app.x_at(int(4.5 * SEC))
        app.drag(rx0, y1 - 6, rx1, y1 + 6, mods=("ctrl",))
        app.key("Delete")
        saved = app.saved_project()
        left = saved.doc["metadata"]["kt"].get("morphers", [])
        app.expect(
            not left,
            f"рамка обязана выделять морфер, а Del — убирать пойманное; осталось {left}",
        )
        app.shot("rubber-deleted")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

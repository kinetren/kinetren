#!/usr/bin/env python3
"""Живой сценарий Р.40 п.14: линейка показывает время и риски.

Претензия дословно: «у таймлайна автоматизации если событие/морфер
маленькое то время в линейке проставляется только 0:00 самое первое, что
тупо. а если зазумится то и время это улетает в бок… линейка должна
отображать и следующую секунду и рисочки тоже желательно (и рисочек на
основном таймлайне тоже нет)».

Обе беды от одной причины: лестница шагов начиналась с СЕКУНДЫ, а подпись
выбирается порогом 70 px — при 1 с ≥ 70 px минорный шаг совпадал с
подписью, и рисок не рисовалось ни одной.

Проверяется по ПИКСЕЛЯМ полосы линейки: сколько на ней вертикальных
штрихов. Харнесс этого не покажет — он не рисует.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def tick_columns(png, y, x0, x1):
    """Сколько «светлых» столбцов на строке y в диапазоне [x0, x1).

    Риски линейки — вертикальные штрихи в 1 px по тёмному фону, поэтому
    достаточно посчитать локальные всплески яркости.
    """
    out = subprocess.run(
        ["convert", png, "-crop", f"{int(x1 - x0)}x1+{int(x0)}+{int(y)}",
         "-depth", "8", "txt:-"],
        capture_output=True, text=True, check=True,
    ).stdout
    vals = []
    for line in out.splitlines()[1:]:
        if "srgb(" not in line and "gray(" not in line:
            continue
        body = line.split("(", 1)[1].split(")", 1)[0]
        nums = [int(v) for v in body.split(",")[:3] if v.strip().isdigit()]
        vals.append(sum(nums) / max(len(nums), 1))
    runs, prev = 0, 0.0
    for v in vals:
        if v > prev + 12:
            runs += 1
        prev = v
    return runs


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "ruler.otio"), [(0.0, 6.0, "synth:Ramp")]
    )

    with Live("ruler", out, project=proj) as app:
        shot = app.shot("open")
        g = app.geom()
        tl = g["timeline"]
        # дамп геометрии даёт координаты ОТНОСИТЕЛЬНО ОКНА, а снимок —
        # экранный: забыть начало окна значит мерить не там (грабли Р.38)
        win = geometry()
        x0, x1 = win["x"] + tl["left"] + 20, win["x"] + tl["left"] + 320
        # ДВЕ строки пикселей, и в этом весь смысл замера:
        #   y_minor — самый верх полосы рисок (STRIPES_H = 11), туда
        #     достают И минорные, И крупные риски, а подписи ещё нет
        #     (её верх на 12);
        #   y_major — ниже подписей, там остаются только крупные риски.
        # Абсолютное число штрихов ничего не доказывает: на первой попытке
        # замер стоял на 20 px, ловил антиалиасинг ЦИФР и был зелёным на
        # старой лестнице тоже.
        y_minor = win["y"] + tl["top"] + 11
        y_major = win["y"] + tl["top"] + 30
        n_minor = tick_columns(shot, y_minor, x0, x1)
        n_major = tick_columns(shot, y_major, x0, x1)
        app.expect(
            n_minor >= 3 * n_major + 5,
            f"минорных рисок обязано быть КРАТНО больше, чем подписей: "
            f"{n_minor} на полосе рисок против {n_major} ниже подписей "
            f"(при 100 px/с шаг 0.1 с — это примерно 30 против 3)",
        )
        app.shot("ruler-ticks")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

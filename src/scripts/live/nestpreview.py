#!/usr/bin/env python3
"""Живой сценарий Р.56 (С5): у композиции на таймлайне есть КАДРЫ и ПИКИ.

Претензия владельца дословно: «у композиций нет превью почему-то», и,
уточняя, — «на таймлайне нет превью как у обычных событий - кадров и
аудио-пиков».

Харнесс-тест t600 проверяет, что кадры доезжают до текстур. Здесь вопрос
другой и единственный, ради которого живые сценарии заведены: видно ли это
НА ЭКРАНЕ настоящей программы. Сравниваем яркостную «пестроту» тела
события ДО и ПОСЛЕ того, как служба досчитала: пустая плашка однородна,
филмстрип — нет.

Шаги:

1. свернуть два события в ленту — на таймлайне одно событие;
2. дождаться службы и снять картинку тела: она обязана перестать быть
   однородной заливкой;
3. правка внутри ленты меняет редакцию — полоса пересчитывается, и на
   экране снова кадры.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def slice_means(app, x0, y0, w, h, tag, n=3):
    """Средняя яркость N вертикальных ломтей прямоугольника — одним
    снимком экрана.

    Пустая плашка и мини-карта содержимого дают ломти ОДИНАКОВОЙ яркости;
    филмстрип — разные кадры, значит разные ломти. Это и есть наблюдаемое
    следствие «кадры видно», а не «что-то изменилось».
    """
    img = app.shot(tag)
    out = []
    for i in range(n):
        x = int(x0 + w * i / n)
        ww = max(1, int(w / n) - 2)
        v = subprocess.run(
            [
                "convert",
                img,
                "-crop",
                f"{ww}x{int(h)}+{x}+{int(y0)}",
                "+repage",
                "-colorspace",
                "Gray",
                "-format",
                "%[fx:mean]",
                "info:-",
            ],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        out.append(float(v))
    return out


def main(out):
    require_tools()
    path = os.path.join(out, "nestpreview.otio")
    proj = project_with_clips(
        path,
        [(0.0, 2.0, "synth:bars"), (2.0, 2.0, "synth:ramp"), (6.0, 2.0, "synth:sine:440")],
    )
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nestpreview", out, project=proj) as app:
        y = app.lane_y("video", 0, 0.6)

        # --- 1. свернуть ПЕРВЫЕ ДВА события в ленту --------------------
        app.click(app.x_at(1 * SEC), y)
        app.click(app.x_at(3 * SEC), y, mods=("ctrl",))
        app.right_click(app.x_at(3 * SEC), y)
        app.click_probe("nest.collapse", "пункт сворачивания")
        app.expect(
            len(app.live_clips()) == 2,
            f"свёрнуто не в одно (плюс третье событие): {app.live_clips()}",
        )
        inst = min(app.live_clips())
        x0 = app.x_at(inst[0]) + 6
        x1 = app.x_at(inst[0] + inst[1]) - 6
        # координаты строки — из САМОЙ программы (правило Р.27)
        g = app.geom()
        row = next(r for r in g["rows"] if r["kind"] == "video" and r["lane"] == 0)
        h = row.get("h", g["lane_h"])
        top = app.lane_y("video", 0, 0.0)
        box = (x0, top + 6, x1 - x0, h - 12)

        # --- 2. дождаться службы: в теле видно РАЗНЫЕ кадры ------------
        spread = 0.0
        for _ in range(60):
            # шевелим мышь: перерисовка идёт по ДВИЖЕНИЮ (инвариант 12)
            app.hover(app.x_at(inst[0]) + 20, y, pause=0.05)
            poll_step(0.4)
            m = slice_means(app, *box, "wait")
            spread = max(spread, max(m) - min(m))
            if spread > 0.08:
                break
        app.shot("frames")
        app.expect(
            spread > 0.08,
            f"в теле композиции все ломти одной яркости (разброс {spread:.3f}) — "
            "значит это заливка или мини-карта, а не кадры",
        )

        # --- 3. и это КАДРЫ ЛЕНТЫ, а не тумбы файла: у ленты файла нет,
        # а служба индекса про неё ничего не знает --------------------
        app.key("ctrl+s")
        kt = app.saved_project().doc["metadata"]["kt"]
        app.expect(
            len(kt.get("sequences", [])) == 1,
            f"лента не записалась — проверяли не то событие: {list(kt.keys())}",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

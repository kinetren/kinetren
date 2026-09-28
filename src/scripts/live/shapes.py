#!/usr/bin/env python3
"""Живой сценарий Р.53 (пп.417-424): фигура с альфой и анимация титра.

Проверяются три вещи, которых харнесс не видит:

1. **Фигура кладётся из сгруппированного меню** («Генератор» → «Фигуры» →
   «Прямоугольник») и её АЛЬФА честная: в превью прямоугольник виден
   поверх кадра нижней дорожки, а рядом с ним просвечивает то, что снизу.
2. **Правило альфы (п.417)**: под фигурой остаётся картинка, а не чёрная
   плита — именно это ломал EventGrid, объявлявший альфу потерянной.
3. **Пресет титра ставит ВСЁ и одним шагом** (п.424): нажал «Машинка» —
   титр поехал печататься; в НУЛЕ события букв нет, дальше они есть, и
   один Ctrl+Z возвращает весь набор полей.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def new_generator(app, group, kind):
    """Создать генератор через меню фона бина: группа, потом вид."""
    win = geometry()
    b = app.geom()["bin"]
    app.right_click(
        win["x"] + int((b["left"] + b["right"]) / 2),
        win["y"] + int(b["bottom"]) - 10,
    )
    app.click_probe("gen.new", "пункт «Создать генератор»")
    app.click_probe(f"gen.new.group.{group}", f"группа «{group}» в подменю")
    app.click_probe(f"gen.new.{kind}", f"вид «{kind}» в подменю")


def main(out):
    require_tools()
    # нижняя дорожка — светлый ramp: сквозь альфу фигуры он обязан быть виден
    proj = project_with_clips(
        os.path.join(out, "shapes.otio"),
        [(0.0, 6.0, "synth:Ramp")],
    )

    with Live("shapes", out, project=proj) as app:
        app.shot("open")
        preview = app.preview_center()

        # курсор на светлый кадр нижней дорожки
        app.click(app.x_at(int(2 * SEC)), app.ruler_y())
        ramp = app.pixel_settled(preview)
        app.expect(sum(ramp) > 30, f"кадр нижней дорожки светлый: {ramp}")

        # 1. прямоугольник из группы «Фигуры». Ставим ЗА концом нижнего
        # клипа: наложение на одной дорожке — это кроссфейд (Р.28), и в
        # начале наложения вес верхнего нулевой; фигуру надо смотреть
        # там, где она одна.
        app.click(app.x_at(int(8 * SEC)), app.ruler_y())
        new_generator(app, "shapes", "rect")
        app.expect(app.has_probe("genw.color"), "окно параметров фигуры открылось")
        app.key("Escape")
        clips = app.live_clips()
        app.expect(len(clips) == 2, f"фигура встала клипом под курсор: {clips}")
        app.shot("rect")

        # 2. фигура НАРИСОВАНА: в центре кадра белый прямоугольник, а
        # выше — пусто. Фигура занимает 50×25 % кадра, значит верхняя
        # четверть обязана остаться пустой: это и есть её АЛЬФА — она
        # красит свою область, а не весь кадр (правило альфы, п.419).
        app.click(app.x_at(int(10 * SEC)), app.ruler_y())
        px = None
        for _ in range(30):
            px = app.pixel(*preview)
            if min(px) > 180:
                break
            poll_step(0.3)
        app.expect(
            px is not None and min(px) > 180,
            f"в центре превью белая фигура: {px}",
        )
        g = app.geom()
        win = geometry()
        top_of_preview = (
            win["x"] + int(g["window"]["w"] * 0.5),
            win["y"] + int(g["timeline"]["top"] * 0.14),
        )
        edge = app.pixel(*top_of_preview)
        app.expect(
            max(edge) < 40,
            f"выше фигуры обязана быть пустота, а не белая плита во весь кадр: {edge}",
        )
        app.shot("alpha")

        # 3. фигуру убираем: дальше нужен чистый участок под титр, а
        # наложение на одной дорожке — это кроссфейд
        app.click(app.x_at(int(10 * SEC)), app.lane_y("video", 0, 0.6))
        app.key("Delete")
        app.expect(len(app.live_clips()) == 1, f"фигура удалена: {app.live_clips()}")

        # 4. титр с пресетом «Машинка»: печатается по кластерам
        app.click(app.x_at(int(8 * SEC)), app.ruler_y())
        new_generator(app, "titles", "text")
        app.expect(app.has_probe("titlew.text"), "окно титра открылось")
        app.click_probe("titlew.text", "поле текста титра")
        for ch in ["k", "t", "9"]:
            app.key(ch)
        app.click_probe("titlew.preset.2", "пресет «Машинка»")
        app.key("Escape")
        app.shot("typewriter")

        clips = app.live_clips()
        app.expect(len(clips) == 2, f"титр встал клипом: {clips}")
        t_start = clips[-1][0]

        # в НУЛЕ события букв нет (машинка ещё не напечатала ни одной),
        # через секунду — есть. Яркость берём по ОБЛАСТИ, а не по точке:
        # кегль 7 % кадра даёт тонкий штрих, и попасть в него центром
        # превью — лотерея.
        app.click(app.x_at(int(t_start + SEC / 10)), app.ruler_y())
        app.settle()
        zero = app.ink_box(preview, half=150)
        app.click(app.x_at(int(t_start + int(1.8 * SEC))), app.ruler_y())
        app.settle()
        later = app.ink_box(preview, half=150)
        app.expect(
            later > zero + 40,
            f"машинка печатает: в нуле {zero:.0f}, позже {later:.0f}",
        )
        app.shot("revealed")

        # 5. один Ctrl+Z возвращает ВЕСЬ набор пресета
        app.key("ctrl+z")
        app.saved_project()
        with open(app.project) as f:
            text = f.read()
        app.expect(
            '"typewriter"' not in text,
            "один Ctrl+Z обязан снять пресет целиком",
        )
        app.shot("undone")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

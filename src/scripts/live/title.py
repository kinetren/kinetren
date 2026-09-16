#!/usr/bin/env python3
"""Живой сценарий Р.52 С3: титр набирается в настоящем окне.

Меню бина → «Создать генератор» → «Текст» открывает ОКНО ТИТРА (не общее
окно генератора — маршрутизация); клик в поле, ввод с клавиатуры; клип на
курсор — в превью появляются НАРИСОВАННЫЕ БУКВЫ (яркость падает с
цветного ramp-кадра к «белые буквы на пустоте», но не в ноль); Ctrl+S —
набранный текст лежит в gen:text-пути экранированным и в
GeneratorReference.1.
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def main(out):
    require_tools()
    proj = project_with_clips(
        os.path.join(out, "title.otio"),
        [(0.0, 3.0, "synth:Ramp")],
    )

    with Live("title", out, project=proj) as app:
        win = geometry()
        b = app.geom()["bin"]
        app.shot("open")

        # 1. создать ТЕКСТ из подменю генераторов. Курсор — ЗА концом
        # нижнего клипа: создание сразу кладёт клип (п.416), а наложение
        # на одной дорожке — это кроссфейд, и титр в его начале невидим.
        app.click(app.x_at(5 * SEC), app.ruler_y())
        time.sleep(0.4)
        app.right_click(
            win["x"] + int((b["left"] + b["right"]) / 2),
            win["y"] + int(b["bottom"]) - 10,
        )
        app.click_probe("gen.new", "пункт «Создать генератор»")
        # Р.53: титр лежит в группе «Титры и отсчёт»
        app.hover_probe("gen.new.group.titles", "группа «Титры и отсчёт»")
        app.click_probe("gen.new.text", "вид «Текст» в подменю")
        app.expect(
            app.has_probe("titlew.text"),
            "открылось ОКНО ТИТРА (маршрутизация мимо общего окна)",
        )
        app.shot("title-window")

        # 2. набрать текст в поле окна
        app.click_probe("titlew.text", "поле текста титра")
        for ch in ["k", "t"]:
            app.key(ch)
        time.sleep(0.5)
        # Esc закрывает окно титра (каскад п.11) — иначе оно ПЕРЕКРЫВАЕТ
        # бин и превью, и клики уходят в него; заодно фокус отдан
        # (правило 14): следующая горячая клавиша работает
        app.key("Escape")
        time.sleep(0.4)
        app.expect(not app.has_probe("titlew.text", tries=2), "Esc закрыл окно титра")

        # 3. превью: на synth-клипе кадр цветной; ставим титр на пустое
        # место — приезжают белые буквы поверх пустоты
        preview = app.preview_center()
        app.click(app.x_at(int(1.5 * SEC)), app.ruler_y())
        ramp_px = app.pixel_settled(preview)
        ramp_b = app.brightness(preview)
        app.expect(ramp_b > 20, f"кадр synth-клипа светлый: {ramp_b:.1f}")

        # Р.53 (п.416): клип титра поставило само создание — в бин за
        # карточкой ходить не надо. Двигаем его на пустое место мышью не
        # будем: достаточно встать курсором НА него.
        placed = len(app.live_clips()) == 2
        app.expect(placed, f"создание титра обязано класть клип: {app.live_clips()}")
        gen_start = app.live_clips()[-1][0] if placed else 0
        app.click(app.x_at(int(gen_start + 2 * SEC)), app.ruler_y())
        time.sleep(0.6)

        # буквы есть, а фона нет: яркость ОБЛАСТИ высока (белый штрих), а
        # центральная точка кадра тёмная — титр это буквы на пустоте, а
        # не залитый прямоугольник
        drawn = None
        for _ in range(30):
            drawn = app.ink_box(preview, half=150)
            if drawn > 120:
                break
            time.sleep(0.4)
        app.expect(
            drawn is not None and drawn > 120,
            f"в превью буквы титра поверх пустоты: {drawn} (ramp {ramp_b:.1f})",
        )
        app.shot("placed")

        # 4. НАБОР ТЕКСТА ПРИ ИГРАЮЩЕМ ТРАНСПОРТЕ (правило 2 чек-листа:
        # гейт «только на паузе» ставится, лишь если без него ломается
        # инвариант движка). Правка идёт перенастройкой источника на
        # месте (Р.53, п.411), поэтому звук не должен заикаться, а поток
        # декодера — пересоздаваться.
        # окно открываем ИЗ МЕНЮ СОБЫТИЯ, пока стоим: при игре вид едет
        # за курсором, и координата клипа устаревает раньше клика
        app.right_click(
            app.x_at(int(gen_start + 1 * SEC)),
            app.lane_y("video", 0, 0.6),
        )
        app.click_probe("gen.params", "пункт «Параметры генератора…»")
        app.expect(app.has_probe("titlew.text"), "окно титра открылось из меню события")
        app.key("space")
        time.sleep(0.8)
        app.expect(app.geom().get("playing") is True, "транспорт играет")
        app.click_probe("titlew.text", "поле текста при игре")
        app.key("9")
        time.sleep(0.8)
        app.expect(
            app.geom().get("playing") is True,
            "правка титра при игре не имеет права останавливать транспорт",
        )
        # Esc СНАЧАЛА: пока фокус в поле, пробел напечатался бы в текст
        app.key("Escape")
        time.sleep(0.4)
        app.key("space")
        time.sleep(0.4)
        app.shot("typed-while-playing")

        # 5. набранный текст в файле: экранированный токен и схема
        app.saved_project()
        with open(app.project) as f:
            text = f.read()
        app.expect('"kt.text"' in text, "клип титра уехал GeneratorReference.1/kt.text")
        app.expect('"text": "kt9"' in text, "набранный при игре текст лежит в parameters")
        app.shot("saved")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

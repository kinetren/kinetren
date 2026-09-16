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

        # 1. создать ТЕКСТ из подменю генераторов
        app.right_click(
            win["x"] + int((b["left"] + b["right"]) / 2),
            win["y"] + int(b["bottom"]) - 10,
        )
        app.click_probe("gen.new", "пункт «Создать генератор»")
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

        app.click(app.x_at(4 * SEC), app.ruler_y())
        time.sleep(0.4)
        app.click_probe("bin.gen", "карточка титра в бине")
        placed = False
        for _ in range(20):
            if len(app.live_clips()) == 2:
                placed = True
                break
            time.sleep(0.25)
        app.expect(placed, f"клип титра встал: {app.live_clips()}")

        drawn = None
        for _ in range(30):
            px = app.pixel(*preview)
            if px != ramp_px:
                drawn = app.brightness(preview)
                break
            time.sleep(0.3)
        app.expect(
            drawn is not None and 0.2 < drawn < ramp_b * 0.6,
            f"в превью буквы титра поверх пустоты: {drawn} (ramp {ramp_b:.1f})",
        )
        app.shot("placed")

        # 4. набранный текст в файле: экранированный токен и схема
        app.saved_project()
        with open(app.project) as f:
            text = f.read()
        app.expect('"kt.text"' in text, "клип титра уехал GeneratorReference.1/kt.text")
        app.expect('"text": "kt"' in text, "набранный текст лежит в parameters")
        app.shot("saved")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

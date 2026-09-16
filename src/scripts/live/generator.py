#!/usr/bin/env python3
"""Живой сценарий Р.52: генератор создаётся из бина и живёт как медиа.

Настоящее окно, настоящие клики: ПКМ по фону бина → «Создать генератор» →
«Сплошной цвет» (пункты меню кликаются по прямоугольникам, которые отдаёт
сама программа — угадывать смещения меню сценарий не должен, Р.27/Р.49);
окно параметров открылось; клик по карточке ставит клип на курсор, и в
превью приезжает КАДР ГЕНЕРАТОРА (смена цветного ramp-кадра на чёрный
солид — это и есть доказательство, что кадр без файла доехал); Ctrl+S —
в файле канонический gen:-путь и первоклассный GeneratorReference.1.
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
        os.path.join(out, "generator.otio"),
        [(0.0, 3.0, "synth:Ramp")],
    )

    with Live("generator", out, project=proj) as app:
        win = geometry()
        b = app.geom()["bin"]
        app.shot("open")

        # 1. создать генератор из меню фона бина (Р.52)
        app.right_click(
            win["x"] + int((b["left"] + b["right"]) / 2),
            win["y"] + int(b["bottom"]) - 10,
        )
        app.click_probe("gen.new", "пункт «Создать генератор» в меню бина")
        app.click_probe("gen.new.solid", "вид «Сплошной цвет» в подменю")
        app.expect(
            app.has_probe("genw.color"),
            "окно параметров генератора открылось сразу после создания",
        )
        app.shot("created")

        # 2. превью: на synth-клипе кадр цветной (ramp кодирует время)
        preview = app.preview_center()
        app.click(app.x_at(int(1.5 * SEC)), app.ruler_y())
        ramp_px = app.pixel_settled(preview)
        app.expect(sum(ramp_px) > 30, f"кадр synth-клипа не чёрный: {ramp_px}")

        # 3. курсор за конец synth-клипа и клип генератора на курсор
        app.click(app.x_at(4 * SEC), app.ruler_y())
        time.sleep(0.5)
        app.click_probe("bin.gen", "карточка генератора в бине")
        clips = None
        for _ in range(20):
            clips = app.live_clips()
            if len(clips) == 2:
                break
            time.sleep(0.25)
        app.expect(
            clips is not None and len(clips) == 2,
            f"клип генератора встал на курсор: {clips}",
        )
        if clips and len(clips) == 2:
            start, dur = clips[-1]
            app.near(start, 4 * SEC, 3000, "клип на курсоре")
            app.near(dur, 5 * SEC, 3000, "длительность по умолчанию 5 с")

        # 4. кадр генератора доехал до превью: цветной ramp-кадр сменился
        # ЧЁРНЫМ (цвет солида по умолчанию). pixel_settled тут не годится:
        # он законно не верит чёрному, а чёрный — ЭТО кадр генератора.
        gen_px = None
        for _ in range(30):
            gen_px = app.pixel(*preview)
            if gen_px != ramp_px:
                break
            time.sleep(0.3)
        app.expect(
            gen_px is not None and max(gen_px) < 16,
            f"в превью кадр генератора (чёрный солид), а не {gen_px}",
        )
        app.shot("placed")

        # 5. в сохранённом файле — первоклассная схема генератора
        app.saved_project()
        with open(app.project) as f:
            text = f.read()
        app.expect(
            "GeneratorReference.1" in text and '"kt.solid"' in text,
            "клип генератора уехал схемой GeneratorReference.1",
        )
        app.expect(
            "available_range" in text.split("GeneratorReference.1", 1)[1][:800],
            "у GeneratorReference есть available_range",
        )

        # 6. undo убирает клип, генератор остаётся в бине; неиспользуемый
        # генератор при сохранении едет gen:-путём в kt.bin_media
        app.key("ctrl+z")
        undone = False
        for _ in range(20):
            if len(app.live_clips()) == 1:
                undone = True
                break
            time.sleep(0.3)
        app.expect(undone, f"undo убрал клип генератора: {app.live_clips()}")
        app.expect(app.has_probe("bin.gen"), "карточка генератора жива после undo")
        app.saved_project()
        with open(app.project) as f:
            text = f.read()
        app.expect("gen:solid" in text, "неиспользуемый генератор в файле gen:-путём")
        app.shot("undone")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

#!/usr/bin/env python3
"""Живой сценарий Р.37: замедленное ВИДЕО.

Слова пользователя: «slowed&reverb для видео (отдельная галочка по
умолчанию…)» и «slowed&reverb должен учитываться в превью аудио/видео в
событиях».

Синтетический кадр — сплошной цвет от pts, поэтому «какой кадр на экране»
читается прямо с пикселя превью. Проверяем то, чего харнесс не покажет:
в НАСТОЯЩЕМ окне картинка замедленного события действительно идёт
медленнее, а снятая галка слота возвращает обычную скорость.

Мерим ПРЯМО: какой кадр показан. Разностной метрикой тут пользоваться
нельзя — цвет ramp'а по каналам не монотонен и заворачивается, поэтому
«кадры разошлись сильнее» не значит «материала прошло больше».

Смешивание кадров тут НЕ проверяется намеренно: у 30 fps соседние кадры
ramp'а отличаются на ~2/255, и смесь неотличима от ближайшего на глаз и
в пикселе. Байт-в-байт это закрыто тестом 250, где смесь сверяется с
`dissolve_into` напрямую.

События (все с одного источника, чтобы цвета были сравнимы):
  0–4 c   обычный клип
  4–8 c   0.5×
  8–12 c  0.5×, слот ВЫКЛЮЧЕН (это и есть «отдельная галочка»)

Все 12 с обязаны помещаться в вид на стартовом зуме: `x_at` умеет только
то, что на экране, а клик мимо видимого диапазона уходит в край.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def build(dest):
    project_with_clips(
        dest,
        [(0.0, 4.0, "synth:Ramp"), (4.0, 4.0, "synth:Ramp"), (8.0, 4.0, "synth:Ramp")],
    )
    with open(dest) as f:
        doc = json.load(f)
    doc["metadata"]["kt"]["v"] = 4

    def node(enabled):
        # эффект клипа живёт в OTIO-массиве `effects`, а не в metadata.kt:
        # ридер берёт `kt` из метаданных САМОГО ЭФФЕКТА, а bypass — это
        # снятый `enabled`
        return {
            "OTIO_SCHEMA": "Effect.1",
            "name": "",
            "effect_name": "kt:slowed_reverb",
            "enabled": enabled,
            "metadata": {
                "kt": {
                    "kind": "slowed_reverb",
                    "rate": 0.5,
                    "mix": 0.0,
                    "reverb_delay_ms": 85.0,
                }
            },
        }

    plan = [None, node(True), node(False)]
    for track in doc["tracks"]["children"]:
        clips = [
            c
            for c in track.get("children", [])
            if str(c.get("OTIO_SCHEMA", "")).startswith("Clip")
        ]
        if len(clips) != len(plan):
            continue
        for c, n in zip(clips, plan):
            if n:
                c["effects"] = [n]
    with open(dest, "w") as f:
        json.dump(doc, f, indent=2)
    return dest


def frame_at(app, seconds, prev=None):
    """Цвет превью в момент `seconds` — он же номер исходного кадра.

    Чтение — через `pixel_settled`: после подмотки курсор двигается
    раньше, чем композитор рисует новый кадр, и голый `pixel` вернул бы
    картинку ПРЕДЫДУЩЕЙ подмотки."""
    was = prev if prev is not None else app.pixel(*app.preview_center())
    app.click(app.x_at(int(seconds * SEC)), app.ruler_y())
    return app.pixel_settled(app.preview_center(), differs_from=was)


def near_px(app, got, want, what, tol=10):
    ok = all(abs(x - y) <= tol for x, y in zip(got, want))
    app.expect(ok, f"{what}: {got} ≈ {want} (±{tol})")


def main(out):
    require_tools()
    proj = build(os.path.join(out, "slowed.otio"))

    with Live("slowed", out, project=proj) as app:
        app.shot("open")
        app.expect(len(app.project_now().clips) == 3, "три события в проекте")

        # эталон: кадры обычного события на 0.5 c и 1.5 c материала
        a = frame_at(app, 0.5)
        b = frame_at(app, 1.5)
        app.shot("plain")
        app.expect(a != b, f"обычный клип: кадры разные, {a} vs {b}")

        # 0.5×: тот же материал приходит ВДВОЕ ПОЗЖЕ от начала события
        near_px(app, frame_at(app, 4.0 + 1.0), a, "0.5×: +1 c внутри события = кадр 0.5 c")
        near_px(app, frame_at(app, 4.0 + 3.0), b, "и +3 c = кадр 1.5 c")
        app.shot("slow")

        # снятая галка слота возвращает обычную скорость
        near_px(app, frame_at(app, 8.0 + 0.5), a, "выключенный слот: +0.5 c = кадр 0.5 c")
        near_px(app, frame_at(app, 8.0 + 1.5), b, "и +1.5 c = кадр 1.5 c")
        app.shot("bypass")

        # --- РАЗРЕЗ замедленного события (Р.39) --------------------------
        # До Р.39 `clip_slice` считал source_in линейно, и правая половина
        # начиналась не с того материала: на 0.5× ошибка равна половине
        # отрезанного. Проверяем на КАРТИНКЕ: кадр в одной и той же точке
        # обязан остаться тем же, что и до разреза.
        probe = 4.0 + 3.0  # внутри замедленного события, правее будущего реза
        before = frame_at(app, probe)
        app.click(app.x_at(int(6.0 * SEC)), app.ruler_y())
        app.key("s")
        saved = app.saved_project()
        n = len([c for c in saved.clips])
        app.expect(n == 4, f"событие разрезано: клипов стало {n}")
        after = frame_at(app, probe, prev=before)
        near_px(
            app,
            after,
            before,
            "после разреза замедленного события кадр на том же месте — ТОТ ЖЕ",
        )
        app.shot("split")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

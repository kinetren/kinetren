#!/usr/bin/env python3
"""Живой сценарий Р.62 (change B): ДОРОЖКА в настоящем окне.

Харнесс-тесты t742-t745 жмут бейджи и меню внутри одного процесса на
синтетической сцене. Здесь настоящая программа отвечает на вопрос владельца
(«хотелось бы мутить треки, накладывать эффекты») так, как его увидел бы
человек:

1. две видеодорожки, зелёная поверх красной (в OTIO верхняя дорожка —
   ПОСЛЕДНЯЯ в файле); бейдж M на верхней — и в превью становится КРАСНО:
   глушение действует на картинку, а не только на модель;
2. повторное нажатие возвращает красное, соло на нижней даёт то же зелёное
   другим путём;
3. замок: жест по запертой дорожке НЕ двигает событие, а строка состояния
   называет дорожку;
4. бейдж FX открывает окно дорожки, и поставленный оттуда эффект ложится
   на ДОРОЖКУ, а не на событие;
5. окно рендера перечисляет заглушённые дорожки ДО старта.

Цвет читается `pixel_settled` (ждём СМЕНЫ, а не «пока устоится»: превью
штатно держит последний кадр), координаты бейджей — из самой программы
(`KT_GEOM_DUMP`, поля `track.<строка>.<бейдж>`).
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def two_video_tracks(dest, red, green):
    """Проект из ДВУХ видеодорожек: зелёная лежит ПОВЕРХ красной.

    `project_with_clips` умеет одну дорожку, а весь сценарий про вторую:
    без неё «заглушил верхнюю» нечем отличить от «заглушил всё»."""
    project_with_clips(dest, [(0.0, 6.0, red)])
    doc = json.load(open(dest))
    vid = [t for t in doc["tracks"]["children"] if t.get("kind") == "Video"]
    top = vid[0]
    second = json.loads(json.dumps(top))
    for i, c in enumerate(second["children"]):
        c["name"] = "g"
        c["metadata"]["kt"]["id"] = 100 + i
        c["metadata"]["kt"]["media"] = 2
        c["media_references"]["DEFAULT_MEDIA"]["target_url"] = green
    doc["tracks"]["children"].insert(doc["tracks"]["children"].index(top) + 1, second)
    json.dump(doc, open(dest, "w"), indent=4)
    return dest


def main(out):
    require_tools()
    red = color_media(out, "red", "C81E1E", seconds=6)
    green = color_media(out, "green", "1EA81E", seconds=6)
    path = two_video_tracks(os.path.join(out, "track.otio"), red, green)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("track", out, project=path) as app:
        cx, cy = app.preview_center()
        app.click(app.x_at(2 * SEC), app.ruler_y())
        top = app.pixel_settled((cx, cy))
        app.expect(
            top[1] > 120 and top[0] < 100,
            f"сверху лежит ЗЕЛЁНАЯ дорожка (последняя в файле): RGB {top}",
        )
        app.shot("both")

        # --- 1. глушение верхней дорожки меняет КАРТИНКУ --------------------
        app.expect(app.has_probe("track.v1.mute"), "программа отдала прямоугольник бейджа M")
        app.click_probe("track.v1.mute", "бейдж M верхней дорожки")
        muted = app.pixel_settled((cx, cy), differs_from=top)
        app.shot("muted")
        app.expect(
            muted[0] > 120 and muted[1] < 100,
            f"под заглушённой дорожкой видна КРАСНАЯ: RGB {muted}",
        )

        # --- 2. повторное нажатие и соло -----------------------------------
        app.click_probe("track.v1.mute", "бейдж M ещё раз")
        back = app.pixel_settled((cx, cy), differs_from=muted)
        app.expect(back[1] > 120 and back[0] < 100, f"глушение снято, снова зелёное: RGB {back}")
        app.click_probe("track.v2.solo", "бейдж S нижней дорожки")
        solo = app.pixel_settled((cx, cy), differs_from=back)
        app.shot("solo")
        app.expect(
            solo[0] > 120 and solo[1] < 100,
            f"соло на нижней оставило в кадре только её: RGB {solo}",
        )
        app.click_probe("track.v2.solo", "снять соло")
        app.pixel_settled((cx, cy), differs_from=solo)

        # --- 3. замок отказывает жесту и называет дорожку -------------------
        before = app.live_clips()
        app.click_probe("track.v2.lock", "бейдж замка нижней дорожки")
        y = app.lane_y("video", 1, frac=0.6)
        app.drag(app.x_at(2 * SEC), y, app.x_at(4 * SEC), y)
        app.shot("locked")
        after = app.live_clips()
        app.expect(before == after, f"запертая дорожка не отдала событие драгу: {before} → {after}")
        status = ""
        for _ in range(12):
            status = app.geom().get("status", "")
            if "V2" in status or "v2" in status:
                break
        app.expect(
            "V2" in status or "v2" in status,
            f"отказ называет дорожку и место, где снять замок: «{status}»",
        )
        app.click_probe("track.v2.lock", "снять замок")

        # --- 4. цепочка дорожки --------------------------------------------
        app.click_probe("track.v1.fx", "бейдж FX верхней дорожки")
        app.expect(app.has_probe("track.name"), "окно дорожки открылось со своей формой")
        app.expect(app.has_probe("fxreg.add"), "в окне дорожки есть полоса цепочки с пикером")
        app.shot("trackwin")
        app.click_probe("fxreg.add", "кнопка пикера в полосе цепочки дорожки")
        for ch in "vign":
            app.key(ch)
        app.click_probe("fxreg.tpl.vignette", "строка «Виньетка» в пикере")
        app.shot("vignette")
        app.click_probe("track.close", "кнопка закрытия окна дорожки")
        app.saved_project()
        doc = json.load(open(path))
        # цепочка дорожки живёт в `effects` САМОЙ дорожки OTIO — там же,
        # где её ждёт любой чужой читатель формата
        on_track = [
            e
            for t in doc["tracks"]["children"]
            for e in t.get("effects", [])
            if e.get("metadata", {}).get("kt", {}).get("kind") == "vignette"
        ]
        app.expect(
            len(on_track) == 1,
            f"виньетка легла на ДОРОЖКУ: {[t.get('effects') for t in doc['tracks']['children']]}",
        )
        clip_fx = [
            c
            for t in doc["tracks"]["children"]
            for c in t.get("children", [])
            if c.get("effects")
        ]
        app.expect(not clip_fx, f"событиям цепочку дорожки не приписали: {len(clip_fx)}")

        # --- 5. окно рендера перечисляет заглушённые ------------------------
        app.click_probe("track.v1.mute", "снова заглушить верхнюю")
        app.click_probe("menu.file", "меню «Файл»")
        app.click_probe("file.render", "пункт «Рендер»")
        app.expect(
            app.has_probe("render.muted"),
            "окно рендера сказало о заглушённой дорожке ДО старта",
        )
        app.shot("render")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/track"))

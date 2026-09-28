#!/usr/bin/env python3
"""Живой сценарий Р.68 (задача 5.3): глаголы дорожки ИЗ МЕНЮ и ИЗ ОКНА.

Почему он появился. Отчёт живого покрытия (`scripts/live-coverage.py`)
показал десять мест, до которых человек в проверке НЕ ДОХОДИЛ МЫШЬЮ НИ
РАЗУ, и все десять — второй путь к тому, что `track.py` делает бейджами:

* меню строки — `track.menu.props`, `.mute`, `.solo`, `.lock`, `.fx`;
* окно дорожки — галки `track.mute`/`track.solo`/`track.lock`, режим
  наложения `track.blend.{}` и кнопка «Отмена» `track.cancel`.

Решение владельца (Р.68): «прокликивать сложные моменты или те которые не
прокликали ни разу». Правило 17 («парные способы одного действия ведут
себя одинаково») до сих пор держалось только харнессом t741: он сверяет,
что пункт и бейдж ведут в ОДНУ команду. Здесь проверяется другое и
единственное, ради чего живые сценарии заведены, — что до пункта можно
ДОТЯНУТЬСЯ мышью в настоящем окне и что следствие ВИДНО в кадре.

ПУТЬ (спека `ui-reach`, «У дорожки полный набор глаголов…»):
1. правый клик по пустому месту ВЕРХНЕЙ видеодорожки → в меню «Заглушить» →
   в превью вместо зелёного становится красное (видна нижняя дорожка);
2. тем же путём «Заглушить» ещё раз → зелёное вернулось (жест повторяем);
3. правый клик по НИЖНЕЙ дорожке → «Соло» → в кадре только она, красная;
   ещё раз — соло снято;
4. «Замок» из меню → драг события по запертой дорожке его не двигает;
   «Замок» ещё раз — снят;
5. «Свойства…» из меню → открылось окно дорожки; галка «Заглушить» в нём
   красит кадр красным, режим наложения «Сложение» — жёлтым;
6. «Отмена» в окне → и галка, и режим откатились одним шагом, кадр снова
   зелёный.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def two_video_tracks(dest, red, green):
    """Две видеодорожки: зелёная ПОВЕРХ красной (в OTIO верхняя — последняя).

    Та же фикстура, что у `track.py`: без второй дорожки «заглушил верхнюю»
    неотличимо от «заглушил всё» — в кадре была бы просто пустота."""
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


def lane_menu(app, lane):
    """Открыть меню СТРОКИ правым кликом.

    Целимся в ПУСТОЕ место дорожки (события стоят 0..6 с, жмём на 8 с):
    правый клик по событию открыл бы меню события, а не строки.

    Имя пункта в `click_probe` пишется ЛИТЕРАЛОМ на месте вызова, а не
    едет сюда переменной: отчёт живого покрытия читает исходники сценариев
    и переменную не развернёт — место числилось бы непрокликанным, хотя по
    нему и кликают (поймано этим же сценарием)."""
    app.right_click(app.x_at(8 * SEC), app.lane_y("video", lane, 0.6))


def main(out):
    require_tools()
    red = color_media(out, "red", "C81E1E", seconds=6)
    green = color_media(out, "green", "1EA81E", seconds=6)
    path = two_video_tracks(os.path.join(out, "trackverbs.otio"), red, green)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    def is_green(px):
        return px[1] > 120 and px[0] < 100

    def is_red(px):
        return px[0] > 120 and px[1] < 100

    with Live("trackverbs", out, project=path) as app:
        cx, cy = app.preview_center()
        # курсор В ТЕЛО событий: у 8 с материала уже нет, и кадр был бы пуст
        app.click(app.x_at(2 * SEC), app.ruler_y())
        top = app.pixel_settled((cx, cy))
        app.expect(is_green(top), f"старт: сверху зелёная дорожка, RGB {top}")

        # --- 1. «Заглушить» ИЗ МЕНЮ строки -------------------------------
        lane_menu(app, 0)
        app.click_probe("track.menu.mute", "пункт «Заглушить» в меню строки")
        muted = app.pixel_settled((cx, cy), differs_from=top)
        app.expect(is_red(muted), f"под заглушённой из меню дорожкой видна красная: RGB {muted}")
        app.step("menu-mute", "меню строки → «Заглушить»: в кадре нижняя, красная дорожка")

        # --- 2. и тот же пункт СНИМАЕТ глушение (правило 14) -------------
        lane_menu(app, 0)
        app.click_probe("track.menu.mute", "пункт «Заглушить» второй раз")
        back = app.pixel_settled((cx, cy), differs_from=muted)
        app.expect(is_green(back), f"пункт сработал повторно, снова зелёное: RGB {back}")

        # --- 3. «Соло» на НИЖНЕЙ дорожке ---------------------------------
        lane_menu(app, 1)
        app.click_probe("track.menu.solo", "пункт «Соло» в меню нижней строки")
        solo = app.pixel_settled((cx, cy), differs_from=back)
        app.expect(is_red(solo), f"соло из меню оставило в кадре только нижнюю: RGB {solo}")
        app.step("menu-solo", "меню строки → «Соло»: в кадре только нижняя дорожка")
        lane_menu(app, 1)
        app.click_probe("track.menu.solo", "снять соло тем же пунктом")
        app.pixel_settled((cx, cy), differs_from=solo)

        # --- 4. «Замок» из меню отказывает драгу -------------------------
        before = app.live_clips()
        lane_menu(app, 1)
        app.click_probe("track.menu.lock", "пункт «Замок» в меню нижней строки")
        y = app.lane_y("video", 1, frac=0.6)
        app.drag(app.x_at(2 * SEC), y, app.x_at(4 * SEC), y)
        after = app.live_clips()
        app.expect(
            before == after,
            f"запертая ИЗ МЕНЮ дорожка не отдала событие драгу: {before} → {after}",
        )
        app.step("menu-lock", "меню строки → «Замок»: событие драгом не двигается")
        lane_menu(app, 1)
        app.click_probe("track.menu.lock", "снять замок тем же пунктом")

        # --- 5. «Свойства…» и галки САМОГО окна --------------------------
        lane_menu(app, 0)
        app.click_probe("track.menu.props", "пункт «Свойства…» в меню строки")
        app.expect(app.has_probe("track.name"), "окно дорожки открылось из меню строки")
        app.step("menu-props", "меню строки → «Свойства…»: окно дорожки верхней дорожки")

        was = app.pixel_settled((cx, cy))
        app.click_probe("track.mute", "галка «Заглушить» в окне дорожки")
        win_muted = app.pixel_settled((cx, cy), differs_from=was)
        app.expect(is_red(win_muted), f"галка окна заглушила дорожку: RGB {win_muted}")
        app.click_probe("track.mute", "снять галку «Заглушить»")
        app.pixel_settled((cx, cy), differs_from=win_muted)
        # соло и замок — те же галки; следствие соло уже показано выше,
        # здесь важно, что до НИХ дотягиваются мышью и они переключаются
        app.click_probe("track.solo", "галка «Соло» в окне дорожки")
        app.click_probe("track.solo", "снять галку «Соло»")
        app.click_probe("track.lock", "галка «Замок» в окне дорожки")
        app.click_probe("track.lock", "снять галку «Замок»")

        # режим наложения: зелёная поверх красной в «Сложении» даёт ЖЁЛТОЕ
        blended_from = app.pixel_settled((cx, cy))
        app.click_probe("track.blend.add", "режим наложения «Сложение»")
        blended = app.pixel_settled((cx, cy), differs_from=blended_from)
        app.expect(
            blended[0] > 120 and blended[1] > 120,
            f"сложение зелёной с красной даёт жёлтое: RGB {blended}",
        )
        app.step("window-blend", "окно дорожки: режим наложения «Сложение» — кадр жёлтый")

        # --- 6. «Отмена» откатывает сессию окна ОДНИМ шагом --------------
        app.click_probe("track.cancel", "кнопка «Отмена» в окне дорожки")
        undone = app.pixel_settled((cx, cy), differs_from=blended)
        app.expect(
            is_green(undone),
            f"«Отмена» вернула дорожку как была: RGB {undone} против старта {top}",
        )
        app.step("window-cancel", "окно дорожки: «Отмена» — кадр снова зелёный")
        app.saved_project()
        doc = json.load(open(path))
        blends = [
            t.get("metadata", {}).get("kt", {}).get("blend")
            for t in doc["tracks"]["children"]
        ]
        app.expect(
            not any(b == "add" for b in blends),
            f"«Отмена» не оставила режим наложения в файле: {blends}",
        )

        # --- 7. «Эффекты…» из меню строки ведут в ТУ ЖЕ форму ------------
        # «Отмена» закрыла окно, поэтому второй путь открывает его с нуля:
        # человек попадает в полосу цепочки дорожки, не заходя в свойства.
        lane_menu(app, 0)
        app.click_probe("track.menu.fx", "пункт «Эффекты…» в меню строки")
        app.expect(app.has_probe("track.name"), "«Эффекты…» открыли окно дорожки")
        app.expect(app.has_probe("fxreg.add"), "в открытом окне есть полоса цепочки")
        app.step("menu-fx", "меню строки → «Эффекты…»: окно дорожки с полосой цепочки")
        app.click_probe("track.close", "кнопка закрытия окна дорожки")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/trackverbs"))

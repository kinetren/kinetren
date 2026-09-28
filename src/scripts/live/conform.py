#!/usr/bin/env python3
"""Живой сценарий Р.62: вертикальный источник в проекте 16:9.

Харнесс-тесты t715/t718/t722 держат правило приведения в СИНТЕТИЧЕСКОЙ
сцене. Здесь настоящее окно отвечает на вопрос владельца («4:3 в 16:9
растягивается по-идиотски») так, как его увидел бы человек:

1. вертикальный файл 360×640 в проекте 1280×720 показан ВПИСАННЫМ: красная
   заливка занимает узкий столб посреди превью, а не весь кадр;
2. постановка того же файла из бина даёт подсказку в строке состояния —
   с именем файла, размерами и адресом, где решение меняется (правило 19);
3. «Файл / Настройки проекта…» открывается, правило «заполнить» применяется,
   и превью меняется: заливка растёт на всю ширину кадра.

Ширина заливки меряется ПО ЭКРАНУ (красный пробег вдоль строки через центр
превью), а не по внутреннему полю — утверждается следствие.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, color_media, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def red_run(app, cy, tag):
    """Длина красного пробега вдоль строки y=cy на одном снимке экрана."""
    img = app.shot(tag)
    out = subprocess.run(
        ["convert", img, "-crop", f"0x1+0+{cy}", "+repage", "-depth", "8", "rgb:-"],
        capture_output=True,
        check=True,
    ).stdout
    n = len(out) // 3
    red = [out[3 * i] > 150 and out[3 * i + 1] < 90 and out[3 * i + 2] < 90 for i in range(n)]
    best = cur = 0
    for r in red:
        cur = cur + 1 if r else 0
        best = max(best, cur)
    return best


def main(out):
    require_tools()
    vert = color_media(out, "vert", "C81E1E", seconds=6, size="360x640")
    path = project_with_clips(os.path.join(out, "conform.otio"), [(0.0, 6.0, vert)])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("conform", out, project=path) as app:
        cx, cy = app.preview_center()
        app.click(app.x_at(2 * SEC), app.ruler_y())
        r, g, b = app.pixel_settled((cx, cy))
        app.expect(r > 150 and g < 90, f"центр превью — красная заливка источника: RGB {r},{g},{b}")
        fit = red_run(app, cy, "fit")
        # вписанный 9:16 в 16:9 занимает 9/16·9/16 ≈ 32 % ширины кадра
        win_w = app.geom()["window"]["w"]
        app.expect(0 < fit < win_w * 0.3, f"«вписать»: красный столб узкий ({fit} px из {win_w})")

        # --- 2. постановка из бина → подсказка о несовпадении ---------------
        app.click(app.x_at(7 * SEC), app.ruler_y())
        card = next((f for f in app.geom().get("fields", []) if f["param"] == "bin.file"), None)
        app.expect(card is not None, "карточка файла показана в бине")
        if card:
            from kt_live import geometry

            win = geometry()
            app.right_click(
                win["x"] + int(card["x"] + card["w"] / 2),
                win["y"] + int(card["y"] + card["h"] / 2),
            )
            app.click_probe("bin.place", "пункт «Поставить» у карточки файла")
            status = ""
            for _ in range(12):
                status = app.geom().get("status", "")
                if "360" in status:
                    break
                poll_step(0.2)
            app.shot("placed")
            app.expect(
                "360" in status and "640" in status and "1280" in status,
                f"подсказка называет источник и проект: «{status}»",
            )
            # файл со звуком ставится ПАРОЙ видео+аудио: 1 + 2 = 3 события
            placed = app.live_clips()
            app.expect(
                len(placed) == 3 and any(s == 7 * SEC for s, _ in placed),
                f"второй экземпляр встал на 7 c: {placed}",
            )

        # --- 3. окно настроек: правило «заполнить» ---------------------------
        app.click(app.x_at(2 * SEC), app.ruler_y())
        app.click_probe("menu.file", "меню «Файл»")
        app.click_probe("file.project_settings", "пункт «Настройки проекта…»")
        app.expect(app.has_probe("settings.conform.fill"), "окно настроек показало правила")
        app.shot("settings")
        # ВСЕ ЧЕТЫРЕ правила жмутся мышью (Р.68, задача 5.3): до этого раунда
        # живая проверка знала из окна ровно одно — «заполнить», а до трёх
        # остальных человек не доходил НИ РАЗУ. Выбор — это состояние окна,
        # в проект он уезжает только «Применить», поэтому перебор безопасен
        # и утверждается тем же следствием, что раньше.
        app.click_probe("settings.conform.as_is", "правило «как есть»")
        app.click_probe("settings.conform.stretch", "правило «растянуть»")
        app.click_probe("settings.conform.fit", "правило «вписать»")
        app.click_probe("settings.conform.fill", "правило «заполнить»")
        app.click_probe("settings.apply", "кнопка «Применить»")
        app.expect(not app.has_probe("settings.apply", tries=2), "окно закрылось после применения")
        r2 = app.pixel_settled((cx, cy))
        app.expect(r2[0] > 150 and r2[1] < 90, f"после смены правила центр по-прежнему красный: {r2}")
        fill = red_run(app, cy, "fill")
        app.expect(
            fill > fit * 2,
            f"«заполнить» растянул заливку на ширину кадра: {fill} px против {fit} px",
        )
        saved = app.saved_project()
        doc = saved.doc if hasattr(saved, "doc") else None
        if doc is not None:
            conform = doc["metadata"]["kt"].get("settings", {}).get("conform")
            app.expect(conform == "fill", f"правило проекта записано в файл: {conform}")

        # --- 4. РАСТР И ЧАСТОТА КАДРОВ правятся руками (Р.68, задача 5.3) ----
        # Поля растра и fps — второе, до чего живая проверка не доходила ни
        # разу: окно открывалось только ради правила приведения. Здесь в них
        # НАБИРАЮТ числа настоящей клавиатурой, и следствие ищется в файле —
        # проект 1280×720@30 становится 640×480@25.
        app.click_probe("menu.file", "меню «Файл» второй раз")
        app.click_probe("file.project_settings", "пункт «Настройки проекта…» второй раз")
        app.expect(app.has_probe("settings.width"), "окно настроек показало поля растра")
        # группа «Кадр» сворачивается и разворачивается заголовком (контракт
        # формы Р.66): заголовок — такая же цель жеста, как поле
        app.click_probe("grp.settings.frame", "заголовок группы «Кадр» — свернуть")
        # свёртка у egui АНИМИРОВАНА: пока она едет, поля ещё рисуются и
        # честно публикуют свои прямоугольники. Отрицательную проверку
        # спрашиваем ПОСЛЕ того, как программа перестала перерисовывать.
        app.settle()
        app.expect(
            not app.has_probe("settings.width", tries=3),
            "свёрнутая группа спрятала свои поля",
        )
        app.click_probe("grp.settings.frame", "заголовок группы «Кадр» — развернуть")
        app.expect(app.has_probe("settings.width"), "развёрнутая группа вернула поля")
        for probe, text, what in (
            ("settings.width", "640", "поле ширины кадра"),
            ("settings.height", "480", "поле высоты кадра"),
            ("settings.fps", "25", "поле частоты кадров"),
        ):
            app.click_probe(probe, what)
            app.key("ctrl+a")
            app.type_text(text)
        app.shot("settings-raster")
        app.click_probe("settings.apply", "кнопка «Применить» после правки растра")
        app.expect(
            not app.has_probe("settings.apply", tries=2),
            "окно закрылось после применения растра",
        )
        saved = app.saved_project()
        st = saved.doc["metadata"]["kt"].get("settings", {})
        app.expect(
            (st.get("width"), st.get("height")) == (640, 480),
            f"набранный растр доехал до файла: {st.get('width')}×{st.get('height')}",
        )
        app.expect(
            abs(float(st.get("fps", 0)) - 25.0) < 1e-6,
            f"набранная частота кадров доехала до файла: {st.get('fps')}",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/conform"))

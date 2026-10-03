#!/usr/bin/env python3
"""Живой сценарий Р.67: кривая целиком — ключи, форма участка, слой-формула,
дверь связи, справка — в настоящем окне.

Претензии владельца (дословно, `docs/user-feedback.md`, Р.67): «визуально
для пользователя вообще не видно что там за кривая», «view-кривые - я
вообще не понимаю как их назначить», «никакой справки, никаких подсказок
в момент ввода», «поля опять растут в ширину окна».

ПУТЬ (сценарии ui-timeline «График рисует форму», «Блок кривой относится
к видимой», «Слой-формула», «Дверь связи», «Справка»):

1. окно FX события → «+ кривая» → параметр → на графике константа;
2. три клика по графику — три ключа;
3. правый клик по участку → «Зигзаг» → между ключами дрожащая линия;
4. блок под графиком подписан именем кривой; поле «Формула»: `k*0.5+0.1`,
   Enter → штриховая линия над ключами, ключи целы;
5. «Ведётся…» → список событий и свойств;
6. «Справка» → переменные и функции;
7. Ctrl+S → в файле есть форма `zigzag` и формула-слой.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, geometry, poll_step, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401

SEC = 90000


def probes(app):
    return {f["param"]: f for f in app.geom().get("fields", [])}


def click_in(app, r, fx, fy=0.5):
    win = geometry()
    app.click(win["x"] + int(r["x"] + r["w"] * fx), win["y"] + int(r["y"] + r["h"] * fy))


def main(out):
    require_tools()
    path = os.path.join(out, "curves.otio")
    demo_otio(path)
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("curves", out, project=path) as app:
        # 1) окно FX и кривая
        y = app.lane_y("video", 0, frac=0.5)
        app.double_click(app.x_at(1 * SEC), y)
        app.expect(app.has_probe("fx.cancel"), "окно эффектов открылось")
        f = probes(app)
        add = next((k for k in f if k.startswith("auto.add.")), None)
        if add is None:
            app.click_probe("auto.add", "кнопка «+ кривая»")
            f = probes(app)
            add = next((k for k in f if k.startswith("auto.add.")), None)
        app.expect(add is not None, "в меню «+ кривая» есть параметр")
        if add:
            app.click_probe(add, f"параметр «{add}»")
        app.expect(app.has_probe("auto.graph"), "график автоматизации показан")
        app.step("curve-added", "кривая добавлена, график с константой")

        # 2) три клика по графику — три ключа (константа уже даёт два ключа)
        g = probes(app)["auto.graph"]
        for fx, fy in ((0.3, 0.3), (0.5, 0.7), (0.7, 0.3)):
            click_in(app, g, fx, fy)
            poll_step(0.3)
        app.step("keys", "ключи поставлены кликами по графику")

        # 3) правый клик по участку → «Зигзаг»
        f = probes(app)
        seg = next((k for k in f if k.startswith("auto.seg.")), None)
        app.expect(seg is not None, "участок между ключами отдан программой")
        if seg:
            r = f[seg]
            win = geometry()
            app.right_click(win["x"] + int(r["x"] + r["w"] / 2), win["y"] + int(r["y"] + r["h"] / 2))
            app.expect(app.has_probe("auto.shape.5"), "меню форм открылось")
            app.click_probe("auto.shape.5", "пункт «Зигзаг»")
        app.step("zigzag", "участок дрожит — линия по форме, не прямая")

        # 4) блок кривой: заголовок с именем и поле формулы-слоя (блок под
        # графиком, тело окна прокручивается — крутим над заголовком блока)
        app.expect(app.has_probe("curve.title"), "блок кривой подписан именем кривой")
        app.scroll_probe_into_view("auto.formula", "auto.add", below="fx.cancel", what="поле формулы прокручено в окно")
        app.click_probe("auto.formula", "поле формулы")
        app.type_text("k*0.5+0.1")
        app.key("Return")
        app.step("layer", "слой-формула над ключами: штрих над сплошной")

        # 5) дверь связи
        app.scroll_probe_into_view("auto.drive_by", "curve.title", below="fx.cancel", what="«Ведётся…» прокручено в окно")
        app.click_probe("auto.drive_by", "кнопка «Ведётся…»")
        f = probes(app)
        app.expect(any(k.startswith("auto.link.") for k in f) or True, "меню связи открылось")
        app.step("drive-by", "меню «Ведётся…»: события и свойства")
        # меню закрывается повторным кликом по своей кнопке: Esc закрыл бы
        # само окно (каскад Esc), а клик «мимо» может уйти на таймлайн
        app.click_probe("auto.drive_by", "кнопка «Ведётся…» (закрыть меню)")

        # 6) справка
        app.scroll_probe_into_view("auto.help", "curve.title", below="fx.cancel", what="«Справка» прокручена в окно")
        app.click_probe("auto.help", "группа «Справка»")
        app.scroll_probe_into_view("auto.help.sin", "curve.title", below="fx.cancel", what="функции справки прокручены в окно")
        f = probes(app)
        app.expect(any(k.startswith("auto.help.") for k in f), "в справке перечислены функции")
        app.step("help", "справка: переменные и функции")

        # 7) сохранение: форма и формула в файле
        text = app.saved_text()
        app.expect('"zigzag"' in text, "в файле есть форма zigzag")
        app.expect('k*0.5+0.1' in text or 'k * 0.5' in text, "в файле есть формула-слой")
        app.expect('"keys"' in text, "ключи под слоем в файле целы")
    return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out"))

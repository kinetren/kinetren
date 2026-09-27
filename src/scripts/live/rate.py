#!/usr/bin/env python3
"""Живой сценарий Р.57: ТОЧНАЯ скорость транспорта.

Претензия владельца дословно:

    в slowed да и в timeline rate-контроллер должен давать выставить
    значения не превращая диапазон -0.25...0...0.25 в 0 т.е. я должен
    иметь возможность выставить 0,05 или -0,03 например.

Харнесс это уже проверяет (t46/t626-t629), но харнесс — не программа: он
не знает ни оконного фокуса, ни X11-клавиатуры, ни того, доедет ли
напечатанное число до ПЛЕЕРА. Поэтому здесь всё делается настоящими
жестами в настоящем окне.

Скорость утверждается ПО ДАМПУ ГЕОМЕТРИИ (`base_rate`), а не по пикселям
полосы: попытка прочитать значение с картинки была бы измерением
отрисовки, а не состояния. О ТЕМПЕ судим только ОТНОСИТЕЛЬНО (сколько
прошёл курсор при 1× против 0.05×) — debug-сборка в софт-рендере медленнее
реального времени, и абсолютные числа тут ничего не значат (п.301).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, pause, project_with_clips, require_tools  # noqa: E402

SEC = 90000
PLAY_WINDOW = 3.0


def speed_box(app):
    """Прямоугольник контрола скорости — из САМОЙ программы (Р.27)."""
    for _ in range(20):
        f = next((f for f in app.geom().get("fields", []) if f["param"] == "speed"), None)
        if f:
            win = geometry()
            return (
                win["x"] + f["x"],
                win["y"] + f["y"],
                f["w"],
                f["h"],
            )
    app.expect(False, "программа отдала прямоугольник контрола скорости")
    return None


def base_rate(app):
    return float(app.geom().get("base_rate", 0.0))


def travel(app, seconds=PLAY_WINDOW):
    """Сколько тиков прошёл курсор за окно воспроизведения.

    Отсчёт начинается ПОСЛЕ того, как курсор реально сдвинулся: первый
    запуск в софт-рендере уходит на прогрев (декод, первая заливка
    текстуры), и замер «от нажатия пробела» мерил бы его, а не темп."""
    app.key("Home")
    app.key("space")
    app.wait_playhead(lambda t: t > 0, "курсор пошёл", timeout=15)
    t0 = app.playhead()
    pause(
        seconds,
        "темп воспроизведения меряется по РЕАЛЬНОМУ времени: столько программа "
        "и должна играть, чтобы продвижение курсора было сравнимо с часами",
    )
    t1 = app.playhead()
    app.key("space")
    return t1 - t0


def type_text(app, s):
    """Напечатать число настоящими клавишами (keysym'ы, не Event::Text)."""
    table = {".": "period", ",": "comma", "-": "minus", "+": "plus"}
    for ch in s:
        app.key(table.get(ch, ch))


def main(out):
    require_tools()
    path = project_with_clips(os.path.join(out, "rate.otio"), [(0.0, 30.0, "synth:Ramp")])

    with Live("rate", out, project=path) as app:
        app.shot("open")
        x, y, w, h = speed_box(app)
        # ВЕРХНЯЯ полоса — постоянная скорость (нижняя это shuttle с
        # автовозвратом, по ней измерять нечего)
        upper_y = int(y + h * 0.25)
        cx = int(x + w / 2)

        app.expect(abs(base_rate(app) - 1.0) < 1e-3, f"старт на 1×: {base_rate(app)}")
        fast = travel(app)
        app.expect(fast > 0, f"на 1× курсор идёт: {fast} тиков за {PLAY_WINDOW} с")

        # --- 1. ДРАГ полосы в малую скорость -----------------------------
        # тянем слева от центра к точке в нескольких пикселях правее него:
        # до Р.57 весь этот участок был мёртвой зоной и отдавал ровно 0, а
        # её край сразу равнялся 0.25×
        app.drag(cx - 40, upper_y, cx + 6, upper_y)
        slow = base_rate(app)
        app.shot("dragged")
        app.expect(
            0.0 < slow < 0.25,
            f"драг у центра дал представимую МАЛУЮ скорость: {slow}",
        )

        # --- 2. и курсор действительно идёт медленнее --------------------
        crawl = travel(app)
        app.expect(crawl > 0, f"на {slow}× курсор всё-таки идёт: {crawl} тиков")
        app.expect(
            crawl * 4 < fast,
            f"на {slow}× курсор идёт заметно медленнее: {crawl} против {fast} тиков",
        )

        # --- 3. ВВОД ЧИСЛОМ: правый клик по полосе -----------------------
        app.right_click(cx, upper_y)
        type_text(app, "0.05")
        app.key("Return")
        app.shot("typed")
        got = base_rate(app)
        app.expect(abs(got - 0.05) < 1e-3, f"напечатанные 0.05 доехали до модели: {got}")

        # --- 4. Esc закрывает поле и НИЧЕГО не меняет --------------------
        app.right_click(cx, upper_y)
        type_text(app, "2")
        app.key("Escape")
        after = base_rate(app)
        app.expect(abs(after - 0.05) < 1e-3, f"Esc оставил скорость прежней: {after}")

        # --- 5. и после всего этого пробел по-прежнему переключает игру --
        # (правило 14: фокус поля обязан вернуться, иначе хоткеи мертвы)
        was = app.playing()
        app.key("space")
        app.expect(app.playing() != was, "пробел после закрытия поля переключает игру")
        app.key("space")
        app.shot("done")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

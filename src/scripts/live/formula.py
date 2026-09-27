#!/usr/bin/env python3
"""Живой сценарий Р.63 (пп.3-5): формула, фокус и меню форм в настоящем окне.

Три претензии владельца, и все три были зелены в харнессе:

3. «я не понимаю как ввести формулы в кривые автоматизации — я прописываю
   а оно либо стирает формулу либо нихера не происходит»;
4. «если начать вводить что-то в поле формулы то фокус ввода переходит
   туда, и как бы я не тыкал по остальной части интерфейса — нажатие
   пробела не приводит к паузе… пока я не закрою окно эффекта»;
5. «невозможно указать формы кривых… контекстное меню просто пропадает я не
   успеваю дотянуться до него».

Сценарий действует руками: открывает окно эффекта двойным кликом, печатает
формулу по символу, жмёт Enter, кликает по таймлайну и проверяет ПРОБЕЛ —
играет ли программа. Состояние читается из дампа геометрии, а не с картинки.
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, geometry, pause, require_tools  # noqa: E402

SEC = 90000


def demo(dest):
    """Обычный демо-проект: кривую заведём РУКАМИ, как человек."""
    subprocess.run([KT_CLI, "demo", dest, "--force"], check=True, capture_output=True)
    return dest


def main(out):
    require_tools()
    path = demo(os.path.join(out, "formula.otio"))
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("formula", out, project=path) as app:
        # 1) окно эффекта двойным кликом по событию
        y = app.lane_y("video", 0, frac=0.5)
        app.double_click(app.x_at(1 * SEC), y)
        app.expect(app.has_probe("auto.add"), "окно эффекта открылось")
        app.shot("fxwin")

        # 1б) заводим кривую РУКАМИ — тем же путём, что человек
        app.click_probe("auto.add", "кнопка «+ кривая»")
        # ВАРП В СПИСОК НЕ БЕРЁМ: у скорости форма только прямая (t698), и
        # меню участка честно показало бы отказ вместо списка форм
        added = next(
            (
                f["param"]
                for f in app.geom().get("fields", [])
                if f["param"].startswith("auto.add.") and not f["param"].endswith(".rate")
            ),
            None,
        )
        app.expect(added is not None, "меню «+ кривая» показало параметры")
        if added:
            app.click_probe(added, f"пункт {added}")
        app.expect(app.has_probe("auto.formula"), "поле формулы показано после заведения кривой")

        # 2) МЕНЮ ФОРМ: правый клик по участку — и до меню НАДО ДОТЯНУТЬСЯ
        # («контекстное меню просто пропадает я не успеваю дотянуться до
        # него»). Указатель уходит с точки клика к пункту — меню обязано
        # остаться, а форма лечь на участок между теми же двумя ключами.
        app.right_click_probe("auto.seg.0", "участок кривой между первыми ключами")
        app.expect(app.has_probe("auto.shape.2"), "правый клик по участку открыл список форм")
        app.shot("shapemenu")
        app.hover_probe("auto.shape.2", "пункт «ступень»")
        app.expect(
            app.has_probe("auto.shape.2"),
            "меню на месте, пока указатель тянется к пункту",
        )
        app.click_probe("auto.shape.2", "пункт «ступень»")
        app.saved_project()
        app.expect(
            '"hold"' in open(path, encoding="utf-8").read(),
            "форма участка доехала до файла",
        )

        # 3) НАБОР ПО СИМВОЛУ: текст не должен исчезать, ключи — умирать
        #
        # Поле прокручивается в окно, а не берётся по старым координатам: в
        # Р.67 блок кривой вырос (форма отрезка, слой, справка, связь), поле
        # ушло под низ тела окна — слепой клик попадал В ДРУГОЙ контрол и
        # сценарий «не видел» формулы, хотя программа была исправна.
        app.scroll_probe_into_view(
            "auto.formula", "auto.add", below="fx.cancel",
            what="поле формулы прокручено в окно",
        )
        app.click_probe("auto.formula", "поле формулы")
        app.type_text("sin(t)*3+5")
        app.shot("typed")
        before = json.load(open(path)) if os.path.exists(path) else None
        app.key("Return")
        saved = app.saved_project()
        doc = json.load(open(path))
        formulas = []

        def walk(x):
            if isinstance(x, dict):
                src = x.get("source")
                if isinstance(src, dict) and src.get("kind") == "formula":
                    formulas.append(src.get("expr"))
                for v in x.values():
                    walk(v)
            elif isinstance(x, list):
                for v in x:
                    walk(v)

        walk(doc)
        app.expect(
            any("sin(t)*3+5" in str(f) for f in formulas),
            f"формула доехала до файла целиком: {formulas}",
        )
        # Р.67, п.6 владельца: формула — СЛОЙ поверх ключей, а не источник
        # вместо них; «я может быть передумал и хотел бы вернуться к зигзагу»
        keys_after = []

        def walk_keys(x):
            if isinstance(x, dict):
                if isinstance(x.get("keys"), list):
                    keys_after.append(len(x["keys"]))
                for v in x.values():
                    walk_keys(v)
            elif isinstance(x, list):
                for v in x:
                    walk_keys(v)

        walk_keys(doc)
        app.expect(
            any(n > 0 for n in keys_after),
            f"ключи под формулой-слоем целы: {keys_after}",
        )
        _ = before, saved

        # 4) ФОКУС: клик по таймлайну — и пробел ИГРАЕТ
        app.click(app.x_at(2 * SEC), y)
        was = app.playing()
        app.key("space")
        pause(
            1.0,
            "воспроизведение идёт в РЕАЛЬНОМ времени: секунда игры — это сама "
            "проверка, а не ожидание реакции интерфейса",
        )
        now = app.playing()
        app.expect(
            now != was,
            f"после клика по таймлайну пробел играет при ОТКРЫТОМ окне эффекта ({was} → {now})",
        )
        if now:
            app.key("space")

        # 5) КНОПКА окна тоже не должна забирать клавиатуру навсегда
        app.click_probe("fxreg.add", "кнопка пикера в окне эффекта")
        app.key("Escape")  # закрыть попап пикера
        was = app.playing()
        app.key("space")
        pause(
            1.0,
            "воспроизведение идёт в РЕАЛЬНОМ времени: секунда игры — это сама "
            "проверка, а не ожидание реакции интерфейса",
        )
        now = app.playing()
        app.expect(
            now != was,
            f"после клика по КНОПКЕ окна пробел играет ({was} → {now})",
        )
        if now:
            app.key("space")
        app.shot("after-space")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/formula"))

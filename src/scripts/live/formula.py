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
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, geometry, require_tools  # noqa: E402

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
        time.sleep(0.4)
        added = next(
            (
                f["param"]
                for f in app.geom().get("fields", [])
                if f["param"].startswith("auto.add.")
            ),
            None,
        )
        app.expect(added is not None, "меню «+ кривая» показало параметры")
        if added:
            app.click_probe(added, f"пункт {added}")
        app.expect(app.has_probe("auto.formula"), "поле формулы показано после заведения кривой")

        # 2) НАБОР ПО СИМВОЛУ: текст не должен исчезать, ключи — умирать
        app.click_probe("auto.formula", "поле формулы")
        app.type_text("sin(t)*3+5")
        app.shot("typed")
        before = json.load(open(path)) if os.path.exists(path) else None
        app.key("Return")
        time.sleep(0.5)
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
        _ = before, saved

        # 3) ФОКУС: клик по таймлайну — и пробел ИГРАЕТ
        app.click(app.x_at(2 * SEC), y)
        time.sleep(0.3)
        was = app.playing()
        app.key("space")
        time.sleep(1.0)
        now = app.playing()
        app.expect(
            now != was,
            f"после клика по таймлайну пробел играет при ОТКРЫТОМ окне эффекта ({was} → {now})",
        )
        if now:
            app.key("space")
            time.sleep(0.3)

        # 4) КНОПКА окна тоже не должна забирать клавиатуру навсегда
        app.click_probe("fxreg.add", "кнопка пикера в окне эффекта")
        app.key("Escape")  # закрыть попап пикера
        time.sleep(0.3)
        was = app.playing()
        app.key("space")
        time.sleep(1.0)
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

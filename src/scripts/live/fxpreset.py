#!/usr/bin/env python3
"""Живой сценарий Р.48 (п.346): библиотека FX-пресетов читается настоящей
программой и переживает её работу.

Харнесс-тесты (t344-t346) гоняют окно и вставку внутри одного процесса, а
хранилище там В ПАМЯТИ — домашний каталог разработчика тесты не трогают.
Здесь проверяется то, чего харнесс не проверяет: настоящий бинарь с
настоящим `KT_USER_STORE_DIR` читает файл библиотеки, открывает по нему
окно FX и НЕ портит чужие записи; а повреждённый файл его не роняет
(политика «битый файл не роняет программу» — п.343).
"""
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, require_tools  # noqa: E402

SEC = 90000

# пресет с кривой и запись из «будущей версии» рядом: старая сборка её не
# понимает, но обязана сохранить
LIBRARY = {
    "version": 1,
    "presets": [
        {
            "name": "live chain",
            "slots": [
                {
                    "fx": {"kind": "blur", "radius": 6},
                    "curves": [
                        {
                            "param": "radius",
                            "ease": True,
                            "keys": [
                                {"t": 0, "v": 1.0},
                                {"t": 90000, "v": 32.0},
                            ],
                        }
                    ],
                }
            ],
        },
        {"name": "from the future", "slots": [{"fx": {"kind": "time_machine", "x": 1}}]},
    ],
}


def open_fx_window(app, tag):
    """Открыть окно FX двойным кликом по событию и убедиться, что оно
    нарисовалось (пикер с библиотекой живёт внутри него)."""
    y = app.lane_y("video", 0, frac=0.5)
    before = app.shot(f"{tag}-before")
    app.double_click(app.x_at(3 * SEC), y)
    app.shot(f"{tag}-fx")
    d = app.screen_diff(before)
    app.expect(d > 0.01, f"{tag}: окно FX открылось (экран изменился на {d:.1%})")


def main(out):
    require_tools()
    store = os.path.join(out, "store")
    os.makedirs(store, exist_ok=True)
    os.environ["KT_USER_STORE_DIR"] = store
    presets = os.path.join(store, "fx-presets.json")

    proj = os.path.join(out, "fxpreset.otio")
    subprocess.run([KT_CLI, "demo", proj], check=True, capture_output=True)

    # --- 1) валидная библиотека: программа её читает и не портит --------
    with open(presets, "w") as f:
        json.dump(LIBRARY, f, indent=2)
    before = json.load(open(presets))
    with Live("fxpreset", out, project=proj) as app:
        open_fx_window(app, "lib")
        app.saved_project()  # ctrl+s: запись проекта библиотеку не трогает
        after = json.load(open(presets))
        app.expect(after == before, "программа не переписала библиотеку без нужды")
        app.expect(
            any(p["name"] == "from the future" for p in after["presets"]),
            "запись из будущей версии пережила сессию",
        )
        rc = app.done()
    if rc:
        return rc

    # --- 2) битый файл: программа обязана открыться ---------------------
    with open(presets, "w") as f:
        f.write("{ this is not json")
    with Live("fxpreset-broken", out, project=proj) as app:
        open_fx_window(app, "broken")
        app.expect(True, "битый файл библиотеки не помешал программе работать")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

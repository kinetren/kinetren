#!/usr/bin/env python3
"""Живой сценарий Р.43 п.1: значение ключа, НАПЕЧАТАННОЕ в поле параметра,
доезжает до файла проекта.

Претензия дословно: «в таймлайне автоматизации для ключей slowed-reverb я
например проставляю в поле ввода rate а он скидывается на предыдущее
значение, а если на графике сдвину ключ - то значение меняется как надо.
Кстати - в морфере это работает хорошо, плавающий баг».

Харнесс-тест на это есть (t281), но он ничего не доказывает про программу,
которую запускает пользователь (правило «ты не проверял», Р.40): здесь
настоящий kt-app, настоящий клик, настоящие символы с клавиатуры и разбор
того, что программа ЗАПИСАЛА на диск.

Координата поля берётся из САМОЙ программы (`KT_GEOM_DUMP`, раздел
`fields`): у `egui::DragValue` нет узла в дереве доступности, и угаданное
смещение внутри окна сделало бы сценарий флаки.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, geometry, require_tools  # noqa: E402

SEC = 90000
TYPED = 7.0


def rate_keys(tl):
    """Значения ключей кривой скорости.

    Форм две: как проект пишем МЫ при подготовке (`metadata.kt.fx_auto` у
    события — так же делает keys.py) и как пишет ПРОГРАММА (`auto` внутри
    своего эффекта). Сценарий читает обе, иначе сравнивал бы «до» и «после»
    в разных системах координат."""
    for track in tl.doc.get("tracks", {}).get("children", []):
        for item in track.get("children", []):
            for e in item.get("effects", []):
                kt = (e.get("metadata") or {}).get("kt") or {}
                for cur in kt.get("auto", []):
                    if cur.get("param") == "rate":
                        return [k["v"] for k in cur["keys"]]
            kt = (item.get("metadata") or {}).get("kt") or {}
            for slot in kt.get("fx_auto", []):
                for cur in slot:
                    if cur.get("param") == "rate":
                        return [k["v"] for k in cur["keys"]]
    return []


def main(out):
    require_tools()
    # демо-проект: у события 25 на аудио-дорожке 1 стоит slowed&reverb с
    # живыми кривыми — ровно та цепочка, о которой претензия
    proj = os.path.join(out, "fxfield.otio")
    subprocess.run([KT_CLI, "demo", proj, "--force"], check=True, capture_output=True)

    with Live("fxfield", out, project=proj) as app:
        app.shot("open")
        before = rate_keys(app.project_now())
        app.expect(len(before) >= 2, f"старт: кривая скорости на месте, {before}")
        app.expect(TYPED not in before, f"пробное значение {TYPED} не должно быть в кривой заранее")

        # окно FX — двойным кликом по событию (п.204)
        # ВНЕ линии уровня: с Р.49 (п.362) двойной клик по ней сбрасывает
        # громкость, а у этого события gain −8 dB, то есть линия стоит
        # примерно на 0.73 высоты дорожки. Целимся заметно выше.
        y = app.lane_y("audio", 1, frac=0.50)
        app.double_click(app.x_at(10 * SEC), y)
        app.shot("fx-open")

        field = next(
            (f for f in app.geom().get("fields", []) if f["param"] == "rate"), None
        )
        app.expect(field is not None, "программа отдала прямоугольник поля «скорость»")
        if field is None:
            return app.done()

        win = geometry()
        fx = win["x"] + int(field["x"] + field["w"] / 2)
        fy = win["y"] + int(field["y"] + field["h"] / 2)

        # клик по полю → режим ввода, символы, Enter
        app.click(fx, fy)
        app.key("7")
        app.key("Return")
        app.shot("typed")

        after = rate_keys(app.saved_project())
        app.expect(
            TYPED in after,
            f"напечатанное значение обязано доехать до ФАЙЛА: было {before}, "
            f"стало {after} (до Р.43 правка поля умирала вместе с кадром)",
        )
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

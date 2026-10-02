#!/usr/bin/env python3
"""Живой сценарий Р.61: буфер огибающих — СИСТЕМНЫЙ, и чтение своего же
буфера не вешает программу.

Претензия владельца: «судя по тому что кнопка может быть "disabled" — в коде
есть какое-то состояние буфера обмена — что плохо, нужно по нажатию кнопки
брать значение из реального буфера и уже потом проверять можно вставить или
нет».

Харнесс этого доказать не может: там нет ни X-сервера, ни системного буфера,
а главная опасность живёт именно там. На X11 буфером ВЛАДЕЕТ окно, и после
копирования владелец — наше же окно; синхронное чтение ушло бы запросом в наш
собственный цикл событий, который в этот момент рисует, и программа повисла
бы РОВНО в том сценарии, ради которого фича делается. Поэтому чтение —
фоновой задачей, и проверяется это здесь: копируем, вставляем, окно живо.
"""
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, require_tools  # noqa: E402
from kt_live import demo_otio, import_project, new_otio  # noqa: E402,F401

SEC = 90000


def walk(node):
    yield node
    if isinstance(node, dict):
        for v in node.values():
            yield from walk(v)
    elif isinstance(node, list):
        for v in node:
            yield from walk(v)


def curve_params(doc):
    """Имена параметров всех кривых документа."""
    out = []
    for n in walk(doc):
        meta = n.get("metadata") if isinstance(n, dict) else None
        if not isinstance(meta, dict):
            continue
        kt = meta.get("kt")
        if isinstance(kt, dict) and isinstance(kt.get("auto"), list):
            out += [c.get("param") for c in kt["auto"] if isinstance(c, dict)]
    return out


def main(out):
    require_tools()
    proj = os.path.join(out, "envclip.otio")
    demo_otio(proj)

    with Live("envclip", out, project=proj) as app:
        # ВНЕ линии уровня: двойной клик по ней сбрасывает громкость (п.362)
        y = app.lane_y("audio", 1, frac=0.50)
        app.double_click(app.x_at(10 * SEC), y)
        app.shot("fx-window")
        app.expect(app.has_probe("env.copy"), "кнопка «копировать огибающую» есть в окне")
        app.expect(app.has_probe("env.paste"), "кнопка «вставить огибающую» есть в окне")

        before = curve_params(app.saved_project().doc)
        app.expect(bool(before), f"у демо-проекта есть кривые: {before}")

        # --- КОПИРОВАНИЕ пишет в системный буфер ----------------------
        app.click_probe("env.copy", "копирование огибающей в системный буфер")
        app.shot("copied")

        # --- ВСТАВКА читает СВОЙ ЖЕ буфер и не вешает окно ------------
        # ровно тот случай, который повесил бы синхронное чтение: владелец
        # буфера — наше собственное окно
        app.click_probe("env.paste", "вставка из своего же буфера")
        app.settle()
        app.shot("pasted")
        app.expect(
            app.has_probe("env.paste"),
            "окно живо после чтения СВОЕГО ЖЕ буфера — синхронное чтение "
            "повисло бы здесь",
        )
        # и программа отвечает на жесты: геометрия читается, значит цикл
        # событий не заблокирован
        g = app.geom()
        app.expect(isinstance(g, dict) and bool(g), "цикл событий не заблокирован")

        app.key("Escape")
        after = curve_params(app.saved_project().doc)
        app.expect(
            set(before).issubset(set(after)),
            f"круг через системный буфер потерял кривые: было {before}, стало {after}",
        )


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/envclip")

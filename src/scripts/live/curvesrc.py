#!/usr/bin/env python3
"""Живой сценарий Р.61: кривая, заданная ФОРМУЛОЙ, видна и доезжает до файла.

Владелец: «возможность задания кривой автоматизации через ввод формулы (без
прокликивания мышки)» и «итоговый граф тоже рисуется но не редактируется
мышкой».

Харнесс проверяет модель и вёрстку по отдельности; здесь — настоящий kt-app:
текст набирается в настоящее поле, следствие ищется в сохранённом файле.
"""
import os
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import KT_CLI, Live, require_tools  # noqa: E402

SEC = 90000


def walk(node):
    yield node
    if isinstance(node, dict):
        for v in node.values():
            yield from walk(v)
    elif isinstance(node, list):
        for v in node:
            yield from walk(v)


def sources(doc):
    """Источники всех кривых документа: «keys» либо вид производного."""
    out = []
    for n in walk(doc):
        meta = n.get("metadata") if isinstance(n, dict) else None
        if not isinstance(meta, dict):
            continue
        kt = meta.get("kt")
        if isinstance(kt, dict) and isinstance(kt.get("auto"), list):
            for c in kt["auto"]:
                if isinstance(c, dict):
                    src = c.get("source")
                    out.append(src.get("kind") if isinstance(src, dict) else "keys")
    return out


def main(out):
    require_tools()
    proj = os.path.join(out, "curvesrc.otio")
    subprocess.run([KT_CLI, "demo", proj, "--force"], check=True, capture_output=True)

    with Live("curvesrc", out, project=proj) as app:
        # ВНЕ линии уровня: двойной клик по ней сбрасывает громкость
        y = app.lane_y("audio", 1, frac=0.50)
        app.double_click(app.x_at(10 * SEC), y)
        time.sleep(0.9)
        app.shot("fx-window")

        before = sources(app.saved_project().doc)
        app.expect(bool(before), f"у демо-проекта есть кривые: {before}")
        app.expect(
            all(s == "keys" for s in before),
            f"до правки все кривые ключевые: {before}",
        )

        app.expect(app.has_probe("auto.formula"), "поле формулы есть в окне автоматизации")

        # --- БИТЫЙ текст показывает ошибку и НЕ портит модель ---------
        app.scroll_probe_into_view(
            "auto.formula", "auto.add", below="fx.cancel",
            what="поле формулы прокручено в окно",
        )
        app.click_probe("auto.formula", "поле формулы")
        app.type_text("(1+")
        time.sleep(0.6)
        app.shot("broken-formula")
        mid = sources(app.saved_project().doc)
        app.expect(
            all(s == "keys" for s in mid),
            f"битый текст доехал до модели: {mid}",
        )

        # --- ВЕРНЫЙ текст применяется ---------------------------------
        #
        # Ввод делается с ПОВТОРОМ: под полной нагрузкой (весь finish.sh)
        # поле успевает потерять фокус между кликом и первой клавишей, и
        # сценарий мигал. Повтор — не «пересдача красного»: каждая попытка
        # заново кликает поле и заново набирает текст, а утверждение
        # проверяется ПОСЛЕ них (§458: мигание чинится, а не игнорируется).
        after = before
        for attempt in range(3):
            app.scroll_probe_into_view(
                "auto.formula", "auto.add", below="fx.cancel",
                what=f"поле формулы прокручено в окно (попытка {attempt + 1})",
            )
            app.click_probe("auto.formula", f"поле формулы (попытка {attempt + 1})")
            time.sleep(0.5)
            # чистим поле по-символьно: ctrl+a у egui выделяет не всегда
            for _ in range(24):
                app.key("BackSpace")
            # набор ТЕКСТОМ, а не по именам клавиш: keysym `asterisk` на
            # раскладке приходит как «8», и в поле оказывалось `t82` —
            # программа честно отказывала («нет имени t82; есть t»), а
            # сценарий винил её в том, что формула «не доехала» (Р.67)
            app.type_text("t*2")
            app.key("Return")
            time.sleep(1.0)
            after = sources(app.saved_project().doc)
            if "formula" in after:
                break
        app.shot("formula-applied")
        app.expect(
            "formula" in after,
            f"формула не доехала до ФАЙЛА: было {before}, стало {after}",
        )

        # --- «отвязать» возвращает обычные ключи ----------------------
        if app.has_probe("auto.unlink"):
            app.click_probe("auto.unlink", "отвязать кривую")
            time.sleep(0.8)
            back = sources(app.saved_project().doc)
            app.expect(
                "formula" not in back,
                f"«отвязать» не сняло источник: {back}",
            )
            app.shot("unlinked")

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "target/live-out/curvesrc"))

#!/usr/bin/env python3
"""Живая проверка UI в НАСТОЯЩЕМ окне kt-app (X11, XTest).

Заведено после раунда 11: harness-тест показывал предел ширины панели
метров, а в живом окне пользователь спокойно растягивал её дальше —
проверялась не программа, а её симуляция. Харнесс ловит вёрстку, этот
скрипт ловит «а что видит человек». Одно другое не заменяет.

Требуется python3-xlib и ImageMagick (`import`, `convert`).

    python3 scripts/ui-probe.py geom                  # где окно
    python3 scripts/ui-probe.py drag 1370 430 200 430 # тянуть (коорд. окна)
    python3 scripts/ui-probe.py key ctrl+l            # клавиша с модификатором
    python3 scripts/ui-probe.py type 130
    python3 scripts/ui-probe.py shot /tmp/ui.png
    python3 scripts/ui-probe.py edges 400             # границы панелей по строке

Драг делается КАК ЧЕЛОВЕК: нажать → сдвинуть на несколько пикселей →
вести дальше. Прыжок «сразу в цель» egui засчитывает как начало драга уже
в конечной точке, и объект не двигается вовсе (см. apps/kt-app/CLAUDE.md).
"""
import subprocess
import sys
import time

from Xlib import X, XK, display
from Xlib.ext import xtest

WM_CLASS = "kinotrain"
d = display.Display()
root = d.screen().root


def window():
    def walk(w):
        try:
            cls = w.get_wm_class()
            if cls and WM_CLASS in cls and w.get_geometry().width > 400:
                return w
        except Exception:
            pass
        for c in w.query_tree().children:
            r = walk(c)
            if r:
                return r
        return None

    w = walk(root)
    if not w:
        sys.exit("окно kt-app не найдено (запущено ли приложение?)")
    return w


def origin(w):
    g = w.get_geometry()
    t = w.translate_coords(root, 0, 0)
    return -t.x, -t.y, g.width, g.height


def move(x, y, pause=0.05):
    xtest.fake_input(d, X.MotionNotify, x=int(x), y=int(y))
    d.sync()
    time.sleep(pause)


def button(pressed, b=1):
    xtest.fake_input(d, X.ButtonPress if pressed else X.ButtonRelease, b)
    d.sync()
    time.sleep(0.09)


def key(spec):
    *mods, name = spec.split("+")
    table = {"ctrl": "Control_L", "shift": "Shift_L", "alt": "Alt_L"}
    mcodes = [d.keysym_to_keycode(XK.string_to_keysym(table[m.lower()])) for m in mods]
    code = d.keysym_to_keycode(XK.string_to_keysym(name))
    for m in mcodes:
        xtest.fake_input(d, X.KeyPress, m)
    xtest.fake_input(d, X.KeyPress, code)
    d.sync()
    time.sleep(0.05)
    xtest.fake_input(d, X.KeyRelease, code)
    for m in reversed(mcodes):
        xtest.fake_input(d, X.KeyRelease, m)
    d.sync()
    time.sleep(0.12)


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else "geom"
    ox, oy, w, h = origin(window())
    if cmd == "geom":
        print(f"окно: экран({ox},{oy}) размер {w}x{h}")
    elif cmd == "drag":
        x1, y1, x2, y2 = (int(v) for v in sys.argv[2:6])
        move(ox + x1, oy + y1)
        move(ox + x1, oy + y1)
        button(True)
        move(ox + x1 + (8 if x2 > x1 else -8), oy + y1)  # порог драга
        for k in (0.35, 0.7, 1.0):
            move(ox + x1 + (x2 - x1) * k, oy + y1 + (y2 - y1) * k)
        button(False)
        time.sleep(0.3)
    elif cmd == "key":
        key(sys.argv[2])
    elif cmd == "type":
        for ch in sys.argv[2]:
            key(ch)
    elif cmd == "shot":
        subprocess.run(["import", "-window", "root", "/tmp/kt-probe-full.png"], check=True)
        subprocess.run(
            ["convert", "/tmp/kt-probe-full.png", "-crop", f"{w}x{h}+{ox}+{oy}",
             "+repage", sys.argv[2]],
            check=True,
        )
        print(sys.argv[2])
    elif cmd == "edges":
        from PIL import Image

        row = int(sys.argv[2])
        subprocess.run(["import", "-window", "root", "/tmp/kt-probe-full.png"], check=True)
        im = Image.open("/tmp/kt-probe-full.png").convert("RGB")
        prev, out = None, []
        for x in range(w):
            c = im.getpixel((ox + x, oy + row))
            if prev and sum(abs(a - b) for a, b in zip(c, prev)) > 14:
                out.append(x)
            prev = c
        print("вертикальные границы по строке", row, ":", out)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()

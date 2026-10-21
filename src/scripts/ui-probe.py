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

from Xlib import X, XK, display, protocol
from Xlib.ext import xtest

WM_CLASS = "kinetren"
d = display.Display()
root = d.screen().root

# СКОЛЬКО ЗДЕСЬ ПРОСПАЛИ (Р.69). Драйвер спит внутри себя, и до Р.69 этот
# сон был невидим: сторож `live-sleeps.py` смотрел только на каркас и
# сценарии, а тут лежало 123 с из 387 с сна всего живого прогона. Теперь
# весь сон драйвера идёт через `_nap`, копится и попадает в тот же отчёт,
# что сон каркаса (`kt_live.slept()`).
SLEPT = [0.0]


def pixels(x, y, w=1, h=1):
    """Прямоугольник экрана ПРЯМЫМ запросом к X: [(r, g, b), …] (Р.69).

    Прежде цвет одной точки стоил полного снимка экрана 1920×1080 через
    `xwd` и двух процессов ImageMagick — 84 мс на чтение и ≈341 чтение за
    прогон. Здесь тот же XGetImage, которым пользуется и `xwd`, только без
    файла и без разбора PNG: 0.04 мс.

    Формат ZPixmap отдаёт по 4 байта на точку (BGRX) при глубине 24;
    число байт на точку считается из длины данных, а не предполагается.
    """
    img = root.get_image(int(x), int(y), int(w), int(h), X.ZPixmap, 0xFFFFFFFF)
    data = img.data
    if isinstance(data, str):  # старые python3-xlib отдают str
        data = data.encode("latin-1")
    n = int(w) * int(h)
    bpp = len(data) // max(n, 1)
    out = []
    for i in range(n):
        px = data[i * bpp : i * bpp + bpp]
        if bpp >= 3:
            out.append((px[2], px[1], px[0]))  # BGRX → RGB
        else:
            out.append((px[0], px[0], px[0]))
    return out


def _nap(seconds):
    """Единственное место, где драйвер спит."""
    if seconds <= 0:
        return
    SLEPT[0] += seconds
    time.sleep(seconds)


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


def close_window():
    """Попросить окно закрыться, как это делает крестик оконного менеджера
    (WM_DELETE_WINDOW). Именно этот путь перехватывает программа, чтобы
    спросить про несохранённую работу (Р.49, п.354)."""
    w = window()
    # окно eframe лежит внутри рамки openbox — протокол висит на нём самом
    wm_protocols = d.intern_atom("WM_PROTOCOLS")
    wm_delete = d.intern_atom("WM_DELETE_WINDOW")
    ev = protocol.event.ClientMessage(
        window=w, client_type=wm_protocols, data=(32, [wm_delete, X.CurrentTime, 0, 0, 0])
    )
    w.send_event(ev, event_mask=X.NoEventMask)
    d.sync()


def move(x, y, pause=0.05):
    xtest.fake_input(d, X.MotionNotify, x=int(x), y=int(y))
    d.sync()
    _nap(pause)


def button(pressed, b=1, pause=0.09):
    """Кнопка мыши. `pause` — сколько ждать после события.

    По умолчанию как было (ручной прогон из командной строки: ждать некому).
    Каркас живых сценариев передаёт почти ноль и ждёт КАДР сам (Р.68): его
    ожидание точнее и под нагрузкой растягивается вместе с кадром.
    """
    xtest.fake_input(d, X.ButtonPress if pressed else X.ButtonRelease, b)
    d.sync()
    _nap(pause)


def key(spec, pause=0.05, after=0.12):
    """Клавиша (с модификаторами через «+»); паузы — см. `button`."""
    *mods, name = spec.split("+")
    table = {"ctrl": "Control_L", "shift": "Shift_L", "alt": "Alt_L"}
    mcodes = [d.keysym_to_keycode(XK.string_to_keysym(table[m.lower()])) for m in mods]
    code = d.keysym_to_keycode(XK.string_to_keysym(name))
    for m in mcodes:
        xtest.fake_input(d, X.KeyPress, m)
    xtest.fake_input(d, X.KeyPress, code)
    d.sync()
    _nap(pause)
    xtest.fake_input(d, X.KeyRelease, code)
    for m in reversed(mcodes):
        xtest.fake_input(d, X.KeyRelease, m)
    d.sync()
    _nap(after)


def hold(spec, secs):
    """Зажать клавишу на время (разгон пробелом, п.3 р.16).

    Автоповтор X11 при этом шлёт серию KeyPress с repeat — ровно то, на
    чём ломался прежний обработчик Space.
    """
    *mods, name = spec.split("+")
    table = {"ctrl": "Control_L", "shift": "Shift_L", "alt": "Alt_L"}
    mcodes = [d.keysym_to_keycode(XK.string_to_keysym(table[m.lower()])) for m in mods]
    code = d.keysym_to_keycode(XK.string_to_keysym(name))
    for m in mcodes:
        xtest.fake_input(d, X.KeyPress, m)
    xtest.fake_input(d, X.KeyPress, code)
    d.sync()
    _nap(secs)
    xtest.fake_input(d, X.KeyRelease, code)
    for m in reversed(mcodes):
        xtest.fake_input(d, X.KeyRelease, m)
    d.sync()
    _nap(0.2)


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
        _nap(0.3)
    elif cmd == "draghold":
        # тянуть и ДЕРЖАТЬ у цели: так проверяется автоскролл у кромки
        # (медиа-бин, п.1 р.16) — курсор стоит, а список обязан ползти
        x1, y1, x2, y2, secs = (float(v) for v in sys.argv[2:7])
        move(ox + x1, oy + y1)
        move(ox + x1, oy + y1)
        button(True)
        move(ox + x1, oy + y1 + (8 if y2 > y1 else -8))
        for k in (0.35, 0.7, 1.0):
            move(ox + x1 + (x2 - x1) * k, oy + y1 + (y2 - y1) * k)
        t0 = time.time()
        while time.time() - t0 < secs:
            move(ox + x2, oy + y2, pause=0.03)
        button(False)
        _nap(0.3)
    elif cmd == "click" or cmd == "rclick":
        x, y = (int(v) for v in sys.argv[2:4])
        move(ox + x, oy + y)
        move(ox + x, oy + y)
        b = 1 if cmd == "click" else 3
        button(True, b)
        button(False, b)
        _nap(0.3)
    elif cmd == "wheel":
        # колесо: кнопки 4 (вверх) и 5 (вниз); n<0 — вверх
        x, y, n = (int(v) for v in sys.argv[2:5])
        move(ox + x, oy + y)
        move(ox + x, oy + y)
        b = 5 if n > 0 else 4
        for _ in range(abs(n)):
            button(True, b)
            button(False, b)
        _nap(0.3)
    elif cmd == "px":
        from PIL import Image

        x, y = (int(v) for v in sys.argv[2:4])
        subprocess.run(["import", "-window", "root", "/tmp/kt-probe-full.png"], check=True)
        im = Image.open("/tmp/kt-probe-full.png").convert("RGB")
        print(im.getpixel((ox + x, oy + y)))
    elif cmd == "hold":
        hold(sys.argv[2], float(sys.argv[3]))
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

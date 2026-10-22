#!/usr/bin/env python3
"""Live UI check in a REAL kt-app window (X11, XTest).

Added after round 11: a harness test showed the width limit of the meter panel,
yet in the live window the user stretched it further without trouble —
the program was not checked, only its simulation. The harness catches layout, this
script catches “what the person sees”. One does not replace the other.

Requires python3-xlib and ImageMagick (`import`, `convert`).

    python3 scripts/ui-probe.py geom                  # where the window is
    python3 scripts/ui-probe.py drag 1370 430 200 430 # drag (window coords)
    python3 scripts/ui-probe.py key ctrl+l            # key with a modifier
    python3 scripts/ui-probe.py type 130
    python3 scripts/ui-probe.py shot /tmp/ui.png
    python3 scripts/ui-probe.py edges 400             # panel edges along the row

The drag is done THE WAY A PERSON DOES: press → move a few pixels →
keep going. A “jump straight to the target” is counted by egui as the start of a drag already
at the end point, and the object does not move at all (see apps/kt-app/CLAUDE.md).
"""
import subprocess
import sys
import time

from Xlib import X, XK, display, protocol
from Xlib.ext import xtest

WM_CLASS = "kinetren"
d = display.Display()
root = d.screen().root

# HOW MUCH TIME WAS SLEPT HERE (R.69). The driver sleeps inside itself, and until R.69 that
# sleep was invisible: the guard `live-sleeps.py` looked only at the rig and
# the scenarios, while 123 s out of 387 s of sleep of the whole live run lay here. Now
# all the driver's sleep goes through `_nap`, accumulates, and lands in the same report
# as the rig's sleep (`kt_live.slept()`).
SLEPT = [0.0]


def pixels(x, y, w=1, h=1):
    """Rectangle of the screen by a DIRECT X request: [(r, g, b), …] (R.69).

    Before, the colour of one point cost a full 1920×1080 screenshot through
    `xwd` and two ImageMagick processes — 84 ms per read, ≈341 reads per run.
    Here it is the same XGetImage that `xwd` uses, only without a file and
    without PNG parsing: 0.04 ms.

    The ZPixmap format gives 4 bytes per point (BGRX) at depth 24;
    the number of bytes per point is computed from the data length, not assumed.
    """
    img = root.get_image(int(x), int(y), int(w), int(h), X.ZPixmap, 0xFFFFFFFF)
    data = img.data
    if isinstance(data, str):  # old python3-xlib returns str
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
    """The only place where the driver sleeps."""
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
        sys.exit("kt-app window not found (is the application running?)")
    return w


def origin(w):
    g = w.get_geometry()
    t = w.translate_coords(root, 0, 0)
    return -t.x, -t.y, g.width, g.height


def close_window():
    """Ask the window to close, as the window manager’s close button does
    (WM_DELETE_WINDOW). This is exactly the path the program intercepts to
    ask about unsaved work (R.49, item 354)."""
    w = window()
    # the eframe window lies inside the openbox frame — the protocol hangs on it itself
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
    """Mouse button. `pause` — how long to wait after the event.

    Default as before (a manual run from the command line: there is no one to wait for).
    The live-scenario harness passes almost zero and waits for the FRAME itself (R.68): its wait
    is more precise, and under load it stretches along with the frame.
    """
    xtest.fake_input(d, X.ButtonPress if pressed else X.ButtonRelease, b)
    d.sync()
    _nap(pause)


def key(spec, pause=0.05, after=0.12):
    """Key (with modifiers joined by “+”); pauses — see `button`."""
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
    """Hold a key for a while (speed-up with Space, item 3 of round 16).

    X11 auto-repeat then sends a series of KeyPress events with repeat — exactly
    what the earlier Space handler broke on.
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
        print(f"window: screen({ox},{oy}) size {w}x{h}")
    elif cmd == "drag":
        x1, y1, x2, y2 = (int(v) for v in sys.argv[2:6])
        move(ox + x1, oy + y1)
        move(ox + x1, oy + y1)
        button(True)
        move(ox + x1 + (8 if x2 > x1 else -8), oy + y1)  # drag threshold
        for k in (0.35, 0.7, 1.0):
            move(ox + x1 + (x2 - x1) * k, oy + y1 + (y2 - y1) * k)
        button(False)
        _nap(0.3)
    elif cmd == "draghold":
        # hold and DRAG to the target: this is how auto-scroll at the edge is verified
        # (media bin, item 1 round 16) — the cursor stands still, and the list must keep crawling
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
        # wheel: buttons 4 (up) and 5 (down); n<0 — up
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
        print("vertical borders along the row", row, ":", out)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Live scenario control-layouts: the Layout & Controls style, from the first entry to the code.

The owner's words: `docs/ktprompts/v26.10.txt`, L1 and L2.

Its own process and an empty store: the question opens by itself at the first start. Each tile is
pressed and shot with the panels it brings; Premiere's L and K run the transport for real; the
Customize part assigns a key, meets a taken one, resets; a wrong code is refused and the right one
applied; Preferences has the door back. The verdicts read the program's own records: the store's
`start.json` and the probes. A screenshot per step (`kt_live.step`).
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, poll_step, require_tools  # noqa: E402

OWN_PROCESS = True


def stored(store):
    try:
        with open(os.path.join(store, "start.json")) as f:
            return json.load(f)
    except (OSError, ValueError):
        return {}


def wait_for(pred, timeout=20.0):
    deadline = time.time() + timeout
    while time.time() < deadline:
        v = pred()
        if v:
            return v
        poll_step(0.25)
    return None


def main(out):
    require_tools()
    store = os.path.join(out, "store")
    if os.path.isdir(store):
        for name in os.listdir(store):
            p = os.path.join(store, name)
            if os.path.isfile(p):
                os.remove(p)
    with Live("controls", out, own_process=True, first_entry=True) as app:
        # ---- the first entry ----
        app.expect(app.has_probe("controls.window"), "the first entry opens the style question")
        app.expect(app.has_probe("controls.tile.0"), "our tile is there")
        app.step("first-entry", "the first entry: five tiles, ours first with “like Vegas”, the trademarks under them")
        code_own = None
        # ---- every tile, with its panels ----
        for n, name in [(1, "premiere"), (2, "resolve"), (3, "final-cut"), (4, "after-effects")]:
            app.click_probe(f"controls.tile.{n}", f"the {name} tile")
            app.settle()
            app.expect(
                wait_for(lambda: stored(store).get("controls") not in (None, code_own)),
                f"the {name} tile reached the store",
            )
            code_own = stored(store).get("controls")
            # the store is written in the click's frame, which drew the tiles before it knew the
            # answer: the shot waits for the frame after it
            app.wait_frame(2, what="the frame after the tile's click", soft=True)
            app.step(f"tile-{name}", f"the {name} tile chosen: the panels moved behind the window, the line says what is not here")
        # ---- Premiere's hands ----
        app.click_probe("controls.tile.1", "back to Premiere")
        app.settle()
        app.click_probe("controls.close", "the window closes")
        app.settle()
        app.wait_gone("controls.window")
        # the welcome window of the first start stands under it
        app.click_probe("welcome.close", "Close")
        app.wait_gone("welcome.window")
        app.step("premiere-window", "the program in the Premiere style: the wide preview, the bin and the meters beside the timeline")
        app.key("l")
        app.expect(wait_for(app.playing), "L plays in the Premiere style")
        app.key("k")
        app.expect(wait_for(lambda: not app.playing()), "K stops")
        # ---- back through Preferences, to our tile, and Customize ----
        app.click_probe("menu.file", "the File menu")
        app.click_probe("file.preferences", "File → Preferences…")
        app.expect(app.has_probe("prefs.controls"), "Preferences has the style row")
        app.step("preferences", "Preferences: the style row names the choice, with a Choose button")
        app.click_probe("prefs.controls", "Choose…")
        app.settle()
        app.click_probe("prefs.close", "Preferences closes")
        app.click_probe("controls.tile.0", "our tile")
        app.settle()
        app.click_probe("controls.customize", "Customize")
        app.settle()
        app.step("customize", "Customize: panels, wheel, the key list with a search field")
        app.scroll_probe_into_view("controls.filter", "controls.customize", below="controls.close")
        app.click_probe("controls.filter", "the search field")
        app.type_text("split")
        app.settle()
        app.step("find-key", "the list narrowed to the command by its name")
        # ---- a key, a taken key ----
        cap = "controls.key.11.0"
        app.click_probe(cap, "the key of Split")
        app.key("q")
        app.settle()
        app.expect(wait_for(lambda: stored(store).get("controls") and len(stored(store)["controls"]) > 11), "the new key reached the store")
        app.step("assigned", "Split is on Q now: the cap shows it, the reset arrow appeared")
        app.click_probe(cap, "the key of Split again")
        app.key("m")
        app.expect(app.has_probe("controls.take"), "a taken key asks")
        app.step("taken", "M is taken: the row outside the scroll names the holder, with Take it and Leave it")
        app.click_probe("controls.keep", "Leave it there")
        app.settle()
        app.click_probe(cap, "the key of Split once more")
        app.key("m")
        app.settle()
        app.click_probe("controls.take", "Take it from there")
        app.settle()
        code = stored(store)["controls"]
        app.step("code", "the code under the tiles carries the change")
        app.click_probe("controls.code_copy", "Copy")
        # ---- the panels and the wheel ----
        app.scroll_probe_into_view("controls.layout", "controls.customize", below="controls.close")
        app.click_probe("controls.layout", "the panels list")
        app.settle()
        app.step("layouts", "the four arrangements by name")
        # a second press closes the list (Esc here closes the whole window: `escape_action` does
        # not see an open combo; the same in every window, written down in the round's report)
        app.click_probe("controls.layout", "the panels list closes")
        app.settle()
        for row in ("plain", "ctrl", "shift", "alt"):
            app.hover_probe(f"controls.wheel.{row}", f"the {row} wheel row")
        # ---- a wrong code, then the right one ----
        app.click_probe("controls.reset", "back to the style as it comes")
        app.settle()
        factory = wait_for(lambda: stored(store).get("controls") if stored(store).get("controls") != code else None)
        app.expect(factory, "the reset reached the store")
        app.scroll_probe_into_view("controls.code_input", "controls.customize", below="controls.close")
        app.click_probe("controls.code_input", "the code field")
        wrong = ("1" if code[0] != "1" else "2") + code[1:]
        app.type_text(wrong)
        app.settle()
        app.click_probe("controls.code_apply", "Apply")
        app.settle()
        app.step("code-refused", "a typo: the error says so, nothing changed")
        app.expect(stored(store).get("controls") == factory, "a wrong code changed the store")
        app.click_probe("controls.code_input", "the code field")
        app.key("ctrl+a")
        app.type_text(code.lower())
        app.settle()
        app.click_probe("controls.code_apply", "Apply")
        app.expect(wait_for(lambda: stored(store).get("controls") == code), "the right code in lower case is applied")
        app.step("code-applied", "the code brought the change back")
        # ---- one command back, and the unbinding cross ----
        app.click_probe("controls.reset.11", "Split back to the style")
        app.settle()
        app.click_probe("controls.key.11.0", "the key of Split")
        app.expect(app.has_probe("controls.unbind.11"), "listening, the cap offers to give the key up")
        app.key("Escape")
        app.settle()
        app.click_probe("controls.add.11", "one more key")
        app.key("Escape")
        app.settle()
        app.click_probe("controls.close", "the window closes")
        app.expect(app.wait_gone("controls.window"), "Close closed the style window")
        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "/tmp/kt-live-controls"))

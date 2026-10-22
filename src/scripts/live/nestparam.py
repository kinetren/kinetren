#!/usr/bin/env python3
"""Live scenario R.59 (stage 3): the news template as a whole, by hand.

The owner’s complaint verbatim: “someone created a template project for a news
broadcast… obviously people won’t be going into the composition each time and changing
the text and logos there”.

The harness tests t682-t685 check reachability and layout in a synthetic
scene. Here the question is different: does the path arrive ENTIRELY by a real mouse — from a
right click on a label inside the composition to a second broadcast that has its own
title, and back to editing the template, which both see.

Steps, each with its own observable consequence:

1. assemble the template: a title and a colored logo tile, collapsed into a composition;
2. INSIDE the composition publish the title (right click on the text field)
   and the logo width (right click on the field’s LABEL) — in the file the reel gets
   two declared handles;
3. place the composition a SECOND time from the bin;
4. on the second broadcast open “Composition parameters” and change the title
   and the width — the override file holds them ONLY for it;
5. edit the title in the template ITSELF — the edit reaches both: for
   the first broadcast the shown text changes (it has none of its own), while
   the second keeps its own.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from kt_live import Live, geometry, poll_step, project_with_clips, require_tools  # noqa: E402

SEC = 90000


def walk(node):
    """All document nodes of the .otio — publications and overrides lie in the sidecar
    `metadata.kt`, and the attachment in the file is a Stack inside a track."""
    if isinstance(node, dict):
        yield node
        for v in node.values():
            yield from walk(v)
    elif isinstance(node, list):
        for v in node:
            yield from walk(v)


def published_of(doc):
    """Declared handles of all reels of the document."""
    out = []
    for n in walk(doc):
        kt = n.get("metadata", {}).get("kt") if isinstance(n.get("metadata"), dict) else None
        if isinstance(kt, dict) and kt.get("published"):
            out.extend(kt["published"])
        if isinstance(n.get("published"), list) and n.get("published"):
            out.extend(n["published"])
    # one and the same sidecar occurs once in the walk, but we will be safe
    seen, uniq = set(), []
    for p in out:
        key = json.dumps(p, sort_keys=True, ensure_ascii=False)
        if key not in seen:
            seen.add(key)
            uniq.append(p)
    return uniq


def override_sets(doc):
    """Sets of overrides for all events — one list per event,
    where it is non-empty."""
    out = []
    for n in walk(doc):
        meta = n.get("metadata")
        if not isinstance(meta, dict):
            continue
        kt = meta.get("kt")
        if isinstance(kt, dict) and kt.get("overrides"):
            out.append(kt["overrides"])
    return out


def title_text(doc):
    """Title text INSIDE the template — what is edited once for all."""
    for n in walk(doc):
        meta = n.get("metadata")
        if not isinstance(meta, dict):
            continue
        kt = meta.get("kt")
        if isinstance(kt, dict) and isinstance(kt.get("gen"), dict):
            g = kt["gen"]
            if "text" in g:
                return g["text"]
    return None


def leave_reel(app):
    """Outward by an Esc CASCADE: one key does not do two things, so
    selection, strip and exit are cleared in turn (R.55)."""
    for _ in range(5):
        if app.geom()["nest"]["depth"] == 0:
            return True
        app.key("Escape")
        poll_step(0.5)
    return app.geom()["nest"]["depth"] == 0


def make_generator(app, group, kind, at_ticks):
    """Create a generator from the bin menu — the same path as for a person."""
    win = geometry()
    b = app.geom()["bin"]
    app.click(app.x_at(at_ticks), app.ruler_y())
    app.right_click(
        win["x"] + int((b["left"] + b["right"]) / 2),
        win["y"] + int(b["bottom"]) - 10,
    )
    app.click_probe("gen.new", "item “Create a generator”")
    app.hover_probe(f"gen.new.group.{group}", f"group “{group}”")
    app.click_probe(f"gen.new.{kind}", f"kind “{kind}” in the submenu")
    # a created generator opens its own effects window — we close it, otherwise
    # it overlaps the bin and the timeline
    app.key("Escape")


def video_row_ys(app):
    """Screen y of all VIDEO rows. An expanded strip adds its own rows
    with numbers from the tail of the numeric space (R.55), and the script has
    nothing to guess them from — it enumerates them."""
    g = app.geom()
    win = geometry()
    return [
        win["y"] + int(r["top"] + r.get("h", g["lane_h"]) * 0.5)
        for r in g["rows"]
        if r["kind"] == "video"
    ]


def open_window_in_rows(app, x, probe, avoid=None, limit=6):
    """Open the effects window of an event whose row is not known in advance.

    BY MENU ITEM, not a double click: inside an expanded strip a double
    click is the strip’s own gesture (collapsing, R.55), and the scenario would close
    what it itself opened. `avoid` is the instance row — we do not touch it."""
    for yy in video_row_ys(app)[:limit]:
        if avoid is not None and abs(yy - avoid) < 12:
            continue
        app.right_click(x, yy)
        poll_step(0.4)
        if app.has_probe("tl.fx", tries=2):
            app.click_probe("tl.fx", "item “Effects…” of the event")
            if app.has_probe(probe, tries=4):
                return True
        app.key("Escape")
        poll_step(0.3)
    return False


def main(out):
    require_tools()
    path = os.path.join(out, "nestparam.otio")
    proj = project_with_clips(path, [(0.0, 2.0, "synth:Ramp")])
    os.environ["KT_AUTOSAVE_SECS"] = "600"

    with Live("nestparam", out, project=proj) as app:
        y = app.lane_y("video", 0, 0.6)

        # --- 1. template: a title and a coloured tile ---------------------------
        make_generator(app, "titles", "text", 3 * SEC)
        make_generator(app, "shapes", "rect", 6 * SEC)
        app.shot("built")
        clips = app.live_clips()
        app.expect(len(clips) == 3, f"generators did not land on the montage: {clips}")

        gens = sorted(c for c in clips if c[0] >= 3 * SEC)
        app.click(app.x_at(gens[0][0] + gens[0][1] // 2), y)
        app.click(app.x_at(gens[1][0] + gens[1][1] // 2), y, mods=("ctrl",))
        app.right_click(app.x_at(gens[1][0] + gens[1][1] // 2), y)
        app.click_probe("nest.collapse", "collapse-into-composition item")
        app.expect(
            len(app.live_clips()) == 2,
            f"the template did not collapse into one composition: {app.live_clips()}",
        )

        # --- 2. publication INSIDE the composition ----------------------------
        inst = max(app.live_clips())
        inst_x = app.x_at(inst[0] + inst[1] // 2)
        app.right_click(inst_x, y)
        app.click_probe("nest.enter", "item “Enter the reel”")
        app.expect(app.geom()["nest"]["depth"] == 1, "entry into the composition did not happen")

        inner = sorted(app.live_clips())
        # the HEADER — on the text field itself: a multi-line field has no label
        # and creating one for a single caption is not allowed (rule 13)
        app.double_click(app.x_at(inner[0][0] + inner[0][1] // 2), y)
        app.expect(app.has_probe("titlew.text"), "the title window inside the composition opened")
        app.right_click_probe("titlew.text", "title text field")
        app.click_probe("pub.publish.text", "“Publish…” item on the title")
        app.expect(app.has_probe("pub.dialog.name"), "handle name dialog opened")
        app.shot("publish-dialog")
        app.click_probe("pub.dialog.ok", "publish confirmation")
        app.key("Escape")

        # the LOGO WIDTH — right click on the LABEL of the numeric field
        app.double_click(app.x_at(inner[1][0] + inner[1][1] // 2), y)
        app.right_click_probe("lbl.w", "logo width label")
        app.click_probe("pub.publish.w", "“Publish…” item on the width")
        app.click_probe("pub.dialog.ok", "width publish confirmation")
        app.key("Escape")
        app.shot("published")

        saved = app.saved_project()
        pubs = published_of(saved.doc)
        app.expect(
            len(pubs) == 2,
            f"declared handles in the file are not two: {[p.get('name') for p in pubs]}",
        )
        ids = {p["target"]["param"]: p["id"] for p in pubs}
        app.expect("text" in ids and "w" in ids, f"the file has not the right handles: {ids}")

        # --- 3. the second issue -------------------------------------
        app.expect(leave_reel(app), "Esc cascade led out")
        app.click(app.x_at(10 * SEC), app.ruler_y())
        card = next((f for f in app.geom().get("fields", []) if f["param"] == "bin.reel"), None)
        app.expect(card is not None, "composition card in the bin is shown")
        if card:
            win = geometry()
            app.right_click(
                win["x"] + int(card["x"] + card["w"] / 2),
                win["y"] + int(card["y"] + card["h"] / 2),
            )
            app.click_probe("bin.place", "“Place” item on the composition card")
        places = app.live_clips()
        app.expect(len(places) == 3, f"the second broadcast did not place: {places}")

        # --- 4. the second issue has ITS OWN values -------------------------
        second = max(places)
        # we aim at the START of the second issue, not at the middle: an instance
        # 8 s long lands under the cursor at the 10th second, and its middle
        # (14 s) at 100 px/s lies PAST the right edge of the window — the right click
        # went to the root WM menu, and the scenario collapsed in a cascade
        # (“`x_at` can only do what is ON SCREEN”, R.37)
        sx = app.x_at(second[0] + app.seconds(1))
        win_right = app.geom()["window"]["w"] + geometry()["x"]
        app.expect(sx < win_right - 8, f"click point inside the window: {sx} < {win_right}")
        app.click(sx, y)
        app.right_click(sx, y)
        app.click_probe("pub.inst_open", "“Composition parameters” item")
        app.shot("instance-window")
        app.expect(
            app.has_probe(f"inst.value.{ids['text']}"),
            "title row in the composition parameters window",
        )
        app.click_probe(f"inst.value.{ids['text']}", "title field of the second broadcast")
        for ch in ["v", "t", "o", "r", "o", "y"]:
            app.key(ch)
        # width: a click on the DragValue switches it into number input
        app.click_probe(f"inst.value.{ids['w']}", "width field of the second broadcast")
        for ch in ["ctrl+a", "0", "period", "9"]:
            app.key(ch)
        app.key("Return")
        app.key("Escape")

        # --- 4b. the CURVE of the handle: it is set up by the named action and is VISIBLE ----
        # The R.60 claim: “I nowhere see automation curves… there is not a single
        # button in the automation window”. Live we check exactly
        # this: the action has a name, after it a widget appears in the window,
        # and the keys go into the file.
        app.right_click(sx, y)
        app.click_probe("pub.inst_open", "composition parameters window once more")
        app.expect(
            not app.has_probe(f"inst.curve.{ids['w']}"),
            "a nameless keyframe-recording glyph remained in the window",
        )
        app.click_probe("inst.auto_add", "“+ curve” menu")
        app.click_probe(f"inst.auto_add.{ids['w']}", "“width” item in the curve menu")
        app.expect(app.has_probe("inst.auto"), "automation widget appeared in the window")
        app.shot("instance-curve")
        app.key("Escape")

        saved = app.saved_project()
        sets = override_sets(saved.doc)
        curved = [
            o
            for st in sets
            for o in st
            if o["pub_id"] == ids["w"] and o.get("curves")
        ]
        app.expect(
            len(curved) == 1,
            f"the handle curve did not reach the file: {sets}",
        )
        keys = curved[0]["curves"][0].get("keys", []) if curved else []
        app.expect(
            len(keys) >= 2,
            f"the starting curve did not lie as a constant over all the material: {keys}",
        )
        app.expect(
            len(sets) == 1,
            f"overrides are not on one event: sets {len(sets)}",
        )
        app.expect(
            bool(sets) and len(sets[0]) == 2,
            f"the second broadcast overrode not both handles: {sets}",
        )
        app.shot("second-differs")

        # --- 5. the edit of the TEMPLATE catches up with both issues ----------------------
        was = title_text(saved.doc)
        app.expect(was is not None, f"template title text found in the file: {was!r}")
        # the FIRST ISSUE — an instance of the same reel, not the source clip
        # of the project: a `min` over the whole edit would take it and click aside
        first = min(c for c in app.live_clips() if c[1] == second[1])
        fx = app.x_at(first[0] + first[1] // 2)
        app.right_click(fx, y)
        app.click_probe("nest.enter", "entry into the composition via the FIRST broadcast")
        inner = sorted(app.live_clips())
        app.double_click(app.x_at(inner[0][0] + inner[0][1] // 2), y)
        app.click_probe("titlew.text", "template title text field")
        for ch in ["z", "z"]:
            app.key(ch)
        app.key("Escape")
        app.expect(leave_reel(app), "Esc cascade led out after the template edit")

        saved = app.saved_project()
        now = title_text(saved.doc)
        app.expect(
            now != was,
            f"template edit was not recorded: was {was!r}, became {now!r}",
        )
        sets = override_sets(saved.doc)
        app.expect(
            len(sets) == 1 and len(sets[0]) == 2,
            f"template edit touched the overrides of the second broadcast: {sets}",
        )
        # the FIRST issue has no title of its own — it shows the new
        # template text; the SECOND has its own, and it stayed
        own = [o for o in sets[0] if o["pub_id"] == ids["text"]]
        app.expect(len(own) == 1, f"title override of the second broadcast disappeared: {sets}")
        # the override value lies tagged (`{"t":…,"v":…}`)
        app.expect(
            own and own[0]["value"].get("v") != now,
            f"the second broadcast became equal to the template: {own} against {now!r}",
        )
        app.shot("template-edited")

        # --- 6. PUBLISHING FROM THE STRIP does not pull inside (R.63 item 11) ---------
        # The owner: “if we publish a parameter without entering the composition
        # fully — then why do we automatically enter the composition fully?”.
        # We expand the strip (double click) and publish
        # FROM THERE: the entry depth must stay zero.
        first = min(c for c in app.live_clips() if c[1] == second[1])
        fx0 = app.x_at(first[0] + app.seconds(1))
        rows_before = len(video_row_ys(app))
        # the expansion is a MENU ITEM, not a double click: the double click has the
        # same effect, but it also collapses, and a script that misses
        # the row would close the strip silently.
        app.right_click(fx0, y)
        app.click_probe("nest.expand_here", "“Expand here” item")
        app.settle()
        app.expect(
            len(video_row_ys(app)) > rows_before,
            f"the strip expanded: there were rows {rows_before}, became {len(video_row_ys(app))}",
        )
        app.expect(
            app.geom()["nest"]["depth"] == 0,
            "strip expanded, but the person stayed at the root",
        )
        app.expect(
            open_window_in_rows(app, fx0, "titlew.text", avoid=y),
            "title window opened FROM THE STRIP",
        )
        app.right_click_probe("lbl.size", "title size label in the strip")
        app.click_probe("pub.publish.size", "“Publish…” item on the size")
        app.click_probe("pub.dialog.ok", "publish confirmation from the strip")
        app.expect(
            app.geom()["nest"]["depth"] == 0,
            "publish confirmation dragged inside the composition (item 11)",
        )
        app.shot("published-from-band")
        saved = app.saved_project()
        pubs = published_of(saved.doc)
        app.expect(
            len(pubs) == 3,
            f"handle from the strip did not land on the reel: {[p.get('name') for p in pubs]}",
        )

        # --- 7. item 10: the EFFECT HANDLE of an instance changes the FRAME ------------
        # “I added Radial blur, published the amount parameter, changed it
        # its value on the composition — and it does not change”.
        app.click_probe("fxreg.add", "effects picker in the title window")
        for ch in "radial":
            app.key(ch)
        app.click_probe("fxreg.tpl.radial_blur", "“Radial blur” line in the picker")
        # the title has its HEAD selected (its source), so the new slot must also be
        # selected — by the strip, as a person does. The zero slot of the strip is the
        # LEADING NODE “Geometry” (it is spliced into the strip at its position,
        # R.62), the blur came next.
        app.click_probe("fx.slot.1", "blur slot in the chain strip")
        app.shot("blur-added")
        app.expect(app.has_probe("lbl.amount"), "blur parameters shown")
        app.right_click_probe("lbl.amount", "blur amount label")
        app.click_probe("pub.publish.amount", "“Publish…” item on the amount")
        app.click_probe("pub.dialog.ok", "amount publish confirmation")
        app.key("Escape")
        saved = app.saved_project()
        pubs = published_of(saved.doc)
        amount = next((p["id"] for p in pubs if p["target"]["param"] == "amount"), None)
        app.expect(amount is not None, f"amount handle not declared: {pubs}")

        # we collapse the strip — by the same menu item (rule 14: the gesture
        # must work AGAIN)
        app.right_click(fx0, y)
        app.click_probe("nest.expand_here", "“Expand here” item a second time")
        app.settle()

        # the cursor goes to the second issue, we remember the frame
        app.click(app.x_at(second[0] + app.seconds(1)), app.ruler_y())
        app.settle()
        before_px = app.pixel_settled(app.preview_center())
        app.shot("before-amount")

        app.click(sx, y)
        app.right_click(sx, y)
        app.click_probe("pub.inst_open", "“Composition parameters” item")
        # the HEAD OF THE STRIP, not its own window (R.63 item 12)
        app.expect(app.has_probe("fx.head"), "composition parameters — head of the chain strip")
        app.expect(
            amount is not None and app.has_probe(f"inst.value.{amount}"),
            "amount handle is not in the head of the strip",
        )
        app.click_probe(f"inst.value.{amount}", "amount field of the second broadcast")
        for ch in ["ctrl+a", "0", "period", "9"]:
            app.key(ch)
        app.key("Return")
        app.settle()
        app.shot("after-amount")
        after_px = app.pixel_settled(app.preview_center(), differs_from=before_px)
        app.expect(
            after_px != before_px,
            f"frame of the second broadcast did not change from the blur handle: {before_px} → {after_px}",
        )
        saved = app.saved_project()
        sets = override_sets(saved.doc)
        app.expect(
            any(o["pub_id"] == amount for st in sets for o in st),
            f"amount override did not reach the file: {sets}",
        )

        return app.done()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "."))

---
id: F-HLP-005
title: Live modifier hint — what changes if you hold Shift/Ctrl/Alt
status: candidate
priority: B
cost: S
category: help/hints
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2]
we_have: partial
crates: [kt-app]
invariants: [12]
ui_rules: [7, 9, 10, 13]
round: null
point: null
tests: []
depends_on: [F-HLP-001, F-UI-002]
checked: 2026-09-07
---

# F-HLP-005 — Live modifier hint — what changes if you hold Shift/Ctrl/Alt

## Essence

The editor holds the cursor over an event. The hint line says what dragging does. He presses Ctrl — and the line CHANGES: now it says what dragging does with Ctrl. Let go — the previous one returns. This is not "what is this", but "what else is possible right now, without releasing the mouse": the layout is not listed all at once, but is shown by the slice selected by the current state of the keyboard.

## Who has it and how

The question of the table's row is one: does the editor show a LIVE answer to a held modifier ABOVE THE EDIT (the timeline, the viewer), and not in a key-settings dialog.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | the status line is occupied by hints of menu items, the recording time and progress; above the timeline a modifier changes only the SHAPE OF THE CURSOR (Alt — slip, Ctrl+Alt — slide, Alt+Shift — slip-trim of the right edge, Ctrl+Alt+Shift — "eraser"), there are no names, the list lives in the help |
| Adobe Premiere | 26.x | none above the timeline | a modifier changes the trim cursor (ripple/roll) and the colour of the indicator; there is a live layout, but only in the Keyboard Shortcuts window: "Press the modifier key on your hardware keyboard" redraws the keyboard entirely |
| DaVinci Resolve | 21 | none above the timeline | Keyboard Customization — a virtual keyboard, the modifier layer is selected by a CLICK on the drawn key; the reaction to a physical press is not confirmed [NOT CHECKED] |
| Final Cut Pro | 12.3 | none above the timeline | Command Editor: "When you press any modifier key on your keyboard… the key colors update" — a live recolouring of the whole keyboard, but this is a registry of commands, not the object under the cursor |
| After Effects | 26.x | none above the composition | a visual editor of keys: pressing Shift/Ctrl highlights the corresponding layer of the layout; above the frame itself the modifiers are described on a separate help page, not in the interface |

**Outside the core.** Without Blender the feature cannot be described: this is the only verified program where a live redrawing of the layout is done ABOVE THE WORKING AREA, not inside the key-settings dialog — the five core NLEs give exactly a dialog.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | Status Bar → Keymap Information | "The left side of the Status Bar displays mouse button shortcuts and the keymap of the active tool"; the layout is redrawn for a held modifier, and a tap on Alt shows the hotkeys of the tools and is switched off by the setting Alt Click Tool Prompt |

## The trick behind it

**The load of the mechanism is not a paragraph and not a picture, but COMBINATORICS**: the Cartesian product "zone under the cursor × mouse button × modifier". It cannot be put into one line by any lengthening of the line — it grows by multiplication, while the line grows by addition. For us this load does not move anywhere: it DISAPPEARS. The live layout does not show the product, it shows one of its slices, and the user himself chooses the slice — by pressing a key. Capacity ceases to be the limit of the description, without growing by a single character.

Three details that are not in the list of features:

1. **The modifier here is a second pull gesture, of exactly the same kind as hovering the mouse.** The formulation belongs to the leading developer of Blender: "Just as you hover your mouse over a button to see it's tool-tip, you can press modifiers to see what they do" — and right after it the honest price: "If we want to show current available keys we could - but we would need to use a lot more space". That is, the choice is made not between "show" and "do not show", but between "show everything at once" and "show on request", and Blender chose the second for exactly the reason for which we have the 160-character ceiling.
2. **This is paid for by manual maintenance of every operator.** Blender's status bar line is maintained BY NAME, it is not derived from the gesture's code: in January 2025 a developer was collecting instructions from the community on how to reach a dozen and a half operators at all, to see what they say in the status bar. The mechanism drifts from the code silently — and this is exactly what we cure with a test rather than with prose (the precedent of R.38).
3. **The live redrawing is so contested that it has been moved into a setting.** A tap on Alt (the hint of the tools' hotkeys) is switched off by the item Alt Click Tool Prompt: the vendor knows that a flickering layout bothers some users, and does not argue with them, but gives a switch.

## Pros

- The line becomes SHORTER, not longer: instead of "Clip: drag — move (Ctrl — no magnet), Ctrl+click — select, double click — FX, right click — menu, S — split" (107 characters en, 116 es) "drag — move, double click — FX, right click — menu" remains, while the content of Ctrl goes into a slice that occupies the same single line and only when Ctrl is pressed.
- The capacity multiplies without a single new pixel: three modifiers give eight keyboard states, that is up to eight lines of 160 characters for the same zone, while at the same time there is still exactly one on the screen.
- The slice does not lie about the zone, but the common list lies: Ctrl above the body of an event removes the magnet, Ctrl above an empty place starts a selection frame (`src/apps/kt-app/src/timeline.rs:1996` and `:2544`) — these are different answers, and today both are written into different lines, which the user reads one after another, guessing which one applies to his case.
- It teaches at the moment of action: the modifier is pressed when the hand is already on the key and the gesture is already conceived, not when one reads the help.
- The mechanism is half ready for us: the keyboard state is already polled every frame, and a press of a modifier by itself gives a redraw (see "How it fits") — only the hint text needs to look at it.

## Cons

- **Pull remains pull.** To press Ctrl in order to learn what Ctrl does — is guessed by someone who already suspects that Ctrl does something. This is exactly the objection from the Blender thread: the live layout works "after the fact". It does not cure, and cannot cure, the cry "WHERE DID THE BUTTON HINTS GO?!?!?!" (R.9 item 1) — the push stays with other notes of the wave.
- Every slice is a new catalog key × 5 languages with the whole price of F-HLP-033 (glyphs, subsets, Arabic bidi, t125/t131). Full coverage "zone × modifier" is dozens of keys; it turns out cheaper to cover only the zones where the modifier really changes something, and we have six of them.
- The line stops being calm. Any Ctrl+C, Ctrl+S, Ctrl+Z is a press of Ctrl, and the hint will flicker with a slice for the duration of the hold. It is cured only by drawing the slice when the cursor stands over a zone for which this modifier means something.
- **Alt is not occupied by anything with us**: across the whole `src/apps/kt-app/src/` it occurs only as `!i.modifiers.alt` (a prohibition), and not one gesture is behind it. The honest slice for Alt would be empty, and an empty or faded hint is forbidden by rules 9 and 16 of the checklist — so Alt does not take part in this feature at all, and "Shift/Ctrl/Alt" from the title reads with us as "Shift/Ctrl".
- Manual maintenance: see item 2 of "The trick behind it" — at Blender this became the main item of expense six years after the mechanism was launched.

## What users say

> It actually does respond to modifier keys. Try it - hold down a modifier and
> you'll see the status bar update.
> — billrey (Blender UI developer), devtalk.blender.org, 21.02.2019,
> https://devtalk.blender.org/t/showing-keys-in-the-status-bar/5793/7

> Yes, but that is "after the fact". It would be nice if it would always say
> [Ctrl] - Does this [Alt] - Does that.
> — eobet, devtalk.blender.org, 22.02.2019,
> https://devtalk.blender.org/t/showing-keys-in-the-status-bar/5793/8

> The problem with this is you end up having to expand a lot more of the key-map
> than fits in the status bar.
> — ideasman42 (Campbell Barton, Blender core), devtalk.blender.org, 22.02.2019,
> https://devtalk.blender.org/t/showing-keys-in-the-status-bar/5793/9

The thread as a whole is a ready analysis of our own dispute: a user asks for a static list of all the modifiers, the developers answer that it does not fit, and present a live reaction as the solution to the capacity problem. Six years later the maintenance of this mechanism still requires manual work:

> We have been trying to make operator status bar displays of keymap entries
> clearer and more informative using icons, etc. We've made great strides, but
> there are still some stragglers that need attention.
> — Harleya, devtalk.blender.org, 18.01.2025,
> https://devtalk.blender.org/t/community-help-needed-for-status-bar-updating/38599

## How it fits Kinotrain

### What already exists

The slot and its machinery — F-HLP-001 (`status_line` in `src/apps/kt-app/src/app.rs:4426`, `hint()` at `:1505`, rendering `draw_status_hint` at `:1274`). What matters here is something else: **the keyboard state is already read every frame, and only the text is lacking it**.

- `src/apps/kt-app/src/timeline.rs:1996` — `self.snap_off = ui.input(|i| i.modifiers.command || i.modifiers.ctrl)`, one line below `ctrl_held`, one more line — `shift_held`. This is done BEFORE the hit-tests, that is, in the same frame where the zone under the cursor is computed and the hint is chosen (`:2178`–`:2265`). The live slice technically means "pick another catalog key in the already ready `match`", not a new subsystem.
- `src/apps/kt-app/src/timeline.rs:1284` — the same `ctrl`/`shift` for the wheel.
- The cursor already changes by zone (`set_cursor_icon` in the same branches) — that is, a live feedback we ALREADY have, but it is a picture without a name, exactly like VEGAS with its slip/slide/eraser cursors.
- The modifiers are listed statically in 17 values of `en.toml` out of 522. Two of them are the most loaded strings of the catalog: `tl.hint.clip` (six actions, two mentions of Ctrl; 107 characters en, 116 es, 109 ru, 99 ar) and `tl.hint.empty` (six actions, three mentions of modifiers — `Ctrl+drag`, `Ctrl`, `Shift`; 101 en, **124 es**, 107 ar, 103 ru).
- The harness can press a modifier without moving the mouse: `h.event(egui::Event::ModifiersChanged(modifiers))` is already used in `src/apps/kt-app/src/layout_tests.rs:4020` (t206). The test for the live slice does not have to be written from scratch.

### What is missing

- No line of the catalog depends on the state of the keyboard: a slice does not exist as a concept.
- **Four live modifier branches are named NOWHERE** — neither in a hint nor in a menu: Shift+Ctrl+wheel = vertical zoom of track height (`src/apps/kt-app/src/timeline.rs:1291`); Ctrl+wheel = zoom of the automation curve (`src/apps/kt-app/src/automation.rs:408`); Shift/Ctrl+click and Ctrl+drag in the bin = the selection set and the frame (`src/apps/kt-app/src/bin_panel.rs:208`, `:702` — the bin hints `hint_free`/`hint_grid`/`hint_group` are silent about this); Shift+double click on an event = the region by event (`timeline.rs:2476`) — named only in the context menu item `tl.menu.region_from_event`, but not in the hint of the event on which the gesture is performed. This is not "too little text", it is the absence of room: in `tl.hint.empty` the fourth wheel combination simply did not fit.
- During a gesture the slot is empty. `if self.drag.is_none() && let Some(pos) = hover` (`src/apps/kt-app/src/timeline.rs:2178`) — while `Drag::*` runs, zone hints are not written at all, `hover_hint` stays `None`, and `status_line` falls through to the standby line: while the user drags an event, he is offered "drop a media file on the window". The only thing that can reach the line during a drag is `self.blocked` (`timeline.rs:2624`, `:2657`), that is, the RED error of a stop (t172). Rule 10 ("exactly one hint") forbids nothing here: there is no conflict, the slot is free. What to write into it during a gesture — the trim delta, the offset, the new timecode — is the subject of F-HLP-007; from here only the link goes there: a modifier held DURING the gesture (`snap_off` lives exactly in the drag) — this is the same slice, and it must displace the standby line, not compete with it.

### What gets in the way

- **The 160-character ceiling** (`src/apps/kt-app/build.rs:42`, `MAX_VALUE_CHARS`) is already almost used up: `tl.hint.empty` in Spanish is 124 characters, the margin is 36, that is exactly one short addition and nothing more. This is the second longest line of the Spanish catalog (the first — `mrph.below_note`, 145). The other side: the live slice removes the pressure by itself, each of the lines becomes half as long as the original.
- Arabic is reordered WORD BY WORD and does not wrap — a long line of the catalog costs the most there; `tl.hint.empty` in `ar.toml` is 107 characters.
- Alt is empty (see "Cons"), and inventing a meaning for it for the sake of symmetry is not allowed.
- One place (rule 10) — but this is not a conflict: the slice does not add a second hint, it REPLACES the same one with its refined version, in the same rectangle, with the same `layout_no_wrap`.
- The ban on popup hints (item 14 of R.8) and its guard **t83** are not affected by anything: no new layers appear, the drawing order does not change, the delay stays zero. The note does not argue with either of the owner's two decisions.

### Which invariants it touches

- **Invariant 12 — not violated, and this is verified by the sources, not by general reasoning.** A press of a modifier WITHOUT mouse movement arrives as `WindowEvent::ModifiersChanged`, and `egui-winit` 0.36.1 (the version from `Cargo.lock` via `eframe` 0.36.1) answers it with `EventResponse { repaint: true }`, additionally putting `egui::Event::ModifiersChanged` into the frame's event queue (`~/.cargo/registry/…/egui-winit-0.36.1/src/lib.rs:484-510`). That is, the redraw happens from an INPUT EVENT — exactly what invariant 12 permits — one frame per press and per release. The idle tick stays 250 ms (`repaint_delay(false)`, `src/apps/kt-app/src/app.rs:4285`, guard t20). Hence the strict implementation constraint: the slice is read from `i.modifiers` in the same frame and is NEVER polled by a timer — polling immediately breaks t20 and invariant 12.
- **Rule 7** (the text fits) — the feature eases it, does not load it. **Rule 9** (nothing superfluous) — forbids an empty slice for Alt. **Rule 10** (one hint) — observed by construction. **Rule 13** (do not waste space) — the main argument "for": today half of the line is occupied by variants that do not relate to the current state of the keyboard.
- **t128** (the label budgets) does NOT measure the status line — it checks menu headings, perf-panel rows and the checkbox. The gap is real, but its address is already taken by F-HLP-002 (`reach_tests::hint_fits_the_status_strip`); this note does not lay claim to it and only relies on the fact that its slices are SHORTER than the current lines, and therefore certainly fit into someone else's future budget.
- The project format, the engine and any invariants 1–11 are not touched: the feature is entirely inside `src/apps/kt-app`.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage of a round |
| Where the main work is | kt-app (`timeline.rs` — choosing the key by `ctrl_held`/`shift_held` in the already existing `match` of the zone; `i18n/*.toml` — slices for the six zones where a modifier means something) |
| Project format | not touched |
| Tests to be written | `reach_tests::every_modifier_branch_is_named` (a scan of live `i.modifiers.*` branches in production code against the catalog — today fails on four); a harness test "`ModifiersChanged` without mouse movement changes `status_line_shown`, and without a press returns the previous line"; t100/t125/t128 automatically for the new keys |
| Depends on | F-HLP-001 (the slot and the priorities), F-HLP-007 (what to write into the same slot during a gesture), F-UI-002 (the command registry — the boundary) |
| Breaks compatibility | no |

**What this adds BEYOND the command registry (F-UI-002).** The registry answers the question "which key invokes command X" and does so uniformly for the whole program. The live slice answers another question — "what these keys do WITH WHAT IS NOW UNDER THE CURSOR" — and its answer differs in different zones of the same timeline. Fundamentally: Ctrl in moving an event and Shift in the wheel are NOT COMMANDS, they are not and cannot be in `enum Command` (they do not get into undo nor into t252), therefore they will never appear in the key editor — neither with us nor with Premiere or FCP. This is exactly why the five core NLEs, having a live layout in the key-settings dialog, show nothing above the timeline.

## Verdict

We take it. This is the only position of the wave where the proposal does not increase the load but reduces it: the line becomes shorter, more precise, and stops lying about the zone, while neither overlap, nor delay, nor the number of hints on the screen changes, and half of the mechanism (polling modifiers every frame and redrawing on an input event) is already written and working. The limitations are honest: the push does not cure this, Alt does not take part in the feature, and without a test the lines will drift from the code exactly as happened with Blender.

TEST: reach_tests::every_modifier_branch_is_named — today fails on four branches: Shift+Ctrl+wheel (vertical zoom, `timeline.rs:1291`), Ctrl+wheel over the curve (`automation.rs:408`), Shift/Ctrl+click and Ctrl+drag in the bin (`bin_panel.rs:208`, `:702`) and Shift+double click on an event (`timeline.rs:2476`) — the live `i.modifiers.*` branches exist in the code, while a line naming them exists in no catalog.

ONE LINE: overlap — not touched | delay — not touched | capacity — does not grow, but falls: on the same single status-bar line the static list of modifiers is replaced by a slice according to the current keyboard state (`tl.hint.clip` 107→~60 characters), and the room for the four branches that are unnamed today is taken from the freed space.

## Sources

- https://docs.blender.org/manual/en/latest/interface/window_system/status_bar.html (Blender 5.2 LTS Manual, Status Bar → Keymap Information; accessed 2026-09-07)
- https://devtalk.blender.org/t/showing-keys-in-the-status-bar/5793 (the 2019 thread in full, posts 5–12; accessed 2026-09-07)
- https://devtalk.blender.org/t/community-help-needed-for-status-bar-updating/38599 (January 2025; accessed 2026-09-07)
- https://helpx.adobe.com/premiere/desktop/get-started/keyboard-shortcuts/about-keyboard-shortcuts.html (Premiere, the visual layout and "Press the modifier key on your hardware keyboard"; accessed 2026-09-07)
- https://support.apple.com/guide/final-cut-pro/view-keyboard-shortcuts-ver4df74f72/mac (Final Cut Pro, Command Editor: "When you press any modifier key on your keyboard… the key colors update"; accessed 2026-09-07)
- https://helpx.adobe.com/after-effects/using/keyboard-shortcuts-reference.html (After Effects, the visual key editor; accessed 2026-09-07)
- http://vegaspro.helpmax.net/en/using-vegas-software/editing-events-on-the-timeline/slipping-and-sliding-events/ (VEGAS, the full list of drag modifiers and "The cursor is displayed as a …"; accessed 2026-09-07)

Caveats on trust in the sources:

- `helpx.adobe.com`, `docs.blender.org` and `devtalk.blender.org` were not opened from this environment during the first wave; here the text proxy `r.jina.ai` and the Discourse API of the forum (`/t/<id>.json`) were used — the techniques recorded in `_editors.md`.
- The VEGAS line rests on the `helpmax` mirror of the help (the text of the Vegas Pro 13 line) and on the description of the status line in the PDF manuals of VEGAS 14–22. Whether none of these mechanisms changed in 2026.0 — [NOT CHECKED].
- The Resolve line: the live reaction of the virtual keyboard of Keyboard Customization to a PHYSICAL press of the confirmation modifier was not obtained; the documentation describes a click on the drawn key — [NOT CHECKED].
- No live check was made for any of the six programs (for non-NLEs it is not assumed, `_editors.md`); everything said about Kinotrain is checked against the sources and given with line numbers.

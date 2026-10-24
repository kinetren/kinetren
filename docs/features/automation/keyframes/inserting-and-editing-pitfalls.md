---
id: F-AUT-005
title: Keyframe editing traps
status: candidate
priority: B
cost: M
category: automation/keyframes
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-app, kt-model]
invariants: [11]
ui_rules: [1, 14, 17, 20]
round: null
point: null
tests: []
depends_on: [F-AUT-001, F-AUT-002]
checked: 2026-09-08
trackers: [kde-invent, kde-bugzilla, shotcut, olive]
---

# F-AUT-005 — Keyframe editing traps

## Essence

The first key is placed easily. Then the work begins: insert a key BETWEEN
ones already standing, select several at once, move them in a batch, copy a piece of animation to another place, delete an extra one. Each of these five gestures
can spoil what has already been done — and it spoils not the stretch that was pulled. A note about the gestures of EDITING; the interpolation types of a key — F-AUT-001, what the key is tied to in time — F-AUT-002.

## Who has it and how

The question for each one is the same: how is a key inserted between existing ones, how are several selected, what can a batch do, and how is a key deleted.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | envelope points; Envelope Edit Tool | a point is inserted by DOUBLE-CLICKING on the envelope itself. Multiple selection is not a gesture but a separate TOOL on the panel: the help explicitly lists what cannot be done in normal mode ("box selection, box magnification, and multiple envelope point selection"). In tool mode the events do not move at all, but the points are selected by a box and dragged as a batch, Alt keeps the move horizontal |
| Adobe Premiere | 26.x | Effect Controls keyframes | a box with the Selection tool right in Effect Controls, Shift-click and Ctrl/Cmd-click — and one can select keys of DIFFERENT parameters of one clip at once. Moving — Alt+drag. Copy-paste by the ordinary Edit → Copy/Paste, pasting from the current cursor position, and the pasted keys REMAIN selected so they can be moved right away |
| DaVinci Resolve | 21 | Keyframe Editor + Curve Editor | a box around keys in the Curve Editor, Shift/Cmd-click; drag one selected — all move. In version 21 a mode of keys for a GROUP of clips appeared: editing goes by the selection on the timeline, not by one clip. The curve and the keys live in two different panels, and a key selected in one is not always "selected" for the other |
| Final Cut Pro | 12.3 | Video/Audio Animation editor | Shift-click and box. Its own rigid feature: Shift-click selects ALL intermediate keys — "You can't select nonadjacent keyframes". A batch moves horizontally in time, vertically changes values keeping them relative to each other; proportional stretching of TIMING is absent from the help, there is only a shift by a frame with the comma and period keys |
| After Effects | 26.x | Timeline keyframes, Graph Editor | box, Shift-click, copy-paste to the CTI position. The main gesture of a batch is Alt+drag on the EXTREME selected key: it stretches the timing of the whole group proportionally, keeping relative distances. It is also the only way to "make the animation twice as long" without recalculating each key by hand |

## The trick behind it

**1. "Inserting a point" is TWO different actions, and the trackers have been confusing them for five years.** Kdenlive task #123 was filed 31.03.2019 as "Rotoscoping — Resize
the whole mask at once", and closed 08.09.2024 under the heading "Inserting
points when we already have keyframes defined can lead to unexpected
behaviors": 27 comments went into understanding that the issue was about a different insertion. In rotoscoping the VALUE of a key is not a number but a structure (a spline of N
points), and adding a point to the contour must retroactively appear in ALL
already placed keys — otherwise the frames stop being interpolable into each other. The second insertion, "a key between two keys in time", has no relation to this at all. The practical conclusion for us is direct: while a key's value is a scalar (`AutoKey { t, v: f64 }`), class #123 does not exist for us BY DESIGN, and the first parameter that is a structure (a mask contour, F-MSK-001) will bring it in whole. This is not a "simplification of the model", it is a boundary that is worth guarding consciously.

**2. VEGAS made multiple selection a MODE, and the price is named in the help itself.** For all the other four a box around keys is an ordinary gesture on an ordinary view. In VEGAS, for its sake, one must switch the tool, and in that tool the events themselves stop moving. The help does not hide this, it lists: three functions absent in normal mode, one of which is selecting several envelope points. This is what an honest price of the decision "gestures do not compete for one press zone" looks like: for us the same fork is already resolved on the timeline (Ctrl+drag and the Selection tool button, items 202–205), and resolved in the other direction — by a box without switching the tool.

**3. "A hotkey for TWO keys at once" is a request for hold, submitted from below, and it came from SOUND.** Kdenlive #1368 (16.03.2022, closed 03.10.2025 — three and a half years) asks for a button and a hotkey that place a pair of keys with a half-second gap: the first holds the ORIGINAL value, the second — the new one. The mechanism behind the request is visible through: where a key has a "hold" type, the second key is not needed at all — the value holds by itself; and where there is no such type, every edit "make it quieter here" spoils the WHOLE stretch to the left, because a single key drags everything from the start of the clip. Telling is the tag: of 98 kde-invent tasks with the word "keyframe" in the title, EXACTLY ONE carries the `Audio` tag, and it is #1368. That is, the common defect of the animation mechanism was first felt not on moving pictures, but when putting music under speech, where two keys are needed in every second gesture.

## Pros

The strengths of others' designs — what we do not have.

- A box around keys is one gesture for all five tasks at once: select, move in time, raise values, delete, copy. Each separate "delete key" button (Shotcut) is the same task solved N times, and N times differently.
- Alt+drag on the extreme key (AE) scales the timing of a GROUP: an animation doubles in length with one gesture, without arithmetic on each key. This is the only operation in the list that cannot be assembled from single edits in a reasonable time.
- Selecting keys of DIFFERENT parameters at once (Premiere) makes "shift this whole moment by 10 frames" one movement, not parameter by parameter — and on a clip with four curves this is the typical edit.
- Keys pasted by copy-paste REMAIN SELECTED (Premiere): pasting and fine-tuning the position is one continuous gesture, not "pasted, lost, search".

## Cons

- A box and a single click share one press zone, and separating them has to be done by a mode (VEGAS) or a modifier. Any choice here bothers someone: the mode takes gestures away from events, the modifier is occupied by something else.
- Selection of "adjacent only" (FCP) looks like an implementation limit but behaves like a trap: a Shift-click on a distant key silently takes everything in between, and a value edit drifts onto keys one did not want to touch.
- Two panels for one animation (Resolve: Keyframe Editor and Curve Editor) double the notion of a "selected key", and they are not always in sync — hence the open issue "you cannot select a specific key to change its interpolation when the Curve Editor is open" (kde-bugzilla catches the same class in Kdenlive, #517035).
- A "delete key" button tied to a parameter row is a source of a classic addressing error: in Shotcut a button on one row deleted the key of another row (#921), and there the Del key deleted the TRACK instead of the key (#992). Both are not about keys, but about the key editor living inside someone else's window and sharing the keyboard with it.
- The group timing-scale gesture (AE) has no inverse: "stretched by 2, compressed by 0.5" does not return the same frames because each key is rounded to the grid.

## What users say

The request reveals the mechanism: two keys are needed because one is not enough for the value to HOLD.

> a lot of times (on audio editing and also on zooms) it's necessary to
> create 2 keyframes at once, the first one have the original value of
> effect and the second one is the one that is used for "real".
> — author of Kdenlive #1368, 16.03.2022,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1368

The same person, five months earlier, about hitting the right value of a volume key with the mouse:

> lowering music when someone is talking is a pain right now.
> — Kdenlive #1227, 21.10.2021,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1227

A key that does the wrong thing because the key editor lives in someone else's window (open since 27.12.2020 — almost six years):

> While editing filter's keyframes, pressing Del key do not delete the
> selected keyframe but this delete the entiere track.
> — Shotcut #992, 27.12.2020,
> https://github.com/mltframework/shotcut/issues/992

And a gesture that is missing — selecting several keys in order to give them the SAME value, not to move them:

> Selecting multiple keyframes and giving them all the same properties is
> a good idea.
> — Kdenlive #828, 08.11.2020,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/828

SIGNAL: key-editing gestures — 27 tasks in kde-invent, `(insert|add|delete|remove|copy|paste|move|select|drag)\w*[^.]{0,20}keyframe|keyframe[^.]{0,20}(insert|add|delet|remov|copy|past|mov|select|drag)`; share 1.2% against 0.3% in shotcut

SIGNAL: a batch of keys in one gesture — 6 tasks in kde-invent, `(multiple|several|all|group of|multi.?select)[^.]{0,20}keyframe|keyframe[^.]{0,20}(multiple|multi.?select|at once|together)`; share 0.3% against 0.1% in shotcut

The fourfold difference between kde-invent and shotcut is read not as "Kdenlive is worse", but as "Kdenlive's key editor is developed to the point where there is something to ask for": Shotcut has four tasks on the topic — three delete buttons and one Del key, that is, the level "the button deletes the wrong key". At the same time a batch of keys is five times rarer than single gestures in BOTH trackers: multiple selection is not the first need but the second, and there is no sense in introducing it before the single gestures are fixed.

## How it fits Kinotrain

### What already exists

- One widget for all curves — `src/apps/kt-app/src/automation.rs`
  (`draw` → `toolbar` + `graph`/`table`), and it serves all three
  carriers: the effect slot (`fx_editor.rs`), the camera (`camera_editor.rs`),
  the transition (`transition_editor.rs`). Nobody has their own key editor
  (item 221).
- **Placing a key — four paths.** The `+` button of the toolbar puts a key at
  the cursor position with the value of the CURVE itself (`kt_model::curve_value`); a click
  on an empty spot of the graph — with the value taken from the pointer's Y; the button
  in an empty table cell — again `curve_value`; creating a curve in
  `fx_editor.rs:419` immediately places TWO keys (`src_in` and the end of the material) with the
  current value of the parameter.
- Dragging a key snaps it to the frame grid
  (`raw - raw.rem_euclid(step)`), the value window is frozen during the drag
  (`AutoView.drag_win`), and the selection follows ITS key after re-sorting.
- Deletion — three paths: the trash button in the toolbar, a double-click on a key on the graph,
  the cross in a table cell.
- Copying — at the level of the ENVELOPE, not of keys: `ops::EnvelopeBlock`
  (`src/libs/kt-model/src/ops.rs`) carries `fx_tag`, the curves and `src_in`
  of the source event; `paste_envelope` checks the compatibility of the effect kind
  and re-hangs the keys through `rebase` ("entry point to entry point", without
  scaling, item 215). There is also `FxChainBlock` for the whole chain.
- The geometry of keys is not broken by event edits: the domain is the material tick, trim/`Split`/reverse do not touch the curves (F-AUT-002, t212).
- The order of keys is fixed by sorting on every edit, and in the table — only on release (`AutoView.t_edit`, t221): a direct complaint of the user in R.33 item 5 about "resets/sorts neighboring values".

### What is missing

- **Multiple selection does not exist at all.** `AutoView.sel` is an
  `Option<(usize, usize)>`: exactly one key of exactly one curve. So there is neither
  a box, nor batch moving, nor "set one value for five keys" (Kdenlive #828),
  nor scaling of a group's timing (the AE gesture). The user asked for this directly
  back in R.33 item 6: "Copy-paste is missing… as well as automation (with the future timeline it turns out one can select and ctrl+c ctrl+v,
  etc.?)" — the answer was given at the level of the ENVELOPE; the question about selection remained unanswered.
- **Copying of individual keys does not exist** — the whole curve of a slot or one parameter entirely is copied (`copy_envelope(c, slot, param)`). "Copy this piece of animation to another place of the same clip" is not expressible.
- **There is no hold key** (this is F-AUT-001), and therefore we have exactly the need from which Kdenlive #1368 grew. It is half closed already, and this should be said out loud: creating a curve places TWO keys at once. But it places them at the ENDS of the material, not next to the cursor, so "let it hold until here, then drop" still requires dragging the first key by hand.

### What gets in the way

1. **`ease` is a flag on the CURVE, but smoothstep is computed PER SEGMENT, and because of this inserting a key is NOT neutral.** Verified by the code and calculated: `curve_value` (`src/libs/kt-model/src/project.rs:1932`) normalizes the position within a pair of neighbors and, with `ease`, passes `s` through `s*s*(3-2s)`. Take a curve 0→1 over 90000 ticks and insert with the `+` button a key exactly in the middle with the value of the curve itself (0.5). With `ease = false` the values change nowhere: 0.25 / 0.5 / 0.75 before and after. With `ease = true` the quarter changes **0.15625 → 0.25**, and the three-quarter point **0.84375 → 0.75** — that is, the "neutral" key rewrote BOTH neighboring stretches by 60 % in relative magnitude. The reason goes deeper than the flag: the derivative of smoothstep is zero at both ends of a segment, so EVERY key is a full stop of motion, and adding a key in the middle of acceleration adds a pause there. And `ease: true` is the default in all three places where the UI creates a curve (`fx_editor.rs:419`, `camera_editor.rs:138`, `transition_editor.rs:263`), and `default_true` is the serde default for old files. The conclusion for which the item is written: one CANNOT claim "with us, inserting a key does not spoil the neighbors, because ease lives on the curve" — this holds only for the linear mode, which is off by default.
2. **A key snaps to the frame grid only when dragged.** The drag does `raw.rem_euclid(step)`, while inserting by a click on an empty spot takes `Tick(t_of(pos.x).clamp(m0, m1))` without alignment, and the `+` button takes the cursor position, which comes from 48 kHz samples and is not a multiple of the project's frame grid (`90000/fps`) — also off the grid. The promise of `src/apps/kt-app/CLAUDE.md` ("keys snap to the frame grid") is kept by one path out of three — this is rule 17 of the checklist ("paired methods of one action behave identically") in pure form.
3. **The model accepts two keys on one tick, but the widget confuses them.** Insertion deduplicates (`c.keys.retain(|k| k.t != t)`), but DRAG does not: it writes `curves[ci].keys[ki] = AutoKey { t, v }` and after sorting looks for its key through `position(|k| k.t == t)`, which returns the FIRST match. Drag a key exactly onto its neighbor (which the frame grid makes easy) — and from then on you drag someone else's, leaving your own standing. `check_fx_auto` skips duplicates (the condition `w[0].t <= w[1].t` is non-strict), so the pair survives saving; `curve_value` on it gives `span.max(1)` — a vertical jump, that is, an accidental hold, which officially does not exist in the model.
4. **The Delete key with a key selected deletes the CLIP.** `handle_keys` (`src/apps/kt-app/src/app.rs:3879`) bails out only on `ctx.egui_wants_keyboard_input()`, and that is raised only by text widgets; the automation graph is `Sense::click_and_drag`, and neither `automation.rs` nor `fx_editor.rs` reads any `egui::Key`. So Delete goes to `delete_action` and removes the selected morpher, camera or clip — together with that same chain and its curves. This is verbatim Shotcut #992, reproduced by us, and it is not caught by any test.
5. **The three carriers of curves create them differently.** A clip gets TWO keys over its whole material; a camera and a transition — ONE at zero of their axis. One widget, three different starting states; rule 17 again.
6. **`rebase` clamps at zero** (`Tick((k.t.0 + d).max(0))`): inserting an envelope into an event with a smaller `source_in` collapses all keys hidden on the left onto the zero tick — several keys at one point, the case from item 3, but now without the mouse involved.

ADDRESS: `src/apps/kt-app/src/automation.rs` — the class is possible, and nothing catches it: the selection is single (`AutoView.sel: Option<(usize, usize)>`), the drag does not deduplicate keys by tick, a click insertion does not snap to the frame grid, and Delete with a key selected goes past the widget into `KtApp::delete_action` (`src/apps/kt-app/src/app.rs`) and deletes the clip. Test t221 in `src/apps/kt-app/src/layout_tests.rs` checks the domain arithmetic and the stability of input in the table — it touches none of the four editing gestures.

### Which invariants it touches

- No. 11 (determinism) — the only root one, and even that tangentially: a key off the frame grid does not break the render (the engine samples the curve at the aligned material position), but makes the result depend on which of the four paths placed the key. None of 1, 4, 5 or 6 is touched: key editing does not touch the audio format, the cache, decoder ownership, or the clock.
- UI rules: 1 (hit with the mouse on the first try — key on key), 14 (the gesture must work REPEATEDLY — a drag after an overlap pulls someone else's key), 17 (paired methods: four insertions and three deletions behave differently), 20 ("nothing grows or switches off by itself" — Delete that erases the clip instead of the key is a violation in pure form).
- The project format is not touched at all: multiple selection is a state of the WINDOW, not of the model, just like `AutoView` as a whole.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages; the model barely changes |
| Where the main work is | kt-app (`automation.rs`: a set instead of `Option`, a box, batch moving, one insertion point with snapping, interception of Delete), kt-model (`ops`: a block of SELECTED keys next to `EnvelopeBlock`) |
| Project format | not touched: the selection is window state; deduplication of keys by tick may tighten `check_fx_auto`, and then normalization on read is needed, not a rejection |
| Tests that will have to be written | insertion "at the curve's value" under `ease` — fix exactly how it changes the neighbors (next to t190); a key placed by any of the four paths lies on the frame grid; dragging a key onto a key does not create a pair and does not intercept someone else's; Delete with a key selected does not touch the clip (harness next to t221); batch moving preserves relative distances |
| Depends on | F-AUT-001 (hold closes half of the "two keys" need), F-AUT-002 (the domain and the visibility of hidden keys) |
| Breaks compatibility | no |

## Verdict

We take it, but not as a "multiple selection feature", but as a repair of the single gestures — the trackers show exactly this order: a batch of keys is five times
rarer than single edits in both compared trackers. First one insertion point with snapping to the grid instead of four different ones, a ban on two keys
on one tick, Delete that in the widget deletes the key rather than the clip, and an honest label of what `ease` does to an insertion. Only then the box, batch
moving and copy-paste of selected keys — which the user asked for in R.33 item 6, and to which the answer so far is given only at the envelope level. Separately: the request for "two keys with one hotkey"
(Kdenlive #1368) as a gesture we do NOT take — it is a symptom of the missing hold, and it is cured in F-AUT-001; by adding it as a separate
button we would fix a workaround instead of the cause.

## Sources

Trackers (accessed 2026-09-08; dumps — `~/.cache/kinotrain/trackers/`,
not in the repository, see `docs/features/_trackers.md`):

- https://invent.kde.org/multimedia/kdenlive/-/work_items/123 — "Inserting
  points when we already have keyframes defined can lead to unexpected
  behaviors"; filed 31.03.2019 under a different heading (rotoscoping),
  closed 08.09.2024, 27 comments
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1368 — "Add hotkey
  to create 2 keyframes at once", 16.03.2022 → 03.10.2025, labels `Audio` +
  `Effects | Compositions` + `Enhancement`
- https://invent.kde.org/multimedia/kdenlive/-/work_items/828 — "[Feature
  Request] Selecting all/multiple keyframes", 08.11.2020
- https://invent.kde.org/multimedia/kdenlive/-/work_items/669 — copy-paste of
  single and multiple keys to the cursor position, 07.05.2020
- https://invent.kde.org/multimedia/kdenlive/-/work_items/794 — a summary list
  of complaints about the keyframe system, open since 09.09.2020
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1227 — hitting the
  right value of a volume key with the mouse, 21.10.2021
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1457 — the first click
  on an unselected clip counts as half a double-click and places a key,
  11.06.2022
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1104 — a copy of an
  effect onto a shorter clip leaves "ghost" keys beyond its boundaries, open since 14.06.2021
- https://github.com/mltframework/shotcut/issues/992 — Del deletes the track
  instead of the key, open since 27.12.2020
- https://github.com/mltframework/shotcut/issues/921 — a delete button of one
  parameter row erases another row's key, 11.10.2020
- https://github.com/mltframework/shotcut/issues/785 — "change how delete
  button works in keyframes screen", 23.10.2019
- https://bugs.kde.org/show_bug.cgi?id=517035 — cannot select a specific key to change
  interpolation while the Curve Editor is open, 03.03.2026
- https://github.com/olive-editor/olive/issues/825 — copy-paste of keys as a
  request, 17.04.2019

Foreign editors (accessed 2026-09-08):

- https://help.magix-hub.com/video/vegas/23/en/content/topics/7-edit/tool.htm
  — Envelope Edit Tool and the list of functions unavailable in normal mode
- https://helpx.adobe.com/premiere/desktop/add-video-effects/control-effects-and-transitions-using-keyframes/select-keyframes.html
  and .../copy-and-paste-keyframes.html — box, Shift/Ctrl-click, keys of different
  parameters, paste from the cursor position
- https://support.apple.com/guide/final-cut-pro/modify-groups-of-keyframes-veraca9b445e/mac
  — "All intermediate keyframes are selected. You can't select nonadjacent
  keyframes"; moving a group in time and in value
- https://davinciresolveclub.com/davinci-resolve-keyframes-curve-editor/ and
  https://jayaretv.com/news/new-keyframe-and-curves-editor-in-davinci-resolve-21/
  — box in the Curve Editor, key mode for a group of clips in version 21
- https://creativecow.net/forums/thread/i-forgot-how-to-adjust-keyframes-proportionately/
  — Alt+drag on the extreme key scales the timing of a group in After Effects

Ours (checked by grep across the tree, not from memory):

- `src/apps/kt-app/src/automation.rs` (`AutoView.sel`, `toolbar`, `graph`,
  `table`), `src/apps/kt-app/src/fx_editor.rs` (`draw_auto_for`, creating the
  curve), `src/apps/kt-app/src/camera_editor.rs`,
  `src/apps/kt-app/src/transition_editor.rs`,
  `src/apps/kt-app/src/app.rs` (`handle_keys`, `delete_action`),
  `src/libs/kt-model/src/project.rs` (`AutoKey`, `ParamCurve`,
  `curve_value`, `check_fx_auto`), `src/libs/kt-model/src/ops.rs`
  (`EnvelopeBlock`, `rebase`, `copy_envelope`, `paste_envelope`),
  `src/apps/kt-app/src/layout_tests.rs` (t221).
- `docs/user-feedback.md` R.33 items 5–6 (the single widget; "select and ctrl+c
  ctrl+v"), R.38 items 14/17/18; item 221;
  `src/apps/kt-app/CLAUDE.md` sections "Automation in the UI" and the checklist
  (rules 1, 14, 17, 20).

The measurement of the discrepancy when inserting a key "at the curve's value" was obtained by substituting into the
`curve_value` formula (a curve 0→1 over 90000 ticks, the key in the middle); the kt-app itself was not run live for this scenario, and foreign editors were not run — the rows of the table rely on vendor help and points 1–2 of the trust order of
`docs/features/_editors.md`.

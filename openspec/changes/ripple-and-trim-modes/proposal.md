# Proposal: ripple-and-trim-modes

> DESIGN, revision 2 (2026-11-24), base `229c694b`: implementation-ready. Q1 decided (A9); Q2 and Q3 at the end set two
> defaults and hold nothing. Code starts on the orchestrator's go. Tests: t3415–t3432, t4239–t4242.

## Why

The owner took candidate ★1 on 2026-11-01 (`docs/ktprompts/kkt-26.11.01-candidates.md`, registry
`kkt-26.11.01-index.md` P1; also `v26.10` N2 "other basic NLE features, check the feature DB" and U9
"a ripple variant waits for F-TL-001"): "Нет ripple — после каждого удалённого куска интервью я руками
тащу весь хвост. Хочу как в Vegas: auto-ripple кнопкой (трек / все треки) с красным индикатором,
ripple-delete, roll, slip, slide, «закрыть все дыры на треке», и ножницы по региону с клавиатуры,
которые сразу схлопывают дырку."

Today the model has no shift operation at all: `ops::punch_out_region` says "Without ripple" in its
doc comment, `C` (region-cut-key, F-TL-015) keeps the gap, paste always overwrites, there is no slip
or slide gesture, and the only roll is the roll of a transition joint. Notes F-TL-001 (ripple),
F-TL-002 (slip and slide) and F-TL-008 (close gaps) did the reconnaissance and took the Vegas form.

## What Changes

- **One shift operation in the model** (`ops::ripple_plan`): given an edit point, a signed length and a
  scope, it returns the batch that moves everything at or after the point — events by `MoveClip` of
  each link group once, markers in a safe order, named areas, morpher blocks, camera events, foreign
  placeholders and `timeline_end` — or a refusal naming the obstacle. The boundary is a VERTICAL line at
  the edit point on every track of the scope, not each track's own "next event" (Vegas's desync trap,
  F-TL-001 trick 2).
- **Post-edit ripple**: after a normal edit, one command shifts what follows by what the edit freed or took, on its track
  (F) or on all tracks (Ctrl+Shift+F). It joins the edit's undo step.
- **Auto-ripple mode**: a three-position toolbar button (off / affected tracks / all tracks; markers
  and areas follow per Q2) with a red outline on the selection and a red playhead while on; default
  off. In the mode, Delete, Cut, region cut (`C`), paste/insert and edge trims ripple.
- **Ripple versions that keep identities** (invariant 3): a rippled region cut keeps the left remainder's
  `ClipId` and makes the right one by `Split`, so the effects' state survives; the old
  `punch_out_region` re-creation is not used under ripple.
- **Locked tracks and other reels stay**: a locked track is named, not moved; a ripple acts in the reel it was made in.
- **Roll** at a plain cut (two butt-joined events), **slip** (the material moves under a fixed event;
  keys stay glued to the material by construction), **slide** (the event moves, its butt-joined
  neighbours trim) — each a drag with a modifier from the controls table, slide only as an explicit
  modifier. The end of the material is shown as an error stop (rule 16), from `MediaRef.duration`.
- **Close gaps** of the selected events (F-TL-008, the Vegas form: by selection; an empty selection on
  one track = the track). No "close every gap in the project" item.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `ui-timeline`: ripple, roll, slip, slide and close-gaps gestures, the auto-ripple mode and its
  indicator, the post-edit ripple command and its keys.
- `ui-reach`: the commands are reachable from the Edit menu, the toolbar, the event's right-click menu
  and the keys of each layout.
- `morpher`: a ripple that would detach a reference-mode layer (`Marks`/`Areas`) from its targets is
  refused with the reason, never applied silently.

## Impact

- kt-model: `ops::{ripple_plan, close_gaps}`, `RippleScope`; commands `MoveForeign`, `SetTimelineEnd`, `Slip`;
  `TimelineModel::amend_batch`; rippled variants of delete, region cut, paste, trims.
- kt-app: toolbar button, indicator, gestures (`timeline.rs`), `controls.rs`, menus, `user_store`, i18n ×5.
- kt-engine, kt-gpu, kt-fx: untouched (FX state survives by id). Project format, OTIO: nothing.
- Feature notes: F-TL-001, F-TL-002, F-TL-008.

## Questions to the owner

Decided (2026-11-20, `kkt-26.11.20.txt` line 14, A9): "Q8 ripple across the edit point: "п.1. но я не очень понимаю о чем речь
вообще - запиши на будущее чтобы я проверил"" — Q1 (a), the ripple refuses and highlights the spanning event; he checks it
live when it is built (`docs/owner-checks.md` C9). Read again on 2026-11-24: "п.1" is option (a) and nothing more; it decides
neither Q2 nor Q3, which no round has asked.

Your item whole (`docs/ktprompts/kkt-26.11.01-candidates.md` line 7, taken by you on 2026-11-01): "Нет ripple — после каждого
удалённого куска интервью я руками тащу весь хвост. Хочу как в Vegas: auto-ripple кнопкой (трек / все треки) с красным
индикатором, ripple-delete, roll, slip, slide, «закрыть все дыры на треке», и ножницы по региону с клавиатуры, которые сразу
схлопывают дырку."

**Q2. "Track" in "трек / все треки": do markers and named areas move with it?** Example: an interview on track 1, music on track 2
with 40 markers on its beats; you delete a 2 s piece of the interview at 1:00 with the mode "track".
- (a) **They stay; with "all tracks" they move** (*recommended*; Vegas's first and third modes): the 40 beat markers stay on the
  beats. A marker that named a place of the interview after 1:00 is now 2 s late. If a morpher layer takes its windows from
  markers or areas over the moved events, the ripple is refused, naming the layer (`MorphTargets::Marks/Areas`): with "all
  tracks" it passes.
- (b) They move always: the interview's markers stay right and no layer is ever left behind; the 40 beat markers leave the beats
  by 2 s, all after 1:00, with every cut.
- (c) Three modes as in Vegas (track; track + markers; all + markers): the button has four positions instead of three.

**Q3. With the mode on, does DRAGGING an event ripple too?** In Vegas it does. Example: five events of 10 s in a row; you drag the
second to the right by 3 s.
- (a) **No: a move stays a move** (*recommended*): the second overlaps the third by 3 s (a crossfade by itself, as today), the
  others stand; delete, cut, region cut, paste and edge trims ripple.
- (b) Yes, as in Vegas: the third, fourth and fifth move 3 s right too, a 3 s gap opens before the second; with "all tracks" the
  music after that point moves as well. To reorder events one first turns the mode off.

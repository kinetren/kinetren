# Proposal: ripple-and-trim-modes

> DESIGN ONLY (spec session, 2026-11-20). Code starts on the owner's word after his answers to the
> questions at the end. Tests reserved: t3415–t3439.

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
- **Post-edit ripple**: after any normal edit, one command shifts what follows by what the edit
  freed or took, in the scope the person chooses — affected tracks; affected tracks + markers and areas;
  all tracks + markers and areas (Vegas's closed list). It joins the edit's undo step.
- **Auto-ripple mode**: a three-position toolbar button (off / affected tracks / all tracks; markers
  and areas follow per Q2) with a red outline on the selection and a red playhead while on; default
  off. In the mode, Delete, Cut, region cut (`C`), paste/insert and edge trims ripple.
- **Ripple versions that keep identities** (invariant 3): a rippled region cut keeps the left remainder's
  `ClipId` and makes the right one by `Split`, so the effects' state survives; the old
  `punch_out_region` re-creation is not used under ripple.
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

- kt-model: `ops::ripple_plan`, `RippleScope`; commands `MoveForeign` and `SetTimelineEnd` (the two
  things a shift must move that no command addresses today); ripple variants of `delete_region`,
  region cut, `paste_block` (insert), trims; `close_gaps`; slip (`SetSourceIn`-style trim pair) and
  slide batches.
- kt-app: toolbar button (the loop button's three-state pattern), indicator, gestures in `timeline.rs`
  (`plan_move` gains the slip/slide/roll classes), commands in `controls.rs` ×4 layouts, menus, i18n ×5.
- kt-engine, kt-gpu, kt-fx: untouched (the edit is model-only; FX state survives by id).
- Project format: the mode lives in `UiState` (serde default, not written off) — no file version
  change; the edits produce ordinary ticks. OTIO: nothing new to lose.
- Feature notes: F-TL-001, F-TL-002, F-TL-008 point to this change.

## Questions to the owner

**Q1. An event standing ACROSS the edit point on another track.** Your rule (R.24 item 6, UI rule 20):
"NOTHING ANYWHERE SHOULD GROW BACK / BE CUT OFF." Your decision R.28: overlaps on one track are legal
and crossfade by themselves. Today: no ripple. When the scope is "all tracks" and a music bed on track 3
spans the cut point, closing the gap slides the later events of track 3 into the bed.
- (a) **Refuse the ripple, highlight the spanning event, say "split it or ripple affected tracks only"**
  (*recommended*: nothing changes that you did not touch).
- (b) Let them overlap — with layering on, a crossfade appears where you did not edit.
- (c) Leave spanning events in place and move only events wholly after the point on that track (the
  music bed does not move, but its later neighbours do — the gap stays there).

**Q2. Do markers and named areas follow the "affected tracks" scope?** Vegas: only its 2nd and 3rd
modes move them. Here a morpher layer in `Marks`/`Areas` mode takes its window from a marker or an area
by NAME: if the clips move and the marker does not, the layer lands on someone else's piece.
- (a) **Two scopes, not three: "affected tracks + markers/areas" and "all tracks + markers/areas"**;
  markers always follow (*recommended*: the layer can never detach).
- (b) Vegas's three; in "affected tracks" a ripple that would detach a layer is refused with the reason.

**Q3. Does auto-ripple also apply to MOVING an event?** Vegas includes moves: dragging an event out
closes the place it left and opens room where it lands.
- (a) **Not in the first stage** (*recommended*): delete, cut, region cut, paste/insert and trims
  ripple; a move stays a move. A dragged event that ripples both ends moves two tails at once.
- (b) Yes, as Vegas.

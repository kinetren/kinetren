---
id: F-TL-002
title: Slip and slide — trims that do not change length
status: candidate
priority: B
cost: S
category: timeline/trimming
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-app]
invariants: [4, 5, 8]
ui_rules: [1, 13, 16, 17, 20]
round: null
point: null
tests: []
depends_on: [F-AUT-002]
checked: 2026-09-07
---

# F-TL-002 — Slip and slide — trims that do not change length

## Essence

The edit is assembled, the durations are approved, but inside one event the frame is
wrong: the actor starts the line half a second later than needed. You cannot trim
it — the entire subsequent timing would move. **Slip**: the event stays in place, and the material under it is pulled through — the same three seconds, but taken from a different place in the file.
**Slide**: the material inside the event stays the same, while the event itself moves right
or left, and one neighbour gets longer by exactly the same amount as the other gets shorter. In both cases the total length of the film does not change by a single frame — and
this is the reason both gestures exist separately from the ordinary trim.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Slip Tool / Slide Tool + modifiers | The help states the pair as mirror images: "When you slip an event, your event maintains its place on the timeline, but the media file moves in the direction you drag" and "When you slide an event, the media file maintains its place on the timeline, but the event moves in the direction you drag". Two ways for each: a tool from the palette, or a modifier without switching tools — "Use the Slip tool or hold Alt while dragging an event", "Use the Slide tool or hold Ctrl+Alt while dragging an event". A separate item is slip-trim of the EDGE ("hold Alt while dragging the right or left edge"): the edge moves, while the material under it stays — that is, Alt inverts the meaning of the ordinary trim. The tools cycle with the D key (backwards — Shift+D); slip/slide have no highlighted letter |
| Adobe Premiere | 26.x | Slip Tool / Slide Tool | Definitions (a quote from an Adobe employee in the official community, verbatim from the help): "A slip edit shifts a clip's In and Out points forward or backward by the same number of frames in a single action. By dragging with the Slip tool, you can change a clip's starting and ending frames without changing its duration or affecting adjacent clips" and "A slide edit shifts a clip in time while trimming adjacent clips to compensate for the move". Separate palette tools, keys **Y** and **U** [NOT VERIFIED — `helpx.adobe.com` does not open from this environment; the page `perform-slip-edits.html` exists but is unavailable; tutorials and shortcut lists feature exactly Y/U]. Effect keyframes during a slip move WITH THE FRAMES (see "The trick behind it", item 4) |
| DaVinci Resolve | 21 | Trim Edit Mode (T) | There are NO separate tools: one mode, and the type of trim is chosen by where inside the clip's rectangle the cursor sits. "In Resolve, trim is a dynamic process. When you're in the Trim Edit mode, you can complete different trim edits depending on where you place your cursor on the clip": at the very edge — ripple, exactly at the junction — roll, "place the cursor on the frame thumbnails to perform a slip edit", and slide — "placing the cursor underneath the thumbnail previews and onto the area with the clip's filename". That is, a horizontal drag means four different things depending on Y [NOT VERIFIED against the primary source: `forum.blackmagicdesign.com` and searches on it return 403; the facts are taken from a secondary analysis] |
| Final Cut Pro | 12.3 | Trim tool (T), slide — Option-drag | Slip: "Performing a slip edit doesn't change a clip's position or duration in the timeline, but instead changes what portion of the clip's media appears in the timeline" — with the Trim tool (T, can be held temporarily) drag the body of the clip. Slide: the same tool, but **Option**-drag; "When you slide a clip, the adjacent clips get longer and shorter to accommodate the change in the clip's position". The only one of the five that explicitly SHOWS the exhaustion of the margin: "If either the start point or the end point turns red as you drag, you've reached the end of the available media for that side of the clip". Two-up display — the Option-U toggle |
| After Effects | 26.x | Pan Behind / Anchor Point tool (Y) | There are no tracks and no neighbours, so slide as a concept does not exist: layers lie in parallel, there is nothing to move. Slip exists and is done with the Pan Behind tool (Y) on the body of the layer in the timeline. The fundamental difference from the NLEs: in AE the keyframe domain is the time of the LAYER, so when dragging "All keyframes, etc should remain in place while you slide the clip around" (a forum formulation) — the material moves, the animation stays. This exact behaviour is what is requested from Premiere in a separate request |

Ripple (the fourth member of the family) is absent from our vocabulary as a concept and is analysed
separately; the only trace in the code is the comment "English: "no ripple"" in
`src/libs/kt-model/src/ops.rs`. About ripple as a MODEL switch on
a speed change — F-SPD-001.

### Four trims of one family

The table for the sake of which this note was worth writing. All four are the same
horizontal drag; they differ in WHAT is declared untouchable.

| Trim | What is dragged | Event boundaries | Material under the event | Neighbours | Film length |
|---|---|---|---|---|---|
| **ripple** | the edge | one edge moves | opens/hides | everything BEHIND the event moves | **changes** |
| **roll** | the junction of two events | both edges at the junction move | both open/hide | only the pair at the junction | the same |
| **slip** | the middle | **do not change** | **moves** | are not touched at all | the same |
| **slide** | the middle | **both move** | **do not change** | the two neighbours change length | the same |

Read by the "what is preserved" column: ripple preserves the content of all
events and sacrifices the timing of the film; roll preserves the timing and sacrifices
the content of two events; slip preserves the WHOLE layout and sacrifices only
the content of one event; slide preserves the content of its own event and
sacrifices the content of two neighbours. Hence the practical rule: slip is
the only trim that cannot break anything in someone else's work at all, and
slide is the only one that changes the length of events that were not dragged.

Of the four, we have **roll** (`Drag::TransitionBody`, a batch
`TrimEnd`+`TrimStart` — `src/apps/kt-app/src/timeline.rs`), and we have no ripple,
slip or slide. At the same time, "slip of a whole area" exists with us in an unexpected place —
see "How it fits Kinotrain".

## The trick behind it

**1. Slip has a precondition that no other trim has — and
five editors answer it differently, while the answer is visible in only one.** Slip requires that material REMAINS beyond the boundaries of the event (handles): you can pull only as far as the file is longer than
the cut piece. For an event taken from the first frame of the file to the last,
slip is physically impossible, and the gesture simply gives no result. For the
user this looks like a tool malfunction rather than a lack of margin: the only thread found is exactly about this ("The trick behind it" is confirmed here by the feedback verbatim — see the quote below). Then the differences begin. **Final Cut** paints the entry or exit point RED as soon as it hits the limit — "you've reached the end of the available media for that side of
the clip". **Premiere** shows nothing: the gesture just stops moving, and one has to figure it out on the forum. **VEGAS** does not hit a limit at all: the event has a Loop switch, and when it is pulled past the end of the file
the material starts to REPEAT — "Enabling the Loop switch on an event will
allow you to drag the right edge of the event out and have it repeat the
file over and over rather than inserting silence". That is, to one and the same
question "what is beyond the edge of the material" the industry gives three incompatible answers:
a red mark, silence without explanation, and endless repetition.

**2. Resolve solved the problem of "the gesture is already taken" with vertical stripes inside
the clip, not with separate tools.** In Trim Edit Mode one and the same
horizontal drag means four different operations depending on the
HEIGHT of the clip's rectangle at which the cursor sits: at the very edge — ripple,
at the junction — roll, on the thumbnail strip — slip, below the thumbnails, on the strip with
the file name — slide. This is the only one of the five implementations where one needs neither
to change the tool nor to hold a modifier — and the price is exactly what is visible
at once: the file-name strip stops being a label and becomes a grab zone,
and a miss of a few pixels upward gives the wrong trim. The other four
preferred an explicit change of mode: Premiere and FCP — a tool, VEGAS — both
a tool and a modifier (Alt / Ctrl+Alt), AE — the Pan Behind tool.
None of the five gave slip a bare drag over the middle: the middle is everywhere occupied by
moving the event.

**3. During a slip the editor shows TWO frames, and this is not decoration but a
consequence of slip having two points of interest.** In ripple one edge is edited — one frame is of interest; in roll and slip two are of interest (the outgoing and the incoming; in slip — the new first and the new last frame of the event itself), and it is impossible to set up a junction while seeing only one of them. Apple moved this into a separate switch (View → "Show 2–Up When Trimming", Option-U) and turns it on for
ripple, roll, slip, slide and in the precision editor. The reverse side surfaces
where the picture in the viewer must be seen ENTIRELY: a user who was matching
an XML against an offline reference via Difference Mode complains that the two- and
four-up display covers his composite, and receives the answer that it can be turned off with the mouse via Shift, and from the keyboard — not at all. That is, the two-up
display is mandatory for setting up a junction and gets in the way for any other
task, and only one of the five gave it a toggle.

**4. Slip is the litmus test for the keyframe domain of automation, and it is exactly here that
Premiere and After Effects diverge, although they are considered the same.** This is a
direct continuation of F-AUT-002 ("The keyframe domain — the source material or the time of the timeline"), where slip is named the litmus test; here — the side of slip. The
boundaries of the event during a slip are fixed, only the material moves, so the question
"where is the animation now" is asked in its pure form, without a mixture of shifts and trims. In
**Premiere** the keyframe is addressed to the material of the clip, and during a slip the animation moves
ALONG WITH THE FRAMES — so inconvenient that a request hangs in the ideas forum: "Allow clip to be slipped without moving its keyframes!" with a ready
proposal: an ordinary slip leaves the keys in place, and the move is done with
Ctrl/Cmd. An Adobe expert answering notes that this would coincide with
the AE behaviour under Pan Behind — and in **AE** it is indeed the reverse: "All
keyframes, etc should remain in place while you slide the clip around".
The reason is not convenience but the domain: in Premiere the key lives in the material, in AE —
in the time of the layer. A third detail from the same place and the most important for us: the standard workaround in the Adobe world is — **to put the effect on an adjustment layer and slip the clip
UNDER it**, because the layer has its own time, and its keys do not see the material beneath it.

## Pros

The items are consequences of the DESIGN of these two trims, not ratings.

- Slip cannot spoil someone else's work: it does not touch a single boundary on the
  timeline, so sync with other tracks, markers, the region,
  transitions and the entire subsequent timing survive it without a single change.
  This is the only trim that is safe to do in an approved edit.
- Slip is an operation on exactly one field (the entry point into the material), so
  it is reversible arithmetically: pulled by N and back by N — the state is bitwise
  the same as before. Neither ripple nor slide gives this: they involve the lengths
  of neighbours and clamps.
- Slide keeps the total length of the triple "neighbour — event — neighbour"
  constant, so it can be done inside an already edited scene
  without touching anything beyond its limits: it is local, unlike ripple.
- The slip+slide pair covers two orthogonal editing mistakes — "took the wrong
  piece" and "placed it at the wrong moment" — without mixing them. With an ordinary
  trim each of them is corrected with two gestures, and between the gestures the edit
  is in a state that is known to be wrong (the length has already shifted, but the compensation has not yet).
- Both have exactly one parameter — an offset in frames — so both get a keyboard
  form for free ("one frame left/right", "by ten"), and it has a precision that a mouse drag
  cannot give in principle. Apple provides it via comma/period, and this is the same
  gesture as for ripple and roll.

## Cons

- **Slip runs into the end of the file, and this state must be ABLE TO BE SHOWN.**
  The limit lives not in the edit but in the source, that is, outside the
  timeline: the user does not see the cause on screen and explains the refusal
  as a malfunction. One of the five (FCP) paints the edge, the others stay silent.
- **Slip has no natural points of attraction.** Magnet in an NLE catches
  event boundaries, markers, the cursor — all of these are TIMELINE coordinates, while slip
  moves the MATERIAL coordinate, which has no marks on the timeline at all.
  The only meaningful stops are the ends of the margin, that is, exactly the places where
  the gesture ends anyway.
- **Slide changes the length of events that the user did not drag.** This is its definition, not a side effect: "the adjacent clips get longer and
  shorter". It cannot be partially undone, and if a neighbour was trimmed for a reason (a line, a beat), slide silently eats that meaning.
- **Slide is undefined at the edges.** There is no neighbour on the left or right — there is nothing to compensate with, and the gesture either turns into an ordinary move or is forbidden; none of the five documents what exactly it does in
  this case.
- **The slip/slide pair is systematically confused by users.** The names
  are almost identical, while the results are opposite, and the question "this is how it works, right?" is asked even by those who described both operations correctly (see the quote).
  This is the price of both gestures being the same horizontal drag over the middle, differing only by a modifier or the selected tool.
- **Slip brings out a keyframe domain the user did not know about.**
  While there is no animation, slip is a harmless operation; as soon as curves appear on the event, the same gesture starts either to carry them away or to leave them, and
  which of the two behaviours is "correct" the industry has not decided (the request to
  Premiere has been open since 2023).

## What users say

> I've been doing some basic editing, and I need to use the slip tool.
> Unfortunately it only has worked SOME of the time.
> — MIWV, Adobe Support Community, 15.12.2018, https://community.adobe.com/t5/premiere-pro-discussions/slip-tool-not-working-well-sometimes/td-p/10242148

The reply in the same thread explains why this is not a bug, and at the same time formulates
the precondition of slip better than any help page:

> Sounds like you just hit the end of a clip ... slip has to have those
> extra frames to slip to.
> — R Neil Haugen, same place, 15.12.2018, https://community.adobe.com/t5/premiere-pro-discussions/slip-tool-not-working-well-sometimes/td-p/10242148

About the confusion of the two names — a question in which the user himself gives the
correct definition of both operations and is still unsure:

> Slip tool is the one that does not change the overall duration of the
> clip, but changes the beginning and end point of the clip. Slide tool on
> the other hand changes the starting point and the end point of the
> surrounding clip. Is this correct?
> — AloneInTheDark0104, Adobe Support Community, 30.04.2019, https://community.adobe.com/questions-729/slip-and-slide-tool-adobe-premiere-1351115

About the two-up display — a rare case where something mandatory for a trim
is asked to be turned OFF, and it turns out this cannot be done from the keyboard:

> If you use the mouse, you can hold the Shift key to disable the 4-up
> preview. Currently, it's not possible to disable the 4-up/2-up when using
> keyboard editing.
> — Rohit Gupta, Creative COW, 26.02.2016, https://creativecow.net/forums/thread/slip-slide-without-the-annoying-4-up-viewae/

About the fact that in AE slip is done with the "wrong" tool and the keys stay in place:

> For a slip edit, In the timeline shorten your clip so that it is shorter
> than the timeline, then use the Pan Behind tool (Y) on the faded out bar
> of your clip. All keyframes, etc should remain in place while you slide
> the clip around.
> — Daniel Waldron, Creative COW, 13.01.2015, https://creativecow.net/forums/thread/slip-slide-or-slide-edit-tool-in-ae/

## How it fits Kinotrain

### What already exists

**The model is ready in full; the GESTURE is not.** This is the main conclusion of the section, and it is verified
by the code, not by feeling.

- `ClipEvent` (`src/libs/kt-model/src/project.rs`) holds `start`,
  `duration` and `source_in` as three independent fields. **Slip is an edit of
  ONE field, `source_in`, with `start` and `duration` unchanged.** No other
  structure is needed for it.
- The conversion "timeline tick → material tick" exists in exactly one place —
  `clip_material_at`, and the paired `clip_timeline_at` next to it — and both
  take `source_in` as a parameter. Neither the render, nor the prefetch, nor the preview knows anything about
  `source_in` beyond this, so a change of the field reaches all consumers automatically
  and correctly (invariant No. 7: the composition code is one).
- The project format is NOT TOUCHED by slip: `source_in` goes into OTIO as the start of the
  `source_range` (`src/libs/kt-otio/src/convert.rs`), and automation keys since v3 are stored
  in material ticks — the way of writing does not depend on slip, and the file version is not raised.
- **Roll is already implemented with us and shows the form in which slide is done:**
  `Drag::TransitionBody` on release sends the batch
  `[TrimEnd{a}, TrimStart{b}]` (`src/apps/kt-app/src/timeline.rs`).
  Slide is the same batch plus moving the middle:
  `[TrimEnd{left neighbour}, MoveClip{event}, TrimStart{right neighbour}]`,
  **all three commands already exist** (`src/libs/kt-model/src/model.rs`).
  `TimelineModel::simulate` checks overlaps ONCE on the result of the batch, and `reconcile_transitions` is also called once — so
  the intermediate states of a slide are legitimate in exactly the same way as for roll.
- The arithmetic of neighbours for slide is already written: `room_left` / `room_right` /
  `obstacles` in `src/libs/kt-model/src/ops.rs` compute how far one can move without
  touching someone else's, taking into account the whole link group and foreign
  placeholders. The same are used by `restore_handles` — a precedent for an operation that
  hands out material to neighbours according to the rules.
- **Slip is already present with us — but as a LAYER, not as a gesture.** `MorphPatch`
  (`src/libs/kt-model/src/project.rs`) carries the fields `source_in_delta` and
  `absolute_source_in`, and `apply_to` writes `c.source_in` to the targets of the morpher
  in `Project::effective_clips()` with them. That is, the morpher can **non-destructively
  slip a whole named area** (time × tracks), cumulatively,
  through undo, with `BakeMorpher` to write the result in. Moreover: a partially covered target is CUT,
  therefore the morpher slips a PIECE of an event without an explicit cut — none of the five has this. What the morpher does not have is the
  gesture "drag with the mouse" and feedback: the value is typed as a number in
  `src/apps/kt-app/src/morpher_editor.rs`.
- The keyframe domain of automation with us is the MATERIAL tick (`AutoKey.t`,
  F-AUT-002), and the axis of the automation window is `range = (source_in, source_in +
  duration)` (`clip_ctx` in `src/apps/kt-app/src/automation.rs`). So
  on slip the window will move, while the keys stay on their frames: **the Premiere behaviour — the animation moves along with the picture — comes out by itself, without a single line of code.**
  Exactly what F-AUT-002 predicted in item 3 of "The trick behind it".
- A precedent for the "tool toggle in the toolbar" already exists: `sel_tool` in
  `src/apps/kt-app/src/toolbar.rs` ("as in vegas — toggle-button",
  R.33 item 7). For slip/slide this is a ready-made form that does not require inventing a palette of tools.

### What is missing

- **A command that writes only `source_in`.** Today `source_in` is changed only by
  `TrimStart`/`TrimEnd` (and then in conjunction with `start`/`duration`, with a
  mirrored branch for `reversed`) and `clip_slice`. Formally, slip is
  expressible today as a batch of three existing commands ("extend right, trim left, put back in place"), and `simulate` will let it pass —
  but this is a trick: it goes through the clamps of `MIN_CLIP_DURATION`, drags
  the whole link group along twice, and leaves one record in undo with an
  unclear meaning. The honest form is one command with a `delta`.
- **Not a single gesture exists.** All 14 variants of `Drag` in
  `src/apps/kt-app/src/timeline.rs` — `Clip(DragState{Move|TrimStart|
  TrimEnd})`, `Fade`, `Level`, `LaneRow`, `Scrub`, `Region`, `Rubber`,
  `RegionEdge`, `Marker`, `MarkerEdge`, `TransitionEdge`,
  `TransitionBody`, `Morph`, `Camera` — and not one of them moves the material.
- **The model does not know the length of the source.** `MediaRef` is `{id, path}` and
  nothing more, so there is NOTHING to check "ran into the end of the file" with in `check_on`: the only guard is `source_in >= 0`, that is, only the left edge.
  The UI knows the length, and asynchronously at that:
  `ThumbService::duration_or_probe` (`src/apps/kt-app/src/thumbs.rs`)
  returns `Option<Tick>` and queues a request to the worker if it does not know yet.
  Beyond the right edge of the material the engine behaves as at EOF — a negative cache and
  a frozen last frame / silence, without an error. That is, today we have the Premiere variant "silently", and even without stopping the gesture.
- There is no two-up display during a trim, and nothing close to it: the preview is one
  (`src/apps/kt-app/src/app.rs`), a "two frames" mode does not exist.

### What gets in the way

- **Room for the gesture.** The top strip of the event `top_strip_h` (35% of the row height, at most 12 px) is occupied by the fade handles and the level line at the direct
  request of the user (R.28 item 12, `docs/user-feedback.md`) — it must not be touched. The middle is occupied by `Move`, the edges by trims, the transition windows lie
  INSIDE the clip bodies and have priority over the clip zones. Free are:
  the **Alt modifier** (grep over `src/apps/kt-app/src/timeline.rs` — `modifiers.alt` does not occur even once; `Ctrl` is taken for disabling the magnet and
  building a selection, `Shift` — for the region on double-click) and the **middle mouse button** (not used in the application code). The Vegas layout
  Alt = slip, Ctrl+Alt = slide fits us without a single conflict; but
  in it Ctrl means "plus one more modifier", while with us Ctrl already means
  "magnet off", and the combination will have to be declared explicitly.
- **Magnet.** The question "does slip snap" has no obvious answer:
  `snap_targets` (`src/apps/kt-app/src/timeline.rs`) collects the boundaries of other
  clips, markers, the boundaries of the region, the playhead and zero — all of these are marks
  of the TIMELINE, while slip moves the material, which has no marks on the timeline. A straightforward
  "snap it like everything else" will give a gesture that jumps to other
  boundaries for no reason at all. There are exactly two meaningful stops — the ends
  of the margin — and under rule No. 16 of the checklist they must be an ERROR (highlighting the
  cause), not a silent stop; rule No. 4, meanwhile, requires
  that the useful behaviour be the default, and a modifier turn it off.
- **Slide against rule No. 20** ("nothing grows back or gets cut off by itself"). Slide by definition changes the length of two events that
  the user did not drag. This is not a prohibition — it is a requirement: slide must be an EXPLICITLY
  chosen mode or modifier, never the default of a drag, and must show which exact neighbours will move (rule No. 16 — the obstacle is highlighted).
- **Slide and transitions.** `reconcile_transitions`
  (`src/libs/kt-model/src/model.rs`) after the batch clamps `in_offset` and
  `out_offset` to the durations of the sides and KILLS a transition whose sides have
  stopped being butt-joined. A slide of an event that has a crossfade with its neighbour shortens the neighbour — and will silently trim the transition window. This is
  rule No. 20 again, and it is not a hypothesis: the same machinery already works for roll.
- **A live slip preview is the first gesture that, during a drag, asks for NEW material.** With slip the cursor usually sits inside the event, so
  at each step of the drag the displayed frame is different, that is, decoding is required.
  Invariant No. 8 ("scroll/zoom does not generate decode requests") is formally about scrolling, but its spirit is the same, and invariant No. 5 requires that the requests go
  through the priority queue of MediaWorker with generation-cancellation, not directly. The warmth of the strips must at the same time be computed by the cache
  (invariant No. 4), not by separate bookkeeping: a slip shifts the window of the needed
  material as a whole, and the old coverage instantly stops being correct.
- **The morpher layer does not save you from "slip carries the keys away".** The standard
  Adobe workaround — put the effect on an adjustment layer and slip the clip under it — does not work for us: `MorphPatch` has `video_fx`/`audio_fx`, but NO
  `fx_auto` field (F-MPH-008), that is, there is nothing to animate the layer's effects with. If "slip without moving the keys" is ever needed, there will be no
  workaround through the morpher — one will need either a modifier on slip itself, or
  curves on the layer.

### Which invariants it touches

The root `CLAUDE.md`: **No. 4** (after a slip the warmth is counted by the cache; do not create separate bookkeeping), **No. 5** (frames for the live preview — only through
the MediaWorker queue with generation-cancellation), **No. 8** (in spirit: the gesture must not turn into a stream of decode requests). Invariants No. 7 and No. 11 are NOT touched, and this
matters: slip does not add a second path of composition and does not change a single reference — `GOLDEN_AUDIO`/`GOLDEN_VIDEO` remain untouched, because
only the model field changes.

The checklist of `src/apps/kt-app/CLAUDE.md`: **No. 1** (hitting a 12-pixel strip with the mouse — if we go the Resolve way with strips by height, this is the main
risk), **No. 13** (do not create a new strip for the sake of a gesture — put it on an already
occupied place with a modifier), **No. 16** (a push against the end of the material is shown
as an ERROR with the cause highlighted, not with silence and not with a faded hint), **No. 17** (paired ways: the gesture and the item of the event's context menu behave
identically; the clip menu already exists in `timeline.rs` and contains Copy/Paste/
Delete/Split/FX/Reverse — slip/slide fit there), **No. 20** (slide touches the neighbours — only by an explicit mode).

## Estimate

| What | Answer |
|---|---|
| Volume | **S** — one stage. Slip: one command `SlipClip{id, delta}` in `kt-model` (it edits only `source_in`; for `reversed` the sign is the same — the material window `[source_in, source_in+duration)` is one and the same for both cases) + a variant of `Drag` + menu items + i18n strings. Slide: ZERO new commands, a batch of three existing ones + a variant of `Drag`. The two-up preview is NOT part of the estimate — it is a separate feature and a separate note |
| Where the main work is | `src/apps/kt-app/src/timeline.rs` (the gesture, zones, showing the stop), `src/libs/kt-model/src/{model.rs,ops.rs}` (the command + assembling the slide batch) |
| Project format | **not touched**: `source_in` is already serialized, automation keys since v3 lie in material ticks, the file version is not raised |
| Tests that will have to be written | model: slip does not change `start`/`duration` and does not touch `fx_auto` (and reversibility: +N and −N give the original state bitwise); slip of a `reversed` event moves the material window in the same direction; slide preserves the sum of the lengths of the triple and does not kill the transition at the junction; slip cannot take `source_in` below zero; a harness test of the gesture in `src/apps/kt-app/src/layout_tests.rs` (hitting with the mouse, showing the stop as an error, pairing of the gesture and the menu item — rule No. 17); i18n tests t100/t125/t128 for the new strings |
| Depends on | F-AUT-002 (the keyframe domain; slip is its litmus test, the behaviour comes out by itself) |
| Breaks compatibility | no |

## Verdict

**We take it, slip first.** A rare case where a feature from the list "what we do not have"
on inspection rests not on architecture but on one missing gesture:
the model is laid out exactly as slip requires (`source_in` is a separate
field, the "tick → frame" conversion is in one place), the format does not move, the references do not
need regeneration, and slide is expressible at all as a batch of commands already written, following the
existing roll as a pattern. Exactly two things in this feature are expensive, and neither
is about slip as such: **showing the push against the end of the material** (the model does not know the length of the
source, the UI knows it asynchronously — and under rule No. 16 the stop must be a
visible error, otherwise we will repeat the complaint from the quoted thread "worked
sometimes") and **the two-up preview**, without which setting up a junction will
still have to be done by eye — the latter is moved into a separate note and is not in the
estimate. We take slide second and only as an explicit mode: it is the only one of the four trims that
changes what the user did not drag, and that is a direct clash with rule No. 20.

## Sources

- https://help.magix-hub.com/video/vegas/23/en/content/topics/7-edit/slipping_and_sliding_events.htm — the definitions of slip/slide and the modifiers Alt / Ctrl+Alt verbatim (accessed 2026-09-07). The help was read on branch 23; for 2026.0 no separate branch was found on the host (`/2026/` → 404), so the table row is marked with the version from `_editors.md`, and the check is by the nearest available branch
- https://help.magix-hub.com/video/vegas/23/en/content/topics/7-edit/tool.htm — the Vegas tool palette (Slip Tool, Slide Tool), switching with D / Shift+D (2026-09-07)
- http://vegaspro.helpmax.net/en/using-vegas-software/editing-events-on-the-timeline/applying-switches-to-events/ — the Loop switch of an event: beyond the end of the file the material repeats rather than giving silence. A SECONDARY source (a mirror of the Vegas help); the primary source for this item did not open (2026-09-07)
- https://community.adobe.com/questions-729/slip-and-slide-tool-adobe-premiere-1351115 — the official definitions of slip edit and slide edit (an Adobe employee's answer, marked as correct), plus a user's question about the confusion (2026-09-07)
- https://community.adobe.com/t5/premiere-pro-discussions/slip-tool-not-working-well-sometimes/td-p/10242148 — "slip has to have those extra frames to slip to" (2026-09-07)
- https://community.adobe.com/feature-requests-730/allow-clip-to-be-slipped-without-moving-its-keyframes-1329704 — the request "slip without moving the keyframes" and the reply about the match with the AE Pan Behind behaviour; there also the workaround through an adjustment layer (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/make-slip-edits-ver1632d8e4/mac — the definition of slip edit and the red indication of the end of the available material (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/make-slide-edits-ver1632caff/mac — slide via Option-drag, the neighbours change length (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/show-a-two-up-display-in-the-viewer-ver3363b235/mac — the two-up display, Option-U, the list of trims for which it appears (2026-09-07)
- https://www.premiumbeat.com/blog/dynamic-trim-tool-resolve/ — Trim Edit Mode and the choice of the type of trim by the position of the cursor along the height of the clip. A SECONDARY source: the primary source (the Resolve Reference Manual) and `forum.blackmagicdesign.com` return 403 from this environment, so the Resolve row is marked `[NOT VERIFIED]` (2026-09-07)
- https://creativecow.net/forums/thread/slip-slide-without-the-annoying-4-up-viewae/ — a complaint about the forced 2-up/4-up and Shift as the only switch (2026-09-07)
- https://creativecow.net/forums/thread/slip-slide-or-slide-edit-tool-in-ae/ — Pan Behind (Y) as slip in AE, the keys stay in place (2026-09-07)
- The Premiere keys **Y** (Slip) and **U** (Slide) are NOT confirmed by the primary source: `helpx.adobe.com` does not open from this environment
  (the page `premiere/desktop/edit-projects/trim-clips/perform-slip-edits.html`
  exists and is visible in the results, but is unavailable), `defkey` returns 403.
  The confirmations found are only the titles of training videos, which by the rules of `_editors.md` is a confirmation, not a source
- Kinotrain code: `src/libs/kt-model/src/{model.rs,ops.rs,project.rs}`,
  `src/libs/kt-otio/src/convert.rs`, `src/apps/kt-app/src/{timeline.rs,
  toolbar.rs,thumbs.rs,automation.rs}`, `docs/user-feedback.md` (R.26, R.28
  item 12), item 246

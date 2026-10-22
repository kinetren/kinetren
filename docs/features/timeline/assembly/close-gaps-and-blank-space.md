---
id: F-TL-008
title: Closing gaps on a track — one, the whole track, the whole project
status: candidate
priority: B
cost: M
category: timeline/assembly
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: no
crates: [kt-model, kt-app]
invariants: [3]
ui_rules: [16, 17, 20]
round: null
point: null
tests: []
depends_on: [F-TL-001]
trackers: [kde-bugzilla, kde-invent, otio, pitivi, shotcut, olive]
checked: 2026-09-08
---

# F-TL-008 — Closing gaps on a track — one, the whole track, the whole project

## Essence

The editor cut two dozen pieces out of an interview - holes remained on the track.
One command, and the events slide left flush, without dragging each one.
Under this word live THREE different operations: close the hole under the cursor, close
all the holes on one track, close all the holes in the whole project. They differ not
in convenience, but in how much of someone else's edit will be shifted: the first moves the neighbour,
the second - the tail of the track, the third - the whole film. A thing separate from ripple (F-TL-001):
there it is a mode that changes ANY edit, here it is a one-off command without state.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Close gaps (Edit menu and right-click on the track) | by SELECTION: "Select two or more events on a track… The second and all following selected events are moved towards the first one so they will border at each order closing the gaps between them". The scope is set by the user's hand, not by a mode. The variant "the whole track" is a degenerate case of the same command: "If no event is selected when performing this command all gaps on the track will be closed". Separately there is a path through ripple: a double click on empty space makes a time selection over the hole, then `Delete` with Auto Ripple on, or `Delete` + `Ctrl+Shift+F` |
| Adobe Premiere | 26 | `Sequence > Close Gap`; right-click on the hole → Ripple Delete (`Shift+Delete`) | the ONLY one of the five whose standard command works on the whole sequence at once, across all video and audio tracks; it can be narrowed only with in/out markers. It snags on any obstacle on a neighbouring track and says nothing about it; it does not remove the hole between zero and the first clip |
| DaVinci Resolve | 21 | Delete Gaps (right-click on a clip or on empty space); selecting the hole + `Delete` | the hole is a selectable object: click on empty space, `Delete` ripples, `Backspace` does not (two keys for two behaviours). `Delete Gaps` closes all the holes of a track [NOT VERIFIED against the primary source: the official manual 21 did not respond to a direct request, the text was checked against two secondary analyses] |
| Final Cut Pro | 12.3 | no "close gaps" command; instead a gap clip | a hole does not arise by oversight: in the primary storyline it does not exist as a state. For a hole to appear it has to be CREATED - `Edit > Replace with Gap` (`Shift+Delete`) places a gap clip, or the Position tool temporarily switches off the magnet and leaves the void. A hole is removed like an ordinary clip: select it and `Delete`. There is no mass command, because there is nothing to remove en masse |
| After Effects | 26 | no | layers are parallel and have no neighbours on a track, "further right on the track" does not exist - there is no concept of a hole at all |

**Outside the core.** The live request with which the note began, and all the counting
material - come from Kdenlive: it is the only programme in the sample where all three
operations are separated into different menu items, and therefore it is visible what each one costs.
The "trick behind it" below is taken from there.

| Programme | Version | What it is called | How it works |
|---|---|---|---|
| Kdenlive | 26.08 | Remove Space · Remove Space In All Tracks · Remove All Spaces After Cursor | all three operations exist at the same time and are addressed by the cursor position, not by selection: the first - the active track, the second - all tracks at the same point, the third - the tail whole. For the first a separate `Track Select` dialog used to ask which track |

## The trick behind it

**1. "Close the hole" is ripple whose scope is cut down to one track, and therefore it breaks sync
QUIETER than a real ripple.** The ripple mode has an indicator, a button, a red cursor and a whole chapter of help about
how dangerous it is to forget it switched on (F-TL-001). "Close the holes" has nothing: a one-off
command, it has no state, there is nothing to forget - and precisely for this reason its consequences
nobody shows. Mechanically it is the same thing: everything to the right of the point moves left,
and everything addressed by absolute time that did not get into the shift stays
in place. The public tracker names the victims one by one: in Shotcut these are the keyframes of the
filters of the TRACK ("Removing empty space moves all the clips forward.
However, it does not move the keyframes", #641), in Kdenlive - the playhead, which
stays at the same point of the SCREEN and ends up in the middle of the clip that has moved in (#408142),
in Premiere - a neighbouring track with an adjustment layer, and the explanation in the thread literally
names the cause: "Because you are performing a ripple delete (on multiple empty
spaces)". That is, the user chose not ripple but "tidying up", and got ripple.

**2. Only the one for whom the hole is an OBJECT can tell an intentional hole from an accidental one.**
A pause in speech, a black frame between scenes, a place for a shot not yet filmed - these are part of the edit,
and on the track they look exactly like rubbish. An absence cannot be selected, named or locked, therefore
automation that works by absence is bound to destroy intentional pauses: it has no sign by which
to tell them apart. Everyone who solved this task solved it the same way - by making the void an entity: in Final Cut it is the gap clip,
which has to be created by the explicit command `Replace with Gap`; in OTIO `Gap` is a full-fledged `Composable` next to `Clip`
(our `src/libs/kt-otio/src/schema.rs`); Pitivi has been discussing the same thing for fifteen years and proposes a "fake clip" that "could be selected and deleted".
The reverse side is visible there too: in Final Cut, where the hole is an object, there is NO command "close
all holes" AT ALL, while in Kdenlive, where the hole is an absence, this command has seven years of regressions in a row.

**3. One and the same word in three places of one product's interface means three different scopes.** Kdenlive #557, verbatim: "'Remove Space' means
something different depend on the place you see it. For example, in the context
menu of the Timeline 'Remove Space' means to remove the space on all tracks.
However, in the main menu Timeline -> Space section 'Remove Space' means to
remove space on just the active track". The hotkey behaves like the main menu. This is not a typo in a label: an item that differs from its neighbour only by the
scope of the shift cannot be named so that the name carries the scope - and in the feature list on the site the difference
is not visible under any reading, because there is one line there, "Remove Space".

## Pros

- **The Vegas "by selection" form sets the scope by hand, not by a mode.**
  "Close these three joints and leave the rest alone" is expressed directly by the selection gesture, which the user already has at their fingertips;
  a separate third operation "the whole project" is not needed at all with such a design.
- **A command without state is cheaper than a mode by a whole class of errors.** It cannot
  be forgotten switched on, it needs no indicator, it does not have to be switched off before the next
  edit - that is, the whole price of the ripple mode (F-TL-001, "Cons") is not paid here.
- **A hole-object makes an intentional pause expressible.** Since the void is created by an explicit
  command, it can be kept, selected, stretched and not lost during tidying up -
  and an "accidental one-frame hole" ceases to exist as a class.
- **Resolve separated the rippling and non-rippling deletion onto different keys**
  (`Delete` versus `Backspace`), that is, the consequence is chosen by a finger at the moment of the gesture, not by a setting found a month ago.
- **Premiere can narrow the scope with in/out markers** - the same idea as in Vegas,
  by another organ: the command is global, but its horizon is set by the user.

## Cons

- **This is ripple without a warning.** Everything said in F-TL-001 about "the edit is visible, but the consequence is outside the screen" applies here in full, but without the indicator,
  the mode and the red cursor that are there, at least.
- **The operation does not distinguish an intentional hole from an accidental one** - see "The trick behind it" 2.
  A pause in speech, left for breathing, collapses together with the rubbish, and it can be restored only by undo, that is, by cancelling the whole useful part of the tidying.
- **It cannot be applied to a selection - only to a whole track or to the whole project - and therefore it is almost always redone by hand.** The request kde
  #371279 asks to remove even the track-selection dialog, that is, the only organ by which the scope was narrowed at all. Vegas is an exception here, not the rule.
- **A snag on an obstacle on another track is silent.** In Premiere the explanation lives on the forum, not in the programme: "If you cannot close the gap it means something
  on the track is blocking" - the user sees only that the command "did not work".
- **The cursor after the operation stays where it was on the screen**, not where the joint now is: the playhead ends up in the middle of the moved-in clip at an arbitrary distance from its start (kde #408142, open since 2019).
- **Everything addressed by absolute time and not included in the shift comes apart:**
  the keys of track filters (Shotcut #641), markers and guides (the mirror request kde #500773 asks the same of the reverse operation - "'Insert Space' should
  have option to move guides too"), clips of neighbouring tracks.
- **The "whole project" variant is almost always a user error.** It breaks sync between tracks in exactly the same way as ripple with the scope "all
  tracks", but is chosen not as an editing mode but as "tidying up", that is, without any expectation of the consequences.

## What users say

> You can currently remove spaces in a track or you can remove the same space in
> the all tracks but it would be a fantastic feature to be able to remove all
> spaces of a single track all at once.
> — Michael Tunnell, 14.11.2019, https://bugs.kde.org/show_bug.cgi?id=414142

The request was closed `RESOLVED FIXED` on 17.03.2026 - seven years later - with the reply
of the maintainer "Feature exists: Remove Space, Remove Space in All Tracks,
Remove All Spaces After Cursor". Between these two dates lies all the counting
material of the note: the command was there the whole time, broke and was repaired.

> 'Remove Space' means something different depend on the place you see it.
> — kdenlive #557, 08.02.2020, https://invent.kde.org/multimedia/kdenlive/-/issues/557

> When removing empty space in timeline the play head stays in the same place on
> screen. This effectively moves the play head forward on the next clip by some
> arbitrary amount.
> — Grzegorz Nowakowski, 31.05.2019, https://bugs.kde.org/show_bug.cgi?id=408142

> Removing empty space moves all the clips forward. However, it does not move
> the keyframes.
> — arximboldi, 26.09.2018, https://github.com/mltframework/shotcut/issues/641

> I dont know why the Adjustment Layer is moving when closing gaps below.
> — TK - JV, 21.02.2025, https://community.adobe.com/t5/premiere-pro-discussions/mind-the-gap/m-p/11330161

> Because you are performing a ripple delete (on multiple empty spaces).
> — Ann Bens (reply to the same), 21.02.2025, https://community.adobe.com/t5/premiere-pro-discussions/mind-the-gap/m-p/11330161

> If you cannot close the gap it means something on the track is blocking.
> — Ann Bens, 01.08.2020, https://community.adobe.com/t5/premiere-pro-discussions/mind-the-gap/m-p/11330161

> When hovering a gap between two clips with the mouse cursor, we can reveal a
> fake clip. The fake clip could be selected and deleted, which would shift the
> following clips to the left.
> — Pitivi #2548, https://gitlab.gnome.org/GNOME/pitivi/-/issues/2548

SIGNAL: closing the void - 8 tasks in kde-invent, `(remove|delete|close)\w*[ -](all )?(the )?(blank|empty|gap|space)`; share 0.4% against 0.1% in kde-bugzilla

SIGNAL: a hole as an object - 4 tasks in otio, `\bgaps?\b`; share 0.6% against 0.2% in pitivi

Read the first line like this: of the 22 tasks of the filter, 17 are KDE ones, and in the
developer tracker (`kde-invent`) they are four times more numerous by share than in the user
bugzilla, although the bugzilla is three times larger. The user asks for this command once, but it is fixed constantly: by the filter
`(remove|delete|clos)\w*[ -](space|gap)\w*.*(not|doesn|broken|wrong|strange|fail)`
in `kde-invent` there are 7 tasks against 3 in `kde-bugzilla` - these are "Remove Space doesn't
work anymore", "broken", "not working (sometimes)", "strange behaviour".

Read the second line like this: `otio` is a LIBRARY without a single interface element,
and yet its share of tasks about `gap` is three times higher than that of Pitivi with a full-fledged
timeline. The hole is expensive even where nobody draws it: as soon as the void
has become an object, it has to be created, split, glued and saved - that is,
the price of the decision "the hole is an entity" is visible on the bare format, without UI.

## How it fits Kinotrain

### What already exists

- **Both operations that PRODUCE a void are written, and not one closing operation exists.**
  `ops::delete_region` (`src/libs/kt-model/src/ops.rs`) removes all clips
  crossing the region, together with their link partners; `ops::punch_out_region`
  cuts the region's interior out of every crossing clip, and the remainders get
  new ids and lose their link group. Both leave exactly the gap the region was in place,
  and both state this honestly: the doc comment of `punch_out_region` ends with the words "Without ripple".
  They are called from `src/apps/kt-app/src/app.rs` — `delete_action` and `punch_out_action`.
- **Half of the future operation already exists and is already covered by a test.**
  `timeline::plan_move_block` (`src/apps/kt-app/src/timeline.rs`) computes the COMMON shift
  of the selected clip batch, clamps it against the first obstacle on its track, and returns `blocker` — the coordinates, kind and track number of the culprit.
  Test t204 (`src/apps/kt-app/src/layout_tests.rs`) pins down that the mutual distances inside the block do not change and that the whole move is ONE undo step.
  "Closing the gap" is exactly `plan_move_block` with a negative delta; only a way to put "everything to the right on this track" into the block is missing.
- **Free space is computed with the same geometry.** The private `room_left`,
  `room_right` and `obstacles` in `ops.rs` give the distance to the nearest neighbour of
  its own `(kind, lane)`, taking into account foreign placeholders `Project.foreign`.
- **A region for us is PURE TIME, without tracks.** `pub struct Region { start,
  end }` (`src/libs/kt-model/src/project.rs`), and `intersects_clip` looks
  only at time. Therefore our `Del` with a region already means "in all tracks",
  and an operation "on this track only" does not exist for us in any form.
- **A named area is the opposite — time × TRACKS.** `MarkedArea` carries
  `lanes: Vec<(LaneKind, u32)>` and `covers_lane()` (an empty list = all), and the keys
  are semantic, not row numbers. So a ready address type for a "track-wise"
  scope of action already exists for us — it is simply not in `Region`.
- **Multi-selection, the frame and operations over the whole selection exist** (t202/t203,
  t205), so the Vegas-style form "close the gaps between the selected events" fits onto the existing
  organs rather than requiring new ones.

### What is missing

1. **The concept of a "gap".** It does not exist in the model: a track is a sparse list of clips with
   `start`/`duration`, and a `Gap` is synthesized only when writing to OTIO
   (`src/libs/kt-otio/src/convert.rs`) and is read back as emptiness. The same
   is noted in F-TL-001, "What is missing" item 2 — but there it got in the way of the
   "double-click on the gap" gesture, while here it directly deprives us of the sign "intentional".
2. **Selection of "everything to the right on THIS track".** `plan_move_block` takes a ready
   list of `ClipId`, and nobody collects it: `Region` does not know tracks, and the frame
   selection works by pixels.
3. **Track addressing for the region** — either `Region` learns to carry `lanes` like `MarkedArea`,
   or the operation takes the track from the selection.

### What gets in the way

- **In the project's default mode the block does not brake against a neighbour at all.**
  `plan_move_block` begins with `if p.overlap_layers && !foreign { continue; }`:
  with layer overlap enabled (and it is enabled by default, at the owner's direct request,
  item 9 of R.24), the ONLY obstacles remaining are foreign placeholders.
  So "driving left until the stop" for us will not stop at anything, and closing a gap
  must compute the target delta itself — from the neighbour's geometry, not by moving until a collision. Otherwise the command will overlay clips on each other and silently
  generate a new auto-crossfade.
- **Shifting ONE side of a joint silently kills the transition.**
  `TimelineModel::reconcile_transitions` (`src/libs/kt-model/src/model.rs`)
  drops a transition if after the edit `a.start + a.duration != b.start`.
  Closing a gap moves the right neighbour and does not move the left one — so every crossfade
  at the BOUNDARY of the operation dies. Next, `ops::restore_handles` comes into play: it returns the material
  that was trimmed when the crossfade was created and clamps the gift by the freed space
  (`gap = b.start − (a.start + a.duration)`), while the operation is precisely the one that consumes this gap.
  The result depends on the order of phases inside the batch — `apply_batch_restoring`
  computes the return from the `preview` of the main batch, so the shift delta must
  be computed AFTER the return of the handles. The very same trap is described for F-TL-001, and
  this is an argument for doing both features with one mechanism, not two.
- **Named areas and mark ranges will not move, and the morpher addresses its targets through them.**
  `MorpherEvent::ref_windows` (`src/libs/kt-model/src/project.rs`)
  in the `MorphTargets::Marks`/`Areas` modes resolves `p.named_range(n)` and
  `p.area(n)` by NAME and intersects the window with the body of the clip. We closed the gaps on one
  track: the clips moved, the name is alive, the window stands at the old ticks — the patch lands on
  someone else's material or on nothing. In Premiere, in the same scenario, the adjustment layer
  DID move and frightened the user with it (quote TK - JV); for us, on the contrary, it will NOT move and will stay silent — which is worse, because there is nothing to notice.
- **The morpher in `Below` mode holds onto the order of rows, and in time onto its own block.** `MorpherEvent::row_in`/`Project::rows()` give the spatial part,
  and the operation does not touch it; but the morpher's temporal window is the `start`/`duration`
  of its own block, and closing gaps under it will desync the patch with its targets
  in time, leaving the spatial binding intact. A desync of exactly half is the worst kind:
  the layer keeps working, but not where it should.
- **A mark has no id — its address is its `t`.** `Command::MoveMarker { from, to }`
  is rejected with `MarkerExists` if someone already stands at `to` (`check_on` in
  `src/libs/kt-model/src/model.rs`). A mass shift to the left must go strictly in ascending order, and a mark that arrives at the tick of an edit point to the left of the point (that one does not move at all) will bring down the whole atomic batch.
- **Foreign placeholders are a hard wall.** `Project::validate()` forbids the overlap `Clip`×`Foreign` always (`ProjectError::ForeignOverlap`), and there are no commands in the model that edit `Project.foreign` and `Project.timeline_end`. Closing gaps on a track with foreign constructions must either honestly refuse, or move them too — a second mechanism does not yet exist.
- **Rule 20 ("nothing grows back and nothing gets cut off by itself").** The wall is the same as for
  F-TL-001, but it is passed more easily here and for a different reason: this is ONE explicit command,
  not a mode, so it does not "fix up the neighbours on the user's behalf" but carries out what was given. The Vegas-style form — by selection — does not contradict the rule at all; the form "across the whole project", chosen by one menu item without a selection, contradicts it literally, and it is precisely this form that the trackers show as the source of the pain.
- **Rule 16 (impossibility = an ERROR).** An abort on an obstacle must be a message naming where to look, not a silent under-move: in Premiere this knowledge lives on a forum. The material is already there — `BlockPlan.blocker` carries the `((start, end), kind, lane)` of the culprit, and the reference for the presentation is t172, "obstacle as an error".
- **Rule 17 (one behaviour for all inputs).** Kdenlive #557 is the reference failure of
  this rule: the main-menu item, the context-menu item and the hotkey do different things under one name. For us this is caught the same way as "M versus Marker here": one body of the operation per gesture, menu and key.

### Which invariants it touches

**Invariant 3** ("editing the model does not destroy FX state") — the only one, and for the same reason as with ripple: the chain diff goes BY the clip ID, so the operation
must move clips with `MoveClip`, not recreate them. The neighbourhood here is dangerous:
gaps most often remain after `punch_out_region`, which precisely RECREATES
the remainders with new ids — so "cut out and close up" in one gesture will break the reverb
tails, if the shift is not done as a separate phase of the batch.

The other eleven are not touched: the operation lives entirely in the model, produces no decode (No. 8), does not concern the audio path (No. 1, No. 2), the composition code (No. 7), or determinism (No. 11). The project format does not change — everything the operation moves is already serialized in ticks.

Related notes: **F-TL-001** — ripple as a MODE that changes any edit; this note is about a SPECIFIC operation without state and with an explicit scope, and what they share is only the shift mechanism, which must be written once. **F-TL-002** (slip/slide)
— operations that do not change track length, that is, that neither produce nor close gaps.

ADDRESS: src/libs/kt-model/project.rs — the class "an edit moved clips out of a named area's window, and the morpher did not notice" is possible for us, and nothing catches it: `Project::validate()` checks overlaps, `ForeignOverlap` and boundaries, but NONE of the checks looks at whether the windows of `MorpherEvent::ref_windows` remain non-empty after an edit; today the class does not manifest only because none of our commands moves clips under a stationary area — and closing gaps will do exactly that.

## Estimate

| What | Answer |
|---|---|
| Size | **M** — 2 stages. (1) The operation in the model: the set "what is to the right on this track", computing the delta from geometry (not "until the stop": with `overlap_layers` there is no stop), the shift phase AFTER `restore_handles`, reference entities — marks, named areas, morphers. (2) UI: a gesture on empty space, menu items in three forms with honest area names, an obstacle error modelled on t172 |
| Where the main work is | `kt-model` (`ops.rs` — the operation and the block's set; `project.rs` — track addressing of the area), `kt-app` (`timeline.rs` — the gesture and highlighting of the gap under the cursor, `app.rs` — actions and the menu). The shift mechanism is shared with F-TL-001 and is written once |
| Project format | not touched: the operation stores nothing, and everything it moves is already in ticks |
| Tests to be written | closing one gap moves exactly the right neighbour and everything after it on this track, neighbouring tracks stay; one undo step; an A/V pair does not drift apart by double (`MoveClip` moves the link group as a whole); a transition at the boundary of the operation SURVIVED, and `reconcile_transitions` did not touch it; the operation + `restore_handles` in one batch give the expected length; `MorpherEvent::ref_windows` after the operation gives the same windows as before it; a mass shift of marks does not fail on `MarkerExists`; with `overlap_layers` the closing does not generate a new crossfade; a track with `foreign` — an intelligible refusal, not a silent drift; a harness test for the gap highlight and for the obstacle error text; an `i18n` test for the new menu items (t100/t171) |
| Depends on | F-TL-001 — does not block, but the shift mechanism must be shared; otherwise we get two pieces of code that handle transitions and marks differently |
| Breaks compatibility | no |

## Verdict

We take it — priority B: this is not "without which the product is not an editor" (unlike ripple itself), but frequent tidying that today has to be done by dragging.
The cost is M and almost entirely shared with F-TL-001: `plan_move_block` and t204 already give
a shift of a batch by one delta in one undo step, and `blocker` already names the culprit
for the message under rule 16.

We take the **Vegas form: by selection**, and "the whole track" is a degenerate case of
the same command with an empty selection. The reason is not taste: for us `Region` is pure
time without tracks, and any "by cursor" form would have to invent track addressing from scratch, whereas the selection for us is already multi-clip and already carries
the track of each participant (t202–t205).

The third operation — **"close all gaps across the whole project" — we do not take**: it is
ripple with the scope "all tracks", chosen by one menu item without a single sign
that the whole film is about to move. It directly contradicts rule 20, and
it is precisely this that produces the bulk of the countable material of the note. If such an operation is ever needed, its place is the third state of the ripple button from F-TL-001,
next to the indicator, not a separate "tidy up" item.

A separate decision the user will have to make: **what counts as an intentional gap.** None of the five core editors can tell them apart, and the only known way — to make the void an object (gap clip in Final Cut,
`Gap` in OTIO). We have no object, and creating one for the sake of a single operation is expensive;
the cheap substitute is to hold an intentional pause as a named area, which the operation
must respect. This is exactly the same entity that the morpher hangs on, and
therefore the decision will have to be made regardless — see `ADDRESS:` above.

## Sources

- https://bugs.kde.org/show_bug.cgi?id=414142 — the original live request ("Feature: Please add a GUI option to remove all blank spaces in a track"), 14.11.2019, closed as FIXED 17.03.2026 with the reply "Feature exists…" (access date 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=408142 — "Improve workflow when removing empty space in timeline": the playhead stays at the SCREEN point; open since 2019 (2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/issues/557 — "'Remove Space' Label & Functionality Inconsistencies": one word, three areas (2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=371279 — a request to remove the `Track Select` dialog from "Remove Space" (2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=500773 — the mirror operation: ""Insert Space" should have option to move guides too" (2026-09-08)
- https://github.com/mltframework/shotcut/issues/641 — "Removing empty space does not move keyframes applied to track filters" (2026-09-08)
- https://gitlab.gnome.org/GNOME/pitivi/-/issues/2548 — "Allow controlling gaps through fake clips": a gap as an object (2026-09-08)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/closing%20gaps.htm — VEGAS help: "Closing gaps" works by selection, an empty selection = the whole track (2026-09-08)
- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/post_edit_ripple.htm — VEGAS 2026: `Ctrl+Shift+F` as an alternative way of closing a gap via ripple (2026-09-08)
- https://community.adobe.com/t5/premiere-pro-discussions/mind-the-gap/m-p/11330161 — Premiere: the adjustment layer moves when gaps are closed, the explanation "this is ripple delete", an obstacle on the track (2020, 2025) (2026-09-08)
- https://www.provideocoalition.com/tool-tip-tuesday-for-adobe-premiere-pro-selectively-close-gaps-in-the-timeline/ — `Sequence > Close Gap` across the whole sequence, narrowing with in/out marks (2026-09-08)
- https://larryjordan.com/articles/fcpx-delete-primary-clips/ and https://larryjordan.com/articles/fcpx-position-tool/ — Final Cut: the primary storyline does not allow gaps, a gap is created explicitly (`Shift+Delete` → gap clip, or the Position tool) (2026-09-08)
- https://support.apple.com/guide/final-cut-pro/verc5d34129/mac — Final Cut, `Edit > Replace with Gap`: the selection is replaced by a gap clip, the project duration is preserved (2026-09-08)
- https://beginnersapproach.com/davinci-resolve-delete-gaps-clips/ — Resolve: `Delete Gaps` in the context menu, `Delete` ripples, `Backspace` does not (2026-09-08)

**Honest about the gaps.** The Resolve row relies on two secondary write-ups:
the official manual 21 did not come back on a direct request, and this is marked explicitly in the
table. The After Effects row is based on how the product is built (layers are parallel), not on
the help page: `helpx.adobe.com` still does not open from this environment (the same gap is recorded in F-TL-001).

**Honest about the export.** The figures were taken on 08.09.2026, when the cache
`~/.cache/kinotrain/trackers/` held eight dumps out of the twelve in the registry
`_trackers.md`: `kde-bugzilla` (6346), `kde-invent` (2217), `pitivi` (2672),
`shotcut` (1368), `olive` (1853), `otio` (680), `mlt` (601) — complete per the registry,
plus `losslesscut` and `flowblade`, which were being downloaded at that moment. Both `SIGNAL:` lines
were read ONLY from the complete dumps (`kde-invent`, `kde-bugzilla`, `otio`, `pitivi`)
and are reproducible with `tracker-rank.py --query` with the filters given;
`openshot`, `blender-vse` and `ffmpeg-trac` are not counted at all.

---
id: F-TL-001
title: Ripple — an edit that shifts everything to the right
status: candidate
priority: A
cost: L
category: timeline/trimming
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-app]
invariants: [3]
ui_rules: [4, 13, 16, 17, 19, 20]
round: null
point: null
tests: []
depends_on: []
checked: 2026-09-07
---

# F-TL-001 — Ripple — an edit that shifts everything to the right

## Essence

The editor cuts a piece out of an interview, and the tail of the film slides left by itself, with no gap and no manual dragging of forty events. Ripple is not a separate "delete and shift" command but a PROPERTY OF AN EDIT: while it is in effect, ANY length change (trimming an edge, deleting, inserting, changing speed) moves everything to the right of the edit point. The form of this property differs: a global mode (Vegas), a lock on a track (Premiere), a pair "normal operation / ripple operation" (Resolve), or the only editing model (FCP), but the meaning is the same.
Hence three faces of one thing: ripple delete (cut, and the gap closes), ripple trim (pull an edge, and the tail moves), ripple insert (insert, and the tail makes room; this is also the difference between an "insert" edit and an "overwrite" edit). The price of the mode is that it changes the edit OUTSIDE the visible screen: you edit minute 03:00, and everything up to 40:00 moves.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Post-Edit Ripple / Auto Ripple | TWO ways, and this is Vegas's main difference from all the others. (1) Retroactively: make a normal edit, then press `F`; "Press F after an edit to ripple the affected tracks, press Ctrl+F to also ripple markers, keyframes, and envelopes. To ripple everything after an edit, press Ctrl+Shift+F". The same menu is `Edit \| Post Edit Ripple`. (2) As a mode: the Auto Ripple button on the toolbar (`Ctrl+L`) with a drop-down arrow for choosing the type. There are exactly three types, a closed list: "Affected Tracks" (events, keyframes and envelopes on the tracks where you edited), "Affected Tracks, Bus Tracks, Markers, and Regions", "All Tracks, Markers, and Regions". The triggers are listed by name: "Adjusting an event's length by trimming, slip-trimming, sliding, or time stretching", "Moving events", "Cutting events", "Pasting events", "Deleting events". In Auto Ripple mode the outline of the selected event and the cursor turn RED, and this can be turned off with a checkbox in `Options \| Preferences \| Editing` |
| Adobe Premiere | 26 | Ripple Delete (`Shift+Delete`), Ripple Edit Tool (`B`), Sync Lock | The scope is set not by a global mode but by a **Sync Lock** switch in the header of EACH track (not to be confused with Track Lock: that forbids editing altogether, while this only controls participation in the shift). A track with sync lock on moves along with an edit on any other track; with it off, the track stays put. In addition, the `B` tool, separate from the mode, performs a ripple trim in one gesture with no mode at all. A known complaint: an edit on a track with sync lock OFF still moves the tracks with sync lock ON (see "What users say") |
| DaVinci Resolve | 21 | Ripple Delete, Trim Edit Mode (`T`), `Timeline > Ripple Timeline Markers` | Ripple is a property of the OPERATION, not of a mode: Ripple Delete has its own shortcut (`Shift+Delete`/`Shift+Backspace`) next to the regular Delete, and ripple trim is in Trim Edit Mode. Markers are moved out into their OWN toggle in the Timeline menu: "When checked, all markers to the right of a clip being ripple edited, trimmed, or ripple deleted will ripple to the left along with the rest of the Timeline". That is, by default markers do NOT move with a ripple, and this is a separate decision of the user, not part of choosing the scope. [NOT VERIFIED: the manual page for version 18.6 returned 404 when accessed directly; the text is taken from the index results] |
| Final Cut Pro | 12.3 | no mode, magnetic timeline | Ripple is not a switchable behavior but the model itself: "When you insert, trim, or move clips in the primary storyline, surrounding clips automatically move out of the way or snap together to avoid unwanted gaps and collisions". Sync is maintained not by a lock but by attachment: "When you move a clip in the primary storyline, all of its connected clips move with it, keeping your edit in sync". `Delete` is a ripple ("any clips to the right of the selection ripple to close the resulting gap"), and the non-ripple variant is on `Shift+Delete` (Replace with Gap). Insert: `W`, "all clips after the insertion point are moved later (or rippled) in the timeline to make room"; `D` (Overwrite): "No clip items are rippled forward, so the duration of your project remains the same" |
| After Effects | 26 | Ripple Insert Edit (button in the Footage panel) | Ripple exists by name and almost not at all in meaning: layers lie in parallel and have no neighbors on their track, so there is nothing to shift. The Overlay Edit / Ripple Insert Edit buttons live in the Footage panel and cut ONE layer at the CTI position, pushing its right half away. No scope, no sync lock, and no markers in the shift [NOT VERIFIED against the primary source: `helpx.adobe.com` does not open from this environment] |

## The trick behind it

**1. In Vegas, ripple is also a command AFTER THE FACT, not only a mode.**
The help describes two sections in a row: "Applying a post-edit ripple manually" and
"Applying a post-edit ripples automatically". In the first, the editor makes a normal
edit (the timeline behaves as usual, nothing moves), and then presses `F`,
`Ctrl+F` or `Ctrl+Shift+F`, and the shift happens as a separate deliberate action
with an EXPLICITLY chosen scope. This is not a convenience but a different model of
responsibility: the edit and its consequence are separated in time and in the key press.
The 2008 forum argument is exactly about this: "Because YOU can then control when the ripple occurs.
Suppose you do two edits. On one of them you want the events to all move…
Suppose on the other one you just cut out something and everything else didn't
need to move" (Edward Troxel). The marketing feature list has a single
word "ripple", and from it one cannot guess that there is a safe retroactive twin, which
half of the veterans use.

**2. The shift boundary is computed HORIZONTALLY, from the edge of the edited event, not
vertically along the line of the cut.** Hence the whole disaster "ripple broke my sync",
and it happens not when the mode is off but when the edit is not perfectly aligned.
A forum formulation (Bill Binder, about Vegas, but the mechanism is general): "I've found
this behavior to be caused by the clips that you THINK are all completely lined
up in sync exactly together ARE ACTUALLY NOT… If you had clips that weren't
aligned, and you slid the clip that started later, the clips that started
earlier would not ripple". A single frame of mismatch between video and audio, and on ripple
the video moves while the audio stays: not a "glitch" but a direct consequence of the fact that
"to the right" is computed separately on each track. This is exactly what Sync Lock in Avid/Premiere was designed for: it turns such an edit not into a silent
drift but into a REFUSAL ("When I try with sync locks on, it obviously gives me the system error
sound").

**3. The same key in the two most-used editors does opposite things.**
In Premiere `Delete` leaves a gap, `Shift+Delete` is a ripple. In Final Cut it is exactly the reverse: `Delete` is a ripple, and `Shift+Delete`
(Replace with Gap) is the way to KEEP the gap. Muscle memory when switching
between them breaks sync silently and in both directions, and there is no setting that
would align them. The third variant is Resolve, where both operations are split into
different menu items, and markers are moved into a third, independent toggle `Timeline >
Ripple Timeline Markers`.

## Pros

- **Retroactive ripple (Vegas `F`) does not need a mode at all.** The edit does
  exactly what the user reached for; the shift is a separate command with its own
  key and its own choice of scope. The class of defects "forgot that the mode
  was on" does not exist with this approach by construction, not by attentiveness.
- **Scope is a closed list of three values, not a set of checkboxes.**
  "Affected Tracks" / "...Bus Tracks, Markers, and Regions" / "All Tracks,
  Markers, and Regions" are three states of one button. The user cannot assemble
  a non-working combination, and their choice fits entirely in a tooltip.
- **Sync Lock is a property of the TRACK, not of the operation.** So "the music bed
  never moves" is configured once for the whole job, instead of being remembered
  before every deletion.
- **FCP's magnetic timeline removes a whole class of editing mistakes: the gap in the picture.**
  Since ripple is a model and not a mode, "an empty frame accidentally left between
  cuts" simply does not arise; to get a gap you have to ASK for it with an
  explicit Replace with Gap command.
- **Ripple insert is three-point editing in its pure form.** You set in/out on the
  source and an insertion point on the timeline; the length is taken from the
  source, and the tail makes room by itself (`W` in FCP). Without ripple the third
  point is meaningless: the length would have to be set as a fourth point.

## Cons

- **The mode changes what is not visible.** The edit is made zoomed in on the current second, while
  the consequence lands at the fortieth minute. This is not "inconvenient" but structural: the feedback is physically
  off-screen, and the mistake is noticed several edits later, when
  undo no longer helps.
- **"Scope" means three different definitions across four editors, and the habit cannot be carried over.**
  Vegas: the ripple type is global (three states).
  Premiere: a per-track lock. Resolve: a property of the specific operation plus a separate
  toggle for markers. FCP: not configurable at all. The same gesture gives
  four different results.
- **The per-track lock is asymmetric, and this is where people get caught.** In Premiere, an edit on a
  track with sync lock OFF still moves the tracks with it ON: to cut a piece
  of music, one has to switch off the lock on ALL the other tracks, that is, temporarily
  dismantle the protection of the whole project for the sake of a single edit.
- **Markers do not move by default, and this is only half right.** A marker "insert title here" is
  tied to a FRAME of the source and must move; a marker "end of act one" is
  tied to running time and must not move. None of the five editors can
  tell them apart, so the decision is placed in a
  global toggle that is the wrong one each time.
- **FCP's price for having no mode is the loss of explicit layout.** A gap
  left on purpose (a place for a plan not yet shot) in the magnetic model
  is expressed by a special placeholder object, and all the orchestration of "what is attached to what"
  moves into connected clip relations instead of a simple "where it lies".

## What users say

> I always leave "Auto Ripple" turned OFF and, if I want to ripple, use the
> Post Edit Ripple options. I've found things are safer that way.
> — Edward Troxel, 13.12.2008, https://www.dvinfo.net/forum/what-happens-vegas/139549-bugs-auto-ripple.html

> The biggest issue is, when you're working on along timeline and you're zoomed
> in to the current section, you can't see what's happening further down. If
> it's accidentally turned on, you can mess things up pretty bad.
> — Mike Kujbida, 14.12.2008, https://www.dvinfo.net/forum/what-happens-vegas/139549-bugs-auto-ripple.html

> i've made those mistakes in the past, moved something then carried on editing
> the 5 mins later it's like WTF has happened
> — Paul Kellett, 14.12.2008, https://www.dvinfo.net/forum/what-happens-vegas/139549-bugs-auto-ripple.html

> When I try with sync locks on, it obviously gives me the system error sound.
> When I turn sync locks off and perform this edit, my audio for the rest of
> the show loses sync.
> — Bryan Roberts (about Avid Media Composer), 25.04.2013, https://creativecow.net/forums/thread/is-it-possible-to-perform-this-ripple-edit-while-n/

> Try using Ripple Delete to cut out a middle section of music on a track where
> Sync Lock is disabled. You'll find it's impossible without disabling Sync
> Lock on all other tracks - which defeats the purpose of the feature.
> — TheEditWolf, 05.05.2025, https://community.adobe.com/t5/premiere-pro-discussions/sync-lock-not-respected-during-ripple-delete/m-p/15305272

> I've found this behavior to be caused by the clips that you THINK are all
> completely lined up in sync exactly together ARE ACTUALLY NOT.
> — Bill Binder, 18.12.2008, https://www.dvinfo.net/forum/what-happens-vegas/139549-bugs-auto-ripple.html

Separately, it is valuable that the vendor ITSELF considers the mode dangerous: in the Vegas 2026
help, the red cursor is explained with the words "These indicators will help you avoid
accidentally making ripple edits when you've forgotten that you are in Auto
Ripple mode" — that is, the indicator exists not for looks but against a
specific, known class of errors that the manufacturer is aware of.

## How it fits Kinotrain

### What already exists

- **A direct marking of "this is missing".** `src/libs/kt-model/src/ops.rs`,
  doc comment of `punch_out_region`: "the right remainder keeps the source alignment. **No ripple**".
  This is the only mention of the word in the whole
  code base, checked by grep over `src/`.
- **Both halves of the operation are already written separately.** `ops::delete_region`
  (delete the clips intersecting the region together with their link partners) and
  `ops::punch_out_region` (cut out the interior of the region from each intersecting
  clip, the remainders get new ids) are exactly "ripple delete without the shift".
  Both are called from `src/apps/kt-app/src/app.rs` (`delete_action`,
  `punch_out_action`) via `apply_batch_restoring`.
- **Atomicity of a gesture is free.** `TimelineModel::apply_batch`
  (`src/libs/kt-model/src/model.rs`) validates the WHOLE series on a clone and
  writes one undo snapshot; `apply_coalesced` keeps a live drag as one history step by its gesture token. Shifting a hundred events is one batch, not
  a hundred undo steps.
- **Almost everything that must move is already addressable by commands.** Clips —
  `MoveClip`; morphers — `SetMorpher` (a full replacement of the event); cameras —
  `SetCamera`; named areas — `SetAreaRange`; markers — `MoveMarker`.
  No new command types are needed for them.
- **Transitions move by themselves.** `TransitionEvent` does not store absolute time:
  only `a`/`b` (clip ids) and `in_offset`/`out_offset` from the cut
  (`src/libs/kt-model/src/project.rs`). Shift both neighbours by the same
  delta, and the transition moves with them, and `reconcile_transitions` does not touch it.
- **A precedent for a three-position toolbar button already exists.**
  `src/apps/kt-app/src/toolbar.rs`: loop is drawn in three states
  (off / on with a region / ARMED without a region: a muted icon plus its own
  tooltip), and `sel_tool` is Vegas's "toggle-button" of the selection mode. Vegas's three-valued ripple button fits this same code.
- **There is room for a mode flag, and the format does not break.** `UiState`
  (`src/libs/kt-model/src/project.rs`) lives OUTSIDE undo, is written to the file, and
  is almost entirely marked `skip_serializing_if` — a new ripple mode flag and
  the per-track "sync lock" fit there, and old files stay byte-stable.
  The ordering of lines from `UiState` into the model was moved precisely because it affects
  the PICTURE; the ripple mode does not affect the picture — so it stays in
  `UiState`, next to `region` and `loop_enabled`.

### What is missing

1. **The shift operation itself.** Neither `MoveClipsAfter`, nor a ripple flag on
   `delete_region`/`punch_out_region`/`paste_block` exists. Insertion today is
   always "over" (overwrite): `ops::paste_block` puts the block at the position and
   nothing makes room.
2. **The concept of a "gap" as an object.** The model has no gaps at all: a track is a
   sparse list of clips with `start`/`duration`, and `Gap` exists only
   in the OTIO wire format (`src/libs/kt-otio/src/convert.rs` synthesizes it when writing).
   For shifting this is good (nothing to recalculate), but it kills the most
   common gesture of competitors — "double-click on the gap → Delete". For us, one has to place a region at that
   spot, and `Del` with a region already does this.
3. **Commands for two things that MUST move but are not addressable.**
   `Project.foreign` (placeholders for foreign OTIO constructs) and
   `Project.timeline_end` (the editorial length of a foreign file) are not edited by ANY
   command — checked by grep over `src/libs/kt-model/src/model.rs`.
4. **A distinction between "to the right on the track" and "to the right in the project".** Today there is no such
   concept in the code in any form; `ops::obstacles`/`room_right`
   count neighbours strictly within a single `(kind, lane)`.

### What gets in the way

- **Our overlaps are LEGAL, while ripple assumes butt-joined clips.** `overlap_layers`
  is enabled by default at the user's direct request (item 9 of R.24), and
  `plan_move` returns `MovePlan::Free` with it on — a drag cuts NOTHING.
  Inside a block that has moved entirely by one delta, overlaps are preserved
  (mutual distances do not change) — only the BOUNDARY breaks: by closing
  the gap, ripple can slide the first surviving clip into the clip on the left. With overlap-layering on, this will silently
  create a NEW automatic crossfade ("the one that started later is on top"), that is, sound and picture
  change where the user touched nothing; with it off, `validate()` returns
  `ClipOverlap`, and the atomic batch rolls back the WHOLE gesture. Both branches are the user's decision,
  not ours: one violates rule 20, the other makes ripple
  non-functional exactly where it is needed.
- **Ripple and the return of crossfade "handles" compete for the same space.**
  `ops::restore_handles` returns the material that was cut off when the crossfade was created, when the transition dies, and CLAMPS
  the gift by the freed space: `gap = b.start − (a.start + a.duration)`, and when `gap == 0` zero is returned.
  Ripple closes exactly this gap — so deleting IN RIPPLE MODE will
  keep LESS material than the same deletion without it. The order is also a trap: `apply_batch_restoring` computes `restore_handles` from the `preview`
  of the main batch, so the shift delta must be computed AFTER the handles are returned, otherwise the clips will
  drift apart by exactly the length of the gift.
- **The morpher layer addresses targets BY NAME, and this is the least obvious consequence.**
  In the `MorphTargets::Marks`/`Areas` modes the action window is taken not from the morpher block
  but from the named range-marker or area:
  `MorpherEvent::ref_windows` (`src/libs/kt-model/src/project.rs`) resolves
  `p.named_range(n)` and `p.area(n)` and intersects them with the clip body. So a
  ripple that shifted the clips but did not shift the markers and areas will BREAK the layer
  from its targets: the name remains alive, the window moves away, and the patch lands on someone else's piece or on nothing at all.
  The same dependency exists for the `Below` mode, only along a different axis — it looks at the ORDER of ROWS (`rows()`), which ripple does not touch, so the spatial part stays intact, while the temporal one (`timed`, the block window `start`/`duration`) must move together with the targets. Conclusion: for us
  "ripple markers and regions" is not a third state of the button to choose from, as in
  Vegas, but a CONDITION OF CORRECTNESS of the morpher layer. This must be decided explicitly: either
  the reference windows always move, or the layer must notice the drift and say
  so (rule 16).
- **Markers are identified by tick, and a mass shift trips over this.**
  `Command::MoveMarker { from, to }` is rejected with `MarkerExists` if someone already stands at `to` (`check_on` in `src/libs/kt-model/src/model.rs`).
  When shifting a batch to the left, the traversal order must be ascending (otherwise a marker will run into a neighbour that has not moved yet), and a marker that arrives at the tick of a marker to the LEFT of the edit point — which does not move at all — will bring down the whole
  atomic batch. The model does not allow merging two markers into one: a marker has no
  id; its address is its `t`.
- **Foreign placeholders are a hard obstacle that layering does not remove.** In `Project::validate()`, overlap of `Clip`×`Foreign` is always forbidden
  (`ProjectError::ForeignOverlap`), because the raw material is written back
  to OTIO, where the track is strictly sequential. A ripple along a track with foreign
  constructs will either be rejected entirely, or must move them too — and there is no
  command for that (see "What is missing", item 3).
- **Rule 20 against ripple is so by definition, not in a detail.**
  "Nothing grows back or gets cut off by itself… smart logic that 'fixes up' the neighbours for the user is a defect, not a convenience"
  (`src/apps/kt-app/CLAUDE.md`), the primary source is item 6 of
  `docs/user-feedback.md`: "let it be exactly as much as it was and must remain… where the 'paired' event was, there it stays and goes nowhere and
  is not cut off. NOTHING ANYWHERE SHOULD GROW BACK / BE CUT OFF". Ripple changes
  exactly what the user did not touch, so the only form in which it does not violate the rule is an EXPLICIT intent. And here the reconnaissance gives
  a ready answer rather than a guess: Vegas has two such forms, and both are explicit —
  the retroactive command `F`/`Ctrl+F`/`Ctrl+Shift+F` (the edit has done its job,
  the shift is a separate press) and a mode with a MANDATORY visible indicator
  (a red event outline and a red cursor). The first form does not contradict rule 20 at all: it does not "fix up", it carries out a command given.
  The second passes only together with the indicator, and under rule 13 the indicator
  must sit on the ALREADY occupied place — the cursor and the selection outline, as in
  Vegas, rather than a new strip. The default under rule 4 is: the mode is off;
  "a useful mode is on by default" does not apply here, because the usefulness of the mode depends on the task, and the price of a mistake is the whole tail of the film.
- **Showing a consequence outside the view requires rule 16.** It already obliges us to say "WHERE to look if the cause is outside the view"
  (the reference is `t172` "an obstacle as an error"). Ripple is the mirror case:
  the cause is visible, but the CONSEQUENCE is outside the view. Mike Kujbida's complaint
  ("you can't see what's happening further down") is exactly this, and our checklist
  already requires us to answer it.
- **In our system the sync of an A/V pair is kept not by a lock but by a link group — and `MoveClip`
  moves the whole group.** `Command::MoveClip` in `mutate_on` shifts the partners
  "by the same offset", therefore a naive ripple that issues `MoveClip` for both the video and the audio of the pair would shift it TWICE. On the other hand, the Sync Lock mechanism is
  almost unnecessary for us: the pair moves together by construction, and "the music bed
  does not move" is expressed by the scope of action (our analogue of "Affected Tracks").

### Which invariants it touches

**Invariant 3** ("a model edit does not destroy FX state") is the only one of the twelve that is touched, and touched for real: the diff of chains goes BY CLIP ID,
so ripple MUST MOVE the clips (`MoveClip`), not recreate them with
new ids. Today's `punch_out_region` does exactly that recreation (the remainders get
fresh ids and lose the link group) — the ripple version of delete would inherit this and
break the reverb tails of the remainders, unless the shift is made a separate phase
of the batch. The other eleven are not touched: ripple lives entirely in the model,
does not produce decode (No. 8), changes neither the audio path (No. 1, No. 2), nor the composition
code (No. 7), nor determinism (No. 11).

UI rules: **20** (the central one — the analysis above), **4** (the default is off),
**13** (the indicator on an already occupied place: the cursor and the selection outline), **16**
(a consequence outside the view is shown as a warning, not as a grey hover),
**17** (`Del` with a region, the menu item and the edge gesture must take the mode into account
in the same way — the reference failure "M versus Marker here" is exactly about this), **19** (the badge about the enabled mode must say where to turn it off).

The project format does not change: the mode flag and the scope live in `UiState`
under `skip_serializing_if`, and everything that ripple moves is already serialized
as ticks.

Related notes: **F-SPD-001** (there ripple is analysed as a switch of the MODEL
of a speed change — "Ripple Edit, Shifting Trailing Clips" in Premiere, "Ripple
Sequence" in Resolve; the speed operation MUST use the same mode
as this note, otherwise there will be two ripples with different rules),
**F-AUT-002** (the keyframe domain is the material tick, therefore the shift of an event does not
touch the curves — we do not have this pain by construction), **F-AUT-003** (track-level envelopes: we do not have them, while Vegas moves exactly them in the "…Bus Tracks…" modes).

## Estimate

| What | Answer |
|---|---|
| Volume | **L** — a whole round. Stages: (1) the shift operation in the model with a scope of action and the order of phases relative to `restore_handles`; (2) ripple versions of delete/trim/insert on top of the existing `delete_region`/`punch_out_region`/`trim_group_*`/`paste_block`; (3) reference entities — markers, named areas, morphers, cameras — plus commands for `foreign` and `timeline_end`; (4) UI: a three-position button, the retroactive command, the red indicator, the hints |
| Where the main work is | `kt-model` (`ops.rs` — the operation and the scope; `model.rs` — commands for `foreign`/`timeline_end`), `kt-app` (`toolbar.rs` — the button after the loop example; `timeline.rs` — the indicator and gestures; `app.rs` — `delete_action`/`punch_out_action`). `kt-engine`/`kt-gpu`/`kt-fx` are not touched at all |
| Project format | not touched: the mode and the scope are in `UiState` under `skip_serializing_if`, old files stay byte-stable |
| Tests that will have to be written | the shift moves exactly what is to the right, and in one undo step; the A/V pair does not move twice (`MoveClip` moves the group); a transition at the junction of two moved neighbours SURVIVED, and `reconcile_transitions` did not fire; markers/areas/morphers/cameras moved by the same delta and `MorpherEvent::ref_windows` gives the same windows as before the edit; a shift of a batch of markers does not fail on `MarkerExists`; ripple along a track with `foreign` is a refusal with a clear error (rule 16), not a silent drift; ripple and `restore_handles` in one batch give the expected length; with `overlap_layers` the shift boundary does not create a crossfade by itself; a harness test in `layout_tests.rs` for the three-position button and the red indicator; an `i18n` test for the new menu strings |
| Depends on | nothing blocks it. F-SPD-001 (speed ↔ length) must use THIS mode and not create its own |
| Breaks compatibility | no |

## Verdict

We take it — priority A: without ripple, editing a dialogue or an interview turns into
manual dragging of the tail after every edit, and this is the only operation from "what we do not have" that the editor
works around dozens of times an hour.

We take the **Vegas form and both halves at once**: first the retroactive
command (`F`/`Ctrl+F`/`Ctrl+Shift+F` after a normal edit), then
the auto mode with a three-position button and a mandatory red indicator.
The retroactive half is cheaper, useful in itself, and does NOT ENTER
into conflict with rule 20 at all — which means it can go out as a separate stage
before the mode.

We make the scope a closed list of three values, as in Vegas, and not
per-track locks, as in Premiere: for us the sync of the A/V pair is held by the link group,
`MoveClip` moves it as a whole, and there is no point in repeating someone else's asymmetric protection
(the TheEditWolf complaint, 2025). A separate decision that the user will have to
make: for us the third state ("…markers and regions") is
not a matter of taste, but a condition of correctness of the morpher layer in the `Marks`/`Areas` modes,
because it addresses targets by name with a window taken from the reference.

We do NOT take the FCP magnetic model (ripple as the only behavior): it is
incompatible with the legal overlaps of `overlap_layers`, introduced at the user's
direct request, and with rule 20 in its literal wording.

## Sources

- https://cdn.borisfx.com/borisfx/Documentation/vegas/2026/en/content/topics/7-edit/post_edit_ripple.htm — official help for VEGAS Pro 2026: the three ripple types by name, the list of triggers, `F`/`Ctrl+F`/`Ctrl+Shift+F`, `Ctrl+L`, the red indicator and the checkbox that turns it off (2026-09-07)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/post_edit_ripple.htm — the same page in the Vegas Pro 22 help; the wording matches (2026-09-07)
- https://www.dvinfo.net/forum/what-happens-vegas/139549-bugs-auto-ripple.html — the thread "Bugs? Auto Ripple", December 2008: why veterans keep Auto Ripple off, and the drift mechanism with imperfect alignment (2026-09-07)
- https://creativecow.net/forums/thread/is-it-possible-to-perform-this-ripple-edit-while-n/ — Avid Media Composer, April 2013: sync lock turns an edit that breaks sync into a REFUSAL with an error sound (2026-09-07)
- https://creativecow.net/forums/thread/auto-ripple-not-affecting-all-events-in-my-timelin/ — Vegas, June 2013: "some events did not move" turned out to be the selected type "affected tracks only" (2026-09-07)
- https://community.adobe.com/t5/premiere-pro-discussions/sync-lock-not-respected-during-ripple-delete/m-p/15305272 — Premiere, May 2025: sync lock is asymmetric, the workaround is to switch it off on all tracks (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/intro-to-the-magnetic-timeline-verb8fcfc133/mac — FCP: clips part and close up by themselves, connected clips move with their anchor (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/remove-clips-from-projects-verc5d34129/mac — FCP: `Delete` ripples, `Shift+Delete` (Replace with Gap) keeps the gap and the connections (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/insert-clips-ver4e2eff6/mac — FCP: insert (`W`) ripples forward and cuts the clip under the cursor (2026-09-07)
- https://support.apple.com/guide/final-cut-pro/overwrite-clips-ver4e30026/mac — FCP: overwrite (`D`) does not ripple, the project length does not change (2026-09-07)
- https://community.adobe.com/t5/after-effects-discussions/these-two-button-options-ripple-insert-overlay-edit-are-not-showing-in-my-footage-window/m-p/13791419 — the Ripple Insert / Overlay Edit buttons in AE live ONLY in the Footage panel (2026-09-07)

**Honest about gaps.** `helpx.adobe.com` (the Premiere and AE help) does not open from this
environment — `proxy refused the connection` — so the Premiere and After Effects rows rely on the official Adobe community and
secondary sources. The Premiere shortcuts (`Shift+Delete` — Ripple Delete, `B` —
Ripple Edit Tool) and Resolve (`Shift+Delete`/`Shift+Backspace`, `T` — Trim
Edit Mode) are taken from
secondary sources (ledet.com, filmora, shortcutkings) that agree
with one another, but have NOT been checked against the official help. The wording `Timeline > Ripple
Timeline Markers` was found in the index results for a mirror of the official Resolve 18.6 manual
(`steakunderwater.com/VFXPedia/__man/Resolve18-6/`), but a direct request for the page returned 404 — the fact is marked `[NOT VERIFIED]`,
to be checked against the Resolve 21 manual in the next wave. `reddit.com`, `web.archive.org` and
`discussions.apple.com` are unavailable, the `WebSearch` budget is exhausted — searching went
through direct queries to Yahoo. None of the five editors was run live.

**Code checked by grep** over the working tree at commit `0732ea8` (branch
`threed`): `src/libs/kt-model/src/ops.rs`, `src/libs/kt-model/src/model.rs`,
`src/libs/kt-model/src/project.rs`,
`src/apps/kt-app/src/{app,timeline,toolbar}.rs`.

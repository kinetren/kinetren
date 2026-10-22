---
id: F-TL-007
title: Guides and spacer tool
status: candidate
priority: B
cost: M
category: timeline/navigation
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: partial
crates: [kt-model, kt-app]
invariants: []
ui_rules: [4, 13, 16, 17, 20]
round: null
point: null
tests: []
depends_on: [F-TL-001]
trackers: [kde-invent, kde-bugzilla, pitivi]
checked: 2026-09-08
---

# F-TL-007 — Guides and spacer tool

## Essence

A guide is a vertical line on the timeline to which edits snap: not a note about a moment, but an element of the GRID. A spacer is a tool that grabs everything to the right of the click point on all tracks at once and, with one gesture,
inserts or removes emptiness. Together they answer one question: when the content has moved, does the markup move with it — or does it stay put and,
after a dozen edits, start lying, showing "the start of the second act" in the middle of
someone else's plan.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Markers, Regions; `Insert \| Time`; Auto Ripple types | There is no separate spacer tool: emptiness is inserted by a command — "insert a specified amount of blank space into the VEGAS Pro project at the cursor position", and without selected events "time is inserted across all tracks". There is also no separate guide — its work is done by markers and regions, to which snapping goes. "Does the marker move" is not a property of the marker, but the ripple TYPE: the three states of the button, analysed in F-TL-001. A 2010 forum reply to the direct question "how to move markers together with clips" — "Enable Auto Ripple… option 2 … or option 3" [the 2026 help returned 403; quotes from the Vegas 23 help on `help.magix-hub.com`] |
| Adobe Premiere | 26 | Track Select Forward Tool (`A`); Sequence markers / Clip markers | The spacer in Premiere is not an insert command but a SELECTION TOOL: `A` takes all clips to the right of the click (by default on all tracks, `Shift+A` — on one), then an ordinary drag. Markers are split into two entities: the sequence marker (on the timeline) and the clip marker (inside the clip) — the correspondence is confirmed by Adobe itself, mapping them onto AE. There is no guide separate from markers [the `A` shortcut is from secondary sources: `helpx.adobe.com` does not open from this environment] |
| DaVinci Resolve | 21 | Timeline markers / Clip markers; `Timeline > Ripple Timeline Markers` | The same pair "timeline marker / clip marker". There is no spacer tool — emptiness is moved by selecting clips forward and an ordinary drag. "Whether timeline markers move" is put into a SEPARATE menu toggle, independent of the gesture by which the edit goes (the analysis and the `[NOT VERIFIED]` note about the unavailable manual page — F-TL-001) |
| Final Cut Pro | 12.3 | neither a spacer nor a guide; gap clip and markers on clips | The magnetic model carries the idea to its end: emptiness is not "pushed apart" by a gesture, it is inserted as an OBJECT (a gap clip), because a hole in a magnetic timeline must be something. Markers are "reference points you can place within clips": FCP has NO timeline-anchored line at all, so the question "does it move" does not arise — what is marked lies inside the clip, and it always moves |
| After Effects | 26 | Composition markers / Layer markers | Exactly the same pair, and in AE it is named directly: a composition marker lives on the composition's time ruler, a layer marker on the layer's bar; Adobe itself maps them onto Premiere (composition = sequence marker, layer = clip marker). There is no spacer: layers are parallel, so there is nothing to move among neighbours on a track. A terminological trap: the word "guide" in AE and Photoshop is taken by a SPATIAL line in the viewer, not a temporal one [`helpx.adobe.com` is unavailable: the mapping is quoted from the index results] |

**Outside the core.** The separation of a "guide" and a "marker" into two entities, and an
explicit answer to the question "does the line move", exist in documented form only in
Kdenlive — and only in its public tracker can one see what this separation costs.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Kdenlive | 26.08 | Spacer tool (`M`); Timeline Markers (formerly Guides) / Markers; Timeline Marker Locked | Spacer: "Use this to create or remove space in the timeline between clips. Temporarily groups clips for dragging them for this purpose." — that is, the tool TEMPORARILY groups the clips and drags the group. Markers are split by owner: "Markers are inside the clips and move with the clips", the timeline marker stays. Whether it moves is a GLOBAL lock in the markers panel: "When locked, the timeline markers stay in place. When unlocked, the timeline markers move with the clip" |

## The trick behind it

**1. The answer to "does the line move" everywhere turned out to be a session mode, although
the question is about the line.** In Kdenlive this is a global lock in the markers panel
(it appeared in 21.08 as a direct answer to a tracker request), in Vegas it is the ripple type
(three button states), in Resolve it is the `Ripple Timeline Markers` toggle. In none
of them is it a property of the marker itself. Kdenlive's own tracker has kept an alternative open since 2023 and
formulates it more precisely than the vendor: "Because
there are two equally valid and useful use cases, it would be awesome if we
could satisfy both" — it proposes an anchor square on the guide itself,
which can be dragged down onto a clip, and then the line is attached to the clip rather than to the
timeline. Three years without movement. This is exactly the hole that F-TL-001
named from the ripple side ("none of the five editors can tell them apart,
so the decision is put into a global toggle which each time is the wrong one") —
and now it is visible that the hole was noticed not only by us: over nine years the same request
came in four independent ways (2016 → closed as `WORKSFORME`, 2020 →
closed with a lock, 2023 → opened, 2025 → twice).

**2. The guide/marker separation blurs from both ends at once: the word was removed,
the thing remained, and a live scenario broke on it.** First "Guide" was recognised as
unclear and renamed "Timeline Marker" (kde-bugzilla #471082, 2023,
FIXED). Then it was declared that there is no difference at all: "Now that timelines are
in fact sequence clips, there is no real difference between `Markers` and
`Guides`" (kde-invent #2011, 2025). The manual page today is called
"Timeline Markers" and lives at `…/cutting_and_assembling/guides.html` —
the renaming reached the heading and did not reach the URL. And five months
after the merger of the names a user arrived: "I read through some of the commits
here, and it seems that all the guides have now been replaced with Markers.
However, I seem to be unable to add a marker to the timeline" — "Add marker"
now requires a SELECTED CLIP, while a guide by definition does not require a clip. The merging of names immediately broke precisely that for which
there were two entities.

**3. The spacer is ripple, expressed as a DIRECT gesture, and it has a different class
of defects.** Across the whole sample of spacer tasks there is NOT ONE instance of "forgot that
the mode is on" — the very class that fills the Vegas forum about Auto Ripple
(F-TL-001, the whole section "What users say"). Instead: it is slow
(kde-invent #963, 28 comments; kde-bugzilla #414006 "prohibitively slow",
open since 2019, closed in 2026), crashes (#435), hangs on grouped clips
(kde-bugzilla #443324, CONFIRMED since 2021 and to this day), cannot ignore
locked tracks (#1436), carries clips through obstacles (#1879). Replacing an
invisible mode with a direct gesture changes the class of errors from ATTENTION to
PERFORMANCE AND SCOPE. The second class is measurable, reproducible and
closed by a test; the first is not. This is the strongest of the known arguments
for a gesture over a mode, and it was obtained by counting, not by taste.

## Pros

- **The spacer needs no state.** There is nothing to forget to switch on and nothing to forget to switch off: the tool is chosen, the gesture is made, the consequence is exactly what was pulled. The class "I set rules at 03:00, it moved up to 40:00 and I did not notice" disappears not through attentiveness but by construction.
- **A guide that moves with the content stays true.** The meaning of the line "the second act starts here" lies in its relation to the material, not to the running time; a line left standing after three insertions shows the middle of someone else's plan and is no longer markup but garbage. This is exactly what is written in the original tracker request.
- **One spacer covers both inserting and deleting emptiness.** Right — insert, left — remove; separate "insert space"/"remove space" commands are not needed, and so there is no need to make sure they behave the same way.
- **The scope of action is visible in the gesture itself.** Where you click is where it moves from; how far you drag is how far it moves. Unlike a mode, where the scope is set in advance and in another place on the screen, here both the point and the amount are shown at the site of the edit.
- **The separation "clip marker / timeline marker" is honest in itself.** A note about a frame of material must move with the material, a note about the structure of the film need not; where there is one entity (FCP, us), one of the two jobs is done badly.

## Cons

- **Splitting into two entities pays in confusion, and the cost is visible in the tracker.** The term had to be admitted unclear and renamed; after the renaming the entities began to merge in the interface, and the add command began to require a clip. Two entities that look almost the same on screen are a constant tax on explanation, paid by every new user.
- **A global "moves/does not move" lock is wrong each time.** It is right for one line and wrong for a neighbouring one standing two seconds away, and it switches for the whole project. This is the same disease as Vegas's ripple types, only made more cheaply.
- **The spacer is the heaviest gesture on the timeline.** It touches everything to the right of the point on all tracks, and it is the only operation whose cost grows with the length of the film. Five tasks in a row in one tracker are about it being slow, hanging or crashing on large projects.
- **"Everything to the right" is not the same as "everything that needs to be moved".** A locked track, a watermark spanning the whole length, a music bed: in Kdenlive it had to be asked for separately that the spacer not touch them. A scope you do not set explicitly has to be defined afterwards by requests for years.
- **The word is taken.** In AE and Photoshop "guide" is a line in the FRAME, not in time; in Pitivi the only task with the word guide in the title is also about a composition grid in the monitor. A term taken into the Russian UI as the word for "guide" will collide with the same line in the viewer, once hands reach the markup of the frame.

## What users say

> I suggest moving the guides along with the clips when using the Spacer Tool.
> I don’t know how correct this is, but from personal experience I want to say
> that whenever I move clips using the Spacer Tool, I want guides to move with
> it.
> — kde-invent #702, 28.05.2020,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/702

> Guides are currently anchored to the timeline, ie: when clips are moved on
> the timeline, the position of the guide is not updated.
> — kde-invent #1691 (open to this day), 08.05.2023,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1691

> Allow guides to be attached directly to clips in the timeline. This way, when
> a clip is moved or adjusted, the guide would move along with it, making it
> much easier to maintain consistent alignment between clips and guides.
> — kde-invent #1989, 07.04.2025,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1989

> Now that timelines are in fact sequence clips, there is no real difference
> between `Markers` and `Guides`.
> — kde-invent #2011, 22.05.2025,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/2011

> I read through some of the commits here, and it seems that all the guides
> have now been replaced with Markers. However, I seem to be unable to add a
> marker to the timeline.
> — kde-invent #2077, 26.10.2025,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/2077

> The problem is that I use a LOT of markers when I’m working on my projects,
> and those do not move with the clips when I move them.
> — Ryan Moyer on VEGAS, 25.01.2010,
> https://creativecow.net/forums/thread/move-markers-together-with-clips/

The negative half is about the cost of the gesture itself, not its meaning:

> Like the "remove space in all track" can be nice if the "spacer tool" can
> move the clips ignoring "locked" tracks.
> — kde-invent #1436, 18.05.2022,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1436

SIGNAL: whether the markup moves together with the content — 6 tasks in kde-invent, `(guide|marker)s?.{0,30}(move|moving|shift|ripple|along|attach|follow)|(move|moving|shift|ripple|attach).{0,30}(guide|marker)`; share 0.3% versus 0.1% in kde-bugzilla

SIGNAL: the spacer as a gesture — 16 tasks in kde-invent, `spacer|insert space|remove space|insert time|remove time`; share 0.7% versus 0.4% in kde-bugzilla

SIGNAL: the word "guide" itself — 19 tasks in kde-invent, `guide`; share 0.9% versus 0.1% in pitivi

How to read this. The first line is the subject of the note: the question "does the marker move"
is asked in the tracker where development lives (kde-invent) about three times as often, relative
to size, as in the tracker where complaints live (kde-bugzilla); this is a request, not a
defect, and it does not dissolve — four independent attempts over nine years
(2016 `WORKSFORME`, 2020, 2023 open, 2025 twice). The second: the spacer
is relatively more often discussed in the same place — and in its tasks speed and
scope dominate, not "an enabled mode". The third: the word "guide" is practically
kde-specific — both Pitivi tasks with the word in the title relate to the timeline
not at all (a starting guide and a composition grid in the monitor), so 0.1% is zero, counted honestly. The guide/marker
separation exists with one vendor, and the whole tally on it is its own. The share of
real fixes among those closed in kde-bugzilla proves nothing here:
"closed" in this tracker means "taken off the queue" no less often than "done"
(`WORKSFORME` — 1549 out of 6346), and the 2016 request was closed precisely that way.

## How it fits Kinotrain

### What already exists

- **Our marker is in fact a guide, and it already does both of its jobs.**
  `Marker` (`src/libs/kt-model/src/project.rs`) lives on the timeline, does not
  belong to any clip, its address is the tick `t`, and it may be a point or a range. In OTIO it is written as a marker of the ROOT `Stack`
  (`src/libs/kt-otio/src/convert.rs`, `tl.tracks.markers`) — by the standard this is
  exactly a "timeline marker", that is, Kdenlive's guide. The second job —
  magnetic — it also already does: `TimelineGeom::snap_targets`
  (`src/apps/kt-app/src/timeline.rs`) puts `m.t` into the targets and, for a range marker, `m.end()`.
- **We have no CLIP marker at all — and this is consciously visible in the code.** Foreign
  clip markers pass through us as an opaque passthrough
  (`markers: c.markers.clone()` in `convert.rs`), but the model has no entity of its own "a note
  inside a clip, moves with the clip". We have nothing to separate: for us a
  guide and a marker were never two objects, because a second object does not exist.
- **The magnet and its targets are pure functions with a test.** `snap_point`, `snap_move`,
  `snap_threshold` (`src/apps/kt-app/src/timeline.rs`), targets: zero, playhead,
  the edges of foreign clips (their own link group is excluded — item 2 of R.11), marks and
  the ends of range marks, the edges of the region. Test t95. The default already follows
  rule 4: the magnet is on, `Ctrl` switches it OFF (`snap_off`,
  `timeline.rs:1996`), and this is a direct decision of the owner ("ctrl, I see no
  problems using it", R.11).
- **Named AREAS — a guide with a scope of action that none of the five has.** `MarkedArea` is a name-address plus time × SEMANTIC track keys
  (`lanes`, `covers_lane`): "this markup is about video 2 and audio 1, not about the whole width of the screen". The region (`Project::region`) is the same thing for one gesture.
- **The spacer's engine is already written and covered by a test.** `plan_move_block`
  (`src/apps/kt-app/src/timeline.rs`, test t204) moves a BLOCK of clips by a
  common delta, does not change the mutual distances inside the block, clamps on the
  first obstacle and returns `blocker` — who exactly was hit. Foreign
  placeholders (`Project.foreign`) remain an obstacle even with layer overlap.
- **An obstacle is already able to be an ERROR, not a faded hint.**
  `KtApp::blocked` (`src/apps/kt-app/src/app.rs`) plus test t172 — rule 16
  for this gesture has already been carried out by someone else's hands.
- **Atomicity and one undo step are free:** `apply_batch` / `apply_coalesced`
  (`src/libs/kt-model/src/model.rs`), see F-TL-001 — I am not reopening it.

### What is missing

1. **A selection rule for "everything to the right of the point".** We have two ways of selecting:
   click and band (`last_band`, tests t202/t203). A third — "from the point to the
   end, on all tracks or on one" — does not exist, and the spacer is precisely that.
2. **Shifting NON-clips.** `plan_move_block` looks only into `p.clips`. Marks,
   areas, morphers and cameras are addressable by commands (`MoveMarker`,
   `SetAreaRange`, `SetMorpher`, `SetCamera`), but no gesture moves them;
   `Project.foreign` and `Project.timeline_end` are not edited by any command —
   a finding of F-TL-001, quoted here, not reopened.
3. **Areas as a magnetic target.** `snap_targets` knows marks and the region and does NOT
   know `p.areas`. A guide to which one cannot snap is colouring, not a grid; we have the richest markup of all five and it is the only non-magnetic one.
4. **A sign of "this line moves with the content".** Neither `Marker` nor
   `MarkedArea` has such a field — the question did not exist until now because a moving gesture did not exist.

### What gets in the way

- **The spacer would be the FIRST shifting gesture, and the price of this is paid by names.**
  The morpher addresses its targets by name: `MorpherEvent::ref_windows`
  (`src/libs/kt-model/src/project.rs`) resolves `p.named_range(n)` and
  `p.area(n)` and intersects the result with the body of the clip. To shift the clips and not
  shift the marks/areas is to tear the layer off its targets: the name will stay alive,
  the window will drift, and the patch will land on someone else's piece. F-TL-001 concluded from this that "ripple
  markers and regions" for us is not a third state of the button but a condition
  of correctness. For the spacer this conclusion is HARSHER: it has no states at all —
  there is one gesture, and "do the referenced windows move" is decided once and for all, not
  chosen by a button before the edit.
- **To the right the spacer is safe, to the left it is not, and this is the main lever on the cost.**
  With the selection rule "everything to the right of the point", no clips of its own remain to the right of the block at all:
  movement to the right cannot run into anything. The only exception is `Project.foreign`: `plan_move_block` counts foreign
  placeholders as an obstacle ALWAYS, even with layer overlap, and there is no command
  to move them (item 2 above). To the left the spacer runs into exactly what the ripple runs into: with `overlap_layers` (the default, the owner's direct request,
  item 9 of R.24) there is no stop — the block will slide into the neighbour on the left and generate an auto-crossfade there where nobody touched anything (rule 20), while
  with layer overlap switched off `validate()` will return `ClipOverlap` and the atomic
  batch will roll back the whole gesture.
- **Shortcuts are taken in both reference editors.** `M` — in Kdenlive it is the spacer, for us "place a marker" (`app.rs:3935`). `A`/`D` — in Premiere the forward selection tool, for us navigation across the joints, and `Shift+A`/`Shift+D` — the region pull (test t119). Not a single foreign layout can be transferred, and this has to be decided before, not after.
- **A marker is identified by its tick, and the mass shift trips over this.**
  `Command::MoveMarker` is rejected with `MarkerExists` if someone already stands at the target tick (`check_on`, `src/libs/kt-model/src/model.rs`): the traversal order must depend on the sign of the delta, and a marker that arrives on a marker to the left of the edit point will bring down the whole batch. The analysis is in F-TL-001, I refer to it.
- **An area is not a segment, but time × tracks.** A spacer on ONE track (a modifier that Premiere has and Kdenlive is asked for) meets an area covering the neighbouring tracks too: it can be neither left in place nor shifted whole without misrepresenting anything. There is no third option, and the choice here belongs to the user, not to us.

### Which invariants it touches

**None of the twelve** — and this is the main argument for "cheap". The only candidate is invariant 3 ("editing the model does not destroy FX state"): the chain diff goes by clip id, so the gesture must MOVE clips (`MoveClip`), not recreate them. The spacer does exactly this — the ids survive the gesture, and so do the reverb
tails. This is precisely where it is cheaper than ripple deletion, which
recreates the remainders with new ids in `ops::punch_out_region`. It produces no decode
(No. 8) and does not touch the audio path (No. 1, No. 2), the composition code (No. 7) or determinism (No. 11).

UI rules: **20** — central and, for once, in our favour: the spacer does not "fix up the neighbours on the user's behalf", it carries out the gesture it was given, so the ONLY form the rule allows for a shift is already available to it by construction; **4** — the default "lines move" is decided once and is overridden by a modifier, exactly as with the magnet; **13** — the displaced block and the stop must be shown in a place already occupied (the block highlight and `blocked`), no new strip should be introduced; **16** — the stop must already be an error and already is one (t172); **17** — the spacer gesture, the menu item and any keyboard form must decide the fate of marks/areas in the same way.

ADDRESS: src/apps/kt-app/src/timeline.rs — the class "the markup lied after a shift" is IMPOSSIBLE for us today (there is no shifting gesture: `plan_move_block` moves only an explicitly selected batch of clips), but a neighbouring class is possible and is not caught by anything: `snap_targets` collects targets from clips, marks, the region, the playhead and zero and does NOT know `p.areas`, so a named area is the only one of our markup elements that is drawn, is addressed by name by the morpher, and yet does not snap the magnet.

Related notes: **F-TL-001** (ripple as a property of an edit; the spacer is its cheap half without a mode, and all conclusions about marks, `restore_handles`,
`foreign` and layer overlap are taken from there), **F-TL-002** (slip/slide — trims
that do not change length, that is, the opposite end of the same axis).

## Estimate

| What | Answer |
|---|---|
| Size | **M** — 2–3 stages. (1) the selection rule "everything to the right of the point" (all tracks / one) plus a gesture on empty space on top of the ready `plan_move_block`; (2) shifting the referencing entities in one batch — marks, areas, morphers, cameras — and an honest refusal on `foreign` (rule 16); (3) UI: the tool cursor, highlight of the block and of the stop on the already occupied place, a hint, a menu item, i18n strings. Stage (1) on its own gives "insert emptiness to the right" and has almost no stops |
| Where the main work is | `kt-app` (`timeline.rs` — the selection rule and the gesture; `app.rs` — the action and the refusal), `kt-model` (`ops.rs` — the batch shifting the markup). `kt-engine`/`kt-gpu`/`kt-fx` are not touched |
| Project format | not touched, UNTIL the sign "this line moves" is introduced as a field of the marker. If we introduce it — it will be a new field of `Marker`/`MarkedArea` under `skip_serializing_if`, old files stay byte-stable, but the OTIO marker gets our `metadata["kt"]` |
| Tests to be written | the selection "everything to the right of the point" takes exactly what lies to the right, on all tracks and on one; a shift to the right runs into nothing except `foreign`, and on `foreign` gives an error, not a silent drift; marks, areas, morphers and cameras moved by the same delta, and `MorpherEvent::ref_windows` returned the same windows as before the gesture; a mass shift of marks does not fail on `MarkerExists` with either sign of the delta; a shift to the left with `overlap_layers` does not create a crossfade by itself; the area in `snap_targets` (an extension of t95); a harness test of the gesture in `layout_tests.rs`; the menu strings in `i18n_tests.rs` |
| Depends on | F-TL-001 neither blocks nor is blocked: the spacer is its half, expressed as a gesture. If both are done, "do marks and areas move" must be ONE decision for both, otherwise we get two shifts with different rules |
| Breaks compatibility | no |

## Verdict

We take it, priority B: inserting and removing emptiness on all tracks at once — the gesture is daily, and we have none of it at all. It sits below A only because
the structural gap ("an edit with a shift of everything to the right") is held by F-TL-001, and
the spacer is its cheap and safe half.

We do NOT introduce the guide as a separate entity. Our marker is already a timeline-anchored
line and already snaps; the second entity ("a note inside a clip") the Kdenlive tracker shows not as an achievement but as a bill: the term
had to be renamed, merged, and at the merger a live scenario broke. From there we must take
not an object but a PROPERTY — whether this line moves with the content or stays put —
and the tracker itself says where it belongs: not in a session toggle but on the line itself
(kde-invent #1691, open three years). This decision is taken BEFORE the first shifting gesture, because it is about the address of the marker, not about convenience.

Separately and above everything else: **the named area must be added to the magnetic targets.** This is one line in `snap_targets`, it breaks nothing, and it closes our only non-magnetic markup — the richest of the existing ones among the five editors.

## Sources

- https://invent.kde.org/multimedia/kdenlive/-/work_items/702 — kde-invent #702 "Move guides along with clips using the Spacer Tool", 28.05.2020, 24 comments, labels `Enhancement`, `UI | UX`; closed in 2021 (access 2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1691 — kde-invent #1691 "Guide anchors and pointers", 08.05.2023, OPEN: a proposal to make "moves/does not move" a property of the line itself via an anchor square (2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1989 — kde-invent #1989, 07.04.2025: the same request independently, mirrored in kde-bugzilla #502544 (2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/2011 — kde-invent #2011, 22.05.2025: "stop Guides, Markers everywhere", 25 comments (2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/2077 — kde-invent #2077, 26.10.2025: after the merger of names a marker cannot be placed without a selected clip (2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1436 — kde-invent #1436, 18.05.2022: a request that the spacer ignore locked tracks (2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/963 — kde-invent #963 "Spacer tool is too slow", 28.02.2021, 28 comments, label `Performance` (2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=414006 — kde-bugzilla #414006 "Spacer tool is prohibitively slow with many timeline clips", filed 10.11.2019, closed FIXED 24.01.2026 (2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=443324 — kde-bugzilla #443324 "Spacer Tool hangs the app if used with grouped clips", CONFIRMED since 04.10.2021 (2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=358570 — kde-bugzilla #358570, 26.01.2016: "guides attached to clips that automatically move with clip", closed `WORKSFORME` (2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=471082 — kde-bugzilla #471082, 15.06.2023: the term "Guide" recognised as unclear, FIXED (2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=500773 — kde-bugzilla #500773, 26.02.2025: "Insert Space" should be able to move guides, FIXED (2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=474577 — kde-bugzilla #474577, 15.09.2023: the insert and remove time commands did not fix guides, FIXED (2026-09-08)
- https://docs.kdenlive.org/en/cutting_and_assembling/guides.html — the Kdenlive 26.08 manual, the page is called "Timeline Markers", the address stayed `guides.html`: the difference between timeline markers and clip markers, the "Timeline Marker Locked" lock and its effect on the spacer (2026-09-08)
- https://docs.kdenlive.org/en/cutting_and_assembling/editing.html and https://docs.kdenlive.org/en/user_interface/menu/tool_menu.html — Spacer tool: shortcut `M`, "create or remove space", temporary grouping of clips, action on all tracks at once (2026-09-08)
- https://help.magix-hub.com/video/vegas/23/en/content/topics/7-edit/inserttime.htm — VEGAS: `Insert | Time` inserts emptiness at the cursor, without a selection — on all tracks (2026-09-08)
- https://creativecow.net/forums/thread/move-markers-together-with-clips/ — VEGAS, January 2010: the same question ten years before Kdenlive, and the answer is the same — the Auto Ripple type, not a property of the marker (2026-09-08)
- https://support.apple.com/guide/final-cut-pro/intro-to-markers-ver397279dd/mac — FCP: markers are placed INSIDE clips (2026-09-08)
- https://blog.nobledesktop.com/learn/premiere-pro/using-the-track-select-forward-tool-in-premiere-pro — Premiere: `A` selects everything to the right of the click, by default on all tracks (a secondary source; 2026-09-08)

**Honest about the gaps.** `helpx.adobe.com` does not open from this environment
(`proxy refused the connection`) — the Premiere and After Effects rows rely on
secondary sources and index results that converge with each other, but have not been checked against the official help. The `Insert Time` page of the VEGAS 2026 help
on `cdn.borisfx.com` returned 403, so the quote is taken from the VEGAS 23 help on
`help.magix-hub.com`, where the wordings were already recognised as matching in the first wave
(F-TL-001). The Resolve row relies on the analysis of F-TL-001 together with its
`[NOT VERIFIED]` note. No editor was run live.

**About the numbers.** The export ran in the background: at the time of writing the cache held nine dumps out of
the twelve in the registry `_trackers.md` (`kde-bugzilla`, `kde-invent`, `pitivi`,
`shotcut`, `mlt`, `otio`, `olive`, `flowblade`, `losslesscut`; `openshot`, `blender-vse`, `ffmpeg-trac` were absent). For the subject of the note this matters little:
the guide/marker separation exists with exactly one vendor from the list, and the whole
tally on it is its own; in the other trackers the filters give single units, and
this is part of the finding, not a gap. The `SIGNAL:` line figures are reproducible with
`python3 src/scripts/tracker-rank.py --query '<filter>'` on the same dumps.

**The code was checked by grep** against the working tree at commit `0732ea8` (branch
`threed`): `src/apps/kt-app/src/timeline.rs` (`snap_targets`, `snap_point`,
`snap_move`, `plan_move_block`, `snap_off`), `src/apps/kt-app/src/app.rs`
(`blocked`, `last_band`, `save_marked_area`, the `M`/`A`/`D` bindings),
`src/libs/kt-model/src/project.rs` (`Marker`, `MarkedArea`, `ref_windows`),
`src/libs/kt-model/src/model.rs` (the mark and area commands),
`src/libs/kt-model/src/ops.rs`, `src/libs/kt-otio/src/convert.rs`.

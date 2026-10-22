---
id: F-COL-023
title: Markers ON A CLIP, not on the edit
status: researched
priority: B
cost: M
category: collab/review
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, otio-lib-0.18]
we_have: partial
crates: [kt-model, kt-otio, kt-media, kt-app]
invariants: [8]
ui_rules: [13, 20]
round: null
point: null
tests: []
depends_on: [F-COL-020]
trackers: []
services: [frame-io, bm-cloud]
checked: 2026-09-08
---

# F-COL-023 — Markers ON A CLIP, not on the edit

## Essence

Half of the remarks are not about the film but about a PIECE of footage: "in this take there is a clapperboard", "the actor stumbled here", "from the 12th second, a focus defect". Such a remark has to travel together with the footage: the edit will be re-assembled tomorrow, but the focus defect stays on the same frame of the same file.

That is why everyone has two different carriers of markup — on the edit and on the clip — and this is not a matter of convenience but of different lifetimes: the first dies on re-assembly, the second does not.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Media markers and regions | the most radical variant: the marker is written INTO THE FILE itself, and if the format cannot hold it — into a `.sfl` sidecar next to it. On the timeline it is shown in every event that references this file, and it is edited only in the Trimmer: "Media markers and regions cannot be edited within the event" |
| Adobe Premiere | 26.x | Clip markers vs Sequence markers | two independent spaces; a ripple edit moves the sequence markers, but "clip markers always travel with a clip". A master-clip marker in the bin is visible in all of its instances on the timeline |
| DaVinci Resolve | 21 | Clip markers · Timeline markers · Flags | THREE carriers, separated by scope: a clip marker, a timeline marker, and a flag that belongs to the source and colours all of its instances, having no time at all |
| Final Cut Pro | 12.3 | Markers (on clips only) | there is no ruler for markers at all: "You work with markers in the same way for clips in the browser and clips in the timeline". Edit markup is expressed by a marker on the placeholder clip — that is, there is exactly one carrier, and it is a clip |
| After Effects | 26.x | Layer markers vs Composition markers | two levels, as in Premiere, but a layer is not the footage: a layer marker travels with the layer, and an element of the footage in the project carries no markup of its own |

**Outside the core.** Two sources are needed: a reference schema showing WHERE such a marker belongs in the exchange file, and a format note that has pulled this out into a separate section.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| OpenTimelineIO | 0.18.x | `Item.markers` | markers are a field of `Item`, of its parent, and of `Clip`, `Track` and `Stack` alike. That is, in the schema a clip marker and an edit marker are ONE and the same object on different owners, and it lives in the coordinate system of its owner: for a clip these are the coordinates of the footage, for a stack the coordinates of the edit |
| OTRN | 1 (November 2025) | `files[]` next to `sequence.notes` | a second, optional section of the file — "a `files` array that allows users to attach additional notes to files and clips outside the sequence range". Notes about footage were deliberately moved out of the edit section: they live even when there is no edit yet |

## The trick behind it

**1. A marker on a clip is durable not because it "travels with the clip", but because it is counted in OTHER COORDINATES.** It is expressed in the time of the footage, so trimming does not move it — it HIDES it: the frame the marker points to has not gone anywhere, it has simply stopped falling into the window of the event. The non-obvious consequence: trim back and the marker returns; while an edit marker that once slid during a ripple does not come back by any means. The difference between the two carriers is not "attached to the clip", but whose coordinates the address is in.

**2. VEGAS took the idea to the end and paid for it with editability.** A media marker is written inside the file (or into a `.sfl` sidecar, if the format cannot hold it), so it is visible in EVERY event that references this material — and that is exactly why it is edited only in the Trimmer: the help says outright that it cannot be edited in the event. This is the honest price of a shared carrier: what belongs to the material cannot be edited from one edit, otherwise an edit would silently change its neighbours. The event becomes a READING window onto someone else's data, not its owner.

**3. Frame.io in Final Cut collects remarks into a compound clip — and breaking apart the compound clip is the moment when a review "lands" on the material.** The instruction literally suggests "Break apart the compound clip so the markers will stay attached to the clips": while it is one flat construction, the remarks refer to the reel; break it apart — and each one sticks to its own piece. That is, the transition "remark about the film → remark about the material" is performed BY HAND and exactly once, and after it the remarks survive any re-assembly.

**4. The notes format moved this into a separate section, not into a field.** In OTRN, `sequence.notes` and `files[]` are different sections, and the second is declared for notes "outside the sequence range". This is an acknowledgement that a note about the material exists BEFORE the edit and outside it: it is legitimate even when no sequence exists yet — for example, in dailies where the footage is marked up while the editor only arrives tomorrow.

## Pros

- A remark survives re-assembly of the edit — the only property for the sake of which the carrier exists.
- One markup instance serves many uses: a take inserted three times brings "here is a defect" with it three times.
- Markup exists before the edit: dailies and QC mark up the footage when there is no timeline at all.
- Trimming does not destroy markup, it hides it: the gesture is reversible, which cannot be said of an edit marker.

## Cons

- A shared carrier means shared ownership: an edit from one place changes its neighbours, and VEGAS honestly forbids editing within the event — the price is inconvenience.
- Writing markers INTO THE SOURCE FILE changes the footage, and sources are conventionally considered untouchable; the sidecar solves this but multiplies files, which get lost when a folder is copied.
- A marker outside the event's window is the most frequent trap: it exists in the data and is visible nowhere, and after trimming back it appears "from nowhere".
- Two carriers require a constant answer from the person to the question "is this note about the film or about the piece", and interfaces usually put both on the same key.
- A marker on a clip does not answer the question "where in the film is this", if the take is used twice: by construction it has no edit address.

## What users say

Practitioners put the difference between two carriers in one phrase — and the same phrase explains why the client's remarks are placed specifically on the clip:

> This is not true if the markers are in the clips themselves; clip markers always
> travel with a clip.
> — Larry Jordan, "Premiere Pro: Timeline Marker Secrets", 06.09.2020,
> https://larryjordan.com/articles/premiere-pro-timeline-marker-secrets/

And the cost of a shared carrier is best named by the VEGAS help, and it is a limitation, not a capability:

> Media markers and regions cannot be edited within the event.
> — VEGAS Pro help, "Using media markers and regions",
> https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/media_markers.htm
> (accessed 2026-09-08)

## How it fits Kinotrain

### What we already have

Checked against the code on 08.09.2026, and the picture turned out worse than "clip markers are not shown".

**We have no carrier of our own.** `ClipEvent`
(`src/libs/kt-model/src/project.rs:1845`) consists of an id, a group, a kind, a track,
time, `source_in`, media, gain, reverse, the FX chain, bypass,
opacity, automation and a master switch. There is no markers field among them, and
`Command` contains not a single operation on them. Our markup is exactly one —
`Project.markers`, global and per edit.

**Foreign clip markers pass through and are invisible.** They are read into
`ClipExtra.markers` (`src/libs/kt-otio/src/convert.rs`) and written back
verbatim; in `kt-app` there is not a single reference to them — the application works only
with `project.markers`. That is, remarks from the client that arrive in a foreign `.otio` on
clips are preserved by us and at the same time do not exist.

**The binding is by `ClipId`, and a split breaks it.** `Retained.clips` is
`HashMap<ClipId, ClipExtra>`, and the writer takes the record strictly by the clip id:
`retained.clips.get(&c.id).cloned().unwrap_or_default()`. On `Command::Split`
the left half KEEPS the original id, the right one gets `new_id`
(`src/libs/kt-model/src/model.rs`). Two things follow at once:

- the left half gets ALL the markers of the clip, including those whose footage time
  now lies beyond its `source_range` — exactly the trap "the marker is in the data but
  outside the window", except that for us it arises not from trimming but from the split;
- the right half gets `ClipExtra::default()` and loses NOT ONLY the markers:
  together with them go the clip name, foreign effects, the whole `media_references` map and the
  `available_range` of the active reference. The last is the most unpleasant:
  `media_origin` for the right half becomes zero, and its `source_range`
  is written without the footage timecode that the left half preserves. The two
  halves of one split drift apart in the file by the whole hour of embedded timecode — the very
  defect for which t179/t180 were created. Our own loop closes correctly
  (we read what we wrote), but reading by a FOREIGN editor breaks.

**We have no sidecar for authored data.** The only existing sidecar —
`.ktindex.db` (`src/libs/kt-media/src/index.rs`) — is deleted and recreated
on ANY error when opening. For the thumbnail and peak cache this is the right
policy; for a person's remarks it is unacceptable: the VEGAS path (a marker in the file or in
a `.sfl` next to it) cannot be reproduced on our current sidecar.

OFFLINE: DEGENERATES — `src/libs/kt-model/src/project.rs:1845` — there is no carrier, while a foreign one exists: the event has no markers field and not a single command over them, so a remark about a piece of footage degenerates into an edit marker, which at the first re-assembly starts pointing to the wrong place; foreign clip markers meanwhile survive the loop as an opaque passthrough tied to the id — they are not visible, cannot be corrected, and on a split they go entirely to the left half together with those now lying outside its range.

NETWORK COST: a remark about footage must lie where the footage lies and survive a change of edit — cross-project storage. Today everything we know about a file lives either inside a single project or in the cache sidecar, which is deleted at the first opening error, so there is physically nowhere to record "this note belongs to the take, not the film".

### What is missing

- A carrier: markup expressed in the time of the FOOTAGE, not of the timeline.
- Display: a clip marker is drawn on the event itself and must disappear when trimmed beyond the window and return on a reverse trim.
- A rule "a piece inherits the markup": today `clip_slice` is one rule for the whole project (R.39, t269–t271), and markers are not part of it.
- A durable place next to the footage that is not deleted by the cache policy.

### What gets in the way

The first is the one-piece rule. Splitting, trimming, reversing and the morpher layer are computed by ONE piece of code (`clip_slice`, item 16, tests t233–t236, t269–t271); any new quantity belonging to a clip must enter it, otherwise a second rule appears, and that is exactly the class of defect for which the rule was created. A clip marker is a quantity in the coordinates of the footage, and its behaviour under reverse (the left half plays the TAIL of the source) is non-obvious.

The second is invariant 8: scrolling and zooming the timeline must not generate decode requests, and an event's markup is drawn on every UI frame. So clip markers must come from the MODEL (or from a sidecar read once), not be requested from the source — as is already done for waveforms and thumbnails.

The third is the already described breakage of `Retained` on a split: while the right half loses the whole passthrough, adding authored data to it means knowingly putting it into a bucket with a hole. The defect is registered here by its address and is fixed separately, not by this note.

### Which invariants it touches

Invariant 8 — directly: the event's markup is drawn on the hot draw path, and its source may only be the model or an already read sidecar, otherwise scrolling would start generating reads. UI checklist rules: 13 (a clip marker must be drawn in an ALREADY occupied place — on the event itself, not in a new strip) and 20 (nothing grows or switches off by itself: a marker that has gone past the edge of a trim must return on the reverse gesture, not vanish forever).

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: a quantity in the coordinates of the footage inside the one-piece rule; display and editing on the event |
| Where the main work is | kt-model (`ClipEvent` and `clip_slice` — markers as part of ONE piece rule), kt-otio (`ClipExtra.markers` stops being a passthrough and becomes data), kt-app (drawing on the event, hiding behind the window) |
| Project format | does not touch the foreign format (the place in the schema already exists — `Item.markers`); our own clip markers bump the version, since an old reader would silently lose them |
| Tests to write | "a split divides markers at the boundary, rather than giving them all to the left half" — next to t233/t234; "trimming hides, does not delete" (the reverse gesture brings them back); "a clip marker survives a reverse on its own frame" next to t26; a `.otio` round-trip with foreign clip markers after a split (today it would fail) |
| Depends on | F-COL-020 (text on the marker), F-MED-005 (relinking: markup must survive a change of the footage path) |
| Breaks compatibility | no |

## Verdict

Take the carrier, do not take the shared storage. A marker in the coordinates of the footage is useful to one person just as it is to two: it survives its own re-assembly — and the editor re-assembles themselves more often than the second participant does it for them. The shared carrier of the VEGAS level (writing into the file or into a `.sfl` next to it) we do not take: it requires storage that outlives projects, and our single sidecar is deleted at the first opening error — and there is no participant for whose sake a second one would be worth creating.

Separately and before the feature: **a split loses the passthrough of the right half** — `retained.clips` is keyed by `ClipId`, the right half gets `default()` and leaves without a name, foreign effects and `available_range`, that is, without the footage timecode. This is a defect of today's code, not a missing capability; it is registered here by its address (`src/libs/kt-otio/src/convert.rs`, `src/libs/kt-model/src/model.rs`, `Command::Split`) and must be fixed by a regression test next to t180, regardless of the fate of this note.

Boundaries: edit markup and its address — F-COL-020; who said it and when — F-COL-022; how many notes fit on a frame — F-COL-021. The difference from F-UI-005: there an event has COLOUR data, a quantity without time, and the question is who owns the paint (source or clip); here the quantity is time, and the question is in which coordinate system this time is counted. The difference from F-TL-007: its verdict "we do not create a second entity" we continue — a clip marker is not a new object but the same marker on another owner, exactly as in the OTIO schema, where `markers` belong to `Item`.

## Sources

- https://larryjordan.com/articles/premiere-pro-timeline-marker-secrets/ —
  ripple edits move sequence markers; "clip markers always travel with a
  clip", 06.09.2020 (accessed 2026-09-08)
- https://help.magix-hub.com/video/vegas/22/en/content/topics/7-edit/media_markers.htm
  — media markers: shown in all events referencing the file; "Media markers
  and regions cannot be edited within the event" (accessed 2026-09-08); the VEGAS
  row is marked `[NOT VERIFIED]` for 2026.0 — the help is published for line 22
- https://help.magix-hub.com/video/vegas/22/en/content/topics/2-window/window_trimmer.htm
  — saving Trimmer markers to a file or to a `.sfl` sidecar (accessed
  2026-09-08)
- https://support.apple.com/guide/final-cut-pro/intro-to-markers-ver397279dd/mac
  — "You work with markers in the same way for clips in the browser and clips in
  the timeline" (accessed 2026-09-08)
- https://support.frame.io/en/articles/2444494-take-the-fcp-tour-legacy —
  "Break apart the compound clip so the markers will stay attached to the clips"
  (accessed 2026-09-08)
- https://otrn.editingtools.io/ — OTRN v1 (November 2025): the `files` section for
  notes "outside the sequence range" (accessed 2026-09-08). No key was added to
  `_editors.md`: this is a format, not an editor
- https://en.editingtools.io/qcmarker/ — "This tool lets you move quality control
  notes related to files and source timecode to the right place in the final
  editing sequence": a separate tool exists precisely because notes about the
  material and edit markup live in different coordinates (accessed
  2026-09-08)
- Semantics of `Item.markers` — the reference OpenTimelineIO 0.18.x library
  (`docs/features/_editors.md`, `otio-lib-0.18`) and our analysis in
  `src/libs/kt-otio/CLAUDE.md`
- Our code checked by grep and reading in `src/` on 08.09.2026:
  `kt-model/src/project.rs` (`ClipEvent`, `Project.markers`),
  `kt-model/src/model.rs` (`Command::Split`, `clip_slice`),
  `kt-otio/src/convert.rs` (`ClipExtra.markers`, `retained.clips.get(&c.id)`,
  `media_origin`), `kt-media/src/index.rs` (the sidecar deletion policy),
  `kt-app/src/*.rs` (not a single reference to clip markers)

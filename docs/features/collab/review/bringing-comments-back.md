---
id: F-COL-026
title: Returning review comments to the edit
status: candidate
priority: B
cost: M
category: collab/review
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: partial
crates: [kt-model, kt-otio, kt-app]
invariants: []
ui_rules: [13, 16]
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [frame-io, bm-cloud]
checked: 2026-09-08
---

# F-COL-026 — Returning review comments to the edit

## Essence

The comments have been collected, and now they must be BROUGHT IN. The editor wants to see them not as a list in an email but as markers on their own timeline: stand on a marker — and immediately see what it is about. The return path looks simple (a dozen lines with a time and a text), but it breaks not on transport but on correspondence: a remark is addressed to the time of the VIDEO, and the edit has since moved. And where the path has nevertheless been laid, a second trouble turns up: what arrives from outside writes into the document past all the rules of the document.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | nothing supports the return: comments arrive by email, markers are placed by hand — **[NOT VERIFIED]** against the 2026 product help |
| Adobe Premiere | 26.x | review panel (Frame.io V4); rebuilt entirely in 25.6 | remarks arrive in the panel and become sequence markers. Before the rebuild they hung on the version's timestamp, the panel overwrote markers on sync, and in V4, unlike V3, there was no manual "sync" button at all |
| DaVinci Resolve | 21 | Presentations, two-way relinking | the only one that promises a return in BOTH directions: "shared markers and comments are automatically and bidirectionally relinked with DaVinci Resolve" — a comment from there becomes a marker, a marker from here becomes a comment |
| Final Cut Pro | 12.3 | no native return | comments are carried over by hand or by a third-party workflow; the library is handled whole, so "accept the remarks" and "edit" are mutually exclusive activities — **[NOT VERIFIED]** |
| After Effects | 26.x | the same review panel | comments are visible in the panel; threads of the kind "imported frame.io comments not visible" are known in the community — that is, the path exists and it is fragile |

## The trick behind it

**1. What breaks is CORRESPONDENCE, not transfer.** A remark is pinned to the time of a flat video, and the timeline has since been reassembled: one insertion at the start — and all addresses to the right have moved, although they look correct. The industry has found exactly one working answer, and it is counterintuitive: the address is translated ONCE, at the moment of acceptance, after which the remark lives not by time but by a MARKER and moves together with the edit. Adobe's wording on the rebuild of the panel: comments are now tied to sequence markers, not to the original timestamp, "so you can edit your sequence without losing track of what part of your sequence a comment was about."

**2. The translation has a price, and nobody pays it explicitly.** As soon as the address has been translated into a marker, the original address (the time in video v3) is lost: there is no longer any way to answer the question "where was this in the version the client watched." None of the five holds two addresses at once — keeping both would mean remembering which revision each video was made from.

**3. The return path turned out NOT to be additive but mirroring — and this is the worst possible choice.** The panel does not add remarks to what exists; it brings the state of the markers into correspondence with its own side, and when its side has fallen behind, it rolls back the editor's work. The user description reads literally: the markers on the timeline revert to the state of the PREVIOUS version of the edit, and the only workaround found is to close the review window altogether.

**4. A general law, for which this note was opened: an external edit that does not go through the document's commands kills undo.** A workaround plugin that appends markers past the editor breaks CTRL+Z — and this is not a peculiarity of the plugin but an inevitability: the edit history is kept by whoever edits, and everything that edits around it does not exist for the history. We have exactly the same construction, and it was recorded long before this note.

## Pros

- A remark in the form of a marker on the timeline answers the question "where" without translation: the editor does not count time in their head and does not miss by a frame.
- Translating the address once, at the moment of acceptance, makes the remark stable under all further editing — this is the only solution the industry knows.
- Two-way relinking (Resolve) closes the reverse direction too: the editor's reply is visible to the reviewer right where they asked.
- A list of remarks next to the edit replaces the email: "done / not done" is checked against the document, not against the correspondence.

## Cons

- A mirror-style sync overwrites its own work, and there is nothing to restore the overwritten markers from: they were not a user edit, so even undo will not bring them back.
- An external writer that bypasses the document's commands breaks undo entirely — the cost of one line of integration equals the cost of the whole edit history.
- After the address is translated, the original time of the video is lost: the dispute "we were talking about a different frame" is unresolvable.
- The text of a remark in the panel is sometimes unavailable even for copying — that is, the comment is visible but not portable.
- A marker carries only a name: it has no author, no status and no time, so "done" is marked by an annotation inside the name itself.

## What users say

About mirroring instead of appending — with a date and verbatim:

> Frame.IO V4 is overwriting & deleting comments markers on my sequences in
> Premiere Pro.
> — Andrew C. Brinsmead, Adobe Community, 28.10.2025,
> https://community.adobe.com/bug-reports-728/frame-io-v4-keeps-overwriting-comments-markers-in-premiere-pro-pro-1332248

In the same thread — how exactly this looks and how people cope:

> Comments in my timeline will revert to match the previous version of the edit
> on Frame.IO.
> — the same author, the same day; the only workaround he found is to close the
> review window in Premiere altogether

## How it fits Kinotrain

### What already exists

Half of the mechanism, the part responsible for DISPLAYING a remark, is done by us, and done reasonably well:

- A marker is `t`, `duration`, `name`, `color` (`src/libs/kt-model/src/project.rs`); it can be a point or a range; a range marker has a band on the time-label row, and its name is written right there, without a separate lane (rule 13 of the checklist).
- The name is entered in place and goes into the model as a command: `Command::SetMarkerName` through the `KtApp::apply` wrapper (`src/apps/kt-app/src/timeline.rs`); a new marker with a name is the batch `[AddMarker, SetMarkerName]`, that is, ONE undo step.
- The marker already takes part in output: the area's name gets into the name of the rendered file if the region coincides with a range marker (`default_render_name`, `src/apps/kt-app/src/render_dialog.rs`).
- The discipline of edits is recorded: all model edits from the UI go through `apply`, `apply_batch`, `apply_batch_restoring`, `apply_coalesced` (`src/apps/kt-app/CLAUDE.md`, item 200), and undo is snapshots of the whole `Project` (`src/libs/kt-model/src/model.rs`).

And right at this same place three holes are visible:

- **A marker is addressed by a TICK, its identity is the time:** `MarkerExists(Tick)` does not allow placing a second one on the same frame (rule 16 of the checklist — an impossibility is shown as an error), and when reading someone else's file the extra ones are silently collapsed by `dedup_by_key`. Two remarks on one frame cannot exist.
- **No edit of the montage moves a marker.** `plan_move`/`plan_move_block` know about clips and obstacles and know nothing about markers. Insert a piece — the markers stay on their ticks.
- **A field for a remark exists in the format we already write, and we discard it.** The OTIO schema `Marker.2` has a `comment` (`src/libs/kt-otio/src/schema.rs`), but the reader drops it, and the writer writes it empty (`src/libs/kt-otio/src/convert.rs`). A foreign `.otio` with comments loses them on the first save — this is a defect, not a decision.

OFFLINE: DEGENERATES — `src/apps/kt-app/src/timeline.rs:4241` — the return of a remark degenerates into a marker name typed by hand: the marker is drawn on its own tick and holds onto it dead; no edit of the montage moves it — therefore already on ONE machine one's own note "fix it here," made yesterday, after the first insertion points to the neighboring shot while looking correct. Staleness is marked by nothing, the marker has neither author nor time, and two remarks on one frame do not occur at all.

NETWORK COST: moving a remark from the time of the old version to the time of the current edit — the edit log — our undo is snapshots of the whole project, so between two videos there is not a single NAMED edit by which the time could be recalculated; without such a log only the person who remembers what they moved can transfer the address.

### What is missing

- Content for the marker: the text of the remark (the `comment` field already exists in the format), author, time and a state of "addressed."
- Two remarks on one frame: identity by tick forbids this by construction.
- A link of the remark to the video version on which it was written (F-COL-024) — without it "on which version was this said" cannot be answered.
- A sign of staleness: today a marker left over from a previous edit is not distinguishable in any way from one just placed.

### What blocks it

The first obstacle is **the identity of a marker by time.** This is not a detail of the model: commands rest on it (`MoveMarker` refuses to move onto an occupied tick), the `MarkerExists` guard, the collapse on reading someone else's file, and `pick_range_marker` in the interface. A remark, by its nature, is an object with its OWN address, which may share a frame with its neighbor. This is a fork of the model, not a field.

The second — **the return path must go through commands, and this is not a wish but an already recorded rule.** Everything that arrives from outside (importing a list of remarks, converting someone else's markers) must become a batch of commands, otherwise `model_rev` will not move, the undo snapshot will not be taken, and the autosave gate will go out — the symptom is known verbatim: edits "looked saved and were lost on exit." That is, someone else's trouble "the plugin breaks CTRL+Z" is reproduced with us one for one, and the only protection against it is that same wrapper.

The third — **mirroring versus appending.** Acceptance of remarks must be appending: bringing the state of the markers "as it is on the other side" is impossible for us to undo meaningfully, because the whole project is undone, not an individual marker.

### Which invariants it touches

None of the twelve — and this is a strong argument for "cheap": a remark touches neither the time model, nor determinism, nor the cache, nor the composition. It touches two rules of the UI checklist: 13 (the text of a remark must fit into an already occupied place — the band and the time-label row, not a new lane) and 16 (an impossibility is shown as an error — including "a marker can no longer be placed here").

## Estimate

| What | Answer |
|---|---|
| Size | M — 2–3 stages: content on a marker (text, author, state) and its own identity; acceptance of a list of remarks as a batch of commands |
| Where the main work is | kt-model (the identity of a marker and its fields), kt-otio (`comment` stops being lost in both directions), kt-app (display, acceptance, the "addressed" mark) |
| Project format | a version is needed: the marker gets fields beyond `Marker.2`; a foreign `comment` at the same time finally begins to survive the round trip instead of being lost |
| Tests that will have to be written | "a foreign `.otio` with marker comments survives the round trip" (fails today — the field is dropped on reading); "two remarks on one frame"; "acceptance of a list of remarks is ONE undo step"; "a marker left over from a previous edit is marked as stale" |
| Depends on | F-COL-024 (the version on which the remark was written), F-COL-011 (the address at which it arrives) |
| Breaks compatibility | no: the new fields are optional, and the foreign part of the file still comes back verbatim |

## Verdict

We take the local half, and it is larger than it seems. Exactly the same problem of correspondence exists for ONE person: a marker placed yesterday points to the wrong place after the first insertion and does not report it. So the content of a marker, its own identity and the sign of staleness are needed by us outside any review — and when a second participant appears, the return path will already be laid. We do not introduce the server half: there is no participant.

Separately — a conclusion that this note must leave in the base as a GENERAL one, not as a peculiarity of Frame.io: **an external edit that does not pass through the model's commands kills undo.** Any future integration — importing remarks, synchronization, a plugin — must be a batch of commands through `KtApp::apply*` and must APPEND, not bring into correspondence. If we violate this, we get exactly what neighbors complain about: silently rolled-back work and a dead CTRL+Z.

And one finding about ourselves, suitable for a separate task: the `comment` field of a marker is in the schema we already read and write, and we discard it on reading and write it empty. A foreign document with remarks loses them with us on the first save — even though an UNKNOWN marker schema survives the round trip whole via passthrough. Our native schema is protected worse than a foreign one.

Boundaries: the version of the video to which the remark is pinned — F-COL-024; how a remark is addressed in someone else's player — F-COL-025; the timecode as a common address — F-COL-011; a marker as data on the event (color as a sign) — F-UI-005; the edit history and snapshots — F-PRJ-002.

## Sources

- https://community.adobe.com/bug-reports-728/frame-io-v4-keeps-overwriting-comments-markers-in-premiere-pro-pro-1332248
  — overwriting and deleting of comment markers, rollback to the state of the previous version, the "close the window" workaround (access date 2026-09-08)
- https://community.adobe.com/t5/premiere-pro-discussions/pr-25-frame-io-v4-export-timeline-markers-as-comments/m-p/15649143
  — a thread on exporting timeline markers as comments and on the rebuild of the panel in 25.6: remarks are tied to sequence markers, not to the original timestamp (access date 2026-09-08)
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration —
  two-way relinking of markers and comments in Presentations (access date 2026-09-08)
- The rows for VEGAS and Final Cut were compiled from the composition of their interfaces; they were not checked live, the `[NOT VERIFIED]` marks stand in the table itself
- Our code checked by reading on 08.09.2026: `src/apps/kt-app/src/timeline.rs`
  (drawing of markers, inline name input, `pick_range_marker`, `plan_move_block`),
  `src/libs/kt-model/src/project.rs` (`Marker`),
  `src/libs/kt-model/src/model.rs` (marker commands, `MarkerExists`, undo snapshots), `src/libs/kt-otio/src/schema.rs` and `convert.rs` (the `comment` field),
  `src/apps/kt-app/CLAUDE.md` (item 200 — all edits through wrappers)

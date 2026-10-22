---
id: F-COL-031
title: Comparing two edits
status: candidate
priority: B
cost: M
category: collab/merge
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, otio-lib-0.18]
we_have: partial
crates: [kt-cli, kt-otio, kt-model, kt-app]
invariants: []
ui_rules: []
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [bm-cloud, resolve-project-server, team-projects]
checked: 2026-09-08
---

# F-COL-031 — Comparing two edits

## Essence

There are two versions of one edit: yesterday's and today's, one's own and the one received, before the director's notes and after. The question is simple and constant — what has changed. Not "how do the files differ", but what happened to the EDIT: where frames were added, where they were cut, where they were moved, where they were trimmed. The tool shows this on two tracks one under the other, colouring the differences; the person carries the decision across by hand.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | there is no way to compare two `.veg` files; the practical answer is to open two instances of the program and look with one's eyes [NOT VERIFIED live] |
| Adobe Premiere | 26.x | none | in Team Projects there is a history of project versions and a rollback to a version — this is a choice of a whole state, not a display of differences. A diff of two sequences is not offered |
| DaVinci Resolve | 21 | Timeline Comparison | the only one in the industry. Two versions of a timeline are compared, and the result is expressed in FOUR verbs of editing: where frames were added, deleted, moved, trimmed. This is a review, not a merge: the carrying-over is done by a human |
| Final Cut Pro | 12.3 | none | inside a library there are no versions as objects; they compare by copies of projects in the browser, by eye |
| After Effects | 26.x | none | it has no model of editing of its own, so there is nothing to compare [NOT VERIFIED live] |

**Outside the core.** Four core rows say "none", and one can explain why the only existing tool compares precisely FRAMES only from the design of the exchange format, not from the editor's interface.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| OpenTimelineIO | 0.18.x | `Clip` schema without identity | an event in the schema has a `name`, a `source_range`, references to media, effects and `metadata` — and there is NO identifier. The position on a track is expressed not by a coordinate but by the duration of the neighbouring `Gap`. So the only address shared by two documents is the frame of the media, and any comparison must be built on it |

## The trick behind it

**Resolve compares not structure but FRAMES — and therefore its diff is defined on the verbs of editing, not on a tree of objects.** The vendor's wording names exactly four outcomes: added, deleted, moved, trimmed. This is not a list of convenient highlights; it is a complete enumeration of what can happen at all to the presence of a frame of media in the film: the frame was absent — it appeared; it was present — it disappeared; it remained, but at another place in the output; it remained in place, but the boundaries of the piece are different. An object tree does not give such an enumeration: two versions of a timeline lie in two database records and have no shared identifiers, so there is nothing to match there. The frame of the media is the only address that both sides have and that is not spoiled by renaming a track, changing the order of tracks, or rearranging effects.

**The reverse side of the same choice: a diff by frames is blind to everything that is not the presence of a frame.** A change of level, a replacement of an effect, an edit of a curve, a different kind of transition — none of this moves a single frame, and therefore is not shown in the comparison at all. The tool answers the question "what moved in the film" and does not answer the question "what changed in the project", and this is not visible from its name.

**Third: the comparison is the only part of collaborative work that does not need a second party.** It works on two files in one folder, and precisely because of this it appeared with the one who sells collaborative work — as a REPLACEMENT for a merge, not as a part of it. Showing a difference is cheap and determinable; merging is not (F-COL-029).

## Pros

- The answer is given in the terms in which a person thinks about editing; one does not need to learn how to read a diff.
- Stable under renaming, under a change of track order, and under rearrangement of effects: the address is the frame of the media, not the name of an object.
- Requires neither a shared base nor identity of objects between documents — two files are enough.
- Display instead of automation: the decision remains with the person, and therefore the tool cannot "fix up" on their behalf (item 20 of the checklist).

## Cons

- It sees nothing but the presence and the boundaries of a frame: an edit of an effect, a level, a curve, or a transition type is not shown.
- Shared media is required: if two versions have different sources (re-encoded, renamed, or the proxies replaced), frames cease to be a common address.
- The result is a picture for a person, not an operation: the change still has to be carried over by hand, and on a long edit this is expensive.
- It exists only in the paid version of one editor; with the other four the task is solved with two open windows.

## What users say

The vendor's wording — it is also the most precise description of the mechanism, because it names the four verbs directly:

> The revolutionary timeline comparison tool lets you visually see the
> differences between two versions of a timeline. You'll be able to see exactly
> where footage has been added, deleted, moved or trimmed.
> — Blackmagic Design, Collaboration page of DaVinci Resolve,
> https://www.blackmagicdesign.com/products/davinciresolve/collaboration
> (access date 2026-09-08)

A living review from a practitioner about the comparison tool itself could not be found, and this is not a saving of search budget. The reason is technical and is named in the source registry: the Blackmagic forum thread can only be read through search snippets (Cloudflare), so no verbatim quotes with dates can be retrieved from there, and Resolve has no public tracker; on the available sites (`liftgammagain.com`, `creativecow.net`) the talk is about locking bins and about project databases, not about comparing timelines. The absence of reviews is itself telling: the tool is called "revolutionary" on the product page and is almost never discussed by practitioners.

## How it fits Kinotrain

### What already exists

- `src/apps/kt-cli/src/main.rs:233` — `roundtrip` already prints `byte-stable`: it compares the written document with the one read back byte for byte. Verified on `kt-cli demo` (16 clips, 8 media): `byte-stable: true`.
- `src/libs/kt-otio/tests/format.rs:141` — t144: the writer is typed and IDEMPOTENT, `write(parse(write(x))) == write(x)`, the indentation is fixed, the file ends with a newline. That is, we have what the industry does not: two identical projects give identical bytes, so a text comparison is meaningful.
- `src/libs/kt-otio/src/convert.rs:483` — an event is written with our identifier (`metadata.kt.id`), which is absent from the OTIO schema (`src/libs/kt-otio/src/schema.rs:220`). Formally, an address for a structural comparison already lies in the file.
- `src/libs/kt-model/src/warp.rs` — the stretch map: the single rule for translating "timeline tick → consumed media", and it lives in the model precisely so that `clip_slice` can call it. Without it the verdict "trimmed" on a stretched event is not computed: `source_in` there is not a frame address.
- `src/libs/kt-model/src/project.rs` — `lane_order`, `areas`, `markers`: everything with which a person marks up an edit is already in the model and in the file, so there is something to show a diff on.

### What is lacking

- The verb itself. `kt-cli` has seven commands (`version`, `new`, `convert`, `demo`, `roundtrip`, `render`, `export`) — `diff` is not among them, and it is not in the interface either.
- Separation of the document from the view. The whole `UiState` — the left edge, the zoom and the POSITION OF THE PLAYHEAD — is written to the same file under `kt.ui` (`src/libs/kt-model/src/project.rs:1571`, the writer is `convert.rs:483`). Measured on `kt-cli demo`: the file is 1365 lines, 57 040 bytes; a shift of the cursor by one tick gives exactly one line of difference — and a text comparison does not distinguish it from an edit of the montage. Byte stability without such a separation gives "the files differ", not "the edit has changed".
- Identity between documents. `metadata.kt.id` is an address within ONE file: `next_clip_id` issues "maximum + 1" (`src/apps/kt-app/src/app.rs:3110`) and reuses the number after deletion. A comparison of two diverged files by this field would silently pair two different clips — worse than the absence of an address.
- Position as a quantity. In the file the position of an event is expressed by the duration of the neighbouring `Gap`, so "moved" and "trimmed" are indistinguishable in text: both look like an edit of two numbers next to each other.

### What stands in the way

Nothing costly, and this is a rare case in the category. One thing stands in the way — the habit of considering a byte comparison to be a comparison of an edit. A real diff must be computed on the PARSED model (two `Project`s, matching by media and frame, four verbs at the output), not on text; `UiState` is excluded from the comparison, and on stretched events the verdict "trimmed" must go through the warp map — otherwise the defect recurs for which the arithmetic over `source_in` was reduced to a single rule (t273).

### In the `collab/` category — two mandatory lines (rule 13)

OFFLINE: REMAINS — `src/apps/kt-cli/src/main.rs:233` — this is the only part of collaborative work that is wholly local and half already written: two edits are compared as two files in one folder, the writer is idempotent and verified by t144, and the program itself already answers the question of whether the bytes match, printing `byte-stable` at the end of the round trip; what is missing is not a second side, but the translation of the answer from bytes into the four verbs of editing.

NETWORK COST: to compare against a version you do not have on disk — project-as-database — in Resolve both sides of a comparison are always at hand, because they are two records of one project database, and the choice of "what to compare against" there equals choosing a row in a list; for us the second version is a second file that someone has to bring, and while a person brings it, the comparison works but a list of versions does not exist.

### Which invariants it touches

Of the twelve — none: the comparison does not decode, does not count frames, does not touch the sound, the cache, or export. It relies not on an invariant but on a property of the format — the idempotence of the write (t144) and the absence of milliseconds in the file. Of the checklist rules in `src/apps/kt-app/CLAUDE.md`, it touches item 20: the comparison must SHOW, not "fix up" — as soon as it starts carrying changes over by itself, it becomes a merge, which no one has (F-COL-029).

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2 stages: a comparison of two parsed projects in four verbs and its display on two lanes |
| Where the main work is | kt-model (matching of events by media and frame), kt-cli (the verb `diff` — a cheap first surface), kt-app (display) |
| Project format | not touched; but the separation of document and view will be requested — `kt.ui` next to it, not inside it |
| Tests to write | movement of the playhead and zoom are not counted as an edit change (today they are counted); a shift of an event gives "moved", not "deleted + added"; on a stretched event "trimmed" is computed through the warp map; two independently edited files are not matched by `kt.id` |
| Depends on | nothing; F-PRJ-002 (versions) gives the second side of the comparison, but is not needed for the first |
| Breaks compatibility | no |

## Verdict

We take it, and we take it whole: this is the only part of collaborative work that has no server half at all. Two files in one folder are enough, and half the work is already done — the writer is idempotent (t144), `byte-stable` is printed, events carry `kt.id`, the warp map can translate a tick into a frame of media.

Honestly about what is lacking, so as not to present what exists as finished. A byte comparison is NOT a comparison of an edit, for two reasons at once. First: in the file next to the document lies the view — `kt.ui` with the playhead position and zoom, and a shift of the cursor by one tick gives one line of difference in a file of 1365 lines, indistinguishable for `diff` from an edit of the montage. Second: `kt.id` is an address inside a single file, not the name of a thing, because `next_clip_id` is "maximum + 1" with reuse of numbers; matching two diverged files by it would pair different clips and be wrong more confidently than the absence of a comparison. A real diff is computed on the parsed model, by media and frame, with a verdict of four verbs — exactly as it was done by the one who did it.

Boundaries: why the display has never turned into a merge at anyone — F-COL-029; the empty conflict-resolution window — F-COL-030; what to do when versions have already overwritten each other — F-COL-032; versions and snapshots of one's own document — F-PRJ-002; portability of a project between machines — F-PRJ-003.

## Sources

- https://www.blackmagicdesign.com/products/davinciresolve/collaboration — "timeline comparison tool" and the four verbs (access date 2026-09-08).
- The OTIO schema and the absence of an identifier in `Clip` — checked offline against our own mirror of the schema `src/libs/kt-otio/src/schema.rs` (2026-09-08).
- The VEGAS and After Effects rows are marked `[NOT VERIFIED]` directly in the table; the absence of live reviews and its reason are named in the section "What users say" (availability of the sites — `docs/features/_services.md`).
- The measurement was made on 08.09.2026 on this machine: `kt-cli demo demo.otio` → `kt-cli roundtrip demo.otio` prints `byte-stable: true`; the file is 1365 lines, 57 040 bytes; a change of `kt.ui.cursor` gives one line of difference.
- Our code read on 08.09.2026: `src/apps/kt-cli/src/main.rs` (233), `src/libs/kt-otio/src/convert.rs` (483), `src/libs/kt-otio/src/schema.rs` (220), `src/libs/kt-otio/tests/format.rs` (141), `src/libs/kt-model/src/project.rs` (1571), `src/libs/kt-model/src/warp.rs`, `src/apps/kt-app/src/app.rs` (3110).

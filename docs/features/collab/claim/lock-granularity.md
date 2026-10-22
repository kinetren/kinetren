---
id: F-COL-016
title: Unit of occupancy — library, bin or sequence
status: researched
priority: B
cost: L
category: collab/claim
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-model, kt-otio, kt-app]
invariants: []
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-PRJ-001]
trackers: []
services: [team-projects, bm-cloud, resolve-project-server, fcp-shared-library, premiere-productions]
checked: 2026-09-08
---

# F-COL-016 — Unit of occupancy — library, bin or sequence

## Essence

Two people have taken up the same edit. Nobody in the industry merges their edits — everyone
prevents them from diverging: what the first one has taken is shown to the second one only for
reading. Therefore the only substantive question of shared work sounds not "is there a lock" but
"WHAT EXACTLY does it occupy". Final Cut occupies the whole library — with all the material and all
the edits; Resolve occupies a separate bin and a separate timeline; Team Projects — the sequence;
Productions — the project file. The size of this unit determines whether two people can work at the
same time at all or only in turns.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | there is no shared work in any form: the project is one `.veg` file and exactly one timeline, and "occupied" is expressed by the file physically lying with one person |
| Adobe Premiere | 26.x | Project locking (Productions) and Sequence locking (Team Projects) | TWO different mechanisms in one program, and they differ by exactly one unit: in Productions the PROJECT FILE is occupied, and the lock is declared by a separate `.prlock` file next to it; in Team Projects the SEQUENCE is occupied, and for the second person it is "view only" until the first one publishes |
| DaVinci Resolve | 21 | Automatic bin and timeline locking | the unit is a DATABASE RECORD: bins and timelines lie in the project library (PostgreSQL) as separate records and are occupied separately, "read only until unlocked by the current user"; two people work in one project while they sit in different bins |
| Final Cut Pro | 12.3 | shared library on a shared volume | the WHOLE library is occupied at once, because it is itself the unit of recording: the lock does not distinguish the events and projects inside it, the second person gets "already in use by …", and the only path is one — to wait |
| After Effects | 26.x | has none of its own; through Team Projects — the same sequence/composition | AE itself has no occupation at all: the project is a file, and it is shared by copying. In Team Projects AE acts as a client of someone else's service and inherits its unit entirely |

## The trick behind it

**The granularity of the lock is ALWAYS equal to the unit of RECORDING, not to the unit of an edit,
and therefore it cannot be chosen — it is inherited from how the program saves.** Final Cut writes the
library as a whole, and therefore occupies it as a whole: inside the library there is nothing that can
be written separately, so there is nothing that can be occupied separately. Resolve writes bins and
timelines as separate database records — and occupies them separately, without inventing anything for
this. Productions is the same law read backwards: Adobe cut one large project into many files, and the
unit of occupation immediately became the file, and the lock became a file next to it. Nowhere is the
"unit of occupation" a separate decision; it is the shadow of the way of recording.

**From this follows a second thing, absent from every feature list: a clip is locked by NOBODY,
although people actually edit clips.** The unit of conversation between two editors ("don't touch the
first five minutes", "V2 is mine") and the unit of occupation coincide for none of the five and cannot
coincide until the recording becomes just as fine-grained. Everything finer than the unit of recording
lives in an oral agreement, in the name of a track (F-COL-012), or in Resolve's built-in chat — that is,
outside the mechanism.

**Third: a large unit has a price, paid in the structure of the document.** So that two people could work
in Premiere at the same time, the edit has to be cut in advance into Productions project files — not
because it is more convenient to edit that way, but because otherwise the lock would capture everything.
The organization of the document is dictated by the lock, and this decision is made BEFORE the first frame.

## Pros

- Pessimistic occupation removes the task of merging altogether: "do not let them diverge" is cheaper
  than "reconcile what has diverged". The absence of automatic merge at all five is not a gap but a
  consequence of this choice.
- A unit inherited from recording is honest by construction: what is occupied is exactly what will be
  overwritten as a whole on saving. "Occupied and did not lose someone else's" becomes a property of the
  construction, not of carefulness.
- A small unit pays off at once and without a server: at Resolve two people sit in one project while they
  work in different bins, and no queue arises at all.
- The state of occupation can be read BEFORE opening: in the Productions panel there is a "Project Locked"
  column with the name of the one who occupied it, so the lock is also an answer to the question
  "who has it now".

## Cons

- A large unit turns shared work into a queue: the Final Cut library is occupied whole, so "two people in one
  project" at Apple means "one at a time".
- The unit of occupation imposes the structure of work: cutting an edit into files is done for the sake of the
  lock, not for the sake of editing, and cannot be glued back together.
- What people really agree on is occupied by nothing: a clip, a piece of a timeline, a track. The agreement is
  held by attentiveness, and the program does not notice its violation.
- A lock inherited from recording inherits its failures too: the `.prlock` file remains after a crash, and the
  project turns out to be occupied by a person who is no longer there.
- Two mechanisms in one program (Productions and Team Projects) mean two different units for one user: what in
  one mode is "my sequence" is in the other "the whole project", and the habit cannot be carried between them.

## What users say

The vendor formulates the unit directly — at Resolve it is the pair "bin and timeline":

> Automatic bin and timeline locking let multiple people work without overwriting
> each others work.
> — Blackmagic Design, page «DaVinci Resolve — Collaboration»,
> https://www.blackmagicdesign.com/products/davinciresolve/collaboration
> (accessed 2026-09-08)

> Bins and timelines are "read only" until unlocked by the current user.
> — ibid. (accessed 2026-09-08)

At Adobe the unit is named just as unambiguously in the reference thread — it is the PROJECT, and the second
person gets read-only mode:

> Project locking helps multiple editors in a workgroup environment who are working
> together on a single Premiere Pro project by warning them when a project is
> currently in use by another user.
> — Rameez_Khan, «FAQ: What is project locking, and how do I use it?»,
> 18.10.2017,
> https://community.adobe.com/t5/premiere-pro/faq-what-is-project-locking-and-how-do-i-use-it/td-p/9431268

## How it fits Kinotrain

### What already exists

- We have exactly one document, indivisible: `TimelineModel`
  (`src/libs/kt-model/src/model.rs:343`) holds a single field `project: Project`, and
  everything else is the history of this same object. Neither a bin as a record, nor a second
  sequence, nor any sub-document exists in the model (F-PRJ-001, `we_have: no`).
- The unit of RECORDING is the whole document: `kt_otio::save_project` serializes the project
  entirely, and `write_project_atomic` (`src/apps/kt-app/src/app.rs:5051`) puts the result into a temporary
  file and replaces the target with it via `rename`. A corrected volume of one clip rewrites the file in exactly
  the same way as a re-edit.
- The unit of an EDIT with us, on the contrary, is small and well delineated: a gesture — `apply_batch`
  (`src/libs/kt-model/src/model.rs:376`), one undo snapshot per gesture, atomicity of the whole series. That is,
  the gap between "what is edited" and "what is written" with us is the greatest possible: a command over one clip
  — against the whole file.
- The only word `locked` in all the product code is the checkbox of a media bin card group
  (`src/libs/kt-model/src/project.rs:1647`), and it protects the layout from one's own slip, not the document from
  a second person.
- The save guard knows exactly one writer: `write_project_guarded`
  (`src/apps/kt-app/src/app.rs:5036`) compares the model revision in MEMORY and protects us from our own background
  autosave, not from a foreign process; there is no read of mtime and no file lock anywhere in `kt-app`.

OFFLINE: DOES NOT REMAIN — `src/libs/kt-model/src/ops.rs:81` — there is nothing to occupy and no one to occupy it, and the place where this is most visible is here: copying ONE selection of clips drags along morphers, areas and markers, that is, objects of the level of the whole document. We have no unit finer than the document even for copy-paste, so the question "what to occupy" is today closed by the fact that the person is alone at the file.

NETWORK COST: to occupy PART of a document without occupying the whole — project-as-database — separately they occupy where they write separately: at Resolve the bin and the timeline are separate PostgreSQL records, while with us the only record is the whole file, and it does not become finer with any number of participants.

NETWORK COST: to learn what is occupied BEFORE the first edit — access rights — the lock needs an owner with a name that outlives its process, and we have neither names of people nor the very idea of "occupied": identifiers of clips and media are issued as "maximum + 1" and coincide for two people.

### What is missing

- A named boundary "document ↔ edit". Today `Project` contains a mix of material and bin (belonging to the document),
  clips, tracks, areas, markers (belonging to the edit), and the region with the viewport (belonging to the session).
  As long as the boundary is not named, any future unit of occupation equals the file.
- Records finer than the file. Even if we divide the model, we write one `.otio`; to occupy the edit separately from
  the material, at least one of them must be SAVED separately.

### What gets in the way

Not the volume of code, but the direction of dependency: the granularity of occupation is derived from the record format,
and changing it separately makes no sense. Our format is one OpenTimelineIO document, and this is a deliberate choice
(F-COL-007: handing over an edit means copying the file; passthrough keeps someone else's content verbatim). Splitting the
document into records would cost exactly what works best for us now — portability as one file. The second obstacle is smaller
but more stubborn: `model.project()` in kt-app is called hundreds of times, and each call treats the document as a single object.

### Which invariants it touches

None of the twelve: occupation calculates and draws nothing. It touches the decision F-PRJ-001 — "at the nearest change of format,
name in the model the boundary of the document and the edit" — and adds to it an argument that was not there: this same boundary
becomes the unit of occupation if a second participant ever appears.

## Estimate

| What | Answer |
|---|---|
| Size | L — a whole round, and even then only for the local half (the boundary document ↔ edit); occupation itself does not begin without a second participant |
| Where the main work is | kt-model (splitting `Project` into document and edit), kt-otio (format version and reading old files), kt-app (133 calls of `model.project()`) |
| Project format | a new version of `kt.v` is needed if the boundary becomes structural; reading old files is not affected |
| Tests to write | round-trip of the split model; "an edit change does not touch the document section"; saving old files without loss of fields |
| Depends on | F-PRJ-001 (the same boundary, but from the side of the document structure) |
| Breaks compatibility | no, as long as the split is expressed inside a single file |

## Verdict

We do not set up occupation — declaring a second participant is not within the note's rights, and a lock without a second
participant is a "do not touch" checkbox, which we already have for the bin group. We take one conclusion from the reconnaissance
and record it where it will work: **the unit of occupation equals the unit of recording, therefore the boundary "document ↔ edit" from
F-PRJ-001 is also the future granularity of the lock**, and the cost of postponing it grows twice as fast as is calculated there.
Having split the document and the edit, we gain the ability to occupy them separately for free; without splitting, we remain forever with the
unit "the whole file", like Final Cut, and with a queue instead of shared work.

How we differ from neighbors. **F-COL-013** (two processes over one project file) arrives at the same law from our single-process hole and
stops at the conclusion "the unit available to us is exactly one — the file"; here this conclusion is not the end but a premise: the unit of recording
is not a constant, it is a consequence of the document structure, and therefore the question of granularity is resolved not in the lock but in F-PRJ-001.
**F-COL-012** (the owner of a track) already names at a second layer that the track is absent from every list of units; here the subject is the reverse — not what
is absent from the list, but WHY the list is exactly such, and evidence from our side is added: we cannot localize even the unit of EDITING
(`block_extras` drags morphers, areas and markers along), therefore "occupy a clip" cannot be expressed with us twice over. **F-COL-015** ("read-only" as a mode) is about
what can be done inside something occupied by someone else; here it is about the size of what is occupied.
**F-PRJ-001** is about why to separate the document and the edit for the sake of editing itself; here it is about the fact that the same separation is the only way ever to
occupy not everything at once. **F-COL-017** is about WHERE the lock lies (in the file, next to it, or in the database); here it is about WHAT it occupies.
**F-COL-002** (auxiliary data next to the project) is about the sidecar and the shared disk, not about the unit of occupation.

## Sources

- https://www.blackmagicdesign.com/products/davinciresolve/collaboration —
  "Automatic bin and timeline locking…", "read only until unlocked by the current
  user" (accessed 2026-09-08)
- https://community.adobe.com/t5/premiere-pro/faq-what-is-project-locking-and-how-do-i-use-it/td-p/9431268
  — the definition of project locking and read-only mode (18.10.2017, accessed
  2026-09-08)
- https://community.adobe.com/t5/premiere-pro-discussions/productions-prlock/m-p/13543552
  — `.prlock` as a declaration of a lock next to the project file (accessed
  2026-09-08)
- https://creativecow.net/forums/thread/library-is-already-in-use/ — the Final Cut message about a
  busy library (18.10.2017, accessed 2026-09-08)
- The units of occupation are collected in `docs/features/_services.md`, column "Unit of occupation":
  sequence (Team Projects), bin and timeline (Blackmagic Cloud and Resolve Project Server), project file
  (Productions), the whole library (shared Final Cut library).
- The VEGAS and After Effects rows reflect the absence of a mechanism and were checked against the composition of
  the products, not against a tracker: there is no public tracker for any of the five
  (`docs/features/_trackers.md`, "Whom we do NOT take").
- Our code was checked by grep over `src/` on 08.09.2026: `kt-model/src/model.rs`
  (`TimelineModel`, `apply_batch`), `kt-model/src/ops.rs` (`block_extras`),
  `kt-model/src/project.rs` (`BinGroup.locked`), `kt-app/src/app.rs`
  (`write_project_guarded`, `write_project_atomic`); `flock`, `create_new`,
  `single-instance` and reading of mtime do not occur even once in `apps/` and `libs/`.

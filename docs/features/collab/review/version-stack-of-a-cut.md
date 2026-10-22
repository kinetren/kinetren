---
id: F-COL-024
title: Version of a cut as the unit of review
status: candidate
priority: B
cost: M
category: collab/review
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-engine, kt-app, kt-model]
invariants: [9, 11]
ui_rules: [13, 19]
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [frame-io, bm-cloud]
checked: 2026-09-08
---

# F-COL-024 — Version of a cut as the unit of review

## Essence

Reviewers review not the project and not the timeline but a RENDERED cut: the client watches the file, not the edit. The cut is made anew after each round of revisions, and by the week seven such files accumulate. While they lie in a heap, the conversation falls apart: it is unclear which of them the remark is written on, whether what was asked for last time is already done, and which file counts as today's. A stack of versions is the answer: the cuts are piled one on another in the order they were made, the stack has one address, and comparing adjacent versions is exactly why the old replies are kept at all.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | there is no entity "render version" at all: Render As writes a file, and after that only the file system remembers it; rounds of revisions are told apart by a suffix in the name, by hand — **[NOT VERIFIED]** against the help of the 2026 line |
| Adobe Premiere | 26.x | review panel inside the editor (Frame.io V4), Share for Review | the stack lives NOT in the editor but on the other side: the editor hands the next export there, and the service stacks the versions. In 25.6 the panel was rewritten entirely, and replies are attached to sequence markers, not to the timestamp of the cut |
| DaVinci Resolve | 21 | Presentations (Blackmagic Cloud), a preset of the same name on the Deliver page | a TIMELINE or a clip is published directly from the editor, not "a file in general"; "shared markers and comments are automatically and bidirectionally relinked with DaVinci Resolve". The stack itself is again on the cloud side |
| Final Cut Pro | 12.3 | Snapshot Project (Shift-Cmd-D), Duplicate As | the closest native thing is a version of the PROJECT, not of the cut: the copy gets the word Snapshot added to its name and the date with the time. That is, the identity of a version = a string in the name, issued by local clock |
| After Effects | 26.x | the same review panel; render queue with name templates | the application itself has no version of the cut as an entity: there is a queue job and a file name template, and only the person knows "which one is this in order" |

The service (Frame.io, Blackmagic Cloud) does not get a row in this table under the doctrine of the three registries (`_services.md`): the verdict "take / do not take" lives next to the editor, not next to someone else's host. The mechanism, however, originates from there, and it is analysed below in prose — and that is the finding: **a stack of versions is not a feature of any of the five editors.**

## The trick behind it

**1. A stack is not a folder and not a renaming, but an object with a PERMANENT address, and the permanence of the address is the whole point.** A link given to the client once does not need to be handed over again: the new version goes into the same stack, and the same link opens it. The non-obvious consequence: the address of the conversation and the address of the frame live DIFFERENT lifetimes — the link is eternal, while a reply is pinned to a specific version. Put the versions into a folder — and a new address has to be sent after each round, and half of the recipients will stay on the old one.

**2. The version number is a DELIVERY log, not an editing log.** A version is created by dropping the file on top of the previous one ("upload the new version, then drag it on top of the original version"), so numbering counts uploads: an old edit re-uploaded becomes the next number, rather than returning to its own place. Hence the classic mismatch between "v4" in the editor's file name and "V4" in the client's stack — these are two different counters, and they coincide only as long as nobody has made a mistake.

**3. Versions of TWO KINDS coexist in one stack, and only their origin tells them apart.** There are versions created by a person (dragging a file), and there are system ones: when working through a mounted storage, local saves "will no longer version stack but instead be automatically nested in your version history and attach to the latest version you're working on", and "Saves do not consume storage". That is, the stack has acquired a second, hidden level — history inside a version — and it is shown to the user not as a version.

**4. Where there is no stack, the identity of a version is a STRING issued by local clock.** Final Cut appends a word and a date with time to the snapshot's name; on one machine this works, on two it gives two "Snapshot 2026-09-08 14:31", and no sorting can tell them apart any more. A string instead of an address is exactly the place where a stack stops being a stack.

## Pros

- The link does not expire: the address is issued once per JOB rather than per file, so a round of revisions does not generate a re-send.
- A reply stays pinned to the frame it was written on: the old version goes nowhere, and "look at what I was talking about" can be checked a month later.
- Comparing adjacent versions turns the archive into a tool: without it old replies are useless, because it is unclear whether they were fulfilled.
- The order of making is visible without agreements about file names — that is, the mechanism does not fall apart because one person named a file differently.

## Cons

- The numbering lies when re-uploading: "v5" may turn out to be older than "v4", and within the stack this is not visible.
- The unit of review is a flat cut, so a reply is addressed to the TIME of the cut, while the edit has already moved on by the moment of reading (this is a separate note, F-COL-026).
- The stack lives on someone else's host: it is not part of the document, and if access is lost, the whole history of the conversation is lost, including the approvals.
- Two version counters (the editor's in the file name, the service's in the stack) diverge silently — nobody checks them against each other automatically.
- Storage is paid for each version, so old versions get deleted — and they are exactly what is needed when a dispute reaches "but this is what we agreed on".

## What users say

About why a version is not a technical trifle but a unit of contract:

> We include 2 rounds of minor revisions. Minor revisions are usually about 1
> hour of editing time for each round of revisions.
> — Greg Ball, Creative COW, "Defining rounds of revisions", 15.04.2015,
> https://creativecow.net/forums/thread/defining-rounds-of-revisions/

And in the same thread — why a round must have a reference point to which one can return:

> You can't change what you previously approved and you can not ask for
> something more that wasn't in the estimate.
> — Ned Miller, the same thread, 16.04.2015

Both quotes are about one thing: a round of revisions is counted, referred to, and used to limit the scope of work — so it must have a name, and the named round must have a demonstrable cut.

## How it fits Kinotrain

### What we already have

Making a cut is done by us entirely and inside the process (invariant 9): mux and encoding go through `kt_engine::export`, without temporary files and without an external `ffmpeg`. There is even more than one might expect:

- **Range.** `src/libs/kt-engine/src/export.rs` supports `range`, and the GUI offers "whole project" and "region" — so "hand out a piece for review" is already expressible.
- **A meaningful name.** `render_file_name`
  (`src/apps/kt-app/src/render_dialog.rs`) assembles
  `kt-<project>-<area>-<length>s.mp4`, and the area name is taken ONLY from a range marker that coincides with the region, "so that the name does not lie about the contents".
- **A check before handing out.** `missing_sources` walks the footage and names what is missing BEFORE the start, not after (F-COL-006 analyses this from the side of rights).
- **A document revision exists.** `model_rev`
  (`src/apps/kt-app/src/app.rs`) grows on every edit of the model through the `apply*` wrappers.
- **Two runs give one byte.** Audio is byte-for-byte always, video on the CPU path — between runs and against the reference (invariant 11). So the question "did anything change between two cuts" has, for us, an honest answer in principle, not an estimate by eye.

### What is missing

All of the above dies together with the job. `RenderJob` knows the path, the settings, the resolution, the fps and the list of missing sources — and this state lives exactly until the window is closed: nowhere is a record left that the cut was made at all. Hence three holes in a row:

- A list of what was made does not exist: no path, no time, no settings, no revision from which the cut was assembled. Open the project and ask "what did I deliver on Monday" — there is no one to ask.
- A version number is absent from the name too: the template encodes the CONTENT (project, area, length in seconds), not the iteration, so a second render of the same piece suggests the same name.
- The document revision does not go into the file: `model_rev` is the key of the drawing caches, `saved_rev` is the guard of autosave; only format versions go into `.otio`. A cut cannot refer to the edit it was made from, because the edit has no presentable name of state.

OFFLINE: DEGENERATES — `src/apps/kt-app/src/toolbar.rs:672` — on one machine the stack degenerates into a folder of files: the render button is the only entry point to making a cut, and after it the program remembers not a single produced file: neither its path, nor its settings, nor the revision it was assembled from. The proposed name encodes the content rather than the iteration, so a second render of the same piece proposes the same name, and the entire stack of versions is held by the question of the system dialog "overwrite?".

NETWORK COST: one stack for two people and confidence that both opened ONE AND THE SAME v4 — cross-project storage — the cut does not belong to any project file: it was made from a revision that is not in the file, and in meaning it lies next to the projects, not inside one of them; until such a place exists, the version number exists only in the file name, and each person's file name is their own.

### What gets in the way

The obstacle is constructive, not volumetric: **the version log has nowhere to live.** Our project is one `.otio` file with a byte-stable round-trip, and the record "the cut was made at such-and-such time from such-and-such revision" does not belong inside it: it is not part of the edit, and it breaks the idempotence of the cycle. A neighbouring place does exist — the sidecar `<project>.ktindex.db`
(`src/libs/kt-media/src/index.rs`) — but it is built for a cache, not for data that would be a pity to lose: rows are keyed by ABSOLUTE path, and on any opening error the database is DELETED and recreated. Storing the history of deliveries there is not allowed, and this is exactly the case where a note must name the obstacle rather than go around it.

The second obstacle is a place in the interface (checklist rule 13): creating a strip for the list of versions is dishonest, since it appears rarely; and the render settings window must say where the decision is later changed (rule 19).

### Which invariants it touches

- **9 (export is in-process).** A log of what was made is a continuation of the same rule: no external job catalogue and no agent, otherwise the first "version manager" brings a process on the other side along with it.
- **11 (determinism).** It is exactly this that makes the stack honest for us and impossible for others: two cuts assembled from one revision with one backend coincide byte for byte, so "the version did not change" is a verifiable statement, not an impression.

Two UI checklist rules are touched: 13 (the list of versions must fit into an already occupied place, not a new strip) and 19 (the render window names where the version number is switched).

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages: a record of the made cut next to the project; the version number in the name and in the render window |
| Where the main work is | kt-app (the render window and the place of display), kt-engine (what exactly is recorded about the job), kt-model (the presentable name of the document state) |
| Project format | does not touch: the log of deliveries is not part of the edit, and it has no place in `.otio` |
| Tests to write | "a second render of the same piece does not suggest the same name"; "the log survives reopening the project" (unlike `.ktindex.db`, which is deleted on error); a layout test "the list of versions does not grow the strip" |
| Depends on | nothing; F-COL-025 (version number in the frame) and F-COL-026 (a reply is addressed to a version) rest on it |
| Breaks compatibility | no |

## Verdict

Take the local half: a record of the made cuts next to the project and a version number issued by the program, rather than typed by hand in the save dialog. The server half we do not create — there is no second participant, and declaring one is not within the note's rights.

**Separately and explicitly: the conclusion of F-PRJ-002 is NOT carried over here.** There it is said about the project file: "a user project is one text file with a byte-stable round-trip, and git solves this half better than we would". Here the subject is different — the cut, an object OUTSIDE the project: it is not text, it is not diffed, it cannot be reproduced from history (that requires the same footage, the same backend and the same hardware), and "Save as" does not create it at all. Neither git nor model snapshots answer the question "what did I hand to the client on Monday and how does it differ from what I will hand over today". The only thing we already can do, and that none of the five can do, is to say of two cuts "these are byte for byte the same" (invariant 11); on this we should build.

Boundaries: the job queue and background export — F-EXP-002 (the queue is internal, otherwise invariant 9); the history of edits and document snapshots — F-PRJ-002; autosave and project versions on disk — F-RES-001; how a reply is addressed in a foreign player — F-COL-025; returning replies to the edit — F-COL-026.

## Sources

- https://creativecow.net/forums/thread/defining-rounds-of-revisions/ — a round of revisions as a unit of contract; quotes from Greg Ball and Ned Miller with dates (accessed 2026-09-08)
- https://help.frame.io/en/articles/9101068-versioning-in-frame-io — how a version is created ("upload the new version, then drag it on top of the original version"), access to previous ones, behaviour of saves with a mounted storage (accessed 2026-09-08)
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration —
  Presentations and bidirectional relinking of markers and comments (accessed 2026-09-08)
- https://support.apple.com/guide/final-cut-pro/duplicate-projects-and-clips-verfd45ffa45/mac
  — Snapshot Project: a word and a date with time are appended to the copy's name (accessed 2026-09-08)
- The VEGAS and After Effects rows were assembled from the composition of interfaces; not checked live — none of the five has a public tracker (`docs/features/_trackers.md`, "Who we do NOT take"), and the mark `[NOT VERIFIED]` stands in the table itself
- Our code checked by reading on 08.09.2026: `src/apps/kt-app/src/toolbar.rs`
  (the render button), `src/apps/kt-app/src/render_dialog.rs` (`render_file_name`,
  `default_render_name`, `launch_render`, `RenderJob`),
  `src/libs/kt-engine/src/export.rs` (`range`),
  `src/libs/kt-media/src/index.rs` (sidecar behaviour on opening error),
  `src/apps/kt-app/src/app.rs` (`model_rev`)

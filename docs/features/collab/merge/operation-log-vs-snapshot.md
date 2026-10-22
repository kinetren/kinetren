---
id: F-COL-036
title: Operation versus snapshot
status: researched
priority: B
cost: XL
category: collab/merge
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, otio-lib-0.18]
we_have: partial
crates: [kt-model, kt-otio, kt-engine, kt-app]
invariants: [3]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-PRJ-002, F-COL-033]
trackers: []
services: [team-projects, bm-cloud, resolve-project-server]
checked: 2026-09-08
---

# F-COL-036 — Operation versus snapshot

## Essence

The history of edits is stored in one of two ways, and there is no third. Either the program remembers the STATE of the document after each gesture — a snapshot — or it remembers the GESTURE ITSELF: "moved clip 7 by 12 frames to the right". The editor sees no difference while pressing Ctrl+Z: both constructions undo identically. The difference shows up on other questions — "what did I do yesterday", "how do these two files differ", "take his edit without touching mine" — and on these the snapshot does not answer, not because it is unfinished, but because it does not store the answer.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Undo History | a visible list of NAMED steps; a click on a row rolls back everything up to it. A step has a name, but the operation itself is not exposed: there is no way to compare two projects |
| Adobe Premiere | 26.x | Undo; History; in Team Projects — Publish and Version History | inside a session — a history of steps; outwards — snapshots: a published version is a state, not a set of edits. One cannot undo one's own unpublished edits at all |
| DaVinci Resolve | 21 | Timeline Comparison + Difference Index | the database is the base, but the comparison works NOT on a log: two timelines are taken as states, the difference is computed after the fact and accepted by a human section by section ("Accept Change") |
| Final Cut Pro | 12.3 | Undo; Snapshot | a snapshot is a copy of the state of an event/project, literally by the name; there is no comparison of two states |
| After Effects | 26.x | Undo (depth is configurable) | snapshots in memory, depth is a setting; no comparison, no merge |

**Outside the core.** One row: the format by which the industry exchanges an edit — and it also explains why the log does not cross the boundary of the program.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| OpenTimelineIO | 0.18.x | Timeline (state) | all exchange forms — AAF, EDL, XML, `.otio` — are snapshots: they describe WHICH edit it is, and none describes WHAT was done to it. A change list (EDL) is no exception: it is also a state, only expressed as a list of events |

## The trick behind it

**1. The one in the industry who actually merges edits does not keep a log.** Resolve's timeline comparison tool takes TWO STATES and computes the difference after the fact, showing by sections where frames were added, deleted, moved or trimmed; the "Difference Index" is derived from snapshots, not a record of events, and changes are accepted by a human, one section at a time. That is, the industry settled the dispute "log versus snapshot" not in favour of the log: it stayed with snapshots and added a lock and a diff to them.

**2. The diff of two snapshots is possible exactly as far as identity is solvable.** A comparison has to say that the clip on the left and the clip on the right are "the same", and no one has identity in the file (F-COL-033), so they compare by lineage: media, start, length, position. Hence a non-obvious consequence for planning: "make a comparison" is a task not about edit history in general, but about identity, and it gets cheaper exactly as far as the addressing has been put in order.

**3. The price of a snapshot is paid not in merging but in SINGLE-USER work.** A snapshot answers perfectly "put it back as it was", and does not answer ANY question of the form "what has changed": neither "what did I do in the last hour", nor "how does the file on disk differ from what is in my window", nor "what did the circle lose through a foreign format". We see this literally: `model_rev` knows that something has changed, and does not know what — the "unsaved" asterisk exists, but a description of the edit exists nowhere.

**4. We do not lack a log — it is THROWN AWAY at the entrance.** Operations are written and typed: 45 variants of `Command`, a single funnel `apply`, a batch `apply_batch` for a complex gesture, and coalescing of a live gesture `apply_coalesced`. A command reaches the model and dies; what is saved is its result — a clone of the whole project. And on disk it is the same: `save_project` serializes the WHOLE project, and the autosave every 30 s writes the next full snapshot over the previous one. The exchange, therefore, has already happened and has been paid for — the note must name it as a price, not as an unfinished part.

**5. One consumer of a diff already works for us, and it is an invariant.** Invariant 3 requires that an edit of the model not kill the reverb tails, and it holds on a comparison of two states: `FxStateStore::sync_clip` compares the fingerprint of the chain with the already-built processor, and `retain_clips` discards the states of dead clips. This is the only place in the program that answers the question "what has changed between two snapshots" — and it relies on `ClipId`, that is, on an address that does not cross the boundary of the document.

## Pros

- A snapshot does not lie: restoration is exact, "undo of undo" is free, and the class of defects "the inverse operation was computed wrongly" is absent by construction — by trackers (F-PRJ-002) precisely this class dominates among those who went with a log.
- The granularity is right and already tuned to a person: a complex gesture is one snapshot, a live drag is coalesced by a token.
- A snapshot survives a change of the model: a field was added to `Project` — the history keeps working, there is nothing to migrate. With a log, each new command is also an inverse operation, and its version.
- On disk, a snapshot means that the project file is self-sufficient: it opens without any history and does not depend on what wrote it.

## Cons

- A snapshot does not know the author and does not know what has already been handed out: "undo someone else's edit without touching mine" is impossible with it even in theory.
- The depth of history is memory: 80 clones of a whole project cost more the larger the edit, and the limit has to be set by a number, not by meaning.
- No question of the form "what has changed" is answered by snapshots without a diff, and a diff is a separate subsystem that runs into identity.
- A log, if it were ever introduced, pays with exactly what a snapshot gets for free: precision of restoration and resilience to the evolution of the model.

## What users say

The real case for which a comparison is switched on is not a merge but "what is new here":

> I was e-mailed a new XML and couldn't remember how to compare timelines so I
> could see which were the new shots.
> — Warren Eagles, 15.10.2018,
> https://liftgammagain.com/forum/index.php?threads/compare-timelines-resolve-tip.11564/

As the vendor sells it — and note that the word "merge" stands next to the word "versions", not next to the words "changes from the log":

> The revolutionary timeline comparison tool lets you visually see the differences
> between two versions of a timeline.
> — Blackmagic Design, Collaboration page,
> https://www.blackmagicdesign.com/products/davinciresolve/collaboration
> (access date 2026-09-08)

## How it fits Kinotrain

### What already exists

- **History is snapshots of the whole document**: `undo: Vec<Project>` in `TimelineModel`, `push_undo` (`src/libs/kt-model/src/model.rs:428`) clones the whole project, the limit `MAX_UNDO = 80` (`:11`), return — `undo` (`:450`).
- **Operations exist and are typed**: 45 variants of `Command` from `AddMedia` to `RemoveCameraLane` (`src/libs/kt-model/src/model.rs`), the batch `apply_batch` (`:376`) and the coalescing of a live gesture `apply_coalesced` (`:414`) give exactly one snapshot per GESTURE.
- **On disk, also a snapshot**: `save_project` (`src/libs/kt-otio/src/lib.rs:91`) serializes the whole project, the background autosave every 30 s writes a full snapshot into the user's file (`src/apps/kt-app/src/app.rs:1078`), and on disk there exists at every moment exactly one state.
- **The only already working diff of two states** — invariant 3: `FxStateStore::sync_clip` (`src/libs/kt-engine/src/fxstore.rs:25`) compares the fingerprint of the chain, `retain_clips` (`:54`) discards the states of dead clips.
- **The lineage for comparison already lies in the model**: `media`, `source_in`, `duration` of an event (`src/libs/kt-model/src/project.rs:1846-1855`) — what everyone who compares uses (F-COL-033).
- **A description of an edit exists nowhere**: `model_rev` (`src/apps/kt-app/src/app.rs:327`) is a counter, not a name; neither the snapshot nor a history step has a title or an author.

OFFLINE: REMAINS — `src/libs/kt-otio/src/lib.rs:91` — the exchange is done whole and works on one machine: saving serializes the WHOLE project, the autosave lays the next full snapshot over the previous one, and precisely for that reason our undo is exact, "undo of undo" is free, and the class "the inverse operation was computed wrongly" is absent. The same thing has bought the fact that the question "how does this file differ from yesterday's" has no answer even for a single user.

NETWORK COST: to bring two edits of one edit together — edit log — a snapshot has nothing to add up: an edit has no name, no author, and no base revision, so "subtracting" one gesture from a state and leaving the others is not possible even in theory.

NETWORK COST: to place two states of one edit side by side in order to compare them — cross-project storage — on disk we have exactly one state of the project, and there is nowhere to open a second line of editing: our program cannot bring someone else's edit alongside its own at all (F-COL-037).

### What is lacking

- Not a log — we do not start one — but a **name for a step** (F-PRJ-002): without it not one "what has changed" question can even be formulated in the interface.
- A function comparing two `Project`s by lineage. It requires neither a network nor a log, and it answers at once three single-user questions: what the circle lost through a foreign format, how an edited file differs from the original, what changed after an import.
- A second line on disk: there is nothing to compare while the state is single (F-RES-001).

### What stands in the way

A snapshot is not a semi-finished log, but a different construction: it has neither a description of the edit nor an inverse operation, and "add an author" to it cannot be done, because an author appears only for an OPERATION. The transition costs what snapshots are precisely good at: precision of restoration and resilience to the evolution of the model — each new command in a log acquires an inverse operation and its own version, while we have 45 commands and a living model. The second obstacle is invariant 3: it identifies a clip by `ClipId`, and the numbers are local and reused (F-COL-033), so any splicing of two histories begins with the numbers ceasing to be addresses.

### Which invariants it touches

Invariant 3 — directly: it is exactly our single diff of two states, and it is bound to `ClipId`. Invariant 11 (determinism) indirectly and in our favour: a snapshot is reproduced byte for byte, therefore comparing two snapshots is a pure function without any wall-clock. The other ten are not touched: history computes nothing and draws nothing.

## Estimate

| What | Answer |
|---|---|
| Scope | XL for the log (it changes the model, the format and the whole path of a command); S for what we take — a function comparing two `Project`s by lineage |
| Where the main work is | kt-model (comparison of states, names of gestures), kt-app (showing the result), kt-otio (comparison of what went into the file) |
| Project format | not touched: states are compared, not records |
| Tests to write | "a circle through a foreign format: the comparison names exactly what was lost"; "the comparison is stable under renumbering of clips"; preservation of the behaviour of t96 (region and loop are not rolled back) |
| Depends on | F-COL-033 (identity for comparison), F-PRJ-002 (names of steps), F-RES-001 (a second state on disk) |
| Breaks compatibility | no |

## Verdict

We do not start an edit log, and not because it is expensive, but because **no one who truly merges an edit has started one**: Resolve compares two states and lets a human accept the difference section by section, rather than replaying someone else's log. Our exchange has already happened, and this is not debt but a purchase: the snapshot of a whole project gave us exact undo, gesture-level granularity, and the absence of a whole class of defects that remains dominant among those who went with a log (F-PRJ-002 showed this from the trackers). Writing `undo: Vec<Project>` down as an unfinished item would mean rewriting the model for the sake of a question that we do not yet have anyone to ask.

We take one thing from the reconnaissance and take it now: **comparison of two states by lineage**. It requires neither a log, nor a network, nor a second participant; it answers a question that today is asked of us three times and in different ways — "what did the circle lose through a foreign format" (F-COL-008), "how does the file on disk differ from what is in the window" (F-COL-034), "what came along with someone else's edit" (F-COL-037); and it is cheaper exactly as far as the addressing has been put in order (F-COL-033). A second decision to record in advance: if a comparison ever appears, its result must not be called a merge — a human accepts the difference, section by section, as at Blackmagic.

How we differ from neighbours. **F-PRJ-002** (edit history and project versions) owns the visible history and named states for ONE person; here the subject is the choice of the CARRIER and which questions it is in principle able to answer. **F-COL-018** (what Ctrl+Z undoes under shared ownership) is about the behaviour of undo when an edit is not only mine; here undo is not the main consumer of history at all. **F-COL-034** (document revision) is about the address of a state; here it is about its content. **F-COL-033** is about what an object is addressed by inside a state, without which a comparison does not count.

## Sources

- https://www.blackmagicdesign.com/products/davinciresolve/collaboration — "revolutionary timeline comparison tool", "Automatic bin and timeline locking", "merge changes into a master timeline" (access date 2026-09-08)
- https://liftgammagain.com/forum/index.php?threads/compare-timelines-resolve-tip.11564/ — why the comparison is switched on in practice (15.10.2018; access date 2026-09-08)
- https://community.adobe.com/t5/premiere-pro-beta-discussions/now-in-beta-updated-version-and-auto-save-history-in-team-projects/td-p/14128940 — a version at Adobe is a state, not a set of edits (03.10.2023; access date 2026-09-08)
- `docs/features/project/history/versions-and-snapshots.md` (F-PRJ-002) — the count from the trackers: the dominant class of undo defects among those who went with a log — "the wrong thing came back"
- Our code checked by grep over `src/` on 08.09.2026: `kt-model/src/model.rs` (`undo`, `push_undo`, `MAX_UNDO`, `apply_batch`, `apply_coalesced`, the set of `Command` — 45 variants), `kt-otio/src/lib.rs` (`save_project`), `kt-engine/src/fxstore.rs` (`sync_clip`, `retain_clips`), `kt-app/src/app.rs` (`model_rev`, the background autosave).

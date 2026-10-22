---
id: F-COL-018
title: What Ctrl+Z undoes once an edit has been handed off
status: researched
priority: B
cost: XL
category: collab/claim
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, syncthing-2]
we_have: partial
crates: [kt-model, kt-engine, kt-app, kt-cli]
invariants: [3]
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-PRJ-002]
trackers: []
services: [team-projects, bm-cloud, syncthing]
checked: 2026-09-08
---

# F-COL-018 — What Ctrl+Z undoes once an edit has been handed off

## Essence

In solo work Ctrl+Z promises something simple: "put it back as it was". As soon as a document has a second writer, the promise splits — "as it was FOR ME" and "as it was AT ALL" — and these are different states. The editor does not distinguish them: he presses the familiar key and expects his last gesture to be undone. The document is undone. Separately and even more painfully there appears a third question, which people also call undo: "return me to what I last handed in" — and it has no relation to the edit history at all.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Undo History | a list of named steps; choosing a point undoes everything up to it; the work is solo, so the question "whose edit did I undo" does not arise by construction |
| Adobe Premiere | 26.x | Undo (each project has its own queue); in Team Projects — "Publish" and nothing reversible | in Productions the document is split into files, and each has its OWN history: moving a sequence between projects is undone with two presses, because these are two histories. In Team Projects one cannot undo one's own unpublished edits at all — people get by by deleting the local cache and reopening from the server version |
| DaVinci Resolve | 21 | Undo per page; separately — timeline comparison | in shared mode the edits go as records into the database, and the question "what changed between versions" is moved to a SEPARATE timeline comparison tool: undo and reconciliation are fundamentally different mechanisms, and Blackmagic does not hide this |
| Final Cut Pro | 12.3 | Undo (local) | foreign edits within a session do not happen by construction: the library is taken entirely, so the history honestly stays solo. Versions are made by a copy and a snapshot, not by undo |
| After Effects | 26.x | Undo (local, depth is configurable) | the same: the project file is solo; in Team Projects AE inherits someone else's model without its own undo |

**Outside the core.** Syncthing is needed here in one line: it is the only registry system where "diverged" is a normal event with a FORMAL outcome, and it is visible that undo takes no part in it at all.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Syncthing | 2.x | sync-conflict | the losing version is neither undone nor merged, but becomes an ordinary file with a label-name and spreads to all nodes; the outcome rule is the older edit time, and on equality the higher 63 bits of the device ID |

## The trick behind it

**The industry ensured the correctness of undo not by the undo mechanism but by a LOCK.** Undo in all five is an operation over the whole document, and it is safe exactly because there are no foreign edits within my session: the document is busy, the second one looks at it read-only. As soon as Team Projects allowed two people to be in one document (in different sequences), undo disappeared at once — not broke, but precisely disappeared: "return or discard one's own changes" has not existed there for four years, and the standard answer became the workaround of deleting the local cache. That is, "Ctrl+Z in a shared document" is not an unfinished feature but the first thing that is lost when the lock is removed.

**The second layer: people call one key by three different things.** Undo of a step (the session history), return to the last HANDED-IN version (what the second one saw), and comparison of two versions (what changed at all) — three different mechanisms with different carriers. Resolve alone separated them explicitly, moving comparison into a separate tool; Adobe reduced everything to "Publish", and hence a four-year request to return at least one of the three. A subtlety visible only in practice: snapshot history (ours, FCP, AE) answers the first question perfectly and does not answer the second at all in principle — a snapshot does not remember what has already been handed out of it.

**The third layer, the least obvious: the boundary of applicability of snapshot undo arrives EARLIER than the network does.** A second writer into the same file is enough, and it exists even on one machine: a script editing the project between saves (F-EXT-002), a folder synchronizer, a second program window, a background autosave aimed at the user's file. A snapshot of the whole document does not distinguish whose edit is inside — and it undoes everything that fell between two snapshots.

## Pros

- Snapshot history is simple and does not lie: restoration is exact, the state is reproduced entirely, and "undo of undo" is free. Errors of the kind "the inverse operation was computed wrongly" cannot occur in it by construction.
- One snapshot per gesture gives the right granularity for a person: a complex gesture (a split of an A/V pair, an insert, a punch-out) is undone with one press, not five.
- The lock makes history correct without any synchronization: in a busy document there are no foreign edits, so "return it as it was" and "return it as it was for me" are one and the same.
- The separation of "undo of a step" and "comparison of versions" (Resolve) pays off beyond shared work: the comparison answers the question "what did I do yesterday" even alone.

## Cons

- A snapshot of the whole document does not know the author: with it one cannot undo someone else's edit without touching one's own — and vice versa. This is not a setting but a property of the carrier.
- A snapshot does not remember what has already been handed out of it, so the question "return to the last handed-in version" is not solved by snapshot history at all; in Team Projects this is exactly the function that is missing.
- The depth of history is memory: snapshots of the whole document cost more the larger the project, and therefore they are always limited by a number rather than by meaning.
- Undo in a shared document, done naively, is more dangerous than the absence of undo: it looks like a local action and produces a global one.

## What users say

A request that has been around for four years — and both halves are named in it: undoing one's own edits and returning to the last shared version:

> When using Team Projects, there is currently no way to revert or discard your own
> changes.
> — TRP-Scott, "Revert or discard changes in Team Projects", 05.02.2023,
> https://community.adobe.com/t5/premiere-pro-ideas/revert-or-discard-changes-in-team-projects/idi-p/13555527

In the same thread the standard answer is not a function but a workaround: close the project, delete the local `.tpr` cache from the autosave directory, and open it again so that the latest published version is pulled in (sngazm, 12.2024); the second proposed path is recovery from the autosave (pauld7352303, 05.2025). That is, even where a server exists, "returning to what was handed in" is done with file-level means.

## How it fits Kinotrain

### What already exists

- History is snapshots of the whole document: `undo: Vec<Project>` in `TimelineModel`, `push_undo` (`src/libs/kt-model/src/model.rs:428`) clones the whole project, the depth is `MAX_UNDO = 80` (`src/libs/kt-model/src/model.rs:11`).
- The granularity is right and already proven: one snapshot per GESTURE — `apply_batch` (`src/libs/kt-model/src/model.rs:376`) for a batch of commands and `apply_coalesced` (`src/libs/kt-model/src/model.rs:414`), which merges a live gesture by a token.
- An edit has neither a name, nor an author, nor a base revision: `model_rev` is the key of the drawing caches, `saved_rev` is the guard of the autosave, and none of them gets into the file. There is no version of the DOCUMENT in the file; there is only a version of the format.
- A list of undo exceptions exists and is written by hand: `undo` (`src/libs/kt-model/src/model.rs:450`) carries into the returned snapshot exactly two fields of the current state — the region and `loop_enabled`. The criterion of this list is "session convenience", and there is no other criterion in it (for example "the edit is not mine").
- A second writer exists with us today and without any network: `kt-cli` writes the same file (`convert`, `roundtrip` — `src/apps/kt-cli/src/main.rs`), the background autosave every 30 s targets the user's file (`src/apps/kt-app/src/app.rs:1078`), and there is not a single read of mtime or a watcher in `kt-app` — an external write is detected by nothing.
- The state of FX is keyed by the clip identifier: `FxStateStore` (`src/libs/kt-engine/src/fxstore.rs`) stores processors by `ClipId` and reuses them while the fingerprint of the chain matches (invariant 3).

OFFLINE: REMAINS — `src/libs/kt-model/src/model.rs:450` — the trick is expressed entirely on one machine: undo transplants the previous snapshot into the document and carries over from the current one exactly two fields, the region and loop, and takes everything else from the past. So an edit that got into the model not from me is undone together with mine and without a single trace, and a second writer exists with us today — kt-cli, a folder synchronizer, a second window.

NETWORK COST: to undo one's OWN edit without undoing someone else's — an edit log — a snapshot has nothing to compare against: an edit has neither a name, nor an author, nor a base revision, so there is nothing to "subtract" one gesture from the state and leave the others, even theoretically.

NETWORK COST: to accept someone else's edit without killing the reverb tail — invariant 3 — the FX state is keyed by `ClipId`, and identifiers are issued as "maximum + 1" and are reused after deletion: a foreign clip with the same number and the same chain structure inherits my processor together with its buffers.

### What is missing

- Not an edit log — we do not start one — but what is needed even alone: DETECTION of an external write into the project file. Today an undo after a foreign write looks like a healthy undo and is in fact a loss.
- An answer to the second question ("return to the last saved state") — we do not have it either: exactly one state exists on disk, and the autosave writes over it (F-RES-001).
- A name for a history step (F-PRJ-002): without a name none of the three questions is formulated in the interface, even a solo one.

### What gets in the way

A snapshot of the whole document is not a semi-finished edit log but a different construction: it has neither a description of the edit nor its inverse operation, and "add the author" to it cannot be done — an author appears only for an OPERATION. The transition to a log costs what snapshots are exactly good for: the precision of restoration and the absence of the class "the inverse operation was computed wrongly" (F-PRJ-002 showed from the trackers that this is the dominant class of undo defects among those who went with a log). The second obstacle is invariant 3: it holds on `ClipId`, and our identifiers are local, and any merging of two histories begins with the numbers ceasing to be addresses.

### Which invariants it touches

Invariant 3 (an edit of the model does not destroy FX state) — directly: its identity is `ClipId`, and it will silently keep working on a foreign clip with the same number. The other eleven are not touched: undo computes and draws nothing.

## Estimate

| What | Answer |
|---|---|
| Volume | XL — an edit log instead of snapshots changes the model, the format and the entire command path; the local half (noticing an external write) — S |
| Where the main work is | kt-model (the carrier of history), kt-app (detecting an external write, the question to the user), kt-engine (clip identity for FX state) |
| Project format | a log would require a version of the document IN THE FILE, which we do not have at all today |
| Tests that will have to be written | "undo after an external write does not stay silent"; "two processes write one file — the second does not overwrite the first blindly"; preservation of the behavior of t96 (region and loop are not rolled back) |
| Depends on | F-PRJ-002 (names of steps and the history panel), F-RES-001 (a second state on disk), F-EXT-002 (an external writer) |
| Breaks compatibility | no, while the history remains within the session |

## Verdict

We do not start an edit log: it is paid for by a second participant, and a note has no right to declare it. We take one line from the reconnaissance and put it in a known place: **the boundary of applicability of `undo: Vec<Project>` runs not along the network but along the second WRITER, and we already have one** — kt-cli writes the same file, the autosave aims at the user's file, a second window is not forbidden in any way, an external change is detected by nothing. So the cheap and useful half today is not "make undo multi-user", but to stop pretending that the writer is one: notice that the file on disk has become foreign, and ask instead of silently undoing. The mechanism for this is offered by F-COL-013 (a declaration next to the file); what matters here is that it has a second, invisible from there reason — undo.

A second decision worth recording in advance: the industry protects undo with a LOCK, not with a smart undo. If we ever reach claiming (F-COL-016, F-COL-017), the correctness of our history will turn out to be its consequence — and conversely, any attempt at a "smart merge" without a lock will require discarding the snapshots, that is, paying with invariant 3 and with the precision of restoration.

How we differ from our neighbors. **F-PRJ-002** (the edit history and project versions) is about step names, the history panel and versions on disk for ONE person; here the subject is what undo means when the state of the document was changed not only by me. **F-RES-001** (autosave and versions) is about how many states lie on disk; **F-EXT-002** (scripting) already calls an external edit of an open project unsafe, but for a different reason (autosave will overwrite it) — here the danger is the reverse: undo will erase an external edit that has MANAGED to get into the model. **F-COL-013** (two processes over one file) is about how to declare a claim and diverge in time; here it is about what happens to the history if diverging did not succeed. **F-COL-016** is about the unit of a claim; undo in this pair is a consequence, not a cause.

## Sources

- https://community.adobe.com/t5/premiere-pro-ideas/revert-or-discard-changes-in-team-projects/idi-p/13555527
  — "no way to revert or discard your own changes", a workaround through deleting the local
  `.tpr` cache and through autosave (05.02.2023, replies 07.2024–05.2025; access date
  2026-09-08)
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration —
  comparison of timeline versions as a SEPARATE tool, not undo (access date
  2026-09-08)
- https://docs.syncthing.net/users/syncing.html — the formal outcome of divergence without
  any undo: the loser becomes an ordinary file with a label-name (access date 2026-09-08)
- The VEGAS, Final Cut and After Effects rows reflect the solo history model and were checked against the product composition; none of the five has a public tracker
  (`docs/features/_trackers.md`, "Whom we do NOT take").
- Our code was checked by grep over `src/` on 08.09.2026: `kt-model/src/model.rs`
  (`undo`, `redo`, `push_undo`, `MAX_UNDO`, `apply_batch`, `apply_coalesced`),
  `kt-engine/src/fxstore.rs` (`FxStateStore` by `ClipId`), `kt-app/src/app.rs`
  (background autosave, absence of mtime reads), `kt-cli/src/main.rs` (`convert`,
  `roundtrip` write the project file).

---
id: F-COL-029
title: Why no one has merging
status: rejected
priority: C
cost: XL
category: collab/merge
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, syncthing-2]
we_have: no
crates: [kt-model, kt-otio, kt-app]
invariants: []
ui_rules: []
round: null
point: null
tests: []
depends_on: []
trackers: [kde-invent, openshot]
services: [team-projects, bm-cloud, resolve-project-server, premiere-productions, fcp-shared-library]
checked: 2026-09-08
---

# F-COL-029 — Why no one has merging

## Essence

Two people edited one edit, and their edits diverged. In any text editor a merge starts at this point: the tool shows three versions and offers to assemble a fourth. In video editing there is nothing like this at NONE of the five — not in the cloud, not on a shared drive, not for money. The industry bought something else: not letting divergence happen. The editor sees this as a lock on a bin, on a sequence, or on the whole library — and as the absence of the very question "whose version do we take".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | nothing | no merge and no prevention: a second instance opens the same `.veg` and writes over it. The divergence is neither resolved nor noticed [NOT VERIFIED live: the mechanism is absent from both the help and the feature list] |
| Adobe Premiere | 26.x | Team Projects, the `Resolve Conflicts` window | the only one for which the word "conflict" exists in the interface at all. But resolution is a CHOICE of a version, not the assembly of a third; the window itself has been arriving empty for four years (F-COL-030). The serverless half (Productions) knows no conflicts at all — there a file-lock sits next to the project |
| DaVinci Resolve | 21 | Collaboration + Timeline Comparison | no merge. There is automatic locking of a bin and a timeline ("Bins and timelines are "read only" until unlocked by the current user"), and SEPARATELY a manual comparison of two timeline versions by a human's eyes (F-COL-031). The comparison does not merge: it shows, so that a person carries the changes over by hand |
| Final Cut Pro | 12.3 | exclusive library lock | there is nothing to diverge: the unit of occupation is the whole library, and a second user does not even get read access. This is prevention taken to the limit |
| After Effects | 26.x | the same Team Projects | it has no model of its own; everything it has comes from the service shared with Premiere [NOT VERIFIED live] |

**Outside the core.** All five core rows say "no merge", and explaining WHY there is no merge with five refusals is impossible: one needs the tool that does resolve divergence after all. There is exactly one such tool, and it is not about editing.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Syncthing | 2.x | conflict file | divergence is not prevented but RESOLVED — and even there the resolution is not a merge: the loser is renamed to `<name>.sync-conflict-<date>-<time>-<who>.<ext>` and stays next to the winner as a whole file. The rule is deterministic and written down, but it selects a winner rather than assembling a third version |

## The trick behind it

**Refusing to merge is neither poverty nor vendor laziness: an edit has no address to which a merge could be bound.** Text has an address, and it is stable: a line survives an edit of its neighbouring lines, so "the same line" is a meaningful notion, and a three-way diff works on it. An event on a timeline is addressed by the pair "track × time", and BOTH coordinates change from someone else's edit that did not touch the event in any field: a ripple on another track shifts my clips, an insertion at the start shifts the whole edit, renaming a track changes the first coordinate entirely. That is, after someone else's edit my event has a different address, although it itself has not changed. "The same edit" is undefinable in such a space, and a three-way diff has no base against which to count.

**The second layer: the unit of occupation for everyone equals the unit of WRITING, not the unit of editing.** Premiere writes the project as a whole — it occupies the project; Final Cut writes the library — it occupies the library; Resolve stores bins and timelines as separate database records — it occupies them separately. Nobody locks a clip, and this is a consequence of the same thing: to occupy a clip, one must be able to write part of a document, and none of the five writes a document in parts. Locking granularity in the industry is not a design decision about collaboration; it is an imprint of the file format.

**The third layer — the most unexpected, and visible only in trackers.** Across the entire exported corpus of editing programs there was ONE genuine request for collaborative editing (kde-invent #1243, opened 06.11.2021, over five years six comments and one vote). And the person who wrote it asked NOT for a merge: his item 5 is called "Save a version" and begins with the words "You can't overwrite the opened project". That is, even the only customer for collaborative work designed it as versioning with a ban on overwriting — precisely the prevention the industry sells.

## Pros

- Prevention is expressed as one operation on one object (a lock is present / a lock is absent), and therefore works on a bare file system — without a server, without a user account, without clocks.
- A refusal that happens BEFORE an edit is cheaper than any resolution afterwards: a person does not spend an hour on an edit that will turn out to be the loser.
- Granularity need not be designed: it is already set by how the document is written to disk, and so cannot diverge from the format.
- The mechanism is explainable to a user in one phrase and does not require them to understand what a "base", a "version", or "three sides" are.

## Cons

- Prevention is a queue: while a bin is occupied, a second user does not work at all. The team's throughput is limited by the number of independently occupiable records, and Final Cut has exactly one for the whole library.
- A lock guarantees nothing: any writer that does not read it (a foreign program, a script, the vendor's own CLI) writes over it.
- Divergence happens anyway — through someone else's copy, a backup, a flash drive — and then the industry has nothing at all: no comparison (except Resolve), no named loss (except Syncthing), no log.
- The absence of a merge is masked by the word "conflict" in the Adobe interface: the word is there, the mechanism is not, and an empty window has lived for four years (F-COL-030).

## What users say

The only collaborative-editing request in the entire exported corpus. What matters is not the fact of the request but its item 5 — the customer asks for a ban on overwriting, not for a merge:

> You can't overwrite the opened project.
> — "REMOTE COLLABORATIVE PROJECT", kde-invent #1243, created 06.11.2021, still open as of 08.09.2026, https://invent.kde.org/multimedia/kdenlive/-/work_items/1243

SIGNAL: collaborative editing of an edit — 1 task in kde-invent, `(collaborative (project|editing|edit)|remote collaborat|multi.?user|simultaneous(ly)? edit)`; a share of 0.05% against 0.00% in openshot (4907 tasks, none). More precisely — 1 of 2217, that is 0.045%; the script prints shares with one decimal place and here shows `0.0%`. The count was taken over the `nle` corpus (eleven rating trackers, 25 442 tasks, dumps of 08.09.2026); it is reproducible with `python3 src/scripts/tracker-rank.py --query '(collaborative (project|editing|edit)|remote collaborat|multi.?user|simultaneous(ly)? edit)'`. The bare `collaborat` gives 4 matches, of which 3 are false — "Haiku port/collaboration", "Request for collaboration from CinGG" and "Archive project don't bundle fonts": in the trackers of editing programs this word almost always concerns collaboration among DEVELOPERS, not collaborative editing. `merge` behaves the same way: 95 tasks in losslesscut (4.9 %) — all about joining files, not one about merging two edits.

## How it fits Kinotrain
CAPABILITY: openspec/specs/project-format/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Rejected: there is no merging of projects — the document is one, and versions are handled by the file system; inserting someone else's project is done by composition (in one step).
ADDRESS: src/libs/kt-model/src/model.rs:429 — kde-invent request #1243 runs into exactly this point for us: the customer asked for versions with a ban on overwriting, while everything we can do with two states of one project is to place them side by side as whole clones in memory and switch back to the previous one.
OFFLINE: DOES NOT REMAIN — `src/libs/kt-model/src/model.rs:429` — nothing of a merge mechanism remains on a single machine, and this is visible exactly here: the only point where two states of one project lie side by side stores them as whole clones without a name, without a time, and without an author, so even our own history we can only rewind, not compare; and the edit object itself is short-lived — `Command` lives until the end of the call and does not enter the document.
NETWORK COST: name the edit so that it can be carried separately from the document — edit log — a merge begins not with the joining, but with the edit having a name and a base revision; we have neither, and the unit we exchange is a whole file, so "take someone else's trim and not take their ripple" cannot even be expressed between two copies in one folder.

## Estimate

| What | Answer |
|---|---|
| Scope | XL — not a round but a change of addressing: a stable event identifier, an edit log, a base revision in the file |
| Where the main work is | kt-model (identity and the log), kt-otio (the document revision in the format), kt-app (showing three sides) |
| Project format | a new version is needed: today the file contains neither a document revision nor a stable event name |
| Tests to write | the union of two edits of one project does not produce `ClipOverlap`; an event's id survives deletion and re-creation; the third side is restored from the file |
| Depends on | F-COL-031 (comparison — a mandatory precondition: until a difference can be shown, there is nothing to merge) |
| Breaks compatibility | yes: a stable identifier cannot be derived from today's files |

## Verdict

We do not take it. Merging edits is undefinable in our own addressing: an event is addressed by the pair "track × time", both coordinates are changed by someone else's edit that did not touch the event, and `ClipId` is issued as "maximum + 1" and reused after deletion — so two independently edited files contain different events with the same numbers. Merging two edits for us is not a "disputed state" but a document that our own `validate()` rejects on two already existing errors, `DuplicateClipId` and `ClipOverlap`. This is precisely the reason no one of the five core editors has merging: the industry bought prevention not out of poverty.

Separately, it is recorded so that it is not reopened anew: the only request for collaborative editing across the whole corpus (kde-invent #1243) asked NOT for a merge but for versions with a ban on overwriting. That is, even the demand, which is almost nonexistent, points toward F-PRJ-002, not here.

Boundaries: occupying and not letting divergence happen at all — `collab/claim/` as a whole (F-COL-013 and following); the empty conflict-resolution window as a separate lesson — F-COL-030; comparison of two edits, the only local part of the topic — F-COL-031; the named loss, when divergence has already happened — F-COL-032; versions and snapshots of one's own document — F-PRJ-002; a project opened on another machine — F-PRJ-003.

## Sources

- https://invent.kde.org/multimedia/kdenlive/-/work_items/1243 — kde-invent #1243 "REMOTE COLLABORATIVE PROJECT", created 06.11.2021, open, 6 comments, 1 vote (access date 2026-09-08; metadata checked against the exported `kde-invent` dump of 08.09.2026).
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration — the wording about read-only bins and timelines and about comparing timeline versions (access date 2026-09-08).
- https://docs.syncthing.net/users/syncing.html — the rule for naming a conflict file and "treated as normal files" (access date 2026-09-08).
- https://support.apple.com/en-us/HT207128 — Apple, a shared volume and the exclusive lock of the Final Cut library (access date 2026-09-08).
- The count over the `nle` corpus: `python3 src/scripts/tracker-rank.py --query …`, eleven rating trackers, 25 442 tasks, dumps of 08.09.2026 (registry and pins — `docs/features/_trackers.md`).
- The VEGAS and After Effects rows are marked `[NOT VERIFIED]` directly in the table.
- Our code read on 08.09.2026: `src/libs/kt-model/src/model.rs` (11, 98, 429), `src/libs/kt-model/src/project.rs` (3204, 3493), `src/libs/kt-otio/src/schema.rs` (220), `src/apps/kt-app/src/app.rs` (3110).

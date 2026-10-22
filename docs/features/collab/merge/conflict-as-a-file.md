---
id: F-COL-035
title: A conflict as a separate file
status: researched
priority: C
cost: S
category: collab/merge
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, syncthing-2]
we_have: no
crates: [kt-otio, kt-app]
invariants: []
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-COL-013, F-RES-001]
trackers: []
services: [syncthing, bm-cloud, team-projects]
checked: 2026-09-08
---

# F-COL-035 — A conflict as a separate file

## Essence

They diverged. From there exactly three outcomes are possible: one version was silently lost; the person was shown a dialog and made to choose one version in full; or **both versions were kept, and the losing one was renamed**. The third outcome looks like a refusal to solve the problem — and it is exactly the only one that works in the field, because it does not require anyone to be online at the moment of divergence: the winner stays in its place under its own name, and the loser becomes an ordinary file that lies beside it and goes nowhere.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | the divergence is not detected at all; "keep both" is done by the person through "Save as…" with a name invented on the spot |
| Adobe Premiere | 26.x | Resolve Conflicts (Team Projects) | the conflict is a modal choice between "take the shared version" and "keep mine"; there is no third outcome; for some users the window arrives EMPTY, and this has not been fixed for four years |
| DaVinci Resolve | 21 | lock of bin and timeline | conflicts are prevented, not resolved; when material is synchronised through Blackmagic Cloud, a divergence still ends with files carrying the suffix `-conflict` — that is, the same third outcome, but unintentionally `[source — forum.blackmagicdesign.com, unavailable for verbatim quotation]` |
| Final Cut Pro | 12.3 | exclusive lock of the library | "The document [Library name] is already in use by [user]" — a second version is never created, because the second person is simply not given the chance to open it |
| After Effects | 26.x | Resolve Conflicts (Team Projects) | inherits Premiere's model whole, including its absent third branch |

**Outside the core.** One row, without which the mechanism cannot be described: the only place where the outcome of a divergence is written down formally, down to the tie-break rule.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Syncthing | 2.x | sync-conflict | the loser is renamed to `<name>.sync-conflict-<date>-<time>-<who>.<ext>`; the file with the older modification time loses, and on a tie the device with the larger value of the first 63 bits of its device ID wins; the received files "are treated as normal files" and spread to all nodes |

## The trick behind it

**1. The system deliberately refuses to create a separate class of objects for a conflict.** The loser is not marked with a flag, is not moved into a special store, and does not wait in a queue for resolution — it becomes an ORDINARY file and lives on by the general rules, including propagation to all nodes. The cost of this decision is visible at once: a conflict cannot be "not noticed" and cannot be lost on restart, but it multiplies — which is exactly why Blackmagic Cloud users accumulate two hundred-plus proxies with the `-conflict` suffix.

**2. The file name turned out to be the only portable carrier of metadata.** Three fields that have nowhere else to lie are packed into the name: the date, the time, and WHO edited the loser. Not in extended attributes (they do not survive copying), not in a separate log (it does not survive moving the folder), not inside the file (an arbitrary file has no place for foreign fields) — in the name, because the name is carried by every file system and every transport. This is the cheapest metadata store among the existing ones and at the same time the most fragile: any renaming breaks it.

**3. The rule of the outcome is chosen not by content but by the ability to apply it LOCALLY.** The loser is the older edit; on equal time, the device with the larger first 63 bits of its identifier. The second rule has nothing to do with the data at all: its task is to let both sides, without negotiating, arrive at the same decision about which of them lost. Hence follows a requirement on the environment that is usually not spoken of: the mechanism rests on COMPARABLE clocks of two machines.

**4. The industry contrast: where there is a server, there is no third branch.** Team Projects offers exactly two outcomes — take the shared version or keep one's own — and live threads show what comes of it when neither works. The presence of a server, contrary to expectation, REMOVES the most stable outcome rather than adding it: the server must have one truth, while a file folder need not.

## Pros

- Nothing is lost, and this is provable: both versions are on disk, both are visible to the file manager, and both survive a restart and a move.
- Resolving a conflict is postponed for any length of time and is done by a person when they are ready for it — the mechanism does not require presence at the moment of divergence.
- It is implemented without a single new format and without a store: a file name and a comparison rule are all that is needed.
- It works exactly the same way on a flash drive, on a shared folder and in the cloud: carrying a conflict is no different from carrying an ordinary file.

## Cons

- Clutter accumulates silently: hundreds of files with a label in their names that nobody sorts through, and which are frightening to delete.
- A name as a carrier of metadata is lost at the first renaming and does not survive limits on name length; no program is obliged to understand it.
- The rule "the older edit time loses" rests on the clocks of two machines — and on file systems with a coarse time resolution it degenerates into a tie-break by device identifier.
- The outcome does not answer the question "what exactly diverged": two files lie side by side, and there is nothing with which to compare their contents (F-COL-036).

## What users say

What happens when there is no third outcome, and the first two do not work:

> Even though I tried each option like take the share version or keep my version,
> the conflict could not be solved.
> — Yijia5C0E, 13.07.2021,
> https://community.adobe.com/t5/team-projects/team-project-could-not-resolve-the-conflict/td-p/12174435

The formal side of the third outcome — from Syncthing's documentation, where it is the only one described before the tie-break rule:

> The file with the older modification time will be marked as the conflicting file
> and thus be renamed.
> — Syncthing docs, "Syncing," https://docs.syncthing.net/users/syncing.html
> (accessed 2026-09-08)

## How it fits Kinotrain

### What already exists

- **A project is one text file, and the write is atomic**: `write_project_atomic`
  (`src/apps/kt-app/src/app.rs:5051`) writes to a temporary file beside it and renames it, so "two versions side by side" is a natural state for us, not an emergency one.
- **Our document has no name of its own.** The name that travels into the file is taken from the passthrough remainders of a foreign document (`src/libs/kt-otio/src/convert.rs:529`), and for a project created by us it is empty: a foreign name we preserve, our own we do not create. The identity of the document lies wholly in the file name — exactly where Syncthing keeps its conflict label.
- **The program composes a file name exactly twice**, and neither time is about a version: `<name>.otio` on importing a `.ktproj` (`src/apps/kt-app/src/app.rs:5435`) and the render name derived from the content of a region (`src/apps/kt-app/src/toolbar.rs:672`). There is no policy in the program of "name as a state label."
- **One artifact that survives the process we already have** — the autosave file of an unnamed project at a fixed path (`autosave_path()`, `src/apps/kt-app/src/app.rs:5020`): without a name, without a time, without an owner, and the first Ctrl+S in any instance deletes it for good.
- **We do not detect divergence**: the mtime of the project file is read nowhere, there is no watcher, and the background autosave targets the user's file every 30 s (`src/apps/kt-app/src/app.rs:1078`).

OFFLINE: DEGENERATES — `src/libs/kt-otio/src/convert.rs:529` — the outcome degenerates into "Save as…" by hand: we can make two copies side by side, but we cannot tell them apart. The name that a document carries inside itself is empty for our project and is filled in only for a foreign file, so the copy and the original are indistinguishable from the inside, and the whole state label rests on a file name that nobody but a person composes.

NETWORK COST: to name the loser without asking anyone — shared clock — the outcome rule compares the edit times of two machines, and we have neither a second time nor trust in our own: the program does not read the mtime of the project file even once.

NETWORK COST: to add to the name WHO edited the loser — user account — the third field of the label is derived neither from the document nor from the file system: we have no person's name and no machine identifier anywhere in the code.

### What is missing

- Detection of the divergence — without it the third outcome has no one to trigger it (F-COL-013, F-COL-034).
- A naming rule for the second copy: today a person invents it, and so two copies from one day are told apart by the words "final" and "final2".
- A way to look at how two copies differ: we can already keep both, but we cannot compare them (F-COL-036).

### What gets in the way

Nothing constructive — and this is a rare case. The mechanism requires no format, no model, and no invariant: it lives entirely in the file-writing layer. Only two things get in the way: the absence of a signal on input (there is nothing to compare the divergence with), and the absence of two of the three fields of the label — a time one can believe in, and the name of the one who edited. From the third field, the date, a label alone cannot be assembled, because it does not answer the question "whose?"

### Which invariants it touches

None of the twelve: the mechanism touches neither sound, nor picture, nor cache, nor time. It does not touch the rules of the UI checklist either — the question to the user, should it ever be needed, lives in the same save dialog as today.

## Estimate

| What | Answer |
|---|---|
| Size | S — the naming rule and one branch in the write path; the whole cost lies at the input, in detecting the divergence (F-COL-034) |
| Where the main work is | kt-app (the project write path, the name of the copy), kt-otio (the document name inside the file, if we decide to introduce it) |
| Project format | does not touch it |
| Tests to write | "a file changed from outside is not overwritten but is saved beside it under a label"; "the name of the copy is deterministic and does not depend on the locale" |
| Depends on | F-COL-034 (how to detect the divergence), F-COL-013 (a guard beside the file), F-RES-001 (how many states on disk) |
| Breaks compatibility | no |

## Verdict

We take the mechanism as an EMERGENCY OUTCOME, not as a feature, and we introduce nothing of our own for it. The reconnaissance gave the most practical conclusion of the wave here: when the program detects that the file on disk has become not the one it read from, it will have three answers, and the right one is the third. Overwriting silently is not allowed (loss), asking "whose version to keep" is dishonest (we do not show how they differ), while keeping it beside the file under a label is honest always and requires neither a server, nor clocks, nor a person's name — precisely because the label is needed not for automatic resolution but so that the person can find the second version a week later.

A separate line — a finding about ourselves: **our document has no name of its own**. A foreign timeline's name we carefully carry around the loop, but our own project writes an empty string there, and the whole identity of the document rests on the file name. This makes us structurally READY for Syncthing's scheme (a label in the name — the only place that already works for us) and at the same time explains why two copies of a project are indistinguishable from the inside.

We do not introduce the server half, following the general wave formula: the conflict-resolution dialog presupposes a second participant, and a note has no right to declare one.

How we differ from neighbours. **F-COL-013** (two processes over one file) is about how NOT to let things diverge; here it is about what to do once they have diverged — a fundamentally different half. **F-RES-001** (autosave and project versions) is about how many states the program keeps on disk by its OWN will; a conflict copy is a state born of someone else's edit, and it cannot be thinned out on a schedule. **F-COL-024** (the version of a reel as a unit of review) is about the pile of RENDERS handed over; here it is about two copies of one document. **F-COL-036** is about how to compare those two copies.

## Sources

- https://docs.syncthing.net/users/syncing.html — the scheme of the name
  `<filename>.sync-conflict-<date>-<time>-<modifiedBy>.<ext>`, the outcome rule by edit time and the tie-break by the first 63 bits of the device ID, "treated as normal files" (accessed 2026-09-08)
- https://community.adobe.com/t5/team-projects/team-project-could-not-resolve-the-conflict/td-p/12174435
  — both outcomes of Team Projects failed to work (13.07.2021; accessed 2026-09-08)
- https://community.adobe.com/t5/team-projects-discussions/premiere-pro-team-project-resolve-conflicts-empty-window/td-p/11387956
  — the empty conflict-resolution window (26.08.2020, confirmed 16.12.2024; accessed 2026-09-08)
- https://support.apple.com/en-us/HT207128 — Final Cut's exclusive library lock: a second version does not arise, because the second person is not allowed to open it (accessed 2026-09-08)
- Files with the `-conflict` suffix in Blackmagic Cloud are known from forum messages, but `forum.blackmagicdesign.com` is closed by Cloudflare — a verbatim quotation with a date could not be obtained, so the fact is given with a note that the primary source is unavailable.
- Our code was checked by grep over `src/` on 08.09.2026: `kt-app/src/app.rs` (`write_project_atomic`, `autosave_path`, the `.ktproj` import, the background autosave), `kt-otio/src/convert.rs` (the timeline name from `Retained`), `kt-app/src/toolbar.rs` (the only composed file name).

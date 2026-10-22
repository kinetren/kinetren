---
id: F-COL-014
title: A lock that outlives its owner
status: candidate
priority: B
cost: S
category: collab/claim
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: no
crates: [kt-app]
invariants: []
ui_rules: [5, 19]
round: null
point: null
tests: []
depends_on: [F-COL-013]
trackers: []
services: [premiere-productions, fcp-shared-library, team-projects]
checked: 2026-09-08
---

# F-COL-014 — A lock that outlives its owner

## Essence

The program crashed, the machine was unplugged from the socket, the laptop went on vacation together with its owner — and the "busy" declaration stayed lying there. The document is held by someone who no longer exists. The main failure of pessimistic schemes is not a conflict of two edits but exactly this: work stands still, there is no one to blame, and the question sounds not "how to remove it by a timeout" but "who has the right to take it away, and what will be lost on the part of the previous owner if he is nevertheless alive".

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | there is no lock — there is nothing to survive; the price is the same as for any absent mechanism: there is no deadlock, but also no protection [NOT VERIFIED live] |
| Adobe Premiere | 26.x | `.prlock` in the production folder | after an abnormal termination the file remains; there is no standard "release" command in the program — removal is deleting the file in the file manager. Who has the right to do this, the vendor does not write; the answer to the user comes from a community MVP (quotes below) |
| DaVinci Resolve | 21 | Collaboration, a lock in the database record | the lock lives in the database, not in a file, therefore it survives not the process but the session; there is no standard procedure in the documentation for seizing it from a second user, but there are complaints about "the timeline is locked by a user who does not have it open" [NOT VERIFIED: `forum.blackmagicdesign.com` is blocked by Cloudflare, only search snippets] |
| Final Cut Pro | 12.3 | an exclusive lock of the library | Apple documents a MESSAGE ("already in use by [user] on [Mac].local"), not a procedure: what to do if the named Mac will not turn on again is not said in the help [NOT VERIFIED live] |
| After Effects | 26.x | Team Projects, a lock of the sequence on the server | there is a server, but sticking still happens anyway — because the lock is released not by "letting go" but by PUBLICATION: one who left without publishing leaves the others in view-only, and this is a design, not a network failure |

## The trick behind it

**A timeout does not cure — it breaks.** A lock that releases itself after N minutes releases it from a LIVE one: the editor stepped out for a smoke, answered a call, went to render in another program — and came back to a document that a neighbor has already taken, and his own unsaved edits became garbage. Therefore the industry in all five cases chose manual seizure. The reverse side of the choice: **not a single vendor documents WHO has the right to take it away.** For Premiere this is found out empirically and means "anyone who has write access to the production folder" — that is, the right of seizure is moved out of the program into file system rights and is not tied to roles inside the project at all.

**The second layer — there is nothing to read in the declaration in order to decide.** `.prlock` names the machine and the person, but carries neither a process identifier, nor a time, nor an updated mark of life. Therefore the program cannot even SUGGEST whether the owner is alive: the difference between "he is typing right now" and "his laptop drowned" is not expressed in the file by anything. The decision is entirely human, and the only way to make it is to call and ask. Tellingly, the typical question sounds not "how to remove it" but "am I allowed to touch this at all" (quote below): a person fears not the complexity of the operation but that it is someone else's.

**The third layer — sticking is not cured by the server either.** In Team Projects the owner is known by name, the connection is alive, and still people sit in view-only, because the lock is tied not to presence but to the state "there are unpublished edits". An owner who left without publishing and an owner who is absent are indistinguishable for the others. That is, the transition to a server moves the problem, it does not remove it: while removing the lock requires an ACTION by the owner, the absence of the owner remains a dead end.

## Pros

- Manual seizure is honest: it does not pretend that the program knows about the liveness of the neighbor, and it leaves the decision to the one who can make a call.
- A file lock is removed by tools outside the editor — a file manager, a script, the volume administrator; it is fixed even when the program does not start.
- The absence of a timeout means the absence of the class of failures "they took it away while I was at lunch", and that is more expensive than downtime: downtime is visible, the loss of edits is not.

## Cons

- A deadlock by default: if the owner is unavailable, the work stands until a person intervenes who knows about the existence of the declaration file. A user who does not know about it simply sees "cannot".
- The right of seizure is not expressed in the program, so it can be neither granted nor restricted: it coincides with the right to write into the directory.
- The one who seized does not learn what the previous owner lost, and the previous owner does not learn that it was taken away from him — until he tries to save.
- The message names the machine, not the person, and after a change of computer ("Home Desktop") it no longer helps to identify the culprit.

## What users say

The question of the right is put most precisely by someone who works alone and still does not dare to touch a file that looks like someone else's:

> Since I don't know what use the locks are, I'm not sure I'm free to delete
> them.
> — Wholeo, "Can I delete .prlock files?", Adobe Product Community, 20.12.2021,
> https://community.adobe.com/t5/premiere-pro-discussions/can-i-delete-prlock-files/td-p/12605679

The answer comes not from the vendor but from a community MVP, and it is also the whole of the industry's documentation on the question "who is entitled":

> Those are there just 'cause if someone is using them, another user can't come
> in to write to the project at the same time. For us using Productions as a
> solo operator, just delete 'em.
> — R Neil Haugen, same thread, 20.12.2021,
> https://community.adobe.com/t5/premiere-pro-discussions/can-i-delete-prlock-files/td-p/12605679

`trackers:` is empty deliberately (rule 12): the topic "a lock left by a crashed process" does not exist as a class in the exported corpus of twelve open-source trackers — joint work is represented there by one real request out of 25,442 tasks, and a filter picked to fit the form would be not a finding but decoration.

## How it fits Kinotrain

### What already exists

- `src/apps/kt-app/src/app.rs:5020` — `autosave_path()`: the ONLY artifact that we have that survives the process — the recovery file of an unnamed project. The path is fixed (`~/.local/share/kinotrain/autosave-project.otio`), the same for all instances, and contains neither a pid, nor a name, nor a time.
- `src/apps/kt-app/src/app.rs:5388` — at startup the program silently ADOPTS this file if there is no `last_project`: someone else's (or one's own past) unfinished work is opened as one's own.
- `src/apps/kt-app/src/app.rs:2814` — the very first Ctrl+S in ANY instance deletes the recovery file: `remove_file(autosave_path())`. The right to "seize and destroy" belongs to everyone and is not asked of anyone.
- `src/apps/kt-app/src/app.rs:5063` — `atomic_tmp_path`: if the program falls between the write and `rename`, `<project>.otio.tmp` remains next to the project — an orphan with a deterministic name that nobody searches for, shows or deletes; the next write will silently overwrite it with the content of the new writer.
- `src/apps/kt-app/src/app.rs:5300` — `on_exit`: the only point of "tidying up on exit". It releases nothing, but WRITES, and when the program crashes it is not executed at all — that is, it fires exactly in the case when the lock would have been released anyway.

### What is missing

- The lock itself: there is nothing to release, because there is nothing to hold it with (F-COL-013).
- A sign of liveness for any of our artifacts. Neither the `.tmp` file nor the recovery file carries a pid, a time, or the name of the machine — that is, there is no way to distinguish "a neighbor is writing right now" from "left over from a past crash" even within one machine, where the operating system does have the answer.
- A conversation at seizure. UI rule 19 requires that a dialog name the place where the decision can be returned, and today the decision "delete someone else's" is made by a person silently (Ctrl+S removes the recovery file, whoever's it is).
- A trace. We have no edit log at all: undo consists of snapshots of the whole `Project` in memory (`src/libs/kt-model/src/model.rs`), so there will be nothing to tell the one who seized what exactly he interrupted, even after the lock appears.

### What gets in the way

What gets in the way is not the code but the order of making a decision. Seizure is the only operation in the whole topic that has no correct automatic answer: the program must show WHAT is known (who, when, from which machine, whether the process is alive, if it is on this same machine) and leave the press to the person. So the price of the feature is not the file but the dialog and the wording, and we have neither a concept of a user nor the name of a machine in the document. The second obstacle is our own habit: `remove_file` without a question is already written and works, and adding a question looks like a regression of convenience until a second instance of the program appears next to it.

### In category `collab/` — two mandatory lines (rule 13)

OFFLINE: DOES NOT REMAIN — `src/apps/kt-app/src/app.rs:5020` — there is no lock in any form, so there is nothing to survive the owner; the place where the absence is visible is our only artifact that survives the process: the recovery file of an unnamed project lies at one fixed path without a name, time or owner, any second instance silently adopts it at startup, and the first Ctrl+S in any instance deletes it forever and asks no one.

NETWORK COST: to tell a fallen owner from a live one — the shared clock — liveness is expressed either by a mark that the owner updates or by the answer of the owner himself, and both forms require a coordinated time: on one machine a process identifier would be enough, but between machines only the file time remains, coming from someone else's clock, so "busy a minute ago" and "busy a year ago" become equally unprovable.

NETWORK COST: to say who is entitled to seize — access rights — today the right of seizure coincides with the right to write into the directory, and therefore is not expressed in the program in any way; to grant or restrict it, a list of people and their roles is needed, not a second place to write the same thing.

### Which invariants it touches

None of the twelve: the topic lives entirely in `kt-app`, next to the project record. Of the checklist rules in `src/apps/kt-app/CLAUDE.md` two are touched. Item 5 "an action does not switch off a neighboring state" is already violated today and without any network: Ctrl+S in one instance deletes the recovery file belonging to another. Item 19 "a dialog must say where the decision can be changed later" is the frame for a future seizure window: this is the only place in the topic where a person presses a button whose consequence is irreversible for a neighbor.

## Estimate

| What | Answer |
|---|---|
| Volume | S — one stage of the round on top of F-COL-013: the content of the declaration, reading it on opening, a window "busy — by whom, when, seize?" |
| Where the main work is | kt-app (`app.rs`: opening, the seizure window, the fate of the recovery file and of the `.tmp` orphans) |
| Project format | does not touch it: everything lives in the declaration next to it and in the dialog |
| Tests that will have to be written | a declaration left by a process that no longer exists is shown as a busy state with a date; seizure requires confirmation; Ctrl+S does not delete the recovery file of another instance (today it fails); an orphaned `.tmp` does not turn into a project |
| Depends on | F-COL-013 (without a declaration there is nothing to remove) |
| Breaks compatibility | no |

## Verdict

We take it together with F-COL-013 and exactly in its volume: a claim without a removal procedure is a dead end that we would be building for ourselves. The local half is unexpectedly rich here, because on ONE machine the liveness of the owner is verifiable (the process either exists or does not), and we already have today an artifact that survives the process, and anyone has the right to silently destroy it. That is, what needs fixing is not a hypothetical network future but today's rule "the first one to save erases someone else's recovery".

We do not introduce a timeout in any form and we record this as a decision: a lock that releases itself takes the document away from a live one and turns a visible downtime into an invisible loss of edits. The only thing that remains automatic is one message — that the process holding the claim on THIS machine no longer exists; the person presses the button.

We do not open the server half by the formula of F-PRJ-003 and F-PRJ-001: the right of seizure as a right requires a list of people, and there is no second participant.

Boundaries: the claim itself and the declaration next to the file — F-COL-013; what is allowed while it is taken by another — F-COL-015; the project's auxiliary companions and the rule for their deletion — F-COL-002; who is entitled to delete the MATERIAL — F-COL-006; recovery after a crash as such — F-RES-004; unsaved work on exit — F-RES-005.

## Sources

- https://community.adobe.com/t5/premiere-pro-discussions/can-i-delete-prlock-files/td-p/12605679
  — Adobe Product Community, thread of 20.12.2021 (access date 2026-09-08).
- https://community.adobe.com/t5/team-projects-discussions/premiere-team-projects-giving-me-quot-view-only-quot-even-though-i-am-the-only-editor-online/td-p/15055508
  — a stuck view-only with a live server, 24.12.2024 (access date
  2026-09-08).
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration —
  "until unlocked by the current user": removing the lock is described as an action of the owner (access date 2026-09-08).
- https://support.apple.com/en-us/HT207128 — Apple: the message about a busy library; there is no seizure procedure in the document (access date 2026-09-08).
- The Resolve lines about sticking locks — only search snippets: `forum.blackmagicdesign.com` is blocked by Cloudflare, so verbatim quotes with dates cannot be obtained from there; marked `[NOT VERIFIED]` in the table.
- Our code was read on 08.09.2026: `src/apps/kt-app/src/app.rs` (2814, 5020, 5063,
  5300, 5313, 5388), `src/libs/kt-model/src/model.rs` (undo — project snapshots).

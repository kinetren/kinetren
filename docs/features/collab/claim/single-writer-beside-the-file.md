---
id: F-COL-013
title: Two processes on one project file
status: candidate
priority: B
cost: S
category: collab/claim
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, sqlite-3.50]
we_have: partial
crates: [kt-app, kt-cli, kt-otio]
invariants: []
ui_rules: [12]
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [premiere-productions, fcp-shared-library, resolve-project-server]
checked: 2026-09-08
---

# F-COL-013 — Two processes on one project file

## Essence

One document, two openers. The question is not "how to merge edits" (nobody knows how to merge), but how the second
one learns that the first one is already inside — BEFORE both save. The industry answer was given long before the clouds
and works entirely without a server: next to the document a declaration file "occupied" is placed, and its existence is
the whole protocol. The editor sees it as a lock on the project icon and a caption naming whose machine holds it; nothing
more exists in the mechanism.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | a second instance opens the same `.veg` freely, nothing appears next to the project; the one who saved last wins, and this is recognized by the content, not by a message [NOT VERIFIED live: the mechanism is absent both from the feature list and from the help] |
| Adobe Premiere | 26.x | Productions, the `.prlock` file | **Productions has no server at all**: the occupation is expressed by a file `<name>.prlock` next to the `.prproj` in the shared folder. It appears when the project is opened and disappears when it is closed; inside — the machine and the user, that is, the lock names the OWNER, not the session |
| DaVinci Resolve | 21 | Collaboration, disk database / PostgreSQL | the project ceases to be a file: the RECORD of the database is occupied, and separately — the bin and the timeline. The vendor's wording: "Bins and timelines are "read only" until unlocked by the current user" |
| Final Cut Pro | 12.3 | an exclusive lock of the library | the unit of occupation is the whole library; the second person does not even get a view mode: "The document [Library name] is already in use by [user] on [Mac].local" |
| After Effects | 26.x | none at the file level | `.aep` is not occupied by anything; occupation appears only inside Team Projects, that is, with a live server — no serverless half remains at Adobe in AE [NOT VERIFIED live] |

**Outside the core.** Without SQLite one cannot explain the main thing: WHY the industry's serverless
lock is a separate file, and not a lock provided by the operating system. The library is vendored in our
own dependency tree, and its source can be read offline.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| SQLite | 3.50.2 (`libsqlite3-sys` 0.35.0, our `src/Cargo.lock`) | POSIX advisory locks in `os_unix.c` | the kernel lock is released when ANY descriptor to this file is closed within the process, and is re-established by the same process through another descriptor; SQLite works around this by keeping a count of locks per inode and deferring `close()`. Whoever cannot afford such bookkeeping puts a file next to it |

## The trick behind it

**The Productions lock is not a state of a process but DATA IN A FOLDER, and this is visible from the way it
breaks.** The `.prlock` file is copied along with the production folder: copy the directory while projects were open
— and the copy on another machine arrives already occupied, with the computer, not the Adobe ID, named as the owner
(MichaelMarkham, quote below). No server lock behaves this way — the state on the server is not copied along with the
bytes. This is not a side effect but the direct price of serverlessness: for the declaration to survive the program's
shutdown and reach another machine, it must be a file, and everything that is a file gets copied, falls into a backup and
is restored from a backup together with its contents.

**The second layer — why precisely a file and not a kernel lock.** The "real" operating-system lock is unsuitable for this
task twice over: a POSIX advisory lock dies together with the process (and, worse, when any other descriptor to the same file
is closed — see the quote from our own `sqlite3.c`), and over network file systems it does not work at all. That is, it can
neither survive the owner nor reach a second machine — precisely the two things for which a lock is set up. A file next to it can
do both, and pays for this by having to be removed by hand (F-COL-014).

**The third layer — the granularity always equals the unit of RECORDING, not the unit of editing.** Premiere writes the project
as a whole — occupies the project as a whole; Final Cut writes the library — occupies the library; Resolve keeps bins and timelines as
separate database records — occupies them separately. A clip is locked by NOBODY, and this is not an oversight: a smaller unit would
require being able to write part of a document, and not one of the five can write part of a document. Our case is defined by this:
the project is one `.otio`, so the unit of occupation available to us is exactly one — the file.

## Pros

- The mechanism works on a bare file system: a flash drive, a mounted volume, a folder inside one machine. It behaves the same everywhere, because "busy" is expressed by the existence of a file, not by a session.
- The declaration survives the program being shut down, so it can protect the scenario "the first one forgot to close it and left", not only "both sit in it at the same time".
- A lock in a directory is visible to tools that know nothing about the editor: a file manager, a script, a backup. Debugging needs neither a server log nor a user account.
- Granularity "file" costs nothing to design: it is already set by how the document is written to disk.

## Cons

- It is copied together with the content: a copy of the folder arrives busy, and the person gets a lock where nobody opened anything.
- It guarantees nothing. It is a DECLARATION: a second writer that does not check it (a foreign program, a script, our own CLI) will overwrite the file, and the lock does not even find out.
- The owner is named by machine and user, but its liveness is not expressed by anything: neither by time nor by a process identifier; hence the whole plot of F-COL-014.
- On network volumes the behavior depends on the protocol, not on the editor: `.prlock` is created and removed by ordinary file operations, and their semantics on SMB/NFS are their own.

## What users say

The most precise description of "the lock as data" comes from a person who copied a production folder and got a busy project on his own second machine:

> I tried to open one of those projects and it is locked to my Home Desktop (not
> my adobe ID) so I can't make any changes to it.
> — MichaelMarkham, "Need to take control of a locked project in a production",
> Adobe Product Community, 24.05.2020,
> https://community.adobe.com/t5/premiere-pro-discussions/need-to-take-control-of-a-locked-project-in-a-production/td-p/11155482

Why a serverless lock is a file in the industry, and not a kernel lock, is stated not by editors but in the source of the library that we link ourselves:

> POSIX advisory locks are broken by design. ANSI STD 1003.1 (1996) section
> 6.5.2.2 lines 483 through 490 specify that when a process sets or clears a
> lock, that operation overrides any prior locks set by the same process.
> — SQLite, comment in `os_unix.c`; checked against the vendored
> `sqlite3.c` (libsqlite3-sys 0.35.0, line 39919), 08.09.2026,
> https://www.sqlite.org/lockingv3.html

`trackers:` is empty deliberately (rule 12): the five editors of the core have no public trackers, and in the exported corpus of twelve open-source trackers the topic of shared work is absent as a class: one real request per 25,442 tasks. Picking a filter just to fill the form would mean fabricating a signal.

## How it fits Kinotrain

### What already exists

- `src/apps/kt-app/src/app.rs:5036` — `write_project_guarded`: a guard of ONE writer. It compares the model revision carried by the ready JSON with the revision of the last written snapshot and refuses to write a stale one. Introduced in R.32 against a concrete race: the background autosave overwrote a newer Ctrl+S.
- `src/apps/kt-app/src/app.rs:5051` — `write_project_atomic`: a tmp file next to it plus `rename`. The name of the temporary file is deterministic (`atomic_tmp_path`, line 5063): `<full name>.tmp`, without a pid and without a random part.
- `src/apps/kt-app/src/app.rs:538` — `KtApp::open`: reads the file, starts the workers, sets `saved_rev` to ZERO. No check that the file is busy by someone, no mark "now it is busy by me", no recording of the file time.
- `src/apps/kt-app/src/app.rs:1078` — the background autosave every 30 s targets the user's file (`project_path`), not a separate version.
- `src/apps/kt-cli/src/main.rs` — a SECOND writer inside our own product: `new`, `convert`, `demo`, `roundtrip` write `.otio` with a bare `std::fs::write` (lines 146, 177, 192, 222) — without a guard and even without tmp+rename.
- `src/apps/kt-app/src/app.rs:5382` — startup restores `last_project` from egui-storage, which is shared by all instances: two running kt-app processes by default open ONE AND THE SAME project.

### What is missing

- Any statement about the file on disk. The guard knows only its revision in memory, and `saved_rev` of each process starts from zero — that is, two programs over one file keep two identical numberings, and neither of them is a revision of the DOCUMENT.
- An audible refusal. `write_project_guarded` can say "did not write" (`Ok(false)`), but all three calls throw this answer away: `.map(|_| ())` in Ctrl+S (line 2805) and in the autosave thread (1093), `let _ =` in `on_exit` (5313). In Ctrl+S this directly leads to the message "Saved" on a write that did not happen.
- An owner of the temporary file. The name `<project>.otio.tmp` is the same for all writers: two processes writing one project meet on it, and `rename` carries into the document what the other one managed to append, not what you wrote.
- A name of the one who took it. The model has no concept of a user at all, and entity identifiers are issued as "maximum + 1" (`src/apps/kt-app/src/app.rs:3110`), and two of them coincide.

### What gets in the way

Nothing expensive: checking, creating and removing the declaration are all file operations next to the existing project record, in one file `app.rs`. What gets in the way is one thing: all of today's save-safety discipline is built on the assumption "exactly one writer": the asterisk in the title, `dirty_content`, `saved_rev`, the autosave into the user's file, and deleting recovery on Ctrl+S. A second writer breaks not the write itself but this assumption, and therefore a cheap change must be made whole, not halfway: a declaration that nobody reads on opening is worse than none, because it creates the appearance of protection.

### In category `collab/` — two mandatory lines (rule 13)

OFFLINE: DEGENERATES — `src/apps/kt-app/src/app.rs:5036` — we do have the claim mechanism, but it has degenerated to one process: the guard compares the model revision in ITS OWN memory, starts counting from zero at every opening, and knows nothing about the file on disk; therefore it honestly separates the background autosave from Ctrl+S inside one program, and cannot say anything about a second kt-app or about kt-cli writing the same `.otio` with a bare `fs::write` without tmp and without the guard.

NETWORK COST: the name of the one who took it — the user account — the industry does not leave empty in the declaration next to the file: inside it there is a machine and a person, and exactly this makes it readable for the second one; we have no concept of a user either in the model or in the format, and identifiers are issued as "maximum + 1" and two of them coincide, so we can declare the claim, but cannot name the one who took it.

### Which invariants it touches

None of the twelve — and this is the main argument for cheapness. The claim lives entirely in `kt-app`, next to the project record: it does not touch the engine, the time model, the cache or export. Of the checklist rules in `src/apps/kt-app/CLAUDE.md` one is touched: item 12 "background work does not get in the way": today the background autosave writes into the user's file every 30 s without asking, and it is exactly this that turns a second opener from an observer into a writer. The refusal that will have to be shown is subject to item 16 (an impossible action is an error, not a faded hint).

## Estimate

| What | Answer |
|---|---|
| Volume | S — one stage of the round: creating and removing the declaration, a check on opening, an honest answer from the guard |
| Where the main work is | kt-app (`app.rs`: `open`, `save`, autosave, `on_exit`), kt-cli (the same write path instead of `fs::write`) |
| Project format | does not touch it: the declaration is a separate file next to it, it does not get into `.otio` |
| Tests that will have to be written | a second instance over a busy file does not write and says so; the guard's `Ok(false)` is not reported as "Saved" (today it fails); the temporary file name differs for two writers; kt-cli respects the declaration of kt-app |
| Depends on | F-COL-014 (removing a declaration that outlived its owner) — without it the claim turns into a dead end |
| Breaks compatibility | no: a foreign program and our old one simply will not see the extra file next to it |

## Verdict

We take the local half, and all of it is local: the industry mechanism that has NO server AT ALL (Premiere Productions) is expressed by file operations next to the project and requires neither a second participant nor a user account. Right now it fixes our own single-process hole: kt-app and kt-cli write one `.otio` without any coordination, and two kt-app processes by default open one and the same last project. This is exactly the concrete address for the caveat of F-EXT-002 "while the project is open, an external edit is unsafe": the unsafety is expressed not by a prohibition in the documentation but by a declaration on disk.

We do not open the server half by the formula of F-PRJ-003 and F-PRJ-001: for the declaration to name a PERSON, a user account is needed, and there is no second participant. A claim without the owner's name is useful anyway — it answers the question "is it free", and only cannot answer "by whom".

Boundaries: removing a lock that outlived its owner, and the right to seize it — F-COL-014; what exactly may be done while the file is taken by another — F-COL-015; the project's auxiliary companions and the rule for destroying them — F-COL-002; autosave over the file or next to it — F-RES-001; atomicity of the write and `fsync` — F-RES-002; a project opened on another machine — F-PRJ-003.

## Sources

- https://community.adobe.com/t5/premiere-pro-discussions/need-to-take-control-of-a-locked-project-in-a-production/td-p/11155482
  — Adobe Product Community, thread of 24.05.2020 (access date 2026-09-08).
- https://www.sqlite.org/lockingv3.html — "File Locking And Concurrency In
  SQLite Version 3" (access date 2026-09-08). The quote was checked offline against the
  vendored `sqlite3.c` (libsqlite3-sys 0.35.0, line 39919).
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration —
  the wording about read-only bins and timelines (access date 2026-09-08).
- https://support.apple.com/en-us/HT207128 — Apple, a shared volume for the Final Cut library and the message "already in use" (access date 2026-09-08).
- The VEGAS and After Effects rows — from documentation and the composition of the catalogs, not checked live; marked `[NOT VERIFIED]` directly in the table.
- Our code was read on 08.09.2026: `src/apps/kt-app/src/app.rs` (538, 1078, 2805,
  2814, 3110, 5020, 5036, 5051, 5063, 5313, 5382),
  `src/apps/kt-cli/src/main.rs` (146, 177, 192, 222).

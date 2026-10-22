---
id: F-COL-017
title: Where the lock lives — in the file, next to the file or in a database
status: researched
priority: B
cost: S
category: collab/claim
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, syncthing-2]
we_have: no
crates: [kt-model, kt-otio, kt-media, kt-app]
invariants: []
ui_rules: []
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [premiere-productions, fcp-shared-library, resolve-project-server, syncthing]
checked: 2026-09-08
---

# F-COL-017 — Where the lock lives — in the file, next to the file or in a database

## Essence

Suppose the claim has been decided to be made. There remains a question that looks like a technical detail and in fact decides everything: where physically the sign "busy" lives. There are exactly three places — inside the document itself, in a separate file next to it, in the database to which the document belongs. The editor sees this from the side where it hurts: a lock that survived a crash of the program does not let him open HIS OWN project; a lock copied together with the folder arrives busy to whoever was given it; a lock that lives in the database falls off together with the database.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | none | there is no lock anywhere: `.veg` is an ordinary file, and it has no sign of busyness either inside or next to it |
| Adobe Premiere | 26.x | `.prlock` (Productions) | NEXT TO: for each busy `.prproj` a same-named `.prlock` file appears with the account name; the presence of the file is the whole lock, deleting the file releases the claim, copying the folder copies it too |
| DaVinci Resolve | 21 | a lock as a field of a record in the project library | IN THE DATABASE: the document itself lives as PostgreSQL records, the sign "busy" is the same field of the same record; there is no separate carrier, because there is no separate file either |
| Final Cut Pro | 12.3 | hidden files inside the library package | INSIDE: the library is a package-directory, and the service files of the claim lie in it too; to claim means to write into the library, and a crash leaves the record, and a second person (including yourself tomorrow) gets "already in use by …" |
| After Effects | 26.x | none | the AE project is a file, there is no lock; in Team Projects the claim belongs to the service and lives on its side, not in the document |

**Outside the core.** Without Syncthing the question "where does the state live" cannot be described: it is the only system in our registry where the choice of place is formulated explicitly and brought to a rule — state never gets into the data itself.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Syncthing | 2.x | `.stfolder`, `.stignore`, a database in the configuration directory | NEXT TO and NEVER inside: the bytes of the synchronized file are not touched at all, therefore the file has no state — the state belongs to the folder; a conflict is not prevented but is named after the fact by renaming the loser |

## The trick behind it

**A lock INSIDE the document means that "to claim" = "to write", and by this it destroys the only property for whose sake a document is made a file — the immutability of the bytes while you have changed nothing.** You opened the project — and the file on disk is already different: another length, another mtime, another checksum. All consequences pour from there: the backup system sees a change where there was no edit; the folder synchronizer spreads the "changed" file across the nodes; a comparison of two versions shows a difference that does not exist; and a crash leaves the claim inside the document forever — it can be removed only by editing the document itself. Precisely for this reason all file editors put the lock NEXT TO it, and database ones put it into the database, and none puts it inside. The only exception is Final Cut, and it is apparent: its library is not a file but a package-directory, that is, the service file still lies next to the rest, simply inside a common wrapper. The price of this exception is visible in the only form in which it is visible to the user at all: after a crash, the person is not let into his own library by his own name.

**The second layer: a lock next to the document inherits all properties of an ordinary file — both good and bad.** It is copied together with the folder (Productions carries the claim into the copy), it is not removed on abnormal exit (a crash leaves `.prlock`), it is removed by hand from Explorer — that is, a lock the program does not know is its own. The reverse side is the same: precisely because it is an ordinary file, it can be deleted without touching the work. A lock inside the document cannot be removed that way.

**The third layer, which is rarely noticed: the place of the lock determines what happens when the CLOCKS are out of sync.** Productions has a known class of failures that is cured by synchronizing the clocks of the machines: the sign of busyness lies in the file system, and the decision "busy or not" is made from its metadata, not from the content. A lock in the database does not depend on clocks at all — there the time is one, the server's. This is the second property (after byte-stability) by which the choice of place cannot be postponed "for later": it determines what exactly the program will be wrong about.

## Pros

- A lock next to the document does not touch the document: the project file stays byte-for-byte the same as it was, and all tools that know nothing about it (backup, git, folder synchronization) continue to work correctly.
- A lock next to the document is removed without the program — by the same means by which any file is deleted; in the industry this is the factual protocol of recovery after a crash.
- A lock in the database removes the question entirely: the document has no separate carrier, because there is no separate document either. At the same time the class "the lock remained, but the owner is gone" disappears: the session is visible to the database itself.
- The place of the lock can be changed without changing the document format — precisely because it is outside the document.

## Cons

- A lock next to the document is a second file, and therefore a second history of failures: not copied, copied one time too many, not removed after a crash, removed by an outsider.
- A lock next to the document does not survive repackaging: handing the project over as an archive or through a file-sharing service means handing it over together with the claim, or silently losing the claim.
- A lock in the database requires a database, that is, a server, user accounts and administration; this is the most expensive of the three places and the only one that cannot be set up alone.
- A lock inside the document has not a single advantage except apparent simplicity: it breaks byte-stability, survives a crash, and is not removed by anything except an edit of the document itself.

## What users say

A lock next to the document is an ordinary file, and it is handled as an ordinary file:

> But when I open the sequence in the copied project, a prlock file is generated
> and the name of the account pops back into the Project Locked column in the
> Productions panel.
> — lori_petersen_waite, 31.01.2023,
> https://community.adobe.com/t5/premiere-pro-discussions/productions-prlock/m-p/13543552

A lock inside the document answers with the only wording in which it is visible that you yourself took it — after a crash of the program:

> The operation couldn't be completed. Permission denied, "[library name]" is
> already in use by "[my name]" on "macintosh-d49a2006fa98.local".
> — David Shore, "\"[Library] is already in use.\"", Creative COW, 18.10.2017,
> https://creativecow.net/forums/thread/library-is-already-in-use/

## How it fits Kinotrain

### What already exists

- The document is one text file of OpenTimelineIO, and its byte-stability is checked by a test: t144 (`src/libs/kt-otio/tests/format.rs:141`) requires `write(parse(write(x))) == write(x)` both on our document and on a foreign one. Any field that changes from OPENING the file breaks this property not in theory but in a concrete assert.
- Our section in the foreign format — `metadata.kt` (`src/libs/kt-otio/src/meta.rs`): the only place where "busy" would technically fit — and at the same time the only place that is obliged to survive the round trip without changes.
- A precedent for session state INSIDE the document already exists, and there is exactly one: the region and loop (`KtTransport`, `src/libs/kt-otio/src/meta.rs:126`) plus the viewport (`ui`). Its rules are special: t96 (`src/apps/kt-app/src/layout_tests.rs:550`) requires that such "convenience edits" do NOT raise the asterisk and do NOT trigger autosave — they reach the disk incidentally, together with the nearest edit of content.
- The place "next to the project" is already occupied with us and declared a cache: `<project>.ktindex.db` (`src/libs/kt-media/src/index.rs`) on ANY opening error is deleted and created anew, and in F-COL-002 this is recorded by the decision "a sidecar will not become a place for data".
- There is no guard of the claim: `write_project_guarded` (`src/apps/kt-app/src/app.rs:5036`) compares the model revision in the memory of our process, while `flock`, `create_new`, a check of mtime and a single launch do not occur in the code even once.

OFFLINE: DEGENERATES — `src/libs/kt-model/src/model.rs:437` — the question "where does the lock live" degenerates into the already answered question "where does the session state live": the region, loop and viewport lie INSIDE the document, outside undo and outside the asterisk, and reach the disk incidentally. A claim in this same place would change the bytes of the file without changing anything in the editing, and would be saved at an unknown time — that is, certainly later than the lock needs.

NETWORK COST: the lock must lie where it is read by the SECOND person, not where it is rewritten by the first — the user account — the one who took it needs a name that survives his process: Final Cut's message contains the account and the machine name, and without them the lock does not distinguish a stranger's from one's own yesterday's.

NETWORK COST: the carrier of the claim that survives not only the process but also the repackaging of the project — project-as-database — the only companion that exists today, `.ktindex.db`, by the doctrine of the cache is deleted on any misunderstood opening error, and any new file next to it travels away and comes back together with the folder, therefore "busy" without an owner of the record lives exactly until the first copying.

### What is missing

- Nothing that would be worth setting up now: there is no lock, because there is no second participant. What is missing is a recorded DECISION about the place — without it the very first attempt to make "busy" will go by the path of least resistance, that is, into `metadata.kt`, where the region and the viewport already lie.
- Separately, what is needed even alone is missing: detection of an external change to the project file (`kt-app` does not read mtime anywhere). This is not a lock, but it is the same question — "who else wrote into my file" — and it has no relation to the network.

### What gets in the way

Not the technology, but decisions already taken that narrow the choice to one variant. Inside the document it is not allowed — there is the byte-stability of the round trip (t144) and the rule "convenience does not dirty the project" (t96): the sign of busyness must get onto the disk IMMEDIATELY, and everything that lies in this part of the document gets there incidentally, together with the nearest edit of content. Into the existing companion it is not allowed — there is the cache doctrine: `.ktindex.db` is deleted on any misunderstood opening error (F-COL-002), and a carrier of the claim in such a place would destroy itself. Into the database it is not allowed — there is no database, and it requires a second participant. Exactly one place remains: a SEPARATE declaration file next to the project, nobody's cache and not part of the document — the very one to which F-COL-013 arrives independently.

### Which invariants it touches

None of the twelve. It touches two regression tests, and both as limiters, not as the subject of editing: t144 (idempotence of the write) and t96 (convenience edits do not dirty the project and do not trigger autosave).

## Estimate

| What | Answer |
|---|---|
| Volume | S — today this is one recorded decision about the place; the lock itself does not start without a second participant |
| Where the main work is | kt-otio (the boundary of `metadata.kt`), kt-media (the rule of life of the companion file), kt-app (the only writer of the project) |
| Project format | does not touch it and must not: the place of the lock was chosen OUTSIDE the document precisely for this |
| Tests that will have to be written | none new: t144 and t96 already guard both closed doors |
| Depends on | F-COL-002 (the rule of life of the companion file), F-COL-013 (a declaration next to the file), F-COL-016 (what exactly is claimed) |
| Breaks compatibility | no |

## Verdict

We do not set up the lock — there is no second participant. But the note is needed for the sake of a decision that is cheaper to take now than to roll back later: **`metadata.kt` is closed for the sign of busyness**. The reason is not a matter of taste and is verified by a test: a claim inside the document means that opening the file changes its bytes, and for us the byte-stability of the round trip is a verifiable property of the format (t144) and the main argument for portability (F-COL-007). The second door is closed by our own decision from F-COL-002: into the existing companion `.ktindex.db` nothing may be put that is not recomputed — it is declared a cache and is deleted on any misunderstood error. The third, the database, requires that very second participant. That is, the question "where does the lock live" is settled BEFORE the task is posed and has exactly one answer — a separate declaration file next to the project; the more valuable because F-COL-013 arrives at it too, from the other side, from our own two processes.

A side conclusion, valid already today and without any lock: we are the only writer of our file only by agreement with ourselves. `write_project_guarded` guards us from our own background autosave and knows nothing about a second process, and an external edit of an open project is already possible with us (kt-cli writes the same file; F-EXT-002 honestly calls this unsafe). The cheap half here is not to occupy the file, but to NOTICE that it has been touched.

How we differ from our neighbors. **F-COL-013** (two processes over one project file) is about the fact that a declaration next to the file is needed by us already today and why a kernel lock is unsuitable for this; here the subject is already the enumeration of ALL THREE places and the proof that two doors out of three are closed by our own decisions, not by the absence of a second person. **F-COL-014** (a lock that outlived its owner) is about removal and the right of seizure, that is, about the life of the lock AFTER the place has been chosen. **F-COL-002** (the companion next to the project) is about the companion file as a cache and about its death on a shared disk; here the companion is considered as a candidate for the place, and the decision from there is used as a ready answer. **F-COL-016** is about WHAT is claimed (the unit); here it is about WHERE the sign lies. **F-RES-002** (atomic write and fsync) is about how the bytes get onto the disk; here it is about which bytes have the right to appear in the document at all. **F-RES-001** (autosave and versions) is about a second state on disk for ONE person.

## Sources

- https://community.adobe.com/t5/premiere-pro-discussions/productions-prlock/m-p/13543552
  — `.prlock` next to `.prproj`, copying the folder and deleting the file as a way to release
  the lock (31.01.2023, access date 2026-09-08)
- https://community.adobe.com/t5/premiere-pro-discussions/issue-with-productions-showing-projects-as-locked-when-they-are-not/td-p/14579493
  — "projects shown as busy although they are not busy": residual lock files and
  clock desynchronization of the machines (access date 2026-09-08)
- https://creativecow.net/forums/thread/library-is-already-in-use/ — a claim inside the
  Final Cut library package and the message with one's own name after a crash
  (18.10.2017, access date 2026-09-08)
- https://docs.syncthing.net/users/syncing.html — the model where state never gets into the
  data itself, and a conflict is named after the fact (access date 2026-09-08)
- The places and carrier types are summarized in `docs/features/_services.md`: `shared folder`
  (a lock next to the file), `own server` and `service` (a lock in the database).
- The VEGAS and After Effects rows reflect the absence of a mechanism; none of the five has a public tracker
  (`docs/features/_trackers.md`, "Whom we do NOT take").
- Our code and tests were checked by grep over `src/` on 08.09.2026: `kt-otio/src/meta.rs`
  (`KtTransport`, section `kt`), `kt-otio/tests/format.rs` (t144),
  `kt-model/src/model.rs` (`set_region`, `set_loop_enabled`, `set_ui_state`),
  `kt-app/src/layout_tests.rs` (t96), `kt-app/src/app.rs`
  (`write_project_guarded`), `kt-media/src/index.rs` (deletion of the database on an opening
  error).

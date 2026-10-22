---
id: F-COL-002
title: Service data next to the project — cache, index and lock on a shared drive
status: candidate
priority: B
cost: S
category: collab/assets
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, sqlite-3.50]
we_have: partial
crates: [kt-media, kt-app, kt-cli]
invariants: [8]
ui_rules: [12]
round: null
point: null
tests: []
depends_on: []
trackers: []
services: [premiere-productions, resolve-project-server, fcp-shared-library, syncthing]
checked: 2026-09-08
---

# F-COL-002 — Service data next to the project — cache, index and lock on a shared drive

## Essence

Every editor, apart from the project itself, has auxiliary data: waveform peaks,
thumbnails, frame caches, seek indexes, a lock file. It lies either next to the
project, next to the media, or inside the bundle, and on a single machine nobody
notices it, because it is silently regenerated. On a shared disk the first thing
to break is exactly this: not the project and not the media, but an invisible
companion that nobody counted as data and therefore nobody protected. The editor
sees it as "the timeline went from blue to red" and does not connect it with the
fact that they had just saved the project.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Peak files (`.sfk`) | peaks are stored NEXT TO THE MEDIA, not next to the project: the companion is tied to the material and moves with the media folder, not with the edit [NOT VERIFIED live: the mechanism is inherited from Sound Forge and is not described in the 2026 feature list] |
| Adobe Premiere | 26.x | Media Cache (`.cfa`/`.pek`) + Media Cache Database; `.prlock` | the auxiliary data is SEPARATED by purpose: the heavy cache goes to a separate configurable directory (by default not on the share), while next to the project on the shared volume only a lock file lies, which means one thing: "busy" |
| DaVinci Resolve | 21 | Disk database, CacheClip, Gallery stills | all auxiliary data is in directories next to the project database; on SMB3 the cache is wiped when the project is saved (see the quote), and the recommended way out is to move the database to PostgreSQL, that is, to stop being a file |
| Final Cut Pro | 12.3 | Render files, Peaks, Thumbnails inside the library bundle | the companion is hidden INSIDE the document: on a share it arrives by construction, together with the library, and therefore inherits the library's exclusive lock as a whole |
| After Effects | 26.x | Disk Cache + Conformed Media (`.cfa`, `.pek`) | the cache is placed in its own directory and is not tied to the project at all: the companion outlives the project, not the other way around [NOT VERIFIED live] |

**Outside the core.** This note could not be written without SQLite: for the five
core editors the auxiliary data is described only by its composition ("cache,
peaks, thumbnails"), while the REASON it does not work over the network is
formulated in exactly one place in the industry, in the SQLite documentation, and
precisely this mechanism is enabled in our build.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| SQLite | 3.50.2 (vendored in `libsqlite3-sys` 0.35.0, our `src/Cargo.lock`) | WAL, wal-index (`-shm`) | the write-ahead log requires all processes to share a small region of memory; this is done by mmap-ing the `-shm` file IN THE SAME directory as the database, so the ban on network filesystems is not an "optimization" but a consequence of the design |

## The trick behind it

**It is not the database that breaks, but its invisible second file.** WAL mode
keeps the pointer to the end of the log not in the database but in a separate
`-shm` file, which is mapped into memory by ALL processes that opened the
database; a network filesystem does not provide shared memory between hosts, and
therefore SQLite forbids WAL over the network not out of caution but by design.
The practical consequence, which is absent from every feature list: a database on
a share does not "work more slowly" and does not "sometimes fail" — it does not
open at all, and moreover the refusal comes because of a file whose existence the
user does not know about and has never seen in the directory.

**The second layer of the same trick is what the program does on receiving this
refusal.** Our sidecar is declared a CACHE, and the cache doctrine is carried out
literally: any opening error — a corrupt file, a foreign schema, an unavailable
lock — leads to `remove_file` and to creating an empty database anew
(`MediaIndexDb::open`, `src/libs/kt-media/src/index.rs`). On one machine this is
correct and prevents a crash. On a shared disk the same rule means that two people
who open a project from a network share delete each other's index in a loop —
without a single message, without a trace in the log, and without a slowdown that
could be noticed: the work simply never finishes, because each time it starts from
zero.

**The third layer — and it is already visible on ONE machine.** The sidecar is
designed as a companion of the project: "lies next to the project file → moves
with it" (`docs/media-index.md`), and the schema says about `path` "as in the
project (relative)". The code writes the result of resolution there, that is, an
ABSOLUTE path (`ensure_media`). A companion that is meant to move with the project
does not survive the move: after the folder is relocated, not a single `media` row
is found by its key, and the peaks, thumbnails and batch index are built again —
although the bytes are the same and `xxh3` would match character for character.
This is the proof of the value of reconnaissance: a divergence between the code and
its own documentation was found not where it was being looked for.

## Pros

- Auxiliary data kept next to the project travels with it and requires neither
  configuration nor a directory "somewhere in the system": copy the folder — and
  you take the warmth with it too.
- The doctrine "this is a cache, it must not fail because of it" removes a whole
  class of failures: a corrupt or foreign sidecar does not prevent the project from
  opening in any way at all.
- One database per project gives a natural unit of cleanup: delete the file — and
  you have deleted all the warmth of this edit, and nothing else was touched.
- Invariant 8 (scrolling and zooming do not produce decodes) rests precisely on this
  file, and therefore we have a SINGLE place where everything derived from the
  material is stored — a rare luxury: Premiere has three different mechanisms for
  this, Resolve has four.

## Cons

- **The rule "any opening error → delete and create anew" does not distinguish
  causes.** A broken database and an unavailable lock are handled the same way, and
  on a network filesystem the second cause is permanent. The self-healing mechanism
  turns into a mutual-destruction mechanism exactly at the moment when there are two
  openers.
- **The same shot is possible without a network, inside one machine.** The sidecar
  is opened independently and simultaneously by the index worker, the thumbnails,
  the opening of sources (`FfmpegSource::open_indexed`), the `reopen-media` thread,
  and by a separate PROCESS — `kt-cli` (`src/apps/kt-cli/src/main.rs`). The
  connections are short-lived; WAL and `busy_timeout` of 5 s are designed for
  contention between threads, but when the timeout is exhausted the loser does not
  wait and does not complain — it DELETES the file with which the winner is working
  at that moment.
- The row key is the absolute path, contrary to the code's own documentation:
  moving the project zeroes the entire index, although the contents of the files
  did not change.
- File identity is size and mtime (`xxh3` is computed only on insertion and when
  mtime diverges). On network filesystems mtime is exactly what lies: different
  granularity, different clocks, copying without preserving times. The fast path
  "size+mtime match → trust" is more dangerous on a share than locally, while the
  slow path is honest and costs a full read of the file.
- The user never learns about this: deletion and re-creation of the database print
  not a single line, and `spawn_index_worker` silently swallows the `Err`
  (`.ok()`) — the signal "warmth is not accumulating" does not reach the person.

## What users say

The most precise formulation of the class is the title and the first sentence of
the thread of a colorist who moved the cache to a NAS:

> if I put the cache files folder onto this NAS via an SMB3 share, merely saving
> my project trashes all of the cache files.
> — Seth Goldin, «Resolve's cache files stored on a NAS via SMB3 are trashed
> whenever a project is saved», Lift Gamma Gain, 15.09.2017,
> https://www.liftgammagain.com/forum/index.php?threads/resolves-cache-files-stored-on-a-nas-via-smb3-are-trashed-whenever-a-project-is-saved.9573/

The cause is named in the same thread and is exactly the same as Apple's in
F-COL-001: the auxiliary data relies on filesystem features that the protocol does
not have:

> The disk database would almost certainly depend on native attributes to
> function.
> — Juan Salvo, ibid., 2017,
> https://www.liftgammagain.com/forum/index.php?threads/resolves-cache-files-stored-on-a-nas-via-smb3-are-trashed-whenever-a-project-is-saved.9573/

The mechanism that makes this unavoidable is formulated in the SQLite
documentation — the very version that is vendored in our build:

> All processes using a database must be on the same host computer; WAL does not
> work over a network filesystem.
> — SQLite, «Write-Ahead Logging», section 2.1 (accessed 08.09.2026),
> https://www.sqlite.org/wal.html

A separate story is the lock as an auxiliary file: in Productions it remains on the
shared volume after a crash, and the owner cannot remove it, because cannot see it.
It is telling that in the thread the dispute is about the FILE, not about the state:
"If we open the projects on another machine the .prlock files appear. Closing the
project removes those files" (atedwards, 25.04.2024,
https://community.adobe.com/t5/premiere-pro-discussions/issue-with-productions-showing-projects-as-locked-when-they-are-not/td-p/14579493).

`trackers:` is empty deliberately (rule 12): the five core editors have no public
trackers, and in the twelve exported open-source trackers the topic of collaborative
work is absent as a class. A number that does not exist is better than an empty
field, than a fabricated filter.

## How it fits Kinotrain

### What already exists

- `src/libs/kt-media/src/index.rs` — `MediaIndexDb::open` (WAL, `busy_timeout`
  5 s, `remove_file` on any error), `ensure_media` (identity: size and mtime, on
  divergence — `xxh3`), `purge_media` (invalidation of ONLY one media_id), tables
  `media`/`packet_index`/`audio_peaks`/`thumbnails`/`poster`.
- `docs/media-index.md` — invalidation rules 1–5 and the promise "moves together
  with the project", from which the code has diverged.
- `src/apps/kt-app/src/util.rs` — `spawn_index_worker` and
  `open_source_or_placeholder`: both points open the database as an `Option` and
  silently live without it.
- `src/apps/kt-app/src/thumbs.rs`, `src/apps/kt-cli/src/main.rs` — two more
  independent openers of the same file, the second one in a separate process.
- Invariant 8 in pure form: while the sidecar is alive, scrolling and zooming do not
  produce a single decode; when it is absent — placeholders and no message.

### What is missing

- Distinguishing the causes of a refusal on opening. Today `Err(_) => remove_file` is
  the only branch; "database busy/unavailable" and "database corrupt" require
  different actions, and the first is permanent on a shared volume.
- A place for a sidecar that does not coincide with the project's place. This is
  exactly the second conclusion of the first wave (README, "user storage outside the
  project"): four features have already run into its absence, and a sidecar on a
  share would be the fifth.
- Agreement of the code with the documentation on the key of the `media` row: either
  a relative path (and then the companion moves), or an honest entry in
  `docs/media-index.md` that the key is absolute and that a move zeroes the index.
- Any signal to the person that warmth is not accumulating: UI rule 12 requires that
  background work not get in the way, but it does not forbid it from reporting.

### What gets in the way

Nothing expensive: all three fixes are inside one file and touch neither the DB
schema nor the project format. The only obstacle is that the doctrine "this is a
cache, loss is not an error" is written in the header of `index.rs` and in
`docs/media-index.md` as unconditional, and any distinction of causes looks like a
violation of it. One must distinguish not "error/not error" but "my error/someone
else's": someone else's database must not be deleted, even if one's own, in such a
state, would be subject to re-creation.

### In the `collab/` category — two mandatory lines (rule 13)

OFFLINE: DEGENERATES — `src/libs/kt-media/src/index.rs:151` — the sidecar is designed as a companion of the project and moves together with it, but the key of the `media` row is the absolute path of the resolved file, therefore moving the folder on ONE machine zeroes the entire index: not a single row is found, the peaks, thumbnails and batch index are built again, although the bytes are the same and the hash would match.

NETWORK COST: an owner of the auxiliary database instead of a file next to it (for Resolve this is PostgreSQL) — invariant 8 · project-as-database — all the warmth on which "scrolling does not decode" rests lives in one file with a WAL, and the WAL requires shared memory of all who have opened it: on a shared volume it is absent, and the file starts not to slow down but to be destroyed.

### Which invariants it touches

Invariant 8 (scrolling and zooming do not produce decode requests; waveforms come
from the peak pyramid, thumbnails from the sidecar index) — the only one, and it is
touched directly: without the sidecar the invariant is formally observed (there are
still no decodes), but the promise is broken — placeholders remain forever in place
of the pictures. UI rule 12 (background work does not get in the way and does not
jerk the interface) is the boundary that future messages must fit into.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage of a round; everything inside `index.rs` and `docs/media-index.md` |
| Where the main work is | kt-media (distinguishing causes of refusal, row key), kt-app (signal "index is not being built") |
| Project format | not touched; the sidecar schema does not change (the row key is not `SCHEMA_VERSION`, but moving the key will require a one-time rebuild) |
| Tests to write | opening a database marked read-only does not delete it; two processes that open one sidecar do not delete each other's rows; moving the project together with the sidecar keeps peaks and thumbnails (today it fails) |
| Depends on | F-COL-001 (there, about protocol requirements in general), F-RES-001 (autosave — a neighboring companion of the project) |
| Breaks compatibility | no: re-creating the sidecar is standard behavior by construction |

## Verdict

We take the local half, and it is cheap: stop deleting a database whose refusal
cause we did not understand, and bring the row key into agreement with our own
documentation. Both fixes are needed on ONE machine — the second repairs the loss of
the index when the project is moved, the first closes the race between our own two
processes (`kt-app` and `kt-cli` on the same `<project>.ktindex.db`) — and both at the
same time remove the worst network scenario, where two people silently delete each
other's warmth in a loop.

We do not open the server half: the database owner (PostgreSQL at Resolve) requires
a second participant, and there is none. The formula is the same as for F-PRJ-003
and F-PRJ-001.

Separately, to record as a decision: **the sidecar will not become a place for data.**
Everything that cannot be recomputed from the material must not be put into it — precisely
because `remove_file` on any error will remain. As soon as even one user-entered field
lands there, the cache doctrine stops being true, and the deletion rule turns into loss
of work.

Boundaries: requirements for the protocol and a shared volume in general — F-COL-001;
derivatives that are HANDED OUT to a second person (proxies, thumbnails as a product) —
F-COL-005; here the subject is the companion file itself and the rule for its
destruction; autosave next to the project — F-RES-001; atomicity of writing the project —
F-RES-002; "the material has not arrived yet" — F-COL-003.

## Sources

- https://www.sqlite.org/wal.html — SQLite, "Write-Ahead Logging", sections on
  network filesystems and `-shm` (accessed 2026-09-08). Verifiable offline against the
  vendored `sqlite3.c` in our own dependency tree.
- https://www.liftgammagain.com/forum/index.php?threads/resolves-cache-files-stored-on-a-nas-via-smb3-are-trashed-whenever-a-project-is-saved.9573/
  — Lift Gamma Gain, thread of 15.09.2017 (accessed 2026-09-08).
- https://community.adobe.com/t5/premiere-pro-discussions/issue-with-productions-showing-projects-as-locked-when-they-are-not/td-p/14579493
  — Adobe Product Community, 25.04.2024 (accessed 2026-09-08).
- https://www.blackmagicdesign.com/products/davinciresolve/collaboration —
  placement options for the Resolve database (accessed 2026-09-08).
- The composition of the auxiliary data of VEGAS and After Effects is taken from the
  documentation and the set of directories, not checked live — marked `[NOT VERIFIED]`
  directly in the table.
- Our code and documentation were verified by reading on 08.09.2026:
  `src/libs/kt-media/src/index.rs`, `docs/media-index.md`,
  `src/apps/kt-app/src/util.rs`, `src/apps/kt-app/src/thumbs.rs`,
  `src/apps/kt-cli/src/main.rs`.

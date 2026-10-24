---
id: F-RES-002
title: Atomic project write and fsync
status: done
priority: B
cost: S
category: resilience/durability
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08, sqlite-3.50]
we_have: yes
crates: [kt-app, kt-otio, kt-media]
invariants: []
ui_rules: [12]
round: "R.49"
point: 349
tests: [351, 352, 353]
depends_on: [F-PRJ-002]
trackers: [kde-bugzilla, kde-invent, pitivi]
checked: 2026-09-15
---

# F-RES-002 — Atomic project write and fsync

## Essence

The editor pressed Ctrl+S, the "unsaved" asterisk went out — and a second later the power went off. The question is exactly one: what is on the disk at the next power-on. There are three options: yesterday's project in full, today's in full, or a file with the right name and zero content. The third option is not a rare case from theory, but a line in a tracker, and it costs the whole day's work. "Saved" and "reached the disk" are two different guarantees, and neither of the usual phrasings buys the second.

## Who has it and how

The column "How it works" answers ONE question: what happens to the project file at the moment of writing. Versions, backup tiers and the history panel are the subject of F-PRJ-002; they are not reopened here, but are used as a fact.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | no name | The live `.veg` is overwritten by saving. The safety net is not a writing order but COPIES alongside: the legacy autosave `.autosave.veg` into a temporary folder and Advanced Backup in tiers (F-PRJ-002). Not a word about the temporary file, renaming or sync — neither in the help nor in the release notes |
| Adobe Premiere | 26.x | no name; indirectly — Auto Save | Auto Save writes SEPARATE files into the folder "Adobe Premiere Pro Auto-Save" and does not touch the live `.prproj` at all — that is, the safety path rearranges the task: a new file instead of replacing the old one. Ctrl+S itself overwrites the project; the writing order is not documented |
| DaVinci Resolve | 21 | Project Library (a disk database or PostgreSQL) + Live Save | The only one of the five that has NO project file: the project is a row in a database, and Live Save writes constantly. Durability is thereby delegated to a DBMS, that is, to code that knows about fsync by profession. Separately — Project Backups as ordinary files |
| Final Cut Pro | 12.3 | a library package with a database; there is no "Save" action | "Automatically saves all the changes you make as you work on a project"; backed up is "the database portion of libraries only, not the media files" (F-PRJ-002). Also delegated to the database, only inside the `.fcpbundle` package |
| After Effects | 26.x | no name; indirectly — Auto-Save, Increment and Save | Both safety paths create a NEW file: copies in the folder "Adobe After Effects Auto-Save", Increment and Save — the same name with number +1. There is no overwriting of the live file in them; nothing is documented about writing the `.aep` itself |

A line visible only when the table is assembled as a whole: **the question "will the write survive a power failure" is not asked aloud by any of the five.**
Two (Resolve, FCP) delegated it to a database; the other three bypassed it from the side — their safety net does not rewrite the live file, but puts a new one next to it. The bypass is honest but incomplete: a new file that did not reach the disk before a power failure is no different from one that was never written.

**Outside the core.** Two programs without the core are missing for different reasons. Without a database a feature cannot be described physically: three levels of guarantee are not formulated in any video-editor documentation, while in the world of DBMSs they are not only formulated but also brought out into a setting — and in addition this very code is already linked into our binary. And Kdenlive is needed as a counterexample: it is the only program whose safety mechanism is both documented and visible in the tracker — and it also gives the 21 tasks of the count below.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| SQLite | 3.50.2 (vendored in `libsqlite3-sys` 0.35.0, our `src/Cargo.lock`) | `PRAGMA synchronous` (OFF / NORMAL / FULL / EXTRA), `journal_mode`, `SQLITE_DISABLE_DIRSYNC` | Durability is a declared parameter with four levels, not a side effect. FULL means sync on every commit; the default is exactly FULL (`SQLITE_DEFAULT_SYNCHRONOUS 2` in `sqlite3.c`; the `libsqlite3-sys` build does not override it). Syncing the DIRECTORY is a separate concern, and disabling it is a separate build flag, not a side effect of something else |
| Kdenlive | 26.08.0 | a backup copy on every save, the `.backup` directory | The copy alongside is taken to the limit: a snapshot on EVERY save and tiered thinning (20/hour + 20/day + 20/7 days + 20 old), see `_editors.md`. Not a line about the writing order of the file itself. This is precisely the clean experiment: the maximum of what the "copy alongside" level gives, without level (c) |

## The trick behind it

**1. `rename` is atomic with respect to other PROCESSES, not with respect to power — and this substitution is what makes level (b) convincing.** The three levels of guarantee look like this: (a) write over the live file — a break in the middle leaves a half-torn project; (b) write to a temporary file next to it and rename it — no reader will ever see a half, because the rename either happened or did not; (c) write the temporary file, wait for its sync, rename, and sync the DIRECTORY. The difference between (b) and (c) is not visible in the code and not visible to the eye: the word "atomic" stands honestly in the documentation of `rename`, but it speaks about the visibility of the name to other processes. What lies in the data blocks under this name after a power loss, `rename` does not promise at all: writing the content and writing the name are two independent trips to the disk, and their order is a matter of the file system, not of the programmer. Hence the characteristic symptom in the trackers: the file EXISTS, the name is correct, the length is zero.

**2. The kernel covers this error — and that is exactly why nobody finds it.** ext4 has `auto_da_alloc` enabled by default: the kernel recognizes the "replace via rename" pattern and forces the delayed blocks to land before the rename is committed. The ext4 documentation explains this in plain text — it is done for applications that do not call fsync, and it "avoids the "zero-length" problem" [NOT VERIFIED live in this session: kernel.org did not open from here, the quote is from memory of the kernel documentation]. The consequence, which is absent from every list of features: on the developer's machine level (b) is NOT DISTINGUISHABLE from level (c), and they diverge where the editors' projects actually live — on an external exFAT drive, on a network share, on a mount without this heuristic. The defect is by construction not reproduced where it is sought, and is reproduced where nobody looks.

**3. We already pay for durability — but not for the right thing.** Our binary has code that syncs on every commit: the sidecar index `<project>.ktindex.db` (`src/libs/kt-media/src/index.rs`) is opened with `journal_mode=WAL` with `synchronous` at the default FULL. That is, thumbnails and the peak pyramid — data that is recomputed from the sources in minutes — survive a power loss guaranteed, while the project file, which is not recoverable from anywhere, does not. The imbalance arose not by decision but by default: SQLite has durability enabled by default, `std::fs::write` does not.

## Pros

- Level (c) makes the state "new name, foreign content" impossible by construction: after power-on the disk holds either the entire old project or the entire new one, and a third state does not exist.
- Syncing the directory closes the second, less obvious end: the rename itself is also a write, and without it after a power loss one can get both files at once (`p.otio` old plus `p.otio.tmp` new), or no new one at all.
- The price is paid for SAVING, not for editing, and does not grow with the size of the project the way serialization does: on an SSD it is a matter of a few milliseconds.
- The guarantee is verifiable by a machine. This is a rare class where a test is stronger than observation: fault injection ("a file system that was forbidden to sync") gives an answer in seconds, while in live use such a thing is caught once in several years and always after the fact.
- A copy alongside (`.bak`) is orthogonal to fsync and cheaper than it: it covers what durability does not cover at all — the user's error, a faulty serializer, a foreign program that spoiled the file. One mechanism does not replace the other, and confusing them is the most frequent error on this topic.

## Cons

- fsync waits for the hardware. On a network share and on a cheap flash drive this is not a matter of single milliseconds but of hundreds, and a synchronous save in the UI thread turns into a freeze of the interface at exactly the moment the user pressed Ctrl+S.
- Syncing a directory is not portable: on Windows a directory has no handle that can be synced with the same call. Level (c) is a platform branch, not one line, and "just add fsync" is wrong as a statement of the task.
- Durability is orthogonal to CORRECTNESS: if the serializer wrote garbage, fsync will carefully deliver the garbage to the disk. It closes the class "the project opened empty" only halfway — the second half is closed by the copy alongside.
- The effect cannot be shown. A mechanism that cannot be demonstrated in a clip, and that the user notices once in a lifetime, is the first to go under the knife when economizing — and, judging by the table above, this is exactly what happened with all five.
- The upper bound of honesty is not the program: a disk with the write cache enabled can confirm a sync before the data has actually landed. fsync is the limit of what SOFTWARE can promise, not an absolute guarantee, and more cannot be promised.

## What users say

SIGNAL: a broken or empty project file — 21 tasks in kde-bugzilla, `(corrupt|damaged|truncated|zero.byte|0 bytes|empty).*(project|\.kdenlive|\.xges|save file)`; share 0.3% against 0.0% (1 task out of 2300) in pitivi

The class is rare in all three, and at the same time it is built the same way: the broad topic "saving" (`save|autosave|backup|recover`) gives even 2.9% / 2.1% / 2.3% across kde-bugzilla / kde-invent / pitivi, while the subclass "the file broke" is a tenth of that. The second number matters more than the first: of the 21 kde-bugzilla tasks, 17 are closed, 10 of them are real fixes, and five are closed as WORKSFORME — almost a third of the closed ones were written off as "not reproducible". For a defect that depends on the file system and on the moment of power loss this is the expected outcome, and it also explains why the count is small: such tasks do not live long enough to reach a state in which they can be counted.

The third number turns this into a finding. Both sides of the count are Kdenlive: a program that keeps a copy of the project on EVERY save and thins the copies in tiers (see the table outside the core). That is, 21 tasks of the class "the project file broke" came from the one who has done more in this area than all five core editors together. A copy alongside covers a lot, and does not cover exactly that beat which is the subject here.

And a separate finding, visible only by counting: **the word `fsync` does not occur in these three trackers even once** — 0 matches across 10,863 tasks (together with the bodies of the kde-invent and pitivi tasks; the kde-bugzilla export has no bodies). `atomic` occurs four times, and all four times it is `atomicity.h` in backtraces. The user cannot name the cause, because they cannot see it; they bring the symptom. So the source for this topic is not a search by word, but a search by consequence.

The consequence, verbatim:

> When I next accesses the sequence (.xges file) it would not open.
> — Pitivi #2245, 13.09.2018, https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2245

A rescue mechanism that works against the user (the task title is "When a project RECOVERY fails don't "create" an empty project with the real
project name"):

> …after that is not possible to load the original project because kdenlive
> think that the project is already loaded (but is empty)
> — Kdenlive #705, 28.05.2020, https://invent.kde.org/multimedia/kdenlive/-/work_items/705

Three titles that give the shape of the class better than any retelling:
"Corrupted save file" (kde-bugzilla #446393, 02.12.2021, closed FIXED),
""Save" -> "Open" a specific project results in a corrupted project"
(#509101, 04.09.2025, open, CONFIRMED) and "Infinite loop when loading empty
project file via command line" (#519355, 24.04.2026, open). The last one is about the second beat of the same accident: a zero-length file does exist in nature, and the program's reaction to it has been verified by no one.

## How it fits Kinotrain
CAPABILITY: openspec/specs/durability/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Writing the project — to a temporary file alongside + fsync + rename; the outcome of the write is its own type, not a serde string.
ADDRESS: src/apps/kt-app/src/app.rs, `write_project_atomic` — the class is possible, and nothing catches it: tmp+rename without a single `fsync`/`sync_all`/`sync_data` in the whole of `src/` (0 matches by grep), t150 checks only the name of the temporary file, t194/t199 — background operation and the race of writers, and for a zero-length file `load_project_file` answers with the raw "EOF while parsing a value at line 1 column 0".

## Estimate

| What | Answer |
|---|---|
| Scope | S — one stage of the round |
| Where the main work is | kt-app (`write_project_atomic`: syncing the temporary file, syncing the directory with a platform branch), plus an error message for an empty/unreadable file |
| Project format | does not touch it |
| Tests to write | fault injection "sync forbidden": make sure that after a break between the write and the rename an INTACT old file remains on the disk; a separate test for a zero-length file — the message names the cause, not the parser's position |
| Depends on | F-PRJ-002 (the copy alongside — its subject), the neighboring note `recovery/` (panic hook) |
| Breaks compatibility | no |

## Verdict

We take it. This is the only position of the base where the full guarantee costs one function and touches none of the invariants, while its absence costs the user a whole day of work — and we already pay the same price for the thumbnail cache, which we do not regret.

Separately, and in the same stage: a zero-length file must get a message naming the cause, not the parser's position. The tracker shows that a zero file does exist in nature, and that the reaction to it has been verified by no one.

The copy alongside (`.bak`) is not brought into this note: it is cheaper and covers something else (the user's error and a broken serializer), and its subject is F-PRJ-002. Confusing them is the most frequent error on this topic, and we do not repeat it here.

## Sources

- `python3 src/scripts/tracker-rank.py --query '(corrupt|damaged|truncated|zero.byte|0 bytes|empty).*(project|\.kdenlive|\.xges|save file)'` — 21 / 6346 (0.3%) kde-bugzilla, 7 / 2217 (0.3%) kde-invent, 1 / 2300 (0.0%) pitivi (08.09.2026)
- `python3 src/scripts/tracker-rank.py --query 'save|autosave|backup|recover'` — 2.9% / 2.1% / 2.3% (08.09.2026)
- `python3 src/scripts/tracker-rank.py --theme 'data loss'` — 211 tasks, weight 333, share of real fixes among the closed ones 0.67 (08.09.2026)
- The count of `fsync` = 0 and `atomic` = 4 (all of them — `atomicity.h`/`atomic_base.h` in backtraces) was obtained by a direct pass over the dumps
  `~/.cache/kinotrain/trackers/*.jsonl` with task bodies: `--query` looks only at the title, labels and component
- https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2245 (access date 08.09.2026, from the dump)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/705 (access date 08.09.2026, from the dump)
- https://bugs.kde.org/show_bug.cgi?id=446393 · https://bugs.kde.org/show_bug.cgi?id=509101 · https://bugs.kde.org/show_bug.cgi?id=519355 (titles and statuses — from the dump of 08.09.2026; the kde-bugzilla export contains no task bodies)
- **Three of twelve trackers exported** (kde-bugzilla 6346, kde-invent 2217, pitivi 2300 records, dumps of 08.09.2026); shotcut, olive, openshot, flowblade, losslesscut, mlt, otio, blender-vse and ffmpeg-trac had not yet been exported at the time of writing the note — the note's numbers do not apply to them
- SQLite: `libsqlite3-sys` 0.35.0 from `src/Cargo.lock`, the vendored `sqlite3/sqlite3.c` (`SQLITE_VERSION "3.50.2"`, `#define SQLITE_DEFAULT_SYNCHRONOUS 2`) and its `build.rs` — checked locally, without the network
- ext4 `auto_da_alloc`: the kernel documentation, [NOT VERIFIED live in this session] — kernel.org did not open from here; the wording is given from memory and is marked as unverified
- The autosave and backup mechanisms of the five core editors were not reopened: the facts and their primary sources are in F-PRJ-002 (`docs/features/project/history/versions-and-snapshots.md`)
- None of the five editors was checked live in this note: the question "will the write survive a power failure" cannot be settled by observation in principle (see "The trick behind it", item 2)

**Implemented in R.49** — item 349
(tests t351, t352, t353).

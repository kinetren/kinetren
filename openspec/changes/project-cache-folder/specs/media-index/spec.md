## REMOVED Requirements

### Requirement: Sidecar index DB — a cache invalidated by file identity and additive tables
**Reason**: the owner's decision (kkt-26.11.20 A14): the caches leave the one SQLite file for a folder of
the project with one place per cache.
**Migration**: "The project's caches live in a folder beside it, one place per cache" and "The media
index is kept by content, one file per entry" below carry every rule and every TEST line (t16, t872, t34,
t401, t402, t401b, t639); an old sidecar is imported once and deleted.

## ADDED Requirements

### Requirement: The project's caches live in a folder beside it, one place per cache
A named project's caches SHALL live in `kinetren-cache/` in the project's folder, one folder shared by the
projects in it.
- **What it holds:** the media index, the reel previews, the persistent cache, the heard mix and the list
  of earlier renders. Each SHALL have a place of its own.
- **The persistent cache SHALL be one store** (`results/`). It holds three kinds: the composites of moments
  (the disk prerender), the results of heavy effects per layer, and the stream history. The two picture
  kinds SHALL be keyed by one layout, `durable_hash` over the plan or over a layer's sub-plan up to the
  kept stage, and stored in one manifest form; heavy sources' copies SHALL lie under the media's content
  id.
- **What it never holds:** anything the project needs to open. Deleting the folder SHALL cost only
  recomputation.
- **Marking:** the folder SHALL carry a `CACHEDIR.TAG` and a `README.txt` that says it may be deleted.
- **Fallback:** where the project's folder cannot be written at all, the folder SHALL be made under the
  user's data folder (`project-caches/`), under the same cyclic rule, and a notice, the project's facts
  and the trace SHALL name the place and the reason. The same place SHALL serve an untitled project.
- **Untitled project:** its folder SHALL move to the new place on the first save.
- **Old stores:** an old sidecar (`<name>.ktindex.db` with `-wal` and `-shm`) SHALL be imported in the
  background on the index lane and then deleted; an unreadable one SHALL be deleted after three tries.
  The former `<data>/render-cache` SHALL be deleted at the first start.
- **Cyclic writing:** a folder's room SHALL be the smaller of its cap and the disk's free space less a
  reserve. A write that does not fit, or that the system refuses for lack of space, SHALL first drop the
  least recently used entries:
  - in this order: the stream history, the heard mix, the picture results, the source copies, the reel
    previews, the media index;
  - never what this process used in the last minute.

  If nothing can be dropped, the write SHALL be skipped and counted, never failing a gesture. The first
  drop of a session in a folder SHALL post one notice naming the folder, with a way to Settings → "Caches
  on disk". Nothing SHALL scan or trim at rest.
- **The registry:** every cache folder SHALL be listed in a registry in the user's data folder, with its
  size and its projects.
- **Threads:** no cache SHALL be written by the UI thread.

Why: kkt-26.11.20 line 19 ("у нас не будет все в одной базе sqlite хранится а будет подпапка в папке
проекта с разными кешами, вот с тем же превью кэшем"). One unbounded SQLite file made a lock or a broken
write cost every cache of a project (t872), and the prerender of every project shared one cap in the
user's folder (project-cache-folder D1–D5). kkt-26.11.20b line 2 ("СУТЬ … В КЭШИРОВАНИИ ЧТОБЫ ЭФФЕКТЫ ТЯЖЕЛЫЕ НЕ
СЧИТАТЬ": the effects' results are the store's point, D1b); line 4: "непринимающая папка - это цикличесская
запись с подчисткой старого говна и алерт в уведомлениях" (D3, D4).

#### Scenario: The folder, its marks and its fallback
- **WHEN** a project in a writable folder, one in a read-only folder and an untitled one each open and cache a clip's peaks
- **THEN** the first has `kinetren-cache/` with `CACHEDIR.TAG` and `README.txt` beside it; the second and third cache under the user's `project-caches/`, and their facts and trace name why
- **TEST** t3824 (`src/libs/kt-media/tests/cache_folder.rs`), t3825 (`src/libs/kt-media/tests/cache_folder.rs`)

#### Scenario: A broken entry costs only itself
- **WHEN** an entry is truncated, another's header names a foreign kind, and a third is written by two processes at once
- **THEN** the first two are misses and are removed, every other entry still hits, and the third reads back whole
- **TEST** t3826 (`src/libs/kt-media/tests/cache_folder.rs`)

#### Scenario: The registry and a synced folder
- **WHEN** two projects in two folders are opened, and a third lies in a folder marked as a Dropbox root
- **THEN** the registry lists the three folders with their projects and sizes, and the third folder carries the "do not sync" attribute
- **TEST** t3827, t3828 (`src/libs/kt-media/tests/cache_folder.rs`)

#### Scenario: An old sidecar is imported once
- **WHEN** a project with a `.ktindex.db` from the former build is opened, and separately one with a locked and one with a garbage sidecar
- **THEN** the peaks, packet index, thumbnails, loudness, sync offsets and reel previews are read from the new folder without decoding, and the three sidecar files are gone; the locked one is kept for the next open, the garbage one is deleted, and nothing waited for the import
- **TEST** t3831, t3832 (`src/libs/kt-media/tests/cache_folder.rs`)

#### Scenario: A full folder writes in a cycle and says so once
- **WHEN** a folder with a small cap holds history, mix runs, picture results and media entries of several ages, new picture results keep arriving, a write meets `ENOSPC` from a test file system, and the loop being played was used a moment ago
- **THEN** each write first drops the least recently used entries in the tenants' order, the loop's entries stay, the `ENOSPC` write recounts the room, drops and succeeds on its retry, one notice names the folder for the session, and at rest nothing is scanned or dropped
- **TEST** t3835, t3836, t3837 (`src/libs/kt-media/tests/cache_folder.rs`)

#### Scenario: A moved, saved or copied project keeps what it can
- **WHEN** an untitled project with peaks is saved; a project folder is renamed; a project is saved as into another folder
- **THEN** the untitled cache is in the new folder after the save; the renamed folder's media hit with no file hashed again; the copy's folder holds the media and reel entries and no prerender
- **TEST** t3838, t3839, t3840 (`src/libs/kt-media/tests/cache_folder.rs`)

### Requirement: The media index is kept by content, one file per entry
The media index SHALL be kept under `media/` of the cache folder, in a folder per content id (the xxh3-128
of the whole file). A path map (path, size, modification time → content id) SHALL spare the hash on every
open; a path inside the project folder SHALL be written relative to it.

The identity rule SHALL stand:
- if the size and modification time match, the entry is trusted;
- otherwise the file is hashed: an equal content id is reused, and another is another entry.

Each entry SHALL be one file:
- with a header that names its kind, the kind's version, the length and a checksum;
- written to a temporary name and renamed.

A header that does not verify, or another version of the kind, SHALL be a miss, and the file SHALL be
removed. The rest SHALL be untouched. Further rules:
- the packet index SHALL be kept, so a second opening does not scan;
- analysis numbers SHALL be stored as integers ×100;
- a sync offset SHALL be keyed by both content ids, so a replaced file on either side misses;
- the reel previews SHALL be kept under `reels/` by (reel, revision, file fingerprint).

A cache error SHALL fall back silently to the ordinary opening. The UI thread SHALL NOT touch the index.

Why: the sidecar's rules (defect j; a schema bump wiped every media's index; f64 drifted in the last
digit; a uniform delete missed the second side of a pair) hold over files. A kind's version replaces the
whole-file schema bump, and the content id survives a renamed project folder, which the path-keyed rows
did not (project-cache-folder D1, D2).

#### Scenario: Invalidation by content
- **WHEN** the content of a file has changed; the mtime is touched without a change of content; an entry file is corrupt
- **THEN** only the entries of this media are recomputed (the neighbours are intact); the hash matched — reuse; the corrupt entry is removed and recomputed without a panic
- **TEST** t16 (`src/libs/kt-media/tests/index_db.rs`)

#### Scenario: A locked or half-written entry is a miss, not a broken index
- **WHEN** an entry is being written by another process (its temporary file present, the final name absent); then a garbage file stands under an entry's name
- **THEN** the first is a miss that leaves the writer's file alone and is reused once renamed; the garbage file is removed with no other entry touched (the SIGBUS of the former SQLite file, seen in t640 under load, has no counterpart)
- **TEST** t872 (`src/libs/kt-media/tests/index_db.rs`)

#### Scenario: The batch index survives opening
- **WHEN** the file is opened a second time with the same cache folder; the file has changed
- **THEN** the second opening does not scan, the index is byte-for-byte equal to a fresh one and functionally equivalent (keyframe_before, sample-accurate audio); the changed file is rescanned
- **TEST** t34 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: Analysis entries are exact, invalidation is two-sided
- **WHEN** loudness and sync offset are written and read; the file is replaced
- **THEN** the round trip is byte-exact (integers ×100); for the replaced file the loudness and ALL pairs involving it miss, a foreign pair still hits; the observer of the pyramid sees every burst and does not change it
- **TEST** t401, t402, t401b (`src/libs/kt-media/tests/index_db.rs`)

#### Scenario: The reel preview is keyed by content (container: capability `compositions`)
- **WHEN** a reel without a file puts frames and peaks into the cache folder
- **THEN** the key is (reel, revision, file fingerprint); any divergence → None, one version per reel; removing a media's entries does not touch the reels
- **TEST** t639 (`src/libs/kt-media/tests/index_db.rs`)

#### Scenario: A renamed project folder still hits
- **WHEN** a project folder holding its media is renamed and the project is opened again
- **THEN** every media entry hits through its relative path line, and no file is hashed
- **TEST** t3829 (`src/libs/kt-media/tests/cache_folder.rs`)

#### Scenario: An untitled project caches its media
- **PATH** File → New → drag two clips into the bin: their waveforms and filmstrips draw → close the bin and open it again: they draw at once, with no Loading
- **WHEN** clips are added to an untitled project
- **THEN** peaks and thumbnails are read back from the user folder's cache after the bin is reopened, with no decode
- **TEST** t3830 (`src/apps/kt-app/src/layout_tests.rs`)

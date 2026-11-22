# Proposal: project-cache-folder

> DESIGN (spec session, 2026-11-20; revised 2026-11-22). The owner decided the direction (kkt-26.11.20 A14)
> and answered its questions (kkt-26.11.20b B2): one folder per project folder, cyclic writing with a notice
> when it cannot take more. Code starts on the orchestrator's go. Tests reserved: t3824–t3843.
> Base: `orch-26-11-01` at `5e7a8ea9`.

## Why

The owner, answering Q23/Q24 (kkt-26.11.20, line 19): "давай сделаем следующее - у нас не будет все в одной
базе sqlite хранится а будет подпапка в папке проекта с разными кешами, вот с тем же превью кэшем".

What the program keeps today, and where:

| Cache | Where | Bound |
|---|---|---|
| Media index: peaks, packet index, thumbnails, posters, loudness, sync offsets | ONE SQLite file beside the project, `film.kt` → `film.ktindex.db` (+ `-wal`, `-shm`) | none |
| Reel previews (strips, frames, peaks) | the same SQLite file | 48 frames a reel, pruned to the reels in use |
| The preview's disk prerender (video segments) | `<data>/render-cache/v2-<build>/<device>/`, the user's data folder, shared by every project | 2 GiB in total |
| Datamosh history, the heard mix (designed: `durable-history-and-mix`) | planned beside the prerender in `<data>` | planned 1 GiB |
| Compiled pipelines, pass times | `<data>/shader-cache/` | 4 files, 8192 lines |

Four things follow from that layout:
- the biggest cache of a project, its prerender, does not live with the project;
- it is shared and evicted across all projects, so one heavy project pushes out another's loop;
- the media index is one file with no bound, so a broken write or a lock on it costs every cache at once
  (t872 was written after a SIGBUS from that file);
- media rows are keyed by the absolute path string, so renaming the project folder makes every row a miss.

## What Changes

- **One cache folder per project folder** (the owner's Q1 a): `<project folder>/kinetren-cache/`, shared by
  the projects in that folder, with one place per cache. It holds:
  - `media/` — per media file, by its content;
  - `reels/` — the reel previews;
  - `results/` — THE PERSISTENT CACHE ("Постоянный кэш", the owner's B1) as one store: the composites of
    the preview's prerender (moved from `<data>`), the results of heavy effects per layer, and the Datamosh
    history, under one key layout (`durable_hash`) and one manifest form (design D1b);
  - `mix/` — the heard mix of `durable-history-and-mix`;
  - `renders.tsv` — earlier renders found again by `cached-final-render`;
  - `media/<content id>/copy-*` — heavy sources' copies (the persistent cache's source half, change
    `disk-copies`).

  Each cache keeps its own files and versions. A broken or locked file in one costs only that entry.
- **No SQLite.** Each entry is a file written to a temporary name and renamed, with a header that names
  its kind, version, length and checksum. A torn or foreign file is a miss and is removed.
- **Media are found by content, paths by a small map.** `media/<content id>/` is named by the xxh3-128
  of the whole file, which the index already computes. A map `media/paths.tsv` (path, size,
  modification time → content id) spares that hash on every open. A path inside the project folder is
  written relative to it, so a moved or renamed project folder still hits.
- **The folder announces itself:** a `CACHEDIR.TAG`, so backup tools (tar, borg, restic) skip it, and a
  `README.txt` that says it can be deleted at any time. Where a cloud-sync tool allows it, the folder is
  marked as not synced.
- **Cyclic writing when the folder cannot take more** (the owner's B2):
  - a folder's room is its cap (Settings, default 20 GB) or the disk's free space less a reserve, whichever
    is smaller;
  - a write that does not fit first drops the oldest entries, least recently used, in the tenants' order;
  - a notice says once per session, per folder: "The cache of <folder> is full: the oldest results are
    replaced", with a button to Settings → "Persistent cache".
  - The page lists every cache folder with its size and projects, a Delete button and the cap.
- **A folder that cannot be written at all** (read-only, a write test fails): the cache goes to
  `<data>/project-caches/`, under the same cyclic rule. The notice and the project's facts say why. An
  untitled project caches there too, and its cache moves to the new folder on the first save.
- **Migration:**
  - an existing `<name>.ktindex.db` is read once in the background into the new layout, then deleted;
  - `<data>/render-cache/` is deleted at the first start, since every build discarded it anyway (its
    folders are salted by the build).
- **Nothing a project needs lives there.** Deleting the folder costs time, never work. Recovery,
  autosave, posters, logs, the trace, crash reports, compiled pipelines and pass times stay where they
  are: they are durability or per-machine data, not the project's caches.

## Capabilities

### New Capabilities
- none

### Modified Capabilities
- `media-index`: the cache folder, its tenants, the media index without SQLite, migration, cyclic writing
  and the fallback.
- `video-plan`: the prerender's store moves into the cache folder under the folder's cycle.
- `telemetry`: per-tenant sizes, hits, misses and evictions in the trace; `kt-cli trace cache`.
- `ui-reach`: Settings → "Persistent cache".

## Impact

- **kt-media:** `index.rs` (`IndexDb`) is replaced by `cache_folder.rs`, which provides:
  - the folder and its registry;
  - `MediaStore` for peaks, packets, thumbnails, posters, loudness and sync;
  - `ReelStore` for the reel previews;
  - the entry header, the atomic write and the import of the old sidecar.
  - `rusqlite` leaves kt-media's dependencies.
- **kt-engine:** `render_cache.rs` takes its root from the cache folder and writes in the folder's
  cycle (the room asked before a write, the oldest dropped).
- **kt-app:**
  - `index_db_path` becomes `cache_folder`;
  - the lanes keep their roles: index, thumbs, media and reel-preview;
  - the Settings page; the untitled project's move on save.
- **kt-cli:** `--cache-dir DIR`, `kt-cli cache <project> [size|prune|clear]`.
- **kt-dirs:** `project_caches_root()`, `cache_folders_registry()`.
- **Invariants:**
  - 4: the bars still draw what the stores hold;
  - 5: each tenant is written by the lane that writes it today, never the UI thread;
  - 8: waveforms and thumbnails come from the media tenant;
  - 11: the export reads no tenant except the lossless history tier, as `durable-history-and-mix`
    decides.
- No project format change.
- The durability capability's rule that the program writes no file into the person's project tree
  (posters, R.58) is amended by the owner's decision for this one folder.

## The owner's answers (kkt-26.11.20b, lines 3–4, row B2)

- **Q1 a — "Одна на папку (Recommended)".** One `kinetren-cache/` per project folder, shared by the projects
  in it.
- **Q2 and Q3 are replaced by one rule:** "непринимающая папка - это цикличесская запись с подчисткой старого
  говна и алерт в уведомлениях". A folder that cannot take more keeps writing in a cycle: the oldest
  entries are dropped to make room, and a notice says so. Design D3 and D4 carry it:
  - a full disk, a quota or the folder's cap all count as "cannot take more";
  - a folder that cannot be written at all (read-only, a write test fails) has nothing to cycle. Its cache
    goes to the user's data folder under the same cyclic rule, and the same notice names why.

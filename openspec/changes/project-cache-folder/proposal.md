# Proposal: project-cache-folder

> DESIGN ONLY (spec session, 2026-11-20). The owner decided the direction (kkt-26.11.20 A14, line 19);
> code starts when the orchestrator gives the go after his answers to Q1–Q3. Tests reserved: t3824–t3843.
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

- **One cache folder per project folder:** `<project folder>/kinetren-cache/`, with one place per cache
  (Q1 for the sharing). It holds:
  - `media/` — per media file, by its content;
  - `reels/` — the reel previews;
  - `render/` — the preview's prerender, moved from `<data>`;
  - `history/` and `mix/` — the durable tiers of `durable-history-and-mix`;
  - `renders.tsv` — earlier renders found again by `cached-final-render`;
  - room for the durable preview store that change `proxies` is becoming.

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
- **A disk limit with one list** (Q2): Settings → "Caches on disk" lists every cache folder the program
  made, with its size, its project and whether that project still exists. Each folder has a Delete
  button. The limit is enforced at rest, the prerender, history and mix first and the media index
  last.
- **Where the project folder cannot hold it** (Q3: read-only, a removable or network drive, or an
  untitled project), the folder is made under `<data>/project-caches/`, and the project's facts and the
  trace say so. An untitled project's cache moves to the new folder on its first save.
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
- `media-index`: the cache folder, its tenants, the media index without SQLite, migration, the limit and
  the fallback.
- `video-plan`: the prerender's store moves into the cache folder under the folder's limit.
- `telemetry`: per-tenant sizes, hits, misses and evictions in the trace; `kt-cli trace cache`.
- `ui-reach`: Settings → "Caches on disk".

## Impact

- **kt-media:** `index.rs` (`IndexDb`) is replaced by `cache_folder.rs`, which provides:
  - the folder and its registry;
  - `MediaStore` for peaks, packets, thumbnails, posters, loudness and sync;
  - `ReelStore` for the reel previews;
  - the entry header, the atomic write and the import of the old sidecar.
  - `rusqlite` leaves kt-media's dependencies.
- **kt-engine:** `render_cache.rs` takes its root from the cache folder, and its cap from the folder's
  limit.
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

## Questions to the owner

**Q1. One cache folder for each project, or one for the whole folder the projects lie in?**
Your words: "подпапка в папке проекта с разными кешами". Today `film.kt` has `film.ktindex.db` beside it,
and the prerender of every project is in one place in your user folder.
- **a) One per project folder, shared by the projects in it** (`kinetren-cache/`). Two versions of a
  film in one folder, `film.kt` and `film-v2.kt`, share every thumbnail, waveform and most of the
  prerender, because the keys are the content, not the project. Deleting one project does not free its
  part; Settings shows the folder as a whole. *(Recommended: your folders usually hold versions of one
  edit, and the disk is short.)*
- **b) One per project** (`film.kinetren-cache/`). A project and its cache go together, and deleting
  them is obvious. Two versions of one film compute and store everything twice.

**Q2. The disk limit: per folder, or one total for all projects?**
Today the prerender has 2 GiB in total for all projects, and the media index has no limit.
- **a) One total** (Settings: "Caches on disk may take up to N GB", default 10 GB). When it is
  exceeded at rest, the folders of the projects opened longest ago are trimmed first and the open
  project last. Within a folder the prerender, history and mix go first and the media index last.
  *(Recommended: the number you set is the number the disk loses, however many projects you have.)*
- **b) A limit per folder** (default 4 GB each). Simple, but ten projects take 40 GB, and the list in
  Settings is the only way to see it.

**Q3. Where does the cache go when the project's folder cannot take it?**
A read-only folder, a USB stick or a network share. A cloud-sync folder can take it, but would upload
gigabytes.
- **a) Into your user folder** (`<data>/project-caches/<folder hash>/`), with the project's facts saying
  so. A synced folder takes it with the "do not sync" mark where the sync tool has one (Dropbox does,
  OneDrive does not). *(Recommended: the program works the same everywhere, and you see where the cache
  went.)*
- **b) No disk cache for such a project:** everything is computed again after a restart.

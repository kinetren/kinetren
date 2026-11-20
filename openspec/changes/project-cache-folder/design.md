# Design: project-cache-folder

## Context

- **The sidecar** (`src/libs/kt-media/src/index.rs`, `IndexDb`):
  - one SQLite file in WAL mode, `<project>.with_extension("ktindex.db")`;
  - tables: `media` (path, size, mtime ns, xxh3-128 of the whole file, probe), `packet_index`,
    `audio_peaks`, `thumbnails`, `thumb_meta`, `thumb_pixels`, `poster`, `loudness`, `sync_offset`,
    and the reel tier's `reel_strip`, `reel_frame` and `reel_peaks`;
  - versions: `SCHEMA_VERSION = 2` (a bump wipes the file), `THUMB_PIXELS_VERSION = 1`,
    `STRIP_ROW_VERSION = 3` (`src/apps/kt-app/src/reel_preview.rs`);
  - rows are keyed by the path string as passed; it has no bound;
  - each lane opens its own connection: index (peaks, loudness), thumbs, the media lane (packet
    index through `FfmpegSource::open_indexed`) and reel-preview;
  - an untitled project has no sidecar (`index_db_path()` is `None`, `src/apps/kt-app/src/app.rs`).
- **The prerender** (`src/libs/kt-engine/src/render_cache.rs`):
  - the folder is `<data>/render-cache/v<FORMAT=2>-<xxh3(build)>/<xxh3(device)>/`, holding segments
    with `.idx` manifests;
  - the cap is 2 GiB (`KT_RENDER_CACHE_MB`), least recently used out;
  - other builds' folders are removed at open;
  - the `render-cache` thread writes it and the `render-cache-read` thread reads it;
  - a file leaf in its key is the project-stored path string, with the size and modification time.
- **Planned tenants:**
  - `durable-history-and-mix`: Datamosh history entries, LZ4 per the owner's Q24 b, and the kept mix
    runs;
  - `cached-final-render`: the list of earlier renders;
  - `proxies`: the durable preview store the owner asked for in place of proxies.
- **Not caches:**
  - the recovery file and autosave (durability);
  - posters and the recent list (the user store);
  - the log, the trace, crash reports, locks;
  - `shader-cache/` (compiled pipelines and pass times). These are per machine and device, not per
    project.

## D1. The folder and its layout

`<project folder>/kinetren-cache/` (Q1 a; under Q1 b, `<stem>.kinetren-cache/`):

```
kinetren-cache/
  CACHEDIR.TAG                    the Cache Directory Tagging signature: backup tools skip the folder
  README.txt                      "Kinetren keeps recomputable data here. Delete it any time."
  lock                            flock for the limit pass and map compaction (never for an entry)
  media/paths.tsv                 path · size · mtime ns · content id   (append-only, compacted)
  media/<content id>/             peaks · packets · thumbs · thumb-pixels · poster · facts · loudness
  pairs/<id a>-<id b>             sync offsets, the pair ordered
  reels/<content hash>/           strip · frames · peaks          (STRIP_ROW_VERSION in the header)
  render/v<FORMAT>-<build>/<dev>/ segments + manifests            (moved from <data>/render-cache)
  history/<build>/<dev>/          stream entries, LZ4             (durable-history-and-mix)
  mix/<build>/                    kept mix runs, raw f32          (durable-history-and-mix)
  renders.tsv                     earlier renders, verified       (cached-final-render)
  usage.tsv                       per tenant: bytes, last used    (for the limit pass)
```

- **`content id`** is the xxh3-128 of the whole file in hex, the identity the sidecar already computes.
  A file copied under another name shares the entry. Two projects in the folder share it under Q1 a.
- **`paths.tsv`:**
  - a path inside the project folder is stored relative to it, so a moved folder hits and is not
    hashed again; any other path is stored absolute;
  - lines are appended, one write each, and a torn last line is ignored when read;
  - compaction happens under `lock` when the file is more than twice its live lines.
- **Each entry file** has a 32-byte header:
  - magic `KTC1`, the kind (4 bytes), the kind's version (u32) and the payload length (u64);
  - the xxh3-64 of the payload, and reserved space.
  - It is written to `.<name>.<pid>.tmp`, then renamed. A header that does not verify is a miss: the
    file is removed and the entry recomputed. A kind's version replaces the old `SCHEMA_VERSION` wipe:
    only that kind's entries miss.
- **Writers** are the lanes that write today, so invariant 5 is untouched:
  - the index lane writes peaks, loudness and pairs;
  - the thumbs lane writes thumbnails and posters;
  - the media lane writes packets;
  - the reel-preview lane writes reels;
  - the render-cache thread writes `render/`;
  - the history and mix writers of `durable-history-and-mix` write theirs.
  - The UI thread touches nothing. Two processes may write the same entry; the content is the same
    (keyed by content), and the last rename wins.

## D2. Reading and invalidation (the sidecar's rules, kept)

- A media file is opened as follows:
  1. look up `paths.tsv` by the path (relative form first);
  2. if the size and modification time are equal, use its content id;
  3. otherwise hash the file:
     - an equal content id is reused, and a new map line is appended;
     - a different one points at another directory, while the old entry stays for another path that
       still has it.
- Invalidation needs no `DELETE`: a changed file has another id. Pairs name both ids, so a replaced file
  on EITHER side misses (t401b's rule holds by construction).
- Analysis numbers stay integers ×100 (t401). The packet index is read back byte for byte (t34).

## D3. The limit (Q2) and the registry

- **The registry:** `<data>/cache-folders.tsv` lists every cache folder the program made or opened:
  path, project paths seen in it, last use, and its size at the last pass. The Settings page reads it.
- **The limit pass** runs at rest, under the prerender's quiet rule: no input for 1.5 s, not playing, no
  export. It also runs once after a project opens. It takes `lock` per folder.
- **Under Q2 a** it trims across folders: least recently used folder first, the open project's folder
  last. Under Q2 b it trims each folder alone.
- **Within a folder** the tenants go in this order: `render/`, `history/`, `mix/`, then `reels/`, then
  `media/`. Inside a tenant the least recently used entry goes first, by `usage.tsv`, never by the file
  system's access time (often off).
- The prerender's own rule stays: it never evicts what this process used.
- `KT_RENDER_CACHE_MB` keeps working as a cap on `render/` for tests and the live check.
- Other builds' `render/`, `history/` and `mix/` subfolders are removed at open, as today.

## D4. Where the folder cannot be (Q3) and the untitled project

- **The folder is tried in the project's folder first:** a test file is written and renamed there. If
  that fails, or the folder is on a read-only mount, the cache goes under
  `<data>/project-caches/<xxh3(project folder path)>/`. The project's facts and the trace name the place
  and the reason.
- **A folder inside a known sync root** is marked as not synced where the tool reads a mark:
  - Dropbox reads the extended attribute `com.dropbox.ignored`;
  - the facts say "inside a synced folder (OneDrive): the cache is uploaded" when no mark exists.
- **An untitled project** caches under `<data>/project-caches/untitled-<session>/`. On the first save the
  folder is renamed into the new place on the same file system, merged entry by entry when the target
  exists. On another file system the media and reel tenants are copied in the background, and the rest
  is dropped. A session's untitled folder left by a crash is removed at the next start.

## D5. Migration

- **An old sidecar** (`<stem>.ktindex.db`, with `-wal` and `-shm`) found at open is imported by the
  index lane in the background:
  1. read each table;
  2. write the entries (the content ids are in its `media` table);
  3. delete the three files.
  - An unreadable or locked sidecar is left for the next open; after three failed imports it is deleted
    (it is a cache).
  - Until the import ends, a miss in the new folder is computed as usual. Nothing waits for the import.
- **`<data>/render-cache/`** is deleted at the first start of the build. Its folders are salted by the
  build, so the new build could not read them anyway.
- **The plans of `durable-history-and-mix` and `cached-final-render`** take their roots from the cache
  folder; neither is built yet.

## D6. A moved, renamed or copied project

| What the person did | What happens |
|---|---|
| moved or renamed the whole project folder | the cache moves with it; relative map lines hit; nothing is recomputed |
| moved only the `.kt` into another folder | that folder's cache starts empty and fills by content; the old folder shows in Settings as "no project here" |
| Save As into another folder | the media and reel tenants are copied there in the background (small, costly to recompute); render, history and mix are not |
| copied the folder to another machine | the media and reel tenants hit; render, history and mix are other device or build, so they miss and are removed at open |
| deleted `kinetren-cache/` | everything is recomputed when needed; nothing is lost |

## D7. Class walk: every file the program writes, and where it belongs after this change

| Writer | Today | After | Why |
|---|---|---|---|
| media index (peaks, packets, thumbs, posters, loudness, sync) | `<stem>.ktindex.db` | `kinetren-cache/media/`, `pairs/` | the owner's decision |
| reel previews | `<stem>.ktindex.db` | `kinetren-cache/reels/` | same |
| prerender segments | `<data>/render-cache/` | `kinetren-cache/render/` | "вот с тем же превью кэшем" |
| Datamosh history, heard mix (planned) | `<data>` | `kinetren-cache/history/`, `mix/` | the same rule for a new kept result |
| earlier renders list (planned) | `<data>` | `kinetren-cache/renders.tsv` | per project |
| the durable preview store (`proxies`, planned) | `<proxy folder>` | `kinetren-cache/` (its own tenant) | one place; the change `proxies` names it |
| compiled pipelines, pass times | `<data>/shader-cache/` | unchanged | per machine and device, shared by every project |
| recovery, autosave | beside the project / `<data>` | unchanged | durability, not a cache |
| posters, recent list, templates | `<data>` user store | unchanged | between projects |
| trace, logs, crash reports, locks | `<data>` | unchanged | per run |
| downloaded FFmpeg, models, fonts | `<local>` | unchanged (`download-library`) | per installation |
| reel previews of an untitled project | memory only | `<data>/project-caches/untitled-*` | they now survive until the first save |
| the frames cache, present memo, kept mix in memory | memory | unchanged | per process |

## D8. Telemetry

- Every tenant reports to the trace:
  - its bytes at open and after each limit pass;
  - its hits and misses;
  - each eviction with the tenant and the reason (limit, foreign build, broken header).
- `kt-cli trace cache` prints them per tenant, beside the prerender's records, which keep their kinds.
- The project's facts name the folder, its place (project folder / user folder) and why.

## D9. Stages and tests

1. The folder, its registry, the entry header and the atomic write; the fallback (t3824–t3828).
2. The media and reel tenants; the sidecar's scenarios rewritten over files (t16, t34, t401, t401b, t639,
   t872 keep their numbers); the import of an old sidecar (t3829–t3832).
3. The prerender's root and cap from the folder; deletion of `<data>/render-cache` (t3833, t3834).
4. The limit pass and the order of the tenants (t3835–t3837).
5. The untitled project's move on save, Save As, a moved folder (t3838–t3840).
6. The trace and `kt-cli cache` (t3841); Settings → "Caches on disk" (t3842); the live scenario
   (planned live scenario `cache_folder`) and the idle check: the program at rest after a limit pass
   costs nothing (t3843).

## Risks

- **Many small files on a slow or network disk:** a thumbnail strip is one file per media, not per
  frame, and peaks are one file per media. The count grows with the media, not with the frames.
- **A person deletes the folder while the program runs:** writers recreate it; readers miss.
- **Antivirus scanning on Windows:** new files in a project folder are scanned. Entries are written once,
  and the trace's write timings show the cost.

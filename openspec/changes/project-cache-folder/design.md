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
  - `disk-copies`, being revised by spec-features into the **persistent cache** ("Постоянный кэш",
    kkt-26.11.20b B1): the results of heavy effects and copies of heavy sources, on disk.
- **Not caches:**
  - the recovery file and autosave (durability);
  - posters and the recent list (the user store);
  - the log, the trace, crash reports, locks;
  - `shader-cache/` (compiled pipelines and pass times). These are per machine and device, not per
    project.

## D1. The folder and its layout

`<project folder>/kinetren-cache/`, one per project folder (the owner's Q1 a, kkt-26.11.20b B2):

```
kinetren-cache/
  CACHEDIR.TAG                    the Cache Directory Tagging signature: backup tools skip the folder
  README.txt                      "Kinetren keeps recomputable data here. Delete it any time."
  lock                            flock for the cycle's drops and map compaction (never for an entry)
  media/paths.tsv                 path · size · mtime ns · content id   (append-only, compacted)
  media/<content id>/             peaks · packets · thumbs · thumb-pixels · poster · facts · loudness
  pairs/<id a>-<id b>             sync offsets, the pair ordered
  reels/<content hash>/           strip · frames · peaks          (STRIP_ROW_VERSION in the header)
  media/<content id>/copy-<q>.mkv a heavy source's copy at quality step q (persistent cache, D1b)
  results/v<FORMAT>/<device class>/   THE PERSISTENT CACHE OF RESULTS ("Постоянный кэш", D1b):
    frames/                       composites of moments, segments + manifests (the disk prerender)
    layers/                       a layer's picture after a heavy node, a track stage's or a reel's output
                                  (persistent-cache D1–D4)
    history/                      stream entries (Datamosh), LZ4      (durable-history-and-mix)
  mix/v<FORMAT>/                  kept mix runs, raw f32          (durable-history-and-mix)
  renders.tsv                     earlier renders, verified       (cached-final-render)
  usage.tsv                       per tenant: bytes, last used    (for the cycle)
```

- **`content id`** is the xxh3-128 of the whole file in hex, the identity the sidecar already computes.
  A file copied under another name shares the entry. Two projects in the folder share it.
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
  - the render-cache thread writes `results/…/frames/` and `results/…/layers/`;
  - the persistent cache's copy job writes the source copies (a background job, change `persistent-cache`);
  - the history and mix writers of `durable-history-and-mix` write theirs.
  - The UI thread touches nothing. Two processes may write the same entry; the content is the same
    (keyed by content), and the last rename wins.

## D1b. The persistent cache is one store (the owner's B1; agreed with spec-features)

The owner: "мне не нравится почему ты думаешь только о проигрывании и декоде - А КАК ЖЕ ЭФФЕКТЫ??? У НАС СУТЬ
ПО БОЛЬШЕЙ ЧАСТИ В КЭШИРОВАНИИ ЧТОБЫ ЭФФЕКТЫ ТЯЖЕЛЫЕ НЕ СЧИТАТЬ!" (kkt-26.11.20b line 2). So the program's
kept results live in ONE store, the persistent cache, designed by spec-features in change `persistent-cache`
(formerly `disk-copies`; branch `claude/spec-features-224d3d`, commit 1e31056c, design D1–D4, D8). This
change hosts it. Its kinds lie side by side in the cache folder:
- `results/v<FORMAT>/<device class>/frames/`: the composites of moments (the disk prerender of
  `render-cache-disk`);
- `results/…/layers/`: a layer's picture after a heavy node, a track stage's or a reel's output;
- `results/…/history/`: stream entries (`durable-history-and-mix`);
- `media/<content id>/copy-<quality>.mkv`: heavy sources' copies.

**The keys.**
- Frames and stage results are keyed by `render_cache::durable_hash`. It covers the plan's fields without
  content serials, `Leaves::digest` of the files' identities and the generators' values. A frame's key
  covers its whole plan. A stage result's key covers the sub-plan up to and including that stage, so an
  edit after it, or on another layer, keeps the entry valid.
- This is the layout `cached-final-render` names (the video-plan delta "The durable key's layout is a
  shared contract").

**What makes an entry stale, without the build** (agreed with spec-features). The owner updates often,
and a build salt would empty the store on every update, against B1.
- **The folder:** `results/v<FORMAT>/<device class>/`. The store's format and the device class (adapter and
  driver) are folders, so another device's entries are never even read.
- **The entry key adds:**
  - the decoder's major (`avcodec_version()`);
  - the versions of the kernels that ran: each plugin's version and its WGSL text's hash;
  - `COMPOSITE_PATH_VERSION`, a constant of kt-engine/kt-gpu that the composite code OUTSIDE the kernels
    bumps (the fusion, `plan_chain`, the core passes, the conversions). A guard test fails when those files
    change and the constant does not, so it is narrower than the build and cannot be forgotten.
- A key that no longer matches is a miss, and the entry is removed when met. There is no sweep at open.

**The manifest forms:**
- frames and stage results: video segments of the machine's hardware encoder with a text manifest per
  segment (codec, encoder, size, rate, salt, one key per frame);
- history: LZ4 of exact entries (it must be lossless: `durable-history-and-mix` D9);
- copies: MKV with a manifest of the original's identity and timestamps.

**Eviction by value per byte** (agreed with spec-features). The history is not evicted first: Datamosh
history is exactly the heavy work B1 is about.
- Within `results/` and `mix/` there is one order: value per byte, the measured cost saved per read × the
  reads in this project ÷ the bytes, lowest first, with least recently used as the tie-break.
- The source copies go by the same value rule.
- When the folder cannot take more (D3, the owner's cyclic rule), that order is applied. Frames this
  process used in the last minute are never dropped.

**Who decides what:**
- this change: the folder, the places, the entry header, the salt's form, the folder's room, the cycle
  and the notice;
- `persistent-cache`: which stages and sources are heavy enough to keep, the value measure, how a kept
  stage result enters the plan in place of its stages, the job and the switch "as in the preview";
- `durable-history-and-mix`: the history entries and the mix runs.

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

## D3. Cyclic writing and the registry (the owner's B2)

- **The registry:** `<data>/cache-folders.tsv` lists every cache folder the program made or opened: path,
  project paths seen in it, last use, and its size at the last count. The Settings page reads it.
- **A folder's room** = min(its cap, the disk's free space − a reserve).
  - The cap is per folder: "Persistent cache" in Settings, default 20 GB.
  - The reserve is max(2 GB, 2 % of the disk), so a cache never fills the disk the person's work is on.
  - `KT_RENDER_CACHE_MB` caps the picture results alone, for tests and the live check.
- **The cycle.** A writer asks the folder for room before it writes an entry. When the entry does not fit,
  the folder drops entries until it does:
  - first `results/`, `mix/` and the source copies in one order, the lowest value per byte first (D1b, the
    measured cost saved per read × reads in this project ÷ bytes; least recently used breaks a tie);
  - then `reels/`, then `media/`, least recently used first, by `usage.tsv` (never the file system's access
    time, often off);
  - never a frame this process used in the last minute, so the cycle cannot eat the loop being played;
  - if nothing droppable remains, the write is skipped and counted. A cache never fails a gesture.

  The drop runs on the writer's own thread, under `lock`, and never on the UI thread.
- **A write that fails for lack of space** (`ENOSPC`, `EDQUOT`) is the same event: the folder recounts
  its room from the disk, drops, and retries once.
- **The notice** ("алерт в уведомлениях"): the first drop of a session in a folder posts one notice:
  - "The cache of «<folder>» is full (<size>): the oldest results are being replaced.";
  - a button opens Settings → "Persistent cache";
  - the same folder posts again only after a restart, or after the cap or the free space changed by more
    than 10 %.

  The trace records every drop with its tenant, its bytes and why (the cap, the disk, a quota).
- **At rest** nothing scans or trims: the cycle runs only when something writes, so the program at rest
  costs nothing.
- Nothing is swept at open: an entry whose salt no longer matches (D1b) is a miss and is removed when met.

## D4. A folder that cannot be written, a synced folder, the untitled project

- **The write test:** a test file is written and renamed in the project's folder at open. If that fails,
  or the mount is read-only, there is nothing to cycle. The cache goes to
  `<data>/project-caches/<xxh3(project folder path)>/`, under the same room rule (D3). The notice says
  "The project's folder takes no files: its cache is in your user folder", and the project's facts name
  the place.
- **A folder inside a known sync root** is marked as not synced where the tool reads a mark:
  - Dropbox reads the extended attribute `com.dropbox.ignored`;
  - where no mark exists (OneDrive), the facts say "inside a synced folder: the cache is uploaded".
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
- **The old `<data>/render-cache/`** is deleted at the first start of this change's build. It lies outside
  any project, and its folders are salted by the whole build. From then on no store is swept: a stale salt
  is dropped lazily on a miss (D1b).
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
| prerender segments | `<data>/render-cache/` | `kinetren-cache/results/…/frames/` | "вот с тем же превью кэшем" |
| heavy layers' results (persistent cache, B1) | none | `kinetren-cache/results/…/layers/` | "СУТЬ … В КЭШИРОВАНИИ ЧТОБЫ ЭФФЕКТЫ ТЯЖЕЛЫЕ НЕ СЧИТАТЬ" |
| Datamosh history, heard mix (planned) | `<data>` | `kinetren-cache/results/…/history/`, `mix/` | the same rule for a new kept result |
| earlier renders list (planned) | `<data>` | `kinetren-cache/renders.tsv` | per project |
| heavy sources' copies (persistent cache, B1; formerly `proxies`) | `<proxy folder>` | `kinetren-cache/media/<content id>/copy-*.mkv` | one place, keyed by the original's content |
| compiled pipelines, pass times | `<data>/shader-cache/` | unchanged | per machine and device, shared by every project |
| recovery, autosave | beside the project / `<data>` | unchanged | durability, not a cache |
| posters, recent list, templates | `<data>` user store | unchanged | between projects |
| trace, logs, crash reports, locks | `<data>` | unchanged | per run |
| downloaded FFmpeg, models, fonts | `<local>` | unchanged (`download-library`) | per installation |
| reel previews of an untitled project | memory only | `<data>/project-caches/untitled-*` | they now survive until the first save |
| the frames cache, present memo, kept mix in memory | memory | unchanged | per process |

## D8. Telemetry

- Every tenant reports to the trace:
  - its bytes at open and after each drop;
  - its hits and misses;
  - each drop with the tenant and the reason (the cap, the disk, a quota, a foreign build, a broken header).
- `kt-cli trace cache` prints them per tenant, beside the prerender's records, which keep their kinds.
- The project's facts name the folder, its place (project folder / user folder) and why.

## D9. Stages and tests

1. The folder, its registry, the entry header and the atomic write; the fallback (t3824–t3828).
2. The media and reel tenants; the sidecar's scenarios rewritten over files (t16, t34, t401, t401b, t639,
   t872 keep their numbers); the import of an old sidecar (t3829–t3832).
3. The prerender's root and cap from the folder; deletion of `<data>/render-cache` (t3833, t3834).
4. Cyclic writing, the order of the tenants and the notice (t3835–t3837).
5. The untitled project's move on save, Save As, a moved folder (t3838–t3840).
6. The trace and `kt-cli cache` (t3841); Settings → "Persistent cache" (t3842); the live scenario
   (planned live scenario `cache_folder`) and the idle check: the program at rest after cyclic writing
   costs nothing (t3843).

## Risks

- **Many small files on a slow or network disk:** a thumbnail strip is one file per media, not per
  frame, and peaks are one file per media. The count grows with the media, not with the frames.
- **A person deletes the folder while the program runs:** writers recreate it; readers miss.
- **Antivirus scanning on Windows:** new files in a project folder are scanned. Entries are written once,
  and the trace's write timings show the cost.

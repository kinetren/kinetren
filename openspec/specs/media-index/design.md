# Media index and decode — why it is this way

## What hurt and for whom

The old engine shared one decoder between prefetch and playback through a mutex and drove it with
seeks (needReset/reopen cycles); one puller thread drew both audio and video, and the slow
video decode starved the sound. Coverage lived in separate ms buckets, not tied to the
cache, and lied in four ways. Audio seek kept a coarse byte index, rebuilt
on every launch, with reopening of the decoder and the resampler on every jump. Scrolling and zooming
the timeline produced a storm of decodes for thumbnails and waveforms, and in the finished Rust version the same
class returned in a negative form: without negative caches for thumbnails, FFmpeg opens reached
1.6k/s while the mouse moved.

The owner set a cross-cutting requirement: "no own FFmpeg build and no
plugins" — only the system FFmpeg. Linking with libav* gave a binary tied to the
major version of the build machine (`libavcodec.so.60: cannot open shared object file` on a system
with FFmpeg 7).

Later another thing hurt: the index service started a new thread for every call and kept silent about
errors (a person saw placeholders without a word about the cause); auto-ducking was computed in the
UI thread and froze the window for tens of seconds; a vanished file dropped the clip from the
composition, and the preview hung in "Loading" forever.

## Decisions and why

- **FFmpeg through dlopen, types by version modules.** Bindgen generates only the types and
  constants from the vendored headers of 6/7/8, the functions are resolved via `dlsym`, the decoder
  implementation is included in each version module — the layout of the structures always matches
  the loaded library. One binary for all majors with all hardware accelerations of the
  system FFmpeg; the build does not require the libav* dev packages.
- **One thread owns the decoder.** Live playback goes only through the `MediaWorker` queue
  with priorities and generation cancellation; playback does not wait for the decode. The offline path
  (index, analysis, export, kt-cli) opens its own source under three conditions — it is not
  shared, it does not promise a pace, it is finite and cancellable. An honest formulation instead of "all clients
  through the queue", which had been untrue.
- **The cache is the only truth about coverage.** An interval set inside the cache, quality in the key,
  post-FX video is a separate kind, invisible to the bars: the invariant "bar = pre-FX data"
  is kept, and yet expensive chains are memoized.
- **Prefetch is stateless and follows the playback grid.** wanted − coverage → submit, and
  wanted is computed by the same code that the render reads (the warp window, the raw reverse window). If they diverge — an eternal miss.
- **The sidecar DB as a cache.** Batch index, peaks, thumbnails, analysis — next to the project; a loss
  is not an error. The file identity is size/mtime/xxhash, and only the affected part is rebuilt.
  New tables are additive, because a schema bump wipes everything for all media of the project.
- **The stub is an ordinary source.** `PlaceholderSource` lives in the ordinary worker: the edit does not
  break when a file vanishes, the render goes on, and the return of the file is a substitution on the fly.
- **A job and a service are different types.** `BgJob` (progress, pause, cancel, `Drop` stops the thread)
  and `IndexService` (one thread, a queue, errors go outward). Analysis is a gate "one task",
  deduplication, a negative cache, and display does not order it.
- **Loudness is computed on the pyramid's pass.** The observer sees every burst that indexing
  reads anyway; a second full decode for the sake of LUFS would be minutes on a one-hour file.
- **Synchronization is by the pyramid, not by the file.** Transients are common to different microphones,
  and the pyramid is already in memory; therefore it is not in the background subsystem — instant work
  does not need a progress window.

## Rejected and why

- Linking with libav*, our own FFmpeg build, plugins — rejected by the owner; a binary on
  someone else's major simply did not run.
- A shared decoder under a mutex — rejected: a ping-pong of seeks between prefetch and playback (a defect
  of the old engine h).
- A separate coverage bookkeeping — rejected: the ms buckets diverged from the cache keys and the
  eviction (defect g).
- Caching audio post-FX — rejected: FX is cheaper than decode, and the state of FX belongs
  only to the audio-render thread.
- Reconfiguring a source as a `RequestKind` — rejected: a seek in the middle of a parameter drag
  would silently cancel it by generation cancellation; it travels as a separate field of the queue.
- A stub when media is added to the project — rejected: a typo in the path would silently become
  a stub; "error = error" is kept there, and a separate helper substitutes the stub.
- A bump of `schema_version` for the new tables — rejected: it wipes the batch index, peaks and
  thumbnails of all media of the project.
- f64 in the analysis tables — rejected: the round trip through the DB must be byte-exact, otherwise
  "recomputed" is indistinguishable from "changed".
- Synchronization inside the background analysis subsystem — rejected: it is computed over the pyramid in
  milliseconds without a decode; a progress window would be a ceremony.
- "Shift 0" on uncertainty and a clamp to zero on alignment — rejected: both look like
  success, while the materials stay out of step; synchronization also does not trim or link —
  these are separate decisions of the editor.
- MT decode of VP9/AV1 — rejected: unstable under frequent seeks; `threads=1`.
- Automatic retry of a failed analysis task — rejected: the program would hammer
  an unavailable file forever.

## Non-obvious details

- `src/libs/kt-media/src/ffmpeg.rs`: two INDEPENDENT demuxers per file (audio and video do not
  fight over seek) plus a scan pass of the batch index without decode. Audio seek — a container
  BACKWARD seek to the indexed packet with a margin of 3 packets and a FULL recreation of the decoder and
  swr; a FIFO with an absolute anchor by the pts of the first frame after the seek — this is how
  sample accuracy is reached (FLAC exact, AAC — zero phase shift). Video: backward seek = seek +
  recreation of the decoder ALWAYS (cheaper than catching artefacts of MT H.264); forward ≤4 s —
  a continuous decode without seek.
- `src/libs/kt-media/src/ffmpeg.rs`: `unsafe impl Send` is correct only because the instance is owned by exactly one worker (`&mut self`); `Sync` is deliberately not implemented.
- `src/libs/kt-ffmpeg/src/source_impl.rs`: the choice of frame when the grids do not match is floor
  and only floor (t112). Previously the forward path took ceil (the previous frame was already released), and
  the export went `f0, f1, f0, f2…` — the user saw backward steps.
- `src/libs/kt-ffmpeg/src/lib.rs` (`iostats`): the counters opens/packet_reads are global
  monotonic atomics, the UI differentiates them by time; t37 prevents the panel from again showing
  dead series.
- `src/libs/kt-engine/src/worker.rs` (`pick_next`): at equal priority — requests
  FORWARD from the cursor, the nearest first; backward — only when forward is empty. The playback key
  becomes the START of the span, and the span top-up runs over the priorities — otherwise Playback singletons
  degenerated spans into "one frame at a time" and the coverage stopped growing (t27). Keys beyond EOF —
  the worker's negative cache.
- `src/libs/kt-engine/src/worker.rs` (`worker_loop`): the retune of the source lies in a
  separate field of the queue, is processed FIRST, before `pick_next`, and the source's cache is cleaned there too —
  the race "the old frame was written back after the cleanup" is removed by construction (t501).
  `content_version` is separated from `project_version`: prefetch wakes up, the renderers are not
  rebuilt.
- `src/libs/kt-engine/src/player.rs` (`replace_media`): the order is mandatory — shut down the
  worker → `invalidate_source` → raise the new one; otherwise the old worker writes the placeholder frame
  after the invalidation. A change of the set of sources rebuilds the player's renderer too, because the
  natives are known only to the OPENED source (`probe_json` is not filled in the sidecar): without this
  a returned file did not change the decode step until the first edit of the timeline (t316).
- `src/libs/kt-cache/src/lib.rs`: quality is part of the key, and the decode step enters THROUGH the quality (a frame at step 2 lies under the key 2×Quality). The consequence for the UI: the bars
  must ask the coverage by the DECODE quality of the clip (`clip_decode_quality` in
  `src/libs/kt-engine/src/video.rs`), otherwise a warmed clip looks cold.
- `src/libs/kt-cache/src/lib.rs`: `contains` does not touch the LRU and the epoch (t27) — a probe without
  a read that refreshed the recency evicted what had been honestly read. `unused_evictions` counts
  records without `had_hit`, evicted OR overwritten (t37). `invalidate_source` goes by the
  `by_source` index, not linearly over the cache — it is called on every keystroke in the generator
  window (t497). Pin excludes from eviction, but not from coverage.
- `src/libs/kt-engine/src/audio.rs` (`merge_burst_ranges`): the ranges of one media are collapsed;
  in meaning this is the same set of bursts, so no test moved, and
  885 coverage requests under the mutex became one (t287). The pre-roll does not pull the micro-event:
  `hard_seek` clamps it by the local position.
- `src/libs/kt-engine/src/audio.rs` (`wanted_audio_bursts_raw`): the raw reverse path plays
  the events dry and linearly, it does not know the warp; the Playback prefetch at `dir < 0` must compute
  the window by this code — a warp window over a slowed event covers another pass of the source,
  and the miss was eternal, up to 1024 frames per skipped burst (t285).
- `src/libs/kt-media/src/ffmpeg.rs` (`open_indexed`): the batch index is written by ANY `open_indexed` after a fresh scan, so already the second opening of the same file (the player, thumbnails, a restart) goes without a scan pass. DB errors are cache errors: a silent fallback
  to the ordinary `open`.
- `src/libs/kt-media/src/index.rs`: the connections are short-lived, WAL + busy_timeout 5 s
  cover the competition of threads; the UI thread does not touch the DB. Additive tables are created
  with `IF NOT EXISTS` at every opening and appear in already existing DBs.
- `src/libs/kt-media/src/index.rs` (`purge_media`): `sync_offset` is keyed by a PAIR, and
  a uniform `DELETE WHERE media_id` does not see the record where the replaced file is the second one —
  the synchronization computed from different sound would remain (t402). The `reel_*` tables this
  purge does not touch: the replacement of a file is caught by the media fingerprint in their key (capability `compositions`).
- `src/libs/kt-media/src/lib.rs` (`PEAK_BLOCK_FRAMES = 256`): a block of the peak pyramid —
  256 frames of 48 kHz (5.3 ms); this is both the limit of the waveform's resolution at any zoom, and
  the step by which the sync by waveform is computed. Changing the block means recomputing the peaks of
  all projects and the sync goldens (t413–t415).
- `src/libs/kt-media/src/index.rs` (`build_observed`): the observer of the pyramid does not change
  the pyramid itself (t401b); the true peak is computed by polyphase only around samples comparable
  with the largest — without the gate an hour-long file hit this sum on every opening of the project.
- `src/libs/kt-media/src/sync.rs` (`offset_from_pyramids`): `a[i]` is compared with
  `b[i+lag]`, a positive lag = "B later" (t414); the first block is compared with itself, and not with zero —
  otherwise every record had a false "onset" at zero, and the correlation of two noises at
  zero shift jumped to 0.98 (t415); the starting level is chosen BY LENGTH, not "the topmost" — at the top there are one or two blocks, and a full search would not have happened at all.
  The confidence threshold 0.30 — below it there is no answer.
- `src/libs/kt-model/src/ops.rs` (`sync_by_audio_commands`): the translation "material →
  timeline" only through `clip_timeline_at`/`clip_material_at` — one's own arithmetic on `source_in` would drift on reverse and on a trimmed start (t417); beyond the start of the project —
  `OutOfRange`, not a clamp (t417b).
- `src/apps/kt-app/src/thumbs.rs`: the keys of `thumbnails` are an exact PK (media, t); "nearest"
  only through `thumbnail_near` with the tolerance as a queue parameter; `ThumbKey` must not be extended —
  a shared cache bin↔timeline (t62). `SourceLru` with capacity 4 instead of `clear()` at >2 — with ≥4
  media the thumbnails reopened the file for every key (t71). Everything that may fail for the
  worker must publish the failure (t69) — otherwise there is a re-order on every UI frame, and
  the counter of FFmpeg opens grows in proportion to the FPS of the interface. The map "event tick → source
  frame" is one (`material_at`) for the filmstrip, the waveform and the miniatures — it knows about reverse.
- `src/apps/kt-app/src/util.rs`: `open_source_indexed` is left strict ("error = error") for adding media and polling permissions in the render menu; the stub is substituted
  only by `open_source_or_placeholder`. `IndexService` reopens the DB only when the path changes.
  In t399 the deadline is deliberately generous: the `synth:` source lasts an hour (`SYNTH_DURATION` in
  `src/libs/kt-media/src/factory.rs`), the test asserts "the queue does not lose work", and not
  "the service is fast" — speed is judged by the release build.
- `src/libs/kt-media/src/placeholder.rs`: the duration of the stub is 1000 hours (it does not know
  how long the file lasted, and must not cut the edit); `fps: None` — the project's grid plans it;
  the frame is built once per quality and cached, it does not depend on time; non-ASCII →
  "?", so that the frame does not depend on the system fonts (t208).
- `src/libs/kt-media/src/generator.rs`: "no sound" = `has_audio:false` AND `Ok(silence)`;
  an `Err` would have landed in the worker's negative cache as a false underrun.
- `src/libs/kt-media/src/factory.rs`: parsing of pseudo-paths only here and
  case-insensitively; before the factory `synth:Ramp` in live scenarios was taken as a missing
  file, and the stub made this silent breakage visible. `can_retune` must match
  `MediaSource::retune` exactly (t499).
- `src/apps/kt-app/src/analysis.rs`: `is_running` is held by a live task, not by "the window is open" — otherwise the window after the finish would heat the system (a repeat of defect t38);
  the stale guard in `src/apps/kt-app/src/app.rs` (`on_analysis_done`) — by revision AND by the presence of the media (t421).
- `src/apps/kt-app/src/bg.rs`: the progress sender OWNS the body of the task, rather than borrowing it — consumers such as export require `Fn + Send + Sync + 'static`, and a reference cannot give such a promise. Cancellation releases the pause (t398b), otherwise a task cancelled on pause would hang forever together with the progress window.
- `src/libs/kt-engine/tests/offline_sources.rs` (t422): the TEXT of invariant 5 and the feasibility of its conditions (`Drop` of the task and of the service) are guarded, not the code — a relative of "catalog keys nobody uses".

## R.67: single image and the truth about alpha

- **A still is recognized by the DEMUXER** (`image2`, `*_pipe`), not by duration: `image2`
  honestly reports 0.04 s, and by length a picture is indistinguishable from a very short video.
- **No seek is made on a still.** It was precisely the attempt at seek that left the stream without a single frame —
  "out of range" at any tick, that same eternal "Loading…". A picture has one frame: it is given
  at any requested tick, the duration is "practically infinite", as for the stub.
- **`has_alpha` is asked of the format descriptor.** It had been nailed to `false`, and the composite,
  which can handle alpha, did not know about it: a PNG was laid down as an opaque rectangle (the defect
  F-MED-003, measured back in R.52).
- **The path of the added media is made absolute.** Relativity is a property of the RECORD and
  is computed from the project file; the writer took a path relative to the launch directory for one relative to the project, and `sources/logo.jpg` turned into `sources/sources/logo.jpg`.

## Deferred

- Relinking of sources and collecting the project into a folder (`docs/features/media/organization/
  relink-and-consolidate.md`, candidate): there is no command in the model for editing a path, `RemoveMedia`
  is closed by the guard "media in use", and a new `AddMedia` gives a new id — a pair of commands with the effect
  of relinking does not exist. Material for relinking BY CONTENT already lies in the DB
  (xxh3-128 in `ensure_media`).
- The list of encoders in the render dialog does not match what will open: `h264_vaapi`/
  `h264_qsv` without a hw context never open, and "Auto" on Intel/AMD silently falls
  back to libx264 (`docs/features/export/pipeline/hardware-encoder-selection.md`; capability `export`).
- Found along the way and not fixed: `warp_prefetch.rs` (t17) fills the legacy field
  `automation` when `fx_auto` is empty, and the migration is not called in the tests — it has no curve
  at all, and it under-tests after the move of automation.

## Windows: the user's FFmpeg is found, or downloaded on a click (windows-build, 2026-10-26)

- **What hurt.** The loader knew only Linux SONAMEs and the unix `dlopen`; Windows has no
  "system FFmpeg" that `LoadLibrary` would find. The owner kept the rule "no FFmpeg build of
  our own" and asked for a download of gyan.dev's shared build (2026-10-22).
- **A pure search plan** (`src/libs/kt-ffmpeg/src/search.rs`) over an injected `Env`, with the
  planned OS's string rules, so the Windows plan is tested on Linux (t844, t845). On Windows
  FOLDERS are searched in a fixed order (chosen folder, `KINETREN_FFMPEG_DIR`, next to the exe,
  the download folder, `PATH` with winget's `Links` followed, scoop/winget/`C:\ffmpeg`); the
  order of folders beats the order of majors — the person's choice wins over a newer FFmpeg
  someone else put on `PATH`. Every rejected folder keeps its reason: a static build
  (`winget install Gyan.FFmpeg` is one), FFmpeg 9, a missing sibling DLL.
- **One folder, dependency order** (avutil → swresample → swscale → avcodec → avformat), by
  full path with `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS`, and
  `GetModuleFileNameW` must place every module in that folder: Windows binds an import by base
  name to whatever namesake is already loaded (t846, the real 8.1.2 build under wine with a
  decoy on PATH).
- **A failure is not cached, a success is** (t847): a person who just installed or located
  FFmpeg must not have to restart; opening media never re-walks PATH (only `rescan()` does).
- **The download is pinned** to `ffmpeg-8.1.2-full_build-shared.7z` with a SHA-256 in our
  source (t848): gyan.dev's "latest" was already FFmpeg 9, and the `.sha256` next to the
  archive on the same server proves nothing. The GitHub mirror is byte-identical. Only the DLLs
  and the licence are extracted, the folder appears whole (`.partial-<pid>` → rename).
- **Rejected:** shipping FFmpeg DLLs in the zip (the owner's rule; GPL duties);
  `SetDllDirectoryW` or editing the process `PATH` (process-global, changes how wgpu and cpal
  load their DLLs); the "latest" link; shelling out to curl/7-Zip.
- **Invariant 10 on Windows** is the PE import table: only Windows system DLLs, no `av*`/`sw*`,
  no mingw runtime (the guard in `release-build.sh`).

## FFmpeg 9 (ffmpeg-9, 2026-10-27)

- **What hurt.** FFmpeg 9.0 "Lei" (2026-08; 9.0.2 on 2026-09-18) bumped every library major
  (avutil 61, swresample 7, swscale 10, avcodec 63, avformat 63). A Linux system that moved to it
  opened no media; on Windows the folder winget and gyan.dev's "latest" give was refused.
- **Headers from the signed tarball, by a script** (`src/scripts/vendor-ffmpeg-headers.sh`,
  `vendor/ffmpeg-9/SOURCE`): the same tarball and hash the `ffmpeg-test` stage builds (distribution),
  so the types that compile `v9` and the libraries that test it are one file. 6–8 were copied by
  hand and have no record; not re-vendored (working modules).
- **One row, one module, an exhaustive dispatch.** `rescan_with` matched `_ => V6`: a major in the
  table without a module would have bound FFmpeg-6 layouts to another major's libraries — memory
  corruption, not an error. Now every major is named and anything else is an error; t862 compares
  each row of `search::MAJORS` with its module's `LIB*_VERSION_MAJOR`. The shared source compiled
  against 9 unchanged; a removed field would be handled in the version module, never by a runtime
  branch on the major.
- **The download is a ROOT** (`kt_dirs::ffmpeg_download_root`): every version folder in it is a
  candidate, newest first by numeric order (t864) — moving the pin to 9.0.2 must not take away the
  8.1.2 a person already downloaded; `.partial-<pid>` entries are never candidates, nothing is
  deleted. The FFmpeg window offers Download while the pin's folder is absent, whatever is loaded
  (t867), and a download over a loaded FFmpeg says "after a restart".
- **The pin moved to gyan.dev 9.0.2** (owner, 2026-10-26: "9.0.2, CI tests 9 only"): both URLs
  byte-identical, SHA-256 `4d2060a8…`; dist-check keeps the version and hash equal in
  `ffmpeg_setup.rs`, `kt-dirs`, `fetch-ffmpeg-win.sh` and `ci.yml` (t860).
- **Rejected:** migrating (renaming) an old download folder — a lie about its content; deleting
  it after a new download — its DLLs may be loaded, and nobody asked; a `dispatch!` macro over the
  enums — four majors are still readable as arms, and it would touch every call site; a generic
  "newest types for any major ≥ 6" — layouts differ between majors, that is why the modules exist.

## A locked sidecar is not a broken one (t872, 2026-10-27)

- **What hurt.** t640 died now and then with SIGBUS (signal 7) inside SQLite —
  `walIndexRecover` on the `reopen-media` thread, in `MediaIndexDb::open` — under load and on
  base code alike. The sidecar is open in several threads of one process at once (index worker,
  thumbnails, the reopen of a returned file, reel previews). Under load an opening outwaited its
  5 s busy timeout and got `database is locked`; `open` took ANY error for "broken" and deleted
  the main file under the live connections. The next opening created a new main file next to
  the OLD `-shm`; POSIX locks of one process do not conflict, so SQLite believed it was the
  first user and truncated that `-shm` — and a connection that still had it mapped read past its
  end. With the busy timeout cut to 1 ms the same chain showed as "disk I/O error" and lost
  records.
- **The rule.** Only a file that is not a database, is corrupt or has a foreign schema
  (`SQLITE_NOTADB`, `SQLITE_CORRUPT`, `SQLITE_ERROR` from our own statements) is recreated, and
  then with its `-wal` and `-shm` (a stale `-wal` would be replayed into the new file, a stale
  `-shm` is what gets truncated). Busy/locked, I/O and "cannot open" are returned to the caller
  as a cache miss — every caller already treats a DB error that way (rule "DB errors are cache
  errors"); the index worker tries to open again on its next task.
- **Rejected:** a longer busy timeout (only moves the threshold, and the delete-on-error path
  stays a loaded gun); one shared connection for the process (the threads are independent by
  design — a decoder per thread, invariant 5 — and a shared connection would serialise them).
- **Open end.** After the fix, one `finish.sh` run (whole workspace plus live scenarios on 32
  cores) still saw t640 and t795 re-render the reel on the second opening — a plain assertion,
  no SIGBUS; two further whole-workspace runs and three kt-app runs did not repeat it. The
  reel-preview job opened the sidecar with `.ok()` and lost its disk tier SILENTLY; it now logs
  the opening error (`reel preview: sidecar open failed: …`), so the next occurrence names its
  cause. The busy timeout is also set before the `journal_mode` pragma (hygiene: no statement
  runs without a busy handler — not shown to be the cause).

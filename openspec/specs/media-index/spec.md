# Media index and decode

## Purpose

Media sources and all their upkeep: the system FFmpeg through dlopen, ownership of the decoder by
one thread, the cache of bursts as the only truth about coverage, prefetch along the playback grid,
the sidecar index DB (peaks, thumbnails, batch index, analysis), and offline analysis of the material — so that
not a single UI gesture generates a decode on demand.

## Requirements

### Requirement: FFmpeg — system copy, loaded at runtime, the binary is not linked with libav*
Decoding and muxing SHALL go through libavcodec/libavformat of FFmpeg majors 6/7/8/9, INSTALLED on the
user's machine and loaded at runtime on first use: the major is determined by
`avcodec_version()`, the struct types are taken from the version module (bindgen over the
vendored headers, generated for the target platform). The table of library majors per release
MUST equal the `LIB*_VERSION_MAJOR` constants of that release's vendored headers. The binary MUST NOT link against libav* and
MUST NOT bring its own FFmpeg build, plugins or an external `ffmpeg` binary at runtime. One
executable works on systems with any of the supported majors and uses all
hardware acceleration of the FFmpeg installed on the machine.

On Linux the libraries SHALL be opened by SONAME (`libavcodec.so.63/62/61/60` …) from the newer
majors to the older ones; the consistency of the set is ensured by `ld.so` via NEEDED. If
`KINETREN_FFMPEG_DIR` is set, its directory is tried FIRST on both OSes: the set is loaded from there by
full paths in dependency order (below), and if the directory lacks a complete set, it is listed in the report as
rejected, and the search goes on by the usual path.

On Windows there is no system FFmpeg, so the loader SHALL search for a DIRECTORY with a complete set of
`avutil-N.dll`, `swresample-N.dll`, `swscale-N.dll`, `avcodec-N.dll`, `avformat-N.dll`
of one major (9: 61/7/10/63/63; 8: 60/6/9/62/62; 7: 59/5/8/61/61; 6: 58/4/7/60/60) in a fixed order:
(1) the directory chosen by the user (saved in the settings); (2) `KINETREN_FFMPEG_DIR`;
(3) `ffmpeg\` and `ffmpeg\bin\` next to the executable; (4) Kinetren's own download directory
(the folder of the current pin first, then the folders of earlier pins, newest version first);
(5) the `PATH` directories where such a set lies — and if a `PATH` directory holds only an `ffmpeg.exe`
that is a link (winget `Links`), the directory of its target; (6) known install locations
(scoop `ffmpeg-shared`, winget packages `Gyan.FFmpeg.Shared` and
`BtbN.FFmpeg.*Shared*`, `C:\ffmpeg\bin`, `%ProgramFiles%\ffmpeg\bin`). The order of directories
matters more than the major: the user's choice beats a newer major elsewhere. Inside a
directory the majors are tried from 9 down to 6. All five libraries MUST be loaded by the FULL path from ONE
directory in dependency order (avutil → swresample → swscale → avcodec → avformat),
so that the imports bind to the modules already loaded from this directory, and not to a same-named
DLL from another place on `PATH`; after loading, each module MUST be found in the chosen
directory, otherwise the set is rejected.

A failed load MUST NOT be remembered until the end of the process: the next attempt after an
explicit "rescan" (or after an installation/download) searches again; a successful load
is remembered forever. A directory holding only a major newer than the table (FFmpeg 10 =
avcodec 64) is rejected with that release named as not supported yet.
Why: a direct requirement of the owner ("no FFmpeg build of its own and no plugins");
linking tied the binary to the major of the build machine (`libavcodec.so.60: cannot open shared
object file` on a system with FFmpeg 7). On Windows `LoadLibrary` by a bare name will not find a
system FFmpeg — there is none there, and DLLs of different programs on `PATH` are easily mixed
("DLL hell"): the Windows loader matches dependencies by base name, so `avcodec`
from one directory would bind to the `avutil` of the same major version but a foreign minor one from
another. winget puts into `PATH` not the package directory but a link to `ffmpeg.exe`, and without
resolving it the DLLs next to it cannot be found. A remembered error would require a restart after
installing FFmpeg — exactly at the moment when the user has just done what they were asked to do.
FFmpeg 9 (2026-08) bumped every library major, so a loader without its row opened no media
on a system that had moved to it, and refused what gyan.dev's "latest" and winget give (ffmpeg-9).

#### Scenario: Source conformance on real fixtures
- **WHEN** a FLAC/AAC/H.264/VP9/muxed file is opened
- **THEN** a burst after seek equals the decode from zero byte-for-byte (lossless), 44.1 kHz gives the correct length at 48 kHz, backward seek of H.264 and seek of VP9 are deterministic, the keyframe index and the quality ladder work
- **TEST** t10 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: Span decode equals per-frame decode and does not recreate the decoder
- **WHEN** the key grid is read by one forward pass
- **THEN** each key is closed by a frame, the frames are byte-for-byte equal to frame-by-frame reading, reopens ≤ 1, each frame is decoded once; frame choice with mismatched grids is floor and only floor
- **TEST** t21, t112 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: I/O counters are live
- **WHEN** a file is opened and read
- **THEN** opens/packet_reads grow — the performance panel shows no dead zeros
- **TEST** t37 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: The muxer does not drop frames (container: capability `export`)
- **WHEN** 60 frames are exported through libx264 with send-retry on EAGAIN
- **THEN** the file ACTUALLY contains 60 frames, counted by our own decoder
- **TEST** t33 (`src/libs/kt-engine/tests/export_options.rs`)

#### Scenario: The release binary is not linked with libav*
- **WHEN** the release tarball (Linux) or zip (Windows) is built
- **THEN** on Linux `ldd … | grep -c libav` gives 0; on Windows the PE import table of `kinetren.exe` and `kt-cli.exe` names no `av*`/`sw*` DLL and no mingw runtime DLL (`libgcc_s*`, `libstdc++*`, `libwinpthread*`); otherwise the build is rejected
- **TEST** guard in `src/scripts/release-build.sh` (the rule of invariant 10 of the root CLAUDE.md)

#### Scenario: The search order on Windows and the DLL names by major
- **WHEN** the search plan is built over substituted environment and file system: the chosen directory, the variable, the directory next to the exe, the download directory, a `PATH` of three directories, known locations; a complete set 9 lies in a known location, a complete set 7 — in the chosen directory
- **THEN** the user's directory with major 7 is chosen; the names of the five DLLs of each major (9, 8, 7, 6) match the table of the requirement; the loading order inside the set is avutil, swresample, swscale, avcodec, avformat; the plan function is pure and checked on any OS
- **TEST** t844 (`src/libs/kt-ffmpeg/tests/loader_plan.rs`)

#### Scenario: Rejected directories are named with a reason; a winget link is resolved
- **WHEN** `PATH` contains: a directory with a static `ffmpeg.exe` without DLLs; the `WinGet\Links` directory with an `ffmpeg.exe` link to a package where a complete set 8 lies; a directory with `avcodec-64.dll` (FFmpeg 10); a directory where `avcodec-63.dll` is present but `avutil-61.dll` is not
- **THEN** the directory of the link's target is chosen; the report lists each rejected directory with a reason ("static build — no DLLs", "FFmpeg 10 (avcodec 64) is not supported yet", "FFmpeg 9: no avutil-61.dll")
- **TEST** t845 (`src/libs/kt-ffmpeg/tests/loader_plan.rs`)

#### Scenario: The set is loaded from one directory (Windows)
- **WHEN** on Windows (or under wine) the pinned gyan.dev 9.0.2 build is loaded from a directory, and a directory with a different `avutil-61.dll` stands earlier in `PATH`
- **THEN** the load succeeds, `avcodec_version()` gives major 63, and the path of each of the five modules lies in the chosen directory; without the pinned build on the machine the test is skipped with a named reason
- **TEST** t846 (`src/libs/kt-ffmpeg/tests/loader_windows.rs`)

#### Scenario: A failure is not remembered, success is remembered forever
- **WHEN** the first load runs with an empty list of directories, then a directory with a complete set is added and a rescan is called, then the rescan is called again
- **THEN** the first load is an error with the search report; the second is a success; the third returns the same loaded set, without opening the libraries again
- **TEST** t847 (`src/libs/kt-ffmpeg/tests/loader_plan.rs`)

#### Scenario: The table of majors equals the vendored headers
- **WHEN** the table of library majors is compared with the `LIBAVUTIL/LIBSWRESAMPLE/LIBSWSCALE/LIBAVCODEC/LIBAVFORMAT_VERSION_MAJOR` constants of each version module (6, 7, 8, 9), and the version modules are counted against the table rows
- **THEN** every row equals its module's constants (9: 61/7/10/63/63), there is a module for every row and a row for every module; the check needs no FFmpeg on the machine
- **TEST** t862 (`src/libs/kt-ffmpeg/tests/loader_plan.rs`)

#### Scenario: The Linux plan knows FFmpeg 9
- **WHEN** the Linux plan is built with the system search on and `KINETREN_FFMPEG_DIR` unset; then with `KINETREN_FFMPEG_DIR` naming a directory with `libavutil.so.61`, `libswresample.so.7`, `libswscale.so.10`, `libavcodec.so.63`, `libavformat.so.63`; then a directory holding `libavcodec.so.63` without `libavutil.so.61`
- **THEN** the system candidates are asked by SONAME for 9, 8, 7, 6 in that order (`libavcodec.so.63` first); the directory is the first candidate and is accepted as FFmpeg 9; the incomplete one is rejected as "FFmpeg 9: no libavutil.so.61" and the system candidates follow it
- **TEST** t863 (`src/libs/kt-ffmpeg/tests/loader_plan.rs`)

### Requirement: Source contract — audio 48 kHz/2ch f32 by bursts, frame at exactly the requested quality
Any `MediaSource` SHALL deliver audio only as 48 kHz stereo f32 interleaved in exactly
1024 frames per burst, where the burst idx covers the samples [idx·1024, (idx+1)·1024) from the start of the stream;
the conversion of formats happens INSIDE the source. The video frame SHALL be exactly the requested
Quality (the composite discards someone else's size); the requested Quality is computed by the layer geometry
(capability `video-plan`), and not by the consumer from the raster of the project. `info()` SHALL report
the pixel aspect ratio (SAR, default 1:1) and the frame kind = native × SAR; all conforming,
thumbnails and "Max (by sources)" are computed from the kind. A source without sound SHALL answer
`has_audio: false` AND `Ok(silence)`, and not `Err`. The parsing of pseudo-paths (`synth:`, `gen:`) SHALL
live only in the factory and be case-insensitive; a broken pseudo-path is an honest error, it does not go to
FFmpeg.
Why: no consumer should see the sample rate of a file (a defect of the old engine —
hiss in slowed&reverb); an `Err` "no sound" turned into false underruns through the negative cache of the
worker; two parsings of `synth:` diverged by case; two files with identical pixels and
different SAR gave byte-for-byte one frame — anamorphic was inexpressible.

#### Scenario: Sample-accurate bursts
- **WHEN** the source is read out of order and in sequence
- **THEN** the results match byte-for-byte
- **TEST** t10 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: The pixel aspect ratio reaches the kind of the frame
- **WHEN** two 640×480 files with identical pixels and SAR 1:1 / 2:1; a synthetic source with `sar`
- **THEN** `info()` returns different kinds (4:3 and 8:3), the conform frames differ, the synthetic without FFmpeg carries the same field
- **TEST** t711 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`), t711b (`src/libs/kt-media/src/synthetic.rs`)

#### Scenario: The retune predicate matches the retune itself
- **WHEN** the factory answers whether the source can `retune`
- **THEN** the answer exactly matches the behaviour of `MediaSource::retune` (a file — never, a generator — yes)
- **TEST** t499 (`src/libs/kt-media/src/factory.rs`)

#### Scenario: Test sources are honest about what they measure
- **WHEN** a test asserts "zero decodes" or "zoom sharpness"
- **THEN** "zero decodes" is measured by the counters of `SyntheticSource`, and sharpness — only on `CheckerSource` with a real native size and box minification (the synthetic native is fictitious)
- **TEST** t13 (`src/libs/kt-engine/tests/viewport.rs`), t315 (`src/libs/kt-engine/tests/decode_sharpness.rs`, capability `video-plan`)

### Requirement: Live playback goes through the MediaWorker priority queue
One thread SHALL own the decoder instance (`&mut self`). Playback, prefetch and
thumbnails MUST NOT touch the decoder directly — only by requests into the queue with priorities
Playback > NearPrefetch > FarPrefetch > Thumbnail and generation cancellation; at equal priority
requests FORWARD from the decoder's cursor are served, backward — when forward is empty; a key is served
no more than once per submission; what is already in the cache is not decoded. Playback MUST NOT block
on decode: no data by the deadline → silence/last frame + an underrun flag.
Playback audio MUST NOT starve behind the video of the same worker: after a jump (seek) into
a file of any length, the first audio bursts with Playback priority SHALL be served before the
video requests (Playback video — not before the audio of the same deadline, video prefetch — only
when there are no Playback/NearPrefetch audio requests), and one video frame MUST NOT hold the thread
for longer than one audio burst without checking the queue; after the pre-roll the underruns SHALL be
zero, while the source is read faster than real time.
Why: the old engine shared one decoder between prefetch and playback through a mutex and drove it with
seeks (defect h), and one puller thread drew both audio and video — slow video decode starved the sound (defect l). R.63: by the flight log of paintitblack every jump into a file of 870 s
(H.264 1080p) gave 1–6 s of total silence (u≈48 000 frames/s at a render cost of 2 % of real
time) — that is, defect l came back in other clothes: the sound waited for the video.

#### Scenario: Scheduler — priority, cancellation, deduplication, direction
- **WHEN** the queue is clogged with a distant prefetch and a Playback arrives; the generation is bumped; one key is queued twice; equal priorities on both sides of the cursor
- **THEN** Playback is served within the first three; the stale generation is dropped; the key is served once; requests forward from the cursor come first; the prefetch pass is idempotent
- **TEST** t9 (`src/libs/kt-engine/tests/scheduler.rs`)

#### Scenario: Sequential grid without a decode storm
- **WHEN** 90 grid frames are ordered in a row, then again over a warm cache; audio and video alternate
- **THEN** zero backward reads, each frame is decoded no more than once, the warm repeat — zero decodes, the audio and video cursors do not zigzag past each other
- **TEST** t21 (`src/libs/kt-engine/tests/video_span.rs`)

#### Scenario: The video cache does not stand under a stream of Playback requests
- **WHEN** a "slow 4K" plus constant Playback singletons of the current frame and NearPrefetch passes
- **THEN** the span top-up runs OVER the priorities (the Playback key becomes the start of the span), the coverage grows continuously to the end, each frame is decoded once; a key beyond EOF is decoded at most once (negative cache)
- **TEST** t27 (`src/libs/kt-engine/tests/video_stall.rs`)

#### Scenario: The worker inserts coverage as a strip along the grid step
- **WHEN** 30 frames are ordered along the grid
- **THEN** the coverage is one continuous run [0, n·step)
- **TEST** t28 (`src/libs/kt-engine/tests/video_stall.rs`)

#### Scenario: Retuning a source is not a request
- **WHEN** a source is retuned in the middle of seeks (a drag of a generator parameter)
- **THEN** the retune is not subject to generation cancellation, is applied first, and its source's cache is cleaned by the SAME worker thread — not a single frame of the old parameters remains
- **TEST** t501 (`src/libs/kt-engine/tests/scheduler.rs`)

#### Scenario: A jump into a long file — the sound does not wait for the video
- **WHEN** a source with slow video (a frame costlier than an audio burst) and a length ≥ 300 s; the player jumps to its tail and to the middle several times in a row, the window video prefetch is enabled
- **THEN** the first audio bursts after each jump are served before any video frame, the underruns after the pre-roll are zero (on the former code — thousands of frames), while the video catches up without dropping grid frames; in a live window on `kt-untitled-review-870s.mp4` after a jump to 817 s the flight log contains no SNAP with `u` > 0 after the first second
- **TEST** t758 (`src/libs/kt-engine/tests/video_stall.rs`), live scenario `src/scripts/live/seekjump.py`

### Requirement: The offline path opens its own short-lived source under three conditions
The index service, offline analysis, export and kt-cli MAY open THEIR OWN instance of the source,
bypassing the queue, only if all three conditions are met: the source is shared with nobody, the pace
is not promised (nobody waits for real time from it), the task is finite and cancellable. A violation of
any — back to the queue. A finite task SHALL be built on a frame with progress, pause,
cancellation and stopping of the thread by `Drop`; the service SHALL be one thread with a queue, die together
with its owner and publish errors (once per address).
Why: the former formulation "all clients go through the queue" was untrue; the index service
raised a new thread on every call (three per project, writing into orphaned `Arc`s), and four
`let _ =` were silent about the index not being built — a person saw placeholders without a single word.

#### Scenario: The text of the rule and the feasibility of its conditions
- **WHEN** invariant 5 of the root CLAUDE.md is read by the guard
- **THEN** it names "Live playback", "Offline path", "is not shared", "does not promise a pace", "finite and cancellable"; the task has a `Drop`, the index service has a `Drop`
- **TEST** t422 (`src/libs/kt-engine/tests/offline_sources.rs`)

#### Scenario: Frame of a finite task
- **WHEN** a task sends progress, it is paused, cancelled, or forgotten
- **THEN** progress and the result arrive, pause holds, cancel stops and releases the pause, `Drop` stops the thread
- **TEST** t398, t398b (`src/apps/kt-app/src/bg.rs`)

#### Scenario: The index service — queue, death with its owner, descriptive errors
- **WHEN** two jobs are queued in a row; a file does not open; the owner drops the service
- **THEN** both are served; an error with the file address is published exactly once; after `Drop` the next job is not served
- **TEST** t399, t400 (`src/apps/kt-app/src/util.rs`)

### Requirement: The burst cache is the only truth about coverage
The warmth bars SHALL be built only from `coverage()` of the cache itself — an interval set,
maintained on insert/evict, without parallel bookkeeping. Quality SHALL be part of the
key (the audio key has no quality); a quality with a different size MUST NOT satisfy the request.
The cache stores PRE-FX data; video post-FX is a separate kind, invisible to coverage, prefetch and pin,
but living in the video budget and removed by `invalidate_source`. Audio post-FX MUST NOT be cached.
Pin (the head of the loop region) excludes records from eviction, but not from coverage. The probe `contains`
MUST NOT touch the LRU and the epoch. Records evicted or overwritten before the first read
SHALL be counted by a separate counter (no-read drops).
Why: the old ms buckets of coverage lied in four ways (defect g); FX is cheaper than decode;
a probe-without-read that refreshed the LRU evicted what had been honestly read; the "dead zeros" of the v1 performance panel.

#### Scenario: Eviction shrinks coverage exactly, quality does not cross-satisfy
- **WHEN** records are evicted; another quality is requested; the cache does not mutate
- **THEN** coverage decreases exactly by the dropped records; another quality is a miss; the epoch changes only on mutation
- **TEST** t8 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: The head of the loop region is pinned
- **WHEN** a loop region plays with a warm cache
- **THEN** the wrap is sample-continuous, the head of the region is not evicted, there are no misses
- **TEST** t14 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: A probe does not touch the LRU and the epoch
- **WHEN** `contains` is called for records, and then an insert evicts
- **THEN** the epoch has not changed, the oldest by the REAL recency is evicted (the honestly read one survives)
- **TEST** t27 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: Video coverage by ranges
- **WHEN** spans [pts, pts+step) are inserted and one is evicted
- **THEN** the strips merge into one; eviction removes exactly its own span; the interval-set operations are correct at the boundaries
- **TEST** t28 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: No-read drops are counted separately
- **WHEN** an unread record is evicted; a read record is evicted; an unread key is overwritten
- **THEN** the counter grows in the first and third cases and does not grow in the second
- **TEST** t37 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: The post-FX kind is invisible to coverage
- **WHEN** post-FX frames of the same source are inserted
- **THEN** the pre-FX coverage does not change, the bytes are counted in the video budget, a hit only by the exact (quality, fx_hash), and `invalidate_source` removes them too
- **TEST** t65 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: Source invalidation is not linear over the cache
- **WHEN** `invalidate_source` is called on every edit of a parameter
- **THEN** only the index of ITS OWN source is scanned
- **TEST** t497 (`src/libs/kt-cache/src/lib.rs`)

### Requirement: Prefetch is idempotent, computes keys on the playback grid, and order ⊇ read
Prefetch SHALL be a loop without state between ticks: wanted (lookahead from the playhead, the visible
range, the head of the loop region) MINUS coverage → submit. The keys SHALL match the
playback grid. The wanted set SHALL cover everything the render reads in this window,
including the warp path and the raw reverse path; overlapping and touching ranges of one
media SHALL be collapsed into one request.
Why: the old prefetch ran on its own grid of 10/12 fps and filled the cache with keys that
playback did not ask for (defect i); an order that diverged from the read is an eternal cache miss; 885 micro-events
on one frame gave 885 coverage requests under the mutex on every pass.

#### Scenario: Wanted covers consumption on a slowed clip
- **WHEN** random jumps over a slowed clip; the warm-up is strictly by wanted; the end of the clip
- **THEN** each read burst enters wanted; the warm render is byte-for-byte equal to the cold one without misses; the tail does not fall into silence
- **TEST** t17 (`src/libs/kt-engine/tests/warp_prefetch.rs`)

#### Scenario: The raw reverse path warms with its own window
- **WHEN** the transport moves backward and the render reads linearly, without warp
- **THEN** the Playback prefetch computes wanted with the raw window; the warp window does not cover the same bursts (otherwise the test guards nothing)
- **TEST** t285 (`src/libs/kt-engine/tests/warp_prefetch.rs`)

#### Scenario: Micro-events — one order
- **WHEN** 400 events of 90 ticks each, end to end, read one frame of the source
- **THEN** wanted contains exactly one range
- **TEST** t287 (`src/libs/kt-engine/tests/scheduler.rs`)

### Requirement: Timeline scroll/zoom does not generate decode requests
Waveforms SHALL be read from the peak pyramid (memory/DB), thumbnails — from the strip in the sidecar DB,
warmth bars — from the coverage of the cache; while there is no index — placeholders. The keys of the thumbnails SHALL
be quantized from the ZERO OF THE FILE (the common cache of the bin and the timeline). Everything the thumbnail worker may "fail to get"
(the duration, decoding of a key) SHALL publish the failure, so that a repeated request is not queued; the thumbnail sources are kept in an LRU, and not reset wholesale.
Why: the scroll/zoom of the old editor produced a storm of decodes (defect m); without negative caches the
storm of FFmpeg opens reached 1.6k/s while the mouse moved; quantization from `source_in` gave a double
decode and a bloated DB.

#### Scenario: The viewport black — zero accesses to the source
- **WHEN** 500 redraws of random viewports over a finished pyramid and a warm cache
- **THEN** the counter of source reads has not moved
- **TEST** t13 (`src/libs/kt-engine/tests/viewport.rs`)

#### Scenario: The peak pyramid is exact and survives reopening
- **WHEN** the pyramid is built and the project is opened again
- **THEN** the levels equal the naive recomputation; the repeated opening reads from the DB without decode
- **TEST** t15 (`src/libs/kt-media/tests/index_db.rs`)

#### Scenario: Peaks without a DB and thumbnails for media added later
- **WHEN** the project is unnamed (no DB); media is added after the service has started
- **THEN** the pyramid is built anyway; the thumbnail of the new media appears
- **TEST** t23 (`src/apps/kt-app/src/util.rs`)

#### Scenario: Negative thumbnail caches
- **WHEN** the duration or the thumbnail of unknown media cannot be obtained
- **THEN** the failure is published once, repeated `request` calls are no-ops; `invalidate_media` resets both caches
- **TEST** t69 (`src/apps/kt-app/src/thumbs.rs`)

#### Scenario: The nearest thumbnail within tolerance
- **WHEN** a bin scrub asks for a moment between the thumbnails of the filmstrip
- **THEN** the nearest within tolerance is returned (an exact one takes priority), outside tolerance — None, someone else's media is not visible
- **TEST** t70 (`src/libs/kt-media/tests/index_db.rs`)

#### Scenario: The LRU of thumbnail sources and unified keys
- **WHEN** there are more sources than the capacity; the bin and the timeline ask for one moment of a file
- **THEN** exactly the oldest is evicted, an access moves an entry to the tail; the key is one for both
- **TEST** t71, t62 (`src/apps/kt-app/src/thumbs.rs`)

### Requirement: Sidecar index DB — a cache invalidated by file identity and additive tables
`<project>.ktindex.db` SHALL be a cache: loss or corruption is not an error, the DB is recreated.
The identity of the media is path + size + mtime + xxhash: if size+mtime match → trust; if mtime changed →
recompute the hash, if it matches → reuse; if it does not match → rebuild ONLY this media_id.
The batch index SHALL be persisted so that a repeated opening of the file does not scan it.
New tables SHALL be added additively (`IF NOT EXISTS`), without a schema bump; the analysis numbers
are stored as integers ×100. Invalidation SHALL remove the records where the media stands on EITHER side of a pair.
DB errors are cache errors: a silent fallback to the ordinary opening. The UI thread does not touch the DB.
Why: the old audio seek kept a coarse byte index and rebuilt it on every launch (defect j);
a schema bump wipes the batch index, peaks and thumbnails of all media of the project; f64 in the DB gives a different number in the
last digit on "recomputed the same thing"; a uniform `DELETE WHERE media_id` does not see the pair
where the replaced file is the second one.

#### Scenario: Invalidation by content
- **WHEN** the content of a file has changed; the mtime is touched without a change of content; the DB is corrupt
- **THEN** only the sections of this media are rebuilt (the neighbours are intact); the hash matched — reuse; the corrupt DB is recreated without a panic
- **TEST** t16 (`src/libs/kt-media/tests/index_db.rs`)

#### Scenario: The batch index survives opening
- **WHEN** the file is opened a second time with the same DB; the file has changed
- **THEN** the second opening does not scan, the index is byte-for-byte equal to a fresh one and functionally equivalent (keyframe_before, sample-accurate audio); the changed file is rescanned
- **TEST** t34 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`)

#### Scenario: Analysis tables are additive and exact, invalidation is two-sided
- **WHEN** loudness and sync offset are written and read; the file is replaced
- **THEN** the round trip is byte-exact (integers ×100); for the replaced file the loudness and ALL pairs involving it are removed, a foreign pair stays intact; the observer of the pyramid sees every burst and does not change it
- **TEST** t401, t402, t401b (`src/libs/kt-media/tests/index_db.rs`)

#### Scenario: The reel preview is keyed by content (container: capability `compositions`)
- **WHEN** a reel without a file puts frames and peaks into the sidecar
- **THEN** the key is (reel, revision, file fingerprint); any divergence → None, one version per reel; `purge_media` does not touch these tables
- **TEST** t639 (`src/libs/kt-media/tests/index_db.rs`)

### Requirement: A missing source is a placeholder; returning means swapping on the fly
An unavailable file SHALL get a `PlaceholderSource` — an ordinary `MediaSource` in an ordinary
worker: a dark deterministic frame with the file name, silence, a duration of "practically
infinite", with no frame grid of its own. The project opens and plays, each missing file
is named separately; the render goes on with a warning before and a summary after. A returned file SHALL
be substituted on the fly: shut down the worker → evict its frames from the cache → raise a new one; the position and the cache
of the other sources are not touched; the native size of the new file recomputes the decode step. Adding
media to the project MUST NOT silently turn a typo into a stub.
Why: a file that did not open used to drop out of the list, and the preview hung in "Loading" forever;
invalidation before stopping the worker allowed it to write a stub frame back after the cleanup.

#### Scenario: The stub frame is deterministic and captioned
- **WHEN** the stub is read twice, at different times, with a non-ASCII name
- **THEN** the bytes are identical, do not depend on time, the caption is visible, different names give different frames, non-ASCII → "?", the audio is silence, the duration > 1000 h
- **TEST** t208 (`src/libs/kt-media/src/placeholder.rs`)

#### Scenario: A project with missing sources opens
- **WHEN** two files of the project are absent
- **THEN** the project is open, each is named in the notifications, the preview is filled with stubs
- **TEST** t209 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The render with stubs goes on
- **WHEN** the project has an unavailable source
- **THEN** the settings window warns before the start, the export goes on, the summary says how many stubs and lost layers there are
- **TEST** t210 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A returned file replaces the stub
- **WHEN** the file has appeared on the disk
- **THEN** the source is substituted without reopening the project, the frames of the old source have left the cache, the new one fills the cache with its own values
- **TEST** t211 (`src/libs/kt-engine/tests/player_dynamic.rs`, `src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The return of a file changes the decode step
- **WHEN** a file with a native margin under a zooming chain takes the place of the stub
- **THEN** the decode quality of the clip has grown, the bars and the prefetch already ask for it
- **TEST** t316 (`src/libs/kt-engine/tests/player_dynamic.rs`)

### Requirement: Sync by sound — by peak pyramid, sign is correct, refusal instead of a guess
The offset of two records SHALL be computed on the peak pyramid (without decode) over the positive part of the first
difference of the envelope; a positive lag = "B later", swapping the arguments gives a minus of the same
offset; below the confidence threshold — a refusal, not zero. The command SHALL only move (not trim,
not link), compute the material by the single rule "event ↔ material" (reverse), and alignment
that leads beyond the start of the project is to be rejected, not pressed to zero. Refusals are named by
different reasons; the event's menu item and the main menu item share one body.
Why: a lost sign spreads the tracks twice as far apart; "offset 0" is indistinguishable from "they already match";
a value pressed to zero looks synchronized, while the materials are out of step.

#### Scenario: The offset is found for different microphones, the sign is right, the unrelated is rejected
- **WHEN** recordings with different frequency response and noise; the arguments are swapped; the recordings are unrelated or pure noise
- **THEN** the offset is within the tolerance of a pyramid block and on the precision grid; the sign inverts, the tie-break is deterministic; a refusal or low confidence
- **TEST** t413, t414, t415 (`src/libs/kt-media/tests/sync.rs`)

#### Scenario: The command aligns the material, only moves, refuses past zero
- **WHEN** B enters the material from another point; B is reversed; the alignment leads beyond the start
- **THEN** the moment of the real world coincides for A and B, a repeat — "already aligned"; duration/source_in/link/track are untouched; `OutOfRange` instead of a clamp
- **TEST** t416, t417, t417b (`src/libs/kt-model/tests/sync_ops.rs`)

#### Scenario: Refusals aloud and one body
- **WHEN** not exactly two events are selected; there are no pyramids yet; the item is called from the event's menu and from the main menu
- **THEN** the reasons differ and are named; the function body is one for the application
- **TEST** t418, t419 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Offline loudness and sync analysis — one job, dedup, negative cache
Analysis of the material (loudness, synchronization) SHALL run as no more than one task at a time,
the queue deduplicates requests, a failure is remembered and is NOT reordered by itself (a repeat — only
on a direct request). A hint, a hover, a window MUST NOT queue a request; while it has not been measured, this is
said so. `is_running` is held by a live task, not by an open window. A result for a
stale revision or for vanished media MUST NOT be applied. The loudness of a source SHALL
be computed on a pass of the pyramid and read from the DB on a repeated run.
Why: analysis reads files whole — three at once would take the disk away from playback; eternal
hammering at an unavailable file; running the mouse over the bin would mean decoding dozens of files; a window
left after the finish would heat the system; a second pass for the sake of LUFS — minutes on an hour-long
source. Ducking left the list of tasks together with the snapshot (R.65): the live sidechain does not need offline
analysis at all, it is computed in the audio-render thread as an ordinary node.

#### Scenario: Gate, deduplication, negative cache, cancellation
- **WHEN** a duplicate and a second task are queued; a task fails; everything is cancelled
- **THEN** the duplicate is not in the queue, one is in progress; the refusal is remembered, does not repeat itself, on request — yes; cancellation clears the queue and the current one, `is_running` goes out
- **TEST** t408, t409, t410 (`src/apps/kt-app/src/analysis.rs`)

#### Scenario: Loudness on the pyramid's pass and from the DB
- **WHEN** the file is indexed; the service starts a second time with the same DB
- **THEN** LUFS/dBTP were computed on the same pass; the second start gives the same number without decode; the synthetic is not measured
- **TEST** t411 (`src/apps/kt-app/src/util.rs`)

#### Scenario: The hint shows and does not order
- **WHEN** the mouse moves over the cards of the bin
- **THEN** a known loudness is shown, an unknown one is labelled "not measured", the number of analysis requests is zero
- **TEST** t412 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The ducking task no longer exists
- **WHEN** the list of background analyses and the sources of kt-app are scanned for an order of ducking
- **THEN** no ducking task exists in the queue, the interface frame does not wait for it, and the loudness and sync work as before
- **TEST** t774 (`src/apps/kt-app/src/analysis.rs`)

### Requirement: A single picture — a source with a frame at any duration; a missing one — a placeholder, not "Loading…"
A single image file (JPEG, PNG — including PNG with alpha, as the system FFmpeg reads it) SHALL be a full-fledged source: it has ONE frame, delivered at ANY requested tick
(there is neither a frame grid nor an end of material — an event of any duration shows the picture
in full), audio — silence, duration "practically infinite" (as for the stub), the decode
step — from the native of the picture, PNG alpha — transparency in the composite. The frame SHALL
reach the preview no later than a video frame of the same size: the "Loading…" (a scrim of incompleteness
longer than 180 ms) over an image event MUST NOT hang after the first read of the file.
A missing picture SHALL get the same stub as a video (a dark frame with the
file name), and the stub is a FULL frame — the "Loading…" scrim MUST NOT appear over it at all;
the person sees the name of the missing file in the frame at once, and not when viewing. The picture fixtures
(JPEG, PNG with alpha) SHALL be generated by `fixtures/gen.sh` and cover reading, the plan,
preview and export.
Why: R.67, the owner: "static pictures are not displayed at all.. there is an eternal
Loading... not only does it not work - you also learn about it only at the moment of
viewing". There was not a single fixture with pictures in the project — the path had never been under
test; the note F-MED-003 measured a single PNG by hand in R.52.

#### Scenario: A picture is read, planned, shown and exported
- **WHEN** a 10 s event on the JPEG fixture and an event on a PNG with alpha over a coloured background; the frame is requested at 0, 3.7 and 9.96 s; it is opened in the harness; 1 s is exported
- **THEN** the source is open, at all three ticks it is the same frame with the picture's pixels; the event is in the frame plan; in the harness the preview is filled within 180 ms (there is no "Loading…" scrim); the PNG with alpha gives the background through the transparent pixels; the export contains the frames of the picture (a pixel probe); determinism — byte-for-byte
- **PATH** drag `still.jpg` onto the timeline → the event has appeared → click on it → the picture is at once in the preview, without "Loading…" → pull the edge of the event to 20 s → the picture is in place over the whole length
- **TEST** t796 (`src/libs/kt-media/tests/ffmpeg_conformance.rs`), t796b (`src/libs/kt-engine/tests/stills.rs`), t796c (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/stills.py`

#### Scenario: A missing picture — a stub with the name, without "Loading…"
- **WHEN** the project refers to a nonexistent `logo.jpg`; it is opened in the harness; the cursor is placed on the event; 500 ms pass
- **THEN** the frame has a dark stub with the text `logo.jpg` (a pixel probe of the caption), the notification names the file; there is no "Loading…" scrim in any frame (on the former code — eternal); returning the file to its place replaces the stub with the picture without reopening
- **PATH** open a project without the picture → at once the event shows a dark frame with the file name, a bell with a notification → put the file in its place → the picture has appeared by itself
- **TEST** t796d (`src/apps/kt-app/src/layout_tests.rs`), live scenario `src/scripts/live/stills.py`

### Requirement: Downloading the pinned FFmpeg build from inside the program (Windows)
On Windows the program SHALL offer the download of FFmpeg with one button. Exactly one
PINNED build is downloaded: `ffmpeg-9.0.2-full_build-shared.7z` from
`https://www.gyan.dev/ffmpeg/builds/packages/`. The archive MUST be checked against the SHA-256 recorded in
the program's sources; the "latest release" link (`ffmpeg-release-full-shared.7z`) MUST NOT
be used. From the archive SHALL be extracted only `bin/*.dll` and the license file, into
`%LOCALAPPDATA%\kinetren\ffmpeg\9.0.2\`. The directory MUST appear only whole: the unpacking
goes into a temporary directory, which is renamed to the final one. The download SHALL start
only by the user's press, after the source and the license (GPLv3, gyan.dev build) are shown,
and runs as a background task with progress and cancellation. Cancellation, a break or a hash mismatch MUST
leave the download directory without traces. After a successful installation the loader rescans the
directories, and the media shown as placeholders are restored by the same path as when a
missing file returns. Nothing is downloaded automatically. A folder left by an EARLIER pin
(`…\ffmpeg\8.1.2\`) MUST keep being searched after the current pin's folder, and MUST NOT be
deleted or replaced by the program: an update of Kinetren does not take away the FFmpeg the person
already downloaded.
Why: the owner's decision (2026-10-22): "item 1 + make it possible to download the gyan.dev
shared build". The version is pinned because a button that could download an unsuitable library
is worse than none: on 2026-10-22 gyan.dev's "latest release" was already FFmpeg 9.0.2, which the
loader did not support then. With FFmpeg 9 supported the owner moved the pin to 9.0.2 (2026-10-26);
the next major again needs a new pin, never the "latest" link. The hash is in our own sources, not in an adjacent `.sha256` from the same
server: a check against the same source proves nothing. We do not distribute
FFmpeg ourselves (neither our own build, nor DLLs in the zip): the user gets the build from its author
under its license.

#### Scenario: Check, unpacking and atomic installation
- **WHEN** the loader receives from a local test server a small 7z archive of the same structure (`<root>/bin/*.dll`, `<root>/LICENSE`, `<root>/bin/ffmpeg.exe`, `<root>/include/…`), first with a wrong hash, then with the right one; then the download is cancelled midway; then the server cuts the connection
- **THEN** with a wrong hash the download directory is empty and the error names the expected and the received hash; with the right one `9.0.2\` appears with only the DLLs and the license (without exe and include), and there are no temporary directories; cancellation and a break leave the download directory empty
- **TEST** t848 (`src/apps/kt-app/src/ffmpeg_setup.rs`)

#### Scenario: An earlier download keeps working after the pin moves
- **WHEN** the Windows plan is built with a download root holding `8.1.2\` (a complete set 8) and no `9.0.2\`; then with both folders complete; then with `9.0.2\` and a non-version entry (`.partial-123`)
- **THEN** in the first case `8.1.2\` is the download candidate and is accepted as FFmpeg 8; in the second `9.0.2\` comes before `8.1.2\` and wins; the partial entry is never a candidate; nothing in the root is removed
- **TEST** t864 (`src/libs/kt-ffmpeg/tests/loader_plan.rs`)

### Requirement: The FFmpeg search report is visible to the person and to kt-cli
The FFmpeg search result SHALL be available as a report: the chosen directory (or the SONAME on
Linux), the major and the full version of avcodec, and each rejected candidate with a reason. The GUI
shows it in the FFmpeg window; `kt-cli ffmpeg` prints it and exits with a non-zero code
if FFmpeg is not loaded. The error text ""FFmpeg not found"" MUST hint at the way of its OS:
on Linux — the distribution's packages, on Windows — the download button or the installation of a shared build
of a supported major.
Why: on Windows there are several causes of failure, and they are not obvious (a static build, FFmpeg 9,
a mixed set). Without a named cause the user sees only placeholders and does not know
what to do.

#### Scenario: kt-cli prints the report
- **WHEN** `kt-cli ffmpeg` is started with `KINETREN_FFMPEG_DIR` pointing at an empty directory, and with FFmpeg available by the usual path; then without an available FFmpeg
- **THEN** in the first case the output names the empty directory as rejected ("no avcodec-*.dll"), the chosen candidate and its version, exit code 0; in the second — the list of rejected ones and a hint for its OS, a non-zero exit code
- **TEST** t849 (`src/apps/kt-cli/tests/cli.rs`)

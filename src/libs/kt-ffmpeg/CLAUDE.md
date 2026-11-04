# kt-ffmpeg — FFmpeg at run time

Capability: `media-index` (requirement "FFmpeg — loaded at run time, the binary does not link
with libav*"), `export` (the encoder cascade).

- **No linking, ever** (invariant 10). `build.rs` generates only TYPES and constants per major
  (6/7/8/9) from the vendored headers, for the cargo TARGET (`--target=$TARGET` to clang; never
  decide by `cfg!(…)` in a build script — that is the host). Functions are resolved at run time
  (`api.rs`).
- **Search plan, then open** (`search.rs`, `loader.rs`; windows-build design D2). The plan is
  PURE over `search::Env` and works on strings with the planned OS's rules, so the Windows
  plan is tested on Linux (t844, t845) — keep every new rule inside the plan.
  - Linux: `KINETREN_FFMPEG_DIR` first, then the system libraries by SONAME, newest major first.
  - Windows: FOLDERS in a fixed order (chosen folder, `KINETREN_FFMPEG_DIR`, next to the exe,
    the download root's version folders, `PATH` with winget's `Links` followed, known scoop/winget/`C:\ffmpeg`
    places). The order of folders beats the order of majors.
  - Every rejected candidate keeps its reason (`Reject`): the report is what a person reads.
- **One folder, dependency order.** The five libraries load by FULL PATH from one folder in
  `LOAD_ORDER` (avutil → swresample → swscale → avcodec → avformat). On Windows with
  `LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS` (no PATH, no current
  folder), and `GetModuleFileNameW` must place every module in that folder — Windows binds an
  import by base name to whatever namesake is already loaded. Never `SetDllDirectoryW` or a
  process `PATH` edit: both are process-global and change how wgpu and cpal load their DLLs.
- **A failure is not cached, a success is.** `rescan()` searches again (startup, Locate…, a
  finished download, Rescan); `backend()`/`init()` after a failure return the stored report
  without touching the disk — opening media must never walk PATH. A loaded set stays for the
  process (its function pointers are in use). `is_loaded()` asks without searching.
- **Encoders** (`encoders.rs`): the cascade per OS with each candidate's pixel format and
  options; the muxer feeds the chosen encoder its own pixel format (QSV: NV12). Pure, tested on
  any OS (t858).
- **Adding a major** (ffmpeg-9, design D1/D2) — five steps, each guarded:
  1. `scripts/vendor-ffmpeg-headers.sh <version> <sha256>` from the official signed tarball
     (`vendor/ffmpeg-N/SOURCE` records it); the Dockerfile's `ffmpeg-test` stage builds the
     same tarball — dist-check (t866) keeps the two pins equal;
  2. `MAJORS` in `build.rs`; 3. `version_module!` + `VERSION_MODULES` + a `V<N>` arm in every
     enum of `lib.rs` (the dispatch in `rescan_with` is exhaustive: a major without a module is
     an error, never another major's layouts); 4. the row in `search::MAJORS` (newest first);
  5. t862 compares every row with its module's `LIB*_VERSION_MAJOR`. A field or constant the
     new major removed is handled inside the version module, never by a runtime branch on the
     major in shared code. Then prove it: `make ffmpeg-test` (the major built from source,
     found by SONAME, `KT_EXPECT_FFMPEG_MAJOR`, t865).
- **A behaviour removed without an API change is caught only by running.** FFmpeg 9 dropped the
  muxer's filling of missing packet fields; our frames carried no duration, and the last video
  frame of every export had lasted zero on EVERY major (t33 failed only on 9, whose exact edit
  list cut that frame). Frames sent to an encoder carry their duration (`muxer_impl.rs`).
- **Hardware contexts are an OPTIONAL table** (hw-encode-context D5): `ffmpeg_hw_api!` builds
  `HwApi` per major, resolved after `Api` into the module's `HW`; a missing name leaves it
  `Err(name)` and FFmpeg still loads. Never add a hardware-only function to the required
  `ffmpeg_api!` — one missing symbol there is "FFmpeg not found" for the whole program.
- **A device candidate is ONE step: device → frames → open** (`HwKind::Vaapi`, D3). What fails
  before the open skips it WITHOUT `avcodec_open2` and lands in `OpenReport.skipped` with its
  reason; a forced one errs naming the part. Each frame is converted to NV12 and uploaded to a
  surface (pts and duration kept); `Drop` frees frames → contexts → frames context → device.
  The list for the UI (`available_encoders`) opens the DEVICE, never the encoder, and kt-app asks
  it once per settings window, not per frame.
- **The hardware seam is thread-local** (`hw_seam`): a test binary runs exports in parallel, and
  `KINETREN_VAAPI_DEVICE`/`KT_DRI_DIR` would steer all of them. Processes (kt-cli tests) use the
  variables. The proof on hardware is opt-in and loud: `KT_EXPECT_HW=vaapi` (`make hw-test`) makes
  t963 fail unless VAAPI opens.
- **The download is a ROOT** (`<local>/ffmpeg`), not one folder: the Windows plan tries every
  version folder in it, newest first (numeric), so an earlier pin's download keeps working
  (t864); temporary `.partial-<pid>` entries are not candidates.

- **The decode's RGBA frame has `RGBA_TAIL_PAD` spare bytes** (export-throughput D1): libswscale's
  x86 yuv2rgb writes past the last row when the width is not a multiple of its block (32 bytes at
  1080×1080 on 6.1) — an exact-size buffer corrupted the heap (the showcase export crashed in 3 s).
  A new conversion into a buffer of ours converts through `sws_to_rgba` or pads the same way (t1450).
- **`prepare_for`'s "already held" exit holds only while the target is before the deferred frame**
  (D8): past it, the jump decides between decoding on and seeking — without the bound a two-minute
  jump decoded every 4K picture in between (t1453). Every key of a span goes through `prepare_for`.
- `read_video_span_sizes` decodes each picture once for every size asked at it (D7, t1454);
  `video_cursor` says where the decoder stands, for a caller choosing among decoders of one file.
- The muxer: `MuxConfig.threads` (0 = `auto`, in the open report's options, t1456); conversion is a
  separate `VideoConverter` per thread, `write_video_frame` takes the frames in order (D3, D4).
- `iostats::{VDEC_FRAMES, VDEC_NS, VIDEO_REOPENS, RGBA_*}` — the decode counters `kt-cli export
  --time` prints; the first thing to read when an export is slow.

Tests: t10/t21/t37/t112 (conformance), t1450 (RGBA tail), t1453–t1455 (span seek/qualities/decoders, in kt-media), t844–t847 (search, one-folder load under wine/Windows,
retry), t858 (encoders), t862 (table == headers), t863 (Linux plan), t864 (earlier download),
t865 (a run proves its major), t960 (device contexts), t961 (optional table == headers), t962
(no device: skip or error, never a crash), t963 (VAAPI on hardware, opt-in), t964 (kt-cli report).

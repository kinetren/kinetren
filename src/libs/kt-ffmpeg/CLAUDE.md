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
- **The download is a ROOT** (`<local>/ffmpeg`), not one folder: the Windows plan tries every
  version folder in it, newest first (numeric), so an earlier pin's download keeps working
  (t864); temporary `.partial-<pid>` entries are not candidates.

Tests: t10/t21/t37/t112 (conformance), t844–t847 (search, one-folder load under wine/Windows,
retry), t858 (encoders), t862 (table == headers), t863 (Linux plan), t864 (earlier download),
t865 (a run proves its major).

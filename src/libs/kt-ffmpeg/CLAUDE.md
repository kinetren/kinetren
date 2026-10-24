# kt-ffmpeg — FFmpeg at run time

Capability: `media-index` (requirement "FFmpeg — loaded at run time, the binary does not link
with libav*"), `export` (the encoder cascade).

- **No linking, ever** (invariant 10). `build.rs` generates only TYPES and constants per major
  (6/7/8) from the vendored headers, for the cargo TARGET (`--target=$TARGET` to clang; never
  decide by `cfg!(…)` in a build script — that is the host). Functions are resolved at run time
  (`api.rs`).
- **Search plan, then open** (`search.rs`, `loader.rs`; windows-build design D2). The plan is
  PURE over `search::Env` and works on strings with the planned OS's rules, so the Windows
  plan is tested on Linux (t844, t845) — keep every new rule inside the plan.
  - Linux: `KINETREN_FFMPEG_DIR` first, then the system libraries by SONAME, newest major first.
  - Windows: FOLDERS in a fixed order (chosen folder, `KINETREN_FFMPEG_DIR`, next to the exe,
    the download folder, `PATH` with winget's `Links` followed, known scoop/winget/`C:\ffmpeg`
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
- FFmpeg 9 (avcodec 63) is not supported yet: the plan names it as "not supported" and the
  Windows download is pinned to 8.1.2 (gyan.dev's "latest" is 9).

Tests: t10/t21/t37/t112 (conformance), t844–t847 (search, one-folder load under wine/Windows,
retry), t858 (encoders).

# Design: Windows build

## Context

See proposal.md, Why. The current state that shapes the approach:

- **FFmpeg loader** (`src/libs/kt-ffmpeg/src/loader.rs`): a table of Linux SONAMEs, opened
  through `libloading::os::unix` with `RTLD_LAZY | RTLD_LOCAL`, newest major first, and the
  avcodec major checked with `avcodec_version()`. `lib.rs` keeps the result in a
  `OnceLock<Result<…>>`, so a failure is final for the process. `build.rs` runs bindgen over
  the vendored headers `vendor/ffmpeg-{6,7,8}/include` for **types only**, without
  functions. bindgen already passes the cargo `TARGET` to clang, so layouts follow the
  target, not the host. Nothing in the GUI reports a load failure today: media just becomes
  placeholders (`util::open_source_or_placeholder`). `spawn_media_watchdog`
  (`src/apps/kt-app/src/util.rs`) swaps a placeholder for the real source when a file comes
  back.
- **Data directory**: `journal::data_dir()` (`src/apps/kt-app/src/journal.rs`) is
  `$XDG_DATA_HOME` → `$HOME/.local/share` → `"."`. `kt-cli` has its own copy
  (`kt_lock_holder`, `src/apps/kt-cli/src/main.rs`). eframe keeps its storage in
  `%APPDATA%\kinetren\data` on Windows and in `$XDG_DATA_HOME/kinetren` on Linux.
- **Project lock** (`src/apps/kt-app/src/project_lock.rs`): `libc::flock` under
  `cfg(unix)`, `true` ("free") elsewhere. The pid is written into the locked file itself.
- **Atomic write** (`src/libs/kt-otio/src/io.rs`): injected file-system trait (t351 checks
  the call order). `sync_dir` is `Unsupported` off unix.
- **Paths** (`src/libs/kt-otio/src/url.rs`): string-level encode/decode/relativise built on
  `std::path`. This means the result depends on the OS that runs it.
- **Audio out** (`src/apps/kt-app/src/audio_out.rs`): picks a 2-ch f32 config covering
  48 kHz, otherwise takes the default config and repeats or skips samples positionally.
- **Encoder cascade** (`src/libs/kt-ffmpeg/src/muxer_impl.rs`): one fixed list.
- **Build** (`github-ci-docker-build`, in flight): one Dockerfile (Ubuntu 22.04, pinned
  toolchain), `release-build.sh` with the checks inside the container, root `Makefile`,
  `ci.yml` (nextest on ubuntu-24.04), `release.yml`, `dist-check.py` guard t834/t835.
- **Goldens**: `golden_export.rs` and `title_golden.rs` compare to constants under
  `cfg(all(target_os = "linux", target_arch = "x86_64"))`. A few hash checks (`video_fx.rs`,
  `fx_thumb.rs`, `geo_fuse.rs`, t662) aren't gated.

The owner's decisions (2026-10-22) are recorded in proposal.md: find the installed FFmpeg,
plus download gyan.dev's shared build; cross-build with mingw in Docker; portable zip.

## Goals / Non-Goals

**Goals:**
- A zip that runs on a clean Windows 10/11 x64 after the user gets FFmpeg with one click.
- One toolchain and one container for both deliveries, built from Linux.
- Behaviour on Linux unchanged, apart from the shared fixes that were wrong there too
  (multichannel output, QSV pixel format).
- Every platform difference lives behind a pure function that tests run on **both** OSes
  (search plan, lock key, URL rules, encoder list, locale mapping, data-dir resolution). The
  OS-specific shell around each one stays thin.

**Non-Goals:**
- An installer, a `.kt` file association, code signing, ARM64, an MSVC build.
- FFmpeg 9 (separate change; see Risks).
- Hardware *decoding* (there is none on any OS today).
- The live Xvfb harness on Windows. Windows UI checks are manual (tasks.md) until a Windows
  harness exists.

## Decisions

### D1. Target `x86_64-pc-windows-gnu`, cross-built in a `windows` stage of the one Dockerfile

The `windows` stage is `FROM` the existing base stage. It adds `mingw-w64`, `clang`, the
Rust target `x86_64-pc-windows-gnu` for the same pinned toolchain, and wine from WineHQ's
repository for jammy (22.04's own wine 6.0 is too old for nextest and recent std). It is
still one file, so the in-flight guard "exactly one Dockerfile" holds. The linker is set
per target in the container's environment (`CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=
x86_64-w64-mingw32-gcc`) and, for developers who have mingw, in `.cargo/config.toml`
(`[target.x86_64-pc-windows-gnu] linker = …`). The mold section stays Linux-only.

- **Why gnu rather than msvc:** the owner's choice. Also, msvc from Linux needs `cargo-xwin`,
  which downloads Microsoft's CRT and SDK under their licence on every clean build. The gnu
  target needs only Debian packages. Rust's gnu target links its own self-contained
  CRT objects and `libgcc_eh` statically. Our only C dependency, rusqlite's bundled SQLite,
  uses Win32 threads. So the exe can depend on system DLLs only, and the PE import check
  (D10) enforces it.
- **Consequences that are accepted:**
  - libm comes from mingw/msvcrt, which is why the goldens are kept per platform (D9).
  - Backtraces on the gnu target come from DWARF, not a PDB, so the crash report behaves as
    on Linux if the symbols survive `strip`. A task checks a forced panic in the Windows dist
    exe. If function names are missing, the dist profile keeps the symbol table for the
    Windows target only (`strip = "none"` + `debug = 1`).
- **bindgen for the target:** `build.rs` decides from `CARGO_CFG_TARGET_OS`/`TARGET`,
  never `cfg!(windows)` (that is the host). clang picks the mingw sysroot for
  `--target=x86_64-pc-windows-gnu` by itself. A first `cargo check --target` shows whether
  any code assumed `c_uint` enums or 64-bit `c_long`. Such places get explicit casts; the
  vendored headers aren't touched.
- **Rejected:**
  - Native MSVC builds on a Windows runner: the owner chose the cross-build, and it would
    mean two toolchains with two sets of bugs.
  - `x86_64-pc-windows-gnullvm`: tier 2 without host tools, and less common for wgpu and
    cpal than `-gnu`.

### D2. The loader becomes "plan, then open"; the plan is pure

`loader.rs` is split in three:
- **`plan(env) -> Vec<Candidate>`**, pure. `env` is a trait for env vars, exe dir, known
  folders, `read_dir`, `exists` and `read_link`. It returns ordered candidates: a SONAME set
  (Linux) or a directory plus major (Windows), each with an optional rejection reason.
- **`open(candidate)`**, OS-specific.
- **`report()`**: the list for humans.

The order of directories is the spec's (media-index). The decisions inside it:

- **Directory order beats major order on Windows.** The user's explicit choice must win over
  a newer FFmpeg that some other program put on `PATH`. On Linux there is one system
  install, so the order stays major-first.
- **Static builds are recognised, not just skipped.** A directory with `ffmpeg.exe` and no
  `avcodec-*.dll` gets the reason "static build — no DLLs". This is the most common
  mistake: `winget install Gyan.FFmpeg` is the static build.
- **winget `Links`:** winget puts a symlink `…\WinGet\Links\ffmpeg.exe` on `PATH`. The plan
  follows `read_link` of `ffmpeg.exe` to the package's `bin\`. If the link can't be read
  (some winget versions use a shim exe instead), the known-locations step still globs
  `%LOCALAPPDATA%\Microsoft\WinGet\Packages\Gyan.FFmpeg.Shared_*\*\bin` and
  `BtbN.FFmpeg.*Shared*`.
- **Unsupported majors are named.** `avcodec-63.dll` gives "FFmpeg 9 (avcodec 63) is not
  supported". The reason is derived from any `avcodec-N.dll` whose N isn't in the table, so
  FFmpeg 10 is reported the same way.
- **Opening on Windows:** `libloading::os::windows::Library::load_with_flags(full_path,
  LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS)` for avutil →
  swresample → swscale → avcodec → avformat. The Windows loader resolves an import by base
  name against modules already in the process, so loading dependencies first binds them to
  our directory's copies. `DLL_LOAD_DIR` covers anything else the DLL imports from its own
  folder. `DEFAULT_DIRS` excludes `PATH` and the current directory, so a stray DLL there
  can't sneak in. After loading, `GetModuleFileNameW` on each handle must lie in the chosen
  directory; otherwise the set is rejected as "mixed with <other path>". Gyan's shared build
  has no other third-party DLLs, because codecs are linked into avcodec.
- **Opening on Linux:** unchanged for SONAMEs. A `KINETREN_FFMPEG_DIR` candidate is opened by
  full paths in the same dependency order; `ld.so` then matches NEEDED by SONAME to the
  already-loaded copies. This gives developers and t848 a way to point at a specific build
  on both OSes.
- **Retry:** the `OnceLock<Result<…>>` becomes `OnceLock<Loaded>` (success, forever) plus a
  `Mutex<Option<Report>>` holding the last failure. `backend()` returns the loaded backend,
  or the stored failure without touching the disk. Only `rescan()` searches again. Its
  callers are startup, Locate, a finished download and the Rescan button. Opening media
  never searches: re-scanning `PATH` on every placeholder open would put file-system walks
  into the media path.
- **The user's choice** is a one-line file `ffmpeg-dir` in the data directory (D4), not
  eframe storage, so `kt-cli` sees the same choice.
- **Recovery after a late load:** kt-app asks the existing `spawn_media_watchdog` path to
  re-try every source that is a placeholder *because FFmpeg wasn't loaded* (the open error
  carries that kind). The swap is the one the media-index spec already describes: stop the
  worker, evict, start the new one.
- **Rejected:**
  - `SetDllDirectoryW` + loading by bare name: a process-global setting that also changes
    how wgpu and cpal load their own DLLs.
  - Adding the FFmpeg folder to `PATH` for the process: the same global effect, and it
    leaks into child processes.
  - `LOAD_WITH_ALTERED_SEARCH_PATH` alone: still searches `PATH` for transitive imports
    when the DLL's own folder lacks them.

### D3. Download: pinned archive, our own hash, pure-Rust extraction, atomic folder

- **Source:** `https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-8.1.2-full_build-shared.7z`
  (59 MB). The SHA-256 is committed in `ffmpeg_setup.rs`
  (`cba748035c21ce1431d0823c7a3a711f38616f89f87a265dceddf9b7f6749d2d`, read from gyan.dev on
  2026-10-22 and confirmed again at apply time by downloading). Fallback mirror: the same
  file name on `github.com/GyanD/codexffmpeg/releases/download/8.1.2/`. It is used only if
  its hash equals the pinned one (checked at apply time); if it differs, both hashes are
  committed with their URLs.
- **Why the versioned `packages/` URL:** the "release" link moves. Today it serves 9.0.2,
  which the loader rejects.
- **HTTP:** `ureq` + `rustls` + `webpki-roots` in kt-app only. This is the same choice as
  `telemetry-posthog` D2: one HTTP stack in the tree, and whichever change lands first adds
  it. `ureq`'s proxy-from-environment is on, because Windows corporate users often sit
  behind `HTTPS_PROXY`. Connect timeout 10 s; no total timeout, since the user sees progress
  and Cancel.
- **Extraction:** `sevenz-rust2` (pure Rust, LZMA/LZMA2 with the BCJ x86 filter that 7-Zip
  uses on executables). It extracts only entries matching `*/bin/*.dll` and `*/LICENSE*`.
  No 7-Zip on the user's machine, and no external process (consistent with invariant 9's
  spirit).
- **Atomic install:**
  1. Stream to `%LOCALAPPDATA%\kinetren\ffmpeg\.download-<pid>.7z`, hashing as bytes arrive.
  2. Verify the hash.
  3. Extract into `.partial-<pid>\`.
  4. `rename` it to `8.1.2\`.
  5. Delete the archive.

  Any failure or Cancel removes both temporary names. A leftover from a killed process is
  removed at the next download, since names carry a pid that is no longer running.
- **Job model:** a `BgJob` (progress, cancel, `Drop` stops the thread), like analysis jobs.
  The UI polls it and repaints only while it moves (invariant 12).
- **Test seam:** debug-only `KT_FFMPEG_DOWNLOAD_URL` and hash override, the pattern of
  `KT_TELEMETRY_URL`. t847 serves a committed tiny `.7z` fixture
  (`src/fixtures/ffmpeg-mini.7z`, a few KB, made by `src/fixtures/gen-ffmpeg-mini.sh` with
  stub DLL files) from a local `TcpListener`.
- **Licence:** the confirmation dialog names gyan.dev, the version, the size and "GPLv3",
  with a link. We don't redistribute: the user downloads the author's build from the
  author's server.
- **Rejected:**
  - The `.zip` from GitHub as primary: it's 106 MB against 59 MB, and the owner named
    gyan.dev.
  - The `.sha256` next to the archive as the check: same origin, so it proves nothing
    against a compromised server.
  - Shelling out to `curl`/`tar`: Windows 10 has both, but the error handling and progress
    would be worse than doing it in-process.

### D4. `kt-dirs`: one resolver for per-user directories

This is a new crate, `src/libs/kt-dirs`, with no dependencies besides `windows-sys` under
`cfg(windows)` (`SHGetKnownFolderPath` for `FOLDERID_RoamingAppData` and
`FOLDERID_LocalAppData`). It provides `data_dir()` and `local_dir()`, both over an injected
`Env` for tests (t850). kt-app and kt-cli both use it, so the second copy in kt-cli
goes away. The kinotrain migration stays in kt-app, compiled only for Linux, and runs only
against a resolved per-user root. kt-app also passes `NativeOptions::persistence_path`
derived from `data_dir()`, so eframe and we can't drift apart. A task checks that on Linux
this is byte-identical to today's path, so no settings move.

- **Why Roaming for data and Local for downloads:** presets, the workspace and recovery are
  small and personal and follow a roaming profile. 100 MB of DLLs must not roam.
- **Rejected:**
  - The `directories` crate: its Windows layout (`{org}\{app}\data`) doesn't match eframe's,
    which would mean two folders.
  - Putting the resolver in kt-otio: kt-otio is the exchange doors and the project file, not
    the user's machine.

### D5. Project lock: `std::fs::File::try_lock` on both OSes, pid in its own file

`File::try_lock` (stable on our toolchain) is `flock` on Linux and `LockFileEx` on Windows,
so the `libc` code and the no-op stub both go. The pid moves to `<key>.pid`, written after
the lock is taken. A Windows lock is mandatory: the second copy can't read a locked file,
but it can read a separate one. The key on Windows is computed by the pure
`lock_key(path, flavor)`:
1. `std::path::absolute`.
2. Strip `\\?\` (and `\\?\UNC\` → `\\`).
3. Use `\` everywhere.
4. `to_lowercase`.

`canonicalize` isn't used: on network drives it is slow, and it would resolve substituted
drives differently from the path the user opened.

- **Revision of the durability decision "flock on a neighbour file"** — the decision stands
  (a kernel lock on a file in the data directory, named after the project). Only the
  mechanism becomes per-OS. The rejected "pid file with `O_EXCL` instead of a lock" stays
  rejected: the pid file here is information only, and the kernel lock is what holds.

### D6. Atomic replace on Windows: `MoveFileExW` + write-through + retry

`write_atomic`'s file-system trait gets a `replace(tmp, dst)` operation. On Windows it is
`MoveFileExW(tmp, dst, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)`; on Linux, as
today, `rename` followed by `sync_dir`. The retry policy is pure and lives in kt-otio: up to
7 attempts with pauses of 10, 20, 40, 80, 160, 320 ms (about 630 ms of waiting, about 1 s in
all). It retries only on `ERROR_SHARING_VIOLATION`, `ERROR_ACCESS_DENIED` and
`ERROR_LOCK_VIOLATION`, and any other error fails at once. On Windows a successful replace
reports `dir_synced = true`, because write-through is that platform's durable rename. kt-otio
gains `windows-sys` under `cfg(windows)`. The rule "no FFmpeg or audio dependency" isn't
touched. The journal rotation and `user_store` call the same function.

- **Rejected:**
  - `ReplaceFileW`: it preserves the *target's* attributes and ACLs, which is good, but it
    fails when the target doesn't exist yet (first save). We'd need two paths for the same
    act.
  - An unbounded retry: a stuck editor would hang the exit save.

### D7. Paths: a `PathFlavor` parameter instead of `std::path` for foreign rules

`url.rs` functions take `PathFlavor::{Posix, Windows}`, with `PathFlavor::NATIVE` for
production callers. They work on strings: drive letter `X:` plus a separator, UNC `\\host\
share`, the verbatim prefix, case folding for Windows comparison. Because they're pure,
t853–t855 run the Windows rules on Linux CI and the Posix rules on Windows CI.

- **"Absolute" on reading is the union of both flavors** (`/…`, `X:\…`, `X:/…`, `\\…`). A
  path absolute anywhere is never joined to the project directory. On the native OS it
  resolves normally; on the other OS it yields a placeholder whose label is the string as
  written, and the writer keeps it verbatim.
- **Relative paths read on Windows** have `/` turned into `\` before the join, so the
  index-DB key and media dedupe don't see `C:\p\sub/clip.mp4` and `C:\p\sub\clip.mp4` as
  two files.
- **This takes up the deferred item** "cross-platform paths in `target_url`" from
  `project-format/design.md` (it was postponed, not rejected). Resolve-style path mapping
  (F-PRJ-003) stays deferred: this change only stops paths from turning into wrong ones.

### D8. Audio output edge: fixed-ratio sinc resampler and channel map inside the callback

`audio_out.rs` gets an `OutputEdge` built before the stream starts, from the device config.
It is one of:
- `Direct` (48 kHz stereo): today's code path, byte for byte.
- `Map` (48 kHz, N channels).
- `Resample` (other rate, any channel count).

`Resample` uses rubato (already a workspace dependency) in a fixed-input mode with
`process_into_buffer`. The input chunk is fixed (480 engine frames), and an output FIFO is
preallocated to cover the largest callback cpal reports. The callback pulls whole input
chunks from the ring as the FIFO runs low. The playhead advances by the engine frames
consumed (invariant 6 counts what the engine handed out), and latency compensation adds the
resampler's delay converted to engine frames. Mono gets (L+R)/2. Allocation-freeness is
proven in t856 with a counting global allocator in the test binary.

- **Why at the edge and not in the render thread:** the ring and the playhead stay in engine
  frames, and nothing upstream learns about device rates (invariant 1).
- **Rejected:**
  - Asking WASAPI for 48 kHz through `AUTOCONVERTPCM`: cpal 0.16 doesn't expose it, and it
    would hand quality to the system's resampler.
  - Exclusive mode: it takes the device from every other program.

### D9. Encoders and goldens: tables keyed by OS

- **Encoders:** a pure function `video_encoder_candidates(os)` returns
  `(name, pix_fmt, quality options)`:
  - nvenc: `cq`
  - amf: `rc=cqp`, `qp_i`/`qp_p`
  - qsv: NV12, `global_quality`
  - vaapi: as today
  - libx264: `crf`
  - mf: `rate_control=quality`, `quality`
  - mpeg4: `q:v`

  The muxer converts through swscale to the candidate's pixel format, so NV12 for QSV costs
  one conversion. `cfg(target_os)` only selects which list production uses; t857 checks
  both lists.
- **Goldens:** each golden constant becomes a table
  `&[(os, arch, value, provenance)]`. Lookup is by `(target_os, target_arch)`; a missing
  entry gives a skip with a named reason. t858 guards that the delivered platforms have
  entries. The Windows values are taken from the first green Windows CI run of the archive
  (D11), and the provenance line names the run. So t858 lands in the same stage that
  commits them, not earlier.
- **Revision of the export decision "constants on linux/x86_64":** it was a single platform
  because there was a single delivery. Now there are two deliveries, and determinism
  between runs is still checked everywhere.

### D10. Delivery check for PE

Inside the container: `x86_64-w64-mingw32-objdump -p <exe> | grep 'DLL Name'` gives the
imports. The check fails on any name matching `av*`, `sw*`, `libgcc_s*`, `libstdc++*` or
`libwinpthread*`, and on any name outside an allow-list of Windows system DLLs. The
allow-list is grown deliberately when a dependency legitimately adds a system DLL:
`KERNEL32`, `USER32`, `GDI32`, `ADVAPI32`, `SHELL32`, `OLE32`, `OLEAUT32`, `WS2_32`,
`bcrypt`, `ntdll`, `msvcrt`, `api-ms-win-*`, `d3d12`, `dxgi`, `d3dcompiler_47`, `dwmapi`,
`uxtheme`, `imm32`, `winmm`, `comctl32`, `comdlg32`, `propsys`, `userenv`, `crypt32`,
`secur32`, `opengl32`, `ksuser`, `avrt`, `mmdevapi`, `CFGMGR32`, `SETUPAPI`, `VERSION`,
`dbghelp`, `synchronization`, `powrprof`. The zip recipe never globs `*.dll`, and the guard
checks that too.

### D11. Tests on Windows: build once, run the archive

- **The Linux job** cross-builds `cargo nextest archive --target x86_64-pc-windows-gnu
  --archive-file win-tests.tar.zst` in the container, with `src/fixtures` (generated by
  `gen.sh` in the same job) included through `.config/nextest.toml`
  `[profile.ci-windows.archive] include`. It uploads the archive with `retention-days: 1`.
- **The `windows-latest` job:**
  1. Install nextest.
  2. Download the archive.
  3. Fetch the pinned gyan.dev build and verify its hash.
  4. Set `KINETREN_FFMPEG_DIR`.
  5. `cargo nextest run --archive-file … --workspace-remap .`

  There's no Rust toolchain build step on the runner.
- **`make windows-test`** runs the same archive under wine in the container.
- **What can't run on Windows** skips with a named reason, never silently: GPU (no adapter
  on the runner, as on Linux CI), Xvfb/live, and the repository guards that call `python3`
  (`dist_guard`, `readme_guard`, the `reach_tests` scanners). The guards check the
  repository, not the platform, and they run in the Linux job. This isn't a weakened
  regression test: the same test runs, just on one OS.
- **Linux-only assumptions in tests become portable:**
  - `/proc/self/no-such-dir` "unwritable" paths become a path under a regular file
    (`<tmpfile>/sub`, which fails on every OS).
  - `/tmp/…` literals become `std::env::temp_dir()`.
  - Source-scanner relative paths are normalised to `/`.
  - t147's absolute fixtures become flavor-aware.
- **`.gitattributes`:** `* text=auto eol=lf`, binary for media, `.db`, `.7z`, `.png`. The
  Windows runner's git has `core.autocrlf=true`, and byte-exact fixtures read at run time
  would otherwise get CRLF.

### D12. Small Windows behaviour

- **Console:** `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]`
  in kt-app's `main.rs`. kt-cli stays a console program.
- **Icon and version resource:** `src/packaging/kinetren.ico` (generated from
  `kinetren.svg` and committed, with sizes 16–256 from `packaging/icons/`) plus a version
  string from kt-version, compiled by `winresource` with `x86_64-w64-mingw32-windres` in
  kt-app's `build.rs`, only when `CARGO_CFG_TARGET_OS == "windows"`.
- **DnD position:** the non-Linux branch of `dnd_pointer_pos` gets a Windows implementation:
  `GetCursorPos` + `ScreenToClient` on the window's HWND (from the eframe frame's raw window
  handle), converted by the pixels-per-point.
- **Locale:** `GetUserDefaultLocaleName` feeds the existing `from_locale` mapping. It
  already accepts `ru-RU`; `zh-Hans-CN` is covered by the `zh` prefix.
- **WARP detection:** "Microsoft Basic Render Driver" is added to the software-adapter
  names next to `llvmpipe`.

## Risks / Trade-offs

- **[FFmpeg 9 is gyan.dev's "latest" and winget's default `Gyan.FFmpeg.Shared`]** → Users
  who install it themselves get the named rejection, and the dialog offers Download (8.1.2).
  FFmpeg 9 support is a separate change. It's needed on Linux too, and it means vendored 9
  headers, a `v9` module and a review of API changes. Until it lands, README-windows names
  the version (`winget install Gyan.FFmpeg.Shared --version 8.1.2`).
- **[gyan.dev removes the 8.1.2 package or the server is down]** → The GitHub mirror with
  the same hash is the fallback. If both fail, the error names the URLs and the manual way
  (any shared 6/7/8 build plus Locate). Pinning means a Kinetren release is needed to move
  the pin; that's acceptable, since the pin moves only with a loader change anyway.
- **[wine passes what Windows fails]** (mandatory locks, rename semantics, WASAPI) → This is
  why the real `windows-latest` job runs the same archive. wine is the fast local loop, not
  the verdict.
- **[mingw libm differs from glibc]** → goldens per platform (D9); determinism between runs
  stays the guard everywhere.
- **[The `-gnu` exe accidentally needs `libwinpthread-1.dll`]** (a C dependency built with
  pthreads, or a future C++ dependency) → the PE check fails the build. The fix is
  static linking (`-C link-arg=-static` for that target), not shipping the DLL.
- **[SmartScreen warns on an unsigned exe]** → stated in README-windows. Signing is a later
  decision (cost and identity).
- **[Antivirus quarantines a freshly downloaded DLL]** → the loader reports "file vanished
  after install". Rescan, and the window, show the state. Nothing is retried automatically.
- **[Path flavor rules missing an exotic form]** (`\\?\Volume{…}`, `\\.\`, subst drives)
  → such paths are left verbatim and stay offline placeholders. The writer never rewrites
  what it doesn't understand.
- **[The download adds network code to kt-app]** → only on click, no background traffic,
  shared HTTP stack with telemetry. The feature note records it, as required for the
  `docs/features/` network rule.

## Migration Plan

Linux users see no change. Data paths, lock files and project files are byte-identical
(t830/t831/t362 stay green, and a task checks the eframe storage path is unchanged).
Windows is a first install, so nothing migrates. Rollback: the Windows parts are additive
behind `cfg(windows)` and the Windows-flavor pure functions. Reverting the change removes
the zip from releases and leaves Linux as it is.

## Open Questions

- Whether the GitHub mirror's `.7z` is byte-identical to gyan.dev's (same hash), or needs
  its own committed hash. This is checked at apply time and doesn't change the design.
- Whether the gnu dist exe keeps function names in crash reports after
  `strip = "debuginfo"`. A task checks a forced panic, and the fallback is fixed in D1.

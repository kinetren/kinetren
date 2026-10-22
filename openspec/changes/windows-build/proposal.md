# Proposal

## Why

Kinetren ships only for Linux. The owner asked for a Windows build (2026-10-22). This
touches more than the build. The FFmpeg loader knows only Linux SONAMEs and the unix
`dlopen` API. There is no "system FFmpeg" on Windows that `LoadLibrary` would find. The
data directory, project lock, atomic save, file URLs and audio output all assume Linux.
An audit of the tree found two blockers (the unix-only loader; bindgen and the build host)
and about twenty places that would build but misbehave on Windows. Some of them do damage:
the data directory falls back to the working directory, and the `kinotrain → kinetren`
migration would then rename a `kinotrain` folder there.

Owner's decisions (2026-10-22): keep the rule "no FFmpeg build of our own" and *find* the
user's installed shared FFmpeg, and also let the user download the gyan.dev shared release
build from inside the program. Cross-build in the existing Docker container with mingw.
First delivery is a portable zip.

## What Changes

- **FFmpeg on Windows is found, not shipped.** The loader gets a per-OS candidate table:
  `avcodec-62.dll`/`avutil-60.dll`/… for 8, `-61/-59` for 7, `-60/-58` for 6. On Windows it
  searches **directories** in a fixed order: the folder the user chose, `KINETREN_FFMPEG_DIR`,
  `ffmpeg\` next to `kinetren.exe`, Kinetren's own download folder, `PATH` (following
  winget's `Links` symlink of `ffmpeg.exe` to the real `bin\`), then known scoop, winget and
  `C:\ffmpeg` locations. All five DLLs are loaded **by full path from one directory, in
  dependency order** (`avutil → swresample → swscale → avcodec → avformat`), so a different
  `avutil-NN.dll` elsewhere on `PATH` can't be bound instead. Every rejected directory is
  reported with its reason: a static build with no DLLs, an unsupported major (FFmpeg 9 is
  `avcodec-63.dll`), or a set mixed from two directories. A failed load is no longer cached
  for the life of the process, so a rescan after an install or download works without a
  restart. Linux behaviour is unchanged.
- **Download FFmpeg from inside the program (Windows).** The "FFmpeg not found" notice and a
  new *View → FFmpeg…* window (next to Language; there is no Help menu) offer **Download**, **Locate folder…** and **Rescan**.
  Download fetches one **pinned** release, `ffmpeg-8.1.2-full_build-shared.7z` from
  `https://www.gyan.dev/ffmpeg/builds/packages/`, and checks it against a SHA-256 committed
  in our source. The `ffmpeg-release-full-shared` "latest" link is avoided on purpose: today
  it is FFmpeg 9, which the loader does not support. Only `bin\*.dll` and the licence are
  extracted, into `%LOCALAPPDATA%\kinetren\ffmpeg\8.1.2\`, and the folder appears only when
  complete. The download runs as a background job with progress and cancel, and starts only
  when the user clicks; the dialog names the source and the GPLv3 licence first. Media
  already showing placeholders recovers the same way as when a missing file comes back.
- **Cross-build for `x86_64-pc-windows-gnu` in the Docker container**: a Windows stage of
  `src/packaging/Dockerfile` (mingw-w64, clang for bindgen with the Windows target, wine for
  tests). `make windows` produces `kinetren-<version>-windows-x86_64.zip` (`kinetren.exe`,
  `kt-cli.exe`, `README-windows.txt`, licences) and its `.sha256`. **Invariant 10 on Windows**:
  the PE import table of both exes names no `av*`/`sw*` DLL and no mingw runtime DLL
  (`libgcc_s`, `libstdc++`, `libwinpthread`). It is checked inside the container like `ldd`.
- **Tests on Windows**: `make windows-test` runs the workspace tests under wine in the
  container. CI gets a `windows-latest` job that runs the cross-built nextest archive
  (fixtures included) on real Windows with the pinned gyan.dev FFmpeg. The release workflow
  attaches the zip next to the Linux tarball.
- **Per-user data on Windows**: `%APPDATA%\kinetren\data`, the same directory eframe already
  uses for its own storage. FFmpeg downloads go to `%LOCALAPPDATA%` (100 MB must not roam).
  Never the working directory. The `kinotrain` migration runs only where a former install
  can exist (Linux). One resolver serves both kt-app and kt-cli.
- **Project lock on Windows**: a real `LockFileEx` lock (it was a no-op returning "free").
  The pid moves to a separate file, because a mandatory lock makes the locked file
  unreadable to the second copy. The lock key normalises case and the `\\?\` prefix.
- **Atomic save on Windows**: replace with `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`,
  retried within a short budget on sharing violations (antivirus, sync clients, editors).
  Write-through counts as the directory sync, so the "not synced" notice doesn't appear on
  every save.
- **Media paths across OSes**: correct `file:///C:/…` and UNC `file://server/share/…` URLs
  both ways. A different drive falls back to an absolute URL, not `..` chains. Comparison is
  case-insensitive on Windows. A path that is absolute on *another* OS (`/home/u/a.mp4` read
  on Windows) stays an offline placeholder showing that path, instead of being joined to the
  project folder. This takes up the item deferred in `project-format/design.md`
  ("cross-platform paths in `target_url`").
- **Audio output edge**: a WASAPI shared-mode device at 44.1 kHz gets a real resampler at
  the output edge, preallocated with no allocations in the callback. Today it gets a
  repeat/skip "resampler". A 5.1/7.1 device gets L/R into FL/FR and silence elsewhere; today
  the right channel is copied into centre, LFE and surrounds. The engine stays 48 kHz/2ch
  (invariant 1).
- **Encoder cascade**: add `h264_amf` and `h264_mf`. `h264_qsv` opens with NV12, because
  YUV420P made it fail. `h264_vaapi` is tried only on Linux.
- **Small Windows fixes**: UI language from `GetUserDefaultLocaleName`; drop position in DnD
  from `GetCursorPos` (today a dropped file always lands at the playhead); no console window
  in release builds; exe icon and version resource; `.gitattributes` keeps LF so byte-exact
  fixtures survive a Windows checkout; tests that use `/proc/…` or `/tmp/…` as "a path that
  can't be written" get a portable form.
- **Golden constants per delivered platform**: the CPU reference hashes become a table keyed
  by `(os, arch)`. The Windows entries are measured on the first green Windows CI run and
  committed. Hash tests that are ungated today and differ only through libm get the same
  table.

## Capabilities

### New Capabilities
<!-- none: everything lands in existing capabilities and in `distribution` (introduced by
     the in-flight github-ci-docker-build change, which is archived first). -->

### Modified Capabilities
- `media-index`: the FFmpeg requirement becomes per-OS. Linux keeps the SONAME `dlopen`. On
  Windows: the directory search, single-directory dependency-order load, a non-cached
  failure and a search report. Invariant 10's check gains its PE form. New requirement:
  in-program download of the pinned gyan.dev shared build.
- `durability`: data directory per OS (never cwd, migration only on Linux), project lock
  with `LockFileEx` + separate pid file + normalised key, and atomic replace with retry and
  write-through on Windows.
- `project-format`: file URLs with drive letters and UNC, relativising across drives and
  case, and absolute paths from a foreign OS.
- `export`: Windows hardware encoders in the cascade and QSV pixel format; golden constants
  per `(os, arch)`.
- `transport`: conversion at the audio-output edge for device rates other than 48 kHz and
  for multichannel layouts.
- `ui-reach`: the FFmpeg window and notice (Download / Locate / Rescan), reachable from
  the View menu and from the missing-FFmpeg notice.
- `distribution`: Windows build stage, `make windows` / `make windows-test`, Windows CI job,
  zip release asset, and the PE delivery check.

## Impact

- Code: `src/libs/kt-ffmpeg/src/{loader.rs,lib.rs,muxer_impl.rs}` (per-OS loader, retryable
  backend, cascade); `src/libs/kt-otio/src/{url.rs,io.rs}`; `src/apps/kt-app/src/{journal.rs,
  project_lock.rs,audio_out.rs,i18n/detect.rs,app.rs,main.rs,user_store.rs}`; new
  `src/apps/kt-app/src/ffmpeg_setup.rs` (window, download job); `src/apps/kt-cli/src/main.rs`
  (lock dir via the shared resolver; `kt-cli ffmpeg` prints the search report). New small
  crate `src/libs/kt-dirs` (per-user directories; no dependencies besides `windows-sys` under
  `cfg(windows)`), used by kt-app and kt-cli.
- Dependencies: `windows-sys` (cfg(windows)); `ureq` + `rustls` in kt-app (the same choice
  as `telemetry-posthog` D2, so whichever change lands first adds it); a pure-Rust 7z
  extractor (`sevenz-rust2`) and `sha2` in kt-app; `winresource` as a build dependency of
  kt-app. Nothing new in kt-time/kt-model/kt-fx/kt-cache/the kt-engine core.
- Build and delivery: `src/packaging/Dockerfile` (Windows stage), `Makefile`
  (`windows`, `windows-test`), `src/scripts/release-build.sh` (`--target windows`, PE
  import check), `.github/workflows/{ci.yml,release.yml}`, `src/scripts/dist-check.py` and
  t834/t835, new `.gitattributes`, `.cargo/config.toml` (mingw linker for the target).
- Docs: root `CLAUDE.md` (invariant 10 wording, Windows build line), README (Windows
  install: unzip, FFmpeg via the Download button or `winget install Gyan.FFmpeg.Shared`
  with a supported version), `CONTRIBUTING.md`.
- Order: after `github-ci-docker-build` is archived, because `distribution` must exist and
  the Dockerfile, Makefile and workflows are its files. It shares `ureq` with
  `telemetry-posthog`.
- Not in scope: an installer and `.kt` file association (portable zip first), code signing
  (SmartScreen will warn), ARM64, **FFmpeg 9 support**. FFmpeg 9 is a separate change, and
  Linux needs it too, since distributions will ship it. Until then a found FFmpeg 9 is
  reported by name, and the Download button installs 8.1.2.
- Feature note: the in-program "find or download the codec library" flow has a precedent in
  other programs (Audacity's *Libraries → FFmpeg: Locate / Download*), so a note goes into
  `docs/features/` before code (task 0.2), plus a `gyan.dev` row in
  `docs/features/_services.md`.

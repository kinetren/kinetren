# Proposal

## Why

FFmpeg 9.0 "Lei" came out in August 2026 (9.0.2 on 2026-09-18). All five libraries we load got
a new major: avutil 61, swresample 7, swscale 10, avcodec 63, avformat 63. The loader knows only
6/7/8. On a Linux system that moved to FFmpeg 9 (rolling and new distributions), Kinetren opens no
media at all. On Windows, a folder with `avcodec-63.dll` is rejected as "FFmpeg 9 is not supported
yet", even though it is what gyan.dev's "latest" link and a plain `winget install` now give.
windows-build deferred this as a separate change, because Linux needs it too (media-index design,
"Deferred").

Owner's decisions (2026-10-26, `docs/user-feedback.md`): the Windows Download button moves to
gyan.dev **9.0.2**, and the windows-latest CI job tests 9.0.2 only. On Linux, FFmpeg 9 is proven
against a **9.0.2 built from the official source tarball in Docker**. That build is for tests
only and is never shipped.

## What Changes

- **FFmpeg 9 is a supported major on both OSes.** The headers of 9.0.2 are vendored as
  `vendor/ffmpeg-9/include` (taken from the same pinned source tarball by a script). `build.rs`
  generates `types_v9.rs` from them, and the `v9` version module and the `Backend/Source/Muxer::V9`
  arms are added. The library table gets the row `9: avutil 61, swresample 7, swscale 10,
  avcodec 63, avformat 63`. Linux tries SONAMEs from 9 down to 6, and inside a Windows folder the
  majors are tried from 9 down to 6. Any API that FFmpeg 9 removed and that our code uses is
  handled in the version module only, never with a runtime branch elsewhere.
- **The "not supported" reason moves on to FFmpeg 10** (avcodec 64). A folder with only a newer
  major is still named with its release, as before.
- **The table cannot drift from the headers.** A new test compares each row of the major table
  with the `LIB*_VERSION_MAJOR` constants of the matching vendored headers.
- **Windows download pin: 8.1.2 → 9.0.2.** It is still a single pinned archive
  (`ffmpeg-9.0.2-full_build-shared.7z`, gyan.dev plus its byte-identical GitHub mirror), with the
  SHA-256 in our source and the size shown before the download. It goes into
  `%LOCALAPPDATA%\kinetren\ffmpeg\9.0.2\`. **An earlier download keeps working:** version folders
  of earlier pins under the download root are searched after the current pin's folder. A person
  who downloaded 8.1.2 does not lose FFmpeg after the update. Nothing is deleted automatically.
- **The Windows tests run on 9.0.2.** These are the windows-latest CI job, `make windows-test`
  under wine, `fetch-ffmpeg-win.sh`, t846 (now major 63) and the t850 path text. The hints, the
  FFmpeg window label, README and `README-windows.txt` name 9.0.2 and "FFmpeg 6, 7, 8 or 9".
- **Linux FFmpeg 9 proof in Docker.** A new `ffmpeg-test` stage of `src/packaging/Dockerfile`
  builds shared FFmpeg 9.0.2 from `ffmpeg-9.0.2.tar.xz` (SHA-256 pinned; GPL with libx264, so the
  export cascade has its usual encoder) and registers it with the dynamic linker, so it is found
  by SONAME like a distribution's FFmpeg 9. `make ffmpeg-test` (`src/scripts/ffmpeg-test.sh`) runs
  the FFmpeg-dependent tests in that stage. A new CI job
  does the same on every push. Each run asserts that the major it was meant to test is the one
  that loaded (`KT_EXPECT_FFMPEG_MAJOR=9`), so a silent fallback to another FFmpeg cannot pass as
  a green FFmpeg 9 run. The existing Linux job keeps testing the runner's FFmpeg 6.
- **Invariant 10 wording** ("6/7/8") becomes "6/7/8/9" in the root `CLAUDE.md`,
  `openspec/config.yaml`, `docs/architecture.md`, `CONTRIBUTING.md`, the README badge and the
  crate notes.

No project-format, model or UI-layout change. The binary still links no libav*, and FFmpeg 6/7/8
keep loading as before.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `media-index`: the runtime-FFmpeg requirement gains major 9 (the SONAME order and the
  Windows DLL table), the table-vs-headers check, and "not supported" moving to FFmpeg 10. The
  Windows download requirement is re-pinned to 9.0.2 and searches earlier download folders. The
  search-report hint names 6–9. The t846 scenario now loads the 9.0.2 build.
- `ui-reach`: the FFmpeg window offers **Download FFmpeg 9.0.2 (gyan.dev, GPLv3)**, and the t850
  path shows "FFmpeg 9 (avcodec 63.x)" on Windows.
- `distribution`: the Windows test requirement uses the 9.0.2 pin. A new requirement covers the
  Linux FFmpeg 9 proof (the Docker stage, `make ffmpeg-test`, the CI job, the expected-major
  assertion), with a dist-check guard on its shape.

`export` keeps its requirements: the cascade is tried by a real open on whatever major is loaded,
and the FFmpeg 9 run proves it there (t33 and the export tests in the `ffmpeg-test` run).

This is not a new feature: it keeps an existing contract working on the current FFmpeg release.
There is no new `docs/features/` note. After archiving, the environment note
`docs/features/resilience/environment/system-libraries-and-drivers.md` gets its verdict updated.

## Impact

- **Code:** `src/libs/kt-ffmpeg` (`build.rs`, `lib.rs`, `search.rs`, `loader.rs`, a new vendored
  `vendor/ffmpeg-9/include` of about 11 MB like the other three, tests `loader_plan.rs` and
  `loader_windows.rs`), `src/libs/kt-dirs` (`FFMPEG_DOWNLOAD_VERSION`, the download root),
  `src/apps/kt-app/src/ffmpeg_setup.rs` (the pin, hash, size and URLs, t848) and `reach_tests.rs`
  (t850), and `src/apps/kt-cli` (the report, `dist_guard.rs`).
- **Build/CI:** `src/packaging/Dockerfile` (the `ffmpeg-test` stage), `Makefile`
  (`ffmpeg-test`), new `src/scripts/ffmpeg-test.sh` and `src/scripts/vendor-ffmpeg-headers.sh`,
  `src/scripts/fetch-ffmpeg-win.sh`, `src/scripts/windows-test.sh`, `src/scripts/dist-check.py`,
  `.github/workflows/ci.yml` (the Windows pin and the new Linux job). The new job adds about
  10–15 minutes of runner time per push, mostly compiling FFmpeg, and the Docker layer cache
  shortens it.
- **Docs:** README, CONTRIBUTING, `docs/architecture.md`, `README-windows.txt`, root and crate
  `CLAUDE.md`, `openspec/config.yaml`.
- **Users:** Linux with FFmpeg 9 works for the first time. On Windows, new downloads get 9.0.2,
  and existing 8.1.2 downloads keep working.

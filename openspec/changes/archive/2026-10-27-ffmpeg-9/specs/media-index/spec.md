# Spec Delta

## MODIFIED Requirements

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

# Spec Delta

## ADDED Requirements

### Requirement: Windows build — cross-compiled in the same container, portable zip
The Windows build SHALL be cross-compiled for `x86_64-pc-windows-gnu` from a `windows`
stage of the one committed Dockerfile (`src/packaging/Dockerfile`). It uses the same
pinned Rust toolchain, mingw-w64 as linker and C compiler, clang for bindgen with the
Windows target, and wine for running tests. `make windows` (and `release-build.sh --target
windows`) SHALL produce `src/target/dist-out/kinetren-<version>-windows-x86_64.zip` with a
`.sha256` next to it. The zip holds `kinetren.exe`, `kt-cli.exe`, `README-windows.txt` (how
to get FFmpeg: the program's Download button, or a shared build of a supported major via
winget/scoop) and the licence files, and no FFmpeg DLL. The delivery checks SHALL run inside
the container: the PE import table of both exes names no `av*`/`sw*` DLL and no mingw
runtime DLL (`libgcc_s*`, `libstdc++*`, `libwinpthread*`); every imported DLL belongs to the
Windows system set. A failed check leaves no zip in `dist-out`. The host still needs only
Docker, make, a POSIX shell and git.
Why: the owner chose a cross-build in Docker (2026-10-22): one machine, one container, one
`make` builds both deliveries, and the Linux owner can build the Windows zip without
Windows. A mingw runtime DLL missing from the zip is the most common way a cross-built exe
fails on a clean Windows ("libgcc_s_seh-1.dll was not found"), so the exe must not need one.
Shipping FFmpeg DLLs would break the owner's rule "no FFmpeg build of our own" and bring
licence duties for distributing it.

#### Scenario: The Windows parts of the tree are consistent
- **WHEN** `src/scripts/dist-check.py --check` runs on the repository; then on temporary copies where, one at a time, the Dockerfile lacks the `windows` stage, the Makefile lacks `windows` or `windows-test`, the release workflow doesn't attach the zip, the CI workflow lacks the Windows job, `release-build.sh` lacks the PE import check, or the zip recipe adds a `*.dll`
- **THEN** the repository passes; each copy fails and the violation is reported with its file
- **TEST** t859 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The zip passes the delivery checks
- **WHEN** `make windows` finishes
- **THEN** both exes import only Windows system DLLs (no `av*`, `sw*`, `libgcc_s*`, `libstdc++*`, `libwinpthread*`), the zip has no `.dll`, and `sha256sum -c` passes on the pair; otherwise the build fails and no zip is left
- **TEST** guard inside `src/scripts/release-build.sh` (invariant 10, Windows form; media-index scenario "Release binary is not linked against libav*")

### Requirement: Windows tests — wine in the container, real Windows in CI
`make windows-test` SHALL build the workspace tests for the Windows target and run them
under wine in the container. GPU and live scenarios skip themselves with a named reason,
and tests that need FFmpeg use the pinned gyan.dev build, fetched once into the container's
cache and checked against the committed SHA-256. The CI workflow SHALL have a
`windows-latest` job that receives the cross-built nextest archive (test binaries plus the
generated fixtures) from the Linux job and runs it on real Windows with the same pinned
FFmpeg build. The job keeps the CI rules: read-only permissions, no lasting uploads, and
only a failure log with `retention-days` of 3 or fewer.
Why: a cross-built exe that has never run on Windows proves nothing. wine catches most
mistakes locally in minutes. The real runner catches what wine forgives: mandatory file
locks, sharing violations on rename, WASAPI, and libm bits for the golden constants. Building
once and running the archive keeps the single toolchain the owner chose: nothing is compiled
on the Windows runner.

#### Scenario: The Windows job runs the cross-built archive
- **WHEN** `dist-check.py --check` reads `ci.yml`
- **THEN** the Windows job runs on `windows-latest`, depends on the Linux job, downloads the nextest archive and the pinned FFmpeg (verifying its SHA-256), runs `cargo nextest run --archive-file …` with no compilation step, and uploads only a failure log with `retention-days ≤ 3`; the archive itself is passed between jobs with `retention-days: 1`
- **TEST** t859 (`src/apps/kt-cli/tests/dist_guard.rs`)

### Requirement: A release carries the Windows zip
The release workflow SHALL build the Windows zip through `release-build.sh --target windows`
in the same run as the Linux tarball, and attach `kinetren-<version>-windows-x86_64.zip`
and its `.sha256` to the same GitHub Release. If either build fails, no release is created.
Why: one tag, one release, both platforms. A release missing one platform would look
complete and send Windows users to an older version.

#### Scenario: A tag produces both assets
- **WHEN** the owner pushes a `v*` tag on a commit of the default branch
- **THEN** the release has the Linux tarball and the Windows zip, each with its `.sha256`, and `sha256sum -c` passes on both pairs; the zip unpacked on a clean Windows 10/11 starts `kinetren.exe`, shows the "FFmpeg not found" notice, and after **Download** plays a clip
- **TEST** manual check after the first tag with this change, recorded in `tasks.md`; the workflow shape is t859

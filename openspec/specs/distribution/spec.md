# distribution Specification

## Purpose
How Kinetren gets from the repository to people: one build container shared by everyone,
the `make` commands a newcomer runs, the checks GitHub runs on every push, and the releases
it publishes on a tag. Only releases are kept for good.

## Requirements

### Requirement: One build container, described by a committed file
The portable build SHALL be described by one committed Dockerfile
(`src/packaging/Dockerfile`, base Ubuntu 22.04, glibc 2.35) that pins an exact Rust
toolchain version. `src/scripts/release-build.sh`, `make` and the release workflow SHALL
all build through this file. None of them may carry a second copy of the image recipe. The
delivery checks (no `libav*` in `ldd` of `kinetren`/`kt-cli`, the required `GLIBC_` at most
2.35) SHALL run inside the container, so the host needs only Docker, make, a POSIX shell and
git (git only for the version string; without it the version is `kt00.00.00-nogit`). The
host's `.cargo/config.toml` (mold) MUST NOT reach the container build. The produced tarball
SHALL be `src/target/dist-out/kinetren-<version>-linux-x86_64.tar.gz` with a `.sha256` next
to it, plus `LATEST` naming the version and the path.
Why: the image recipe used to live in a heredoc inside `release-build.sh`, and the checks
called `ldd`/`objdump` on the host. A newcomer without binutils, or a CI job that built the
image a second way, would get a different result from the one the owner checks.

#### Scenario: The committed tree is consistent
- **WHEN** `src/scripts/dist-check.py --check` runs on the repository
- **THEN** exit 0: exactly one Dockerfile exists, the toolchain version in it is the one CI
  installs, `release-build.sh` builds from that file and has no inline recipe, the Makefile
  has the targets `all build install run test clean help`, both workflows exist with the
  triggers and permissions required below, and the README's "Install" section shows
  `make` and `make install`
- **TEST** t834 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The guard catches each class of drift
- **WHEN** it runs on a temporary copy where, one at a time, the Dockerfile's toolchain
  differs from CI's, `release-build.sh` has a heredoc `FROM`, a Makefile target is missing,
  the CI workflow uploads an artifact without `retention-days`, the release workflow
  triggers on something other than `v*` tags, the README's install section lost `make install`, the desktop
  entry starts `kt-app`, the tarball carries `kt-app`, or a second Dockerfile appears
- **THEN** exit non-zero, and each violation is reported with its file
- **TEST** t835 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The delivered binaries pass the delivery checks
- **WHEN** `release-build.sh` (or `make`) finishes
- **THEN** both binaries have no `libav*` in `ldd` and need at most `GLIBC_2.35`; otherwise
  the build fails and no tarball is left in `dist-out`
- **TEST** guard inside `src/scripts/release-build.sh` (invariant 10; media-index scenario
  "Release binary is not linked against libav*")

### Requirement: The program is called `kinetren` on disk
The GUI binary SHALL be named `kinetren` everywhere a person meets it: the build output
(`src/target/<profile>/kinetren`), the release tarball, `~/.local/bin/kinetren` after
installation, the `.desktop` file (`Exec=kinetren %f`, `StartupWMClass=kinetren`, matching
the window's application id `kinetren`) and `make run`. The cargo package MAY stay `kt-app`
(`cargo run -p kt-app` keeps working); `kt-cli` keeps its name. `install.sh` SHALL remove a
`~/.local/bin/kt-app` left by an older install, the same way it already removes `kinotrain`
leftovers, so the menu doesn't start an old build. No tarball, desktop entry or script
that people run may start a binary named `kt-app`.
Why: the owner's request ("when building and distributing, call it kinetren, not kt-app …");
`kt-app` is an internal crate name that leaked into what people install.

#### Scenario: Packaging names agree
- **WHEN** `dist-check.py --check` reads the manifest of `kt-app`, `install.sh`,
  `kinetren.desktop`, `release-build.sh` and the Makefile
- **THEN** the binary is `kinetren` in all of them, the desktop `Exec`/`StartupWMClass`
  name `kinetren`, and `install.sh` installs `kinetren` and removes `kt-app`; a temporary
  copy with `Exec=kt-app` or a tarball list with `kt-app` fails with that file named
- **TEST** t834, t835 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The installed menu entry starts the installed binary
- **WHEN** a tarball is installed over an older install that has `~/.local/bin/kt-app`
- **THEN** `~/.local/bin/kinetren` exists, `~/.local/bin/kt-app` does not, the window's
  WM_CLASS is `kinetren` (checked with `xprop` under Xvfb)
- **TEST** manual check recorded in `tasks.md`

### Requirement: Simple make commands at the repository root
A `Makefile` at the repository root SHALL provide:
- `make` / `make build`: the tarball through the build container.
- `make install`: build if needed, then the tarball's own `install.sh` into `~/.local`.
- `make run`: start the built `kinetren` from `dist-out` without installing it.
- `make test`: `cargo test --workspace` on the host, for developers, with a message naming
  the host requirements if `cargo` is missing.
- `make clean`: remove `dist-out` and the container's target directory, but not the cargo
  registry volume.
- `make help`: lists every target with one line each.

Every target that builds SHALL need nothing on the host besides Docker, make and git. The
cargo registry and the container's target directory SHALL persist between runs, so a second
`make` after no change rebuilds nothing.
Why: the owner wants people to stop "suffering through the README": the old quick start
asked for a Rust toolchain, clang and dev packages before anything ran. Running the GUI
inside Docker was rejected (X11/Wayland, GPU and ALSA passthrough are fragile); the
container builds, the host runs.

#### Scenario: A newcomer builds and installs with two commands
- **WHEN** on a machine with Docker, make, git and the FFmpeg 6/7/8 runtime libraries,
  `make` and then `make install` run in a fresh clone
- **THEN** `~/.local/bin/kinetren` exists and starts, and `kinetren.desktop` is installed;
  `make` again finishes without recompiling
- **TEST** manual check recorded in `tasks.md` (Docker isn't available to `cargo test`);
  the targets' existence is t834

### Requirement: CI on every push and pull request
`.github/workflows/ci.yml` SHALL run on every push and every pull request to any branch.
It builds the workspace, runs `cargo clippy --workspace --all-targets` (warnings are reported, not fatal, until
the existing ones are fixed), runs the workspace tests
(nextest) with the system FFmpeg and the `ffmpeg` binary for fixtures, and runs the
repository guards (`readme-check.py`, `dist-check.py`, `specs-user-path.py`). GPU
conformance SHALL skip itself for lack of an adapter, as it already does, and the live Xvfb
scenarios are NOT run in CI. A newer push to the same branch SHALL cancel the run in
progress. The workflow SHALL have read-only repository permissions. It MUST NOT upload build
outputs. Its only upload is the test log of a failed run, with `retention-days` of 3 or fewer.
Why: the owner wants automatic checks on the free plan, while storing only releases. In a
public repository, minutes are free and unlimited, but anything uploaded lingers 90 days by
default. Build caches live in the Actions cache (capped at 10 GB per repository and evicted
automatically), not in artifacts.

#### Scenario: Workflow shape is guarded
- **WHEN** `dist-check.py --check` reads `ci.yml`
- **THEN** it finds the push and pull_request triggers, `permissions: contents: read`, a
  concurrency group with cancel-in-progress, no upload without `retention-days ≤ 3`, and
  `RUSTFLAGS` cleared for the build (mold isn't on the runner)
- **TEST** t834, t835 (`src/apps/kt-cli/tests/dist_guard.rs`)

### Requirement: Release on a version tag
`.github/workflows/release.yml` SHALL run only when a tag matching `v*` is pushed. It builds
the tarball with `release-build.sh` through the committed Dockerfile, so the delivery checks
run on every release. Then it creates a GitHub Release named after the tag and attaches the tarball and its `.sha256`. A tag with a `-` suffix (`v26.11.01-rc1`) is
marked pre-release; any other tag is a normal release. It SHALL have `contents: write`
permission and no other write permission. A tag whose commit isn't on the repository's
default branch SHALL fail before building. Old releases are NOT deleted automatically.
Why: release assets don't count toward the Actions storage quota, so "keep only releases"
means: CI uploads nothing that lingers, and a release keeps its files. Deleting old releases
was rejected: people may need an older build, and it costs nothing to keep one.

#### Scenario: A tag produces a release with a verifiable tarball
- **WHEN** the owner pushes `v26.10.22` on a commit of the default branch
- **THEN** a GitHub Release `v26.10.22` exists with
  `kinetren-26.10.22-<hash>-linux-x86_64.tar.gz` and its `.sha256`, and
  `sha256sum -c` passes on the downloaded pair
- **TEST** manual check after the owner's first tag, recorded in `tasks.md`; the
  workflow shape is t834/t835

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
- **TEST** t860 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The zip passes the delivery checks
- **WHEN** `make windows` finishes
- **THEN** both exes import only Windows system DLLs (no `av*`, `sw*`, `libgcc_s*`, `libstdc++*`, `libwinpthread*`), the zip has no `.dll`, and `sha256sum -c` passes on the pair; otherwise the build fails and no zip is left
- **TEST** guard inside `src/scripts/release-build.sh` (invariant 10, Windows form; media-index scenario "Release binary is not linked against libav*")

### Requirement: Windows tests — wine in the container, real Windows in CI
`make windows-test` SHALL build the workspace tests for the Windows target and run them
under wine in the container. GPU and live scenarios skip themselves with a named reason,
and tests that need FFmpeg use the pinned gyan.dev build (9.0.2, the same pin as the program's
Download button), fetched once into the container's cache and checked against the committed
SHA-256. The CI workflow SHALL have a `windows-latest` job that receives the cross-built
nextest archive (test binaries plus the generated fixtures) from the Linux job and runs it on
real Windows with the same pinned FFmpeg build. The pin of the CI job, of
`fetch-ffmpeg-win.sh` and of the Download button MUST be one version with one SHA-256.
FFmpeg 6/7/8 on Windows are covered by the pure search-plan tests, not by a second run. The job
keeps the CI rules: read-only permissions, no lasting uploads, and only a failure log with
`retention-days` of 3 or fewer.
Why: a cross-built exe that has never run on Windows proves nothing. wine catches most
mistakes locally in minutes. The real runner catches what wine forgives: mandatory file
locks, sharing violations on rename, WASAPI, and libm bits for the golden constants. Building
once and running the archive keeps the single toolchain the owner chose: nothing is compiled
on the Windows runner. The run uses the build a new user downloads. The owner chose to test
only that build ("9.0.2, CI tests 9 only", 2026-10-26).

#### Scenario: The Windows job runs the cross-built archive
- **WHEN** `dist-check.py --check` reads `ci.yml`, `src/scripts/fetch-ffmpeg-win.sh` and `src/apps/kt-app/src/ffmpeg_setup.rs`
- **THEN** the Windows job runs on `windows-latest`, depends on the Linux job, downloads the nextest archive and the pinned FFmpeg (verifying its SHA-256), runs `cargo nextest run --archive-file …` with no compilation step, and uploads only a failure log with `retention-days ≤ 3`; the archive itself is passed between jobs with `retention-days: 1`; the version and SHA-256 of the job's FFmpeg equal those of the fetch script and of the Download button, and a copy where one of the three differs fails naming the file
- **TEST** t860 (`src/apps/kt-cli/tests/dist_guard.rs`)

### Requirement: A release carries the Windows zip
The release workflow SHALL build the Windows zip through `release-build.sh --target windows`
in the same run as the Linux tarball, and attach `kinetren-<version>-windows-x86_64.zip`
and its `.sha256` to the same GitHub Release. If either build fails, no release is created.
Why: one tag, one release, both platforms. A release missing one platform would look
complete and send Windows users to an older version.

#### Scenario: A tag produces both assets
- **WHEN** the owner pushes a `v*` tag on a commit of the default branch
- **THEN** the release has the Linux tarball and the Windows zip, each with its `.sha256`, and `sha256sum -c` passes on both pairs; the zip unpacked on a clean Windows 10/11 starts `kinetren.exe`, shows the "FFmpeg not found" notice, and after **Download** plays a clip
- **TEST** manual check after the first tag with this change, recorded in `tasks.md`; the workflow shape is t860

### Requirement: FFmpeg 9 on Linux is proven against a pinned source build
The build container SHALL have an `ffmpeg-test` stage. It builds the SHARED libraries of FFmpeg
9.0.2 from the official source tarball `ffmpeg-9.0.2.tar.xz`, whose SHA-256 is committed next
to the recipe. The configuration is GPL with libx264, so the export cascade has its usual
software encoder, with zlib (the PNG decoder every distribution build has), and no programs or
docs are built. The libraries SHALL be registered with the container's dynamic linker, so the
program finds them by SONAME exactly as it finds a distribution's FFmpeg 9; the container holds
no other FFmpeg. The stage also carries nextest. `make ffmpeg-test`
(`src/scripts/ffmpeg-test.sh`) SHALL run the tests that load FFmpeg (the packages `kt-ffmpeg`,
`kt-media`, `kt-engine`, `kt-cli`) in that stage with `KT_EXPECT_FFMPEG_MAJOR=9`. The fixtures
are generated on the host, as for the Windows tests.
The CI workflow SHALL have a Linux job that does the same on every push and pull request,
under the CI rules: read-only permissions, no lasting uploads, and only a failure log with
`retention-days` of 3 or fewer. When `KT_EXPECT_FFMPEG_MAJOR` is set, a test MUST fail unless
the loaded FFmpeg is that major. The libraries of this stage MUST NOT enter a tarball, a zip
or a release. The headers vendored for FFmpeg 9 MUST come from the same pinned tarball.
Why: the owner chose to prove FFmpeg 9 on Linux with a build from the official source in
Docker (2026-10-26). The runner and the owner's machine have only FFmpeg 6.1, and prebuilt
Linux shared builds are git snapshots whose links get pruned, so they cannot be pinned by
hash. Without the expected-major check, a run with a broken folder would quietly load another
FFmpeg (or skip the FFmpeg tests) and still pass as an FFmpeg 9 proof. Building it ourselves
for tests does not break "no FFmpeg build of our own": nothing built here reaches a user.

#### Scenario: The FFmpeg 9 run proves its major
- **WHEN** the tests run in the `ffmpeg-test` stage with `KT_EXPECT_FFMPEG_MAJOR=9`; then with the variable set to 9 and FFmpeg unreachable (`KINETREN_FFMPEG_SYSTEM=0`, no folder); then with the variable unset
- **THEN** the first run loads FFmpeg 9 (avcodec 63.x) by SONAME and the FFmpeg tests pass on it (t10, t21, t37, t112, t33, t796, t849); the second fails, naming the expected major and the search report; the third skips the check with a printed reason
- **TEST** t865 (`src/libs/kt-ffmpeg/tests/loader_plan.rs`); the run itself is `make ffmpeg-test` and the CI job

#### Scenario: The FFmpeg 9 parts of the tree are consistent
- **WHEN** `src/scripts/dist-check.py --check` runs on the repository; then on temporary copies where, one at a time, the Dockerfile lacks the `ffmpeg-test` stage or its SHA-256 check of the tarball, the vendoring script pins another version or hash than the Dockerfile, the Makefile lacks `ffmpeg-test`, the CI workflow lacks the FFmpeg 9 job or the job does not set `KT_EXPECT_FFMPEG_MAJOR`, or `release-build.sh` copies anything from the `ffmpeg-test` stage
- **THEN** the repository passes; each copy fails and the violation is reported with its file
- **TEST** t866 (`src/apps/kt-cli/tests/dist_guard.rs`)

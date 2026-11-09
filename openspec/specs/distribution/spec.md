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
  entry starts `kt-app`, the tarball carries `kt-app`, a second Dockerfile appears, or the
  build container mounts `src/` alone (the GUI compiles in `docs/guide/*.md` from the repository
  root; with `src/` as `/work` every release build failed, 2026-10-31)
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
Every job that compiles Rust SHALL restore a build cache before it compiles (the native `test`
job, the Windows cross-build, the FFmpeg 9 build), and every cache in the workflow SHALL be
written only by a run on the default branch; other branches and pull requests read it. A cache
key SHALL name what decides its content (the toolchain, `Cargo.lock`, the Dockerfile or the
manifest it builds), never a date or a run number.
Why: the owner wants automatic checks on the free plan, while storing only releases. In a
public repository, minutes are free and unlimited, but anything uploaded lingers 90 days by
default. Build caches live in the Actions cache (capped at 10 GB per repository and evicted
automatically), not in artifacts. The Windows cross-build compiled 308 crates from scratch on
every push (6.4 min) and was the whole critical path of a 13-min run; caches written from side
branches compete for the same 10 GB and evict the default branch's. The job has no disk clean-up:
after it the runner had 108 GB free, and the job used 5 GB.

#### Scenario: Workflow shape is guarded
- **WHEN** `dist-check.py --check` reads `ci.yml`
- **THEN** it finds the push and pull_request triggers, `permissions: contents: read`, a
  concurrency group with cancel-in-progress, no upload without `retention-days ≤ 3`, and
  `RUSTFLAGS` cleared for the build (mold isn't on the runner)
- **TEST** t834, t835 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: Only the default branch writes a cache
- **WHEN** `dist-check.py --check` runs on the repository; then on temporary copies where, one at
  a time, a job uses `actions/cache` (which saves from any branch), an `actions/cache/save` step
  has no condition on the default branch, or `Swatinem/rust-cache` has no such `save-if`
- **THEN** the repository passes; each copy fails, naming the job and the step
- **TEST** t1420 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: Every compiling job restores a cache keyed by its inputs
- **WHEN** `dist-check.py --check` runs on the repository; then on temporary copies where, one at
  a time, the `windows-build` or `ffmpeg9` job compiles without restoring its target directory, or
  a cache key lacks the toolchain or `Cargo.lock` hash
- **THEN** the repository passes; each copy fails, naming the job and what its key lacks
- **TEST** t1421 (`src/apps/kt-cli/tests/dist_guard.rs`)

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
The Windows build SHALL be cross-compiled for `x86_64-pc-windows-gnu` from the `windows-cross`
stage of the one committed Dockerfile (`src/packaging/Dockerfile`), which holds the same
pinned Rust toolchain, mingw-w64 as linker and C compiler, and clang for bindgen with the
Windows target. The `windows` stage is `windows-cross` plus wine for running tests; the CI
job, which runs nothing Windows, compiles in `windows-cross`. `make windows` (and `release-build.sh --target
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
Wine is 2.5 GB of the 5.5 GB image, so the CI job does not carry it.
Shipping FFmpeg DLLs would break the owner's rule "no FFmpeg build of our own" and bring
licence duties for distributing it.

#### Scenario: The Windows parts of the tree are consistent
- **WHEN** `src/scripts/dist-check.py --check` runs on the repository; then on temporary copies where, one at a time, the Dockerfile lacks the `windows-cross` or `windows` stage, the Makefile lacks `windows` or `windows-test`, the release workflow doesn't attach the zip, the CI workflow lacks the Windows job, `release-build.sh` lacks the PE import check, or the zip recipe adds a `*.dll`
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

### Requirement: A release build asks once a day, in the background, whether a newer release is out
A release build of the GUI SHALL, after its first frame is drawn, check whether the latest
normal release of `github.com/kinetren/kinetren` is newer than the running build — at most once
per UTC calendar day per data directory. The check SHALL be one HTTPS `GET` of
`https://api.github.com/repos/kinetren/kinetren/releases/latest` made from a background thread,
carrying no query string, no cookie and no header that identifies the person or the machine:
only `Accept: application/vnd.github+json` and `User-Agent: kinetren/<kt_version::FULL>`. The UI
thread SHALL only start the thread and later read its result; it MUST NOT wait for it, and
`on_exit` MUST NOT wait for it either. The connect timeout SHALL be 5 s and the whole request
10 s; the body read SHALL stop at 1 MiB. A failed check (no network, timeout, HTTP error,
unparsable body) SHALL be silent — one `journal::log` line, no notice, no title change — and
SHALL NOT mark the day as checked, so the next start tries again. `kt-cli` SHALL never check.

The result of a successful check SHALL be stored in `<data_dir>/update-check` as plain text:
the UTC day of the check and the version of the latest normal release (or nothing, when the
release carries no recognisable version). A start on a day already recorded SHALL send no
request and use the stored release version; an unreadable or malformed file SHALL count as "not
checked today" and be rewritten after the next successful check.

"Newer" SHALL be decided only from the release's asset names, which carry the binary's own
version form (`kinetren-<YY.MM.DD>-<hash8>-<os>-<arch>.<ext>`, `release-build.sh`): the release is
newer if and only if its date is strictly later than the running build's commit date AND its
hash differs. A release marked `prerelease` or `draft`, a release with no asset of that form, an
older or same-day release, and a running build without a git version (`kt00.00.00-nogit`) SHALL
all count as "not newer"; the last one SHALL not even send the request.
Why: the owner asked for a reminder that the version is old, checked "if today we have not
checked yet", without bothering the person at start (2026-10-27). The tag is only the release's
name (distribution design: "the tag is only the release name"), and a tag can be dated later
than the commit it points to — comparing tags with the binary's commit date would call the very
release a person just downloaded "outdated". Being wrong in the "not newer" direction costs one
day of silence; being wrong in the other direction teaches the person to ignore the mark.

#### Scenario: "Newer" is decided from asset names, conservatively
- **WHEN** the decision is asked for the running build `kt26.10.20-aaaaaaaa` against: a release
  with asset `kinetren-26.11.01-bbbbbbbb-linux-x86_64.tar.gz`; the same asset in a release with
  `prerelease: true`; with `draft: true`; asset `kinetren-26.10.20-bbbbbbbb-…` (same day, other
  hash); `kinetren-26.11.01-aaaaaaaa-…` (same hash); `kinetren-26.10.01-cccccccc-…` (older); a
  release with tag `v26.12.01` and only a `notes.txt` asset; a body that is not JSON; and the
  running build `kt00.00.00-nogit` against the first release
- **THEN** only the first case is "newer" and it reports `kt26.11.01-bbbbbbbb`; every other
  case is "not newer"; the `nogit` build reports that no check is to be made at all; and
  `src/scripts/release-build.sh` still names its assets `kinetren-$VERSION-$TARGET-$ARCH`
  (the form the decision parses — a rename there would silently make every release "not newer")
- **TEST** t872 (`src/apps/kt-app/src/update_check.rs`)

#### Scenario: Once a day, the day's answer is reused, a failure does not count
- **WHEN** in an empty temp data directory the "is a check due" question is asked for UTC day
  2026-10-27; then a successful check with a newer release `kt26.11.01-bbbbbbbb` is recorded;
  then the question is asked again for 2026-10-27 and for 2026-10-28; then the file is replaced
  by `garbage`; then, in a fresh directory, a failed check is recorded for 2026-10-27
- **THEN** the first answer is "due"; after the record the same day is "not due" and returns
  the stored `kt26.11.01-bbbbbbbb`, the next day is "due"; after `garbage` the answer is "due"
  and nothing panics; after the failed check the same day is still "due" and the file does not
  exist
- **TEST** t873 (`src/apps/kt-app/src/update_check.rs`)

#### Scenario: The request is anonymous and a dead server costs the UI nothing
- **WHEN** the checker points at a local `TcpListener` that accepts and never answers, the
  check is started and the checker is dropped (what `on_exit` leaves behind); then at a local
  listener that answers `200` with a release JSON
- **THEN** starting returns in under 50 ms (it only spawns the thread), dropping returns in
  under 50 ms (nothing joins the thread), no notice is produced and the day is not recorded; against the answering listener exactly one `GET` with
  path `/repos/kinetren/kinetren/releases/latest` and no query string arrives, it carries
  `Accept` and `User-Agent: kinetren/<FULL>` and no header outside the allow-list `Host`,
  `Accept`, `User-Agent`, `Accept-Encoding`, `Connection`, and the text of the request holds neither the user name
  nor the host name nor the data directory path
- **TEST** t874 (`src/apps/kt-app/src/update_check.rs`)

### Requirement: A newer release is said in the bell and in the title, never in a pop-up
When the day's check (or the stored answer of today's check) says a newer release is out, the
program SHALL post exactly ONE info notice to the notices window — on the start whose check
found it, not on later starts the same day — naming the running version, the new version, the
address `github.com/kinetren/kinetren/releases` and the File › Preferences… item that switches the
check off. The window title SHALL end with the running version followed by a short translated word —
`kt26.10 (outdated)` in a release build — both in the OS window title and in the program's own
title row, where the word SHALL be painted in the warning colour right after the version, and
hovering it SHALL show in the status-bar hint the new version and the releases address. The
program MUST NOT open a window, a dialog or a modal for this, MUST NOT download anything and
MUST NOT change any installed file. When the check is switched off, neither the notice nor the
title word SHALL appear, even from a stored answer.
Why: the owner's words — "remind him in the bell notice that the version is old (but do not
bother him with pop-up windows at startup), and also write at the top in the title bar,
somehow briefly, that the version is outdated" (2026-10-27); he chose "kt26.10 (outdated)"
over showing the new number in the title. The kt-app checklist forbids pop-ups and new
windows (rules 10 and 22) and new strips for rare labels (rule 13): the title row is already
occupied by the version, so the mark costs no space.

#### Scenario: The newer release reaches the bell once and the title stays marked
- **WHEN** the harness app starts with the check on and an injected check result "newer:
  `kt26.11.01-bbbbbbbb`" for the running build `kt26.10.20-aaaaaaaa`; the frame is drawn; the
  pointer is moved over the title word; then the app is rebuilt with File → New; then a
  second start the same day reads the stored answer
- **THEN** after the first start the notices list holds exactly one update notice (info
  severity) with both versions, the releases address and the Preferences item name; the title ends
  with the version and the translated "outdated" word; no window from the `WINDOWS` registry
  and no `Ask` is open; hovering the word puts the new version and the address into the
  status-bar hint; after File → New the title is still marked and there is no second notice;
  after the second start the title is marked and the notices list holds no update notice
- **PATH** start Kinetren a day after a newer release came out → no window opens; the bell in
  the status bar shows 1 and the title row reads "… — kt26.10 (outdated)" with the word in the
  warning colour → hover the word → the status bar says which version is out and where →
  click the bell → the notice says "Kinetren kt26.11.01 is out (you have kt26.10.20)", the
  address and where to turn the check off
- **TEST** t875 (`src/apps/kt-app/src/reach_tests.rs`)

#### Scenario: Every language has the four texts and their glyphs
- **WHEN** the five catalogs are read and the update texts (the checkbox, its hover text, the
  notice and the title word) are laid out in the program's fonts
- **THEN** every catalog has all four keys with the notice's `{}` placeholders intact, the
  notice in every language contains the checkbox name, the title word is at most 12 characters,
  and no glyph falls back to the placeholder box (CJK and Arabic subsets included)
- **TEST** t876 (`src/apps/kt-app/src/i18n_tests.rs`, `src/apps/kt-app/src/font_tests.rs`)

### Requirement: The check is switched off in Preferences; development builds never ask
The check SHALL be on unless any of these holds: the build is a debug build and `KT_UPDATE_URL`
is not set; the environment has `KINETREN_UPDATE_CHECK=0` (or `off`/`false`); the person turned
it off. The choice SHALL be stored in eframe storage under `update_check` (`on`/`off`). The File ›
Preferences… window SHALL carry a checkbox "Check for new versions daily" right after the telemetry checkbox;
its hover text SHALL say what is asked (the latest release, from GitHub, once a day), what is
sent (nothing but the program version) and that nothing is downloaded. Turning it off SHALL
remove the title word on the next frame; turning it on SHALL make the check due on the next start (not
in the middle of the session). When the environment or the build blocks the check, the
checkbox SHALL be shown disabled with the reason in its hover text. A debug build SHALL honour
`KT_UPDATE_URL` (the live scenario's local server) and check then regardless of the day; a
release build SHALL ignore `KT_UPDATE_URL`, so a shipped binary cannot be pointed elsewhere.
Why: the owner chose a View checkbox like the telemetry one (2026-10-27); a per-machine
preference lives next to the other ones (checklist rule 13). The owner's live scenarios and
tests start the real `kinetren` dozens of times a day — none of them may reach GitHub
(the same reason telemetry keys only release builds). On 2026-11-01 (kkt-26.11.01 item 13) the
program's preferences left View for File › Preferences…, the two checkboxes staying neighbours.

#### Scenario: Consent resolution
- **WHEN** the consent is resolved over the table: debug build, no `KT_UPDATE_URL`; debug +
  `KT_UPDATE_URL`; release, nothing stored; release + `KINETREN_UPDATE_CHECK=0`; release +
  stored `off`; release + stored `on` + `KINETREN_UPDATE_CHECK=1`; release + `KT_UPDATE_URL`
- **THEN** the results are: off (blocked by the build); on, URL from the variable, the day
  ignored; on; off (blocked by the environment); off; on; on with the GitHub address (the
  variable ignored) — and a checker resolved off never spawns its thread
- **TEST** t877 (`src/apps/kt-app/src/update_check.rs`)

#### Scenario: The checkbox is next to telemetry and its choice holds
- **WHEN** the harness app with the check on and a stored "newer" answer opens File › Preferences…, unchecks
  "Check for new versions daily", draws a frame, then restarts with the stored state
- **THEN** the Preferences window holds the checkbox right after the telemetry checkbox, checked, and its hover
  text names GitHub and says nothing is downloaded; after unchecking, the title has no
  "outdated" word on the next frame and the word is no longer painted, the stored value is `off`, and after the restart the
  checker spawns no thread and the title stays unmarked
- **PATH** File › Preferences… → "Check for new versions daily" is checked under "Send anonymous usage
  statistics" → hover it → the status bar says it asks GitHub once a day and downloads nothing
  → uncheck it → the "(outdated)" word leaves the title → restart → the checkbox is
  still unchecked and the title is plain
- **TEST** t878 (`src/apps/kt-app/src/reach_tests.rs`)

### Requirement: A package installed without its Recommends names what the checks need
Every `apt-get install --no-install-recommends` in `.github/workflows/ci.yml`,
`.github/workflows/release.yml` and every Dockerfile SHALL also install, by name, each package
that a check run after it needs and that the install does not pull. The pairs live in one table
of `src/scripts/dist-check.py` (`RECOMMENDS_NEEDED`):
- `flatpak-builder` → `elfutils` (it strips with `eu-strip`);
- `flatpak-builder` → `librsvg2-common` (its `appstreamcli compose` renders the SVG icon through
  gdk-pixbuf, whose SVG loader is a Recommends of `librsvg2-2`);
- `flatpak-builder` → `webp-pixbuf-loader` (compose reads the WebP screenshots through gdk-pixbuf
  too, and its WebP loader is a package nothing in the install pulls);
- `xdg-utils` → `libfile-mimeinfo-perl` (xdg-mime without a desktop asks `mimetype`);
- `shared-mime-info` → `libglib2.0-bin` (`gio`, the oracle t935 asks).
A new pair is a new row.
Why: the CI run of 94e36e9e lost two jobs to dropped Recommends. The Flatpak build had no
`eu-strip`, and t935 got `application/json` for our project, because xdg-mime fell back to
libmagic's `file`, which never reads the MIME database. The run of 9e1f1a45 lost two more: compose
could not read the icon without the SVG loader, and perl's `mimetype` called a Kotlin source ours.
The run of 21505ad2 lost the Flatpak job once more: without the WebP loader compose dropped every
screenshot, and Flathub's linter found none in the catalogue.
Each was silent on the owner's desktop, which has the full packages and asks GIO.

#### Scenario: Each dropped Recommends a check needs is named
- **WHEN** `dist-check.py --check` runs on the repository; then on temporary copies where, one at
  a time, `ci.yml` or `release.yml` installs `flatpak-builder` without `elfutils`, or `ci.yml` or
  the Dockerfile installs `xdg-utils` without `libfile-mimeinfo-perl`
- **THEN** the repository passes; each copy fails, naming the file, the package and the one it
  must install
- **TEST** t1060 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The SVG loader and the MIME oracle are named
- **WHEN** `dist-check.py --check` runs on temporary copies where, one at a time, `ci.yml` or
  `release.yml` installs `flatpak-builder` without `librsvg2-common`, or `ci.yml` or the
  Dockerfile installs `shared-mime-info` without `libglib2.0-bin`
- **THEN** each copy fails, naming the file, the package and the one it must install
- **TEST** t1380 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The WebP loader is named
- **WHEN** `dist-check.py --check` runs on temporary copies where, one at a time, `ci.yml` or
  `release.yml` installs `flatpak-builder` without `webp-pixbuf-loader`
- **THEN** each copy fails, naming the file, the package and the one it must install
- **TEST** t1410 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The MIME database is asked as a file manager asks it
- **WHEN** t935 builds a MIME database from our MIME file and the system's `freedesktop.org.xml`
  and asks GIO (`gio info -a standard::content-type`) about a project written by
  `project_file::to_text`, a Kotlin source `Main.kt` and the legacy fixture
- **THEN** GIO names `application/x-kinetren-project`, `text/x-kotlin` and
  `application/x-kinotrain-legacy-project`. Where perl's `mimetype` exists, it names our project
  and the legacy fixture too. It is not asked about `Main.kt`: it takes the first of two
  equal-weight globs without reading the file, and that order differs between shared-mime-info
  versions
- **TEST** t935 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The MIME check runs only where it asks the database
- **WHEN** t935 runs on a machine with `update-mime-database` but no `gio`
- **THEN** it says it did not run and names `libglib2.0-bin`, instead of failing on another
  program's answer; on CI and in the build container it runs (t1380 holds the package)
- **TEST** t935 (`src/apps/kt-cli/tests/dist_guard.rs`)

### Requirement: A registry volume is handed to the container's user
A script that mounts the crates.io registry into a build container
(`release-build.sh`, `windows-test.sh`, `ffmpeg-test.sh`) SHALL take it from
`src/scripts/cargo-registry.sh`: the host's `~/.cargo/registry` when it exists, otherwise the
named volume `kinetren-cargo-registry`, chowned to the calling user by a root container on every
run. Why: the containers run as the caller (`-u UID:GID`), and a fresh named volume belongs to
root. The ffmpeg9 CI job of 94e36e9e died in `cargo metadata` ("failed to create directory
`/usr/local/cargo/registry/cache`: Permission denied") — the runner has no `~/.cargo/registry`,
and only `release-build.sh` had the chown; the two test scripts carried their own copy without it.

#### Scenario: Every registry mount goes through the one script
- **WHEN** `dist-check.py --check` runs on the repository; then on temporary copies where
  `ffmpeg-test.sh` names the volume itself, `cargo-registry.sh` lost its chown, or
  `cargo-registry.sh` is missing
- **THEN** the repository passes; each copy fails, naming the script
- **TEST** t1064 (`src/apps/kt-cli/tests/dist_guard.rs`); the run itself is the `ffmpeg9` CI job
  (reproduced locally: a fresh volume refuses `mkdir` to the user, the handed one accepts it)

### Requirement: The embedded version follows the checked-out commit, in a worktree too
The version baked into `kinetren` and `kt-cli` (`kt_version::FULL`) SHALL name the commit checked out
where they are built, after any commit, checkout or merge, in a plain checkout and in a git worktree
alike (where `.git` is a file). The build script SHALL watch the files git names for that checkout —
its HEAD, the loose file of the branch HEAD points to (or, for a branch that is only packed, the
directory the next commit writes it into) and `packed-refs` — and SHALL watch only paths that exist.
A commit on another branch of the same repository SHALL NOT change any watched file. Without git the
build watches nothing and the version comes from the environment or is `kt00.00.00-nogit`.
Why: the former rule watched `../../../.git/HEAD`; in a worktree it does not exist, the version was
never rebuilt, and the crash window said `kt26.11.01-a498ecfe` while `finish.sh` printed `b6c8f121` —
the "stale program" class `finish.sh` exists against. A missing watched path reruns the script on
every build (a relink of everything), and the whole shared `refs/heads` would relink a worktree on
every parallel session's commit.

#### Scenario: A worktree commit rebuilds the version
- **WHEN** a temporary repository has a worktree on its own branch; a commit is made on the main
  branch, then in the worktree, then the refs are packed and another commit is made in the worktree;
  and a directory outside any repository is asked
- **THEN** the plain checkout watches its `.git/HEAD` and its branch file; the worktree watches its
  HEAD under `.git/worktrees/<name>` and its branch file; the main-branch commit changes none of the
  worktree's watched files; each worktree commit changes one (the packed case through the watched
  directory); outside a repository nothing is watched
- **TEST** t1170 (`src/libs/kt-version/tests/git_watch.rs`)

### Requirement: The AppStream metadata is composed before the Flatpak build
`src/scripts/flatpak-build.sh` SHALL run `src/scripts/appstream-compose-check.sh` before
`flatpak-builder`. The script installs the files of the manifest's own `install … /app/share/…`
lines into a temporary folder, the same way the build does. It then runs `appstreamcli compose` on
that folder with the arguments that flatpak-builder 1.4.6+ passes under `flatpak-build.sh`'s
screenshot mirror (`--no-partial-urls`, `--prefix=/`, the origin, `--media-baseurl` =
`https://dl.flathub.org/media`, the media, result, data and icons folders,
`--components=<ID>,<ID>.desktop`). It fails with compose's own hints when compose reports an
error. It also fails, with the hints and the WebP loader's package name, unless the catalogue holds
as many screenshots as the metainfo and every image URL starts with the media URL, names a `.png`,
and exists in the media folder. Where `appstreamcli compose` is not available (no
`appstream-compose`), it says so and exits 0: flatpak-builder needs it anyway and fails there.
Why: on 9e1f1a45 the Flatpak job built for ten minutes, then compose could not read the icon. The
failure belongs to the listing and the host, not to the build, and shows in seconds without it.
On 21505ad2 compose said `Success!` and dropped all four WebP screenshots (`screenshot-save-error`,
no WebP loader); only the linter noticed, after the build.

#### Scenario: The listing composes, a broken icon is caught before the build
- **WHEN** `appstream-compose-check.sh` runs on the repository; then on a temporary copy whose SVG
  icon is not an image
- **THEN** the repository composes (exit 0, a catalog file with the component); the copy fails and
  names `file-read-error` and the icon's path. Without `appstreamcli compose`, or when the
  listing's first screenshot is unreachable (compose fetches the screenshots), the test says why it
  did not run
- **TEST** t1381 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: Every screenshot reaches the catalogue, an unreadable one fails before the build
- **WHEN** `appstream-compose-check.sh` runs on the repository; then on a temporary copy whose
  last screenshot URL points at a text file of the same release tag (what compose saw of every
  WebP file without the WebP loader)
- **THEN** the repository passes and says `4 screenshots mirrored`; the copy fails, saying
  `3 of 4 screenshots reached the catalogue` and naming `screenshot-save-error`. On a host
  without `webp-pixbuf-loader` (the packages of 21505ad2) the repository itself fails, in about a
  second, with `0 of 4`
- **TEST** t1411 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The pre-flight stays in the build script
- **WHEN** `dist-check.py --check` runs on a temporary copy whose `flatpak-build.sh` does not run
  `src/scripts/appstream-compose-check.sh`
- **THEN** it fails, naming the missing pre-flight
- **TEST** t1380 (`src/apps/kt-cli/tests/dist_guard.rs`)

### Requirement: The Flatpak's screenshots are mirrored as Flathub's linter wants
`src/scripts/flatpak-build.sh` SHALL build with `--mirror-screenshots-url=https://dl.flathub.org/media
--compose-url-policy=full` (the media URL named once, `MEDIA_URL`), so that the catalogue names
every screenshot by its full Flathub media URL and the repository carries the `screenshots/<arch>`
ref with the PNGs. Before anything else it SHALL refuse a flatpak-builder older than 1.4.6,
naming `ppa:flatpak/stable`. `ci.yml` and `release.yml` SHALL add that PPA before the install that
names `flatpak-builder`. `dist-check.py` holds the two arguments, the pre-flight's matching
`--media-baseurl` and `--no-partial-urls`, and the PPA.
Why: the run of 21505ad2 built and installed the Flatpak, and `flatpak-builder-lint repo` refused
it: `appstream-missing-screenshots` (D1 of change ci-flatpak-screenshots) and
`appstream-screenshots-not-mirrored-in-ostree`. The linter also refuses partial screenshot URLs
(`appstream-external-screenshot-url`). Flathub's linter page says these exceptions are "never
granted" and names this flow for builds made outside Flathub. `--compose-url-policy` came in
flatpak-builder 1.4.6, and the screenshots ref is committed by flatpak-builder itself since 1.4.5;
Ubuntu 24.04 ships 1.4.2.

#### Scenario: The mirror, the URL policy and the PPA are guarded
- **WHEN** `dist-check.py --check` runs on temporary copies where, one at a time, `flatpak-build.sh`
  loses `--mirror-screenshots-url` or `--compose-url-policy=full`, `appstream-compose-check.sh`
  loses `--media-baseurl` or `--no-partial-urls`, or `ci.yml` or `release.yml` loses
  `add-apt-repository -y ppa:flatpak/stable`
- **THEN** each copy fails, naming the file and the missing piece
- **TEST** t1410 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The whole Flatpak job passes the linter
- **WHEN** `src/scripts/flatpak-build.sh` runs in `ubuntu:24.04` with the packages of the CI job
  (`ppa:flatpak/stable`; flatpak, flatpak-builder, appstream, elfutils, librsvg2-common,
  webp-pixbuf-loader)
- **THEN** the pre-flight, the build, the install, both `flatpak-builder-lint` runs (no errors),
  `appstreamcli validate --strict` and `flatpak-check.sh` pass
- **TEST** manual check recorded in `tasks.md` (Docker isn't available to `cargo test`); the CI job `flatpak` runs it

### Requirement: Desktop integration speaks every interface language and claims only our files
The desktop entry (`src/packaging/com.kinetren.Kinetren.desktop`) and the MIME description
(`src/packaging/com.kinetren.Kinetren.xml`) SHALL carry a translation for EVERY language of the
interface catalogs (`src/apps/kt-app/i18n/*.toml`), mapped to the freedesktop locale names
(`ru` → `ru`, `es` → `es`, `ar` → `ar`, `zh-Hans` → `zh_CN`; English is the untranslated value):
`GenericName`, `Comment` and `Keywords` of the desktop entry, and the `<comment>` of every MIME
type. The translated texts SHALL come from one place — a section of the catalogs — and the
packaging files SHALL be equal to it; a new catalog language without its packaging lines is red.
The desktop entry SHALL have `Keywords=` (English and translated) naming what a person types in
an application menu to find a video editor.
The MIME description SHALL claim only files that are ours: the project type
`application/x-kinetren-project` SHALL be recognised by the glob `*.kt` TOGETHER with a content
match on the project file's root marker (`"kinetren"` near the start of the file), because
`*.kt` is also the Kotlin source glob of shared-mime-info (`text/x-kotlin`) and a glob alone would
make every Kotlin source a "Kinetren project". The legacy `.ktproj` SHALL NOT be described as a
Kinetren project: it gets its own type ("Older Kinotrain project") so a file manager names it
truthfully, and opening it with Kinetren reaches the "Open" refusal that names the import door
(ui-reach, "Import of an old-version project").
Why: kkt item 3 (2026-10-27): "`kinetren-mime.xml` is not fully translated" (the file is now `com.kinetren.Kinetren.xml`) — the desktop and
MIME files carried only `ru` next to English while the program speaks five languages; the MIME
file still globbed `*.ktproj` as a Kinetren project although R.70 made `.kt` the project file and
`.ktproj` an import-only legacy format (project-format design: "two formats under one suffix is a
trap").

#### Scenario: Every catalog language is in the desktop and MIME files
- **WHEN** the catalog languages are listed from the build tables and both packaging files are parsed; each translated value is compared with the catalogs' packaging section; separately a copy of the tree gets a sixth catalog language, and a copy has `Comment[es]` removed
- **THEN** for every language and every key (`GenericName`, `Comment`, `Keywords`, each MIME `<comment>`) the packaging value equals the catalog value (the keyword list joined with `;` and ending with `;`); `Keywords=` exists untranslated; both broken copies are red naming the language and the key
- **TEST** t930 (`src/apps/kt-app/src/i18n_tests.rs`)

#### Scenario: The MIME file claims only our files
- **WHEN** `com.kinetren.Kinetren.xml` is parsed by the distribution guard
- **THEN** `application/x-kinetren-project` has the glob `*.kt` and a magic rule matching the root marker `"kinetren"` within the first bytes; no type of ours globs `*.ktproj` except the legacy type, whose comment names the old program; `application/x-opentimelineio` globs `*.otio`; the desktop `MimeType=` lists exactly the three types; a copy with `*.ktproj` back on the project type is red
- **TEST** t931 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The desktop database resolves the files the way the person expects
- **WHEN** in a temporary `XDG_DATA_HOME` the MIME file is installed and `update-mime-database` is run, and the type of three files is queried: a project written by `kt_model::project_file::to_text` named `film.kt`, a Kotlin source `Main.kt`, and the legacy `fixtures/demo.ktproj`
- **THEN** `film.kt` is `application/x-kinetren-project`, `Main.kt` is `text/x-kotlin`, `demo.ktproj` is the legacy type; without `update-mime-database` on the machine the test prints why it did not run, and the build container installs it so CI always runs it
- **TEST** t935 (`src/apps/kt-cli/tests/dist_guard.rs`)

### Requirement: The long tests start first
`src/.config/nextest.toml` SHALL hold a profile used by CI and by `finish.sh` that starts the
tests measured as the longest before the others, so the run does not end on one long test while
the other cores idle. Every test it names SHALL exist in the workspace. A long test whose cases
are independent MAY run them on threads inside the test; it keeps its name, its cases and its
assertions.
Why: in run 9e1f1a45 the last tests to finish were the longest (t1053 74 s, t943 48 s on Linux;
t1053 67 s, t943 52 s, t1171 42 s on Windows), started late by the default order.

#### Scenario: The ordered tests exist
- **WHEN** `dist-check.py --check` runs on the repository; then on a temporary copy where the
  nextest profile names a test that no `#[test]` in the workspace defines
- **THEN** the repository passes; the copy fails, naming the missing test
- **TEST** t1422 (`src/apps/kt-cli/tests/dist_guard.rs`)

### Requirement: A full run under load tells a defect from a busy machine
A test SHALL wait for what must happen with `kt_time::test_deadline()`, a ten-minute guard against a
hang and no claim about speed. A wall-clock limit of its own is allowed only where the limit IS the
check, a budget the program promises to a person, and the guard lists each such limit with its reason.
A test that must keep up with the wall clock SHALL be judged against what the machine gave at that
moment. A live scenario SHALL wait for the program: its fixed steps stretch with the observed frame
time, and a window that closes is waited out with `wait_gone`. Nothing of this retries a test or
weakens its observable consequence. The rerun of a red live scenario alone stays, and its report still
names what passed on the second try.
Why: every full run under load gave random reds, the orchestrator rechecked each merge by hand, and a
real defect was easy to take for noise (`docs/found-defects.md`, rows 24–30, 47, 57–59, 79, 82). One
fsync took 44 s on the working disk while a build ran: "the download did not end" in 20 s, "probe not
published" in 45 s and t815's 2 s all fired on correct code.

#### Scenario: Runs of the fixture generator meet
- **WHEN** six runs of `fixtures/gen.sh` start at once on an empty `out/`, with a stand-in `ffmpeg` that
  writes its output in two steps
- **THEN** every run succeeds and no file in `out/` is half-written
- **TEST** t1670 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: A test does not bound its wait by the clock
- **WHEN** the sources of `src/apps`, `src/libs` and `src/plugins` are read
- **THEN** no line sets `Instant::now() + Duration::from_…` or compares `.elapsed() < Duration::from_…`
  outside the guard's list of limits that are the check, and every entry of that list still exists
- **TEST** t1671 (`src/apps/kt-cli/tests/dist_guard.rs`)

### Requirement: A test's clock is on the list or not in the test
Test code (a file of tests, or what follows a file's first `#[cfg(test)]`) SHALL NOT bound `.elapsed()`
by anything but `kt_time::TEST_WAIT`/`test_deadline()`, nor read the thread's processor time, outside
t1671's list of limits that are the check; each entry says why. A processor-time comparison SHALL
interleave the rounds of what it compares and take each one's cheapest.
Why: t1914 measured two scenes one after the other and read 8.5 ms against 4.2 ms at load ~60 on correct
code (found-defects 126); the scan of t1671 saw only `Duration::from_` bounds, and constants and
processor clocks passed it.

#### Scenario: The wider scan
- **WHEN** the sources of `src/apps`, `src/libs` and `src/plugins` are read
- **THEN** besides t1671's former patterns, no test code bounds `.elapsed()` by a constant or reads
  `thread_ns()` outside the list, and every entry of the list still exists
- **TEST** t1671 (`src/apps/kt-cli/tests/dist_guard.rs`); t1913, t1914 (`src/apps/kt-app/src/frame_cost_tests.rs`)

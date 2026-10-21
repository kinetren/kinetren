# Spec Delta

## Purpose

How Kinetren gets from the repository to people: one build container shared by everyone,
the `make` commands a newcomer runs, the checks GitHub runs on every push, and the releases
it publishes on a tag. Only releases are kept for good.

## ADDED Requirements

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
  triggers and permissions required below, and the README quick start names `make` and
  `make install`
- **TEST** t834 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The guard catches each class of drift
- **WHEN** it runs on a temporary copy where, one at a time, the Dockerfile's toolchain
  differs from CI's, `release-build.sh` has a heredoc `FROM`, a Makefile target is missing,
  the CI workflow uploads an artifact without `retention-days`, the release workflow
  triggers on something other than `v*` tags, or the README quick start lost `make install`
- **THEN** exit non-zero, and each violation is reported with its file
- **TEST** t835 (`src/apps/kt-cli/tests/dist_guard.rs`)

#### Scenario: The delivered binaries pass the delivery checks
- **WHEN** `release-build.sh` (or `make`) finishes
- **THEN** both binaries have no `libav*` in `ldd` and need at most `GLIBC_2.35`; otherwise
  the build fails and no tarball is left in `dist-out`
- **TEST** guard inside `src/scripts/release-build.sh` (invariant 10; media-index scenario
  "Релизный бинарь не линкован с libav*")

### Requirement: The program is called `kinetren` on disk

The GUI binary SHALL be named `kinetren` everywhere a person meets it: the build output
(`src/target/<profile>/kinetren`), the release tarball, `~/.local/bin/kinetren` after
installation, the `.desktop` file (`Exec=kinetren %f`, `StartupWMClass=kinetren`, matching
the window's application id `kinetren`) and `make run`. The cargo package MAY stay `kt-app`
(`cargo run -p kt-app` keeps working); `kt-cli` keeps its name. `install.sh` SHALL remove a
`~/.local/bin/kt-app` left by an older install, the same way it already removes `kinotrain`
leftovers, so the menu doesn't start an old build. No tarball, desktop entry or script
that people run may start a binary named `kt-app`.
Why: the owner's request ("при билде и распространении надо называть не kt-app … а
kinetren"); `kt-app` is an internal crate name that leaked into what people install.

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
It builds the workspace, runs `cargo clippy --all-targets`, runs the workspace tests
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

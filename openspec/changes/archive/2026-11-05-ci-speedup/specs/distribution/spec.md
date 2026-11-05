## MODIFIED Requirements

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

## ADDED Requirements

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

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
branches compete for the same 10 GB and evict the default branch's.

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

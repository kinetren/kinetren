# Spec Delta

## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: FFmpeg 9 on Linux is proven against a pinned source build
The build container SHALL have an `ffmpeg-test` stage. It builds the SHARED libraries of FFmpeg
9.0.2 from the official source tarball `ffmpeg-9.0.2.tar.xz`, whose SHA-256 is committed next
to the recipe. The configuration is GPL with libx264, so the export cascade has its usual
software encoder, and no programs or docs are built. The stage also carries nextest.
`make ffmpeg-test` (`src/scripts/ffmpeg-test.sh`) SHALL run the tests that load FFmpeg (the
packages `kt-ffmpeg`, `kt-media`, `kt-engine`, `kt-cli`) in that stage, with
`KINETREN_FFMPEG_DIR` pointing at the built libraries, the system search switched off, and
`KT_EXPECT_FFMPEG_MAJOR=9`. The fixtures are generated on the host, as for the Windows tests.
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
- **WHEN** the tests run with `KT_EXPECT_FFMPEG_MAJOR=9` and `KINETREN_FFMPEG_DIR` at the built 9.0.2 libraries; then with the variable set to 9 and the directory pointing at an empty folder; then with the variable unset
- **THEN** the first run loads FFmpeg 9 (avcodec 63.x) from that folder and the FFmpeg tests pass on it (t10, t21, t37, t112, t33, t849); the second fails, naming the expected major and the search report; the third skips the check with a printed reason
- **TEST** t865 (`src/libs/kt-ffmpeg/tests/loader_plan.rs`); the run itself is `make ffmpeg-test` and the CI job

#### Scenario: The FFmpeg 9 parts of the tree are consistent
- **WHEN** `src/scripts/dist-check.py --check` runs on the repository; then on temporary copies where, one at a time, the Dockerfile lacks the `ffmpeg-test` stage or its SHA-256 check of the tarball, the vendoring script pins another version or hash than the Dockerfile, the Makefile lacks `ffmpeg-test`, the CI workflow lacks the FFmpeg 9 job or the job does not set `KT_EXPECT_FFMPEG_MAJOR`, or `release-build.sh` copies anything from the `ffmpeg-test` stage
- **THEN** the repository passes; each copy fails and the violation is reported with its file
- **TEST** t866 (`src/apps/kt-cli/tests/dist_guard.rs`)

# Tasks

## 0. Binary name `kinetren` (D10)

- [x] 0.1 Add `[[bin]] name = "kinetren"` to `src/apps/kt-app/Cargo.toml`. Verify that `cargo build -p kt-app` produces `target/debug/kinetren`, that `cargo run -p kt-app` starts it, and that `cargo test --workspace` stays green
- [x] 0.2 Switch every script that starts the GUI from `target/*/kt-app` to `target/*/kinetren` (`finish.sh`, `live-check.sh`, `live/kt_live.py`, `live/worker.py`, `live/gpufx.py`, `ui-probe.py`, `subset-fonts.py`, `specs-user-path.py`, `readme-shots.py` if it names the path) and the "run this" lines in root `CLAUDE.md`, `src/apps/kt-app/CLAUDE.md` and `CONTRIBUTING.md`. Verify that `grep -rn "debug/kt-app\|release/kt-app" src/scripts CLAUDE.md CONTRIBUTING.md src/apps/kt-app/CLAUDE.md` is empty and `./scripts/live-check.sh` is green
- [x] 0.3 Change `src/packaging/kinetren.desktop` to `Exec=kinetren %f` and `StartupWMClass=kinetren`, and `install.sh` to install `kinetren` + `kt-cli`, remove `~/.local/bin/kt-app`, and use the `Exec=` fallback with the new name. Verify by installing over a fake older install (`touch ~/.local/bin/kt-app` in a temp `HOME`): `kinetren` is present and `kt-app` is gone. Then run `xprop WM_CLASS` on the window under Xvfb, record the output here, and confirm that it names `kinetren`
  - Result: `xprop -id <window> WM_CLASS` under Xvfb prints `WM_CLASS(STRING) = "", "kinetren"`. Installing over a temp `HOME` that held `~/.local/bin/kt-app` leaves `kinetren` and `kt-cli` and removes `kt-app`; the desktop file reads `Exec=…/kinetren %f` and `StartupWMClass=kinetren`.
- [x] 0.4 Commit stage 0 (`github-ci-docker-build stage 0: the GUI binary is called kinetren`) with `cargo test` and live scenarios green
  - Result: live-check is green (47 scenarios; `fade` passed on the second, solo run, the known Р.69 flake class). nextest: the first cold run under live load lost t476 (45 s probe wait) and t640 (SIGBUS); both pass alone in under 0.2 s, and the full rerun gives 1100 passed, 12 skipped.

## 1. Build container as a file (D1–D3)

- [x] 1.1 Move the heredoc image from `src/scripts/release-build.sh` to `src/packaging/Dockerfile` with `ARG RUST_TOOLCHAIN=1.97.1`. Verify that `docker build -f src/packaging/Dockerfile src/packaging` succeeds and that `rustc --version` inside it prints 1.97.1
  - Result: the image builds from `src/packaging/Dockerfile`; `docker run --rm kinetren-build:ubuntu2204 rustc --version` prints `rustc 1.97.1 (8bab26f4f 2026-07-14)`.
- [x] 1.2 Move the `ldd`/`GLIBC_` checks into the container's `docker run` and delete `dist-out` when a check fails. Verify by temporarily pointing the check at a binary that links `libavcodec` (any host binary from `ldd /usr/bin/ffmpeg`): the build fails, the message names invariant 10, and `dist-out` has no tarball. Then revert the probe
  - Result: the `delivery_checks` function of `release-build.sh` run against `/usr/bin/ffmpeg` prints "ОШИБКА: ffmpeg слинкован с libav* — нарушение инварианта №10" and returns 1. With a `docker` shim that fails the container build, `release-build.sh` exits 1 with "build or delivery check failed, no tarball", and the planted old tarball in `dist-out` is gone. The script itself was not edited for the probe.
- [x] 1.3 Use a named volume `kinetren-cargo-registry` when `~/.cargo/registry` is absent, and write `<tarball>.sha256`. Verify with `HOME=$(mktemp -d) src/scripts/release-build.sh`: the tarball and `.sha256` appear, `sha256sum -c` passes, and a second run finishes without `Compiling kt-`
  - Result: `HOME=<empty> make` (with `DOCKER_CONFIG` pointing at the real `~/.docker`: the machine's proxy lives there; without it crates.io stalls) used the volume `kinetren-cargo-registry`, downloaded the crates and built the tarball; `sha256sum -c` OK; the second `make` printed `Finished … in 0.17s` and no `Compiling`. Delivered: `kinetren` needs GLIBC_2.35 (40M), `kt-cli` GLIBC_2.34 (18M), tarball 23 MB.
- [x] 1.4 Commit stage 1 (`github-ci-docker-build stage 1: build container as a committed Dockerfile, checks inside it`) with `cargo test` green

## 2. Makefile (D4)

- [x] 2.1 Add the root `Makefile` with `all build install run test clean help`. Check that `make help` lists them; `make` builds the tarball; `make run` starts `kinetren` from `dist-out`, whose debug-less title shows the short version; `make install` installs it into `~/.local/bin`; `make clean` removes `dist-out` and `dist-container` but keeps the volume
  - Result: `make help` lists all seven targets; `HOME=<tmp> make install` installs `kinetren` + `kt-cli` and removes a planted `kt-app`; `make run` under Xvfb opens a window with WM_NAME "Kinetren" and WM_CLASS "kinetren"; `make clean` removes `dist-out` and `dist-container`, and `docker volume ls` still lists `kinetren-cargo-registry`.
- [x] 2.2 Get friendly failures: `PATH` without docker → `make` prints the single install hint and exits non-zero, and `make run` before any build prints "run `make` first". Verify both by hand and record the output here
  - Result: `env -i PATH=/nonexistent make build` → "make: Docker is required to build Kinetren: https://docs.docker.com/engine/install/", exit 2; `make run` before a build → "make: nothing built yet — run `make` first", exit 2.
- [x] 2.3 Commit stage 2

## 3. Guard dist-check.py (D8)

- [x] 3.1 Write `src/scripts/dist-check.py --check [--root DIR]` covering the spec's shape rules (binary name `kinetren` in the manifest, `.desktop`, `install.sh`, `release-build.sh` and the Makefile; one Dockerfile, toolchain equality with `ci.yml`, no inline `FROM` in `release-build.sh`, Makefile targets, CI triggers/permissions/concurrency/retention/RUSTFLAGS, release `v*` trigger and `contents: write` only, README quick start names `make` and `make install`), reporting each violation with its file
- [x] 3.2 Add `src/apps/kt-cli/tests/dist_guard.rs`: t834, where the repository passes, and t835, where a temp copy with each defect from the spec scenario fails with that file named. Both tests are red until sections 4–5 land, so they land together with section 5
  - Result: t834 and t835 pass (`cargo test -p kt-cli --test dist_guard`); t835 covers nine kinds of drift: the toolchain, an inline recipe, a Makefile target, retention, the release trigger, the README, `Exec=kt-app`, `kt-app` in the tarball, a second Dockerfile.
- [x] 3.3 Add the guard line to `src/scripts/finish.sh` next to `readme-check.py`

## 4. Workflows (D5–D7)

- [x] 4.1 Before writing CI, check whether `cargo clippy --all-targets -- -D warnings` is clean locally. If it isn't, fix the warnings (each fix is a code change that keeps `cargo test` green) or report the count to the owner and use plain `clippy` without `-D`. Record which one was chosen here
  - Decision: not clean. About 25 warnings across kt-model, kt-app and kt-cli (`is_multiple_of`, a complex type, dead code, …). CI runs `cargo clippy --workspace --all-targets` without `-D warnings`. Fixing them and making them fatal is a follow-up; the count is reported to the owner.
- [x] 4.2 Measure the CI build's disk usage locally: run `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 RUSTFLAGS= CARGO_TARGET_DIR=$(mktemp -d) cargo nextest run --workspace --no-fail-fast` and record its green result and `du -sh` of that target here. The target must stay under 12 GB, otherwise apply the D-risk fallback
  - Result: with the CI env and a separate target dir, nextest gives 1102 passed, 12 skipped, and `du -sh` of the target is 1.8 GB (the runner has about 14 GB). The first run under a concurrent Docker LTO build lost t815 (a 10 s write-time assertion) and t802 (display collision); both are green in the rerun without load. Added after the measurement: `xvfb openbox python3-xlib` and an explicit `cargo build -p kt-app -p kt-cli` in CI, so the live-framework guards t801/t802 run every time instead of depending on the runner image.
- [x] 4.3 Write `.github/workflows/ci.yml` per D5/D7 (push + pull_request, `contents: read`, concurrency cancel-in-progress, free-disk, apt deps incl. `ffmpeg`, pinned toolchain, rust-cache with `save-if` on the default branch, clippy, nextest, guards, failure-log upload with `retention-days: 3`). Verify with `actionlint` if installed and with `dist-check.py --check`
  - Result: `dist-check.py --check` is green. `actionlint` is not installed on this machine and wasn't downloaded.
- [x] 4.4 Write `.github/workflows/release.yml` per D6 (`tags: ['v*']`, `contents: write`, default-branch ancestry check, `release-build.sh`, `gh release create --verify-tag --generate-notes`, `--prerelease` for `-` tags, tarball + `.sha256`). Verify with `actionlint` if installed and with `dist-check.py --check`
  - Result: `dist-check.py --check` is green. `actionlint` is not installed.

## 5. README and CONTRIBUTING (D9)

- [x] 5.1 Wait until `readme-showcase` is committed (it edits README). Then rewrite only the README "Quick start": releases first, then `make` + `make install`, plus the runtime requirements. Verify that `readme-check.py --check` passes and that t834 is green
  - Result: `readme-showcase` was archived and committed in the meantime, and its README has an "Install" section, not "Quick start"; that section now reads: release → `./install.sh`, or clone + `make` + `make install`, with a link to CONTRIBUTING for the cargo route. readme-check and dist-check are green.
- [x] 5.2 Move the apt + `cargo run` route into CONTRIBUTING.md ("Building from source without Docker") and add a "CI" paragraph (what runs on push, what only `finish.sh` runs, how a release is cut). Verify that `readme-check.py --check` passes
- [x] 5.3 Commit stages 3–5 together (guard + workflows + docs) with t834/t835 green

## 6. Finish and hand-off

- [x] 6.1 Run `src/scripts/finish.sh --release` green; the report states the version and the tarball path, and `make` produces the same tarball name
  - Result: `finish.sh --release` green at kt26.10.22-897b4578 (tree = stages 0-5 plus another session's doc-only commits): release tarball `kinetren-26.10.22-897b4578-linux-x86_64.tar.gz` built in 191 s, workspace tests ok, SIMD ok, live scenarios ok (`fade` and `formula` red in the parallel run, green alone: the known Р.68/Р.69 rerun mechanism), readme-check and dist-check green. 821 s against the 300 s budget; the live scenarios took 603 s.
- [ ] 6.2 Hand-off note to the owner (in the report, not automated): push `master` → the first CI run, then `git tag vYY.MM.DD && git push kinetren vYY.MM.DD` → the first release. The final repository is `github.com/kinetren/kinetren` (owner, 2026-10-22); the local remote still points to `kvazi-team/kinetren` — the owner repoints it before pushing. After the first tag, record the CI run's duration, the release URL and the `sha256sum -c` result here
- [ ] 6.3 After the owner's first green CI and release: move the change to the archive (`/opsx:archive`), so that `openspec/specs/distribution/` is created; then add a design.md for `distribution` with D1–D9's reasons and rejected options

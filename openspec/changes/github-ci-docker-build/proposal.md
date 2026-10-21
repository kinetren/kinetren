# Proposal

## Why

The repository is about to go public at github.com/kinetren/kinetren, and nothing checks it
there: tests run only on the owner's machine through `finish.sh`, and a release tarball exists
only if someone runs `release-build.sh` by hand. The README's quick start asks people to
install a Rust toolchain, clang and dev packages before they can try the editor, although a
container that builds the portable tarball already exists inside `release-build.sh`. The owner
asked for automatic builds on GitHub's free plan (keeping only releases, not piles of build
artifacts) and for a Docker build with simple `make` commands.

## What Changes

- **CI on every push and pull request** (`.github/workflows/ci.yml`): build, `clippy`,
  workspace tests through nextest on an Ubuntu runner with system FFmpeg, and the repository
  guards. There's no GPU or Xvfb in CI: GPU conformance skips itself, and the live scenarios
  stay in `finish.sh`. Nothing is uploaded except, on failure, a log that expires in 3 days.
- **Release on a `v*` tag** (`.github/workflows/release.yml`): runs the same
  `release-build.sh` container build (glibc 2.35, `ldd` without libav*), then creates a
  GitHub Release with `kinetren-<version>-linux-x86_64.tar.gz` and its `.sha256`. Release
  assets don't count toward any storage quota, so the only files kept for good are the
  releases themselves.
- **The build container becomes a file**: the Dockerfile moves out of the heredoc in
  `release-build.sh` into `src/packaging/Dockerfile`, with one pinned Rust toolchain version
  that CI uses too. The delivery checks (`ldd`, the GLIBC ceiling) run inside the container,
  so the host needs only Docker and make. A cargo registry volume and the target directory
  persist between builds.
- **A root `Makefile`**: `make` (tarball in Docker), `make install` (the tarball's
  `install.sh` into `~/.local`), `make run` (start the built `kinetren`), `make test` (host
  tests, for developers), `make clean`, and `make help`.
- **The GUI binary is called `kinetren`** (owner, 2026-10-22): a `[[bin]] name = "kinetren"`
  in `kt-app`'s manifest, so the build output (`target/*/kinetren`), the tarball,
  `~/.local/bin/kinetren`, the `.desktop` file (`Exec=` and `StartupWMClass=`) and `make run`
  all use one name. The cargo package stays `kt-app` (`cargo run -p kt-app` still works),
  and `kt-cli` keeps its name. `install.sh` removes the `kt-app` an older install left.
  Dev scripts that start `target/debug/kt-app` (finish.sh, live scenarios, ui-probe) follow.
- **README quick start rewritten** around "download a release" and "`make` + `make install`".
  The cargo route moves to CONTRIBUTING.md.
- A guard (`src/scripts/dist-check.py --check`, run from `cargo test`) keeps the workflows,
  the Dockerfile, the Makefile and the README consistent with each other.

## Capabilities

### New Capabilities
- `distribution`: how Kinetren is built for people and checked in public: the build
  container, the make commands, CI on push and pull request, release on tag, and what is
  (and is not) stored on GitHub.

### Modified Capabilities
<!-- none: the media-index scenario "Релизный бинарь не линкован с libav*" keeps its guard in
     src/scripts/release-build.sh; this change only makes CI run it on every release. -->

## Impact

- Renamed binary: `src/apps/kt-app/Cargo.toml` (`[[bin]]`), `src/packaging/{install.sh,
  kinetren.desktop}`, scripts that start `target/debug/kt-app` (`finish.sh`, `live-check.sh`,
  `live/*.py`, `ui-probe.py`, `subset-fonts.py`, `specs-user-path.py`), root and crate
  `CLAUDE.md`, `CONTRIBUTING.md`. Internal file names (`kt-app.log` in the data dir) stay.
- New files: `.github/workflows/ci.yml`, `.github/workflows/release.yml`,
  `src/packaging/Dockerfile`, `Makefile`, `src/scripts/dist-check.py`,
  `src/apps/kt-cli/tests/dist_guard.rs`.
- Changed: `src/scripts/release-build.sh` (reads the Dockerfile, checks inside the container,
  works without `~/.cargo` on the host), `README.md` (quick start), `CONTRIBUTING.md` (building
  from source, CI), `src/scripts/readme-check.py` only if the new README sections need it.
- No change to the engine, crates or invariants. Invariant 10 (`ldd` without libav*) now runs
  on every release in CI. `.cargo/config.toml` (mold) stays: CI sets `RUSTFLAGS` the same way
  the container already does.
- No editor feature is involved, so there's no `docs/features/` note.
- Touches the README alongside the in-flight `readme-showcase` change: only the "Quick start"
  section, applied after that change.
- The owner pushes the repository and the first tag; workflows can't be run end to end
  before then. Everything up to that point is checked locally (the Dockerfile, make and the
  guard). The workflows are checked with `actionlint` when it's available.

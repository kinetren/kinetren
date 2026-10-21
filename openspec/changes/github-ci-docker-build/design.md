# Design

## Context

- `src/scripts/release-build.sh` already builds the portable tarball in an Ubuntu 22.04
  container. The Dockerfile is a heredoc inside the script, `rustup` installs whatever
  `stable` is that day, the cargo registry is mounted from `~/.cargo/registry` if it exists,
  and the delivery checks (`ldd`, `objdump -T`) run on the HOST against the container's
  output. `RUSTFLAGS=` is passed so the host's mold setting (`src/.cargo/config.toml`)
  doesn't reach a container without mold.
- `finish.sh` is the owner's full check: build, nextest (or `cargo test`), SIMD parity,
  live scenarios in Xvfb, guards. With nextest it is green; with plain `cargo test`, t663
  can time out under load (finish.sh comment, Р.68).
- Tests need: the FFmpeg 6/7/8 shared libraries at runtime (dlopen), the `ffmpeg` binary
  (`src/fixtures/gen.sh` generates fixtures on first run), clang/libclang (bindgen over the
  vendored headers), `libasound2-dev`. GPU tests skip themselves without an adapter.
- Locally `src/target/debug` is 26 GB (2,700+ files in `deps`): the dev profile keeps
  full debuginfo. A GitHub-hosted runner has roughly 14 GB free.
- Ubuntu 22.04 ships FFmpeg 4.4, which we don't support. Ubuntu 24.04 ships 6.1, which we do.
- The git remote's default branch is `master` (`kinetren/master`).

## Goals / Non-Goals

**Goals:**
- One image recipe and one toolchain version for every portable build (owner, make, CI).
- CI that is green exactly when `finish.sh` minus GPU/live would be green.
- Nothing stored on GitHub except releases and short-lived failure logs.

**Non-Goals:**
- Running the GUI inside Docker. That path was rejected with the owner's answer.
- Live Xvfb scenarios or GPU conformance in CI. These stay in `finish.sh` on a real machine.
- Other platforms (macOS, Windows, aarch64), Flatpak, AppImage, `.deb`.
- Publishing a container image to a registry. That would be storage the owner didn't ask for.
- Pushing, tagging or editing repository settings. The owner does those. The final
  repository is `github.com/kinetren/kinetren` (owner, 2026-10-22). Workflows don't hardcode
  it; only the README badges and links name it, and they already do.

## Decisions

### D1. Dockerfile in `src/packaging/Dockerfile`, toolchain pinned by `ARG`
The heredoc moves out verbatim, plus `ARG RUST_TOOLCHAIN=<x.y.z>` (today's `rustc 1.97.1`)
used by `rustup --default-toolchain`. CI reads the same value: the CI workflow has
`RUST_TOOLCHAIN` in `env:` and `dist-check.py` requires the two values to be equal.
*Alternative*: `rust-toolchain.toml` at `src/`. Rejected for now: it would also change
the owner's local toolchain choice in every `cargo` call, which is a separate decision.
The guard keeps the two places equal, which is enough.

### D2. Delivery checks run inside the container
`release-build.sh` runs `cargo build --profile dist` and then the same `ldd`/`objdump`
checks in one `docker run … bash -c`. The binutils are already in the image. The script
keeps the checks' text and messages, so the media-index scenario ("guard in
release-build.sh") stays literally true. The host keeps only: computing the version from
git, `docker build`/`docker run`, packing the tarball, writing `.sha256` and `LATEST`.
If a check fails, `dist-out` is removed, so a failed build can't leave a tarball that looks
good.

### D3. Registry and target persistence without a host Rust install
If `~/.cargo/registry` exists, it is mounted as before (the owner's case). Otherwise the
script uses a named Docker volume `kinetren-cargo-registry`. The target directory stays
`src/target/dist-container` (bind-mounted with the workspace), so `make clean` can remove
it and a second `make` is incremental. The container runs as `-u $(id -u):$(id -g)`, as it
does now. The volume is created with the right owner on first use (an init `docker run` as
root does `chown`).

### D4. Root `Makefile` is a thin front over the scripts
Targets call `src/scripts/release-build.sh`, `dist-out/<ver>/install.sh`, and
`dist-out/<ver>/kinetren` (the path comes from `dist-out/LATEST`). There's no build logic in
make, so the script stays the one place where the build is defined, and `finish.sh
--release` and `make` can't drift apart. `make` without Docker prints the one line that is
missing ("install Docker: https://docs.docker.com/engine/install/"), not a stack of errors.
`make run` without a build says to run `make` first.

### D5. CI runs natively on `ubuntu-24.04`, not in the 22.04 image
Tests need FFmpeg ≥ 6, which 22.04 doesn't have, and caching with `Swatinem/rust-cache`
works natively. The steps are: free disk (remove the preinstalled Android/.NET/Haskell
toolchains, about 25 GB), `apt install clang libclang-dev pkg-config libasound2-dev ffmpeg`,
`dtolnay/rust-toolchain` with `${RUST_TOOLCHAIN}` + clippy, `taiki-e/install-action`
nextest, `cargo clippy --all-targets -- -D warnings`, `cargo nextest run --workspace
--no-fail-fast`, then the guards. Env: `RUSTFLAGS=""` (no mold; same as the container),
`CARGO_PROFILE_DEV_DEBUG=0` and `CARGO_INCREMENTAL=0` to keep the target in the runner's
disk and the cache small. These change only debuginfo, not `opt-level`, overflow checks
or `debug_assert!`. Those keep the dev-profile semantics the tests are written against.
*Alternative*: run tests inside a 24.04 variant of the image, so "CI == `make test`".
Rejected: a second image recipe is the exact drift D1 removes, and building the image
on every push costs minutes for nothing.
*Alternative*: `cargo test` instead of nextest. Rejected: t663 times out under
`cargo test`'s per-binary parallelism (Р.68 measurement). nextest is how `finish.sh` runs it.

### D6. Release workflow reuses `release-build.sh` as is
`on: push: tags: ['v*']`, `permissions: contents: write`. Steps: checkout with
`fetch-depth: 0`, then "tag is on default branch" via `git merge-base --is-ancestor
$GITHUB_SHA origin/${{ github.event.repository.default_branch }}`, then
`src/scripts/release-build.sh`, then `gh release create "$TAG" --title "$TAG"
--verify-tag [--prerelease if the tag contains '-'] dist-out/*.tar.gz dist-out/*.sha256`. The
release text is generated by `--generate-notes`. The tarball's version stays the
commit-date+hash scheme from `kt-version`, and the tag is only the release name. Making
them equal would mean changing `kt-version`. The tag is not used to derive the version,
because a moved tag would then disagree with the binary's own version string.
*Alternative*: `softprops/action-gh-release`. Rejected: `gh` is preinstalled on runners
and one command line is easier to audit than a third-party action holding `contents: write`.

### D7. Storage: nothing lingers but releases
CI uploads only on failure: `nextest`'s JUnit/log via `actions/upload-artifact` with
`retention-days: 3`. The release workflow uploads no artifacts. It attaches files to the
release directly. The Actions cache (rust-cache) isn't an artifact. It's capped at 10 GB per
repository and evicted LRU, and `rust-cache` is set to `cache-targets: true` only on the
default branch (`save-if`), so PR branches read the cache but don't fill it.
*Rejected*: automatically deleting old releases. Release assets don't count toward any
quota, and people may need an older build.

### D8. `dist-check.py` follows the `readme-check.py` pattern
Pure Python, standard library only. There's no YAML parser in the standard library, so it
checks the workflows with line patterns (`on:` keys, `permissions`, `retention-days`,
`tags:`) — enough for the shape guarantees in the spec, not a YAML validator. It accepts
`--root DIR` so t835 can run it on a temporary copy with one defect at a time. It's called
from `src/apps/kt-cli/tests/dist_guard.rs` (like `readme_guard.rs`) and from `finish.sh`
next to `readme-check.py`. `actionlint` is run by the apply step if it's installed. It's
not a dependency.

### D9. README: product first, the cargo route moves to CONTRIBUTING
Quick start becomes: (1) download the latest release tarball, `./install.sh`; (2) or
`git clone` + `make` + `make install`; plus the runtime requirements (FFmpeg 6/7/8,
ALSA, GPU driver), which stay. The `apt install build-essential …` + `cargo run` block
moves to `CONTRIBUTING.md` ("Building from source without Docker"). This follows the
owner's third-round rule, "the README is about the product and how to run it". The
`readme-check.py` guard must stay green: English, links resolve.

### D10. Rename the binary in cargo, not only in packaging
`[[bin]] name = "kinetren", path = "src/main.rs"` in `src/apps/kt-app/Cargo.toml`. The
build output is then `target/<profile>/kinetren`, and the tarball, `make run`, `finish.sh`'s
"run this" line and the live scenarios all name the same file. The owner asked for the
name "при билде и распространении", and `finish.sh` exists precisely so the owner runs what
was just built (п.7 р.12).
*Alternative*: copy `kt-app` as `kinetren` only when packing the tarball. Rejected: there
would be two names for one program, `finish.sh` would print a `kt-app` path, and a script
could start a stale `kt-app` left in `target/` after the rename.
The package name stays `kt-app`: `-p kt-app` is in every test command, spec `TEST` path and
crate `CLAUDE.md`, and nobody installs a package name. `kt-cli` isn't renamed (only the
GUI was asked for; `kt-cli` is documented under that name in the README). Data-dir files
(`kt-app.log`, `kt-app-crash.txt`) stay too: renaming them would need a migration
for no visible gain.
`.desktop`: `StartupWMClass=kinetren` matches `with_app_id("kinetren")` (`app.rs`). The
current `StartupWMClass=kt-app` predates the rename. The apply step checks WM_CLASS
with `xprop` under Xvfb rather than assuming winit's X11 class.
`install.sh` removes `~/.local/bin/kt-app` the same way it removes `kinotrain` leftovers.

## Risks / Trade-offs

- [The runner runs out of disk on a cold build] → free-disk step + `CARGO_PROFILE_DEV_DEBUG=0`.
  The apply step measures `du -sh target` locally with the same env and records it in
  tasks.md. If it's still over ~12 GB, `debug = "line-tables-only"` for deps only is the
  fallback.
- [Time-sensitive tests flake on a 4-vCPU runner] → nextest with its default retries off.
  A flake is investigated as a defect, not retried away. Any test that genuinely needs a
  fast machine is reported to the owner. Its spec test can't be weakened without their decision.
- [FFmpeg 6.1 on the runner covers only one of the three majors] → that's the same on the
  owner's machine. Multi-major coverage is the media-index contract tested by the vendored
  headers, not by CI. Testing 7/8 in CI would need custom builds, which is out of scope.
- [The pinned toolchain gets old] → bump it in one place (the Dockerfile `ARG`). The guard
  forces CI to follow.
- [Workflows can't be run before the owner pushes] → local checks: `docker build` of the
  committed Dockerfile, `make` end to end, the shape guard (t834), `actionlint` if
  available. The first push/tag is the real test, and its result goes in tasks.md.
- [`readme-showcase` is mid-flight and also edits README] → this change edits only the
  "Quick start" section and starts README work after `readme-showcase` is committed.

## Migration Plan

1. Land the Dockerfile + script change, check `make` locally (same tarball name, checks pass).
2. Land the workflows + guard. The owner pushes, and the first CI run shows on GitHub.
3. The owner pushes the first `v*` tag, and the release appears.
Rollback: remove `.github/workflows/*`. The Dockerfile/make split has no effect on how the
owner works day to day (`finish.sh` is unchanged except for the new guard line).

## Open Questions

- Exact free-disk recipe and resulting peak disk usage: measured during apply. They
  don't change the approach.

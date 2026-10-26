# Distribution — why it is built this way

## What was missing and who asked

Before going public at `github.com/kinetren/kinetren`, nothing checked the repository
there: tests ran only on the owner's machine through `finish.sh`, and a release tarball
existed only when someone ran `release-build.sh` by hand. The README asked people to install
a Rust toolchain, clang and dev packages before they could try the editor (owner,
2026-10-22, `docs/user-feedback.md`, "Pre-release: GitHub CI and Docker build"). Change
`github-ci-docker-build`, archived 2026-10-26.

## Owner decisions (do not change without a new request)

- **Free plan, keep only releases**: CI uploads nothing that lingers; a release keeps its
  files; old releases are not deleted.
- **Build in Docker, run on the host**, with simple `make` commands.
- **The program is called `kinetren` on disk** (build output, tarball, `~/.local/bin`, the
  `.desktop` file, `make run`); the cargo package stays `kt-app`, `kt-cli` keeps its name.
- The final repository is `github.com/kinetren/kinetren`.

## Where it lives

- `src/packaging/Dockerfile` — the one build container (Ubuntu 22.04, glibc 2.35, the Rust
  toolchain pinned by `ARG RUST_TOOLCHAIN`).
- `src/scripts/release-build.sh` — the build and the delivery checks, inside the container.
- `Makefile` (repository root) — a thin front over the scripts.
- `.github/workflows/ci.yml`, `.github/workflows/release.yml`.
- `src/scripts/dist-check.py` + `src/apps/kt-cli/tests/dist_guard.rs` (t834, t835) — the
  tree stays consistent.

## Decisions and why

- **One recipe, checks inside it.** The image recipe used to be a heredoc inside
  `release-build.sh`, and the checks ran `ldd`/`objdump` on the host: a newcomer without
  binutils, or CI building the image another way, would get a different verdict from the
  owner's. Now the Dockerfile is a file, the checks run in the container, and a failed check
  removes `dist-out` so no tarball that looks good is left behind.
- **The toolchain is pinned in two places kept equal by the guard** (the Dockerfile `ARG`
  and `RUST_TOOLCHAIN` in `ci.yml`). Bump both together.
- **No host Rust needed**: the crate registry is the host's `~/.cargo/registry` if present,
  otherwise a named Docker volume; the target directory persists in
  `src/target/dist-container`, so a second `make` is incremental.
- **make is only a front.** No build logic in make, so `make`, `finish.sh --release` and the
  release workflow cannot drift apart.
- **CI runs natively on ubuntu-24.04**, not in the 22.04 image: the tests need FFmpeg ≥ 6,
  and native caching works. `RUSTFLAGS=""` because the host's mold is not on the runner;
  `CARGO_PROFILE_DEV_DEBUG=0` keeps the runner's disk; nothing changes `opt-level`, overflow
  checks or `debug_assert!`. Tests run through nextest, as in `finish.sh` (t663 times out
  under `cargo test`).
- **The release reuses `release-build.sh` as is** and refuses a tag whose commit is not on the
  default branch. The tarball's version stays the commit-date + hash of `kt-version`; the tag
  is only the release name — a moved tag would otherwise disagree with the binary's own version.
- **The binary is renamed in cargo** (`[[bin]] name = "kinetren"`), not only when packing:
  otherwise there would be two names for one program, and a script could start a stale
  `kt-app` left in `target/`.

## Rejected and why

- **`rust-toolchain.toml`** — it would also change the owner's local toolchain in every
  `cargo` call; the guard keeping two values equal is enough.
- **Tests inside a 24.04 variant of the image** — a second image recipe is exactly the drift
  the one Dockerfile removes, and building it on every push costs minutes for nothing.
- **`softprops/action-gh-release`** — `gh` is on the runners, and one command line is easier
  to audit than a third-party action holding `contents: write`.
- **Deleting old releases automatically** — release assets count toward no quota, and
  people may need an older build.
- **`clippy -D warnings` in CI** — about 25 existing warnings; making them fatal is a
  separate clean-up.

## Non-obvious details

- The workflows are checked with line patterns (`dist-check.py`): the standard library has
  no YAML parser, and these are shape guarantees, not a YAML validator.
- **The first runs on a clean runner found defects the owner's machine hid** (2026-10-25):
  tools the tests call but the runner lacks (t809 needs `xdpyinfo`, `xwd`, `convert` — the
  packages `x11-utils`, `x11-apps`, `imagemagick`), and tests racing over the real tree (t804
  wrote and removed files in `src/scripts/live` while t807/t808 globbed it). A test that
  needs to break the tree works on a copy (`KT_LIVE_DIR`); a test's external tools belong in
  the CI package list.

# Contributing

Kinetren is a cargo workspace in [`src/`](src). This page covers building, testing
and the rules every change follows. Documentation, code comments and commit messages
are written in English.

## Build

Requirements: a stable Rust toolchain (edition 2024), `clang`/`libclang` (bindgen),
`pkg-config`, `libasound2-dev`, and the FFmpeg 6/7/8 shared libraries at runtime. The
`ffmpeg` binary is needed only to generate test fixtures (`src/fixtures/gen.sh`). On
Debian or Ubuntu:

```bash
sudo apt install build-essential pkg-config clang libclang-dev libasound2-dev ffmpeg
```

```bash
cd src
cargo build -p kt-app -p kt-cli   # target/debug/kinetren and target/debug/kt-cli
cargo run -p kt-app
```

The package is `kt-app`, but the binary it builds is `kinetren`: that is the name people
install and run.

The portable release archive is built in a container from `src/packaging/Dockerfile`
(Ubuntu 22.04, so it runs on glibc 2.35 and newer, with a pinned Rust toolchain): `make` at
the repository root, or `src/scripts/release-build.sh`, which `make` calls. The result is
`src/target/dist-out/kinetren-<version>-linux-x86_64.tar.gz` plus its `.sha256`. Inside
the container the build checks that neither binary links libav* and that neither needs a
glibc newer than 2.35.

The Windows zip is cross-built in the `windows` stage of the same Dockerfile (mingw-w64,
target `x86_64-pc-windows-gnu`): `make windows`, or `src/scripts/release-build.sh --target
windows`. The result is `src/target/dist-out/kinetren-<version>-windows-x86_64.zip` plus
its `.sha256`; inside the container the PE imports of both exes are checked (only Windows
system DLLs: no FFmpeg, no mingw runtime). The zip never carries FFmpeg: the program finds
the user's shared build or downloads the pinned gyan.dev build on a click
(`src/apps/kt-app/src/ffmpeg_setup.rs`, the search plan in
`src/libs/kt-ffmpeg/src/search.rs`).

The dev profile is built with `opt-level = 1` (dependencies with 2), because audio
rendering of a dense edit does not keep up with real time unoptimised. Judge speed only
on a release build: `cargo build --release -p kt-app` and `kt-cli probe … --time`.

`kt-time`, `kt-model`, `kt-fx`, `kt-cache` and the core of `kt-engine` build and test
without FFmpeg, audio hardware or a GPU — keep it that way. wgpu lives only in `kt-gpu`;
the engine knows only the `VideoGpuExec` trait.

## Test

```bash
cd src
cargo test --workspace          # unit, harness and conformance tests
cargo clippy --all-targets
./scripts/live-check.sh         # live scenarios: the real kinetren in Xvfb, real gestures
./scripts/finish.sh             # everything above + guards, rebuilds kinetren/kt-cli, prints the version
```

- Regression tests are numbered across the workspace (`tNNN`). Every scenario in an
  OpenSpec spec names its test; named tests are never weakened or removed without the
  owner's decision.
- A UI test asserts what a person sees and does — the rectangle grew, the click landed,
  the model changed — not an internal field. Harness tests (`egui_kittest`) prove
  layout; only a live scenario (`src/scripts/live/*.py`) proves a gesture works in the
  real window.
- `make windows-test` runs the workspace tests for the Windows target under wine in the
  build container, with the pinned FFmpeg build (`src/scripts/fetch-ffmpeg-win.sh`). wine
  is the fast loop; the `windows-latest` CI job is the verdict. Tests that need python3,
  bash, Xvfb or a GPU skip themselves there with the reason printed.
- `cargo test` builds a separate test binary and does **not** rebuild
  `target/debug/kinetren` (the binary of the `kt-app` package). Finish every piece of work with `./scripts/finish.sh` and quote
  the version it prints.

## CI and releases

GitHub Actions runs `.github/workflows/ci.yml` on every push and pull request: clippy,
the workspace tests through nextest (with the system FFmpeg of Ubuntu 24.04), and the
repository guards (`readme-check.py`, `dist-check.py`, `specs-user-path.py`). The runner
has no GPU, so GPU conformance skips itself; the live Xvfb scenarios and the SIMD
conformance run only in `finish.sh`, on a real machine. A second pair of jobs cross-builds
the Windows test archive in the container and runs it on `windows-latest` without
compiling there. CI uploads nothing that lingers: the Windows archive and fixtures expire
after a day, the test log of a failed run after three days.

A release is cut by pushing a tag on the default branch:

```bash
git tag v26.10.22
git push origin v26.10.22
```

`.github/workflows/release.yml` builds the archive with the same container as `make` and
creates a GitHub Release with the Linux archive, the Windows zip and their `.sha256`
files. A tag with a suffix
(`v26.11.01-rc1`) becomes a pre-release. The Rust version is pinned in
`src/packaging/Dockerfile` and in `ci.yml`; `src/scripts/dist-check.py` keeps them
equal, so bump both together.

## How changes are made

Work goes through [OpenSpec](openspec) changes: propose → apply → archive. Contracts and
their reasons live in `openspec/specs/<capability>/{spec,design}.md`; each crate's
`CLAUDE.md` holds its invariants and the non-obvious rules for writing its code. Read
them before changing a crate. The `openspec` CLI needs Node.js 20 or newer.

- A new **feature** starts with a note in `docs/features/`: what other editors do and
  which non-obvious detail makes it work.
- A new **defect** is documented right away: the scenario and its test in the spec, the
  reason in `design.md`.
- The owner's requests and decisions are recorded verbatim in `docs/user-feedback.md`
  before the work starts. A limit, mode or look the owner asked for is not changed
  without a new request.
- Every finished stage is committed with a green check. The first line of the message
  names the stage and what was done; the body says which tests cover it.
  **Commits carry no `Co-Authored-By` or other attribution lines.**

## The twelve invariants

Breaking any of them is a regression, and each has a test; the full text is in
`openspec/specs`.

1. Audio inside the engine is always 48 kHz stereo `f32`.
2. Effect state is touched only by the audio render thread.
3. Editing the model never destroys effect state.
4. The cache is the only truth about coverage; decode quality is part of the request.
5. One decoder instance belongs to exactly one thread.
6. The audio callback is the master clock; starvation gives silence, never a stopped clock.
7. Preview and export share one composition code; the CPU path is the byte reference.
8. Scrolling and zooming the timeline never trigger decoding.
9. Export is fully in-process.
10. The binary never links libav*: FFmpeg 6/7/8 is loaded at runtime.
11. Renders are deterministic.
12. The video display is not quantised by the audio callback, and an idle UI does not repaint.

The workspace: `apps/kt-app` (GUI), `apps/kt-cli` (headless), and `libs/` — `kt-time`
(90 kHz ticks), `kt-model` (timeline model, undo, the `.kt` format), `kt-otio`
(OpenTimelineIO), `kt-ffmpeg` (dlopen of FFmpeg), `kt-media` (sources, generators,
sidecar index), `kt-text` (titles), `kt-cache`, `kt-fx` (effects), `kt-gpu` (wgpu),
`kt-engine` (render graph, transport, export). Overview: `docs/architecture.md`.

## Screenshots and the README

The README is about the product: what it does and how to run it. Architecture,
invariants and history belong here and in the design documents, not in the README.

Images under `docs/images/` are produced only by the screenshot pipeline, from free
footage (credits: [docs/images/CREDITS.md](docs/images/CREDITS.md)):

```bash
python3 src/scripts/fetch-showcase-media.py          # ≈ 0.9 GB into src/target/showcase-media
cd src && cargo build -p kt-app -p kt-cli && cd ..
python3 src/scripts/readme-shots.py --sources src/target/showcase-media
```

`kt-cli showcase` cuts the montage (including a nested reel, Slowed+Reverb curves on the
music and a morpher) and `readme-shots.py` drives the real editor in a virtual screen.
The same pipeline runs on the test fixtures in every `finish.sh`.
`src/scripts/readme-check.py --check` (run by `cargo test` and `finish.sh`) keeps the
README and the guide English, their links and images valid, and the image set equal to
what the pipeline declares.

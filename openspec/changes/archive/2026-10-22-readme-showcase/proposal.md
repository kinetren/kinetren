# Proposal

## Why

The repository is about to be published on GitHub by the owner, and its front page is a
15-line Russian stub that shows nothing of the editor. The owner asked (2026-10-21) for a
complete README, screenshots and documentation, with a timeline "packed with all kinds of
editing" cut from the real footage in `kt.9/sources/README_SOURCES`, including slowed+reverb
automation curves shown in the README. The owner also set a standing rule: **commits and all
text from now on are in English** — which contradicts the current Russian-only convention in
`openspec/config.yaml`, `CLAUDE.md` and the apply guidance, so that convention has to be
revised explicitly rather than silently ignored.

Screenshots made by hand rot on the next UI change (the landing page's `site/screenshots.py`
already drives the window by fixed pixel coordinates captured before Р.49). The showcase
therefore has to be reproducible: one command builds the showcase project from a sources
directory, one command re-takes every README image in a virtual screen.

## What Changes

- **`kt-cli showcase <out.kt> --sources <dir> [--force]`** — a new headless subcommand that
  builds a dense, valid showcase project from whatever video files are in `<dir>` (sorted by
  name, durations probed through `kt-media`): a fast-cut montage with linked A/V pairs and
  several transition kinds on the main track, overlays with on-screen geometry, video FX and
  a title generator, a music bed with **Slowed+Reverb whose `rate` and `mix` are automated by
  multi-key curves**, a morpher batch, named tracks, markers, a looped region and a tidy
  media bin. Same directory contents → byte-identical project file. The footage is never
  copied or committed; the project references it by absolute path.
- **`src/scripts/readme-shots.py`** — re-takes every README/doc image: runs `kt-cli
  showcase`, launches the real `kt-app` with `KT_LANG=en` under the existing live framework
  (`src/scripts/live/kt_live.py`, Xvfb + XTest), reaches each view through named probe
  rectangles (not pixel coordinates), and writes WebP images to `docs/images/`. The real-footage run is
  manual (the footage exists only on the owner's machine); the same code runs on the
  generated fixture media as a live scenario inside `live-check.sh`, so the pipeline cannot
  rot unnoticed.
- **README.md rewritten in English**: hero screenshot, feature tour with images (timeline,
  slowed+reverb automation, video FX, transitions, morpher, titles/generators, media bin,
  render dialog, performance panel), quick start (system requirements, build, run), `kt-cli`
  usage, short architecture overview with the twelve invariants in one line each, project
  format, links to the docs, status, footage credit line, license.
- **English user documentation** under `docs/guide/` (getting started, editing on the
  timeline, effects and automation incl. slowed+reverb, morpher, compositions and
  generators, export and `kt-cli`, keyboard and mouse reference) and a root
  `CONTRIBUTING.md` (build, tests, `finish.sh`, OpenSpec workflow, project rules). Existing
  Russian developer documents stay as they are and are linked as such.
- **Guard `src/scripts/readme-check.py --check`** (run by a cargo test and `finish.sh`):
  every image referenced from README/`docs/guide` exists, every image in `docs/images/` is
  referenced, no image exceeds its size budget, README and `docs/guide` contain no Cyrillic,
  every relative link resolves.
- **Language convention revised** — `openspec/config.yaml` context and apply guidance,
  root `CLAUDE.md` agent rules and the owner's memory note switch new text and commit
  messages to English; existing Russian documents are not translated by this change.

## Capabilities

### New Capabilities
- `showcase`: the public face of the repository — README and `docs/guide` content
  contract (English, links and images verified), the reproducible screenshot pipeline and
  its guard.

### Modified Capabilities
- `export`: the "kt-cli — headless interface" requirement gains the `showcase` subcommand
  (deterministic, refuses a missing/empty sources directory, never overwrites without
  `--force`).

## Impact

- Code: `src/apps/kt-cli/src/main.rs` (new subcommand next to `demo_project`), new test
  in `src/apps/kt-cli/tests/cli.rs`; no engine, model or format changes, file version
  unchanged.
- Scripts: new `src/scripts/readme-shots.py`, `src/scripts/readme-check.py`;
  `src/scripts/finish.sh` runs the guard.
- Docs: `README.md`, `CONTRIBUTING.md`, `docs/guide/*.md`, `docs/images/*.webp`,
  `docs/user-feedback.md` (owner's words), `openspec/config.yaml`, `CLAUDE.md`.
- Repository size: screenshots committed as WebP within a total budget (≈ 8 MB); the source
  footage (≈ 375 MB) is never committed.
- Independent of the open change `r71-ktprompts-index` (it reserves t823; this change
  starts at t824).

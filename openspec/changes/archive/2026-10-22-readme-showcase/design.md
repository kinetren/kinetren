# Design

## Context

See proposal.md — Why. Relevant current state:

- `kt-cli demo` (`src/apps/kt-cli/src/main.rs`, `demo_project`) already builds a rich
  project in Rust from model types — six tracks, markers, looped region, FX chains, a
  morpher, and Slowed+Reverb with `rate`/`mix` curves on the music clip. It references
  `synth:` sources by contract (export spec, t388: "demo renders out of the box"), so it
  cannot be pointed at real footage without breaking that contract.
- The live framework `src/scripts/live/kt_live.py` runs the real `kt-app` in Xvfb, drives
  it with XTest, and targets widgets by **probe names** that the app publishes
  (`widgets::field_probe::note`); `live-check.sh` runs every `live/*.py` scenario (except
  `kt_live`, `worker`, `balance`, `_*`) and `live-coverage.py` checks probe names are still
  published.
- `site/screenshots.py` (landing page) drives the window by fixed pixel coordinates
  captured before R.49; it is the counter-example this design avoids.
- The footage (`../sources/README_SOURCES`, ≈ 375 MB): six music/film videos, 69–563 s,
  resolutions 3840×2160, 1920×1080 (25 and 60 fps), 1280×720 @ 29.97, 1920×800, 1080×1080,
  all with audio. Mixed sizes and rates are useful: they exercise conform on screen.
- `Cargo.toml` declares `license = "UNLICENSED"`; there is no LICENSE file.
- The project convention (config.yaml, CLAUDE.md, apply guidance) is Russian text and
  Russian commit messages; the owner has now ordered English.

## Goals / Non-Goals

**Goals:**
- One command produces the showcase project from any directory of videos; one command
  re-takes every README image from it; a guard keeps README/guide/images consistent.
- The pipeline is exercised on every `finish.sh` (fixture media), so it does not rot.

**Non-Goals:**
- Translating the existing Russian developer documents (`docs/architecture.md`, specs,
  crate `CLAUDE.md`s, `docs/user-feedback.md`) — they are linked as Russian.
- Fixing `site/screenshots.py` or the landing page (separate follow-up; it could later
  reuse the probe-driven pipeline).
- Animated GIF/video captures, a hosted docs site, i18n of the README.
- Any engine, model or project-format change; the file version stays as is.

## Decisions

### D1. A new `kt-cli showcase` subcommand, not a `demo` flag and not a Python generator
The project is built in Rust from model types next to `demo_project`, so it is
compile-checked and passes `Project::validate()` before writing, and survives future
format changes (the writer is the library's). Alternatives: (a) `demo --sources` — mixes a
synthetic-by-contract command with a real-media one and muddies t388; (b) Python editing an
exported OTIO like `project_with_clips` — would hand-write the `metadata["kt"]` envelope
(automation curves, morpher, geometry), which is exactly the format knowledge the library
must own (project-format spec).

### D2. Deterministic layout rule
Sources: regular files in `<dir>` whose probe yields a video stream, sorted by file name.
Durations come from the same probe the editor uses (`kt-media`). Everything else is a pure
function of (sorted names, durations):
- **Main video track (V1 "MAIN") ≈ 150 s**: a montage cycling through the sources with a
  fixed cut-length pattern (e.g. 2.4, 3.2, 1.6, 4.0, 2.0, 2.8 s …) and in-points at fixed
  fractions of each source (inside 10 %–85 % of its length, to skip intros/credits). Each
  event gets a linked audio partner on A1 "DIALOG" at reduced gain. Every 4th–5th joint
  carries a transition, cycling at least three kinds (crossfade plus two others from
  `TransitionKind`, chosen at implementation).
- **V2 "OVERLAY"**: an opening title generator ("KINOTRAIN"), a picture-in-picture event
  with non-default on-screen geometry (the square source is a natural PiP), and FX events
  (glow, VHS, mirror/edges, event grid — at least three kinds).
- **V3 "PLATES"**: a few full-frame plates with FX, so three video layers overlap.
- **A2 "MUSIC"**: two long beds from the two longest sources, joined by a crossfade; the
  first carries **Slowed+Reverb with `rate` and `mix` curves of 5–6 keys each**, with at
  least one non-linear segment shape (R.67 segment shapes) so the curve reads as a curve in
  the screenshot; the second carries a gain envelope.
- **A3 "AMBIENCE"**: a quiet bed, so the mixer/meters show three audio tracks.
- **Morph track "BATCH"**: one morpher over a mid section applying a video FX batch.
- **Markers** INTRO / CHASE / DROP / BREAK / OUTRO; a looped region over a middle section;
  bin grouped "FOOTAGE" / "SOUND"; `UiState` with a zoom that fits the whole edit and the
  cursor at a visually busy point (so the preview shows layered video).
- Project settings fixed at 1920×1080, 25 fps, 48 kHz.
No randomness; if variety is needed, a fixed-seed integer hash of the index. Paths are
written absolute (canonicalized `<dir>`), so the project works from any output location.

### D3. Screenshots through the live framework and probe names
`src/scripts/live/readme_shots.py` holds the whole pipeline and a module-level literal
`SHOTS = [(name, what, size), …]` that declares every output image. Run by `live-check.sh`
without arguments it uses copies of the fixture videos and writes into its artifact
directory; `src/scripts/readme-shots.py --sources DIR [--out DIR]` is the thin manual
entry that defaults `--out` to `docs/images/`. Steps: `kt-cli showcase` → `Live(...)` with
`KT_LANG=en` in the environment → window resized to 1920×1080 → wait for a decoded preview
frame (`pixel_settled` on the preview centre) and thumbnails → for each shot, open the view
by probe (`click_probe`, `right_click_probe`, menu probes), assert the view's probe exists
(`has_probe`) — otherwise fail naming the step — then `shot()` and crop. Crop rectangles
come from probe rectangles of the panel/window (`_probe_rect`), never constants; where a
window publishes no probe for its frame, the app gets one `field_probe::note` call for it
(the only kt-app change, covered by `live-coverage.py`).

### D4. WebP, quality ≈ 92, via Pillow
Full-window PNGs of decoded footage are 1.5–3 MB each; GitHub renders WebP in Markdown,
and the landing page already uses WebP. Lossy WebP at high quality keeps UI text crisp at
200–600 KB. Budget per image 900 KB, total 8 MB (guard-enforced, spec `showcase`).
Alternative: quantized PNG — larger and posterizes footage.

### D5. Guard in Python, wrapped by a cargo test
`src/scripts/readme-check.py --check` (same style as `specs-user-path.py`, t799):
Cyrillic scan of README/CONTRIBUTING/`docs/guide`, relative link and anchor resolution
(GitHub anchor slug rules), image existence, orphan images, size budgets, and equality of
the referenced image set with `SHOTS` — read by `ast.literal_eval` of that one assignment,
so the guard never imports the X11-dependent live code. A `--root DIR` option lets t827 run
it on temporary trees. The cargo wrapper lives in `src/apps/kt-cli/tests/readme_guard.rs`
(kt-cli is the crate the showcase belongs to; t826/t827).

### D6. Documentation layout
- `README.md` — hero image; one-paragraph pitch; feature tour (each feature: two or three
  sentences + screenshot); quick start (system packages, `cargo run -p kt-app`); `kt-cli`
  examples (`showcase`, `render`, `export`, `probe`); architecture in brief (workspace map,
  the twelve invariants in one line each, time model); project format; documentation index;
  status; footage credit; license.
- `docs/guide/`: `getting-started.md`, `timeline.md`, `effects-and-automation.md`
  (incl. Slowed+Reverb and curves), `morpher.md`, `compositions-and-generators.md`,
  `export.md` (render dialog + `kt-cli`), `shortcuts.md`. Facts (menu names, shortcuts,
  FX lists) are taken from the code and i18n `en.toml` at write time, not from memory.
- `CONTRIBUTING.md` — build requirements, test commands, `finish.sh`, OpenSpec workflow,
  the hard rules (regression tests, invariants, commit per stage) in English.

### D7. Revision: language convention — Russian → English for new text
Revision: `openspec/config.yaml` ("all documents and artifacts are written in Russian";
apply guidance "commit message in Russian") and the root `CLAUDE.md` rule "commit message
in Russian" — **replaced by English for all new text and commit messages**, reason: direct
owner instruction of 2026-10-21 ("commits and the text of everything from now on in
English"). Existing Russian text is not translated; a spec may hold Russian and English
requirements side by side. `docs/user-feedback.md` keeps the owner's words verbatim in the
original language, with the surrounding entry in English. This change's own artifacts are
already in English.

### D8. No `docs/features/` note
The rule applies to new editor functions researched against other editors; `kt-cli
showcase` and the README are repository tooling and documentation, not an editor feature.
Recorded here so the omission is a decision, not an oversight.

## Risks / Trade-offs

- [Copyrighted footage in public screenshots] → footage is never committed; the README
  credits it to its owners and says it is used for illustration; the owner chose the
  sources and publishes the repo; `--sources` lets anyone re-shoot with other footage.
- [Binary churn in git history on every re-shoot] → re-shoot only when the visible UI
  changes; WebP budgets cap each revision.
- [`finish.sh` time (R.68 budget)] → the live scenario runs on small fixture media through
  the shared worker bench; target ≤ 40 s; if it exceeds that, run only the hero + one
  window shot in `live-check.sh` and the full set manually.
- [4K source decode slows the first preview frame] → wait on a decoded-frame condition,
  not a sleep; the showcase cursor sits on a 1080p section.
- [Probe names change] → `live-coverage.py` already fails when a scenario targets a name the
  app no longer publishes.
- [Fixture media have no meaningful audio/peaks] → the fixture run checks the pipeline, not
  the look; the look is checked by the owner on the real-footage run.

## Migration Plan

Documentation-only from the user's point of view; no file-format or behaviour migration.
Rollback = revert the commits; the `showcase` subcommand is additive.

## Open Questions

- **License.** `Cargo.toml` says `UNLICENSED`. Until the owner picks a license the README's
  License section states "All rights reserved" and the guard does not check it; choosing
  MIT/Apache/GPL later changes only that section and adds a `LICENSE` file.
- **Repository URL / badges.** The GitHub URL is unknown until the owner pushes; the
  README uses relative links only and no CI badges.

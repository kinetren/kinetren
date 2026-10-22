# Tasks

## 1. Owner's words and the language rule

- [x] 1.1 Add an entry to `docs/user-feedback.md` (English framing, owner's words verbatim in the original): README/screenshots/docs request, footage from `kt.9/sources/README_SOURCES`, slowed-reverb curves shown in README, "commits and all text from now on in English", owner publishes to GitHub; verify with `git diff docs/user-feedback.md`
- [x] 1.2 Revise the language convention (design D7): `openspec/config.yaml` context + apply guidance, root `CLAUDE.md` agent rule on commit messages, memory note `kinotrain-auto-commit.md`/`MEMORY.md`; verify `openspec validate readme-showcase` still passes and `grep -n 'in Russian' openspec/config.yaml CLAUDE.md` shows no remaining "write in Russian" rule
- [x] 1.3 Commit stage 1 (English message)

## 2. `kt-cli showcase`

- [x] 2.1 Implement `showcase <out.kt> --sources <dir> [--force]` in `src/apps/kt-cli/src/main.rs` next to `demo_project`: directory scan (non-recursive, name-sorted, video probe via `kt-media`), refusal on missing/empty dir naming it, same `write_project`/`--force` path as `demo`; verify by t825 (`src/apps/kt-cli/tests/cli.rs`)
- [x] 2.2 Implement the deterministic layout of design D2 (V1 montage ≥ 24 events with linked audio and ≥ 3 transition kinds; V2 title generator, PiP geometry, ≥ 3 video FX kinds; V3 plates; A2 two beds with crossfade, Slowed+Reverb `rate`/`mix` curves ≥ 4 keys with a non-linear segment shape; A3; morpher; markers; looped region; bin groups; `UiState` zoom/cursor; 1920×1080@25), `Project::validate()` before write; verify by t824 on copies of `src/fixtures/out/*.mp4` (two runs byte-identical, all counts asserted, absolute paths into the dir)
- [x] 2.3 Run `kt-cli showcase` on the real `../sources/README_SOURCES` into `src/target/readme-shots/showcase.kt`, open it with `cargo run -p kt-app` and check it loads without missing media and plays; `kt-cli probe <showcase.kt> 60 64` lists the expected layered events; `kt-cli probe … --time` on the release build stays under real time
- [x] 2.4 Add the ADDED requirement and scenarios to `openspec/specs/export/spec.md` via the delta; `cargo test -p kt-cli` and `cargo clippy --all-targets` green; commit stage 2

## 3. Screenshot pipeline

- [x] 3.1 Write `src/scripts/live/readme_shots.py` (design D3) with the `SHOTS` literal: `hero` (full window), `timeline`, `slowed-reverb-automation` (audio FX editor of the music bed with `rate`/`mix` curves visible; plus the curve on the timeline event if the timeline draws it), `video-fx`, `transitions`, `morpher`, `titles`, `media-bin`, `render-dialog`, `perf-panel`; `KT_LANG=en`, 1920×1080, decoded-frame wait, `has_probe` precondition per step, crops from probe rectangles, WebP q≈92 via Pillow; under `live-check.sh` it uses fixture media and its artifact dir; each step has `kt_live.step`; verify `./scripts/live-check.sh readme_shots` green and the step list printed; then put the full path `src/scripts/live/readme_shots.py` into the TEST line of the showcase delta spec and lower `UNTOUCHED_LIMIT` in `live-coverage.py` back from 88 by the `win.*`/`fxreg.popup`/`render.start` probes the scenario now touches (t804)
- [x] 3.2 Where a needed window/panel publishes no probe for its frame, add one `field_probe::note` in kt-app; verify `python3 src/scripts/live-coverage.py --check` green and the kt-app `reach_tests.rs`/`layout_tests.rs` still pass
- [x] 3.3 Write the manual entry `src/scripts/readme-shots.py --sources DIR [--out DIR]` (defaults to `docs/images/`, usage message without `--sources`); verify it prints usage with exit ≠ 0 when called bare
- [x] 3.4 Run it on `../sources/README_SOURCES` into `docs/images/`; inspect every image by eye (decoded frames, English UI, no crop cutting a window, curves clearly visible in the automation shot); adjust D2 layout/cursor if a shot looks empty; commit stage 3 with the images

## 4. Guard

- [x] 4.1 Write `src/scripts/readme-check.py --check [--root DIR]` (design D5: Cyrillic, links + anchors, image existence, orphans, 900 KB / 8 MB budgets, referenced set == `SHOTS` via `ast.literal_eval`); verify by t827 (`src/apps/kt-cli/tests/readme_guard.rs`) on temporary trees with each defect class
- [x] 4.2 Add t826 (same file) running the guard on the repository, and call the guard from `src/scripts/finish.sh`; verify t826 is red before the README exists and green after section 5

## 5. Documentation

- [x] 5.1 Rewrite `README.md` in English per design D6 (hero, feature tour with all `SHOTS`, quick start with system packages, `kt-cli` examples incl. `showcase`, architecture in brief with the twelve invariants, project format, docs index, status, footage credit, License "All rights reserved" until the owner decides); facts checked against code and `src/apps/kt-app/i18n/en.toml`; verify `readme-check.py --check`
- [x] 5.2 Write `docs/guide/` (getting-started, timeline, effects-and-automation incl. Slowed+Reverb and curves, morpher, compositions-and-generators, export, shortcuts) — shortcuts and menu names taken from the key-binding code and `en.toml`; verify `readme-check.py --check`
- [x] 5.3 Write `CONTRIBUTING.md` (build requirements, `cargo test`, `clippy`, `finish.sh`, live checks, OpenSpec workflow, hard rules); verify `readme-check.py --check`
- [x] 5.4 Add the `showcase` capability spec via the delta; `openspec validate readme-showcase --strict` green; commit stage 5

## 6. Finish

- [x] 6.1 `cd src && cargo test --workspace` and `cargo clippy --all-targets` green (t824–t827 included)
- [x] 6.2 `src/scripts/finish.sh` green (guard + live `readme_shots` included); report the version `vYY.MM.DD-hash` and the list of step screenshots; commit

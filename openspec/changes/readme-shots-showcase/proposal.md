# Proposal: readme-shots-showcase

## Why

The owner (2026-11-03): "надо обновить скриншоты в readme, там скучноватые эффекты используются,
не показываются новые симуляционные эффекты (ДО/ПОСЛЕ) и пр." (the README screenshots need
updating: the effects used are dullish, the new simulation effects are not shown as BEFORE/AFTER,
and so on). Registered in `docs/user-feedback.md`, "2026-11-03 — README screenshots are dull".

docs-refresh added four shots to the pipeline (`fx-datamosh`, `fx-camera-isp`, `fx-vhs-tape`,
`masks`) and proved them on the fixtures only (`AWAITING_FOOTAGE`). The showcase project still
dresses its events in the looks of the first README: a glow, the VHS look-alike, plain noise,
mirror+edges, and the morpher's VHS+glow. The gallery shot shows its top rows (presets,
Geometry, Mirror). Every footage image predates kt26.11.

## What Changes

- `kt-cli showcase` dresses the montage in looks a reader notices: Duotone neon, Halftone print,
  Kaleidoscope mandala, Chromatic Aberration with a dream glow, Posterize with a vignette, VHS Tape
  (worn tape), a golden glow with a vignette, a fisheye. Each is a gallery cell (the plugin's
  template plus its named variant), never numbers made up in the builder. The picture-in-picture
  floats with a far drop shadow; the insert is filmed again by Camera ISP "Cheap webcam"; the
  morpher swaps the source and plays it back from a worn VHS tape (VHS Tape "Worn rental tape").
- The pipeline (`readme_shots.py`): the effect gallery is scrolled to its end, so the stylize,
  Camera ISP and VHS Tape cells show, each on the event's own frame. A new shot `welcome`: the
  welcome window with the showcase as the recent project and its poster. The four effect shots
  take only an event with no look of its own, so the "before" half is the plain frame.
- The whole set is re-taken on the free showcase footage, every image looked at; the four effect
  shots and the welcome window are shown in the README; `AWAITING_FOOTAGE` is empty.

## Capabilities

### Modified Capabilities

- `durability`: work the program does by itself (a reel's preview) is not an edit; an untouched
  project with a reel asks no question and leaves no recovery file (t1413). Found by the welcome
  step of the pipeline.
- `ui-reach`: a window whose content arrives after it opens is sized, not fitted; the project
  search shows its finds again (t1414). Found by the search step on the footage.
- `showcase`: the README requirement names the welcome window and the striking looks; the effect
  shots scenario takes an event with no look of its own; a new scenario holds the showcase's looks
  (t1412).

## Impact

- `src/apps/kt-cli/src/showcase.rs`, `src/apps/kt-cli/tests/cli.rs` (t1412)
- `src/scripts/live/readme_shots.py`
- `src/apps/kt-app/src/app.rs` (the reel preview poll), `layout_tests.rs` (t1413),
  `docs/guide/whats-new.md`; `app.rs` `draw_search` (t1414)
- `README.md`, `docs/guide/*.md` alt text, `docs/images/*.webp`

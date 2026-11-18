# Tasks

Tests t3100–t3101 (the `features-slow` block t3100–t3199). Branch `bin-scrub-timecode` from
`orch-26-11-01`.

## 1. Note and test first

- [x] 1.1 Feature note `docs/features/media/organization/bin-scrub-timecode.md` (F-MED-008), `features-index.py --check` green
- [x] 1.2 t3100 written; red with the label's call removed ("the moment under the cursor at 0.5")

## 2. The label

- [x] 2.1 `bin_panel.rs`: `draw_scrub_timecode` in the scrub branch, `scrub_timecode_galley` with four forms; `KtApp::bin_scrub_tc` keeps the galley per moment
- [x] 2.2 t3100, t3101 green

## 3. Live, docs, close

- [x] 3.1 Live scenario `scripts/live/bin_scrub_tc.py`: a screenshot per step (hover, move, leave), reads the `bin.scrub.tc` probe; green
- [x] 3.2 Rest cost: the live `idle` scenario green (one red on the first run — the FX window's pause step, not the bin; green on rerun and on the base; found-defects row 780); a still pointer replays the galley (t3101)
- [x] 3.3 What's new line; registry `v26.10` MW1 → partial (timecode done, drag-from-moment a question); F-MED-008 `done`
- [x] 3.4 `cargo nextest run -p kt-app` 654/655 (t1644 red once under load 45, green alone — row 781; t2380 likewise — row 782), clippy clean in the touched code, target-prune; archived

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

- [ ] 3.1 Live scenario `scripts/live/bin_scrub_tc.py`: a screenshot per step (hover, move, leave), reads the `bin.scrub.tc` probe
- [ ] 3.2 Rest cost: the live `idle` scenario stays green; a hover over a card wakes nothing after the pointer stops
- [ ] 3.3 What's new line; registry `v26.10` MW1 → partial (timecode done, drag-from-moment a question); F-MED-008 `done`
- [ ] 3.4 `cargo test -p kt-app`, clippy, target-prune; archive the change

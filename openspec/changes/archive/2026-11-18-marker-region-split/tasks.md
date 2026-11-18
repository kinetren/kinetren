# Tasks

Test t3104 (the `features-slow` block). Branch `marker-region-split` from `orch-26-11-01`.

## 1. The split

- [x] 1.1 Feature note `docs/features/timeline/navigation/range-marker-from-region.md` (F-TL-016)
- [x] 1.2 `KtApp::marker_over_region_new`, `marker_to_stretch`, `stretch_marker_over_region`; the region-edge menu draws the two items; M (`marker_here`) untouched
- [x] 1.3 Catalogs `tl.menu.marker_stretch`, `msg.marker_region_taken` in five languages
- [x] 1.4 t3104 green; t142, t170 green unchanged

## 2. Live, docs, close

- [x] 2.1 Live scenario `scripts/live/marker_region_split.py`, five screenshots; the live dump now carries `markers` ([tick, length]); the menu items publish `tl.marker_region`/`tl.marker_stretch`. First run red: the scenario's own Escape cleared the region (Esc cascade) — fixed in the scenario
- [x] 2.2 i18n/font/reach/layout tests: font subsets already hold the new glyphs (t100 green); t804 green once the scenario touched the new probes
- [x] 2.3 What's new; registry U7 → done; found-defects "Changes left unfinished" line removed; F-TL-016 done; archive

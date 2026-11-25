# Tasks

Revision 2 (2026-11-25): nothing waits. Wave W1 of F-FX-007 (`effects-transitions-catchup`, D2). Test numbers t1010–t1015 are reserved by
this change. Every stage ends green and is committed at once (English message, first line = the
stage and what was done); `python3 src/scripts/target-prune.py` after every committed stage.

## 0. Records before code

- [x] 0.1 The live re-check of the lens group: F-FX-008 (`docs/features/fx/stylize/lens-housing-dirt-drops-flare.md`), its README row and the `fx/` coverage 8 / 8; F-FX-007's lens row cites it (src 8); `features-index.py --check` green
- [x] 0.2 Verify `openspec validate fx-lens-and-camera --strict` green and t979/t982 (`src/apps/kt-cli/tests/fx_catalogue.rs`) green with D2's W1 block naming `fx-video-stylize`; commit stage 0

## 1. Presets (no kernel)

- [ ] 1.1 Add "Fisheye lens" (after stage 2 lands, the slot list is fixed now), "Zoom Shake" and "Handheld" to `src/apps/kt-app/assets/fx-presets-factory.json` (D6); "Punch in" untouched
- [ ] 1.2 t1014 (`src/apps/kt-app/src/user_store.rs`): the presets parse, name registered kinds and existing parameters, "Handheld" keys carry a zigzag shape, "Punch in" equals its former record; red first with an unknown parameter in a copy

## 2. `lens_housing`

- [ ] 2.1 The positioned tap in `src/plugins/fx-video-color` (passport, WGSL, `cook`), catalog keys ×5, `BUILTIN_IDS`; F-FX-007's video list +1 (t978)
- [ ] 2.2 The stored-frame rows (t328, t329); t1010 (`src/libs/kt-gpu/tests/lens_housing.rs`) red first with the radius measured in frame pixels

## 3. `lens_dirt`, `lens_flare`

- [ ] 3.1 Both barriers in `src/plugins/fx-video-blur` with glow's threshold pass (D1, D3, D5); catalog keys ×5; `BUILTIN_IDS`; F-FX-007's list (t978)
- [ ] 3.2 The stored-frame rows; t1011 (`src/libs/kt-gpu/tests/lens_dirt.rs`) red first with dirt added without the highlight factor; t1013 (`src/libs/kt-gpu/tests/lens_flare.rs`) red first with the flare position read from the frame's brightest pixel

## 4. `lens_drops`

- [ ] 4.1 The warp plugin in `src/plugins/fx-video-distort` with its WGSL arm and its f64 map (D4); `BUILTIN_IDS`; F-FX-007's list (t978)
- [ ] 4.2 t1012 (`src/libs/kt-gpu/tests/lens_drops.rs`) red first with the WGSL hash differing by one constant; the geometry cycle and the minified case green

## 5. The picker and finish

- [ ] 5.1 t1015 (`src/apps/kt-app/src/reach_tests.rs`, the PATH of the presets scenario): "fisheye" in the picker finds the preset, a click adds both slots in one undo step; i18n/font tests (`i18n_tests.rs`, `font_tests.rs`) green; a live scenario step screenshot per PATH step (`src/scripts/live/`)
- [ ] 5.2 `cargo test --workspace` and `cargo clippy --all-targets` green; commit
- [ ] 5.3 Run `./scripts/finish.sh` from `src/`; record the version `vYY.MM.DD-hash` in the report
- [ ] 5.4 After archiving: F-FX-008 `status: done`, `CAPABILITY: openspec/specs/video-fx/`; F-FX-007's lens and shake rows `have`; the "why" of D1–D5 into `openspec/specs/video-fx/design.md`

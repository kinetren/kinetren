# Tasks

Wave W1 of F-FX-007 (`effects-transitions-catchup`, D2). Test numbers t1010–t1015 are reserved by
this change. Every stage ends green and is committed at once (English message, first line = the
stage and what was done); `python3 src/scripts/target-prune.py` after every committed stage.

## 0. Records before code

- [x] 0.1 The live re-check of the lens group: F-FX-008 (`docs/features/fx/stylize/lens-housing-dirt-drops-flare.md`), its README row and the `fx/` coverage 8 / 8; F-FX-007's lens row cites it (src 8); `features-index.py --check` green
- [x] 0.2 Verify `openspec validate fx-lens-and-camera --strict` green and t979/t982 (`src/apps/kt-cli/tests/fx_catalogue.rs`) green with D2's W1 block naming `fx-video-stylize`; commit stage 0

## 1. Presets (no kernel)

- [ ] 1.1 Add "Fisheye lens" (after stage 2 lands, the slot list is fixed now), "Zoom Shake" and "Handheld" to `src/apps/kt-app/assets/fx-presets-factory.json` (D6); "Punch in" untouched
- [ ] 1.2 t1014 (`src/apps/kt-app/src/user_store.rs`): the presets parse, name registered kinds and existing parameters, "Handheld" keys carry a zigzag shape, "Punch in" equals its former record; red first with an unknown parameter in a copy

## 2. `lens_housing` (waits for the positioned tap of `plugin-api-stage1`)

- [ ] 2.1 The kernel in `src/plugins/fx-video-color` (CPU tap + WGSL), passport, catalog keys ×5, `BUILTIN_IDS`; F-FX-007's video list +1 (t978)
- [ ] 2.2 The golden row (t328) and conformance (t329); t1010 (`src/plugins/fx-video-color/tests/lens_housing.rs`) red first with the radius measured in frame pixels

## 3. `lens_dirt`, `lens_flare` (wait for the barrier pass of `plugin-api-stage1`)

- [ ] 3.1 Both kernels in `src/plugins/fx-video-stylize` with glow's threshold helper (D1, D3, D5); catalog keys ×5; `BUILTIN_IDS`; F-FX-007's list (t978)
- [ ] 3.2 Golden rows and conformance; t1011 (`src/plugins/fx-video-stylize/tests/lens_dirt.rs`) red first with dirt added without the highlight factor; t1013 (`src/plugins/fx-video-stylize/tests/lens_flare.rs`) red first with the flare position read from the frame's brightest pixel

## 4. `lens_drops` (waits for geometry primitives reachable from the API)

- [ ] 4.1 Warp mode 5 `drops` in `src/libs/kt-fx/src/geom.rs` and `src/libs/kt-gpu/src/shaders/geom_fused.wgsl` in ONE commit (D4); the plugin in `src/plugins/fx-video-distort`; `BUILTIN_IDS`; F-FX-007's list (t978)
- [ ] 4.2 t1012 (`src/libs/kt-fx/tests/lens_drops.rs`) red first with the WGSL hash differing by one constant; the t181 geo cases and the minified case green

## 5. The picker and finish

- [ ] 5.1 t1015 (`src/apps/kt-app/src/reach_tests.rs`, the PATH of the presets scenario): "fisheye" in the picker finds the preset, a click adds both slots in one undo step; i18n/font tests (`i18n_tests.rs`, `font_tests.rs`) green; a live scenario step screenshot per PATH step (`src/scripts/live/`)
- [ ] 5.2 `cargo test --workspace` and `cargo clippy --all-targets` green; commit
- [ ] 5.3 Run `./scripts/finish.sh` from `src/`; record the version `vYY.MM.DD-hash` in the report
- [ ] 5.4 After archiving: F-FX-008 `status: done`, `CAPABILITY: openspec/specs/video-fx/`; F-FX-007's lens and shake rows `have`; the "why" of D1–D5 into `openspec/specs/video-fx/design.md`

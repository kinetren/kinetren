# Tasks

Tests t3108–t3110 (the `features-slow` block). Branch `kind-picker-previews` from `orch-26-11-01`.

## 1. Note, service, pickers

- [x] 1.1 Feature note `docs/features/automation/presets/kind-picker-previews.md` (F-AUT-012)
- [x] 1.2 `fx_thumbs::Swap` (generator kind, transition kind), `with_swap`, `PickerCtx::swap_texture`, `kind_cell`, `gallery_width`
- [x] 1.3 The generator kind picker (`gen_ui::draw_gen_section`) and the transition kind picker (`transition_editor`) as galleries; the transition window has its misclick clock; the scene's sources close with the last cell window
- [x] 1.4 t3108, t3109, t3110 green; t227, t511 green with the gallery

## 2. Live, docs, close

- [x] 2.1 Live scenario `scripts/live/kind_picker_previews.py`, five screenshots; it found the generator cells drawn by the present kind (fixed, t3109)
- [x] 2.2 kt-app 661/661, clippy clean in the touched files; rest cost: live `idle` green (a pause wakes only load-watch and the bench), the cells exist only while a gallery is open
- [x] 2.3 What's new; registry G6 → done; F-AUT-012 done; archive

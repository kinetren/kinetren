# Tasks: chroma-key-upgrade

> DESIGN ONLY until the orchestrator's go; no open question. Revision 3. Tests: t3558–t3567 (t3568, t3569 spare).
> Areas: COLOR-FX (`plugins/fx-video-color`), a hook in kt-plugin-api and `kt-fx/src/geom.rs`, SHELL (`app.rs`
> badge, `fx_editor.rs` form). Needs nothing unmerged; `Viewer::cut` is `cut_event` after fx-graph v2d (text merge).

## 0. Notes first

- [x] 0.1 F-MSK-004 refreshed; proposal, design (D0–D10), deltas `video-fx`, `project-format`, `ui-reach`; strict
- [x] 0.2 A22 folded in; the mapping, the tap position and the row limit corrected against the code (D0)

## 1. The contract (kt-plugin-api, kt-fx)

- [ ] 1.1 `VideoKernel::neighbours`; `geom::is_pointwise` asks it; t3565
- [ ] 1.2 `PluginTap::of_at` cooks from the whole value slice; t329 unchanged before 2.1
- [ ] 1.3 `UiDecl.internal`: out of the picker, the library and the guides; `check` refuses a file naming one; t3566

## 2. The kernel (`plugins/fx-video-color/src/lib.rs`, `key_px.wgsl`, `tap_greenscreen.wgsl`, `greenscreen.wgsl`, `choke.wgsl`, `key_join.wgsl`)

- [ ] 2.1 the rows, `key_px` in both positions, the neutral branch, despill; the distance kernel deleted; references stored again naming A22; t3558, t3559, t3567
- [ ] 2.2 choke and soften in `gpu_values`; t1054 over the new rows' tops at K 1 and 4; t3560
- [ ] 2.3 old files: t973 as it is, `demo-concat` 04 and 07, curves on `tolerance`; the new rows' version; t3561

## 3. Views and Analyze

- [ ] 3.1 `matte_view`, `Viewer.view`, the chips in the badge (`preview.viewer.view.<mode>` probes); t3563
- [ ] 3.2 `kt_fx::key_analyze`; `fx_thumbs::compose_scene` factored out; `AnalysisKind::Key`; the button on three owners; t3562, t3564

## 4. Close

- [ ] 4.1 i18n ×5 and `subset-fonts.py`; live scenario `key` (a registered scenario with a screenshot per step: Analyze on the fixture, the Viewer, Status, Esc, a blue-screen event, an event with no screen); the guide's effect page
- [ ] 4.2 the load check: the heavy showcase paused with the Viewer's Status on a key, the mouse moving, an effects window open
- [ ] 4.3 What's new; F-MSK-004 → `CAPABILITY: openspec/specs/video-fx/`; registry B3 and A22 moved
- [ ] 4.4 `src/scripts/finish.sh` (orchestrator), version in the report

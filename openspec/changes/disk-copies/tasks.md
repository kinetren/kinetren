# Tasks: disk-copies

> DESIGN ONLY until the owner answers Q1–Q2 (`proposal.md`). Renamed from `proxies` (A10). Tests reserved:
> t3440–t3459 (used t3440–t3450).

## 0. Notes first

- [x] 0.1 F-MED-002 renamed and refreshed with the owner's A10
- [x] 0.2 proposal, design (D0–D9, class walk), deltas `media-index`, `export`, `ui-reach`; `openspec validate disk-copies --strict`
- [ ] 0.3 the owner's answers Q1–Q2 in `docs/user-feedback.md`, folded in; the final name everywhere

## 1. The file (kt-ffmpeg, kt-media)

- [ ] 1.1 `MuxConfig.time_base` + per-frame pts; MKV; `keyint=1`; audio stream copy; t3440
- [ ] 1.2 `kt_media::disk_copy::make` (own source, `.part` + rename, progress, cancel) into the project cache folder (after `project-cache-folder`); t3441
- [ ] 1.3 the binding in the cache folder's index; camera-copy checks; t3442

## 2. The engine (kt-engine)

- [ ] 2.1 `CopySource` (original natives, `decoded_scale`), decode step clamp; t3443
- [ ] 2.2 the adaptive size, the costly-file rule from the player's decode counters; t3449
- [ ] 2.3 `Player::set_disk_copies` through `replace_media` on the sources thread; epoch; t3444
- [ ] 2.4 `note_leaf` names the opened file; t3446
- [ ] 2.5 export guard; t3445

## 3. The interface (kt-app)

- [ ] 3.1 the switch beside the quality (modes, count, size), card badges and items, `UiState.disk_copies`; i18n ×5; t3447, t3450
- [ ] 3.2 jobs as `Background` lane jobs
- [ ] 3.3 planned live scenario `disk_copies`, a screenshot per step; t3448
- [ ] 3.4 idle: live `idle` with ready copies and the switch on

## 4. Close

- [ ] 4.1 What's new line; F-MED-002 verdict → `CAPABILITY: openspec/specs/media-index/`
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator, per stage), version in the report

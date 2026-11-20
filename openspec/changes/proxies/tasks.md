# Tasks: proxies

> DESIGN ONLY until the owner answers Q1–Q3 (`proposal.md`). Tests reserved: t3440–t3459.

## 0. Notes first

- [x] 0.1 F-MED-002 refreshed with the current code
- [x] 0.2 proposal, design (D1–D8, class walk), deltas `media-index`, `export`, `ui-reach`; `openspec validate proxies --strict`
- [ ] 0.3 the owner's answers Q1–Q3 in `docs/user-feedback.md`, folded into the design

## 1. The file (kt-ffmpeg, kt-media)

- [ ] 1.1 `MuxConfig.time_base` + per-frame pts; MKV; `keyint=1`; audio stream copy; t3440
- [ ] 1.2 `kt_media::proxy::make` (own source, `.part` + rename, progress, cancel); t3441
- [ ] 1.3 sidecar table `proxy` (no schema bump), attach checks; t3442

## 2. The engine (kt-engine)

- [ ] 2.1 `ProxySource` (original natives, `decoded_scale`), decode step clamp; t3443
- [ ] 2.2 `Player::use_proxies` through `replace_media` on the sources thread; epoch; t3444
- [ ] 2.3 `note_leaf` names the opened file; t3446
- [ ] 2.4 export guard; t3445

## 3. The interface (kt-app)

- [ ] 3.1 bin card menu, badges, the switch with its count, `UiState.proxies`; i18n ×5; t3447
- [ ] 3.2 the job as a `Background` lane job; the one-time hint for slow decode (Q3 a)
- [ ] 3.3 planned live scenario `proxies`, a screenshot per step; t3448
- [ ] 3.4 idle: live `idle` with a ready proxy and the switch on

## 4. Close

- [ ] 4.1 What's new line; F-MED-002 verdict → `CAPABILITY: openspec/specs/media-index/`
- [ ] 4.2 `src/scripts/finish.sh` (orchestrator, per stage), version in the report

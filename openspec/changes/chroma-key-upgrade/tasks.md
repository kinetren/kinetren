# Tasks: chroma-key-upgrade

> DESIGN ONLY until the owner answers Q1–Q2 (`proposal.md`). Tests reserved: t3558–t3569.

## 0. Notes first

- [x] 0.1 F-MSK-004 refreshed (eyedropper and slot masks exist; the matte view through the Viewer)
- [x] 0.2 proposal, design (D1–D7, class walk), deltas `video-fx`, `ui-reach`; `openspec validate chroma-key-upgrade --strict`
- [ ] 0.3 the owner's answers in `docs/user-feedback.md`, folded into the design

## 1. The kernel (`src/plugins/fx-video-color`)

- [ ] 1.1 rows (omitted at defaults), the difference model as a barrier pass graph (alpha+despill, choke H/V, soften H/V); the distance path untouched; t3558, t3559, t3560, t3561
- [ ] 1.2 the picker's variant sets `model: difference`; "Use the new key"

## 2. Views and analysis

- [ ] 2.1 the Viewer's view mode for a key node, preview plan only (kt-engine); t3563
- [ ] 2.2 Analyze: five moments at the slot input, histogram cluster, percentiles, one `SetFx`; t3562

## 3. Close

- [ ] 3.1 i18n ×5; planned live scenario `key` written with the scenario, t3564; idle check with the Viewer on a key
- [ ] 3.2 What's new; F-MSK-004 verdict → `CAPABILITY: openspec/specs/video-fx/`; registry row B3 moved
- [ ] 3.3 `src/scripts/finish.sh` (orchestrator), version in the report

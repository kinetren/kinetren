# Tasks: multicam

> Decided 2026-11-20 (A18); revision 2 implementation-ready at `fc60a5d9` (2026-11-25). Code starts on the orchestrator's go, after
> `shared-events`. Tests: t3505–t3514, t4247–t4249.

## 0. Notes

- [x] 0.1 F-TL-005; proposal, design (D0–D8), deltas `compositions`, `media-index`, `project-format`, `ui-reach`; `openspec validate multicam --strict`
- [x] 0.2 his answers Q28–Q30 (kkt-26.11.20 lines 28–29); planned OTIO row

## 1. The model (kt-model)

- [ ] 1.1 `Multicam`, the reserved handles, `ParamOwner::Angle` in `apply_overrides`, validation; t4247
- [ ] 1.2 `ops::{create_multicam, cut_switch}`, `SetAngle`; `amend_batch` unless it is in; t3505, t3507
- [ ] 1.3 expand; t3508; the next free file version; t3511; kt-otio; t3512

## 2. Sources and the engine

- [ ] 2.1 `Source::timecode`, `MediaInfo.start_tc` (kt-ffmpeg, kt-media); t4248
- [ ] 2.2 no request for hidden angles; t3506. The nest stream handed on; t3509
- [ ] 2.3 `AngleCells`, its lane, ledger row, withdrawals; t3510

## 3. The interface (kt-app)

- [ ] 3.1 the dialog, its job and its "not found" rows; the corner button, the grid, digits (the key list, t1700), menus; i18n ×5; t3513, t4249
- [ ] 3.2 live `multicam` with a screenshot per step (t3514); `idle`

## 4. Close

- [ ] 4.1 What's new ("multicam: cameras matched by their sound, switched live with a click or a digit"); F-TL-005 verdict; M1, N1, A18 moved
- [ ] 4.2 `finish.sh` (orchestrator)

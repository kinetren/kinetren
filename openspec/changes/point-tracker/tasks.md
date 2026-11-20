# Tasks: point-tracker

> DESIGN ONLY until the owner answers Q1–Q2 (`proposal.md`). Tests reserved: t3520–t3534.

## 0. Notes first

- [x] 0.1 F-MSK-005 refreshed (links exist now; point tracker first)
- [x] 0.2 proposal, design (D1–D8, class walk), deltas `tracking`, `automation`, `project-format`, `ui-reach`; `openspec validate point-tracker --strict`
- [x] 0.3 planned row in "What OTIO export loses"
- [ ] 0.4 the owner's answers in `docs/user-feedback.md`, folded into the design

## 1. The model (kt-model)

- [ ] 1.1 `Tracker`, `TrackPoint`, `Sample`, validation, `SetTrackers`, split/copy rules; t3521
- [ ] 1.2 `ParamOwner::Tracker`, link reading (linear, threshold hold), derived angle/scale/corners
- [ ] 1.3 format by content, one line per second; t3525

## 2. The matcher and the session

- [ ] 2.1 luma pyramid, ZNCC, sub-pixel, adaptation, prediction — `kt_media::tracker`; t3520
- [ ] 2.2 the offline run job (own source, forward span reads, background class, play pause, stop rules); t3522

## 3. Consumers (kt-engine through links)

- [ ] 3.1 apply gestures writing links with mapping expressions; t3523, t3524

## 4. The interface (kt-app)

- [ ] 4.1 preview right-click, boxes and path mesh, head controls, effects window section, Apply menu; i18n ×5; t3526
- [ ] 4.2 the planned live scenario `tracker` (written with the scenario), t3527; `idle` scene with a tracker shown

## 5. Close

- [ ] 5.1 What's new; F-MSK-005 verdict → `CAPABILITY: openspec/specs/tracking/`
- [ ] 5.2 `src/scripts/finish.sh` (orchestrator), version in the report

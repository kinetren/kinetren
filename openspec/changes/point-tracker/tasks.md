# Tasks: point-tracker

> DESIGN ONLY until the orchestrator's go; the owner decided both questions (A20). Tests reserved: t3520–t3534
> (used t3520–t3531).

## 0. Notes first

- [x] 0.1 F-MSK-005 refreshed (links exist now; point tracker first)
- [x] 0.2 proposal, design (D1–D8, class walk), deltas `tracking`, `automation`, `project-format`, `ui-reach`; `openspec validate point-tracker --strict`
- [x] 0.3 planned row in "What OTIO export loses"
- [x] 0.4 the owner's answers (A20) folded into the design: the planar kind (D9), the defaults (D10)

## 1. The model (kt-model)

- [ ] 1.1 `Tracker`, `TrackPoint`, `Sample`, validation, `SetTrackers`, split/copy rules; t3521
- [ ] 1.2 `ParamOwner::Tracker`, link reading (linear, threshold hold), derived angle/scale/corners
- [ ] 1.3 format by content, one line per second; t3525

## 2. The matcher and the session

- [ ] 2.1 luma pyramid, ZNCC, sub-pixel, adaptation, prediction — `kt_media::tracker`; t3520
- [ ] 2.2 the offline run job (own source, forward span reads, background class, play pause, stop rules); t3522

- [ ] 2.3 the planar kind: corners, seeded robust fit, region refinement with the ignore mask; t3528
- [ ] 2.4 the defaults (box sizes, both ways, native size, thresholds); t3530

## 3. Consumers (kt-engine through links)

- [ ] 3.1 apply gestures writing links with mapping expressions; t3523, t3524
- [ ] 3.2 the plane's gestures; the mask transform's perspective form; t3529

## 4. The interface (kt-app)

- [ ] 4.1 preview right-click, boxes and path mesh, head controls, effects window section, Apply menu, "Track plane" with the quad; i18n ×5; t3526, t3531
- [ ] 4.2 the planned live scenario `tracker` (written with the scenario), t3527; `idle` scene with a tracker shown

## 5. Close

- [ ] 5.1 What's new; F-MSK-005 verdict → `CAPABILITY: openspec/specs/tracking/`
- [ ] 5.2 `src/scripts/finish.sh` (orchestrator), version in the report

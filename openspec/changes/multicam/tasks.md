# Tasks: multicam

> DESIGN ONLY until the owner answers Q1–Q3 (`proposal.md`). Tests reserved: t3505–t3519.

## 0. Notes first

- [x] 0.1 F-TL-005 refreshed: nesting (F-TL-004) and sync (F-SPD-004) now exist
- [x] 0.2 proposal, design (D1–D8, class walk), deltas `compositions`, `media-index`, `project-format`, `ui-reach`; `openspec validate multicam --strict`
- [x] 0.3 planned row in "What OTIO export loses"
- [ ] 0.4 the owner's answers in `docs/user-feedback.md`, folded into the design

## 1. The model (kt-model)

- [ ] 1.1 `Multicam`, `Angle`, `AngleSel`, `AudioSel`; validation; `resolve_reel` applies the selection; `instance_content` includes it
- [ ] 1.2 `CreateMulticam` ops (group, sync by `offset_from_pyramids` / timecode / start, build, replace); t3505
- [ ] 1.3 `cut_switch`, `SetAngle`; live coalescing by gesture token; t3507
- [ ] 1.4 expand with the selection; t3508

## 2. The engine (kt-engine)

- [ ] 2.1 dropped angles produce no request/coverage; t3506
- [ ] 2.2 zero-seam sound across a cut with a fixed sound; t3509
- [ ] 2.3 the cells: plans of the reel per angle, compose-thread background class, `Priority::Thumbnail`, bounded look-ahead, frames-cache sizing; t3510

## 3. The interface (kt-app)

- [ ] 3.1 create dialog, menus, digits in `controls.rs` ×layouts, the angle viewer; i18n ×5; t3513
- [ ] 3.2 live scenario `src/scripts/live/multicam.py`; t3514; the `idle` scene with the viewer open on a pause

## 4. Format and close

- [ ] 4.1 `.kt` version by content, kt-otio stacks with enabled angle; t3511, t3512
- [ ] 4.2 What's new; F-TL-005 verdict → `CAPABILITY: openspec/specs/compositions/`
- [ ] 4.3 `src/scripts/finish.sh` (orchestrator), version in the report

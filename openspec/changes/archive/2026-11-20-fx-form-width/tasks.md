# Tasks: fx-form-width

> Applied 2026-11-20 (the orchestrator's go). Tests: t3591–t3594, t3593b.

## 0. Notes

- [x] 0.1 proposal, design (D1–D5, class walk), delta `ui-reach`; `openspec validate fx-form-width --strict`

## 1. The flow (kt-app `widgets.rs`)

- [x] 1.1 `FormFlow` (min column from measured labels, cached), columns of grids, group cards in a masonry; pairs on one row
- [x] 1.2 every caller moves to the flow (effect, morpher, transition, camera, track, generator, title, settings, render); the ISP stages as groups; t3591

## 2. The guard

- [x] 2.1 the three-width guard over every kind and registry window, `FORM_EXEMPT` with reasons; red on the old code first; t3593, t3594
- [x] 2.2 the live scenario `form_width` written with its scenario (screenshots at three widths), t3592

## 3. Close

- [x] 3.1 What's new; the repeated-requirements row of `docs/user-feedback.md` points at the guard; registry E7 moved
- [ ] 3.2 `src/scripts/finish.sh` (orchestrator), version in the report

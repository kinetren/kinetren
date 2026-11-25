# Tasks: edit-analysis

> Implementation-ready by stages at `fc60a5d9` (2026-11-25): design D10 says what starts before its dependency; code on the orchestrator's go. Tests: t4211–t4228. Stage A3 needs `ripple-and-trim-modes`,
> `speech` needs `pause-detection`, stage B `download-library` and the recognizer's loader.

## 0. Notes

- [x] 0.1 F-MED-011; proposal, design (D1–D10, class walk), deltas; the planned OTIO row; `openspec validate edit-analysis --strict`

## 1. Stage A1 — sound

- [ ] 1.1 `measure.rs`, the dependency graph, the store; `audio` moved, its bytes kept; t4211, t4212
- [ ] 1.2 `onset`, `beats`; t4213, t4214
- [ ] 1.3 `BeatFix`, format by content; t4215, t4216
- [ ] 1.4 the switch, the overlay, snapping, Mark beats; t4217, t4218; live `beats`

## 2. Stage A2 — picture

- [ ] 2.1 `luma`, `motion`, `cuts`; t4219, t4220
- [ ] 2.2 the overlay, Mark and Split at cuts, Still stretches; t4221, t4222; live `cuts_motion`

## 3. Stage A3 — acts

- [ ] 3.1 Find moments, "Instead of this"; t4223, t4224; live `find_moments`
- [ ] 3.2 Cut on the beat; t4225

## 4. Stage A4 — links

- [ ] 4.1 `with_material`, `ParamOwner::Material`, the per-event memo's key, `ensure_measures`; after the link owner `geo`; t4226, t4227
- [ ] 4.2 idle on the heavy showcase, switches on, panel open; t4228

## 5. Close

- [ ] 5.1 What's new; F-MED-011 verdict; registry N5 and candidate 3 moved; stage B entered in the build queue
- [ ] 5.2 `src/scripts/finish.sh` (orchestrator), version in the report

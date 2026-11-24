# Tasks: pause-detection

> DESIGN ONLY until the orchestrator's go; no open question. Tests: t3466, t4204–t4209. Needs nothing first; Delete waits
> for `ripple-and-trim-modes`.

## 0. Notes

- [x] 0.1 F-AUD-008 pointed here; proposal, design (D1–D7, class walk), deltas; `subtitles-and-text-editing` D4 hands its pauses over; `openspec validate pause-detection --strict`

## 1. The detector

- [ ] 1.1 `PeakPyramid::build_until` reads the partial last burst, the PEAK version; t4204
- [ ] 1.2 `kt-media/src/pauses.rs`: the floor, the score, hysteresis, merging, margins, edges; t3466, t4205, t4206
- [ ] 1.3 the mapping to the timeline, the intersection of tracks; t4207

## 2. The window

- [ ] 2.1 the window, the overlay mesh, exclusion by click, the three actions, the series job; t4208
- [ ] 2.2 i18n ×5; live scenario `pauses`; t4209 (idle with the window open)

## 3. Close

- [ ] 3.1 What's new; F-AUD-008 verdict; registry N4 moved
- [ ] 3.2 `src/scripts/finish.sh` (orchestrator), version in the report

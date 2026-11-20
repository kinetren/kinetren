# Tasks: track-device-sound

> His answer kkt-26.11.18b line 29 "Дать звук отдельно"; the orchestrator's go 2026-11-20. Tests t3230–t3249.

## 1. Model and file
- [x] 1.1 `Track.sound`, `TrackProps.sound`; validation in `SetTrackProps` and `Project::validate`
- [x] 1.2 the pair rule for tracks after `SetTrackFx`/`SetTrackProps`; the default link; removal clears; t3230, t3231
- [x] 1.3 project format 6 by content; t3233
- [x] 1.4 kt-otio track envelope `sound`, v17 by content; t3234; the row in "What OTIO export loses"

## 2. Engine
- [x] 2.1 `TrackProcessor` seats its nodes at timeline zero with the project grid; t3232

## 3. Window
- [x] 3.1 the track window's pair editor and the "Device sound" row; i18n ×5; t3235
- [x] 3.2 idle with the track window open; t3236
- [x] 3.3 live `track_sound` with a screenshot per step; live `idle` on the heavy showcase with the track window open

## 4. Close
- [ ] 4.1 specs' tests, nextest of touched crates, clippy, What's new, registry U8, found-defects rows, target-prune, archive

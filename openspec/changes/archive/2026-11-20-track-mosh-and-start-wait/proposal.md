# Proposal

## Why

Two rows of `docs/found-defects.md`, handed over by the orchestrator (2026-11-20):

- **820** — Datamosh is offered in a video TRACK's picker, yet the engine skips a stream slot on a
  track stage (`video-fx`: "A stream slot on a reel instance or a track stage SHALL be skipped"): a dead
  slot (kt-app rule 9), and track-device-sound left it without a sound half. The owner asked sound for
  all three devices on a track (kkt-26.11.18b line 29).
- **822** — live scenarios waited for the program's window by a clock: `_wait_window` 180 s, the
  stand's first frame 60 s, its geometry 20 s, and every other `ui-probe.window()` call exited at once
  when the window was not mapped yet. At a load average of 28, `idle` and `track_sound` went red on
  the first try with "the kt-app window did not appear".

## What Changes

- **Datamosh on a track is refused with its reason** in the track window, at the one door after the
  graph (the picker, a paste and a preset all land there); slots an older project holds stay. Making it
  work on a track is new architecture (design D1), handed to the engine designers.
- **The live rig waits for the window as a consequence, up to the test's guard**: one door
  `kt_live.the_window` asks again while the program lives, up to `TEST_DEADLINE` (the ten minutes of
  `kt_time::test_deadline()`); the start waits of `kt_live` and of the stand use the same guard.

## Capabilities

### Modified Capabilities

- `ui-reach`: Datamosh refused on a track; the live rig's start wait.

## Impact

- kt-app `track_window.rs`, i18n ×5; `scripts/live/kt_live.py`, `scripts/live/worker.py`.
- Tests: t3237, t3238.

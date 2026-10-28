# Proposal

## Why

The owner (kkt list 2026-10-27, item 1, `docs/user-feedback.md`): on Windows, at least under wine,
"select a loop region and press Play — the video plays but freezes until you click somewhere inside
the region; from then on this playback is fine. Select another region — the same again." The sound
keeps looping; only the picture stops. Reading the code shows a defect of the preview's
"never backward" ratchet that every platform has and that a large output latency makes visible:
the ratchet is released when the HEAD wraps, while the shown frame (head minus latency, folded into
the region) folds back one latency LATER — by then the release is spent and the fold is refused as
a rollback. t117, the guard of the earlier loop freeze (R.15 item 2), runs with zero latency and
cannot see it.

## What Changes

- The display key's ratchet is released by a change of the DISPLAY position's lap (the moment the
  shown frame itself folds back to the region start), not by the head's wrap counter. The head's
  wrap no longer releases it at all: at that moment the shown frame is continuous.
- Re-anchoring under latency is defined: after a jump (Play from outside the region, a click, a
  region change) the shown frame starts at the jump target and does not fold into the region's
  END before the first audible frame of the target arrives — today Play from outside shows the last
  `latency` of the region first and then holds it.
- A region edit during loop playback re-anchors the display lap against the new region.
- The fix stays inside the display clock (kt-engine `player.rs`) and its one consumer
  (`draw_preview` in kt-app); the transport's head, wraps and generation, the audio path and
  the export are untouched.
- A probe of the real output latency and callback size under wine (perf counters, the black box)
  is recorded before the fix, so the reproduction is measured, not assumed.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `transport`: the requirement "Video display clock is interpolated and compensates latency" gains
  the rule for re-anchoring under latency (what releases the ratchet, where the shown frame starts
  after a jump, the loop under a latency larger than a UI frame) and the scenarios that hold it.

## Impact

- Code: `src/libs/kt-engine/src/player.rs` (`video_display_frame` / `video_playhead` report the
  display lap, `preview_key_step` callers), `src/apps/kt-app/src/app.rs` (`draw_preview`: the
  release condition), `src/apps/kt-app/CLAUDE.md` (its stale line "a rollback ≤2 cells is
  ignored" is corrected).
- Tests: t900–t905 (kt-engine `player.rs`, `tests/frame_order.rs`, kt-app `layout_tests.rs`); t117
  stays and gains a non-zero latency case.
- No project-format change, no new dependency; invariants 6 and 12 hold (no lock or allocation
  added to the callback, the display stays interpolated and repaint-on-motion).

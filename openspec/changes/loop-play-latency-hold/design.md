# Design

## Context

See proposal.md — Why. The parts of the display clock this change touches (observed in the code,
read-only):

- **The release.** `draw_preview` (`src/apps/kt-app/src/app.rs:6138-6150`) computes
  `tgen = (transport.generation(), transport.wraps())` and passes
  `reanchor = !is_playing || tgen != preview_gen` to `preview_key_step`
  (`src/libs/kt-engine/src/player.rs:1288`). The ratchet returns the previous key on any backward
  step unless `reanchor` is set (`player.rs:1295-1299`); a refused step bumps
  `perf.video_rollbacks` (`app.rs:6150`, shown in the perf panel as the second number of "V skipped",
  `app.rs:6905-6911`).
- **The shown frame.** `Player::video_playhead` (`player.rs:778-806`) reads `latency_smooth`
  (an EMA `sm + (raw − sm)/8` updated by the render thread, `player.rs:1012-1016`) and returns
  `video_display_frame(head + extra, latency, region, dir, rate)`. Inside the region that is
  `rs + (head − rs − lat).rem_euclid(span)` (`player.rs:1330`): for a head in `[rs, rs + lat)` the
  shown frame lies at the END of the region, `re − lat + (head − rs)`.
- **The wrap counter.** `Transport::advance_frames_wrapped` (`src/libs/kt-engine/src/transport.rs:70-97`)
  bumps `wraps` when the head crosses the region end (`transport.rs:88`), called by the render thread
  with the frames the callback consumed (`player.rs:1063-1066`). R.15 item 2 introduced it so the
  ratchet could be released on a wrap without bumping the generation (the reverb tail must ring on).
- **Play from outside.** `transport_toggle` returns `Resume { seek: Some(region start) }` when Loop is
  on and the cursor is outside the region (`app.rs:2525-2527`); `on_transport_toggle` seeks there
  (`app.rs:2631-2634`). The seek bumps the generation, so the first playing frame re-anchors — onto
  `video_display_frame(rs, lat, …) = re − lat`, the region's end.
- **The latency.** The cpal callback stores `playback − callback` (in 48 kHz frames) plus the output
  edge's resampler delay (`src/apps/kt-app/src/audio_out.rs:70-77`; the edge since commit 97b152c,
  windows-build stage 6). cpal 0.16's WASAPI backend computes `playback = callback +
  frames_available` (`cpal-0.16.0/src/host/wasapi/stream.rs:564-582`), where `frames_available =
  buffer − padding` (`stream.rs:254-262`) — the size of the chunk about to be written, i.e. about
  one device buffer; at 44.1 kHz the edge resampler adds its own delay. Under wine the latency is
  therefore at least one callback and typically several UI frames.
- **The guard that missed it.** t117 (`src/libs/kt-engine/tests/frame_order.rs:242-345`) plays from
  the region start with `latency_smooth` never set (zero), so the head's wrap and the display fold
  coincide and the release always lands.

### What the arithmetic predicts (a model, not yet a measurement)

A scratch model of exactly these three functions (`video_display_frame`, `preview_key_step`, the
wrap rule; 60 Hz UI, 30 fps cells, a 1 s region, four laps) gives the longest hold of the key:

| callback | latency | Play from outside | seek inside |
|---|---|---|---|
| any | 0 | 2 UI frames | 2 |
| 2 048 | 512 | 61 (the first lap) | 2 |
| 2 048 | 2 048 | 61 | 178 (every lap) |
| 480 | 512…14 400 | 42…240 (every lap) | 42…181 |

So: (1) any non-zero latency freezes the FIRST lap after Play from outside — the re-anchor lands on
the region's end, and the later fold to its start is refused; (2) when the latency exceeds the head's
advance between its wrap and the first UI frame that sees the wrap (about one callback), EVERY lap
freezes. On Linux (PipeWire quantum 1 024–2 048, latency below it) only (1) is possible; under wine
(small WASAPI period, latency of a whole buffer plus the resampler) the model predicts (2) as well.
The owner's "after a click it is fine" is consistent with (1) but not with (2) — which is why stage 1
measures the real numbers on wine before the fix, and t901 sweeps both regimes.

## Goals / Non-Goals

**Goals:**
- The picture never holds while the head moves, for any latency and callback size, in both
  directions, with Play from outside, a click inside, or a region edit.
- The ratchet stays a rare safety net (t114's bound holds): the fix must not reopen rollbacks.

**Non-Goals:**
- Correcting cpal's WASAPI latency estimate (padding vs. frames available) — a separate A/V-sync
  question; this change works for any latency value, right or wrong.
- Any change to the audio path, the transport head, `wraps`, the generation or the loop arming.

## Decisions

### D1. Release by the display lap, not by the head's wrap
`video_display_frame` (or a sibling that `video_playhead` calls) reports, together with the frame,
the display lap: `wraps − 1` while the shown frame still lies in the previous lap (the head is within
`L·|rate|` after the region start), `wraps` otherwise; mirrored in reverse. `draw_preview` puts
`(generation, display_lap)` into `preview_gen` instead of `(generation, wraps)`. The release then
happens on the very UI frame where the shown frame folds, which is the only moment a backward step is
legitimate.
Alternatives: (a) release on the head's wrap AND keep the release "armed" until the next backward
step — a pending release also lets a jitter rollback through after the fold, the very thing the
ratchet exists for; (b) subtract the latency from the wrap position inside the transport — the
transport would learn about the output, against the clock's single truth (invariant 6); (c) allow
rollbacks bigger than half the region — a size tolerance is what R.14 item 14 removed ("blind to
speed").

### D2. After a jump, the shown frame starts at the target
For the first `L·|rate|` frames after a jump to `p` the display position is clamped to `p` (in the
direction of play), exactly like the existing clamp at zero for a linear timeline
(`(playhead − lat).max(0)`): what is audible in that time is the silence of the pre-roll, not the
region's end. The display lap is counted from the jump, so the first fold happens one lap later.
This needs the jump target (the head at the last generation change) next to the head; `Player`
already observes generation changes in `video_playhead`'s clock state (`video_clock`), so the target
lives there — no new atomics in the callback.
Alternative: keep showing `re − lat` and only fix the release — the first `L` of every Play from
outside would still flash the region's last frames before the start.

### D3. The display lap counts forward only
Within one generation the lap never decreases: the EMA latency jitters (t114: ±a quantum), and
without this rule a head standing near `rs + L` would flip the lap back and forth and release the
ratchet twice. A region change or a speed-sign change is a jump (D2) and restarts the count.

### D4. The tests run without a wall clock
t900/t902 are pure-function tests; t901/t903 drive `Transport`, the display function and
`preview_key_step` from a deterministic callback/UI schedule (the scratch model's loop in Rust), so
the sweep over quanta and latencies is exact and fast; t904 is the only live-player test (as t117),
with the latency injected through `AudioShared::output_latency_frames` before Play so the EMA has
converged. t905 checks the consumer in kt-app.

## Risks / Trade-offs

- [The wine measurement contradicts the model, e.g. the freeze has another cause (prefetch of the
  region head, a stall)] → stage 1 records `latency_smooth`, the callback size, `video_rollbacks` and
  the black box (`Ev::Wrap`, `Ev::Stall`) on wine before any code change; if the rollback counter does
  not grow during the freeze, the hypothesis is wrong and the stage stops with the finding.
- [D2 holds the first frame for `L` after every seek — with a 200 ms latency a click shows the target
  frame for 200 ms before it moves] → that is the truth of what is audible; today the same 200 ms show
  a frame from another place.
- [The display lap needs the latency at the same instant as the frame] → both come from one
  `video_playhead` call; no second read of `latency_smooth`.

## Migration Plan

None: no format, no setting. Rollback = revert the commits; t117 and t114 keep guarding the old
behaviour's good parts.

## Open Questions

1. Should the wrong WASAPI latency estimate (cpal reports the chunk about to be written, not the
   queued padding) be corrected for A/V sync on Windows? Recommended: yes, as a separate defect
   change after this one, measured with the same stage-1 probe; this change does not depend on it.

# Transport — why it is this way

## What hurt and for whom

- **Starvation of the ring stopped the clock.** When the ring was empty, the callback repeated the last
  frame and did not increase consumption, and the playhead moved exactly by what was consumed. Every
  underrun frame was a frame the timeline did not advance over: real time went on, but the programme
  played slower. The owner saw this as "M3TEOR is somehow slowed down" and "then there is no sound or
  video at all for the main event" — it was not the event that slowed down, but the whole programme.
  This contradicted the engine's own invariant ("a miss at the deadline → silence + underrun flag"):
  there was silence, and time stood still.
- **Underruns were measured in two units.** The path without FX counted frames, the path with FX
  counted burst misses; one starvation gave 24000 against 24. The counters in the UI were not labelled,
  and the owner read them as "left/right" channels ("underruns broke a long time ago, we will not do a
  bisect" — and no bisect was needed).
- **Editing a region rebuilt the whole engine.** Every move of a boundary and every press of Loop cloned
  the project into 1961 events and stopped the audio stream; the ring emptied, and after hours under
  starvation everything played slower from then on.
- **Video showed exactly 24 frames/s** regardless of quality and backend — the playhead moved in quanta
  of the callback (PipeWire up to 2048 samples = 42.7 ms), and consumers read the raw transport.
- **Judder at slow speed.** The output latency was subtracted from the timeline position in wall-clock
  frames without taking the speed into account: at 0.25× the compensation was four times too large, and
  the jitter of the latency EMA rolled the picture back.
- **Loop was out of step with the transport.** A seek to the right of the region's end: the cursor moved
  linearly, while the sound and the preview looped the region. Reverse loop silently broke across the
  wrap. Switching Loop off on the fly jumped the sound into silence far beyond the cursor.
- **Speed.** The interval (0, 0.25) on the transport strip was unrepresentable by construction; in slowed
  a small or negative value silently became 0.25 through three independent clamps; a negative speed inside
  an event did not exist as a notion at all. The owner: "I must be able to set 0.05 or −0.03".
- **At pause the system lagged** even with an empty project: the render thread spun 500 times per second,
  the prefetch cloned the project every 100 ms.

## Decisions and why

- **The clock runs on the frames handed out by the callback.** Starvation is audible silence and
  `callback_underruns`, but not a stop of time. The only exception is before the FIRST frame after
  start/flush: there is nothing to go from, and the interpolation buffers hold the material up to the seek.
  Invariant 6 of the root CLAUDE.md was reformulated exactly so.
- **The callback knows only the speed module.** The direction is applied by the render thread when the
  playhead advances; a sign change is a mini-seek without a generation bump. Reverse transport (rate < 0 on
  the transport) plays through the raw path without FX — "like Vegas" — and this is a separate branch from
  the signed speed INSIDE an event (capability `audio-render`).
- **The display clock is a separate function.** Wall-clock interpolation between the steps of the master
  clock, smoothed latency, scaling of latency by speed, ratchet of the key. All consumers of the display
  position (the preview, the timeline cursor, the toolbar timecode) go through it — invariant 12 (capability `ui-timeline`).
- **The loop's armed state — only on jumps.** The membership condition is one for all: a wrap when crossing
  the end FROM INSIDE, the edge is inclusive, reverse is mirrored. Flipping the region Some↔None during
  playback is a mini-seek. The region and the Loop flag live in the UI "convenience" and are not read from
  the project by the engine: the loop arrives through a separate call, the content — through model commands.
- **A wrap is a seam, not a seek.** The FX state is not reset, the reverb rings out across the loop; a real
  seek with pre-roll is recognised by `reset()`. Pre-roll misses are not underruns (they are not audible).
- **The transport scale is piecewise.** Linear 0.01 → 0.25 over the first 30 % of travel, then logarithmic
  0.25 → 2; the slopes at the joint, 0.0096 and 0.0089 ×/px, are not perceptible as a break. Stop — only a
  narrow zone `|t| < 0.018` (1.5 px at a half-strip of 85 px). Accepted by the owner: "smooth scale ±0.01…±2 +
  Shift + typed input", stop 1–2 px at the centre, a double click — 1×, the height of the widget does not change.
- **Two axes of the event's speed.** P = ∫rate — the signed position (the read address, irreversible); A = ∫|rate|
  — the consumption (monotonic, reversible, the resampler cursor and the trims live on it). The mirror axis of
  "Reverse" is the span (min P, max P). The owner's decision: minus — like the velocity envelope of Vegas, the
  direction may change in the middle of an event; 0 — a freeze frame; at the edge of the material we hold the
  frame; the "Reverse" flag remains a mirror on top.
- **Holding below 0.01.** The owner's decision: "drag down to 0.01, silence only at zero". The cursor stands
  still, the output is zero, the nodes keep working — the reverb tail rings out, as in a pause between events.
- **Pause = parking the threads** on a condvar with a safety timeout; the prefetch rebuilds the graphs only on
  the project version.
- **Black box** — a ring of 512 records, deduplication, a snapshot once per second when deltas are non-zero, `STALL`.
  The owner asked for compactness because of jumps over small regions.

## Rejected and why

- **Not moving time on starvation** (the earlier behaviour) — violated the engine's invariant
  and made the whole programme slower than real time.
- **A pure logarithm for the speed scale** — it has no zero (stop would still have to be pinned with a zone),
  and at the bottom the scale is infinitely sensitive.
- **Shift as division of the cursor position** (`t = (pos.x − left)/8`) — Shift pressed in the middle of a gesture
  would instantly pull the value toward the centre. The increment is divided instead.
- **The speed input field as a separate window** — the owner fixed the toolbar's form ("two parallel strips, do not
  change the height"); the field is in place of the strip.
- **"Minus in slowed = a second input to the Reverse button"** — rejected by the owner: the minus goes backward through
  the material inside the event and may change direction in its middle.
- **"Mute below 0.25"** — rejected by the owner in favour of holding below 0.01.
- **Reversing the position P** — it is non-monotonic, there is nothing to reverse; the consumption had to be reversed.
- **Deciding the hold per frame** — the resampler cursor would not survive it; the decision is per chunk.
- **`render_head` as the source of the armed state** — with an armed loop it grows linearly and folds on every burst,
  unsuitable for the membership condition.
- **An absolute hysteresis threshold of the boost 0.02** — on a base of 0.05 this is 40 % of the base: the widget showed
  a boost while the player played as if standing still.
- **`set_project` on an edit of the region** — cloning the project and stopping the render thread for an edit the plan
  does not touch.

## Non-obvious details

- The consumption in `varispeed_fill` (`src/libs/kt-engine/src/player.rs`) includes two frames of interpolation priming:
  at rate = 1 and 100 output frames `consumed == 102`; after the end of the source the last frame is output, but the
  consumption keeps growing. A non-positive or NaN step — silence without consumption (otherwise the phase went to −∞
  and the interpolation diverged — dangerously loud).
- `callback_step` (`src/libs/kt-engine/src/player.rs`) returns `None` at `|rate| < 0.001` — the only real lower limit of speed;
  `set_playback_rate` clamps to ±`ENGINE_RATE_MAX` = 8. All other "0.25"s were a slider, not the engine.
- The raw `output_latency` jitters between callbacks by ±1 frame: the `latency_smooth` is read
  (EMA `sm + (raw − sm)/8` in the render thread, `src/libs/kt-engine/src/player.rs`), and
  `Player::video_playhead` takes it together with `display_extrapolation`.
- `display_extrapolation` clamps the step to the size of the last quantum; `set_playback_rate` resets the extrapolation
  window (it was learned on the steps of the previous speed) — therefore a boost by the space bar must go with hysteresis
  (`boost_needs_update`, `src/apps/kt-app/src/app.rs`): 2 % of the base with a floor of 0.0005.
- `video_display_frame(head, latency, region, dir, rate)` (`src/libs/kt-engine/src/player.rs`) subtracts `latency · |rate|`,
  and in reverse adds it (what is audible now is LATER on the timeline); measurement t114: the old formula at 0.25× gave 186
  rollbacks out of 1600 samples, the new one — 10; at 1× both — 34.
- `preview_key_step` (`src/libs/kt-engine/src/player.rs`) is a ratchet: rollback is forbidden at any size, re-anchoring only
  on seek/pause, in reverse the norm is a decrease. The ratchet is a rare safeguard; if it fires constantly, the display clock jitters.
- `loop_arm_after_jump(head, region, dir)` (`src/libs/kt-engine/src/player.rs`): forward — up to and including the end, reverse —
  from the start inclusive; without a region and with a degenerate region — always armed. The guard of reverse-fill is
  `render_head <= re`; the former `> rs` went out when render_head (ahead of the playhead by the depth of the ring) went below the start of the region.
- `lookahead_windows(head, len, region)` (`src/libs/kt-engine/src/player.rs`) folds the lookahead window into the region and no longer than
  the loop — otherwise dozens of orders for one burst and the eviction of the region's head (prefetch-thrash).
- The seam of a wrap is `ClipProcessor::seam` (`src/libs/kt-fx/src/audio/mod.rs`): it re-sets the source without resetting the state.
  Edits of the region boundaries without a flip (Shift+A/D inside the loop) do not trigger a rebuild.
- `on_region_changed` (`src/apps/kt-app/src/app.rs`) does not call `set_project`; the loop arrives through `Player::set_loop_region`.
  A precedent of the same class — `retune_media` (capability `generators`).
- The three "0.25"s in different places — `RATE_KNEE` (`src/apps/kt-app/src/app.rs`), the former `RATE_CLAMP` (`src/libs/kt-model/src/warp.rs`),
  the `rate` spec of the effect — coincided as a number for different reasons, and only one was real. Rule: a limit without a written reason is
  a suspect, not a fact; before repeating a number next to it, ask what it follows from THERE.
- The constants of the scale (`src/apps/kt-app/src/app.rs`, `slider_to_rate`/`rate_to_slider`): `RATE_STOP = 0.018`, `RATE_FLOOR = 0.01`,
  `RATE_KNEE = 0.25` at `RATE_KNEE_T = 30 %` of travel, `RATE_MAX = 2`; 1× stands at 65–80 % of travel; `|r| > 2` sits at the edge (the boost mark
  remains visible). Weak notches at 0.1 and 0.25 without labels.
- `drag_advance` (`src/apps/kt-app/src/app.rs`) accumulates a virtual cursor `speed_drag_vx`, Shift divides the increment by `SPEED_FINE_DIV = 8`;
  the "second wind" (the mouse going beyond the edge of the strip) computes `over_px` from the same virtual cursor.
- The input field `speed_entry` (`draw_speed_entry`, `src/apps/kt-app/src/app.rs`) — the shape of the timecode mask of the toolbar (170×20, monospaced,
  underline in the accent colour); events are consumed so that digits and Enter do not leak into hotkeys; the first digit replaces the dimmed current value
  entirely; an unreadable value does NOT close the field and colours the underline; `surrender_focus` — on EVERY closing path, including the Esc cascade
  (the guard in `draw_speed_control` returns the focus on the next frame).
- `parse_rate_text` (`src/apps/kt-app/src/app.rs`) accepts a comma, a typographic minus and the suffix "×"/"x"; `normalize_rate` — an almost-zero (< 0.005)
  goes to the stop, above it — the floor 0.01 with a sign.
- The bit identity of the warp map on positive curves is by construction, not by hope (`src/libs/kt-model/src/warp.rs`): the body of `source_progress`
  is untouched, the sign appeared from the symmetric clamp; `consumed_abs` splits the segment at zero STRICTLY when `r0·r1 < 0` — on positive curves
  the branch is not executed, `fabs` of a positive f64 is identical, the order of additions is the same. The guard — a frozen copy of the old map
  IN THE TEST t603d, comparison by `to_bits()`: GOLDEN would find out later and would not say WHAT had shifted.
- `span()` vs `used()` (`ClipWarp`, `src/libs/kt-model/src/warp.rs`): on a signed curve the event can go back and return, consuming twice what was passed.
  The mirror is taken around the span; the mirrored branch did not always have the clamp `max(0)` to the start of the file — t604g found it.
- The coverage of the speed module for the resampler is computed over the SPLIT segments (`src/libs/kt-model/src/warp.rs`): from the ends alone the minimum
  would come out overstated, and the resampler would reject a change of ratio — found red as t322 (capability `audio-fx`).
- The trims of a stretched clip extrapolate the speed at zero; at rate ≤ 0 division by zero gave `i64::MIN` and an overflow in debug — now the edge answers
  "the material to the left is not consumed by this event" (`trim_min_start`/`trim_max_end`, `src/libs/kt-model/src/warp.rs`).
- `RATE_AUDIO_FLOOR = 0.01` lives in `src/libs/kt-model/src/warp.rs`; the hold is decided per chunk by the midpoint rule (`src/libs/kt-fx/src/audio/mod.rs`,
  `eff_rate(..).abs() < RATE_AUDIO_FLOOR`); the module goes into rubato (`src/libs/kt-fx/src/audio/warp.rs`). There `set_rate` previously swallowed a refusal via
  `let _ =` — a curve outside the window ±32 around the anchor silently got stuck; now a refusal is counted (`rejected`) and caught by `debug_assert`.
- The address map of audio — runs of constant direction; the run positions accumulate as INTEGERS `p_{k+1} = p_k ± (a_{k+1} − a_k)`
  (`src/libs/kt-fx/src/audio/mod.rs`): with rounding separately at a joint there would be a skip or a doubled frame — a click exactly at the turning point.
- The constant −1 stays on the FLAT path (`has_warp` looks at the modulus, there is no stretching, the resampler is not needed) — therefore the equality of t616
  with "Reverse" is exact, not "within tolerance" (`src/libs/kt-engine/tests/reverse.rs`).
- Reverse transport has ITS OWN `wanted_audio_bursts_raw` (`src/libs/kt-engine/src/audio.rs`): the raw path reads linearly, while the window by the warp
  was warming the wrong bursts — an eternal miss (capability `audio-render`).
- The gate of the sub-mix of a nested reel is by the NON-MONOTONICITY of the read, not by the reverse flag (`NestStream::new_for` with pre-roll and a block,
  `src/libs/kt-engine/src/audio.rs`; the signedness of the curve — `kt_model::rate_signed` through `src/libs/kt-fx/src/audio/mod.rs`): a mirrored instance
  and a signed speed mean the same thing — a move on every burst (capability `compositions`).
- `FlightRecorder` (`src/libs/kt-engine/src/flight.rs`) is written only by the render thread — it does not enter the callback (invariant 6); `STALL` — playing,
  while the playhead stands still for more than a second; the dump goes to `<project>.ktflight.log`, and it is exactly that which proved the freeze for the owner.

## Deferred

- The extended resampler mode (a curve below 0.25) limits the anchor to 2×: on fast sections of SUCH a curve the
  anti-aliasing cut is coarser. Left deliberately — the combination "below 0.01 and above 2× in one curve" is rare,
  and memory costs more (`src/libs/kt-fx/src/audio/warp.rs`).
- The known phase shift of a live edit when editing the region boundaries without a Some↔None flip is kept as it was.

# Transport

## Purpose

The playback clock and its control: the cpal callback as the master clock, the playhead by the frames handed out,
the interpolated clock of the video display, the loop region with its "armed" state, the speed with a sign on the
transport and inside an event (freeze frame, backward motion through the material), and a continuous speed scale
with fine adjustment and numeric input.

## Requirements

### Requirement: Master clock — the cpal callback; the playhead follows the frames DELIVERED
The playhead SHALL advance exactly by the number of frames that the audio callback handed to the device,
not by the number of frames successfully read from the ring. Starvation of the ring
SHALL produce audible silence and an increase of the callback underrun counter, but MUST NOT
stop time. The single exception — until the FIRST frame has arrived after a start or flush:
then consumption is zero, and silence is output, not the material before the seek.
In the callback — no allocations, no locks; the wall clock in the render paths is forbidden.
Why: starvation that froze the playhead made the whole programme play slower than real time, and the owner saw
it as "the event is slowed down".

#### Scenario: Starvation of the ring does not stop the clock
- **WHEN** the stream has started and then broke off, while the callback keeps handing buffers to the device
- **THEN** the playhead moves by the number of frames handed out (plus two frames of interpolation priming), the callback underruns grow, the output is the last frame / silence
- **TEST** t284, t32 (`src/libs/kt-engine/src/player.rs`)

#### Scenario: Before the first frame time stands still
- **WHEN** the ring is empty and no frame has arrived yet (the start of playback, after a flush)
- **THEN** consumption is zero, and zeros go to the output — the material before the seek is not emitted
- **TEST** t284b (`src/libs/kt-engine/src/player.rs`)

#### Scenario: The callback does not know the direction
- **WHEN** the transport speed is negative or close to zero
- **THEN** the ring read step equals `device_step × |rate|`; at `|rate| ≈ 0` — silence without consumption; the sign is applied when the playhead advances, and the wrap of the region works in both directions; outside the region a negative advance is clamped to 0
- **TEST** t45 (`src/libs/kt-engine/src/player.rs`), t43 (`src/libs/kt-engine/src/transport.rs`)

#### Scenario: Underruns are measured in frames and labelled
- **WHEN** an event with FX and the same without FX starve; the counters of the render and the callback are shown in the UI
- **THEN** both paths count misses in frames (numbers of the same order, not 1024×); the counters are labelled "render/callback", not read as L/R, and the callback silence is shown in milliseconds
- **TEST** t280 (`src/libs/kt-engine/tests/loop_fx.rs`), t289 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Reverse transport — an exact frame-by-frame mirror of forward
- **WHEN** the transport runs backward (dir < 0) and the ring is filled with blocks [head−N, head) with a frame-by-frame reversal, including across a wrap of the loop region
- **THEN** the concatenation of the reverse blocks is byte-for-byte equal to the frame-by-frame mirror of the forward run of the same range, and the backward wrap goes by rem_euclid
- **TEST** t42 `t42_reverse_blocks_are_exact_mirror_of_forward`, `t42_reverse_looped_wraps_backwards` (`src/libs/kt-engine/tests/reverse_transport.rs`)

### Requirement: Video display clock is interpolated and compensates latency
The DISPLAY position SHALL be interpolated by the wall clock between the steps of the master clock
(the clamp — the size of the last quantum), subtract the smoothed output latency,
scaled by speed (`latency · |rate|`, in reverse — added with a plus sign), and pass through a
ratchet: rolling the key back is allowed only at a seek / re-anchoring. The raw playhead of the transport MUST NOT
be read for display during playback. Why: the callback quantum (PipeWire up to 42.7 ms) limited the display to 24 frames/s,
and an unscaled latency at 0.25× rolled the picture back.

#### Scenario: Interpolation covers every cell of the grid
- **WHEN** the playhead steps by quanta of 2048 frames, and the UI samples at 60 Hz
- **THEN** the sequence of keys of the 60 fps grid is monotonic and without gaps
- **TEST** t56 (`src/libs/kt-engine/src/player.rs`)

#### Scenario: Latency compensation, clamp and wrap
- **WHEN** the display frame is chosen at an output latency, at the start, inside the loop region and in reverse
- **THEN** the latency is subtracted (in reverse it is added), clamped at zero, a negative latency does not move forward, inside the region it wraps to its end/start
- **TEST** t22, t44 (`src/libs/kt-engine/src/player.rs`)

#### Scenario: The display does not roll back at slow speed
- **WHEN** the speed from {0.01, 0.05, 0.25, 0.5, 1, 2, 8} in both directions, the latency jitters by ±a quantum
- **THEN** the display key never goes against the direction, the ratchet fires rarely (< 50 out of 1600 samples at rate ≤ 1); rollback is allowed only by seek/pause, forward the skips are counted, in reverse the norm is a decrease
- **TEST** t114 (`src/libs/kt-engine/src/player.rs`)

### Requirement: Loop region wraps by transport rules; "armed" state only on jumps
The wrap SHALL happen only when the end of the region is crossed FROM INSIDE (the edge
`p == e` is inclusive; for reverse — `p == s`), and all consumers (both fill branches,
the Playback prefetch, the advance of the playhead, the display) SHALL see one `effective_region`.
The armed state SHALL be recomputed only on jumps (a change of generation, of the speed sign,
of the region value) by the wrapped playhead; a Some↔None flip on the fly is a
mini-seek without a generation bump. A wrap is a SEAM, not a seek: the FX state is not
reset, the misses of pre-roll go to `warmup_misses`, not to underruns. An edit of the
region MUST NOT rebuild the engine. Why: inconsistent membership conditions gave "the cursor moves linearly while the sound loops the region", and a rebuild on every move of a boundary dropped the audio stream.

#### Scenario: The wrap is sample-continuous, the head of the region is pinned
- **WHEN** the region is aligned neither to a burst nor to a frame, and it plays several laps
- **THEN** the output is byte-for-byte equal to the reference from direct renders, on a warm cache there are no misses; the wrap arithmetic of the transport: before the region — linear, past the end — back to the start
- **TEST** t14 (`src/libs/kt-engine/tests/audio_engine.rs`)

#### Scenario: A seek past the end of the region disarms the loop until the next jump
- **WHEN** during loop playback the cursor is placed beyond the end of the region
- **THEN** the sound and the transport are linear, `wraps` does not grow; a seek inside arms again; a target exactly at the end — inside; the display beyond the region — linear, at the edge — an inclusive fold
- **TEST** t132 (`src/libs/kt-engine/tests/loop_disarm.rs`), t134 (`src/libs/kt-engine/src/player.rs`)

#### Scenario: A reverse loop lives longer than a lap, the approach from the right is linear
- **WHEN** the reverse transport runs the region for three laps, then a seek past the end
- **THEN** the output is byte-for-byte equal to the block reference (a render_head below the start of the region does not switch the loop off); from the right — a linear approach to the region and the loop's pick-up at the entrance
- **TEST** t133 (`src/libs/kt-engine/tests/loop_disarm.rs`)

#### Scenario: An edit of the region does not rebuild the engine
- **WHEN** the boundaries of the region or the Loop flag change on the fly
- **THEN** the project is not cloned into the engine and the renderers are not rebuilt, while an edit of the content — MUST arrive
- **TEST** t286 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The prefetch lookahead window folds into the region
- **WHEN** the cursor is inside the region, and the window is longer than the remainder or the lap
- **THEN** the window is cut into "to the end" + "wrap to the head", no longer than a lap; outside the region — linear; the edge of the end — is already the head of the next lap
- **TEST** t167b (`src/libs/kt-engine/tests/loop_fx.rs`)

### Requirement: When paused the player is quiet; edits to the media composition on the fly do not disturb transport
Pause SHALL park the render and prefetch threads (a safety timeout, ≤ 8 passes/s),
rebuilding the renderers without an edit of the model MUST NOT happen, pause→resume MUST NOT
deadlock. The addition and the substitution of sources SHALL survive coverage, the playhead and the
generation, and a substitution SHALL clear the cache of the old source and recompute the decode scale
step on the fly. Why: at pause the system lagged even with an empty project, and after a file returned, the preview showed the placeholder for a long time.

#### Scenario: Pause — parking of the threads
- **WHEN** the player stands on pause for a second; then an edit of the model; then resume
- **THEN** ≤ 8 render passes, zero prefetch rebuilds; the edit wakes exactly one rebuild; after resume the ring fills again
- **TEST** t20 (`src/libs/kt-engine/tests/player_dynamic.rs`)

#### Scenario: Adding media incrementally
- **WHEN** a new file is added on a warm cache
- **THEN** coverage, bytes, playhead and generation are preserved, the new worker is alive
- **TEST** t18 (`src/libs/kt-engine/tests/player_dynamic.rs`)

#### Scenario: Substitution of a source on the fly (container: capability `media-index`, `video-plan`)
- **WHEN** a source that has returned to the disk replaces the placeholder during playback
- **THEN** the frames of the old source leave the cache, and the new native changes the decode scale step without an edit of the timeline — the strips and the prefetch already ask for it
- **TEST** t211, t316 (`src/libs/kt-engine/tests/player_dynamic.rs`)

### Requirement: Transport speed scale is continuous ±0.01…±2; stop is a narrow zone; Shift and numeric input
The speed strip SHALL display continuously `RATE_FLOOR = 0.01 … RATE_MAX = 2` by
modulus with an odd, strictly monotonic, invertible function; exactly 0 SHALL be given
only by the narrow zone in the centre (`|t| < RATE_STOP`), and the edge of the zone — the floor 0.01, not 0.25.
Shift SHALL divide the INCREMENT of a gesture by 8 (the virtual cursor), a right click or
Ctrl+click on the strip SHALL open the input field IN PLACE of the strip (the height does not change),
a double click — 1×. The boost by the space bar SHALL reach the player with a threshold
relative to the base. The engine accepts any `|rate| ≥ 0.001`, clamps ±8.
Why: the owner demanded that 0.05 and −0.03 can be set; the former dead zone made the interval (0, 0.25) unrepresentable by construction.

#### Scenario: Strip mapping
- **WHEN** the position on the strip is converted to a speed and back
- **THEN** the stop only at `|t| < RATE_STOP`, the edge of the zone gives `RATE_FLOOR`, the mapping is odd and strictly monotonic with a step of 0.001, the round trip holds on {0.01, 0.03, 0.05, …, 2}, 1× stands at 65–80 % of travel, 0.03 and 0.05 are separated by ≥ 1.9 px at a half-strip of 85 px
- **TEST** t46 (`src/apps/kt-app/src/app.rs`)

#### Scenario: Arithmetic of input and fine adjustment
- **WHEN** what was typed is parsed ("0,05", "−0,03" with a typographic minus, "2×"), a value is normalised, a drag with Shift takes place, the boost hysteresis is computed
- **THEN** an almost-zero is the stop, above the threshold — the floor with a sign, the ceiling — the engine limit, NaN — the stop; garbage is not parsed; Shift divides the increment, not the position; the boost threshold is 2 % of the base
- **TEST** t626 (`src/apps/kt-app/src/app.rs`)

#### Scenario: The typed value reaches the player
- **WHEN** a right click or Ctrl+click on the strip, typing "0.05" / "−0,03", Enter
- **THEN** the base speed and the player's speed equal what was typed, the field is closed and the focus is released; the space bar with the field open does not play, Esc does not edit; it works repeatedly; the tooltip of the strip promises both inputs
- **TEST** t627 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: A drag gives small speeds
- **WHEN** the strip is dragged 6 px from the centre, the same with Shift, released at 1 px from the centre, a double click is made
- **THEN** ~0.05 (not 0.25 and not a stop); with Shift — the same path, 8 times smaller; 1 px — exactly the stop; a double click — 1×
- **TEST** t628 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Boost by the space bar from a small base
- **WHEN** the base is 0.05 and the space bar is held
- **THEN** the player's speed lags behind the displayed multiplier by no more than 10 % of the base, does not exceed `base × SPACE_BOOST_MAX`, the strip shows the boost; after release — a return to the base
- **TEST** t629 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Event speed with sign — two axes: position P = ∫rate and consumption A = ∫|rate|
The warp map SHALL store the signed speed in `RATE_CLAMP = (−32, 32)`, zero
is allowed. The position in the material SHALL equal the entry point + ∫rate (non-monotonic,
not invertible); ONLY the consumption A SHALL be inverted (monotonic, for the trims).
The mirror axis of the "Reverse" flag is the SPAN `(min P, max P)`, not "consumed". The extrema
of the span SHALL be sought at the zeros of the speed, and for a monotonic curve give exactly the ends.
On curves lying entirely within 0.25…32 the new map MUST be bit-for-bit equal to the former one.
Why: a slowdown "was not applied" because of three independent clamps at 0.25, and people tried to invert a position that cannot be inverted.

#### Scenario: The map goes back, the consumption only grows
- **WHEN** the curve goes 1 → −1 (a turn in the middle), a curve with a plateau of zero speed
- **THEN** the position rises and returns, A is non-decreasing and A⁻¹(A(t)) = t; the splitting of the segment at zero is exact (checked against the numerical ∫|rate|); on the plateau the position and the consumption stand still, A⁻¹ gives the left edge of the plateau
- **TEST** t603a, t603b, t603c (`src/libs/kt-model/src/warp.rs`)

#### Scenario: Positive curves are bit-for-bit equal to the frozen oracle
- **WHEN** the curves lie entirely within 0.25…32
- **THEN** the position, the consumption and the inverse map match by `to_bits()` the copy of the old map that lies in the test; the span — `(0, used)`
- **TEST** t603d (`src/libs/kt-model/src/warp.rs`)

#### Scenario: The extrema of the span and the mirror axis
- **WHEN** the curve goes forward and back (a maximum inside the event); an event with the "Reverse" flag on a signed curve
- **THEN** the span finds the vertex inside the segment, a monotonic curve gives exactly the ends (a bit identity of the prefetch windows); the mirror is taken around the span and on positive curves collapses into the former formula
- **TEST** t603e, t603f (`src/libs/kt-model/src/warp.rs`)

#### Scenario: The clamp is symmetric, zero passes through
- **WHEN** a curve with keys outside ±32, a curve exactly 0, the parameter registry of the effect
- **THEN** the clamp −32…32, zero remains zero (the floor is a property of audio, not of the map), a curve with a minus is "signed", the modulus coverage sees zero INSIDE a segment; the spec `rate` — (−32, 32) and accepts −32, −0.03, 0, 0.05, 32
- **TEST** t603g (`src/libs/kt-model/src/warp.rs`), t190 (`src/libs/kt-model/tests/fx_auto.rs`)

#### Scenario: The format survives the sign (container: capability `project-format`)
- **WHEN** a project with a base of −0.5 and keys {1, 0, −2} is saved and read
- **THEN** the base and the keys come back without being pulled to the former minimum of 0.25; the write is idempotent
- **TEST** t606 (`src/libs/kt-otio/tests/convert.rs`)

### Requirement: Video at signed speed — reverse motion, freeze frame, source edge, order == read
The video SHALL go back through the material at negative speed and turn around
at zero; zero SHALL be a freeze frame (one key in the order for the whole event); at the start of the
file the frame SHALL hold, not go into negative; the blend pair SHALL be taken from the
same side as at positive speed; the window of the prefetch order SHALL coincide
with the render's read, including under a change of sign.
The END of the material is the same edge as the start, and for BOTH paths: an event that consumes
more material than the file contains (a stretch, a trim), beyond the end of the file SHALL hold
the last frame and play silence, and this MUST NOT count as an underrun either on the live or on the
offline path — an underrun is a miss of what EXISTS. The tail of an event without material SHALL
be visible on the event itself (hatching from the point where the material ended, taking the speed into account)
and be named in `kt-cli probe` ("material ends at … s, then silence/freeze frame"). The trim of the
right edge of an event SHALL stop at the end of material of known length, taking the speed into account
(as the left one stops at its start); events that already exist longer than the material MUST NOT
shorten themselves (rule 20) — they are shown and play as stated above, the limit for them is not less than their current
end (one may trim them down, but may not extend them further; the edge does not jump back).
Why: a window by the two ends would miss an excursion, and a cache miss would become eternal. R.63:
the events of Rolling Stones in paintitblack at rate 0.75 consume 237.7 s from a file of 226.3 s —
the last 11 s sounded as silence and were counted as underruns (180 480 frames in 5 s), and on the
screen the tail was not distinguishable from the material; the engine by the law of P does not know
the length of the media, so the edge is delivered by the source (the fetcher), not by the plan.

#### Scenario: Backward motion and a freeze frame
- **WHEN** the curve goes 1 → −1; the curve is exactly 0
- **THEN** ∫rate over the event equals zero (it came back), the second half strictly decreases; at zero all frames are the frame of the entry point, the order is one key
- **TEST** t604a, t604b (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: The edge of the material and the blend pair
- **WHEN** speed −1 from an entry point of 1 s; a fractional position under the minus
- **THEN** the frame does not go past the start of the file and holds at zero; the pair is (a, a+STEP) with a weight of ½
- **TEST** t604c, t604d (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: The order equals the read under a sign change, reverse on top of a minus goes forward
- **WHEN** a curve with an excursion (a maximum inside the window); the "Reverse" flag on a constant −0.5
- **THEN** the prefetch order ⊇ the render read, and the scene is not degenerate (the excursion guard); the mirror leads from the lower edge of the span to the entry point, the motion is monotonic
- **TEST** t604e, t604g (`src/libs/kt-engine/tests/video_warp.rs`)

#### Scenario: The end of the material — silence and a freeze frame without underruns, the tail is named
- **WHEN** an event with slowdown consumes more material than a synthetic source of known length lasts; it is played offline and through the player, the region loop stands on its tail; `kt-cli probe` is called on the tail
- **THEN** beyond the end of the material the audio is silence, the video is the last frame; `underruns == 0` and `warmup_misses == 0` on both paths (on the former code — hundreds of thousands of frames); the prefetch order ⊇ the read and does not grow past the edge; probe prints the moment where the material ended, and "then silence/freeze frame"
- **TEST** t748 (`src/libs/kt-engine/tests/material_edge.rs`), t748b (`src/apps/kt-cli/tests/cli.rs`)

#### Scenario: The trim stops at the end of the material, the existing is not shortened
- **WHEN** for an event with a known media length the right edge is pulled past the end of the material at speed 1 and at 0.5; a project is opened where the event is already longer than the material
- **THEN** the trim stops at the end of the material taking the speed into account and says so with an error (rule 16); with an unknown length (a generator, a reel, a file without duration) there is no stop; the existing event passes validation, does not change its length and can be trimmed freely, but does not grow further; its tail is drawn with hatching
- **TEST** t748c (`src/libs/kt-model/tests/trim_material.rs`), t748d (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: Audio at signed speed — modulus into the resampler, address by runs, hold below 0.01
The resampler SHALL receive `|rate|`; the read address SHALL be taken from the address map
of runs of constant direction with integer accumulation of positions. A `|rate|` below
`RATE_AUDIO_FLOOR = 0.01` SHALL be a hold: silence without consumption, the nodes
keep working (the reverb tail rings out); the decision is taken per chunk. A constant −1
goes into the material BEFORE the entry point and equals the "Reverse" flag only with a shifted
entry point — this is the definition. Why: the owner's decision "drag down to 0.01, silence only at zero"; a static mirror for the whole clip did not express a turn in the middle of an event.

#### Scenario: −1 equals "Reverse" with a shifted entry point, byte for byte
- **WHEN** "Reverse" at +1 from the entry point X and speed −1 from the entry point X + the length of the event
- **THEN** the outputs are byte-for-byte equal and are not silence
- **TEST** t616 (`src/libs/kt-engine/tests/reverse.rs`)

#### Scenario: The reverb sounds forward, the turn is a palindrome
- **WHEN** a chain with a reverb over a backward read; the curve 1 → −1
- **THEN** the reverb equals the reverb over the same input and does NOT equal the mirror of a finished forward render; around the turning point the output is a palindrome without a skipped or doubled frame
- **TEST** t617, t618 (`src/libs/kt-engine/tests/reverse.rs`)

#### Scenario: Zero — silence with a ringing-out tail
- **WHEN** the speed reaches zero after sounding material
- **THEN** the dry output is silence, and with the reverb the tail on the hold has a non-zero RMS
- **TEST** t619 (`src/libs/kt-engine/tests/reverse.rs`)

### Requirement: The playback "black box" is compact and written only by the render thread
The flight log SHALL be a ring of fixed size, deduplicate consecutive
events (`×N`), write a snapshot of the deltas once per second and only when the deltas are non-zero,
record the sticking `STALL` (playing, while the playhead stands still); it MUST NOT
enter the cpal callback. Why: the owner jumps across small loop regions, and the log must not produce "tons of logs".

#### Scenario: A thousand events do not produce a thousand lines
- **WHEN** 1000 wraps of one loop; jumps by 4×CAP across different regions; an empty snapshot
- **THEN** one record with `×1000`; no more than CAP records; in silence the log does not grow
- **TEST** t175 (`src/libs/kt-engine/tests/loop_fx.rs`)

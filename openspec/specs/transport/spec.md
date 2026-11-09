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

### Requirement: The audio output edge converts to the device's rate and channel layout

The engine's output stays f32 48 kHz/2ch (invariant 1). When the output device cannot be
opened at 48 kHz stereo (a WASAPI shared-mode device whose mix format is 44.1 kHz, or
5.1/7.1), the conversion SHALL happen only at the output edge. Rate: a band-limited
resampler whose state and buffers are allocated before the stream starts; no allocation
and no lock in the callback (invariant 6). Channels: L into front-left, R into front-right,
silence in every other channel; a mono device gets (L+R)/2. Playhead accounting SHALL stay
in engine frames: the playhead advances by the 48 kHz frames the edge consumed, not by
device frames. A 48 kHz stereo device MUST take the direct path with no conversion,
byte for byte as today.
Why: the fallback repeated or skipped samples positionally, so 44.1 kHz output aliased
audibly. Multichannel devices got the right channel copied into centre, LFE and every
surround. Both setups are rare on PipeWire and common on Windows, where shared mode
reports only the mix format.

#### Scenario: 44.1 kHz and 6-channel devices
- **WHEN** the edge converts 1 s of a 1 kHz stereo sine (L) and silence (R) for a 44.1 kHz stereo device, a 48 kHz 6-channel device and a 48 kHz stereo device, filling in callback-sized chunks of 441, 512 and 1024 frames under an allocation-counting allocator
- **THEN** 44 100 device frames consume between 48 000 and 48 000 + one resampler chunk + the kernel's two priming frames; the spectrum of the 44.1 kHz output peaks at 1 kHz and every other component is at least 60 dB down; the 6-channel output has the sine in channel 0 and exact zeros in channels 1–5; the stereo output is the kernel's output as before the edge, bit for bit; zero allocations happen on the filling thread during the fills (counted per thread, so another thread of the harness cannot blur it)
- **TEST** t857 (`src/apps/kt-app/src/audio_out.rs`)

### Requirement: A start begins with its sound
A start (opening, a seek, a change of direction, a loop flip) SHALL hold the render until the audio
of a whole ring and the look-ahead margin is in the cache, at most `START_AUDIO_GRACE`, and SHALL then
go on with what there is. The clock SHALL stand until the first frame reaches the output. A pass of
the callback before that first frame, with the ring empty, SHALL NOT count as an underrun.
Why: the render filled the ring in one pass when Play landed, ahead of the decoders: 0.34 s of silence
on the first play, on 1 start of 4 of the showcase (`docs/found-defects.md` row 11; cold-start-audio,
2026-11-04).

#### Scenario: A cold start
- **WHEN** a range plays with no warm-up from a source that takes 4 ms a burst
- **THEN** no frame of it is an underrun, and it plays in real time
- **TEST** t1537 (`src/libs/kt-engine/tests/preview_throughput.rs`)

#### Scenario: A source that does not answer
- **WHEN** a source does not answer until the play has ended
- **THEN** the start goes on after the grace, and the play ends with its underruns counted
- **TEST** t1538 (`src/libs/kt-engine/tests/preview_throughput.rs`)

### Requirement: The space bar plays while held from pause, reverses with Ctrl, and releases smoothly
A short press of Space (shorter than the hold threshold of about 0.3 s) SHALL toggle play/pause as
before, and two quick presses SHALL still stop at the region start (R.16: "don't break the functions of
the double and single press"). A press that STARTS while paused and is held past the threshold SHALL
play only while held. The press SHALL be held past the threshold to count, and its release SHALL pause
at the position reached. A press held DURING playback SHALL boost the speed as before (up to 4× over
5 s — limits set by the owner in R.16/R.17). Its release SHALL return to the base speed by a decay
whose time constant is shorter than the rise's, not by a jump; the rise SHALL start without a jump
(an ease-in). Ctrl+Space held SHALL play in REVERSE at the base speed, boosted the same way. The
same pause-on-release rule SHALL apply when it starts from pause. Its release during forward playback
SHALL return to forward base speed. The display of the speed SHALL show the temporary speed while it
lasts.
Why: the owner, `ktprompts_v26.10` USABILITY 2–4, and his answer to question 7 of R.71 ("a threshold
~0.3 s"). The release jumped from 4× to 1× in one frame. A press from pause kept playing after
release, so "look at this spot while I hold" was impossible.

#### Scenario: A short press and a double press keep their meaning
- **WHEN** Space is pressed and released within 0.1 s while paused; separately — pressed twice within 0.3 s while playing
- **THEN** the first plays and keeps playing; the second stops at the region start (at the project start without a region)
- **PATH** paused → tap Space → the playhead runs and keeps running → tap Space twice quickly → the playhead stands at the region start
- **TEST** t1620 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Held from pause plays only while held
- **WHEN** paused, Space is pressed and held 1 s, then released; separately — Ctrl+Space the same
- **THEN** playback starts after the threshold, the speed is boosted while held, and on release the transport is paused at the reached position (not at the start point); with Ctrl the playhead moved BACKWARD and stopped likewise
- **PATH** paused at 00:00:10 → hold Space → after a moment the picture plays and speeds up → release → the picture stops where it was, the play button shows "paused"
- **TEST** t1621 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: Ctrl+Space during playback reverses while held
- **WHEN** playing forward at 1×, Ctrl+Space is held 1 s, then released
- **THEN** while held the player's rate is negative (−1× and growing in modulus); after release it is +1× (the base) and playback continues
- **PATH** playing → hold Ctrl+Space → the picture runs backward, the speed strip shows a negative speed → release → it runs forward at 1× again
- **TEST** t1622 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The boost eases in and decays faster than it rose
- **WHEN** the boost is held 5 s during playback and released; the speed is sampled per frame with dt values 1/60, 1/30 and 0.1
- **THEN** the first sampled step above 1× is smaller than the later ones (no jump at the start); after release the speed is back within 2 % of the base in less time than the rise took to reach the same distance; no step diverges at any dt
- **PATH** playing → hold Space → the speed strip rises smoothly → release → it comes down to 1× quickly but without a jump
- **TEST** t1623 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: The arrows and Ctrl+click on Back/Forward step one frame
← and → SHALL move the playhead exactly one frame of the project fps grid back and forward. The step
SHALL land on an aligned tick, so a repeated step never sticks. Holding the arrow SHALL repeat the step.
During playback the first step SHALL pause, then step. Ctrl+click on the toolbar's Back and Forward
buttons SHALL run the same body (rule 17). Shift on Back/Forward and Shift+A/D keep the region-edge
drag (R.16). The arrows SHALL not act while a text field has focus.
Why: the owner, `ktprompts_v26.10` TIMELINE 11, and his answer to question 8 of R.71 ("Ctrl instead of
Shift? Or just arrows, if that is how other editors do it?"). The program had no frame step at all
(F-TL-003).

#### Scenario: One frame per press, from any place the person acts
- **WHEN** at a 30 fps project, paused at frame 100, → is pressed three times, ← once; separately — Ctrl+click on Forward; separately — → during playback
- **THEN** the playhead is at frame 102 (aligned), after Ctrl+click at frame 103; during playback the first → pauses and steps one frame from the paused position
- **PATH** paused → press → → the picture advances by one frame, the timecode +1 frame → press ← → back one frame → Ctrl+click the Forward button → one frame forward
- **TEST** t1624 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: A playback stall is one episode and reaches the bell
The engine SHALL record ONE stall per episode: a record when the playhead has stood still for 1 s while
playing at a nonzero rate, and no further record until the playhead moves again. At rate 0 (a freeze,
the shuttle centre) there SHALL be no stall. The application SHALL post one warning to the notices
(the bell) per new stall episode, naming the time position, in addition to the counter in the
performance panel.
Why: the owner, `ktprompts_v26.10` item 5 of the memory block: "what does 'playback froze: 5' mean? if
it is important it must be in notices, not somewhere in the corners". The engine pushed a record on every
loop pass of one freeze, so the counter overstated it and flooded the 512-entry ring.

#### Scenario: One freeze — one record, one notice
- **WHEN** a synthetic player plays, the head is held still for 3 s, then moves; separately — the rate is 0 for 3 s
- **THEN** the flight ring holds exactly one stall record for the first case and none for the second; the application posts exactly one warning with the position
- **PATH** there is no gesture: the freeze is produced by the test player → the bell gets one red entry "playback froze at 00:01:02"
- **TEST** t1625 (`src/libs/kt-engine/tests/flight_stall.rs`), t1626 (`src/apps/kt-app/src/layout_tests.rs`)

### Requirement: "Freeze frame here" holds the frame at the playhead and continues after it
The event's menu SHALL have the command "Insert frame hold (freeze frame)" for the event under the
playhead. The name follows the industry: Premiere's "Insert Frame Hold Segment", Resolve's "Freeze
Frame". It SHALL make three pieces in ONE undo step. The piece before the playhead stays unchanged. A
hold of 2 s (or up to the event's end, if shorter) shows the frame at the playhead at rate 0. The piece
after it continues from that same frame. The event's total length SHALL not change, and the material
that no longer fits is cut at the end. No other event SHALL move (rule 20). Linked A/V partners SHALL
follow as for any split, and the sound of the hold is silence. On an event with a speed curve the piece
after the hold SHALL start at the material of the frozen frame.
Why: the owner, `ktprompts_v26.10` "a convenient way to 'stop' an event: I want a freeze frame, and
then continue it" (S1). Rate 0 is a freeze since R.57, but making one took a split, a second split and
a hand-typed speed.

#### Scenario: The freeze keeps the length and continues from the frozen frame
- **WHEN** a 10 s event at constant 1×; separately — one with a speed curve; the playhead at 4 s; the command is run, then undone
- **THEN** three pieces: 0–4 s as before, 4–6 s at rate 0 showing the material of 4 s, 6–10 s starting at the material of 4 s; the neighbour events are where they were; one undo returns the single event byte-equal
- **PATH** right-click the event under the playhead → "Insert frame hold (freeze frame)" → the event shows a 2 s piece with a frozen frame, play → the picture stops for 2 s and goes on from the same frame
- **TEST** t1627 (`src/apps/kt-app/src/reach_tests.rs`), t1638 (`src/libs/kt-engine/tests/clip_gain_floor.rs`)

### Requirement: A paused frame waits for what it lacks
A frame that came out incomplete SHALL be composed again when the layers it lacked are in the cache,
told by an insertion (`BurstCache::on_next_insert`), or on a pause that doubles from 250 ms and ends
at 8 s. It is NOT composed again because a UI frame happened.
Why: `need` held `!was_complete`, and `last_compose_retry` was written and never read. Each move of
the mouse over a picture that lacked something paid the whole chain on the UI thread, and a chain of
a slow effect made the interface wait for it (E2, E8).

#### Scenario: A moving pointer over an incomplete frame
- **WHEN** the frame is incomplete, nothing arrives, and the pointer moves over 40 UI frames
- **THEN** at most one composite runs
- **TEST** t1832 (`src/apps/kt-app/src/idle_tests.rs`)

#### Scenario: The next insertion is heard once
- **WHEN** a waiter is armed and two bursts are inserted
- **THEN** it is called once
- **TEST** t1836 (`src/libs/kt-cache/src/lib.rs`)

### Requirement: An edit that changes nothing reaches no engine
A command or a batch whose result equals the canon SHALL NOT be an edit: the model pushes no undo
step, starts no coalesced gesture and keeps its revision (`TimelineModel::revision`), and the app's edit
doors (`apply`, `apply_batch`, `apply_coalesced`) then bump no revision, hand the player no project and
mark nothing dirty. A real change SHALL still be exactly one edit.
Why: every edit door handed the player the whole project whether or not anything changed, and a swap
rebuilds the audio renderer on the audio render thread and both renderers on the prefetch thread. A
window that re-sent what the model held each frame turned every mouse move into that rebuild
(kkt-26.11.07b G5/G7).

#### Scenario: The model takes an equal value as no edit
- **WHEN** a gain equal to the held one is sent through `apply_coalesced`, `apply` and `apply_batch`, then a different gain under the same gesture token, then an undo
- **THEN** the equal value moves neither the revision nor the undo depth, the real change is one step and one revision, and the undo is a revision
- **TEST** t1873 (`src/libs/kt-model/tests/parity_ops.rs`)

#### Scenario: The app's doors take an equal value as no edit
- **WHEN** an event's chain is sent back unchanged through the three doors, then with one value changed
- **THEN** the unchanged chain moves no revision, undo step or player project and marks nothing dirty; the changed one does each once
- **TEST** t1874 (`src/apps/kt-app/src/input_work_tests.rs`)

### Requirement: The UI's own audio renderer is built once per project version
The renderer the UI thread uses (the levels under the cursor on a pause, the pinned head of the loop
region) SHALL be the player's shared build of the version (see "One build per project version, shared
by every consumer"); region changes and meter readings SHALL build nothing, and a new version SHALL be
built once for every consumer.
Why: each change of the loop region built a whole audio renderer on the UI thread, 447 ms on the owner's
1974 events, and the first meter reading after an edit built another.

#### Scenario: Region changes and meter readings on one version
- **WHEN** the loop region changes five times and the level is read five times on one project version, then the project is swapped and the region changes twice
- **THEN** nothing is built for the first five, and the new version is built once (an audio and a video renderer)
- **TEST** t1875 (`src/libs/kt-engine/tests/probe_renderer.rs`)

### Requirement: An edit during playback does not stop the sound
A model edit SHALL rebuild the audio renderer on a builder thread, never on the audio render
thread; while the transport plays, the render thread SHALL keep filling the ring with the renderer it
has until the new one arrives, then carry the FX state over (invariant 3) syncing only the events
whose fingerprint changed (`sync_fx_since`). A build asked on a pause SHALL hold the next start until
it arrives. The first model is built once. A start, the playback look-ahead and the background prefetch SHALL NOT order sources for the
windows the final-mix store holds.
Why: on the owner's project `AudioRenderer::new` takes 412 ms against a 341 ms ring; every edit
during playback stopped the sound (his load report: 12–16 bursts a second instead of 47 at those
moments, kkt-26.11.07b). A covered range must start and play with no decode. Under a loaded test run
the swap itself (a sync of every event, a reel's pre-roll asked per placement) took 244–425 ms on the
render thread and the sound stopped for a burst; a pause's inline build delayed the next Play by its
5–7 s.

#### Scenario: A rebuild syncs what the edit changed
- **WHEN** one of 50 events with a reverb changes its gain and the renderer is rebuilt
- **THEN** one event is synced, the rest keep their processors, and the sound after the swap equals a full sync's
- **TEST** t1940 (`src/libs/kt-engine/tests/mix_store.rs`)

#### Scenario: Edits while a covered loop plays
- **WHEN** a large project (a reel placed 400 times, each rebuild longer than the ring) loops a short range for 8 s and four edits arrive
- **THEN** there is no callback underrun and no render underrun, and after two laps every burst comes from the kept mix
- **TEST** t1935 (`src/libs/kt-engine/tests/mix_player.rs`)

### Requirement: A short loop over a stream stage plays, every decoded frame read
A loop over an event with a stream stage whose history does not fit the cache SHALL come to show frames
(V-fps above 0) and SHALL NOT drop a decoded frame before it is read; the decoders' work SHALL stay bounded
by a few times the history, not grow with the time the loop plays.
Why: the owner, kkt-26.11.08 line 14 (registry H8): "а loop-picture-throughput может связан с тяжелым
эффектом? типо я поменял там что-то и оно пошло считать?" — four loops in a row "V-fps 0.0, new frames 0"
after he selected Datamosh, ~20 000 requests a second (decode-request-storm).

#### Scenario: A cold loop over a cut mosh plays on model decoders
- **WHEN** an 8-frame loop 3 s into a cut mosh (144 lines, a 103-frame history) plays cold for 10 s through the real player with a 4 MB video cache, on model decoders that take 2 ms a picture and count their seeks
- **THEN** frames are shown, no entry is dropped before its first read, and the pictures decoded are at most four times the history plus eight GOPs
- **TEST** t2044 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A person plays a short loop over a Datamosh event
- **PATH** open the heavy showcase whose longest plain event carries a cut mosh at 1080 lines and a 0.4 s loop 3.3 s into it → the preview stands on the loop's start → press Space → the loop plays, the picture of the mosh comes and moves
- **WHEN** the live scenario runs on our fixtures
- **THEN** no 3 s window asks the decoders for far more than they decode, the loop shows frames, and the screenshots of each step are saved
- **TEST** t2045 live `decode_storm.py` (`src/scripts/live/decode_storm.py`, via `src/scripts/live-check.sh`)

### Requirement: No interface action waits for a picture
The preview SHALL compose on a thread of its own: the UI thread asks for a frame and goes on, the
newest ask replaces a waiting one, one composite runs at a time, and a frame composed for a project
state the model has since left SHALL NOT land. The UI frame SHALL wait for an answer at most a few
milliseconds, and without one it shows the last picture. A chain whose executor time passes
`SLOW_CHAIN` SHALL compose on the offline device and come back as bytes, so the display's queue never
stands behind it; the first frame of each new project state SHALL be timed there; the plan's own time
(a stream stage's kernel on the compute host's device) SHALL NOT count. Undo, every window and the
transport SHALL answer while a composite takes seconds.
Why: the owner, kkt-26.11.07 E8: an anomalous Datamosh parameter lagged the whole interface until "Я
просто БУКВАЛЬНО НЕ МОГУ ОТМЕНИТЬ СВОЙ ЖЕ ЭФФЕКТ". His release journal (kkt-26.11.08 H9): UI frames of
0.3–4.5 s during playback at a quarter of a core. The composite ran in the UI frame, and a stream stage
decodes the event's history inside the plan, one compute job a frame, each waited for.

#### Scenario: A stuck composite holds nobody
- **WHEN** the executor's composite stands until the test lets it go, and meanwhile the UI asks for a frame, the project is swapped, four more frames are asked and the renderer is queried
- **THEN** every call returns; after the release only the newest ask is answered, and two composites ran, not five
- **TEST** t1840 (`src/libs/kt-engine/tests/compose_queue.rs`)

#### Scenario: A finished frame wakes the UI
- **WHEN** a frame is asked and the executor answers
- **THEN** the wake the UI set is called once and the frame is taken without waiting
- **TEST** t1840b (`src/libs/kt-engine/tests/compose_queue.rs`)

#### Scenario: A slow chain stays off the display's device
- **WHEN** a new project state is composed, then timed fast, then an edit is timed slow twice, then fast
- **THEN** the new state goes to the offline device first, the fast chain to the display, the slow one stays off it until it is fast again
- **TEST** t1841 (`src/libs/kt-engine/src/compose_queue.rs`)

#### Scenario: Undo acts while the kernel is stuck
- **WHEN** an effect is added while the executor's composite never ends, five frames run, and Undo is called
- **THEN** the frames return, Undo removes the effect at the next frame, the frame of the state Undo left is asked for, and its answer lands once the executor is released
- **PATH** add an effect whose frame never finishes → the picture stays as it was; press Ctrl+Z → the effect is gone at once, the picture follows when it is ready
- **TEST** t1842 (`src/apps/kt-app/src/ui_wait_tests.rs`)

### Requirement: The UI's own audio renderer of a new version is built off the UI thread
After the first, the renderer the UI thread uses (the meters on a pause, the loop's pinned head) SHALL
come from the player's builder thread, once per version, and its FX states SHALL be synced there (only
the events whose fingerprint changed); the one before SHALL answer meanwhile, and the loop region's head
SHALL be pinned again when the new one lands.
Why: the build takes 385–447 ms on the owner's project (found-defects 111, 117), and these are asked
on the UI thread by every meter reading and every change of the loop region after an edit.

#### Scenario: The meters and the loop region while the build is held
- **WHEN** after an edit the builder is held at its start, and the loop region changes three times and the meters read three times
- **THEN** every call returns, the held build has not landed, and once released the probe of the new version lands
- **TEST** t1844 (`src/libs/kt-engine/tests/probe_async.rs`)

### Requirement: The UI thread waits for no lock held through long work
No lock the UI thread takes SHALL be held by another thread through an engine build, a decoder's
join, a file's opening or the executor's composite: the project lock is released before the audio
render thread builds, the source facts are cloned out before the prefetch thread builds, a decoder is
joined after the workers' write guard is gone, the thumbnail worker opens a file with the media map
free, and the display target is published apart from the engine's lock. The render window SHALL find
the largest source on a thread, keyed by the files.
Why: a Rust guard taken inside an expression lives to the end of the statement, so
`AudioRenderer::new(&project.lock().unwrap().clone())` held the project lock through a 412 ms build,
and the UI's next edit (`set_project`) waited for it; the same form held the workers' map through a
decoder's join and the thumbnail map through a file's opening.

#### Scenario: The display target while the engine is held
- **WHEN** a frame is presented, then the engine is held as a composite holds it, and the display target and the glow are asked
- **THEN** both answer at once with the presented target
- **TEST** t1845 (`src/libs/kt-gpu/tests/preview_display.rs`)

### Requirement: One build per project version, shared by every consumer
A project version (or the same version with new source facts) SHALL be built once, as one audio and
one video renderer, by the player's `engine-build` thread, and published. The audio render thread,
prefetch, the probe (the meters on a pause, the loop region's pinned head) and the preview composer
SHALL all read that build; none SHALL build its own. Only the newest version handed over is built;
the ones in between are never built.
Why: one edit was built 2 audio + 2–3 video times (the UI's video build, the `audio-build` thread,
prefetch's pair, the probe; found-defects rows 110, 111, 150): on the owner's 1974 events a slider
step cost ~197 ms of CPU and 5 renderer builds (release, `kt-cli edit-cost`).

#### Scenario: Five edits while playing
- **WHEN** a playing player of 400 events takes five edits, and after each the meters are read, the loop region is set, and the probe and prefetch take the new build
- **THEN** ten renderers are built in the process, five audio and five video
- **TEST** t2000 (`src/libs/kt-engine/tests/shared_build.rs`)

### Requirement: Nothing on the UI thread builds or waits for a build
Handing a project to the player (`set_project` with the model's shared `Arc<Project>`), a meter
reading, a loop region change, the grid and coverage questions and a preview ask SHALL build no
renderer, derive no event and resolve no reel on the calling thread, and SHALL return while the
builder is held. The opening's first build is the one exception (the player's constructor). The app
SHALL hand the model's shared snapshot of the state (`Model::doc_arc`), not a copy.
Why: invariant 12 and the owner's rule "no interface action waits for a picture" (2026-11-07). The
UI thread spent 88 ms (p50) per slider step on the owner's project: a video build and the generator
sources, which resolve every reel.

#### Scenario: The UI's calls while the build is held
- **WHEN** the builder is held and ten edits are handed over, each followed by a meter reading, a region change, the grid, the decode qualities, the reel coverage and a preview ask
- **THEN** the calling thread counts no renderer build, no derived event and no reel pass, the held build has not landed, and once released the build of the newest version lands
- **TEST** t2001 (`src/libs/kt-engine/tests/shared_build.rs`)

#### Scenario: Edits through the app's doors
- **WHEN** five steps of an effect's parameter go through `apply_coalesced`, a frame drawn after each
- **THEN** each reaches the player, the UI thread builds, derives and resolves nothing, and the player holds the model's snapshot itself
- **PATH** an event with VHS Tape, its effects window open → step the carrier-to-noise field five times: the value moves each time, the picture follows, the window answers at once
- **TEST** t2008 (`src/apps/kt-app/src/input_work_tests.rs`)

#### Scenario: A slider dragged in the effects window while a big project plays
- **WHEN** the heavy showcase plays and the VHS Tape dark clip field of the composition's effects window is dragged to and fro four times
- **THEN** there is no callback underrun, at most one shared build (two renderers) is built per edit, and a UI frame of the drag costs the UI thread within twice a hovering frame plus 10 ms
- **PATH** the heavy showcase open → right click the composition → "Effects…" → click the VHS Tape slot: the deck face and its grid → scroll to "Dark clip" → Space: the picture plays → drag the field left and right four times: the value follows the pointer, the picture and the sound go on → Space: stopped, nothing runs
- **TEST** t2007 live scenario `src/scripts/live/edit_drag.py`

### Requirement: A loop region change is not an edit
A change of the loop region SHALL build nothing and move no project version: the region's head is
pinned with the renderer already built, and pinned again by the builder when a new version lands.
Why: found-defects row 117 — a region change built a whole audio renderer on the UI thread (412 ms
on the owner's project, ~12 ms after builder-cost-guard).

#### Scenario: Twenty region changes
- **WHEN** the loop region changes twenty times and is cleared
- **THEN** no renderer is built and the project version stands
- **TEST** t2002 (`src/libs/kt-engine/tests/shared_build.rs`)

### Requirement: A frame is composed by the renderer of its state
A preview composite (and the recheck of the kept moments) SHALL use the build of the newest state
handed over; when that build has not landed, the compose thread SHALL wait for it, never the UI
thread. The UI's readers of the renderer (the grid, the coverage bars) read the last published build,
and the bars' cache is keyed by it.
Why: a composite of the old state under the new version would be shown as the edit's picture, and the
bars would mark a moment "plays without work" by the old plan's key.

#### Scenario: A new frame rate while its build is held
- **WHEN** the builder is held, the project's frame rate changes from 25 to 30 and a frame at 5000 ticks is asked
- **THEN** the ask returns at once, nothing is composed while the build is held, and after the release the frame is composed on the 30 fps grid (3000 ticks)
- **TEST** t2006 (`src/libs/kt-engine/tests/shared_build.rs`)

### Requirement: Space read by a slow frame keeps its meaning
The play key's presses and releases SHALL be handled in the order a frame read them. A hold SHALL be
proven: a frame that began `SPACE_HOLD_MS` after the frame that read the press finds no release in its
input; a release of a press never proven held is a tap. The double-press window SHALL be measured
between frame starts and widen by the busy frame the program measures.
Why: under the full live run `space_hold` read its two quick presses as one (a pause, not the stop at
the start), and a tap whose release a slow frame read past the threshold did nothing at all, neither a
tap nor a hold (found-defects 99, 166). The program cannot tell 150 ms from 450 ms inside a 450 ms
frame; it can keep from deciding against the person on that.

#### Scenario: A tap read late and two presses in one frame
- **WHEN** the press is read by one frame and its release by a frame 400 ms later; then one frame reads
  two presses
- **THEN** the tap plays (no boost); the two presses stop at the project start
- **TEST** t2212 (`src/apps/kt-app/src/layout_tests.rs`)

#### Scenario: The double press on busy frames
- **WHEN** playback runs on frames 450 ms apart and the second press is read 450 ms after the first; then
  on fast frames a second press comes 2 s after the pause
- **THEN** the first pair stops at the project start; the second press resumes
- **TEST** t2213 (`src/apps/kt-app/src/layout_tests.rs`); live `space_hold`

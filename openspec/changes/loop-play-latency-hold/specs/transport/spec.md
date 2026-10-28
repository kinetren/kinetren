## MODIFIED Requirements

### Requirement: Video display clock is interpolated and compensates latency
The DISPLAY position SHALL be interpolated by the wall clock between the steps of the master clock
(the clamp — the size of the last quantum), subtract the smoothed output latency,
scaled by speed (`latency · |rate|`, in reverse — added with a plus sign), and pass through a
ratchet: rolling the key back is allowed only at a seek / re-anchoring. The raw playhead of the transport MUST NOT
be read for display during playback. Why: the callback quantum (PipeWire up to 42.7 ms) limited the display to 24 frames/s,
and an unscaled latency at 0.25× rolled the picture back.

Re-anchoring under latency. The ratchet SHALL be released by exactly two events: a jump of the
transport (a seek, a pause/resume, a change of the speed sign or of the loop region) and a change
of the DISPLAY position's lap — the moment the shown frame itself folds from the end of the loop
region back to its start (in reverse — from the start to the end). The wrap of the transport head
MUST NOT release the ratchet: under a latency `L` the head wraps `L · |rate|` frames BEFORE the shown
frame does, and at the head's wrap the shown frame is continuous. The display lap SHALL count only
forward (in the direction of play) within one transport generation, so a latency that jitters around
the fold releases the ratchet at most once per lap. After a jump to a target `p` the shown frame
SHALL be `p` until the first audible frame of `p` arrives (until the head has gone `L · |rate|` past
`p`) and the head minus the scaled latency after that; it MUST NOT fold into the previous lap of the
region — there was no previous lap. These rules hold for every latency, including one larger than a
UI frame and larger than a callback quantum.
Why: the release was tied to the head's wrap counter; under wine/WASAPI (latency of a whole device
buffer plus the 44.1→48 kHz edge resampler) Play from outside a looped region showed the region's last
frames, refused the fold back to its start as a rollback and froze the picture until a click; t117 ran
with zero latency and saw nothing (kkt 2026-10-27 item 1).

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

#### Scenario: The second lap of the loop does not freeze at zero latency
- **WHEN** the live player plays a region of 0.5 s from its start for two laps with zero output latency
- **THEN** the transport counts the wraps without bumping the generation, the display key returns to the start of the region after the jump and moves on
- **TEST** t117 (`src/libs/kt-engine/tests/frame_order.rs`)

#### Scenario: The display lap changes when the shown frame folds, not when the head wraps
- **WHEN** the display position is computed for a head running through a region of 48 000 frames for three laps at latencies 0, 512, 4 800 and 14 400 frames, forward and in reverse, with the latency jittering by ±512 frames around the fold
- **THEN** the reported display lap changes exactly once per lap, at the head position `region start + latency` (in reverse — `region end − latency`), never at the head's wrap; at latency 0 both moments coincide
- **TEST** t900 (`src/libs/kt-engine/src/player.rs`)

#### Scenario: Play from outside a looped region under latency
- **WHEN** a deterministic clock (no wall clock) plays from outside a region: the cursor stands before the region, Play seeks to its start, callbacks of 480, 1 024 and 2 048 frames advance the head, the UI samples at 60 Hz, the latency is 0, 512, 1 024, 2 048, 4 800, 9 600 or 14 400 frames, four laps are played
- **THEN** for every pair (quantum, latency) the display key never stands still for longer than one grid cell plus two UI frames while the head moves; after each fold the key is at the region start within one cell; the ratchet fires at most once per lap; with the release by the head's wrap the same run holds the key for most of a lap (the red proof)
- **TEST** t901 (`src/libs/kt-engine/tests/frame_order.rs`)

#### Scenario: After a jump the shown frame starts at the target
- **WHEN** a seek to `p` happens at latency `L` — `p` the start of the region (Play from outside), `p` inside it (a click), `p` outside it; forward and in reverse; at speed 1 and 0.25
- **THEN** until the head is `L · |rate|` past `p` the shown frame is `p`, never a frame from the end of the region; after that it is the head minus the scaled latency; the first fold of the shown frame happens only after the head itself has wrapped once
- **TEST** t902 (`src/libs/kt-engine/src/player.rs`)

#### Scenario: A region change during loop playback under latency
- **WHEN** the deterministic clock of t901 plays a region at latency 4 800 frames and, mid-lap, the region end is pulled to the left of the head, the region is moved by its body, and a new region is drawn elsewhere
- **THEN** each change re-anchors the display at the transport's new position; the key never stands still longer than in t901; the shown frame never belongs to the region that no longer exists
- **TEST** t903 (`src/libs/kt-engine/tests/frame_order.rs`)

#### Scenario: The live player loops under a large latency
- **WHEN** the live player (as in t117) gets an output latency of 9 600 frames (200 ms) before Play, plays a region of 1 s from a cursor placed before the region for three laps, and the UI loop releases the ratchet by the display lap
- **THEN** after every fold the display key is again within six cells of the region start and moves on; the number of refused rollbacks is at most one per lap
- **TEST** t904 (`src/libs/kt-engine/tests/frame_order.rs`)

#### Scenario: The preview follows the loop when Play starts outside the region
- **WHEN** the harness app has a project of two coloured clips, a region is dragged over the ruler, the cursor is placed before the region, the player's output latency is set to 9 600 frames, and Space is pressed; three laps are played
- **THEN** the preview key changes on every lap and returns to the region start after each fold; the perf panel's rollback counter grows by at most one per lap; the toolbar timecode and the timeline cursor stay inside the region
- **PATH** drag on the ruler from 2 s to 4 s → the region is shaded and Loop is on → click the ruler at 1 s → the cursor stands left of the region → press Space → the preview shows the frame at 2 s, the picture moves with the sound to 4 s and jumps back to 2 s on each lap; nothing freezes and no click is needed → open the perf panel → "V skipped" shows the rollback count of at most one per lap
- **TEST** t905 (`src/apps/kt-app/src/layout_tests.rs`)

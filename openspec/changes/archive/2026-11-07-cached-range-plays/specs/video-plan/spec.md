## MODIFIED Requirements

### Requirement: Memoization of the composite and its key
A composed frame SHALL be memoized in the preview as a function of the whole plan, keyed by the plan key: on the readback path in the executor's readback memo, on the display path in the executor's present memo (`PresentMemo`), where presenting a held key runs only the display pass (`GpuStages.replayed`, the work row `composite_replays`). Only complete frames are kept; the present memo is bounded (512 MB), least recently shown first. The key SHALL include the hash of the chain with its
curves, the geometry of the clip, the resolved camera (it is selected per frame and is not visible to
a static hash), the masks of the frame, the decode step of the input, the parameters of the
transition kind, the step of the reel and the blend flag. Export MUST NOT memoize (each t is
composed once).
Why: any parameter that affects the bytes and is not in the key is a stale value, which shows up as "did not apply". preview-throughput D2 moved the preview to the display path, which had no memo: a covered range was composed again on every lap — 16 V-fps on the owner's project (kkt-26.11.07b, line 3).

#### Scenario: The step is in the key
- **WHEN** one clip at one Quality is composed from input 1× and from input 2×
- **THEN** the two frames differ, and repeats return each its own bytes
- **TEST** t313 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: Transition parameters in the GPU plan key
- **WHEN** each transition kind is executed at several phases, both axes of direction, and both states of the flags
- **THEN** the frames are within tolerance of their stored frames (volumetric ones — `GEOM`); a transition frame of another kind is not returned from the plan cache
- **TEST** t224 (`src/libs/kt-gpu/tests/parity.rs`)

#### Scenario: A presented composite is kept
- **WHEN** ten moments are presented twice, then one event's level changes and they are presented again
- **THEN** the second presents are kept composites with nothing composed and the same picture; after the edit exactly the moments of the edited event compose again
- **TEST** t1934 (`src/libs/kt-gpu/tests/preview_display.rs`)

### Requirement: The cache is the only truth about coverage
The coverage bars SHALL be `coverage()` of the cache by the DECODE quality of the clip (for a clip cut by a morpher layer — the union of the qualities of the pieces), and not a separate bookkeeping and not a poll by the preview's Quality. Memoized composites MUST NOT be visible to
prefetch and pin. Over the decoded ranges the bars SHALL draw what plays without work: the moments whose presented composite the executor still holds and the current plan still asks (`Player::final_video_runs`), and the frames the audio render thread's final mix holds (`Player::final_audio_runs`).
Why: the cache is quality-exact; a clip with a step, warmed properly, would look entirely cold when polled by Quality — a lie in the user's eyes. A decoded range still needs its composite and its effects: the bars promised a free play that was not (kkt-26.11.07b, "не рабочий кэш").

#### Scenario: The bars ask about the decode quality
- **WHEN** prefetch has put the frames of a clip with a step strictly by the order
- **THEN** the coverage by the decode quality is non-empty, by the preview's Quality it is empty
- **TEST** t317 (`src/libs/kt-engine/tests/decode_scale.rs`)

#### Scenario: The bars show what plays without work
- **WHEN** a range is played once with a device and an edit then changes one event in it
- **THEN** the bars' final layer holds the played moments and frames, and after the edit the edited event's moments leave it while the others stay
- **TEST** t1936 (`src/libs/kt-engine/tests/final_runs.rs`)

#### Scenario: A covered loop plays from what is kept, live
- **PATH** open the heavy showcase with a half-second loop over the composition → press Space → the first lap composes, then the strips over the loop turn bright → the picture plays at the project's rate, no frame is composed, no burst runs the effects, nothing is decoded, no underrun → press Space → nothing runs
- **TEST** live scenario `src/scripts/live/cached_range.py`

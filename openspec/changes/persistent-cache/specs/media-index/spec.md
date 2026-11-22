## ADDED Requirements

### Requirement: A source copy is an intra-frame copy as long as its original
A source copy in the persistent cache SHALL be written by an offline job with its own short-lived source as an MKV whose
video is H.264 with every frame a keyframe at the size the adaptive rule asks, whose pictures carry the original's
own timestamps one for one, and whose audio is the original's audio packets copied without re-encoding; it SHALL lie
in the project's cache folder. The job SHALL run at background priority, pause while the transport plays or an export
runs, be cancellable at any moment leaving no partial file, and record its binding keyed by the original's identity.
A camera's copy SHALL be accepted only when its duration and frame count match the original within one frame and its
audio length within one burst; otherwise it SHALL be refused naming what differs.
Why: invariant 5 (offline sources); F-MED-002 — a grid-timed copy drifts on fractional rates; the owner's A14 — the
caches live in the project's folder.

#### Scenario: A copy of fractional-rate footage matches the original
- **WHEN** a copy is made of a 23.976 fps, 10-bit HEVC fixture with a 5.1 audio stream, and of a VFR fixture
- **THEN** every picture's pts equals the original's, the duration equals the original's to the tick, every frame is a keyframe, the audio decoded from the copy is byte-equal to the original's, and the file lies under the project's cache folder
- **TEST** t3440 (`src/libs/kt-media/tests/persistent_cache_copy.rs`)

#### Scenario: Cancel and pause leave no trace
- **WHEN** a copy job is cancelled halfway, and another is running when Play is pressed
- **THEN** no `.part` or copy file remains and no binding is written; the running job makes no progress while playing and resumes on pause
- **TEST** t3441 (`src/libs/kt-media/tests/persistent_cache_copy.rs`)

#### Scenario: A mismatched camera copy is refused with its reason
- **WHEN** a camera copy one second shorter, and one with a different frame count, are offered
- **THEN** both are refused, the message names the duration and the frame count respectively; a matching one is accepted and bound
- **TEST** t3442 (`src/libs/kt-media/tests/persistent_cache_copy.rs`)

### Requirement: A source copy's size follows the preview and only heavy sources get one
In the mode "As in the preview" a file's wanted copy size SHALL be its decode step at the current preview quality,
clamped to at most 1/2 and at least 1/8 of its native; at a preview quality of 1/1 the original SHALL play. A copy at
least as large as wanted SHALL be used; a smaller one SHALL be used while a background job makes the wanted size, and
SHALL be deleted after the swap; lowering the quality SHALL start no job. In the modes 1/2 and 1/4 the size SHALL be
that fraction; in the mode Off no copy SHALL be read. Without a click, only costly files SHALL get a copy: a long-GOP
codec above 1920×1080, or one the player measured decoding slower than real time at the wanted size.
Why: the owner, A10 — "как играем в превью так и храним? и если выбрали лучшее качество то рендерим лучшее чем есть".

#### Scenario: Raising the quality makes a better copy, lowering costs nothing
- **WHEN** a costly 4K file plays at preview 1/4 with "As in the preview"; the quality is raised to 1/2; then lowered to 1/8; then raised to 1/1
- **THEN** a 1/4 copy is made and read; at 1/2 the 1/4 copy keeps playing until a 1/2 copy is ready, then the 1/4 file is gone; at 1/8 no job starts and the 1/2 copy is read at 1/8; at 1/1 the original plays; a cheap intra-frame file never gets a copy
- **TEST** t3449 (`src/libs/kt-engine/tests/persistent_cache.rs`)

### Requirement: Reading a source copy changes sharpness, never framing or sound
A source opened on a source copy SHALL report the original's native size, pixel aspect, frame rate and duration, and
deliver pictures at the copy's size as a lower decode step; conform, geometry, masks and the frame plan SHALL be the
same with and without copies. Switching SHALL replace the sources through the one swap path without moving the
playhead or resetting the effects' state, and SHALL clear the replaced sources' cache, kept composites and kept final
mix by the source's epoch. A composite made from a copy SHALL never be stored under the key of a composite of the
original, in memory or on disk.
Why: invariant 4 — one truth of coverage; invariant 7 — the plan is the same; a disk tier keyed by the original file
would replay copy frames after a restart.

#### Scenario: The same plan, a softer picture
- **WHEN** a project with a fit-conformed 4K event, a mask and a corner-pin is planned with the original and with a 1/2 copy at preview 1/1
- **THEN** the layer geometry, masks and the fx chain of the plans are equal; only the decode size differs; the composite is within `FrameTol::BILINEAR` of the original's upscaled 1/2 decode
- **TEST** t3443 (`src/libs/kt-engine/tests/persistent_cache.rs`)

#### Scenario: Toggling while paused and while playing
- **WHEN** source copies are switched on and off during playback of a slowed-reverb event and on a pause
- **THEN** the playhead does not jump, the audio has no reset (the effects' state objects are the same), the coverage bars of the swapped sources empty and refill, the other sources keep theirs
- **TEST** t3444 (`src/libs/kt-engine/tests/persistent_cache.rs`)

#### Scenario: The disk tier keeps copy and original apart
- **WHEN** a slow moment is prerendered to the disk tier with copies on, the player is restarted with copies off
- **THEN** the moment is not restored from the copy's entry; it composes from the original and keeps its own entry
- **TEST** t3446 (`src/libs/kt-engine/tests/persistent_cache.rs`)

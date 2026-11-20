## ADDED Requirements

### Requirement: A speed gives speed, acceleration and speed-change channels to links
A link SHALL be able to read, from an event's warp, a composition instance's warp or a morpher layer's warp, its
speed, its acceleration (the slope of its speed per second of its own time) and a speed-change value (a peak-hold of
the absolute acceleration scaled by a full-at value with an attack and a linear release), each a pure function of the
speed curve at the tick: the same value in playback, scrubbing, a seek and an export.
Why: the owner, v26.10 TD2 — a fisheye that wakes on acceleration and braking, without copying another event's curve.

#### Scenario: The fisheye follows a composition's braking
- **WHEN** a Lens strength is linked to the speed change of a composition instance whose speed goes 1 → 0.3 over 1 s, holds, then back to 1; the range is rendered, then sought into the middle of the release and rendered again
- **THEN** the strength is 0 during steady speed, rises during the change, falls linearly over the release after it; the value at every tick equals the closed form; the seek render equals the continuous render byte for byte
- **TEST** t3947 (`src/libs/kt-model/tests/motion_links.rs`), t3948 (`src/libs/kt-engine/tests/motion_links.rs`)

### Requirement: The playhead's motion is a preview-only input
The transport speed and the rewinding value SHALL be readable by links in the preview only; every offline render
SHALL read normal playback (speed 1, rewinding 0); a frame whose plan reads them SHALL be kept only under a key with
the transport value and SHALL never be stored on disk; paused, they SHALL read normal playback.
Why: the owner — "в момент перемотки тот же участок … уже будет с fisheye"; invariants 4 and 11.

#### Scenario: Rewound in the preview, plain in the render
- **WHEN** a fisheye linked to rewinding is scrubbed fast over t0–t1, played at 1× over it, paused inside it, and the range is exported
- **THEN** the scrubbed frames show the fisheye, the 1× and paused frames do not, the export equals a render without the link; no kept composite of a scrubbed frame is shown at 1×
- **TEST** t3949 (`src/libs/kt-engine/tests/motion_links.rs`)
